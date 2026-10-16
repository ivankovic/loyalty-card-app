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

//! What more than one test binary shares. A `mod.rs` in a folder, so that cargo treats it as a
//! module and not as a test binary of its own.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The container engine: `LOYALTY_CONTAINER_ENGINE`, or podman if there is one, or docker.
pub fn engine() -> String {
    if let Ok(engine) = std::env::var("LOYALTY_CONTAINER_ENGINE") {
        return engine;
    }
    let has = |name: &str| {
        Command::new(name)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    };
    if has("podman") { "podman" } else { "docker" }.to_string()
}

fn run(args: &[&str]) -> String {
    let output = Command::new(engine()).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{} {} failed:\n{}",
        engine(),
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

const IMAGE: &str = "localhost/loyalty-card-app:test";

/// Builds the image from the working tree, once per test binary.
fn image() -> &'static str {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let root = root();
        let status = Command::new(engine())
            .args([
                "build",
                "-q",
                "-f",
                "packaging/docker/Dockerfile",
                "-t",
                IMAGE,
                ".",
            ])
            .current_dir(&root)
            .status()
            .unwrap();
        assert!(status.success(), "the image did not build");
    });
    IMAGE
}

/// The app's container and an nginx in front of it, on a network of their own. Removed when
/// dropped.
pub struct Stack {
    id: String,
    pub port: String,
}

impl Stack {
    pub const ORIGIN: &'static str = "https://cards.test";

    /// `None` unless `LOYALTY_CONTAINER_TESTS=1`.
    pub fn start() -> Option<Stack> {
        if std::env::var("LOYALTY_CONTAINER_TESTS").as_deref() != Ok("1") {
            return None;
        }
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let id = format!(
            "loyalty-test-{}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::SeqCst)
        );
        let image = image();
        run(&["network", "create", &id]);
        let mut stack = Stack {
            id: id.clone(),
            port: String::new(),
        };
        // As the Compose service runs it: read-only, no capabilities, nobody.
        run(&[
            "run",
            "-d",
            "--name",
            &format!("{id}-app"),
            "--network",
            &id,
            "--network-alias",
            "loyalty",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--tmpfs",
            "/data:rw,mode=1777",
            "-e",
            &format!("LOYALTY_ORIGIN={}", Stack::ORIGIN),
            "-e",
            "LOYALTY_TRUST_PROXY=true",
            image,
        ]);
        let conf = root().join("tests/container/nginx.conf");
        run(&[
            "run",
            "-d",
            "--name",
            &format!("{id}-nginx"),
            "--network",
            &id,
            "-p",
            "127.0.0.1::80",
            "-v",
            &format!("{}:/etc/nginx/conf.d/default.conf:ro,Z", conf.display()),
            "docker.io/library/nginx:alpine",
        ]);
        let port = run(&["port", &format!("{id}-nginx"), "80"]);
        stack.port = port
            .lines()
            .next()
            .and_then(|l| l.rsplit(':').next())
            .unwrap_or_default()
            .trim()
            .to_string();
        stack.wait_until_ready();
        Some(stack)
    }

    fn wait_until_ready(&self) {
        for _ in 0..100 {
            if self.curl(&["/healthz"]).starts_with("HTTP/1.1 200") {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        panic!(
            "the stack never became ready; the app said:\n{}",
            run(&["logs", &format!("{}-app", self.id)])
        );
    }

    /// `curl -i` against nginx: the path first, then any other arguments.
    pub fn curl(&self, args: &[&str]) -> String {
        let url = format!("http://127.0.0.1:{}{}", self.port, args[0]);
        let output = Command::new("curl")
            .args(["-s", "-i", "--max-time", "5", &url])
            .args(&args[1..])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// Runs a command in the app's container.
    pub fn exec(&self, args: &[&str]) -> String {
        let mut all = vec!["exec", &self.id];
        let app = format!("{}-app", self.id);
        all[1] = &app;
        all.extend_from_slice(args);
        run(&all)
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        for name in [format!("{}-nginx", self.id), format!("{}-app", self.id)] {
            let _ = Command::new(engine())
                .args(["rm", "-f", "-t", "0", &name])
                .output();
        }
        let _ = Command::new(engine())
            .args(["network", "rm", &self.id])
            .output();
    }
}
