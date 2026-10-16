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

//! Golden files: an exact copy of a generated text, kept in `tests/golden/`, that a test compares
//! against (only compiled for tests; see `lib.rs`).
//!
//! A substring assertion checks that one thing is present; a golden pins every byte, so an
//! accidental change anywhere (a reordered field, a renamed column, a lost line) fails a test
//! instead of reaching a phone or a database.
//!
//! To update after an intended change: `UPDATE_GOLDENS=1 cargo nextest run`, then review the
//! diff like any other change.

use std::path::PathBuf;

pub fn assert_golden(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name);
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden file tests/golden/{name}: generate it with \
             UPDATE_GOLDENS=1 cargo nextest run, then review and commit it"
        )
    });
    assert_eq!(
        actual, expected,
        "the output no longer matches tests/golden/{name}. If the change is intended, \
         regenerate with UPDATE_GOLDENS=1 cargo nextest run and review the golden's diff."
    );
}
