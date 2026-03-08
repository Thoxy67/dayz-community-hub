use crate::api::Server;
use crate::{Result, errors::Error};
use async_a2s::{A2SClient, info::Info, players::Player, rules::Rule};

use std::time::Duration;

/// Convert a Duration to milliseconds, ensuring at least 1ms for non-zero durations.
/// This prevents sub-millisecond pings from showing as 0ms.
#[inline]
fn duration_to_ms_floor1(d: Duration) -> u32 {
    let ms = d.as_millis() as u32;
    if ms == 0 && d.as_micros() > 0 { 1 } else { ms }
}

/// Create a configured A2S client and format the server's query address.
async fn make_a2s_client(server: &Server) -> Result<(A2SClient, String)> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;
    // set_timeout now returns Result in async-a2s
    client
        .set_timeout(Duration::from_secs(5))
        .map_err(|e| Error::A2sQuery(format!("Failed to set timeout: {}", e)))?;
    let addr = format!("{}:{}", server.endpoint.ip, server.endpoint.port as u16);
    Ok((client, addr))
}

/// Query server information using A2S protocol
pub async fn query_server_info(server: &Server) -> Result<Info> {
    let (client, addr) = make_a2s_client(server).await?;
    let (info, _latency) = client
        .info(&addr, None) // None = use default timeout
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {}", e)))?;
    Ok(info)
}

