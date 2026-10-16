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

//! Slows down password guessing.
//!
//! Each sign-in is counted twice: against the name tried and against the address it came from
//! (an IPv6 address by its /64, which one client usually holds all of). After
//! [`FREE_FAILURES`] failures in a row, the next attempt for that key waits, twice as long
//! after each further failure, up to [`MAX_WAIT_SECS`]. A success clears both keys.
//!
//! Counted in memory: a restart forgets, which only matters to someone who can restart the
//! server, and keeps a stream of wrong passwords from turning into a stream of writes.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

pub const FREE_FAILURES: u32 = 5;
pub const MAX_WAIT_SECS: i64 = 15 * 60;
const FIRST_WAIT_SECS: i64 = 30;

#[derive(Default)]
pub struct Throttle {
    failures: Mutex<HashMap<String, (u32, i64)>>,
}

impl Throttle {
    /// The keys an attempt is counted under.
    pub fn keys(name: &str, ip: Option<IpAddr>) -> Vec<String> {
        let mut keys = vec![format!("name:{}", name.to_lowercase())];
        if let Some(ip) = ip {
            keys.push(format!("ip:{}", client_key(ip)));
        }
        keys
    }

    /// How many seconds the caller must still wait before another attempt, if any.
    pub fn wait(&self, keys: &[String], now: i64) -> Option<i64> {
        let failures = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        keys.iter()
            .filter_map(|k| failures.get(k))
            .map(|&(_, until)| until - now)
            .filter(|&left| left > 0)
            .max()
    }

    pub fn failed(&self, keys: &[String], now: i64) {
        let mut failures = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        for key in keys {
            let entry = failures.entry(key.clone()).or_insert((0, 0));
            entry.0 += 1;
            if entry.0 >= FREE_FAILURES {
                let doublings = (entry.0 - FREE_FAILURES).min(10);
                entry.1 = now + (FIRST_WAIT_SECS << doublings).min(MAX_WAIT_SECS);
            }
        }
    }

    pub fn succeeded(&self, keys: &[String]) {
        let mut failures = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        for key in keys {
            failures.remove(key);
        }
    }
}

/// An address as one client: IPv4 as it is, IPv6 by its /64.
pub fn client_key(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<String> {
        Throttle::keys("Marko", Some("192.0.2.7".parse().unwrap()))
    }

    #[test]
    fn the_first_failures_cost_nothing() {
        let throttle = Throttle::default();
        for _ in 0..FREE_FAILURES - 1 {
            throttle.failed(&keys(), 100);
        }
        assert_eq!(throttle.wait(&keys(), 100), None);
    }

    #[test]
    fn then_each_failure_doubles_the_wait_up_to_a_limit() {
        let throttle = Throttle::default();
        let mut waits = Vec::new();
        for _ in 0..FREE_FAILURES + 7 {
            throttle.failed(&keys(), 100);
            waits.push(throttle.wait(&keys(), 100));
        }
        assert_eq!(
            waits[FREE_FAILURES as usize - 1..],
            [
                Some(30),
                Some(60),
                Some(120),
                Some(240),
                Some(480),
                Some(900),
                Some(900),
                Some(900)
            ]
        );
        assert_eq!(
            throttle.wait(&keys(), 100 + MAX_WAIT_SECS),
            None,
            "the wait never ended"
        );
    }

    #[test]
    fn a_success_clears_the_count() {
        let throttle = Throttle::default();
        for _ in 0..FREE_FAILURES {
            throttle.failed(&keys(), 100);
        }
        throttle.succeeded(&keys());
        assert_eq!(throttle.wait(&keys(), 100), None);
    }

    #[test]
    fn a_name_is_throttled_from_any_address() {
        let throttle = Throttle::default();
        for _ in 0..FREE_FAILURES {
            throttle.failed(&keys(), 100);
        }
        let elsewhere = Throttle::keys("marko", Some("198.51.100.1".parse().unwrap()));
        assert_eq!(throttle.wait(&elsewhere, 100), Some(30));
        let other_name = Throttle::keys("ana", Some("198.51.100.1".parse().unwrap()));
        assert_eq!(throttle.wait(&other_name, 100), None);
    }

    #[test]
    fn an_ipv6_client_is_its_64() {
        let a: IpAddr = "2001:db8:1:2:aaaa::1".parse().unwrap();
        let b: IpAddr = "2001:db8:1:2:bbbb::2".parse().unwrap();
        let c: IpAddr = "2001:db8:1:3::1".parse().unwrap();
        assert_eq!(client_key(a), client_key(b));
        assert_ne!(client_key(a), client_key(c));
        assert_eq!(client_key(a), "2001:db8:1:2::/64");
    }
}
