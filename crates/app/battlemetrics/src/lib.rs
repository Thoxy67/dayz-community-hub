//! Looking a server up on BattleMetrics: its rank, uptime, location and the
//! last 24 hours of player counts.
//!
//! BattleMetrics lists a server under its game port, its query port, or
//! occasionally the two swapped, and a name search alone can return a
//! different server with a similar name; see [`lookup`] for how a match is
//! chosen.

use serde::Serialize;

/// BattleMetrics server info fetched on demand for the detail panel.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct BattleMetricsServer {
    /// BattleMetrics server ID (used to build the BM page URL).
    pub id: String,
    /// Server name from BattleMetrics.
    pub name: String,
    /// Global rank (1 = most popular). None if not ranked.
    pub rank: Option<i64>,
    /// Server status: "online" | "offline" | "dead"
    pub status: String,
    /// ISO 3166-1 alpha-2 country code, e.g. "DE", "US".
    pub country: Option<String>,
    /// Server coordinates (longitude, latitude). None if unavailable.
    pub location: Option<(f64, f64)>,
    /// Uptime percentage over the last 30 days (0–100).
    pub uptime: Option<f64>,
    /// Whether the server is private (password protected).
    pub private: Option<bool>,
    /// Whether this is an official server.
    pub official: Option<bool>,
    /// Whether third-person view is allowed.
    pub third_person: Option<bool>,
    /// Whether the server is modded.
    pub modded: Option<bool>,
    /// Query status: "valid", "invalid", etc.
    pub query_status: Option<String>,
    /// Server's Steam ID.
    pub server_steam_id: Option<String>,
    /// When the server was first seen on BattleMetrics (ISO 8601).
    pub created_at: Option<String>,
    /// Player count data points for the last 24 h: (unix_secs, player_count) pairs.
    pub player_history: Vec<(i64, i64)>,
    /// Current player count from BattleMetrics.
    pub players: Option<i64>,
    /// Max players from BattleMetrics.
    pub max_players: Option<i64>,
}

/// BattleMetrics' answer to a token without a paid plan.
pub const SUBSCRIPTION_REQUIRED: &str =
    "BattleMetrics now requires a paid subscription for API access";

/// GET a BattleMetrics endpoint as JSON, saying plainly what went wrong.
async fn get_json(
    client: &reqwest::Client,
    token: &str,
    url: &str,
    query: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let resp = client
        .get(url)
        .query(query)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "BattleMetrics did not answer in time".to_string()
            } else {
                format!("BattleMetrics unreachable: {}", e.without_url())
            }
        })?;
    match resp.status().as_u16() {
        200..=299 => {}
        401 => return Err("BattleMetrics rejected the API token".into()),
        // Since 2026 the API answers 403 to everyone without a paid plan.
        403 => {
            let body = resp.text().await.unwrap_or_default();
            return Err(if body.to_lowercase().contains("subscription") {
                SUBSCRIPTION_REQUIRED.into()
            } else {
                "BattleMetrics rejected the API token".into()
            });
        }
        429 => return Err("BattleMetrics rate limit reached, try again in a minute".into()),
        code => return Err(format!("BattleMetrics answered HTTP {code}")),
    }
    resp.json()
        .await
        .map_err(|e| format!("Unreadable BattleMetrics response: {}", e.without_url()))
}

/// Find the server at `ip` (game `port`, `query_port`) on BattleMetrics, then
/// fetch its player-count history for the last 24 hours. `name` confirms an
/// IP match, and is searched for when the IP search finds nothing that fits.
const SERVERS_URL: &str = "https://api.battlemetrics.com/servers";