/// Query player information using A2S protocol
pub async fn query_player_info(server: &Server) -> Result<Vec<Player>> {
    let (client, addr) = make_a2s_client(server).await?;
    let (players, _latency) = client
        .players(&addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S players query failed: {}", e)))?;
    Ok(players)
}

/// Query rules (cvars) using A2S protocol
pub async fn query_rules(server: &Server) -> Result<Vec<Rule>> {
    let (client, addr) = make_a2s_client(server).await?;
    let (rules, _latency) = client
        .rules(&addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S rules query failed: {}", e)))?;
    Ok(rules)
}

/// Measure RTT via a single A2S info query.
/// Returns the round-trip time in milliseconds.
pub async fn ping_via_a2s(server: &Server) -> Result<u32> {
    let (client, addr) = make_a2s_client(server).await?;
    let (_info, latency) = client
        .info(&addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {}", e)))?;
    Ok(duration_to_ms_floor1(latency))
}

/// Measure RTT via A2S info query and return player count.
/// Returns (rtt_ms, players, max_players).
pub async fn ping_via_a2s_with_info(server: &Server) -> Result<(u32, u8, u8)> {
    let (client, addr) = make_a2s_client(server).await?;
    let (info, latency) = client
        .info(&addr, None)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {}", e)))?;

    let ms = duration_to_ms_floor1(latency);
    Ok((ms, info.players, info.max_players))
}

/// Sentinel value emitted when all ping attempts fail — treated as TIMEOUT on
/// the frontend.  Any value >= 5 000 ms is displayed as "TIMEOUT"; we use 9 999
/// so it is clearly a sentinel rather than a real RTT.
pub const PING_TIMEOUT_SENTINEL: u32 = 9_999;

/// Get comprehensive server information including players and rules.
///
/// All three A2S queries (info, players, rules) are issued concurrently via
/// `tokio::join!` — wall-clock time is ~1 RTT instead of ~3 RTTs.
/// The ping is measured using the built-in latency from the info query,
/// which is the most meaningful latency indicator for the user.
pub async fn get_server_details(server: &Server) -> Result<ServerDetails> {
    // Create client once, reuse for all three queries
    let (client, addr) = make_a2s_client(server).await?;

    // All three queries run concurrently - async-a2s handles socket multiplexing
    let (info_result, players_result, rules_result) = tokio::join!(
        client.info(&addr, None),
        client.players(&addr, None),
        client.rules(&addr, None),
    );

    // Use info query latency as the ping
    let (info, latency) =
        info_result.map_err(|e| Error::A2sQuery(format!("A2S info failed: {}", e)))?;
    let ping_ms = duration_to_ms_floor1(latency);

    // Players and rules are optional (may fail on some servers)
    let players = players_result.ok().map(|(p, _)| p);
    let rules_opt = rules_result.ok().map(|(r, _)| r);

    // Extract mods from rules if available
    let mods_from_rules = rules_opt
        .as_ref()
        .map(|r| extract_mods_from_rules(r))
        .unwrap_or_default();

    Ok(ServerDetails {
        info,
        players,
        rules: rules_opt,
        ping_ms: Some(ping_ms),
        mods_from_rules,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_a2s::A2SClient;
    use tokio::try_join;

    /// Direct A2S test against a live server — mirrors the a2s lib's own async test.
    /// Run with: cargo test --package dayz-community-hub-core -- a2s_query::tests::test_live_server --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_server() {
        let addr = "195.60.166.46:27016";
        println!("=== Direct A2SClient test against {} ===", addr);

        let client = A2SClient::new().await.expect("A2SClient::new failed");

        // info only first — cheapest query
        match client.info(addr, None).await {
            Ok((info, _latency)) => println!(
                "INFO ok: name={:?} map={:?} players={}/{} version={:?}",
                info.name, info.map, info.players, info.max_players, info.version
            ),
            Err(e) => println!("INFO error: {:?}", e),
        }

        // new client for players (socket is stateful)
        let client2 = A2SClient::new().await.expect("A2SClient::new failed");
        match client2.players(addr, None).await {
            Ok((players, _latency)) => println!("PLAYERS ok: {} players", players.len()),
            Err(e) => println!("PLAYERS error: {:?}", e),
        }

        println!("=== Via get_server_details wrapper ===");
        let server = crate::api::Server {
            endpoint: crate::api::Endpoint {
                ip: "195.60.166.46".to_string(),
                port: 27016,
            },
            ..Default::default()
        };
        match get_server_details(&server).await {
            Ok(details) => {
                println!("get_server_details ok:");
                println!("  name    = {:?}", details.info.name);
                println!("  map     = {:?}", details.info.map);
                println!(
                    "  players = {}/{}",
                    details.info.players, details.info.max_players
                );
                println!("  ping    = {:?}ms", details.ping_ms);
                if let Some(ref pl) = details.players {
                    println!("  online  = {} players", pl.len());
                    for p in pl.iter().take(5) {
                        println!("    {:?}", p.name);
                    }
                } else {
                    println!("  players query failed (ok for DayZ)");
                }
            }
            Err(e) => println!("get_server_details error: {:?}", e),
        }
    }

    /// Test the same query port but via try_join (exactly like the a2s lib's own test)
    #[tokio::test]
    #[ignore]
    async fn test_live_try_join() {
        let addr = "195.60.166.46:27016";
        println!("=== try_join test against {} ===", addr);

        let client = A2SClient::new().await.expect("A2SClient::new failed");
        let info_fut = client.info(addr, None);
        let players_fut = client.players(addr, None);

        match try_join!(info_fut, players_fut) {
            Ok(((info, _), (players, _))) => {
                println!("try_join ok");
                println!(
                    "  name={:?} players={}/{}",
                    info.name, info.players, info.max_players
                );
                println!("  {} online", players.len());
            }
            Err(e) => println!("try_join error: {:?}", e),
        }
    }
}

/// Extract mod list from A2S rules response.
/// DayZ servers return Protocol3-decoded mod entries with name="mod" and value=mod name.
/// Returns a list of mod names extracted from the rules.
pub fn extract_mods_from_rules(rules: &[Rule]) -> Vec<String> {
    rules
        .iter()
        .filter(|r| r.name == "mod")
        .map(|r| r.value.clone())
        .collect()
}

/// Comprehensive server details from A2S queries
#[derive(Debug, Clone)]
pub struct ServerDetails {
    pub info: Info,
    pub players: Option<Vec<Player>>,
    pub rules: Option<Vec<Rule>>,
    pub ping_ms: Option<u32>,
    /// Mod names extracted from A2S rules (Protocol3 decoding for DayZ/Arma).
    /// Empty if rules query failed or no mods present.
    pub mods_from_rules: Vec<String>,
}

impl ServerDetails {
    /// Format as a string for display
    pub fn format(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!("Server: {}", self.info.name));
        lines.push(format!("Map: {}", self.info.map));
        lines.push(format!("Game: {}", self.info.game));
        lines.push(format!(
            "Players: {}/{}",
            self.info.players, self.info.max_players
        ));
        lines.push(format!("Version: {}", self.info.version));

        if let Some(ref players) = self.players {
            if !players.is_empty() {
                lines.push("Online players:".to_string());
                for player in players {
                    lines.push(format!("  {} ({} score)", player.name, player.score));
                }
            }
        }

        if let Some(ref rules) = self.rules {
            lines.push("Rules:".to_string());
            for rule in rules {
                lines.push(format!("  {} = {}", rule.name, rule.value));
            }
        }

        lines.join("\n")
    }
}
