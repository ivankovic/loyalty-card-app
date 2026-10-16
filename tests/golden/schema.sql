CREATE TABLE blobs (
        sha256 TEXT PRIMARY KEY,
        mime TEXT NOT NULL,
        bytes BLOB NOT NULL
    );
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
CREATE TABLE households (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        created_at INTEGER NOT NULL
    );
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
CREATE TABLE photos (
        card_id INTEGER NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
        side TEXT NOT NULL CHECK (side IN ('front', 'back')),
        sha256 TEXT NOT NULL REFERENCES blobs(sha256),
        PRIMARY KEY (card_id, side)
    );
CREATE TABLE sessions (
        token_sha256 TEXT PRIMARY KEY,
        user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
        created_at INTEGER NOT NULL,
        last_seen_at INTEGER NOT NULL
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
CREATE INDEX cards_by_household ON cards(household_id);
CREATE INDEX photos_by_blob ON photos(sha256);
