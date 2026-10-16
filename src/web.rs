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

//! The web server: the JSON API the app talks to, the photos, and the app itself.
//!
//! # The order the guards run in
//!
//! Every request passes through, outermost first:
//!
//! 1. **The request log**: one line per request on standard error, with tokens in paths
//!    replaced by `…`.
//! 2. **Security headers**, on every response including errors.
//! 3. **CSRF**: a request that changes something (any method but GET and HEAD) must come from the
//!    app's own origin (`Origin`) and carry the `X-Loyalty` header, which a form or a link on
//!    another site cannot add without a CORS preflight this server never answers.
//! 4. **The session**, resolved per handler by the [`Viewer`] extractor, so that the handlers
//!    that need no session (sign-in, invites, the app's files) say so by not asking for one.
//!
//! [`serve`] runs the accept loop around it, which bounds how long a request head may take and
//! how many connections are open at once.
//!
//! # Cookies
//!
//! The session cookie is `HttpOnly` and `SameSite=Lax`. When the configured origin is HTTPS it
//! is also `Secure` and named with the `__Host-` prefix, which pins it to this host and path `/`.
//! An `http://` origin (a test, or a first try on a home network) gets a plain cookie, since a
//! browser drops a `Secure` cookie from a page that is not.

mod api;
mod serve;

pub use serve::{ServeLimits, serve, serve_on};

use crate::accounts::{self, User};
use crate::db::Db;
use crate::throttle::Throttle;
use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header, request::Parts};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

/// Everything a handler can reach.
#[derive(Clone)]
pub struct AppState {
    db: Arc<Mutex<Db>>,
    /// Where the app is served from, as a browser writes it in `Origin`: `https://cards.example`.
    pub origin: String,
    /// Whether to believe `X-Real-IP`: only behind a proxy that sets it.
    pub trust_proxy: bool,
    throttle: Arc<Throttle>,
}

impl AppState {
    pub fn new(db: Db, origin: &str, trust_proxy: bool) -> anyhow::Result<AppState> {
        let origin = origin.trim_end_matches('/').to_string();
        let host = origin
            .strip_prefix("https://")
            .or_else(|| origin.strip_prefix("http://"));
        anyhow::ensure!(
            host.is_some_and(|h| !h.is_empty() && !h.contains('/')),
            "the origin is a scheme and a host, like https://cards.example.org, not {origin:?}"
        );
        Ok(AppState {
            db: Arc::new(Mutex::new(db)),
            origin,
            trust_proxy,
            throttle: Arc::default(),
        })
    }

    /// Runs `f` with the database, off the async threads.
    pub async fn with_db<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Db) -> T + Send + 'static,
    ) -> T {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            let mut db = db.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut db)
        })
        .await
        .expect("a database task panicked")
    }

    fn secure(&self) -> bool {
        self.origin.starts_with("https://")
    }

    pub fn cookie_name(&self) -> &'static str {
        if self.secure() {
            "__Host-loyalty"
        } else {
            "loyalty"
        }
    }

    fn session_cookie(&self, token: &str) -> String {
        format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
            self.cookie_name(),
            accounts::SESSION_IDLE_SECS,
            if self.secure() { "; Secure" } else { "" }
        )
    }

    fn expired_cookie(&self) -> String {
        format!(
            "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
            self.cookie_name(),
            if self.secure() { "; Secure" } else { "" }
        )
    }
}

pub fn router(state: AppState) -> Router {
    api::routes()
        .layer(middleware::from_fn_with_state(state.clone(), csrf_guard))
        .layer(middleware::from_fn(security_headers))
        .layer(middleware::from_fn_with_state(state.clone(), request_log))
        .with_state(state)
}

/// The person signed in, for handlers that need one. Without a valid session the request ends
/// with 401.
pub struct Viewer(pub User);

impl FromRequestParts<AppState> for Viewer {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let Some(token) = cookie_value(&parts.headers, state.cookie_name()) else {
            return Err(error(StatusCode::UNAUTHORIZED, "sign in first"));
        };
        let found = state
            .with_db(move |db| accounts::session_user(db, &token, crate::db::now()))
            .await;
        match found {
            Ok(Some(user)) => Ok(Viewer(user)),
            Ok(None) => Err(error(StatusCode::UNAUTHORIZED, "sign in first")),
            Err(err) => Err(internal(&err)),
        }
    }
}

/// The value of cookie `name` in a request's `Cookie` headers.
pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
        .filter(|value| !value.is_empty())
}

/// The client's address: the peer's, or `X-Real-IP` behind a trusted proxy.
pub fn client_ip(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
) -> Option<IpAddr> {
    if state.trust_proxy
        && let Some(ip) = headers
            .get("x-real-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse().ok())
    {
        return Some(ip);
    }
    peer.map(|p| p.ip())
}

/// A JSON error: `{"error": "<message>"}`.
pub fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(serde_json::json!({ "error": message }))).into_response()
}

/// A failure that is the server's, not the request's: logged in full, answered in general.
pub fn internal(err: &anyhow::Error) -> Response {
    eprintln!("error: {err:#}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "the server failed; try again",
    )
}

pub const CSP: &str = "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; \
    style-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; \
    connect-src 'self'; worker-src 'self'; manifest-src 'self'; object-src 'none'; \
    base-uri 'none'; form-action 'self'; frame-ancestors 'none'";

