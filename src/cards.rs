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

//! Cards: what each one is, who holds it, who may see it, and the checks on what is stored.
//!
//! A card belongs to a household. It is held by one person in it (a flight miles card) or by
//! nobody, which makes it the family's (a supermarket card). Everyone in the household sees and
//! may change every card, so that a parent can show a child's card at the till, except a card
//! its holder marked private: that one exists only for its holder. A card from another
//! household, or another person's private card, is reported as not found, never as forbidden,
//! so that its existence is not given away.
//!
//! The barcode is stored as the value it encodes and its symbology; the app draws it. What is
//! checked here is what a scanner at a till would reject anyway (a wrong EAN check digit, a
//! letter in a UPC), so that a typing mistake is caught when the card is saved, not at the till.

use crate::accounts::User;
use crate::db::Db;
use rusqlite::{OptionalExtension as _, params};
use serde::{Deserialize, Serialize};

/// The symbologies the app can draw and read, named as the API spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeFormat {
    Qr,
    Ean13,
    Ean8,
    Upca,
    Upce,
    Code128,
    Code39,
    Code93,
    Itf,
    Codabar,
    Pdf417,
    Aztec,
    Datamatrix,
}

impl CodeFormat {
    pub const ALL: [CodeFormat; 13] = [
        CodeFormat::Qr,
        CodeFormat::Ean13,
        CodeFormat::Ean8,
        CodeFormat::Upca,
        CodeFormat::Upce,
        CodeFormat::Code128,
        CodeFormat::Code39,
        CodeFormat::Code93,
        CodeFormat::Itf,
        CodeFormat::Codabar,
        CodeFormat::Pdf417,
        CodeFormat::Aztec,
        CodeFormat::Datamatrix,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CodeFormat::Qr => "qr",
            CodeFormat::Ean13 => "ean13",
            CodeFormat::Ean8 => "ean8",
            CodeFormat::Upca => "upca",
            CodeFormat::Upce => "upce",
            CodeFormat::Code128 => "code128",
            CodeFormat::Code39 => "code39",
            CodeFormat::Code93 => "code93",
            CodeFormat::Itf => "itf",
            CodeFormat::Codabar => "codabar",
            CodeFormat::Pdf417 => "pdf417",
            CodeFormat::Aztec => "aztec",
            CodeFormat::Datamatrix => "datamatrix",
        }
    }

    pub fn parse(text: &str) -> Option<CodeFormat> {
        CodeFormat::ALL.into_iter().find(|f| f.as_str() == text)
    }

    /// Two-dimensional codes carry any text, line breaks included.
    fn is_2d(self) -> bool {
        matches!(
            self,
            CodeFormat::Qr | CodeFormat::Pdf417 | CodeFormat::Aztec | CodeFormat::Datamatrix
        )
    }

    /// Whether `value` can be encoded in this symbology, and if not, why.
    pub fn check(self, value: &str) -> Result<(), String> {
        let digits = value.bytes().all(|b| b.is_ascii_digit());
        let need_digits = |lengths: &[usize]| {
            if !digits || !lengths.contains(&value.len()) {
                let lengths: Vec<String> = lengths.iter().map(usize::to_string).collect();
                return Err(format!(
                    "{} takes {} digits",
                    self.label(),
                    lengths.join(" or ")
                ));
            }
            Ok(())
        };
        match self {
            CodeFormat::Ean13 => {
                need_digits(&[12, 13])?;
                check_digit(value, 13)
            }
            CodeFormat::Ean8 => {
                need_digits(&[7, 8])?;
                check_digit(value, 8)
            }
            CodeFormat::Upca => {
                need_digits(&[11, 12])?;
                check_digit(value, 12)
            }
            CodeFormat::Upce => need_digits(&[6, 7, 8]),
            CodeFormat::Itf => {
                if digits && value.len() >= 2 && value.len().is_multiple_of(2) {
                    Ok(())
                } else {
                    Err("ITF takes an even number of digits".into())
                }
            }
            CodeFormat::Code39 => {
                if value
                    .chars()
                    .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase() || " -.$/+%".contains(c))
                {
                    Ok(())
                } else {
                    Err("Code 39 takes digits, capital letters and - . $ / + % and space".into())
                }
            }
            CodeFormat::Codabar => {
                let inner = value
                    .strip_prefix(['A', 'B', 'C', 'D'])
                    .and_then(|v| v.strip_suffix(['A', 'B', 'C', 'D']))
                    .unwrap_or(value);
                if !inner.is_empty()
                    && inner
                        .chars()
                        .all(|c| c.is_ascii_digit() || "-$:/.+".contains(c))
                {
                    Ok(())
                } else {
                    Err("Codabar takes digits and - $ : / . +".into())
                }
            }
            CodeFormat::Code128 | CodeFormat::Code93 => {
                if value.bytes().all(|b| (0x20..0x7f).contains(&b)) {
                    Ok(())
                } else {
                    Err(format!(
                        "{} takes letters, digits and punctuation without accents",
                        self.label()
                    ))
                }
            }
            CodeFormat::Qr | CodeFormat::Pdf417 | CodeFormat::Aztec | CodeFormat::Datamatrix => {
                Ok(())
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            CodeFormat::Qr => "QR",
            CodeFormat::Ean13 => "EAN-13",
            CodeFormat::Ean8 => "EAN-8",
            CodeFormat::Upca => "UPC-A",
            CodeFormat::Upce => "UPC-E",
            CodeFormat::Code128 => "Code 128",
            CodeFormat::Code39 => "Code 39",
            CodeFormat::Code93 => "Code 93",
            CodeFormat::Itf => "ITF",
            CodeFormat::Codabar => "Codabar",
            CodeFormat::Pdf417 => "PDF417",
            CodeFormat::Aztec => "Aztec",
            CodeFormat::Datamatrix => "Data Matrix",
        }
    }
}

