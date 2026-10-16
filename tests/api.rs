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

//! The API end to end: requests through the real router and every guard in front of it, against
//! a real database, as the app sends them. No socket: `tower::ServiceExt::oneshot` calls the
//! router directly, which is the same code the server runs per request.

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use http_body_util::BodyExt as _;
use loyalty::accounts::{self, User};
use loyalty::cards::{self, CardInput, CodeFormat};
use loyalty::db::Db;
use loyalty::web::{self, AppState};
use serde_json::{Value, json};
use tower::ServiceExt as _;

const ORIGIN: &str = "https://cards.example";
const PASSWORD: &str = "a good password";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0 a tiny jpeg";

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| {
            panic!(
                "the body was not JSON:\n{}",
                String::from_utf8_lossy(&self.body)
            )
        })
    }

    fn header(&self, name: &str) -> &str {
        self.headers
            .get(name)
            .map(|v| v.to_str().unwrap())
            .unwrap_or_default()
    }

    /// The session token this answer sets, if any.
    fn session(&self) -> Option<String> {
        self.headers
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap())
            .find_map(|c| c.strip_prefix("__Host-loyalty="))
            .map(|c| c.split(';').next().unwrap().to_string())
            .filter(|t| !t.is_empty())
    }
}

/// A household, Home, with marko (admin) and ana; and a second household with nina.
struct App {
    state: AppState,
    marko: String,
    ana: String,
    nina: String,
}

impl App {
    fn new() -> App {
        let mut db = Db::open_in_memory().unwrap();
        let home = accounts::add_household(&db, "Home", 1).unwrap();
        let neighbours = accounts::add_household(&db, "Neighbours", 1).unwrap();
        let mut arrive =
            |household: &accounts::Household, name: &str, display: &str, admin: bool| {
                let token = accounts::invite(&db, household, name, admin, 1).unwrap();
                let user: User =
                    accounts::accept_invite(&mut db, &token, PASSWORD, Some(display), 2).unwrap();
                accounts::start_session(&db, &user, loyalty::db::now()).unwrap()
            };
        let marko = arrive(&home, "marko", "Marko", true);
        let ana = arrive(&home, "ana", "Ana", false);
        let nina = arrive(&neighbours, "nina", "Nina", true);
        App {
            state: AppState::new(db, ORIGIN, false).unwrap(),
            marko,
            ana,
            nina,
        }
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        session: Option<&str>,
        body: Body,
        content_type: &str,
    ) -> Answer {
        let mut request = Request::builder().method(method.clone()).uri(path);
        if method != Method::GET {
            request = request
                .header(header::ORIGIN, ORIGIN)
                .header("x-loyalty", "1");
        }
        if let Some(token) = session {
            request = request.header(header::COOKIE, format!("__Host-loyalty={token}"));
        }
        request = request.header(header::CONTENT_TYPE, content_type);
        self.raw(request.body(body).unwrap()).await
    }

    async fn raw(&self, request: Request<Body>) -> Answer {
        let response = web::router(self.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        Answer {
            status,
            headers,
            body,
        }
    }

    async fn get(&self, path: &str, session: &str) -> Answer {
        self.send(
            Method::GET,
            path,
            Some(session),
            Body::empty(),
            "text/plain",
        )
        .await
    }

    async fn json(&self, method: Method, path: &str, session: Option<&str>, body: Value) -> Answer {
        self.send(
            method,
            path,
            session,
            Body::from(body.to_string()),
            "application/json",
        )
        .await
    }

    async fn new_card(&self, session: &str, body: Value) -> Value {
        let answer = self
            .json(Method::POST, "/api/cards", Some(session), body)
            .await;
        assert_eq!(
            answer.status,
            StatusCode::CREATED,
            "the answer was {}",
            answer.json()
        );
        answer.json()
    }

    async fn user_id(&self, session: &str) -> i64 {
        self.get("/api/me", session).await.json()["id"]
            .as_i64()
            .unwrap()
    }
}

fn card(name: &str) -> Value {
    json!({
        "holder": null,
        "name": name,
        "code_value": "4006381333931",
        "code_format": "ean13",
        "color": "#1c7455",
    })
}

#[tokio::test]
async fn signing_in_starts_a_session_that_signs_requests_in() {
    let app = App::new();
    let answer = app
        .json(
            Method::POST,
            "/api/login",
            None,
            json!({"name": "marko", "password": PASSWORD}),
        )
        .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the answer was {}",
        answer.json()
    );
    assert_eq!(answer.json()["name"], "marko");
    let token = answer.session().expect("no session cookie was set");
    let cookie = answer.header("set-cookie");
    for flag in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(cookie.contains(flag), "the cookie lacks {flag}: {cookie}");
    }
    let me = app.get("/api/me", &token).await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.json()["household"]["name"], "Home");
}

