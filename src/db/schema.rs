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

//! The schema, as a list of migrations. `PRAGMA user_version` is how many have run; opening a
//! database runs the rest, each in its own transaction, so a database is never left between two
//! versions.
//!
//! A migration is never edited once released: a database written by that release has already run
//! it. A change is a new entry at the end. `tests/upgrade.rs` opens a database written by every
//! release and checks that it arrives at the current schema with its rows intact.

use rusqlite::Connection;

pub const MIGRATIONS: &[&str] = &[
    // 1: households, their people, sessions, invites, cards and photos.
    "
    CREATE TABLE households (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        household_id INTEGER NOT NULL REFERENCES households(id),
        name TEXT NOT NULL UNIQUE COLLATE NOCASE,
        display_name TEXT NOT NULL,
        -- NULL until the person has set a password through their invite.
        pw_hash TEXT,
        is_admin INTEGER NOT NULL DEFAULT 0,
        created_at INTEGER NOT NULL
    );
    -- Only a token's SHA-256 is stored: a copy of the database signs nobody in.
    CREATE TABLE sessions (
        token_sha256 TEXT PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        last_seen_at INTEGER NOT NULL
    );
    -- An invite makes a new person (user_id NULL) or resets an existing one's password.
    CREATE TABLE invites (
        token_sha256 TEXT PRIMARY KEY,
        household_id INTEGER NOT NULL REFERENCES households(id),
        name TEXT NOT NULL,
        is_admin INTEGER NOT NULL DEFAULT 0,
        user_id INTEGER REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        expires_at INTEGER NOT NULL,
        used_at INTEGER
    );
    -- holder_user_id NULL is a family card. Only a person's own card can be private.
    CREATE TABLE cards (
        id INTEGER PRIMARY KEY,
        household_id INTEGER NOT NULL REFERENCES households(id),
        holder_user_id INTEGER REFERENCES users(id),
        private INTEGER NOT NULL DEFAULT 0 CHECK (private = 0 OR holder_user_id IS NOT NULL),
        name TEXT NOT NULL,
        program TEXT NOT NULL DEFAULT '',
        code_value TEXT NOT NULL,
        code_format TEXT NOT NULL,
        display_number TEXT NOT NULL DEFAULT '',
        color TEXT NOT NULL,
        notes TEXT NOT NULL DEFAULT '',
        use_count INTEGER NOT NULL DEFAULT 0,
        last_used_at INTEGER,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    CREATE INDEX cards_by_household ON cards(household_id);
    -- Photos by content: the URL of a photo is its hash, so a browser may keep it forever.
    CREATE TABLE blobs (
        sha256 TEXT PRIMARY KEY,
        mime TEXT NOT NULL,
        bytes BLOB NOT NULL
    );
    CREATE TABLE photos (
        card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
        side TEXT NOT NULL CHECK (side IN ('front', 'back')),
        sha256 TEXT NOT NULL REFERENCES blobs(sha256),
        PRIMARY KEY (card_id, side)
    );
    CREATE INDEX photos_by_blob ON photos(sha256);
    ",
];

/// Runs the migrations `conn` has not run yet.
pub fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    let done: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let done = usize::try_from(done).unwrap_or(0);
    anyhow::ensure!(
        done <= MIGRATIONS.len(),
        "the database is at schema version {done}, newer than this program ({}); \
         run the release that wrote it, or a later one",
        MIGRATIONS.len()
    );
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(done) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// The schema as SQLite reports it, one statement per line, in a stable order: what the golden
/// in `tests/golden/schema.sql` pins.
pub fn dump(conn: &Connection) -> anyhow::Result<String> {
    let mut statement = conn
        .prepare("SELECT sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type DESC, name")?;
    let rows = statement.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = String::new();
    for row in rows {
        out.push_str(&row?);
        out.push_str(";\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrating_twice_changes_nothing() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let before = dump(&conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(dump(&conn).unwrap(), before);
    }

    #[test]
    fn a_database_from_a_newer_release_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
            .unwrap();
        let err = migrate(&mut conn).unwrap_err().to_string();
        assert!(
            err.contains("newer than this program"),
            "the error was: {err}"
        );
    }

    #[test]
    fn the_schema_is_pinned() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        crate::golden::assert_golden("schema.sql", &dump(&conn).unwrap());
    }

    #[test]
    fn a_private_family_card_is_refused_by_the_database_itself() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO households (id, name, created_at) VALUES (1, 'Home', 1)",
            [],
        )
        .unwrap();
        let result = conn.execute(
            "INSERT INTO cards (household_id, private, name, code_value, code_format, color, created_at, updated_at)
             VALUES (1, 1, 'Shop', '1', 'qr', '#000000', 1, 1)",
            [],
        );
        assert!(
            result.is_err(),
            "a private card without a holder was stored"
        );
    }
}
