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

//! Photos of a card's front and back.
//!
//! The app shrinks a photo and re-encodes it before sending it (at most 1600 pixels a side, JPEG
//! or WebP), which also drops the camera's EXIF data: where and when the photo was taken stays on
//! the phone. Here a photo is only checked to be an image by its first bytes and to fit
//! [`MAX_BYTES`]; it is stored as sent.
//!
//! Photos are stored by content (`blobs`, keyed by SHA-256) and the card points at its hash. The
//! URL of a photo is its hash, so it never changes meaning and a browser can keep it forever; a
//! changed photo is a new URL. A blob no card points at any more is deleted in the same
//! transaction that let go of it.

use crate::accounts::User;
use crate::cards::{self, CardError};
use crate::db::Db;
use rusqlite::{OptionalExtension as _, params};
use sha2::{Digest as _, Sha256};

/// The largest photo accepted. A 1600-pixel JPEG at the app's quality is around 300 KB; this
/// leaves room for a detailed card without letting the database fill with full-size camera files.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Front,
    Back,
}

impl Side {
    pub fn parse(text: &str) -> Option<Side> {
        match text {
            "front" => Some(Side::Front),
            "back" => Some(Side::Back),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Side::Front => "front",
            Side::Back => "back",
        }
    }
}

/// The image type of `bytes`, from its first bytes, if it is one the app sends.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Sets the photo of one side of a card. Returns its hash.
pub fn put(
    db: &mut Db,
    by: &User,
    card: i64,
    side: Side,
    bytes: &[u8],
    now: i64,
) -> Result<String, CardError> {
    cards::get(db, by, card)?;
    if bytes.len() > MAX_BYTES {
        return Err(CardError::Invalid(format!(
            "a photo is at most {} MB",
            MAX_BYTES / (1024 * 1024)
        )));
    }
    let mime = sniff(bytes)
        .ok_or_else(|| CardError::Invalid("a photo is a JPEG, PNG or WebP image".into()))?;
    let hash = crate::accounts::hex(&Sha256::digest(bytes));
    let tx = db.conn_mut().transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO blobs (sha256, mime, bytes) VALUES (?1, ?2, ?3)",
        params![hash, mime, bytes],
    )?;
    tx.execute(
        "INSERT INTO photos (card_id, side, sha256) VALUES (?1, ?2, ?3)
         ON CONFLICT (card_id, side) DO UPDATE SET sha256 = excluded.sha256",
        params![card, side.as_str(), hash],
    )?;
    tx.execute(
        "UPDATE cards SET updated_at = ?1 WHERE id = ?2",
        params![now, card],
    )?;
    collect_garbage(&tx)?;
    tx.commit()?;
    Ok(hash)
}

pub fn remove(db: &mut Db, by: &User, card: i64, side: Side, now: i64) -> Result<(), CardError> {
    cards::get(db, by, card)?;
    let tx = db.conn_mut().transaction()?;
    tx.execute(
        "DELETE FROM photos WHERE card_id = ?1 AND side = ?2",
        params![card, side.as_str()],
    )?;
    tx.execute(
        "UPDATE cards SET updated_at = ?1 WHERE id = ?2",
        params![now, card],
    )?;
    collect_garbage(&tx)?;
    tx.commit()?;
    Ok(())
}