#[tokio::test]
async fn a_wrong_password_is_refused_and_repeated_ones_are_slowed_down() {
    let app = App::new();
    let attempt = || {
        app.json(
            Method::POST,
            "/api/login",
            None,
            json!({"name": "marko", "password": "nope"}),
        )
    };
    for _ in 0..loyalty::throttle::FREE_FAILURES {
        let answer = attempt().await;
        assert_eq!(answer.status, StatusCode::UNAUTHORIZED);
        assert_eq!(answer.json()["error"], "wrong name or password");
        assert!(answer.session().is_none());
    }
    let answer = attempt().await;
    assert_eq!(answer.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(answer.header("retry-after"), "30");
    let right = app
        .json(
            Method::POST,
            "/api/login",
            None,
            json!({"name": "marko", "password": PASSWORD}),
        )
        .await;
    assert_eq!(
        right.status,
        StatusCode::TOO_MANY_REQUESTS,
        "the right password got past the wait"
    );
}

#[tokio::test]
async fn without_a_session_nothing_is_shown() {
    let app = App::new();
    for path in [
        "/api/me",
        "/api/cards",
        "/api/cards/1",
        "/api/household",
        &format!("/photos/{}", "a".repeat(64)),
    ] {
        let request = Request::builder().uri(path).body(Body::empty()).unwrap();
        let answer = app.raw(request).await;
        assert_eq!(answer.status, StatusCode::UNAUTHORIZED, "{path}");
        assert_eq!(answer.json()["error"], "sign in first", "{path}");
    }
    let made_up = app.get("/api/me", &"0".repeat(64)).await;
    assert_eq!(made_up.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_change_from_another_site_is_refused() {
    let app = App::new();
    let body = || Body::from(card("Shop").to_string());
    let cookie = format!("__Host-loyalty={}", app.marko);
    let table: &[(&str, Option<&str>, bool)] = &[
        ("no Origin", None, true),
        ("another origin", Some("https://evil.example"), true),
        ("no X-Loyalty header", Some(ORIGIN), false),
    ];
    for (what, origin, with_header) in table {
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/api/cards")
            .header(header::COOKIE, &cookie)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, *origin);
        }
        if *with_header {
            request = request.header("x-loyalty", "1");
        }
        let answer = app.raw(request.body(body()).unwrap()).await;
        assert_eq!(answer.status, StatusCode::FORBIDDEN, "{what}");
    }
    assert_eq!(
        app.get("/api/cards", &app.marko).await.json(),
        json!([]),
        "a refused request saved a card"
    );
}

#[tokio::test]
async fn a_card_is_made_shown_changed_used_and_deleted() {
    let app = App::new();
    let made = app.new_card(&app.marko, card("Coop")).await;
    let id = made["id"].as_i64().unwrap();
    let path = format!("/api/cards/{id}");
    assert_eq!(
        app.get(&path, &app.ana).await.json()["name"],
        "Coop",
        "ana does not see the family's card"
    );

    let mut changed = card("Coop Supercard");
    changed["holder"] = json!(app.user_id(&app.ana).await);
    let answer = app
        .json(Method::PUT, &path, Some(&app.marko), changed)
        .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the answer was {}",
        answer.json()
    );
    assert_eq!(answer.json()["holder"]["display_name"], "Ana");

    let used = app
        .json(
            Method::POST,
            &format!("{path}/used"),
            Some(&app.ana),
            json!({}),
        )
        .await;
    assert_eq!(used.json()["use_count"], 1);

    let deleted = app
        .send(
            Method::DELETE,
            &path,
            Some(&app.marko),
            Body::empty(),
            "text/plain",
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get(&path, &app.marko).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn another_household_cannot_reach_a_card() {
    let app = App::new();
    let id = app.new_card(&app.marko, card("Coop")).await["id"]
        .as_i64()
        .unwrap();
    let path = format!("/api/cards/{id}");
    assert_eq!(app.get("/api/cards", &app.nina).await.json(), json!([]));
    assert_eq!(
        app.get(&path, &app.nina).await.status,
        StatusCode::NOT_FOUND
    );
    let answer = app
        .json(Method::PUT, &path, Some(&app.nina), card("Mine"))
        .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    let answer = app
        .send(
            Method::DELETE,
            &path,
            Some(&app.nina),
            Body::empty(),
            "text/plain",
        )
        .await;
    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(app.get(&path, &app.marko).await.json()["name"], "Coop");
}

#[tokio::test]
async fn a_private_card_is_its_holders_alone() {
    let app = App::new();
    let mut secret = card("Diary club");
    secret["holder"] = json!(app.user_id(&app.ana).await);
    secret["private"] = json!(true);
    let id = app.new_card(&app.ana, secret).await["id"].as_i64().unwrap();
    assert_eq!(app.get("/api/cards", &app.marko).await.json(), json!([]));
    assert_eq!(
        app.get(&format!("/api/cards/{id}"), &app.marko)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/api/cards", &app.ana).await.json()[0]["private"],
        true
    );
}

#[tokio::test]
async fn a_mistake_in_a_card_is_explained() {
    let app = App::new();
    let mut wrong = card("Coop");
    wrong["code_value"] = json!("4006381333932");
    let answer = app
        .json(Method::POST, "/api/cards", Some(&app.marko), wrong)
        .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        answer.json()["error"],
        "the last digit should be 1: the number has a typing mistake"
    );

    let mut unknown = card("Coop");
    unknown["code_format"] = json!("ean-13");
    let answer = app
        .json(Method::POST, "/api/cards", Some(&app.marko), unknown)
        .await;
    assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        answer.json()["error"]
            .as_str()
            .unwrap()
            .contains("unknown variant"),
        "{}",
        answer.json()
    );

    let answer = app
        .send(
            Method::POST,
            "/api/cards",
            Some(&app.marko),
            Body::from("{"),
            "application/json",
        )
        .await;
    assert_eq!(answer.status, StatusCode::BAD_REQUEST);
    assert!(answer.json()["error"].is_string());
}

