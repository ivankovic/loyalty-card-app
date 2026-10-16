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

//! Every release's database, opened by this one.
//!
//! `tests/fixtures/db/db-<version>.sql` is a dump of a database that release wrote, with a
//! household, people, cards and photos in it, recorded when the release is cut (see
//! CONTRIBUTING.md, "Releasing"). Each must open here, arrive at the current schema, and keep
//! every row. A migration that loses a card fails here, not on someone's phone.

use loyalty::db::{Db, schema};
use std::path::PathBuf;

/// The releases with a fixture, oldest first. A release adds itself here when it is cut.
const RELEASES: &[&str] = &[];

fn fixture(version: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/db/db-{version}.sql"))
}

#[test]
fn every_fixture_on_disk_is_listed() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/db");
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter_map(|n| Some(n.strip_prefix("db-")?.strip_suffix(".sql")?.to_string()))
                .collect()
        })
        .unwrap_or_default();
    on_disk.sort();
    let mut listed: Vec<String> = RELEASES.iter().map(|r| r.to_string()).collect();
    listed.sort();
    assert_eq!(on_disk, listed, "tests/fixtures/db and RELEASES disagree");
}

#[test]
fn every_release_database_opens_with_its_rows() {
    for version in RELEASES {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loyalty.db");
        let sql = std::fs::read_to_string(fixture(version)).unwrap();
        // Loaded as the old release left it: without this release's migrations.
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(&sql)
            .unwrap();
        let count = |db: &Db, table: &str| -> i64 {
            db.conn()
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        };
        let db = Db::open(&path)
            .unwrap_or_else(|e| panic!("the {version} database does not open: {e:#}"));
        let version_now: i64 = db
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version_now, schema::MIGRATIONS.len() as i64, "{version}");
        for table in ["households", "users", "cards", "photos", "blobs"] {
            assert!(
                count(&db, table) > 0,
                "the {version} database lost its {table}"
            );
        }
    }
}
