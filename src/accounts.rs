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

//! Households, the people in them, their passwords, sessions and invites.
//!
//! Nobody signs up. An administrator makes an invite (`loyalty invite`, or the app's
//! "Invite someone" for an admin), which is a one-time link; whoever opens it chooses the
//! password for that name. A forgotten password is a new invite for the same person
//! (`loyalty user reset-password`). So the server never sends mail and never shows a password.
//!
//! Tokens (sessions and invites) are 32 random bytes, shown once, and stored only as their
//! SHA-256: a copy of the database signs nobody in and redeems no invite.

use crate::db::Db;
use anyhow::Context as _;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rusqlite::{OptionalExtension as _, params};
use serde::Serialize;
use sha2::{Digest as _, Sha256};

/// How long an invite link works.
pub const INVITE_TTL_SECS: i64 = 7 * 24 * 3600;
/// A session ends after this long without a request. Long, because the app lives on a phone's
/// home screen and signing in at the till is the worst moment for it.
pub const SESSION_IDLE_SECS: i64 = 400 * 24 * 3600;
/// `last_seen_at` is written at most this often, not on every request.
const SESSION_TOUCH_SECS: i64 = 3600;
pub const MIN_PASSWORD: usize = 8;
const MAX_PASSWORD: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Household {
    pub id: i64,
    pub name: String,
}

/// A person, as the rest of the program sees them once signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct User {
    pub id: i64,
    pub household_id: i64,
    pub name: String,
    pub display_name: String,
    pub is_admin: bool,
}

/// Someone else in the household, as the app lists them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub display_name: String,
}

/// What an invite link says before it is used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InviteInfo {
    pub household: String,
    pub name: String,
    /// True when the invite resets an existing person's password.
    pub reset: bool,
    pub expires_at: i64,
}

/// Why an account operation was refused, in words for the person who asked.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Invalid(String),
    NotFound,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Invalid(why) => f.write_str(why),
            Refused::NotFound => f.write_str("not found"),
        }
    }
}

impl std::error::Error for Refused {}

// ---------------------------------------------------------------------------------------------
// Households

pub fn add_household(db: &Db, name: &str, now: i64) -> anyhow::Result<Household> {
    let name = name.trim();
    anyhow::ensure!(
        !name.is_empty() && name.chars().count() <= 60,
        "a household's name is 1 to 60 characters"
    );
    db.conn()
        .execute(
            "INSERT INTO households (name, created_at) VALUES (?1, ?2)",
            params![name, now],
        )
        .with_context(|| format!("cannot add the household {name:?}; is the name taken?"))?;
    Ok(Household {
        id: db.conn().last_insert_rowid(),
        name: name.to_string(),
    })
}

