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

//! The container image behind a real nginx, as it runs on a server: built from
//! `packaging/docker/Dockerfile`, proxied by `nginx:alpine` with `packaging/nginx/loyalty.conf`,
//! and asked with curl.
//!
//! Only with `LOYALTY_CONTAINER_TESTS=1` (every test returns at once without it): they need
//! podman or docker (`LOYALTY_CONTAINER_ENGINE` picks one), build an image, and take a minute.
//! `make integration-test` runs them.

mod common;

use common::Stack;

#[test]
fn the_app_is_served_through_nginx() {
    let Some(stack) = Stack::start() else { return };
    let page = stack.curl(&["/"]);
    assert!(page.starts_with("HTTP/1.1 200"), "the answer was:\n{page}");
    assert!(
        page.contains("<title>Cards</title>"),
        "the page was:\n{page}"
    );
    assert!(
        page.to_lowercase()
            .contains("content-security-policy: default-src 'self'"),
        "{page}"
    );
}

#[test]
fn the_health_check_passes_inside_the_container() {
    let Some(stack) = Stack::start() else { return };
    let status = stack.exec(&["/loyalty", "healthz"]);
    assert_eq!(status.trim(), "ok");
}

#[test]
fn an_invited_person_signs_in_through_the_proxy() {
    let Some(stack) = Stack::start() else { return };
    stack.exec(&["/loyalty", "household", "add", "Home"]);
    let link = stack.exec(&["/loyalty", "invite", "--household", "Home", "marko"]);
    let token = link
        .split("/i/")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .unwrap_or_else(|| panic!("no link in:\n{link}"));
    let accepted = stack.curl(&[
        &format!("/api/invite/{token}"),
        "-X",
        "POST",
        "-H",
        &format!("Origin: {}", Stack::ORIGIN),
        "-H",
        "X-Loyalty: 1",
        "-H",
        "Content-Type: application/json",
        "--data",
        r#"{"password":"a good password"}"#,
    ]);
    assert!(
        accepted.starts_with("HTTP/1.1 200"),
        "the answer was:\n{accepted}"
    );
    assert!(
        accepted.contains("set-cookie: __Host-loyalty="),
        "the answer was:\n{accepted}"
    );
}
