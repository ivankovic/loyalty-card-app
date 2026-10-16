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

//! The database: one SQLite file holding everything, photos included, so that a backup is one
//! file and a restore is putting it back.
//!
//! One connection, behind the web server's mutex (see `web::AppState`). A household's traffic is
//! a few requests a minute; a pool would add nothing but the question of which connection saw
//! which write. WAL lets `backup` and the borg pull read while the server writes.

pub mod schema;

use anyhow::Context as _;
use rusqlite::Connection;
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Opens the database at `path`, creating it if it is not there, and brings its schema up to
    /// date.
    pub fn open(path: &Path) -> anyhow::Result<Db> {
        let conn = Connection::open(path)
            .with_context(|| format!("cannot open the database {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // FULL rather than WAL's usual NORMAL: a card saved is a card kept, even through a power
        // cut. The cost is an fsync per write, and writes are rare.
        conn.pragma_update(None, "synchronous", "FULL")?;
        Db::ready(conn)
    }

    /// A database in memory, for tests and for `--dry-run`-like checks.
    pub fn open_in_memory() -> anyhow::Result<Db> {
        Db::ready(Connection::open_in_memory()?)
    }

    fn ready(conn: Connection) -> anyhow::Result<Db> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let mut db = Db { conn };
        schema::migrate(&mut db.conn)?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Writes a consistent copy of the database to `target`, which must not exist yet. Safe while
    /// the server runs: SQLite copies from one read transaction.
    pub fn backup_to(&self, target: &Path) -> anyhow::Result<()> {
        anyhow::ensure!(
            !target.exists(),
            "{} already exists; a backup never overwrites a file",
            target.display()
        );
        let target = target
            .to_str()
            .context("the backup's path is not valid UTF-8")?;
        self.conn
            .execute("VACUUM INTO ?1", [target])
            .context("the backup failed")?;
        Ok(())
    }
}

/// Seconds since the Unix epoch: how every time is stored.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_file_is_created_with_the_current_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loyalty.db");
        let db = Db::open(&path).unwrap();
        let version: i64 = db
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, schema::MIGRATIONS.len() as i64);
        assert!(path.exists());
    }

    #[test]
    fn reopening_keeps_what_was_written() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loyalty.db");
        Db::open(&path)
            .unwrap()
            .conn()
            .execute(
                "INSERT INTO households (name, created_at) VALUES ('Home', 1)",
                [],
            )
            .unwrap();
        let name: String = Db::open(&path)
            .unwrap()
            .conn()
            .query_row("SELECT name FROM households", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Home");
    }

    #[test]
    fn a_backup_is_a_database_with_the_same_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("loyalty.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO households (name, created_at) VALUES ('Home', 1)",
                [],
            )
            .unwrap();
        let copy = dir.path().join("copy.db");
        db.backup_to(&copy).unwrap();
        let count: i64 = Db::open(&copy)
            .unwrap()
            .conn()
            .query_row("SELECT count(*) FROM households", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn a_backup_never_overwrites_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let there = dir.path().join("there.db");
        std::fs::write(&there, b"keep me").unwrap();
        let err = db.backup_to(&there).unwrap_err().to_string();
        assert!(err.contains("already exists"), "the error was: {err}");
        assert_eq!(std::fs::read(&there).unwrap(), b"keep me");
    }

    #[test]
    fn a_database_that_cannot_be_opened_names_its_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing-dir/loyalty.db");
        let err = format!("{:#}", Db::open(&path).err().unwrap());
        assert!(err.contains("missing-dir"), "the error was: {err}");
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let db = Db::open_in_memory().unwrap();
        let result = db.conn().execute(
            "INSERT INTO users (household_id, name, display_name, created_at) VALUES (99, 'a', 'A', 1)",
            [],
        );
        assert!(
            result.is_err(),
            "a user in a household that does not exist was accepted"
        );
    }
}
