//! A2S (Source query protocol) against DayZ servers: ping, info, players
//! and rules.
//!
//! Many servers at once (a scan, a listing) go through one [`Engine`]:
//! a shared socket, a paced rate, replies routed by address. A one-off
//! query takes the query address as `"ip:port"` and a socket of its own.
//! A2S_RULES and A2S_PLAYER are read here ([`rules`], [`players`]) rather
//! than by `async-a2s`, which drops the mods' Workshop ids and fails on the
//! players of busy servers; its A2S_INFO reader is kept.

mod engine;
mod exchange;
pub mod keywords;
pub mod players;
pub mod rules;

pub use engine::{Engine, Pace};
pub use keywords::DayzInfo;
pub use players::{A2sPlayer, query_players};
pub use rules::{A2sMod, DayzRules, query_dayz_rules};

use async_a2s::info::Info;
use dz_common::{Error, Result};
use exchange::info_within;

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

/// Sentinel RTT reported when a query fails. The window shows any value
/// >= 5 000 ms as "TIMEOUT"; 9 999 is clearly not a real RTT.
pub const PING_TIMEOUT_SENTINEL: u32 = 9_999;

/// What one A2S_INFO round trip tells a ping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ping {
    pub ms: u32,
    /// Human players (bots subtracted, see [`human_player_count`]).
    pub players: u8,
    pub max_players: u8,
    pub bots: u8,
}

/// Convert a Duration to milliseconds, at least 1 ms for a non-zero duration
/// so a sub-millisecond ping never shows as 0.
#[inline]
fn duration_to_ms_floor1(d: Duration) -> u32 {
    let ms = d.as_millis() as u32;
    if ms == 0 && d.as_micros() > 0 { 1 } else { ms }
}

/// The address to query for `ip` and `port`, when they make one.
pub fn socket_addr(ip: &str, port: i64) -> Option<SocketAddr> {
    let ip: IpAddr = ip.parse().ok()?;
    Some(SocketAddr::new(ip, u16::try_from(port).ok()?))
}

/// What A2S_INFO answered.
pub struct InfoAnswer {
    pub info: Info,
    /// The first round trip, from the request leaving to the first reply:
    /// the challenge's, for a server that sends one.
    pub latency: Duration,
    /// The challenge it was asked with, good for A2S_RULES for a while
    /// (measured on 38 DayZ servers: 37 of 37 answered the rules at once,
    /// still good 10 s later, gone after 150 s).
    challenge: Option<[u8; 4]>,
}

impl InfoAnswer {
    /// The answer as a ping.
    pub fn ping(&self) -> Ping {
        ping_of(&self.info, self.latency)
    }

    /// The challenge to send A2S_RULES with ([`Engine::rules`]); `None`
    /// when the server asked for none.
    pub fn challenge(&self) -> Option<[u8; 4]> {
        self.challenge
    }
}

/// Real (human) player count for an A2S_INFO response.
///
/// Per the Source query spec `players` counts bots too, and many DayZ servers
/// inflate their population by reporting `bots == players` (a "full" server
/// with nobody on it). Legitimate servers report `bots == 0`, so this is a
/// no-op for them.
#[inline]
pub fn human_player_count(info: &Info) -> u8 {
    info.players.saturating_sub(info.bots)
}

fn ping_of(info: &Info, latency: Duration) -> Ping {
    Ping {
        ms: duration_to_ms_floor1(latency),
        players: human_player_count(info),
        max_players: info.max_players,
        bots: info.bots,
    }
}

/// A one-off query's tries (the detail panel, a manual ping): two of half
/// the usual wait, so one lost UDP packet does not read as a silent server,
/// and still no longer than 5 s in all, the challenge included. A scan keeps
/// its own retries.
const ONE_OFF_TRIES: u32 = 2;
const ONE_OFF_TIMEOUT: Duration = Duration::from_millis(2500);

async fn one_off_info(addr: &str) -> Result<InfoAnswer> {
    let mut last = None;
    for _ in 0..ONE_OFF_TRIES {
        match info_within(addr, ONE_OFF_TIMEOUT).await {
            Ok(a) => return Ok(a),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| Error::A2sQuery(format!("A2S query to {addr} was never sent"))))
}

/// Ping on a socket of its own.
pub async fn ping(addr: &str) -> Result<Ping> {
    Ok(one_off_info(addr).await?.ping())
}

/// A2S_INFO.
pub async fn query_info(addr: &str) -> Result<Info> {
    Ok(one_off_info(addr).await?.info)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live A2S query against a public server.
    /// Run with: cargo test -p dz-a2s -- --ignored --nocapture
    #[tokio::test]
    #[ignore = "queries a public server"]
    async fn live_server() {
        let addr = "195.60.166.46:27016";
        match query_info(addr).await {
            Ok(info) => println!(
                "INFO ok: name={:?} map={:?} players={}/{}",
                info.name, info.map, info.players, info.max_players
            ),
            Err(e) => println!("INFO error: {e}"),
        }
        match ping(addr).await {
            Ok(p) => println!("PING ok: {p:?}"),
            Err(e) => println!("PING error: {e}"),
        }
    }

    #[test]
    fn addresses_of_either_family() {
        assert_eq!(
            socket_addr("1.2.3.4", 27016),
            Some("1.2.3.4:27016".parse().unwrap())
        );
        assert_eq!(
            socket_addr("::1", 2303),
            Some("[::1]:2303".parse().unwrap())
        );
        assert_eq!(socket_addr("1.2.3.4", 70_000), None);
        assert_eq!(socket_addr("nope", 1), None);
    }

    #[test]
    fn floor_to_one_ms() {
        assert_eq!(duration_to_ms_floor1(Duration::from_micros(300)), 1);
        assert_eq!(duration_to_ms_floor1(Duration::ZERO), 0);
        assert_eq!(duration_to_ms_floor1(Duration::from_millis(42)), 42);
    }
}
