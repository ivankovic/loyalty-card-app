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

//! Fixtures for the tests in `src/` and `tests/`: a household with people and cards in a real
//! database in memory. Not part of the program.

use crate::accounts::{self, Household, User};
use crate::cards::{self, CardInput, CodeFormat};
use crate::db::Db;

/// A household called "Home" whose administrator, marko, has signed in once.
pub struct Home {
    pub db: Db,
    pub household: Household,
    pub marko: User,
}

impl Home {
    pub const PASSWORD: &'static str = "marko's password";

    pub fn new() -> Home {
        let mut db = Db::open_in_memory().unwrap();
        let household = accounts::add_household(&db, "Home", 1).unwrap();
        let marko = arrive(&mut db, &household, "marko", "Marko", true);
        Home {
            db,
            household,
            marko,
        }
    }

    /// Someone else in this household, with the password `Home::PASSWORD`.
    pub fn person(&mut self, name: &str) -> User {
        let display = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
        arrive(&mut self.db, &self.household, name, &display, false)
    }

    /// Another household, with one person in it.
    pub fn neighbours(&mut self) -> (Household, User) {
        let household = accounts::add_household(&self.db, "Neighbours", 1).unwrap();
        let person = arrive(&mut self.db, &household, "nina", "Nina", true);
        (household, person)
    }

    /// A card held by `holder` (the family's if `None`), made by the holder or by marko.
    pub fn card_for(&self, holder: Option<&User>, private: bool) -> i64 {
        let by = holder.unwrap_or(&self.marko);
        cards::create(
            &self.db,
            by,
            &CardInput {
                holder: holder.map(|h| h.id),
                private,
                ..card_input("Shop")
            },
            10,
        )
        .unwrap()
        .id
    }
}

impl Default for Home {
    fn default() -> Self {
        Home::new()
    }
}

/// A valid card called `name`, for the family.
pub fn card_input(name: &str) -> CardInput {
    CardInput {
        holder: None,
        private: false,
        name: name.into(),
        program: String::new(),
        code_value: "4006381333931".into(),
        code_format: CodeFormat::Ean13,
        display_number: String::new(),
        color: "#1c7455".into(),
        notes: String::new(),
    }
}

fn arrive(db: &mut Db, household: &Household, name: &str, display: &str, admin: bool) -> User {
    let token = accounts::invite(db, household, name, admin, 1).unwrap();
    accounts::accept_invite(db, &token, Home::PASSWORD, Some(display), 2).unwrap()
}