#[tokio::test]
async fn a_photo_is_kept_with_its_card_and_shown_only_to_the_household() {
    let app = App::new();
    let id = app.new_card(&app.marko, card("Coop")).await["id"]
        .as_i64()
        .unwrap();
    let path = format!("/api/cards/{id}/photos/front");
    let answer = app
        .send(
            Method::PUT,
            &path,
            Some(&app.marko),
            Body::from(JPEG),
            "image/jpeg",
        )
        .await;
    assert_eq!(answer.status, StatusCode::OK);
    let hash = answer.json()["front"].as_str().unwrap().to_string();

    let photo = app.get(&format!("/photos/{hash}"), &app.ana).await;
    assert_eq!(photo.status, StatusCode::OK);
    assert_eq!(photo.body, JPEG);
    assert_eq!(photo.header("content-type"), "image/jpeg");
    assert_eq!(
        photo.header("cache-control"),
        "private, max-age=31536000, immutable"
    );
    assert_eq!(
        app.get(&format!("/photos/{hash}"), &app.nina).await.status,
        StatusCode::NOT_FOUND
    );

    let removed = app
        .send(
            Method::DELETE,
            &path,
            Some(&app.marko),
            Body::empty(),
            "text/plain",
        )
        .await;
    assert_eq!(removed.json()["front"], Value::Null);
    assert_eq!(
        app.get(&format!("/photos/{hash}"), &app.marko).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn what_is_not_a_photo_is_refused() {
    let app = App::new();
    let id = app.new_card(&app.marko, card("Coop")).await["id"]
        .as_i64()
        .unwrap();
    let put = async |side: &str, body: Vec<u8>| {
        let path = format!("/api/cards/{id}/photos/{side}");
        app.send(
            Method::PUT,
            &path,
            Some(&app.marko),
            Body::from(body),
            "image/jpeg",
        )
        .await
    };
    assert_eq!(
        put("front", b"<svg/>".to_vec()).await.status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        put("middle", JPEG.to_vec()).await.status,
        StatusCode::NOT_FOUND
    );
    let mut big = JPEG.to_vec();
    big.resize(loyalty::photos::MAX_BYTES + 2048, 0);
    assert_eq!(put("back", big).await.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        app.get("/photos/not-a-hash", &app.marko).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_administrator_invites_someone_who_then_signs_in() {
    let app = App::new();
    let answer = app
        .json(
            Method::POST,
            "/api/invites",
            Some(&app.marko),
            json!({"name": "luka"}),
        )
        .await;
    assert_eq!(
        answer.status,
        StatusCode::CREATED,
        "the answer was {}",
        answer.json()
    );
    let link = answer.json()["link"].as_str().unwrap().to_string();
    let token = link
        .strip_prefix(&format!("{ORIGIN}/i/"))
        .expect("the link is not on the origin");

    let info = app.get(&format!("/api/invite/{token}"), "").await;
    assert_eq!(info.json()["name"], "luka");
    assert_eq!(info.json()["household"], "Home");

    let arrived = app
        .json(
            Method::POST,
            &format!("/api/invite/{token}"),
            None,
            json!({"password": "luka's password", "display_name": "Luka"}),
        )
        .await;
    assert_eq!(
        arrived.status,
        StatusCode::OK,
        "the answer was {}",
        arrived.json()
    );
    let session = arrived.session().expect("no session after accepting");
    assert_eq!(
        app.get("/api/me", &session).await.json()["display_name"],
        "Luka"
    );

    let again = app
        .json(
            Method::POST,
            &format!("/api/invite/{token}"),
            None,
            json!({"password": "luka's password"}),
        )
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    assert_eq!(
        app.get(&format!("/api/invite/{token}"), "").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn only_an_administrator_invites() {
    let app = App::new();
    let answer = app
        .json(
            Method::POST,
            "/api/invites",
            Some(&app.ana),
            json!({"name": "luka"}),
        )
        .await;
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let taken = app
        .json(
            Method::POST,
            "/api/invites",
            Some(&app.marko),
            json!({"name": "ana"}),
        )
        .await;
    assert_eq!(taken.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(taken.json()["error"], "the name \"ana\" is taken");
}

#[tokio::test]
async fn an_invite_with_a_short_password_stays_usable() {
    let app = App::new();
    let link = app
        .json(
            Method::POST,
            "/api/invites",
            Some(&app.marko),
            json!({"name": "luka"}),
        )
        .await
        .json()["link"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/invite/{}", link.rsplit('/').next().unwrap());
    let short = app
        .json(Method::POST, &path, None, json!({"password": "short"}))
        .await;
    assert_eq!(short.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(app.get(&path, "").await.status, StatusCode::OK);
}

#[tokio::test]
async fn signing_out_ends_the_session() {
    let app = App::new();
    let answer = app
        .send(
            Method::POST,
            "/api/logout",
            Some(&app.marko),
            Body::empty(),
            "text/plain",
        )
        .await;
    assert_eq!(answer.status, StatusCode::NO_CONTENT);
    assert!(answer.header("set-cookie").contains("Max-Age=0"));
    assert_eq!(
        app.get("/api/me", &app.marko).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        app.get("/api/me", &app.ana).await.status,
        StatusCode::OK,
        "ana was signed out too"
    );
}

#[tokio::test]
async fn a_password_is_changed_with_the_current_one() {
    let app = App::new();
    let wrong = app
        .json(
            Method::POST,
            "/api/me/password",
            Some(&app.ana),
            json!({"current": "nope", "new": "a newer password"}),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::UNPROCESSABLE_ENTITY);
    let right = app
        .json(
            Method::POST,
            "/api/me/password",
            Some(&app.ana),
            json!({"current": PASSWORD, "new": "a newer password"}),
        )
        .await;
    assert_eq!(right.status, StatusCode::NO_CONTENT);
    let login = app
        .json(
            Method::POST,
            "/api/login",
            None,
            json!({"name": "ana", "password": "a newer password"}),
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
}

#[tokio::test]
async fn the_household_lists_its_people() {
    let app = App::new();
    let household = app.get("/api/household", &app.ana).await.json();
    let names: Vec<&str> = household["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["display_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Ana", "Marko"]);
}

#[tokio::test]
async fn every_answer_carries_the_security_headers() {
    let app = App::new();
    for answer in [
        app.get("/api/cards", &app.marko).await,
        app.get("/api/cards/999", &app.marko).await,
        app.raw(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await,
    ] {
        assert_eq!(answer.header("content-security-policy"), web::CSP);
        assert_eq!(answer.header("x-content-type-options"), "nosniff");
        assert_eq!(answer.header("referrer-policy"), "no-referrer");
        assert!(
            answer
                .header("permissions-policy")
                .contains("camera=(self)")
        );
    }
    assert_eq!(
        app.get("/api/cards", &app.marko)
            .await
            .header("cache-control"),
        "no-store"
    );
}

#[tokio::test]
async fn the_health_check_says_ok() {
    let app = App::new();
    let answer = app
        .raw(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
    assert_eq!(
        (answer.status, answer.body.as_slice()),
        (StatusCode::OK, &b"ok\n"[..])
    );
}

/// The JSON the app reads, pinned: a change here is a change the app must follow.
#[tokio::test]
async fn the_shapes_of_a_card_and_of_me_are_pinned() {
    let app = App::new();
    let ana_id = app.user_id(&app.ana).await;
    let id = app
        .state
        .with_db(move |db| {
            let ana = accounts::user_by_name(db, "ana").unwrap().unwrap();
            let input = CardInput {
                holder: Some(ana_id),
                private: true,
                name: "Miles & More".into(),
                program: "Lufthansa".into(),
                code_value: "M1IVANKOVIC/ANA".into(),
                code_format: CodeFormat::Pdf417,
                display_number: "992 000 123 456".into(),
                color: "#0a2f6b".into(),
                notes: "Gold until March".into(),
            };
            cards::create(db, &ana, &input, 1_760_000_000).unwrap().id
        })
        .await;
    let card = app.get(&format!("/api/cards/{id}"), &app.ana).await.json();
    let me = app.get("/api/me", &app.ana).await.json();
    let text = format!(
        "{}\n{}\n",
        serde_json::to_string_pretty(&card).unwrap(),
        serde_json::to_string_pretty(&me).unwrap()
    );
    loyalty::golden::assert_golden("api.json", &text);
}
