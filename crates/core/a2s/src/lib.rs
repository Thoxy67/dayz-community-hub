//! A2S (Source query protocol) against DayZ servers: ping, info, players
//! and rules.
//!
//! Every function takes the query address as `"ip:port"`.

use async_a2s::{A2SClient, info::Info, players::Player, rules::Rule};
use dz_common::{Error, Result};

use std::time::Duration;

pub use async_a2s::A2SClient as Client;

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

/// A fresh A2S client with the standard 5 s timeout.
///
/// Create one per scan and share it: `async-a2s` multiplexes responses on
/// its single UDP socket, so concurrent queries on one client are safe and
/// save a socket bind each.
pub async fn new_client() -> Result<A2SClient> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {e}")))?;
    client
        .set_timeout(Duration::from_secs(5))
        .map_err(|e| Error::A2sQuery(format!("Failed to set timeout: {e}")))?;
    Ok(client)
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

/// Ping through a caller-owned client (the bulk-scan hot path).
pub async fn ping_using(client: &A2SClient, addr: &str) -> Result<Ping> {
    let (info, latency) = client
        .info(addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {e}")))?;
    Ok(Ping {
        ms: duration_to_ms_floor1(latency),
        players: human_player_count(&info),
        max_players: info.max_players,
        bots: info.bots,
    })
}

/// Ping with a client of its own.
pub async fn ping(addr: &str) -> Result<Ping> {
    ping_using(&new_client().await?, addr).await
}

/// A2S_INFO.
pub async fn query_info(addr: &str) -> Result<Info> {
    let client = new_client().await?;
    let (info, _latency) = client
        .info(addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {e}")))?;
    Ok(info)
}

/// A2S_PLAYER.
pub async fn query_players(addr: &str) -> Result<Vec<Player>> {
    let client = new_client().await?;
    let (players, _latency) = client
        .players(addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S players query failed: {e}")))?;
    Ok(players)
}

/// A2S_RULES (cvars; DayZ also lists its mods here).
pub async fn query_rules(addr: &str) -> Result<Vec<Rule>> {
    let client = new_client().await?;
    let (rules, _latency) = client
        .rules(addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S rules query failed: {e}")))?;
    Ok(rules)
}

/// Mod names from an A2S rules response: DayZ servers return Protocol3
/// decoded entries named "mod" whose value is the mod's name.
pub fn extract_mods_from_rules(rules: &[Rule]) -> Vec<String> {
    rules
        .iter()
        .filter(|r| r.name == "mod")
        .map(|r| r.value.clone())
        .collect()
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
    fn floor_to_one_ms() {
        assert_eq!(duration_to_ms_floor1(Duration::from_micros(300)), 1);
        assert_eq!(duration_to_ms_floor1(Duration::ZERO), 0);
        assert_eq!(duration_to_ms_floor1(Duration::from_millis(42)), 42);
    }
}