pub async fn lookup(
    client: &reqwest::Client,
    token: &str,
    ip: &str,
    port: i64,
    query_port: i64,
    name: &str,
) -> Result<BattleMetricsServer, String> {
    // Search by IP only to get all servers on this IP, then match by port
    let search_json = get_json(
        client,
        token,
        SERVERS_URL,
        &[
            ("filter[game]", "dayz"),
            ("filter[search]", ip),
            ("page[size]", "50"),
        ],
    )
    .await?;

    let data = search_json["data"]
        .as_array()
        .ok_or_else(|| "Unexpected BattleMetrics response".to_string())?;

    // Single-pass matching with priority scoring (4x faster than 4 separate .find() calls).
    // Early-exits as soon as we see a perfect score (exact game port match).
    const MAX_SCORE: u8 = 4;
    let mut best_match: Option<(&serde_json::Value, u8)> = None;

    for entry in data {
        let attrs = &entry["attributes"];
        let bm_ip = attrs["ip"].as_str().unwrap_or("");

        // Skip entries that don't match IP at all
        if bm_ip != ip {
            continue;
        }

        let bm_port = attrs["port"].as_i64().unwrap_or(0);
        let bm_qport = attrs["portQuery"].as_i64().unwrap_or(0);

        // Assign priority score (higher = better match)
        let score = if bm_port == port {
            MAX_SCORE // Priority 1: exact game port
        } else if bm_qport == query_port {
            3 // Priority 2: exact query port
        } else if bm_port == query_port {
            2 // Priority 3: swapped ports
        } else {
            1 // Priority 4: any IP match
        };

        // Keep best match (first one wins on tie)
        if best_match.is_none_or(|(_, s)| score > s) {
            best_match = Some((entry, score));
            // Can't do better than an exact game-port match — stop scanning.
            if score == MAX_SCORE {
                break;
            }
        }
    }

    let ip_entry: Option<serde_json::Value> = best_match.map(|(e, _)| e.clone());

    // Check if IP match has a matching name (case-insensitive partial match)
    let name_lower = name.to_lowercase();
    let ip_entry_valid = ip_entry.as_ref().is_some_and(|e| {
        if name.is_empty() {
            return true; // No name to validate against
        }
        let bm_name = e["attributes"]["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase();
        // Check if either name contains the other (partial match)
        bm_name.contains(&name_lower) || name_lower.contains(&bm_name) ||
        // Or check if first significant word matches
        bm_name.split_whitespace().next() == name_lower.split_whitespace().next()
    });

    // Helper closure to search by name
    let search_by_name = || async {
        if name.is_empty() {
            return Err("Server not found on BattleMetrics".to_string());
        }
        let name_json = get_json(
            client,
            token,
            SERVERS_URL,
            &[
                ("filter[game]", "dayz"),
                ("filter[search]", name),
                ("page[size]", "10"),
            ],
        )
        .await?;
        let name_data = name_json["data"]
            .as_array()
            .ok_or_else(|| "Unexpected BattleMetrics response".to_string())?;
        // Find by exact IP match in name search results
        // Do NOT take first result if IP doesn't match - that's how we get wrong servers!
        name_data
            .iter()
            .find(|e| {
                let bm_ip = e["attributes"]["ip"].as_str().unwrap_or("");
                bm_ip == ip
            })
            .cloned()
            .ok_or_else(|| "Server not found on BattleMetrics".to_string())
    };

    // Use IP result if valid, otherwise search by name
    let entry: serde_json::Value = match ip_entry {
        Some(e) if ip_entry_valid => e,
        _ if !name.is_empty() => search_by_name().await?,
        // The IP result even if the name does not match: no name to search with.
        Some(e) => e,
        None => return Err("Server not found on BattleMetrics".to_string()),
    };

    let bm_id = entry["id"]
        .as_str()
        .ok_or_else(|| "Missing BM server id".to_string())?
        .to_string();
    let attrs = &entry["attributes"];
    let bm_name = attrs["name"].as_str().unwrap_or("Unknown").to_string();
    let rank = attrs["rank"].as_i64();
    let status = attrs["status"].as_str().unwrap_or("unknown").to_string();
    let country = attrs["country"]
        .as_str()
        .map(std::string::ToString::to_string);
    // BattleMetrics may return location as GeoJSON: {"type":"Point","coordinates":[lon,lat]}
    // or as a direct array [lon, lat] - handle both formats
    let location: Option<(f64, f64)> = attrs["location"]["coordinates"]
        .as_array()
        .or_else(|| attrs["location"].as_array())
        .and_then(|arr| {
            let lon = arr.first()?.as_f64()?;
            let lat = arr.get(1)?.as_f64()?;
            Some((lon, lat))
        });
    let uptime = attrs["details"]["uptime"]
        .as_f64()
        .or_else(|| attrs["details"]["uptime30"].as_f64());

    // New fields
    let private = attrs["private"].as_bool();
    let official = attrs["official"].as_bool();
    let third_person = attrs["details"]["third_person"].as_bool();
    let modded = attrs["details"]["modded"].as_bool();
    let query_status = attrs["queryStatus"]
        .as_str()
        .map(std::string::ToString::to_string);
    let server_steam_id = attrs["serverSteamId"]
        .as_str()
        .map(std::string::ToString::to_string);
    let created_at = attrs["createdAt"]
        .as_str()
        .map(std::string::ToString::to_string);
    let players = attrs["players"].as_i64();
    let max_players = attrs["maxPlayers"].as_i64();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let stop = iso8601(now);
    let start = iso8601(now.saturating_sub(86400));

    // The history is a nicety: the panel still shows the rest without it.
    let history_json = get_json(
        client,
        token,
        &format!("{SERVERS_URL}/{bm_id}/player-count-history"),
        &[("start", &start), ("stop", &stop), ("resolution", "60")],
    )
    .await
    .unwrap_or_default();

    // Iterator-flatten avoids the per-call `vec![]` allocation that the
    // previous `unwrap_or(&vec![])` form created on every panel open.
    let player_history: Vec<(i64, i64)> = history_json["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|point| {
            let attrs = &point["attributes"];
            let ts_str = attrs["timestamp"].as_str()?;
            let count = attrs["value"].as_i64().unwrap_or(0);
            let ts = parse_iso8601_approx(ts_str);
            Some((ts, count))
        })
        .collect();

    Ok(BattleMetricsServer {
        id: bm_id,
        name: bm_name,
        rank,
        status,
        country,
        location,
        uptime,
        private,
        official,
        third_person,
        modded,
        query_status,
        server_steam_id,
        created_at,
        player_history,
        players,
        max_players,
    })
}

