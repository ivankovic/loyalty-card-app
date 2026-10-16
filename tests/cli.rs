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

//! The real binary, the way the container and an administrator run it.

use std::io::{BufRead as _, BufReader};
use std::process::{Child, Command, Stdio};

fn loyalty(db: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_loyalty"));
    command.arg("--db").arg(db).env_remove("LOYALTY_ORIGIN");
    command
}

/// A server on a free port, and the address it printed.
fn serve(db: &std::path::Path) -> (Child, String) {
    let mut child = loyalty(db)
        .args([
            "serve",
            "--listen",
            "127.0.0.1:0",
            "--origin",
            "http://localhost",
        ])
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let first = lines.next().unwrap().unwrap();
    let address = first
        .strip_prefix("listening on ")
        .unwrap_or_else(|| panic!("the server's first line was {first:?}"))
        .to_string();
    // Keep reading the log, so that a full pipe never blocks the server.
    std::thread::spawn(move || lines.for_each(drop));
    (child, address)
}

#[test]
fn the_health_check_passes_against_a_running_server_and_fails_without_one() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("loyalty.db");
    let (mut server, address) = serve(&db);
    let check = loyalty(&db)
        .args(["healthz", "--address", &address])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "healthz failed: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&check.stdout), "ok\n");

    // SIGTERM, as `docker stop` sends it: the server ends cleanly.
    let pid = server.id().to_string();
    assert!(
        Command::new("kill")
            .args(["-TERM", &pid])
            .status()
            .unwrap()
            .success()
    );
    let status = server.wait().unwrap();
    assert!(status.success(), "the server ended with {status}");

    let check = loyalty(&db)
        .args(["healthz", "--address", &address])
        .output()
        .unwrap();
    assert!(!check.status.success(), "healthz passed with no server");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(
        stderr.starts_with("loyalty: nothing answers on"),
        "stderr was: {stderr}"
    );
}

#[test]
fn serving_needs_an_origin() {
    let dir = tempfile::tempdir().unwrap();
    let output = loyalty(&dir.path().join("loyalty.db"))
        .args(["serve", "--listen", "127.0.0.1:0"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--origin <ORIGIN>"), "stderr was: {stderr}");
}

#[test]
fn an_invite_is_printed_as_a_link_from_the_environment() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("loyalty.db");
    assert!(
        loyalty(&db)
            .args(["household", "add", "Home"])
            .status()
            .unwrap()
            .success()
    );
    let output = loyalty(&db)
        .args(["invite", "--household", "Home", "marko", "--admin"])
        .env("LOYALTY_ORIGIN", "https://cards.example")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("    https://cards.example/i/"),
        "stdout was:\n{stdout}"
    );
}

#[test]
fn a_failing_command_says_why_and_exits_non_zero() {
    let dir = tempfile::tempdir().unwrap();
    let output = loyalty(&dir.path().join("loyalty.db"))
        .args(["user", "remove", "nobody"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "loyalty: there is nobody called \"nobody\"\n"
    );
}