pub fn household_by_name(db: &Db, name: &str) -> anyhow::Result<Option<Household>> {
    Ok(db
        .conn()
        .query_row(
            "SELECT id, name FROM households WHERE name = ?1",
            [name],
            |r| {
                Ok(Household {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            },
        )
        .optional()?)
}

pub fn households(db: &Db) -> anyhow::Result<Vec<Household>> {
    let mut statement = db
        .conn()
        .prepare("SELECT id, name FROM households ORDER BY name")?;
    let rows = statement.query_map([], |r| {
        Ok(Household {
            id: r.get(0)?,
            name: r.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn household(db: &Db, id: i64) -> anyhow::Result<Household> {
    Ok(db
        .conn()
        .query_row("SELECT id, name FROM households WHERE id = ?1", [id], |r| {
            Ok(Household {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })?)
}

/// Everyone in a household who has set a password, by display name.
pub fn members(db: &Db, household_id: i64) -> anyhow::Result<Vec<Member>> {
    let mut statement = db.conn().prepare(
        "SELECT id, name, display_name FROM users
         WHERE household_id = ?1 AND pw_hash IS NOT NULL
         ORDER BY display_name COLLATE NOCASE, id",
    )?;
    let rows = statement.query_map([household_id], |r| {
        Ok(Member {
            id: r.get(0)?,
            name: r.get(1)?,
            display_name: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

// ---------------------------------------------------------------------------------------------
// People

/// A sign-in name: what someone types to sign in, so short and plain. Compared without case.
pub fn check_name(name: &str) -> Result<(), Refused> {
    let ok = (1..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(Refused::Invalid(
            "a name is 1 to 32 letters, digits, dots, dashes or underscores".into(),
        ))
    }
}

fn check_display_name(name: &str) -> Result<(), Refused> {
    let count = name.trim().chars().count();
    if (1..=40).contains(&count) && !name.chars().any(char::is_control) {
        Ok(())
    } else {
        Err(Refused::Invalid(
            "a display name is 1 to 40 characters".into(),
        ))
    }
}

pub fn check_password(password: &str) -> Result<(), Refused> {
    let count = password.chars().count();
    if count < MIN_PASSWORD {
        Err(Refused::Invalid(format!(
            "a password needs at least {MIN_PASSWORD} characters"
        )))
    } else if count > MAX_PASSWORD {
        Err(Refused::Invalid(format!(
            "a password has at most {MAX_PASSWORD} characters"
        )))
    } else {
        Ok(())
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(argon2::Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("cannot hash the password: {e}"))?
        .to_string())
}

fn password_matches(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        argon2::Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

const USER_COLUMNS: &str = "id, household_id, name, display_name, is_admin";

fn user_row(r: &rusqlite::Row) -> rusqlite::Result<User> {
    Ok(User {
        id: r.get(0)?,
        household_id: r.get(1)?,
        name: r.get(2)?,
        display_name: r.get(3)?,
        is_admin: r.get(4)?,
    })
}

pub fn user_by_name(db: &Db, name: &str) -> anyhow::Result<Option<User>> {
    Ok(db
        .conn()
        .query_row(
            &format!("SELECT {USER_COLUMNS} FROM users WHERE name = ?1"),
            [name],
            user_row,
        )
        .optional()?)
}

/// The person `name` if `password` is theirs. Someone who does not exist costs the same time as
/// a wrong password, so the answer does not tell which names exist.
pub fn sign_in(db: &Db, name: &str, password: &str) -> anyhow::Result<Option<User>> {
    let found: Option<(User, Option<String>)> = db
        .conn()
        .query_row(
            &format!("SELECT {USER_COLUMNS}, pw_hash FROM users WHERE name = ?1"),
            [name],
            |r| Ok((user_row(r)?, r.get(5)?)),
        )
        .optional()?;
    match found {
        Some((user, Some(hash))) if password_matches(&hash, password) => Ok(Some(user)),
        Some((_, Some(_))) => Ok(None),
        _ => {
            password_matches(dummy_hash(), password);
            Ok(None)
        }
    }
}

/// A hash to check against when there is none, so that a missing name costs one hash.
fn dummy_hash() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| hash_password("no one has this password").unwrap_or_default())
}

pub fn change_password(db: &Db, user: &User, current: &str, new: &str) -> anyhow::Result<()> {
    if sign_in(db, &user.name, current)?.is_none() {
        return Err(Refused::Invalid("the current password is not right".into()).into());
    }
    check_password(new)?;
    db.conn().execute(
        "UPDATE users SET pw_hash = ?1 WHERE id = ?2",
        params![hash_password(new)?, user.id],
    )?;
    Ok(())
}

/// A person as `loyalty user list` shows them.
#[derive(Debug, PartialEq, Eq)]
pub struct Listed {
    pub household: String,
    pub name: String,
    pub display_name: String,
    pub is_admin: bool,
    /// Whether they have set a password.
    pub arrived: bool,
}

/// Everyone, by household.
pub fn users(db: &Db) -> anyhow::Result<Vec<Listed>> {
    let mut statement = db.conn().prepare(
        "SELECT h.name, u.name, u.display_name, u.is_admin, u.pw_hash IS NOT NULL
         FROM users u JOIN households h ON h.id = u.household_id
         ORDER BY h.name, u.name",
    )?;
    let rows = statement.query_map([], |r| {
        Ok(Listed {
            household: r.get(0)?,
            name: r.get(1)?,
            display_name: r.get(2)?,
            is_admin: r.get(3)?,
            arrived: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Removes a person. Their private cards go with them; their other cards become the family's.
pub fn remove_user(db: &mut Db, name: &str) -> anyhow::Result<()> {
    let user = user_by_name(db, name)?.ok_or(Refused::NotFound)?;
    let tx = db.conn_mut().transaction()?;
    tx.execute(
        "DELETE FROM cards WHERE holder_user_id = ?1 AND private = 1",
        [user.id],
    )?;
    tx.execute(
        "UPDATE cards SET holder_user_id = NULL WHERE holder_user_id = ?1",
        [user.id],
    )?;
    tx.execute("DELETE FROM users WHERE id = ?1", [user.id])?;
    crate::photos::collect_garbage(&tx)?;
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Tokens

/// A new random token, as 64 hex characters.
pub fn new_token() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no randomness: {e}"))?;
    Ok(hex(&bytes))
}

pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------------------------
// Sessions

pub fn start_session(db: &Db, user: &User, now: i64) -> anyhow::Result<String> {
    let token = new_token()?;
    db.conn().execute(
        "INSERT INTO sessions (token_sha256, user_id, created_at, last_seen_at) VALUES (?1, ?2, ?3, ?3)",
        params![token_hash(&token), user.id, now],
    )?;
    Ok(token)
}

/// The person a session token belongs to, if the session is still open. Keeps it open.
pub fn session_user(db: &Db, token: &str, now: i64) -> anyhow::Result<Option<User>> {
    let hash = token_hash(token);
    let found: Option<(User, i64)> = db
        .conn()
        .query_row(
            "SELECT u.id, u.household_id, u.name, u.display_name, u.is_admin, s.last_seen_at
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_sha256 = ?1",
            [&hash],
            |r| Ok((user_row(r)?, r.get(5)?)),
        )
        .optional()?;
    let Some((user, last_seen)) = found else {
        return Ok(None);
    };
    if now - last_seen > SESSION_IDLE_SECS {
        db.conn()
            .execute("DELETE FROM sessions WHERE token_sha256 = ?1", [&hash])?;
        return Ok(None);
    }
    if now - last_seen > SESSION_TOUCH_SECS {
        db.conn().execute(
            "UPDATE sessions SET last_seen_at = ?1 WHERE token_sha256 = ?2",
            params![now, hash],
        )?;
    }
    Ok(Some(user))
}

pub fn end_session(db: &Db, token: &str) -> anyhow::Result<()> {
    db.conn().execute(
        "DELETE FROM sessions WHERE token_sha256 = ?1",
        [token_hash(token)],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Invites

/// An invite for a new person called `name` in `household`. Returns the token for the link.
pub fn invite(
    db: &Db,
    household: &Household,
    name: &str,
    is_admin: bool,
    now: i64,
) -> anyhow::Result<String> {
    check_name(name)?;
    if user_by_name(db, name)?.is_some() {
        return Err(Refused::Invalid(format!("the name {name:?} is taken")).into());
    }
    let token = new_token()?;
    db.conn().execute(
        "INSERT INTO invites (token_sha256, household_id, name, is_admin, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            token_hash(&token),
            household.id,
            name,
            is_admin,
            now,
            now + INVITE_TTL_SECS
        ],
    )?;
    Ok(token)
}

/// An invite that lets `name` choose a new password. Their sessions stay open until it is used.
pub fn reset_invite(db: &Db, name: &str, now: i64) -> anyhow::Result<String> {
    let user = user_by_name(db, name)?.ok_or(Refused::NotFound)?;
    let token = new_token()?;
    db.conn().execute(
        "INSERT INTO invites (token_sha256, household_id, name, is_admin, user_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            token_hash(&token),
            user.household_id,
            user.name,
            user.is_admin,
            user.id,
            now,
            now + INVITE_TTL_SECS
        ],
    )?;
    Ok(token)
}

struct OpenInvite {
    household_id: i64,
    household: String,
    name: String,
    is_admin: bool,
    user_id: Option<i64>,
    expires_at: i64,
}

fn open_invite(db: &Db, token: &str, now: i64) -> anyhow::Result<Option<OpenInvite>> {
    Ok(db
        .conn()
        .query_row(
            "SELECT i.household_id, h.name, i.name, i.is_admin, i.user_id, i.expires_at
             FROM invites i JOIN households h ON h.id = i.household_id
             WHERE i.token_sha256 = ?1 AND i.used_at IS NULL AND i.expires_at > ?2",
            params![token_hash(token), now],
            |r| {
                Ok(OpenInvite {
                    household_id: r.get(0)?,
                    household: r.get(1)?,
                    name: r.get(2)?,
                    is_admin: r.get(3)?,
                    user_id: r.get(4)?,
                    expires_at: r.get(5)?,
                })
            },
        )
        .optional()?)
}

/// What an unused, unexpired invite is for; `None` for any other token.
pub fn invite_info(db: &Db, token: &str, now: i64) -> anyhow::Result<Option<InviteInfo>> {
    Ok(open_invite(db, token, now)?.map(|i| InviteInfo {
        household: i.household,
        name: i.name,
        reset: i.user_id.is_some(),
        expires_at: i.expires_at,
    }))
}

/// Uses an invite: makes the person (or sets their new password) and returns them. A reset ends
/// every session the person had, since a reset is what someone does when a phone is lost.
pub fn accept_invite(
    db: &mut Db,
    token: &str,
    password: &str,
    display_name: Option<&str>,
    now: i64,
) -> anyhow::Result<User> {
    let invite = open_invite(db, token, now)?.ok_or(Refused::NotFound)?;
    check_password(password)?;
    let display_name = display_name
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or(&invite.name)
        .to_string();
    check_display_name(&display_name)?;
    let hash = hash_password(password)?;
    let tx = db.conn_mut().transaction()?;
    let user_id = match invite.user_id {
        Some(id) => {
            tx.execute(
                "UPDATE users SET pw_hash = ?1 WHERE id = ?2",
                params![hash, id],
            )?;
            tx.execute("DELETE FROM sessions WHERE user_id = ?1", [id])?;
            id
        }
        None => {
            tx.execute(
                "INSERT INTO users (household_id, name, display_name, pw_hash, is_admin, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    invite.household_id,
                    invite.name,
                    display_name,
                    hash,
                    invite.is_admin,
                    now
                ],
            )
            .map_err(|_| Refused::Invalid(format!("the name {:?} is taken", invite.name)))?;
            tx.last_insert_rowid()
        }
    };
    tx.execute(
        "UPDATE invites SET used_at = ?1 WHERE token_sha256 = ?2",
        params![now, token_hash(token)],
    )?;
    let user = tx.query_row(
        &format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"),
        [user_id],
        user_row,
    )?;
    tx.commit()?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Home;

    #[test]
    fn an_invite_makes_a_person_who_can_sign_in() {
        let mut home = Home::new();
        let token = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        let ana = accept_invite(&mut home.db, &token, "correct horse", Some("Ana"), 101).unwrap();
        assert_eq!(
            (ana.name.as_str(), ana.display_name.as_str()),
            ("ana", "Ana")
        );
        assert_eq!(
            sign_in(&home.db, "ana", "correct horse").unwrap(),
            Some(ana)
        );
    }

    #[test]
    fn names_are_signed_in_without_regard_to_case() {
        let home = Home::new();
        assert!(
            sign_in(&home.db, "MARKO", Home::PASSWORD)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn a_wrong_password_and_a_stranger_are_both_refused() {
        let home = Home::new();
        assert_eq!(sign_in(&home.db, "marko", "not it at all").unwrap(), None);
        assert_eq!(sign_in(&home.db, "nobody", Home::PASSWORD).unwrap(), None);
    }

    #[test]
    fn a_person_invited_but_not_yet_arrived_cannot_sign_in() {
        let home = Home::new();
        invite(&home.db, &home.household, "ana", false, 100).unwrap();
        assert_eq!(sign_in(&home.db, "ana", "").unwrap(), None);
    }

    #[test]
    fn an_invite_works_once() {
        let mut home = Home::new();
        let token = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        accept_invite(&mut home.db, &token, "correct horse", None, 101).unwrap();
        let again = accept_invite(&mut home.db, &token, "another one!", None, 102);
        assert!(again.is_err());
        assert_eq!(invite_info(&home.db, &token, 102).unwrap(), None);
    }

    #[test]
    fn an_invite_expires() {
        let mut home = Home::new();
        let token = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        let late = 100 + INVITE_TTL_SECS;
        assert_eq!(invite_info(&home.db, &token, late).unwrap(), None);
        assert!(accept_invite(&mut home.db, &token, "correct horse", None, late).is_err());
    }

    #[test]
    fn an_invite_says_what_it_is_for() {
        let home = Home::new();
        let token = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        assert_eq!(
            invite_info(&home.db, &token, 101).unwrap(),
            Some(InviteInfo {
                household: "Home".into(),
                name: "ana".into(),
                reset: false,
                expires_at: 100 + INVITE_TTL_SECS,
            })
        );
    }

    #[test]
    fn a_taken_name_cannot_be_invited() {
        let home = Home::new();
        let err = invite(&home.db, &home.household, "Marko", false, 100).unwrap_err();
        assert!(err.to_string().contains("taken"), "the error was: {err}");
    }

    #[test]
    fn two_invites_for_one_name_make_one_person() {
        let mut home = Home::new();
        let first = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        let second = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        accept_invite(&mut home.db, &first, "correct horse", None, 101).unwrap();
        let err = accept_invite(&mut home.db, &second, "correct horse", None, 102).unwrap_err();
        assert!(err.to_string().contains("taken"), "the error was: {err}");
    }

    #[test]
    fn names_and_passwords_are_checked() {
        let table: &[(&str, Result<(), ()>)] = &[
            ("ana", Ok(())),
            ("ana.m-1_x", Ok(())),
            ("", Err(())),
            ("ana maria", Err(())),
            ("žana", Err(())),
            (&"a".repeat(33), Err(())),
        ];
        for (name, expected) in table {
            assert_eq!(
                check_name(name).map_err(|_| ()),
                *expected,
                "the name was {name:?}"
            );
        }
        assert!(check_password("short").is_err());
        assert!(check_password("long enough").is_ok());
        assert!(check_password(&"x".repeat(257)).is_err());
    }

    #[test]
    fn a_short_password_is_refused_when_accepting() {
        let mut home = Home::new();
        let token = invite(&home.db, &home.household, "ana", false, 100).unwrap();
        let err = accept_invite(&mut home.db, &token, "short", None, 101).unwrap_err();
        assert!(err.to_string().contains("at least"), "the error was: {err}");
        assert!(
            invite_info(&home.db, &token, 101).unwrap().is_some(),
            "a refused attempt used the invite up"
        );
    }

    #[test]
    fn a_reset_sets_a_new_password_and_ends_old_sessions() {
        let mut home = Home::new();
        let session = start_session(&home.db, &home.marko, 100).unwrap();
        let token = reset_invite(&home.db, "marko", 100).unwrap();
        assert!(invite_info(&home.db, &token, 101).unwrap().unwrap().reset);
        accept_invite(&mut home.db, &token, "brand new pass", None, 101).unwrap();
        assert!(
            sign_in(&home.db, "marko", Home::PASSWORD)
                .unwrap()
                .is_none()
        );
        assert!(
            sign_in(&home.db, "marko", "brand new pass")
                .unwrap()
                .is_some()
        );
        assert_eq!(session_user(&home.db, &session, 102).unwrap(), None);
    }

    #[test]
    fn a_reset_for_nobody_is_refused() {
        let home = Home::new();
        assert!(reset_invite(&home.db, "nobody", 100).is_err());
    }

    #[test]
    fn a_session_signs_its_person_in_until_it_ends() {
        let home = Home::new();
        let token = start_session(&home.db, &home.marko, 100).unwrap();
        assert_eq!(
            session_user(&home.db, &token, 101).unwrap(),
            Some(home.marko.clone())
        );
        end_session(&home.db, &token).unwrap();
        assert_eq!(session_user(&home.db, &token, 102).unwrap(), None);
    }

    #[test]
    fn a_session_unused_for_too_long_ends() {
        let home = Home::new();
        let token = start_session(&home.db, &home.marko, 100).unwrap();
        let late = 100 + SESSION_IDLE_SECS + 1;
        assert_eq!(session_user(&home.db, &token, late).unwrap(), None);
        assert_eq!(
            session_user(&home.db, &token, 101).unwrap(),
            None,
            "the session was not removed"
        );
    }

    #[test]
    fn using_a_session_keeps_it_open() {
        let home = Home::new();
        let token = start_session(&home.db, &home.marko, 100).unwrap();
        let later = 100 + SESSION_IDLE_SECS - 10;
        assert!(session_user(&home.db, &token, later).unwrap().is_some());
        assert!(
            session_user(&home.db, &token, later + SESSION_IDLE_SECS - 10)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn only_a_tokens_hash_is_stored() {
        let home = Home::new();
        let token = start_session(&home.db, &home.marko, 100).unwrap();
        let stored: String = home
            .db
            .conn()
            .query_row("SELECT token_sha256 FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, token);
        assert_eq!(stored, token_hash(&token));
    }

    #[test]
    fn changing_a_password_needs_the_current_one() {
        let home = Home::new();
        let err = change_password(&home.db, &home.marko, "wrong", "another password").unwrap_err();
        assert!(err.to_string().contains("current"), "the error was: {err}");
        change_password(&home.db, &home.marko, Home::PASSWORD, "another password").unwrap();
        assert!(
            sign_in(&home.db, "marko", "another password")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn members_lists_only_people_who_arrived() {
        let home = Home::new();
        invite(&home.db, &home.household, "ana", false, 100).unwrap();
        let names: Vec<_> = members(&home.db, home.household.id)
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["marko"]);
    }

    #[test]
    fn households_are_listed_and_found_by_name() {
        let home = Home::new();
        add_household(&home.db, "Neighbours", 1).unwrap();
        let names: Vec<_> = households(&home.db)
            .unwrap()
            .into_iter()
            .map(|h| h.name)
            .collect();
        assert_eq!(names, ["Home", "Neighbours"]);
        assert_eq!(
            household_by_name(&home.db, "Home").unwrap(),
            Some(home.household.clone())
        );
        assert_eq!(household_by_name(&home.db, "Elsewhere").unwrap(), None);
        assert!(
            add_household(&home.db, "Home", 1).is_err(),
            "a second household called Home"
        );
        assert!(
            add_household(&home.db, "  ", 1).is_err(),
            "a household without a name"
        );
    }

    #[test]
    fn removing_a_person_hands_their_shared_cards_to_the_family() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let shared = home.card_for(Some(&ana), false);
        let private = home.card_for(Some(&ana), true);
        remove_user(&mut home.db, "ana").unwrap();
        let holder: Option<i64> = home
            .db
            .conn()
            .query_row(
                "SELECT holder_user_id FROM cards WHERE id = ?1",
                [shared],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(holder, None);
        let left: i64 = home
            .db
            .conn()
            .query_row("SELECT count(*) FROM cards WHERE id = ?1", [private], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(left, 0, "a private card outlived its holder");
        assert!(remove_user(&mut home.db, "ana").is_err());
    }

    #[test]
    fn users_lists_everyone_with_their_state() {
        let home = Home::new();
        invite(&home.db, &home.household, "ana", false, 100).unwrap();
        assert_eq!(
            users(&home.db).unwrap(),
            [Listed {
                household: "Home".into(),
                name: "marko".into(),
                display_name: "Marko".into(),
                is_admin: true,
                arrived: true,
            }]
        );
    }
}
