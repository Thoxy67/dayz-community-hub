use crate::api::Server;
use crate::{Result, errors::Error};
use a2s::{A2SClient, rules};
use ping_async::IcmpEchoRequestor;

use std::time::{Duration, Instant};

/// Query server information using A2S protocol
pub async fn query_server_info(server: &Server) -> Result<a2s::info::Info> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    // Use endpoint port (query port)
    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .info(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S query failed: {}", e)))
}

/// Query player information using A2S protocol
pub async fn query_player_info(server: &Server) -> Result<Vec<a2s::players::Player>> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .players(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S players query failed: {}", e)))
}

/// Query rules (cvars) using A2S protocol
pub async fn query_rules(server: &Server) -> Result<Vec<rules::Rule>> {
    let mut client = A2SClient::new()
        .await
        .map_err(|e| Error::A2sQuery(format!("Failed to create A2S client: {}", e)))?;

    let timeout = Duration::from_secs(5);
    let _ = client.set_timeout(timeout);

    let query_port = server.endpoint.port as u16;
    let addr = format!("{}:{}", server.endpoint.ip, query_port);

    client
        .rules(&addr)
        .await
        .map_err(|e| Error::A2sQuery(format!("A2S rules query failed: {}", e)))
}

/// Ping a server using unprivileged ICMP (SOCK_DGRAM / IPPROTO_ICMP).
/// No root required — works when `/proc/sys/net/ipv4/ping_group_range` includes your GID.
/// Sends a single Echo Request and returns the RTT in milliseconds.
pub async fn ping_server(server: &Server) -> Result<u32> {
    let ip = server
        .endpoint
        .ip
        .parse()
        .map_err(|_| Error::A2sQuery(format!("Invalid IP: {}", server.endpoint.ip)))?;

    let pinger = IcmpEchoRequestor::new(ip, None, None, None)
        .map_err(|e| Error::A2sQuery(format!("ICMP init failed: {}", e)))?;

    let start = Instant::now();
    let reply = pinger
        .send()
        .await
        .map_err(|e| Error::A2sQuery(format!("Ping failed: {}", e)))?;

    // reply.round_trip_time() is the authoritative RTT from the library,
    // but fall back to our own elapsed if it returns zero.
    let rtt = reply.round_trip_time();
    if rtt.is_zero() {
        Ok(start.elapsed().as_millis() as u32)
    } else {
        Ok(rtt.as_millis() as u32)
    }
}

/// Get comprehensive server information including players and rules
pub async fn get_server_details(server: &Server) -> Result<ServerDetails> {
    // Measure ping during info query
    let start = Instant::now();
    let info = query_server_info(server).await?;
    let ping_ms = start.elapsed().as_millis() as u32;

    let players = query_player_info(server).await.ok();
    let rules = query_rules(server).await.ok();

    Ok(ServerDetails {
        info,
        players,
        rules,
        ping_ms: Some(ping_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use a2s::A2SClient;
    use tokio::try_join;

    /// Direct A2S test against a live server — mirrors the a2s lib's own async test.
    /// Run with: cargo test --package dayzsa-core -- a2s_query::tests::test_live_server --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_server() {
        let addr = "195.60.166.46:27016";
        println!("=== Direct A2SClient test against {} ===", addr);

        let client = A2SClient::new().await.expect("A2SClient::new failed");

        // info only first — cheapest query
        match client.info(addr).await {
            Ok(info) => println!("INFO ok: name={:?} map={:?} players={}/{} version={:?}",
                info.name, info.map, info.players, info.max_players, info.version),
            Err(e) => println!("INFO error: {:?}", e),
        }

        // new client for players (socket is stateful)
        let client2 = A2SClient::new().await.expect("A2SClient::new failed");
        match client2.players(addr).await {
            Ok(players) => println!("PLAYERS ok: {} players", players.len()),
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
                println!("  players = {}/{}", details.info.players, details.info.max_players);
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
        let info_fut = client.info(addr);
        let players_fut = client.players(addr);

        match try_join!(info_fut, players_fut) {
            Ok((info, players)) => {
                println!("try_join ok");
                println!("  name={:?} players={}/{}", info.name, info.players, info.max_players);
                println!("  {} online", players.len());
            }
            Err(e) => println!("try_join error: {:?}", e),
        }
    }
}

/// Comprehensive server details from A2S queries
#[derive(Debug, Clone)]
pub struct ServerDetails {
    pub info: a2s::info::Info,
    pub players: Option<Vec<a2s::players::Player>>,
    pub rules: Option<Vec<rules::Rule>>,
    pub ping_ms: Option<u32>,
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
