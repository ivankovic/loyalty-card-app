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

//! Loyalty Card App: a household's loyalty cards, kept on its own server.
//!
//! The binary (`main.rs`) is the command line; everything it does is here, so that the
//! integration tests in `tests/` reach the same code. This is not a library for anyone else: its
//! modules are public for those tests, and change without notice.

pub mod accounts;
pub mod assets;
pub mod cards;
pub mod cli;
pub mod db;
pub mod photos;
pub mod throttle;
pub mod web;

#[doc(hidden)]
pub mod golden;
#[doc(hidden)]
pub mod testing;