/// The GS1 check digit (EAN and UPC): a value one digit short of `full` is fine (the app adds
/// it), a full one must end in the right digit.
fn check_digit(value: &str, full: usize) -> Result<(), String> {
    if value.len() < full {
        return Ok(());
    }
    let digits: Vec<u32> = value.bytes().map(|b| u32::from(b - b'0')).collect();
    let (body, last) = digits.split_at(full - 1);
    let sum: u32 = body
        .iter()
        .rev()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { d * 3 } else { *d })
        .sum();
    let expected = (10 - sum % 10) % 10;
    if last[0] == expected {
        Ok(())
    } else {
        Err(format!(
            "the last digit should be {expected}: the number has a typing mistake"
        ))
    }
}

/// A card as the app sends it to be saved.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardInput {
    /// The person who holds the card; `None` for the family.
    pub holder: Option<i64>,
    #[serde(default)]
    pub private: bool,
    pub name: String,
    #[serde(default)]
    pub program: String,
    pub code_value: String,
    pub code_format: CodeFormat,
    /// The number printed under the barcode, when it differs from what the code encodes.
    #[serde(default)]
    pub display_number: String,
    pub color: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    pub id: i64,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Code {
    pub format: CodeFormat,
    pub value: String,
}

/// A card as the app receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub id: i64,
    pub holder: Option<Holder>,
    pub private: bool,
    pub name: String,
    pub program: String,
    pub code: Code,
    pub display_number: String,
    pub color: String,
    pub notes: String,
    /// The photos' hashes: each is served at `/photos/<hash>`.
    pub front: Option<String>,
    pub back: Option<String>,
    pub use_count: i64,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CardError {
    NotFound,
    Invalid(String),
    /// The database failed: the server's problem, not the request's.
    Database(String),
}

