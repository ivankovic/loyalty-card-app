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

//! Embeds the web app, `web/dist`, into the binary, so that the container image is one file.
//!
//! Writes `$OUT_DIR/assets.rs`: one entry per file under `web/dist`, with its URL path, its bytes
//! (`include_bytes!`) and its content type. `make web` builds `web/dist`. Without it the binary
//! still builds, with no files, and the server says so on every page: the Rust tests do not need
//! the web app, and a CI job that only checks the Rust should not have to build it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = root.join("web/dist");
    println!("cargo:rerun-if-changed=web/dist");
    let mut files = Vec::new();
    if dist.is_dir() {
        collect(&dist, &mut files);
    } else {
        println!(
            "cargo:warning=web/dist is missing: this build serves no web app (run `make web`)"
        );
    }
    files.sort();
    let mut out = String::from("pub static ASSETS: &[Asset] = &[\n");
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let path = file
            .strip_prefix(&dist)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        writeln!(
            out,
            "    Asset {{ path: {:?}, bytes: include_bytes!({:?}), mime: {:?} }},",
            format!("/{path}"),
            file.display().to_string(),
            mime(&path)
        )
        .unwrap();
    }
    out.push_str("];\n");
    let target = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("assets.rs");
    std::fs::write(target, out).unwrap();
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

/// The content type for a file name. A type not listed here is served as bytes, so a new kind of
/// file shows up as a download rather than being guessed at.
fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "webmanifest" => "application/manifest+json",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