async fn security_headers(request: Request, next: Next) -> Response {
    let api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let set = |headers: &mut HeaderMap, name: &'static str, value: &'static str| {
        headers.insert(name, HeaderValue::from_static(value));
    };
    set(headers, "content-security-policy", CSP);
    set(headers, "x-content-type-options", "nosniff");
    set(headers, "referrer-policy", "no-referrer");
    set(headers, "x-frame-options", "DENY");
    set(headers, "cross-origin-opener-policy", "same-origin");
    set(headers, "cross-origin-resource-policy", "same-origin");
    // The camera, for scanning; nothing else a page could ask the phone for.
    set(
        headers,
        "permissions-policy",
        "camera=(self), microphone=(), geolocation=(), payment=(), usb=()",
    );
    if api {
        set(headers, "cache-control", "no-store");
    }
    response
}

async fn csrf_guard(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if matches!(*request.method(), Method::GET | Method::HEAD) {
        return next.run(request).await;
    }
    let headers = request.headers();
    let same_origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|o| o == state.origin);
    if !same_origin {
        return error(
            StatusCode::FORBIDDEN,
            "this request did not come from the app",
        );
    }
    if !headers.contains_key("x-loyalty") {
        return error(
            StatusCode::FORBIDDEN,
            "this request did not come from the app",
        );
    }
    next.run(request).await
}

async fn request_log(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = redact(request.uri().path());
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    let ip = client_ip(&state, request.headers(), peer);
    let response = next.run(request).await;
    let ip = ip.map_or_else(|| "-".to_string(), |ip| ip.to_string());
    eprintln!("{method} {path} {} ({ip})", response.status().as_u16());
    response
}

/// A path for the log: invite tokens replaced, control characters dropped.
fn redact(path: &str) -> String {
    let mut parts: Vec<&str> = path.split('/').collect();
    for i in 1..parts.len() {
        if matches!(parts[i - 1], "invite" | "i") && !parts[i].is_empty() {
            parts[i] = "…";
        }
    }
    parts
        .join("/")
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect()
}

/// A response with a body and a content type.
fn bytes(content_type: &str, body: impl Into<Body>) -> Response {
    let mut response = Response::new(body.into());
    if let Ok(value) = HeaderValue::from_str(content_type) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(origin: &str) -> AppState {
        AppState::new(Db::open_in_memory().unwrap(), origin, false).unwrap()
    }

    #[test]
    fn an_https_origin_gets_a_secure_host_cookie() {
        let cookie = state("https://cards.example").session_cookie("abc");
        assert_eq!(
            cookie,
            "__Host-loyalty=abc; Path=/; HttpOnly; SameSite=Lax; Max-Age=34560000; Secure"
        );
    }

    #[test]
    fn an_http_origin_gets_a_plain_cookie() {
        let s = state("http://localhost:8080");
        assert_eq!(
            s.session_cookie("abc"),
            "loyalty=abc; Path=/; HttpOnly; SameSite=Lax; Max-Age=34560000"
        );
        assert_eq!(
            s.expired_cookie(),
            "loyalty=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
        );
    }

    #[test]
    fn an_origin_is_a_scheme_and_a_host() {
        for bad in [
            "cards.example",
            "https://cards.example/app",
            "ftp://cards.example",
        ] {
            assert!(
                AppState::new(Db::open_in_memory().unwrap(), bad, false).is_err(),
                "{bad} was accepted"
            );
        }
        assert_eq!(
            state("https://cards.example/").origin,
            "https://cards.example"
        );
    }

    #[test]
    fn a_cookie_is_found_among_others_by_its_exact_name() {
        let mut headers = HeaderMap::new();
        headers.append(header::COOKIE, "a=1; loyalty-x=2".parse().unwrap());
        headers.append(header::COOKIE, "loyalty=3; b=4".parse().unwrap());
        assert_eq!(cookie_value(&headers, "loyalty").as_deref(), Some("3"));
        assert_eq!(cookie_value(&headers, "loyal"), None);
        headers.insert(header::COOKIE, "loyalty=".parse().unwrap());
        assert_eq!(
            cookie_value(&headers, "loyalty"),
            None,
            "an empty cookie is no cookie"
        );
    }

    #[test]
    fn the_real_ip_header_is_believed_only_behind_a_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "203.0.113.9".parse().unwrap());
        let peer: SocketAddr = "172.18.0.2:5000".parse().unwrap();
        let direct = state("https://cards.example");
        assert_eq!(client_ip(&direct, &headers, Some(peer)), Some(peer.ip()));
        let proxied = AppState {
            trust_proxy: true,
            ..direct
        };
        assert_eq!(
            client_ip(&proxied, &headers, Some(peer)),
            Some("203.0.113.9".parse().unwrap())
        );
        headers.insert("x-real-ip", "nonsense".parse().unwrap());
        assert_eq!(client_ip(&proxied, &headers, Some(peer)), Some(peer.ip()));
    }

    #[test]
    fn tokens_are_kept_out_of_the_log() {
        let table = [
            ("/api/invite/0123abcd", "/api/invite/…"),
            ("/i/0123abcd", "/i/…"),
            ("/api/cards/7", "/api/cards/7"),
            ("/i/", "/i/"),
        ];
        for (path, expected) in table {
            assert_eq!(redact(path), expected, "the path was {path}");
        }
    }
}