impl std::fmt::Display for CardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardError::NotFound => f.write_str("no such card"),
            CardError::Invalid(why) | CardError::Database(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for CardError {}

impl From<rusqlite::Error> for CardError {
    fn from(err: rusqlite::Error) -> Self {
        CardError::Database(format!("the database failed: {err}"))
    }
}

fn invalid(why: impl Into<String>) -> CardError {
    CardError::Invalid(why.into())
}

fn text(field: &str, value: &str, min: usize, max: usize) -> Result<(), CardError> {
    let count = value.chars().count();
    if count < min || count > max {
        return Err(if min > 0 {
            invalid(format!("{field}: {min} to {max} characters"))
        } else {
            invalid(format!("{field}: at most {max} characters"))
        });
    }
    Ok(())
}

impl CardInput {
    /// The input with spaces trimmed, checked against everything but the household.
    fn cleaned(&self) -> Result<CardInput, CardError> {
        let mut input = self.clone();
        input.name = input.name.trim().to_string();
        input.program = input.program.trim().to_string();
        input.display_number = input.display_number.trim().to_string();
        input.code_value = input.code_value.trim().to_string();
        text("Name", &input.name, 1, 60)?;
        text("Programme", &input.program, 0, 60)?;
        text("Printed number", &input.display_number, 0, 64)?;
        text("Notes", &input.notes, 0, 2000)?;
        text("Code", &input.code_value, 1, 2048)?;
        let line_breaks_ok = input.code_format.is_2d();
        if input
            .code_value
            .chars()
            .any(|c| c.is_control() && !(line_breaks_ok && matches!(c, '\n' | '\r' | '\t')))
        {
            return Err(invalid("the code has a control character in it"));
        }
        for (field, value) in [
            ("name", &input.name),
            ("programme", &input.program),
            ("printed number", &input.display_number),
        ] {
            if value.chars().any(char::is_control) {
                return Err(invalid(format!(
                    "the {field} has a control character in it"
                )));
            }
        }
        input
            .code_format
            .check(&input.code_value)
            .map_err(invalid)?;
        let colour = input.color.as_bytes();
        if colour.len() != 7 || colour[0] != b'#' || !colour[1..].iter().all(u8::is_ascii_hexdigit)
        {
            return Err(invalid("the colour is #rrggbb"));
        }
        input.color = input.color.to_ascii_lowercase();
        if input.private && input.holder.is_none() {
            return Err(invalid("a family card cannot be private"));
        }
        Ok(input)
    }

    /// Checks the holder against the household and the person saving.
    fn check_holder(&self, db: &Db, by: &User) -> Result<(), CardError> {
        if let Some(holder) = self.holder {
            let found: Option<i64> = db
                .conn()
                .query_row(
                    "SELECT id FROM users WHERE id = ?1 AND household_id = ?2",
                    params![holder, by.household_id],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_none() {
                return Err(invalid("the holder is not in this household"));
            }
            if self.private && holder != by.id {
                return Err(invalid("only its holder can make a card private"));
            }
        }
        Ok(())
    }
}

const CARD_SELECT: &str = "
    SELECT c.id, c.holder_user_id, u.display_name, c.private, c.name, c.program, c.code_value,
           c.code_format, c.display_number, c.color, c.notes,
           (SELECT sha256 FROM photos WHERE card_id = c.id AND side = 'front'),
           (SELECT sha256 FROM photos WHERE card_id = c.id AND side = 'back'),
           c.use_count, c.last_used_at, c.created_at, c.updated_at
    FROM cards c LEFT JOIN users u ON u.id = c.holder_user_id";

/// The condition for the cards `?1` (a household) and `?2` (a person in it) may see.
const VISIBLE: &str = "c.household_id = ?1 AND (c.private = 0 OR c.holder_user_id = ?2)";

fn card_row(r: &rusqlite::Row) -> rusqlite::Result<Card> {
    let holder: Option<i64> = r.get(1)?;
    let format: String = r.get(7)?;
    Ok(Card {
        id: r.get(0)?,
        holder: holder.map(|id| Holder {
            id,
            display_name: r
                .get::<_, Option<String>>(2)
                .ok()
                .flatten()
                .unwrap_or_default(),
        }),
        private: r.get(3)?,
        name: r.get(4)?,
        program: r.get(5)?,
        code: Code {
            // The schema only ever receives formats from `CodeFormat`.
            format: CodeFormat::parse(&format).unwrap_or(CodeFormat::Qr),
            value: r.get(6)?,
        },
        display_number: r.get(8)?,
        color: r.get(9)?,
        notes: r.get(10)?,
        front: r.get(11)?,
        back: r.get(12)?,
        use_count: r.get(13)?,
        last_used_at: r.get(14)?,
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
    })
}

/// Every card `viewer` may see, the most used first.
pub fn list(db: &Db, viewer: &User) -> Result<Vec<Card>, CardError> {
    let mut statement = db.conn().prepare(&format!(
        "{CARD_SELECT} WHERE {VISIBLE} ORDER BY c.use_count DESC, c.name COLLATE NOCASE, c.id"
    ))?;
    let rows = statement.query_map(params![viewer.household_id, viewer.id], card_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(db: &Db, viewer: &User, id: i64) -> Result<Card, CardError> {
    db.conn()
        .query_row(
            &format!("{CARD_SELECT} WHERE {VISIBLE} AND c.id = ?3"),
            params![viewer.household_id, viewer.id, id],
            card_row,
        )
        .optional()?
        .ok_or(CardError::NotFound)
}

pub fn create(db: &Db, by: &User, input: &CardInput, now: i64) -> Result<Card, CardError> {
    let input = input.cleaned()?;
    input.check_holder(db, by)?;
    db.conn().execute(
        "INSERT INTO cards (household_id, holder_user_id, private, name, program, code_value,
                            code_format, display_number, color, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
        params![
            by.household_id,
            input.holder,
            input.private,
            input.name,
            input.program,
            input.code_value,
            input.code_format.as_str(),
            input.display_number,
            input.color,
            input.notes,
            now
        ],
    )?;
    get(db, by, db.conn().last_insert_rowid())
}

pub fn update(db: &Db, by: &User, id: i64, input: &CardInput, now: i64) -> Result<Card, CardError> {
    get(db, by, id)?;
    let input = input.cleaned()?;
    input.check_holder(db, by)?;
    db.conn().execute(
        "UPDATE cards SET holder_user_id = ?1, private = ?2, name = ?3, program = ?4,
                          code_value = ?5, code_format = ?6, display_number = ?7, color = ?8,
                          notes = ?9, updated_at = ?10
         WHERE id = ?11",
        params![
            input.holder,
            input.private,
            input.name,
            input.program,
            input.code_value,
            input.code_format.as_str(),
            input.display_number,
            input.color,
            input.notes,
            now,
            id
        ],
    )?;
    get(db, by, id)
}

pub fn delete(db: &mut Db, by: &User, id: i64) -> Result<(), CardError> {
    get(db, by, id)?;
    let tx = db.conn_mut().transaction()?;
    tx.execute("DELETE FROM cards WHERE id = ?1", [id])?;
    crate::photos::collect_garbage(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Counts a showing of the card at a till; the list puts the most shown first.
pub fn mark_used(db: &Db, by: &User, id: i64, now: i64) -> Result<Card, CardError> {
    get(db, by, id)?;
    db.conn().execute(
        "UPDATE cards SET use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    get(db, by, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{Home, card_input};

    fn names(cards: &[Card]) -> Vec<&str> {
        cards.iter().map(|c| c.name.as_str()).collect()
    }

    #[test]
    fn a_saved_card_comes_back_as_it_was_given() {
        let home = Home::new();
        let card = create(
            &home.db,
            &home.marko,
            &CardInput {
                program: "  Supercard ".into(),
                display_number: "4006 3813 3393 1".into(),
                notes: "Ask for the receipt".into(),
                color: "#ABCDEF".into(),
                ..card_input(" Coop ")
            },
            50,
        )
        .unwrap();
        assert_eq!(
            card,
            Card {
                id: card.id,
                holder: None,
                private: false,
                name: "Coop".into(),
                program: "Supercard".into(),
                code: Code {
                    format: CodeFormat::Ean13,
                    value: "4006381333931".into()
                },
                display_number: "4006 3813 3393 1".into(),
                color: "#abcdef".into(),
                notes: "Ask for the receipt".into(),
                front: None,
                back: None,
                use_count: 0,
                last_used_at: None,
                created_at: 50,
                updated_at: 50,
            }
        );
    }

    #[test]
    fn the_household_sees_shared_cards_and_only_the_holder_a_private_one() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let family = home.card_for(None, false);
        let anas = home.card_for(Some(&ana), false);
        let anas_secret = home.card_for(Some(&ana), true);
        let seen = |who: &User| -> Vec<i64> {
            let mut ids: Vec<i64> = list(&home.db, who).unwrap().iter().map(|c| c.id).collect();
            ids.sort();
            ids
        };
        assert_eq!(seen(&ana), [family, anas, anas_secret], "what ana sees");
        assert_eq!(seen(&home.marko), [family, anas], "what marko sees");
        assert_eq!(
            get(&home.db, &home.marko, anas_secret),
            Err(CardError::NotFound)
        );
    }

    #[test]
    fn another_household_sees_none_of_the_cards() {
        let mut home = Home::new();
        let (_, nina) = home.neighbours();
        let card = home.card_for(None, false);
        assert!(list(&home.db, &nina).unwrap().is_empty());
        assert_eq!(get(&home.db, &nina, card), Err(CardError::NotFound));
        let input = card_input("Mine now");
        assert_eq!(
            update(&home.db, &nina, card, &input, 60),
            Err(CardError::NotFound)
        );
        assert_eq!(
            mark_used(&home.db, &nina, card, 60),
            Err(CardError::NotFound)
        );
        assert_eq!(delete(&mut home.db, &nina, card), Err(CardError::NotFound));
    }

    #[test]
    fn a_holder_from_another_household_is_refused() {
        let mut home = Home::new();
        let (_, nina) = home.neighbours();
        let input = CardInput {
            holder: Some(nina.id),
            ..card_input("Shop")
        };
        let err = create(&home.db, &home.marko, &input, 1).unwrap_err();
        assert_eq!(
            err,
            CardError::Invalid("the holder is not in this household".into())
        );
    }

    #[test]
    fn only_the_holder_can_make_a_card_private() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let input = CardInput {
            holder: Some(ana.id),
            private: true,
            ..card_input("Ana's")
        };
        let err = create(&home.db, &home.marko, &input, 1).unwrap_err();
        assert_eq!(
            err,
            CardError::Invalid("only its holder can make a card private".into())
        );
        assert!(create(&home.db, &ana, &input, 1).is_ok());
    }

    #[test]
    fn a_family_card_cannot_be_private() {
        let home = Home::new();
        let input = CardInput {
            private: true,
            ..card_input("Shop")
        };
        let err = create(&home.db, &home.marko, &input, 1).unwrap_err();
        assert_eq!(
            err,
            CardError::Invalid("a family card cannot be private".into())
        );
    }

    #[test]
    fn anyone_in_the_household_may_change_a_shared_card() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let id = home.card_for(None, false);
        let changed = update(&home.db, &ana, id, &card_input("Renamed"), 70).unwrap();
        assert_eq!((changed.name.as_str(), changed.updated_at), ("Renamed", 70));
    }

    #[test]
    fn the_most_used_card_comes_first() {
        let home = Home::new();
        for name in ["Apple", "Banana", "Cherry"] {
            create(&home.db, &home.marko, &card_input(name), 1).unwrap();
        }
        let cherry = list(&home.db, &home.marko).unwrap()[2].id;
        let used = mark_used(&home.db, &home.marko, cherry, 99).unwrap();
        assert_eq!((used.use_count, used.last_used_at), (1, Some(99)));
        assert_eq!(
            names(&list(&home.db, &home.marko).unwrap()),
            ["Cherry", "Apple", "Banana"]
        );
    }

    #[test]
    fn a_deleted_card_is_gone() {
        let mut home = Home::new();
        let id = home.card_for(None, false);
        let marko = home.marko.clone();
        delete(&mut home.db, &marko, id).unwrap();
        assert_eq!(get(&home.db, &marko, id), Err(CardError::NotFound));
    }

    #[test]
    fn what_a_card_may_hold_is_checked() {
        let table: &[(&str, CardInput, &str)] = &[
            (
                "no name",
                CardInput {
                    name: "  ".into(),
                    ..card_input("")
                },
                "Name: 1 to 60 characters",
            ),
            (
                "a long name",
                card_input(&"x".repeat(61)),
                "Name: 1 to 60 characters",
            ),
            (
                "a long programme",
                CardInput {
                    program: "p".repeat(61),
                    ..card_input("Shop")
                },
                "Programme: at most 60 characters",
            ),
            (
                "no code",
                CardInput {
                    code_value: " ".into(),
                    ..card_input("Shop")
                },
                "Code: 1 to 2048 characters",
            ),
            (
                "a colour name",
                CardInput {
                    color: "green".into(),
                    ..card_input("Shop")
                },
                "the colour is #rrggbb",
            ),
            (
                "a short colour",
                CardInput {
                    color: "#fff".into(),
                    ..card_input("Shop")
                },
                "the colour is #rrggbb",
            ),
            (
                "long notes",
                CardInput {
                    notes: "n".repeat(2001),
                    ..card_input("Shop")
                },
                "Notes: at most 2000 characters",
            ),
            (
                "a tab in the name",
                card_input("Sh\top"),
                "the name has a control character in it",
            ),
            (
                "a line break in a 1D code",
                CardInput {
                    code_format: CodeFormat::Code128,
                    code_value: "a\nb".into(),
                    ..card_input("Shop")
                },
                "the code has a control character in it",
            ),
            (
                "a wrong check digit",
                CardInput {
                    code_value: "4006381333932".into(),
                    ..card_input("Shop")
                },
                "the last digit should be 1: the number has a typing mistake",
            ),
        ];
        let home = Home::new();
        for (what, input, expected) in table {
            assert_eq!(
                create(&home.db, &home.marko, input, 1),
                Err(CardError::Invalid(expected.to_string())),
                "{what}"
            );
        }
    }

    #[test]
    fn a_qr_code_may_hold_line_breaks() {
        let home = Home::new();
        let input = CardInput {
            code_format: CodeFormat::Qr,
            code_value: "BEGIN:VCARD\nFN:Ana\nEND:VCARD".into(),
            ..card_input("Contact")
        };
        assert!(create(&home.db, &home.marko, &input, 1).is_ok());
    }

    #[test]
    fn each_symbology_takes_what_its_scanners_read() {
        use CodeFormat::*;
        let table: &[(CodeFormat, &str, bool)] = &[
            (Ean13, "4006381333931", true),
            (Ean13, "400638133393", true),
            (Ean13, "4006381333930", false),
            (Ean13, "40063813339", false),
            (Ean13, "40063813339a", false),
            (Ean8, "96385074", true),
            (Ean8, "9638507", true),
            (Ean8, "96385075", false),
            (Upca, "036000291452", true),
            (Upca, "03600029145", true),
            (Upca, "036000291453", false),
            (Upce, "0425261", true),
            (Upce, "04252", false),
            (Itf, "1234", true),
            (Itf, "123", false),
            (Itf, "12a4", false),
            (Code39, "ABC-123 $/+%.", true),
            (Code39, "abc", false),
            (Codabar, "A40156B", true),
            (Codabar, "40156", true),
            (Codabar, "AB", false),
            (Codabar, "4x0", false),
            (Code128, "Member #42-x", true),
            (Code128, "Café", false),
            (Code93, "CODE 93", true),
            (Code93, "€", false),
            (Qr, "anything at all ✓", true),
            (Pdf417, "M1DOE/JOHN", true),
            (Aztec, "x", true),
            (Datamatrix, "x", true),
        ];
        for (format, value, ok) in table {
            assert_eq!(
                format.check(value).is_ok(),
                *ok,
                "{format:?} {value:?}: {:?}",
                format.check(value)
            );
        }
    }

    #[test]
    fn symbology_names_round_trip() {
        for format in CodeFormat::ALL {
            assert_eq!(CodeFormat::parse(format.as_str()), Some(format));
            let json = serde_json::to_string(&format).unwrap();
            assert_eq!(json, format!("\"{}\"", format.as_str()));
        }
        assert_eq!(CodeFormat::parse("ean-13"), None);
    }

    #[test]
    fn a_holders_name_comes_with_the_card() {
        let mut home = Home::new();
        let ana = home.person("ana");
        let id = home.card_for(Some(&ana), false);
        let card = get(&home.db, &home.marko, id).unwrap();
        assert_eq!(
            card.holder,
            Some(Holder {
                id: ana.id,
                display_name: "Ana".into()
            })
        );
    }

    #[test]
    fn errors_read_as_sentences() {
        assert_eq!(CardError::NotFound.to_string(), "no such card");
        assert_eq!(CardError::Invalid("x".into()).to_string(), "x");
    }
}