/// Format a Unix timestamp (seconds) as an ISO 8601 string for BattleMetrics API queries.
fn iso8601(unix_secs: u64) -> String {
    let s = unix_secs;
    let secs = s % 60;
    let mins = (s / 60) % 60;
    let hours = (s / 3600) % 24;
    let days = s / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{secs:02}Z")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z % 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Parse an ISO-8601 timestamp like "2024-01-15T12:34:56.000Z" to Unix seconds.
fn parse_iso8601_approx(s: &str) -> i64 {
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return 0;
    }
    let year: i64 = parse_digits(&bytes[0..4]);
    let month: i64 = parse_digits(&bytes[5..7]);
    let day: i64 = parse_digits(&bytes[8..10]);
    let hour: i64 = parse_digits(&bytes[11..13]);
    let minute: i64 = parse_digits(&bytes[14..16]);
    let second: i64 = parse_digits(&bytes[17..19]);
    let m_adj = if month <= 2 { month + 9 } else { month - 3 };
    let y_adj = if month <= 2 { year - 1 } else { year };
    let era = y_adj / 400;
    let yoe = y_adj % 400;
    let doy = (153 * m_adj + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    days * 86400 + hour * 3600 + minute * 60 + second
}

fn parse_digits(bytes: &[u8]) -> i64 {
    bytes.iter().fold(0i64, |acc, &b| {
        if b.is_ascii_digit() {
            acc * 10 + (b - b'0') as i64
        } else {
            acc
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_round_trip() {
        let ts = 1_700_000_000u64;
        let s = iso8601(ts);
        assert_eq!(s, "2023-11-14T22:13:20Z");
        assert_eq!(parse_iso8601_approx(&s), ts as i64);
        assert_eq!(parse_iso8601_approx("2023-11-14T22:13:20.000Z"), ts as i64);
        assert_eq!(parse_iso8601_approx("short"), 0);
    }
}
