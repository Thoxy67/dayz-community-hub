use crate::api::Server;
use crate::{Result, errors::Error};
use a2s::{A2SClient, rules};
use surge_ping::{Client, Config, PingIdentifier, PingSequence, ICMP};

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

/// Shared ICMP clients (one per address family) — cheap to clone, expensive to create.
pub struct IcmpClients {
    pub v4: Client,
    pub v6: Client,
}

impl IcmpClients {
    pub fn new() -> Result<Self> {
        let v4 = Client::new(&Config::builder().kind(ICMP::V4).build())
            .map_err(|e| Error::A2sQuery(format!("ICMPv4 client init failed: {e}")))?;
        let v6 = Client::new(&Config::builder().kind(ICMP::V6).build())
            .map_err(|e| Error::A2sQuery(format!("ICMPv6 client init failed: {e}")))?;
        Ok(Self { v4, v6 })
    }

    pub fn for_ip(&self, ip: std::net::IpAddr) -> &Client {
        if ip.is_ipv4() { &self.v4 } else { &self.v6 }
    }
}

/// Measure RTT via a single A2S info query — used as a fallback when ICMP fails.
/// Returns the round-trip time in milliseconds.
pub async fn ping_via_a2s(server: &Server) -> Result<u32> {
    let start = Instant::now();
    query_server_info(server).await?;
    Ok(start.elapsed().as_millis() as u32)
}

/// Sentinel value emitted when all ping attempts fail — treated as TIMEOUT on
/// the frontend.  Any value >= 5 000 ms is displayed as "TIMEOUT"; we use 9 999
/// so it is clearly a sentinel rather than a real RTT.
pub const PING_TIMEOUT_SENTINEL: u32 = 9_999;

/// Batch ping using a pre-built shared client.
/// Retries up to `retries` ICMP attempts; if ICMP succeeds the RTT is returned
/// immediately.  Only if *all* ICMP attempts fail is A2S tried as a last resort.
/// Always returns `Ok` — unreachable servers return `Ok(PING_TIMEOUT_SENTINEL)`
/// so the caller can emit a result without needing to handle `Err`.
pub async fn ping_once(
    client: &Client,
    ip: std::net::IpAddr,
    ident: PingIdentifier,
    timeout: Duration,
    retries: u16,
    server: &Server,
) -> u32 {
    const PAYLOAD: [u8; 8] = [0; 8];
    let mut pinger = client.pinger(ip, ident).await;
    pinger.timeout(timeout);

    // Try ICMP up to (retries + 1) times.  The first Ok reply is the RTT.
    let icmp_ms: u32 = 'icmp: {
        for seq in 0..=retries {
            if let Ok((_pkt, dur)) = pinger.ping(PingSequence(seq), &PAYLOAD).await {
                break 'icmp dur.as_millis() as u32;
            }
        }
        PING_TIMEOUT_SENTINEL
    };

    // Only fall back to A2S when ICMP got no reply at all.
    if icmp_ms < PING_TIMEOUT_SENTINEL {
        icmp_ms
    } else {
        ping_via_a2s(server).await.unwrap_or(PING_TIMEOUT_SENTINEL)
    }
}

/// Ping a server using unprivileged ICMP via surge-ping.
/// Creates its own client — use for manual single-server pings only.
/// Retries up to 2 times on failure.
/// `timeout_ms` controls the per-attempt timeout (default: 5000 ms).
pub async fn ping_server(server: &Server, timeout_ms: Option<u64>) -> Result<u32> {
    let ip: std::net::IpAddr = server
        .endpoint
        .ip
        .parse()
        .map_err(|_| Error::A2sQuery(format!("Invalid IP: {}", server.endpoint.ip)))?;

    let timeout = Duration::from_millis(timeout_ms.unwrap_or(5_000));
    const MAX_RETRIES: u32 = 2;
    const PAYLOAD: [u8; 8] = [0; 8];

    let icmp_kind = if ip.is_ipv4() { ICMP::V4 } else { ICMP::V6 };
    let client = Client::new(&Config::builder().kind(icmp_kind).build())
        .map_err(|e| Error::A2sQuery(format!("ICMP client init failed: {e}")))?;

    let ident = PingIdentifier(std::process::id() as u16);
    let mut pinger = client.pinger(ip, ident).await;
    pinger.timeout(timeout);

    let mut last_err: Option<String> = None;
    for seq in 0..=MAX_RETRIES {
        match pinger.ping(PingSequence(seq as u16), &PAYLOAD).await {
            Ok((_packet, duration)) => {
                return Ok(duration.as_millis() as u32);
            }
            Err(e) => {
                last_err = Some(e.to_string());
            }
        }
    }

    Err(Error::A2sQuery(format!(
        "Ping failed: {}",
        last_err.unwrap_or_default()
    )))
}

/// Get comprehensive server information including players and rules.
///
/// All three A2S queries (info, players, rules) are issued concurrently via
/// `tokio::join!` — wall-clock time is ~1 RTT instead of ~3 RTTs.
/// The ping is measured as the time until the first response (info) arrives,
/// which is the most meaningful latency indicator for the user.
pub async fn get_server_details(server: &Server) -> Result<ServerDetails> {
    let start = Instant::now();

    let (info_result, players_result, rules_result) = tokio::join!(
        query_server_info(server),
        query_player_info(server),
        query_rules(server),
    );

    let ping_ms = start.elapsed().as_millis() as u32;
    let info = info_result?;
    let players = players_result.ok();
    let rules = rules_result.ok();

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
    /// Run with: cargo test --package dayz-community-hub-core -- a2s_query::tests::test_live_server --nocapture --ignored
    #[tokio::test]
    #[ignore]
    async fn test_live_server() {
        let addr = "195.60.166.46:27016";
        println!("=== Direct A2SClient test against {} ===", addr);

        let client = A2SClient::new().await.expect("A2SClient::new failed");

        // info only first — cheapest query
        match client.info(addr).await {
            Ok(info) => println!(
                "INFO ok: name={:?} map={:?} players={}/{} version={:?}",
                info.name, info.map, info.players, info.max_players, info.version
            ),
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
        let info_fut = client.info(addr);
        let players_fut = client.players(addr);

        match try_join!(info_fut, players_fut) {
            Ok((info, players)) => {
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
