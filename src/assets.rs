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

//! The web app's files, built into the binary by `build.rs` from `web/dist`.
//!
//! Files under `/assets/` have their content's hash in their names (bun's bundler names them so),
//! so a browser may keep them for a year. Everything else (`index.html`, the service worker, the
//! manifest, the icons) keeps its name across releases and is revalidated on every load, which
//! is how a new release reaches a phone.

use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

pub struct Asset {
    pub path: &'static str,
    pub bytes: &'static [u8],
    pub mime: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The file for a URL path. The app's own pages (`/` and invite links, `/i/<token>`) are
/// `index.html`; the app reads the rest of the URL itself.
pub fn find<'a>(assets: &'a [Asset], path: &str) -> Option<&'a Asset> {
    let path = if path == "/" || path.starts_with("/i/") {
        "/index.html"
    } else {
        path
    };
    assets.iter().find(|a| a.path == path)
}

/// The fallback route: an embedded file, or 404.
pub async fn serve(method: Method, uri: Uri) -> Response {
    serve_from(ASSETS, &method, &uri)
}

fn serve_from(assets: &[Asset], method: &Method, uri: &Uri) -> Response {
    if !matches!(*method, Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    if assets.is_empty() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "This build of loyalty has no web app in it: run `make web` before `cargo build`.\n",
        )
            .into_response();
    }
    let Some(asset) = find(assets, uri.path()) else {
        return (StatusCode::NOT_FOUND, "Not found\n").into_response();
    };
    let mut response = asset.bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(asset.mime));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if asset.path.starts_with("/assets/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[Asset] = &[
        Asset {
            path: "/index.html",
            bytes: b"<html>",
            mime: "text/html; charset=utf-8",
        },
        Asset {
            path: "/assets/main-abc.js",
            bytes: b"js",
            mime: "text/javascript; charset=utf-8",
        },
        Asset {
            path: "/sw.js",
            bytes: b"sw",
            mime: "text/javascript; charset=utf-8",
        },
    ];

    fn get(path: &str) -> Response {
        serve_from(FILES, &Method::GET, &path.parse().unwrap())
    }

    fn header(response: &Response, name: header::HeaderName) -> &str {
        response
            .headers()
            .get(name)
            .map(|v| v.to_str().unwrap())
            .unwrap_or_default()
    }

    #[test]
    fn the_app_and_invite_links_are_the_page() {
        for path in ["/", "/i/0123abcd", "/index.html"] {
            let response = get(path);
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(
                header(&response, header::CONTENT_TYPE),
                "text/html; charset=utf-8",
                "{path}"
            );
        }
    }

    #[test]
    fn hashed_files_are_kept_for_a_year_and_the_rest_revalidated() {
        let table = [
            ("/assets/main-abc.js", "public, max-age=31536000, immutable"),
            ("/sw.js", "no-cache"),
            ("/", "no-cache"),
        ];
        for (path, expected) in table {
            assert_eq!(
                header(&get(path), header::CACHE_CONTROL),
                expected,
                "{path}"
            );
        }
    }

    #[test]
    fn a_missing_file_is_404_and_a_post_is_refused() {
        assert_eq!(get("/nothing.js").status(), StatusCode::NOT_FOUND);
        let post = serve_from(FILES, &Method::POST, &"/".parse().unwrap());
        assert_eq!(post.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[test]
    fn a_build_without_the_app_says_how_to_add_it() {
        let response = serve_from(&[], &Method::GET, &"/".parse().unwrap());
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
