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

//! The command line: `serve` for the container, `healthz` for its health check, and the
//! administrator's commands, run as `docker exec loyalty /loyalty <command>`.

use crate::accounts;
use crate::db::{Db, now};
use crate::web::{self, AppState, ServeLimits};
use anyhow::Context as _;
use clap::{Parser, Subcommand};
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;

/// A household's loyalty cards, on its own server.
#[derive(Parser, Debug)]
#[command(name = "loyalty", version)]
pub struct Cli {
    /// The database file. Made on first use.
    #[arg(
        long,
        env = "LOYALTY_DB",
        hide_env_values = true,
        default_value = "/data/loyalty.db",
        global = true
    )]
    pub db: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Serve the app and its API.
    Serve {
        /// The address to listen on. Port 0 picks a free one and prints it.
        #[arg(
            long,
            env = "LOYALTY_LISTEN",
            hide_env_values = true,
            default_value = "0.0.0.0:8080"
        )]
        listen: SocketAddr,
        /// Where people open the app, as their browser shows it: https://cards.example.org.
        #[arg(long, env = "LOYALTY_ORIGIN", hide_env_values = true)]
        origin: String,
        /// Believe the X-Real-IP header: only behind a proxy (nginx) that sets it.
        #[arg(long, env = "LOYALTY_TRUST_PROXY", hide_env_values = true)]
        trust_proxy: bool,
    },
    /// Check that the server answers: the container's health check.
    Healthz {
        #[arg(long, default_value = "127.0.0.1:8080")]
        address: SocketAddr,
    },
    /// Households: the groups of people who share cards.
    #[command(subcommand)]
    Household(HouseholdCommand),
    /// Print a one-time link that lets someone join a household and choose their password.
    Invite {
        /// The household to join.
        #[arg(long)]
        household: String,
        /// The name they will sign in with.
        name: String,
        /// Let them invite others too.
        #[arg(long)]
        admin: bool,
        /// Where people open the app, for the link: https://cards.example.org.
        #[arg(long, env = "LOYALTY_ORIGIN", hide_env_values = true)]
        origin: String,
    },
    /// People.
    #[command(subcommand)]
    User(UserCommand),
    /// Write a consistent copy of the database to a new file, while the server runs.
    Backup {
        /// The copy to write. It must not exist yet.
        target: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum HouseholdCommand {
    /// Add a household.
    Add { name: String },
    /// List the households.
    List,
}

#[derive(Subcommand, Debug)]
pub enum UserCommand {
    /// List everyone, with their household.
    List,
    /// Print a one-time link that lets someone choose a new password. Their phones stay signed
    /// in until they use it.
    ResetPassword {
        name: String,
        #[arg(long, env = "LOYALTY_ORIGIN", hide_env_values = true)]
        origin: String,
    },
    /// Remove someone. Their private cards are deleted; their other cards become the family's.
    Remove { name: String },
}

/// Runs a command, printing its result to `out`.
pub fn run(cli: Cli, out: &mut impl Write) -> anyhow::Result<()> {
    match cli.command {
        Command::Serve {
            listen,
            origin,
            trust_proxy,
        } => {
            let db = Db::open(&cli.db)?;
            let state = AppState::new(db, &origin, trust_proxy)?;
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(web::serve(
                web::router(state),
                listen,
                ServeLimits::default(),
            ))
        }
        Command::Healthz { address } => healthz(address, out),
        Command::Household(HouseholdCommand::Add { name }) => {
            let db = Db::open(&cli.db)?;
            let household = accounts::add_household(&db, &name, now())?;
            writeln!(out, "Added the household {:?}.", household.name)?;
            Ok(())
        }
        Command::Household(HouseholdCommand::List) => {
            let db = Db::open(&cli.db)?;
            for household in accounts::households(&db)? {
                writeln!(out, "{}", household.name)?;
            }
            Ok(())
        }
        Command::Invite {
            household,
            name,
            admin,
            origin,
        } => {
            let db = Db::open(&cli.db)?;
            let found = accounts::household_by_name(&db, &household)?.with_context(|| {
                format!("there is no household {household:?}; add it with `loyalty household add`")
            })?;
            let token = accounts::invite(&db, &found, &name, admin, now())?;
            write_link(
                out,
                &origin,
                &token,
                &format!("{name} to join {}", found.name),
            )
        }
        Command::User(UserCommand::List) => {
            let db = Db::open(&cli.db)?;
            for person in accounts::users(&db)? {
                let mut notes = Vec::new();
                if person.is_admin {
                    notes.push("admin");
                }
                if !person.arrived {
                    notes.push("has not set a password");
                }
                let notes = if notes.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", notes.join(", "))
                };
                writeln!(
                    out,
                    "{}: {}, {}{notes}",
                    person.household, person.name, person.display_name
                )?;
            }
            Ok(())
        }
        Command::User(UserCommand::ResetPassword { name, origin }) => {
            let db = Db::open(&cli.db)?;
            let token = accounts::reset_invite(&db, &name, now())
                .map_err(|_| anyhow::anyhow!("there is nobody called {name:?}"))?;
            write_link(
                out,
                &origin,
                &token,
                &format!("{name} to choose a new password"),
            )
        }
        Command::User(UserCommand::Remove { name }) => {
            let mut db = Db::open(&cli.db)?;
            accounts::remove_user(&mut db, &name)
                .map_err(|_| anyhow::anyhow!("there is nobody called {name:?}"))?;
            writeln!(out, "Removed {name}.")?;
            Ok(())
        }
        Command::Backup { target } => {
            let db = Db::open(&cli.db)?;
            db.backup_to(&target)?;
            writeln!(out, "Wrote {}.", target.display())?;
            Ok(())
        }
    }
}