/// A photo by its hash, if it belongs to a card `viewer` may see: (type, bytes).
pub fn get(db: &Db, viewer: &User, hash: &str) -> Result<Option<(String, Vec<u8>)>, CardError> {
    Ok(db
        .conn()
        .query_row(
            "SELECT b.mime, b.bytes FROM blobs b
             WHERE b.sha256 = ?1 AND EXISTS (
                 SELECT 1 FROM photos p JOIN cards c ON c.id = p.card_id
                 WHERE p.sha256 = b.sha256 AND c.household_id = ?2
                   AND (c.private = 0 OR c.holder_user_id = ?3))",
            params![hash, viewer.household_id, viewer.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// Deletes the photos no card points at any more.
pub fn collect_garbage(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM blobs WHERE sha256 NOT IN (SELECT sha256 FROM photos)",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Home;

    const JPEG: &[u8] = b"\xff\xd8\xff\xe0 a tiny jpeg";
    const OTHER_JPEG: &[u8] = b"\xff\xd8\xff\xe0 another one";

    fn blobs(db: &Db) -> i64 {
        db.conn()
            .query_row("SELECT count(*) FROM blobs", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn images_are_told_by_their_first_bytes() {
        let table: &[(&[u8], Option<&str>)] = &[
            (JPEG, Some("image/jpeg")),
            (b"\x89PNG\r\n\x1a\n....", Some("image/png")),
            (b"RIFF\x10\0\0\0WEBPVP8 ", Some("image/webp")),
            (b"RIFF\x10\0\0\0WAVE", None),
            (b"<svg xmlns=", None),
            (b"", None),
        ];
        for (bytes, expected) in table {
            assert_eq!(sniff(bytes), *expected, "the bytes were {bytes:?}");
        }
    }

    #[test]
    fn a_photo_is_stored_and_shown_on_its_card() {
        let mut home = Home::new();
        let card = home.card_for(None, false);
        let marko = home.marko.clone();
        let hash = put(&mut home.db, &marko, card, Side::Front, JPEG, 70).unwrap();
        let shown = cards::get(&home.db, &marko, card).unwrap();
        assert_eq!(
            (shown.front.as_deref(), shown.back, shown.updated_at),
            (Some(hash.as_str()), None, 70)
        );
        assert_eq!(
            get(&home.db, &marko, &hash).unwrap(),
            Some(("image/jpeg".to_string(), JPEG.to_vec()))
        );
    }

    #[test]
    fn replacing_a_photo_deletes_the_old_one() {
        let mut home = Home::new();
        let card = home.card_for(None, false);
        let marko = home.marko.clone();
        let old = put(&mut home.db, &marko, card, Side::Back, JPEG, 70).unwrap();
        put(&mut home.db, &marko, card, Side::Back, OTHER_JPEG, 71).unwrap();
        assert_eq!(blobs(&home.db), 1);
        assert_eq!(get(&home.db, &marko, &old).unwrap(), None);
    }

    #[test]
    fn the_same_photo_on_two_cards_is_stored_once_and_outlives_one_of_them() {
        let mut home = Home::new();
        let first = home.card_for(None, false);
        let second = home.card_for(None, false);
        let marko = home.marko.clone();
        let hash = put(&mut home.db, &marko, first, Side::Front, JPEG, 70).unwrap();
        put(&mut home.db, &marko, second, Side::Front, JPEG, 70).unwrap();
        assert_eq!(blobs(&home.db), 1);
        cards::delete(&mut home.db, &marko, first).unwrap();
        assert!(get(&home.db, &marko, &hash).unwrap().is_some());
        cards::delete(&mut home.db, &marko, second).unwrap();
        assert_eq!(blobs(&home.db), 0);
    }

    #[test]
    fn removing_a_photo_deletes_it() {
        let mut home = Home::new();
        let card = home.card_for(None, false);
        let marko = home.marko.clone();
        put(&mut home.db, &marko, card, Side::Front, JPEG, 70).unwrap();
        remove(&mut home.db, &marko, card, Side::Front, 71).unwrap();
        assert_eq!(blobs(&home.db), 0);
        assert_eq!(cards::get(&home.db, &marko, card).unwrap().front, None);
    }

    #[test]
    fn what_is_not_an_image_or_too_big_is_refused() {
        let mut home = Home::new();
        let card = home.card_for(None, false);
        let marko = home.marko.clone();
        let err = put(&mut home.db, &marko, card, Side::Front, b"<html>", 70).unwrap_err();
        assert_eq!(
            err,
            CardError::Invalid("a photo is a JPEG, PNG or WebP image".into())
        );
        let mut big = JPEG.to_vec();
        big.resize(MAX_BYTES + 1, 0);
        let err = put(&mut home.db, &marko, card, Side::Front, &big, 70).unwrap_err();
        assert_eq!(err, CardError::Invalid("a photo is at most 2 MB".into()));
        assert_eq!(blobs(&home.db), 0);
    }

    #[test]
    fn a_photo_is_shown_only_to_who_may_see_its_card() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let (_, nina) = home.neighbours();
        let secret = home.card_for(Some(&ana), true);
        let hash = put(&mut home.db, &ana, secret, Side::Front, JPEG, 70).unwrap();
        assert!(get(&home.db, &ana, &hash).unwrap().is_some());
        assert_eq!(get(&home.db, &home.marko, &hash).unwrap(), None);
        assert_eq!(get(&home.db, &nina, &hash).unwrap(), None);
        let marko = home.marko.clone();
        assert_eq!(
            put(&mut home.db, &marko, secret, Side::Back, JPEG, 70),
            Err(CardError::NotFound)
        );
        assert_eq!(
            remove(&mut home.db, &nina, secret, Side::Front, 70),
            Err(CardError::NotFound)
        );
    }

    #[test]
    fn sides_are_named_front_and_back() {
        assert_eq!(Side::parse("front"), Some(Side::Front));
        assert_eq!(Side::parse("back"), Some(Side::Back));
        assert_eq!(Side::parse("side"), None);
    }
}
