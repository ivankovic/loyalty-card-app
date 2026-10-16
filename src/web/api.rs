/*  This file is part of Loyalty Card App.
 *
 *  Copyright (C) 2026 Marko Ivankovic
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU Affero General Public License as published
 *  by the Free Software Foundation, version 3 of the License.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 *  GNU Affero General Public License for more details.
 *
 *  You should have received a copy of the GNU Affero General Public License
 *  along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

//! The routes and their handlers. Each handler is the HTTP side of a function in `accounts`,
//! `cards` or `photos`: it takes the request apart, calls it on the database thread, and turns
//! the result into JSON and a status.

use super::{AppState, Viewer, bytes, client_ip, cookie_value, error, internal};
use crate::accounts::{self, Refused, User};
use crate::cards::{self, CardError, CardInput};
use crate::db::now;
use crate::photos::{self, Side};
use crate::throttle::Throttle;
use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;

/// The largest JSON body. A card with the longest notes and code is under 10 KB.
const MAX_JSON_BYTES: usize = 64 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/me", get(me))
        .route("/api/me/password", post(change_password))
        .route("/api/household", get(household))
        .route("/api/invites", post(create_invite))
        .route("/api/invite/{token}", get(invite_info).post(accept_invite))
        .route("/api/cards", get(list_cards).post(create_card))
        .route(
            "/api/cards/{id}",
            get(get_card).put(update_card).delete(delete_card),
        )
        .route("/api/cards/{id}/used", post(card_used))
        .route(
            "/api/cards/{id}/photos/{side}",
            put(put_photo)
                .delete(delete_photo)
                .layer(DefaultBodyLimit::max(photos::MAX_BYTES + 1024)),
        )
        .route("/photos/{hash}", get(photo))
        .fallback(crate::assets::serve)
        .layer(DefaultBodyLimit::max(MAX_JSON_BYTES))
}

/// A JSON body whose rejection is a JSON error like every other.
struct JsonBody<T>(T);

impl<T, S> FromRequest<S> for JsonBody<T>
where
    axum::Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Response> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(value)) => Ok(JsonBody(value)),
            Err(rejection) => Err(error(rejection.status(), &rejection.body_text())),
        }
    }
}

/// The peer's address, when the server knows it (the tests call the router without a socket).
struct Peer(Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for Peer {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Peer(
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<SocketAddr>>()
                .map(|c| c.0),
        ))
    }
}

fn card_error(err: CardError) -> Response {
    match err {
        CardError::NotFound => error(StatusCode::NOT_FOUND, "no such card"),
        CardError::Invalid(why) => error(StatusCode::UNPROCESSABLE_ENTITY, &why),
        CardError::Database(why) => internal(&anyhow::anyhow!(why)),
    }
}

fn account_error(err: anyhow::Error) -> Response {
    match err.downcast_ref::<Refused>() {
        Some(Refused::Invalid(why)) => error(StatusCode::UNPROCESSABLE_ENTITY, why),
        Some(Refused::NotFound) => error(StatusCode::NOT_FOUND, "not found"),
        None => internal(&err),
    }
}

fn json(value: impl serde::Serialize) -> Response {
    axum::Json(value).into_response()
}

fn with_cookie(mut response: Response, cookie: String) -> Response {
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

// ---------------------------------------------------------------------------------------------

async fn healthz(State(state): State<AppState>) -> Response {
    let ok = state
        .with_db(|db| db.conn().query_row("SELECT 1", [], |r| r.get::<_, i64>(0)))
        .await;
    match ok {
        Ok(_) => bytes("text/plain; charset=utf-8", "ok\n"),
        Err(err) => internal(&err.into()),
    }
}

#[derive(Deserialize)]
struct Login {
    name: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Peer(peer): Peer,
    headers: HeaderMap,
    JsonBody(form): JsonBody<Login>,
) -> Response {
    let keys = Throttle::keys(&form.name, client_ip(&state, &headers, peer));
    if let Some(wait) = state.throttle.wait(&keys, now()) {
        let mut response = error(
            StatusCode::TOO_MANY_REQUESTS,
            &format!(
                "too many wrong passwords; try again in {} minutes",
                (wait + 59) / 60
            ),
        );
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(wait));
        return response;
    }
    let name = form.name.trim().to_string();
    let found = state
        .with_db(move |db| {
            let user = accounts::sign_in(db, &name, &form.password)?;
            match user {
                Some(user) => {
                    let token = accounts::start_session(db, &user, now())?;
                    Ok::<_, anyhow::Error>(Some((me_json(db, &user)?, token)))
                }
                None => Ok(None),
            }
        })
        .await;
    match found {
        Ok(Some((me, token))) => {
            state.throttle.succeeded(&keys);
            with_cookie(json(me), state.session_cookie(&token))
        }
        Ok(None) => {
            state.throttle.failed(&keys, now());
            error(StatusCode::UNAUTHORIZED, "wrong name or password")
        }
        Err(err) => internal(&err),
    }
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_value(&headers, state.cookie_name()) {
        let ended = state
            .with_db(move |db| accounts::end_session(db, &token))
            .await;
        if let Err(err) = ended {
            return internal(&err);
        }
    }
    with_cookie(
        StatusCode::NO_CONTENT.into_response(),
        state.expired_cookie(),
    )
}

fn me_json(db: &crate::db::Db, user: &User) -> anyhow::Result<serde_json::Value> {
    let household = accounts::household(db, user.household_id)?;
    Ok(json!({
        "id": user.id,
        "name": user.name,
        "display_name": user.display_name,
        "is_admin": user.is_admin,
        "household": household,
    }))
}

/// Who is signed in. Also renews the cookie, so that a phone that opens the app keeps its
/// session for another [`accounts::SESSION_IDLE_SECS`].
async fn me(State(state): State<AppState>, Viewer(user): Viewer, headers: HeaderMap) -> Response {
    let found = state.with_db(move |db| me_json(db, &user)).await;
    match found {
        Ok(me) => match cookie_value(&headers, state.cookie_name()) {
            Some(token) => with_cookie(json(me), state.session_cookie(&token)),
            None => json(me),
        },
        Err(err) => internal(&err),
    }
}

#[derive(Deserialize)]
struct NewPassword {
    current: String,
    new: String,
}

async fn change_password(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    JsonBody(form): JsonBody<NewPassword>,
) -> Response {
    let changed = state
        .with_db(move |db| accounts::change_password(db, &user, &form.current, &form.new))
        .await;
    match changed {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => account_error(err),
    }
}

async fn household(State(state): State<AppState>, Viewer(user): Viewer) -> Response {
    let found = state
        .with_db(move |db| {
            let household = accounts::household(db, user.household_id)?;
            let members = accounts::members(db, user.household_id)?;
            Ok::<_, anyhow::Error>(json!({
                "id": household.id,
                "name": household.name,
                "members": members,
            }))
        })
        .await;
    match found {
        Ok(value) => json(value),
        Err(err) => internal(&err),
    }
}

#[derive(Deserialize)]
struct NewInvite {
    name: String,
    #[serde(default)]
    admin: bool,
}

async fn create_invite(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    JsonBody(form): JsonBody<NewInvite>,
) -> Response {
    if !user.is_admin {
        return error(
            StatusCode::FORBIDDEN,
            "only an administrator can invite people",
        );
    }
    let origin = state.origin.clone();
    let made = state
        .with_db(move |db| {
            let household = accounts::household(db, user.household_id)?;
            let at = now();
            let token = accounts::invite(db, &household, form.name.trim(), form.admin, at)?;
            Ok::<_, anyhow::Error>(json!({
                "link": format!("{origin}/i/{token}"),
                "expires_at": at + accounts::INVITE_TTL_SECS,
            }))
        })
        .await;
    match made {
        Ok(value) => (StatusCode::CREATED, axum::Json(value)).into_response(),
        Err(err) => account_error(err),
    }
}

async fn invite_info(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    let found = state
        .with_db(move |db| accounts::invite_info(db, &token, now()))
        .await;
    match found {
        Ok(Some(info)) => json(info),
        Ok(None) => error(
            StatusCode::NOT_FOUND,
            "this invite link was used or has expired",
        ),
        Err(err) => internal(&err),
    }
}

#[derive(Deserialize)]
struct Arrival {
    password: String,
    display_name: Option<String>,
}

async fn accept_invite(
    State(state): State<AppState>,
    Path(token): Path<String>,
    JsonBody(form): JsonBody<Arrival>,
) -> Response {
    let done = state
        .with_db(move |db| {
            let at = now();
            let user = accounts::accept_invite(
                db,
                &token,
                &form.password,
                form.display_name.as_deref(),
                at,
            )?;
            let session = accounts::start_session(db, &user, at)?;
            Ok::<_, anyhow::Error>((me_json(db, &user)?, session))
        })
        .await;
    match done {
        Ok((me, session)) => with_cookie(json(me), state.session_cookie(&session)),
        Err(err) => match err.downcast_ref::<Refused>() {
            Some(Refused::NotFound) => error(
                StatusCode::NOT_FOUND,
                "this invite link was used or has expired",
            ),
            _ => account_error(err),
        },
    }
}

// ---------------------------------------------------------------------------------------------
// Cards

async fn list_cards(State(state): State<AppState>, Viewer(user): Viewer) -> Response {
    match state.with_db(move |db| cards::list(db, &user)).await {
        Ok(list) => json(list),
        Err(err) => card_error(err),
    }
}

async fn get_card(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<i64>,
) -> Response {
    match state.with_db(move |db| cards::get(db, &user, id)).await {
        Ok(card) => json(card),
        Err(err) => card_error(err),
    }
}

async fn create_card(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    JsonBody(input): JsonBody<CardInput>,
) -> Response {
    match state
        .with_db(move |db| cards::create(db, &user, &input, now()))
        .await
    {
        Ok(card) => (StatusCode::CREATED, axum::Json(card)).into_response(),
        Err(err) => card_error(err),
    }
}

async fn update_card(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<CardInput>,
) -> Response {
    match state
        .with_db(move |db| cards::update(db, &user, id, &input, now()))
        .await
    {
        Ok(card) => json(card),
        Err(err) => card_error(err),
    }
}

async fn delete_card(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<i64>,
) -> Response {
    match state.with_db(move |db| cards::delete(db, &user, id)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => card_error(err),
    }
}

async fn card_used(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<i64>,
) -> Response {
    match state
        .with_db(move |db| cards::mark_used(db, &user, id, now()))
        .await
    {
        Ok(card) => json(card),
        Err(err) => card_error(err),
    }
}

fn no_such_side() -> Response {
    error(StatusCode::NOT_FOUND, "a card has a front and a back")
}

async fn put_photo(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path((id, side_name)): Path<(i64, String)>,
    body: Bytes,
) -> Response {
    let Some(side) = Side::parse(&side_name) else {
        return no_such_side();
    };
    let stored = state
        .with_db(move |db| {
            photos::put(db, &user, id, side, &body, now())?;
            cards::get(db, &user, id)
        })
        .await;
    match stored {
        Ok(card) => json(card),
        Err(err) => card_error(err),
    }
}

async fn delete_photo(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path((id, side_name)): Path<(i64, String)>,
) -> Response {
    let Some(side) = Side::parse(&side_name) else {
        return no_such_side();
    };
    let removed = state
        .with_db(move |db| {
            photos::remove(db, &user, id, side, now())?;
            cards::get(db, &user, id)
        })
        .await;
    match removed {
        Ok(card) => json(card),
        Err(err) => card_error(err),
    }
}

async fn photo(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(hash): Path<String>,
) -> Response {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return error(StatusCode::NOT_FOUND, "no such photo");
    }
    match state.with_db(move |db| photos::get(db, &user, &hash)).await {
        Ok(Some((mime, data))) => {
            let mut response = bytes(&mime, data);
            // `private`: a shared cache must never hand one household's photo to another.
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=31536000, immutable"),
            );
            response
        }
        Ok(None) => error(StatusCode::NOT_FOUND, "no such photo"),
        Err(err) => card_error(err),
    }
}