fn write_link(
    out: &mut impl Write,
    origin: &str,
    token: &str,
    for_whom: &str,
) -> anyhow::Result<()> {
    let origin = origin.trim_end_matches('/');
    writeln!(
        out,
        "Send this link to {for_whom}. It works once, for {} days:\n\n    {origin}/i/{token}\n",
        accounts::INVITE_TTL_SECS / 86400
    )?;
    Ok(())
}

/// `GET /healthz` over a plain socket: the container image has no curl.
fn healthz(address: SocketAddr, out: &mut impl Write) -> anyhow::Result<()> {
    use std::io::Read as _;
    let timeout = std::time::Duration::from_secs(5);
    let mut stream = std::net::TcpStream::connect_timeout(&address, timeout)
        .with_context(|| format!("nothing answers on {address}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut answer = String::new();
    stream.read_to_string(&mut answer)?;
    anyhow::ensure!(
        answer.starts_with("HTTP/1.1 200") && answer.ends_with("ok\n"),
        "the server on {address} is not well: {}",
        answer.lines().next().unwrap_or("no answer")
    );
    writeln!(out, "ok")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(db: &std::path::Path, args: &[&str]) -> anyhow::Result<String> {
        let mut argv = vec!["loyalty", "--db", db.to_str().unwrap()];
        argv.extend_from_slice(args);
        let mut out = Vec::new();
        run(Cli::try_parse_from(argv)?, &mut out)?;
        Ok(String::from_utf8(out).unwrap())
    }

    #[test]
    fn an_administrator_sets_up_a_household() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("loyalty.db");
        assert_eq!(
            run_args(&db, &["household", "add", "Home"]).unwrap(),
            "Added the household \"Home\".\n"
        );
        assert_eq!(run_args(&db, &["household", "list"]).unwrap(), "Home\n");
        let link = run_args(
            &db,
            &[
                "invite",
                "--household",
                "Home",
                "marko",
                "--admin",
                "--origin",
                "https://cards.example/",
            ],
        )
        .unwrap();
        assert!(
            link.starts_with("Send this link to marko to join Home. It works once, for 7 days:\n\n    https://cards.example/i/"),
            "the output was:\n{link}"
        );
        assert_eq!(
            run_args(&db, &["user", "list"]).unwrap(),
            "",
            "someone who has not used their invite is not a user yet"
        );
    }

    #[test]
    fn an_invite_to_a_missing_household_says_how_to_add_it() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("loyalty.db");
        let err = run_args(
            &db,
            &[
                "invite",
                "--household",
                "Home",
                "ana",
                "--origin",
                "https://x",
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("loyalty household add"),
            "the error was: {err}"
        );
    }

    #[test]
    fn people_are_listed_reset_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loyalty.db");
        {
            let mut db = Db::open(&path).unwrap();
            let home = accounts::add_household(&db, "Home", 1).unwrap();
            let token = accounts::invite(&db, &home, "ana", true, 1).unwrap();
            accounts::accept_invite(&mut db, &token, "ana's password", Some("Ana"), 2).unwrap();
        }
        assert_eq!(
            run_args(&path, &["user", "list"]).unwrap(),
            "Home: ana, Ana (admin)\n"
        );
        let link = run_args(
            &path,
            &["user", "reset-password", "ana", "--origin", "https://x"],
        )
        .unwrap();
        assert!(
            link.contains("ana to choose a new password"),
            "the output was:\n{link}"
        );
        assert_eq!(
            run_args(&path, &["user", "remove", "ana"]).unwrap(),
            "Removed ana.\n"
        );
        for args in [
            &["user", "remove", "ana"][..],
            &["user", "reset-password", "ana", "--origin", "https://x"],
        ] {
            let err = run_args(&path, args).unwrap_err().to_string();
            assert_eq!(err, "there is nobody called \"ana\"");
        }
    }

    #[test]
    fn a_backup_is_written_once() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("loyalty.db");
        let copy = dir.path().join("copy.db");
        let copy_arg = copy.to_str().unwrap();
        assert_eq!(
            run_args(&db, &["backup", copy_arg]).unwrap(),
            format!("Wrote {copy_arg}.\n")
        );
        assert!(run_args(&db, &["backup", copy_arg]).is_err());
    }

    #[test]
    fn healthz_fails_when_nothing_answers() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let err = healthz(address, &mut Vec::new()).unwrap_err().to_string();
        assert!(err.contains("nothing answers"), "the error was: {err}");
    }

    #[test]
    fn healthz_fails_on_an_unwell_answer() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            use std::io::Read as _;
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 256];
            let _ = stream.read(&mut buf).unwrap();
            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\n\r\n")
                .unwrap();
        });
        let err = healthz(address, &mut Vec::new()).unwrap_err().to_string();
        server.join().unwrap();
        assert!(
            err.contains("500 Internal Server Error"),
            "the error was: {err}"
        );
    }

    #[test]
    fn the_help_is_pinned() {
        use clap::CommandFactory as _;
        let mut help = String::new();
        let mut command = Cli::command();
        help.push_str(&command.render_long_help().to_string());
        for sub in ["serve", "invite", "user", "backup"] {
            let sub = command.find_subcommand_mut(sub).unwrap();
            help.push_str("\n---\n");
            help.push_str(&sub.render_long_help().to_string());
        }
        crate::golden::assert_golden("help.txt", &help);
    }
}
