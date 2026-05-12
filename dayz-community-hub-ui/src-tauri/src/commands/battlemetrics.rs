use std::time::{Duration, Instant};
use tauri::State;

use crate::dto::BattleMetricsDto;
use crate::state::{BM_CACHE_TTL_SECS, SharedState, insecure_client};
use crate::utils::error::ResultExt;

/// Fetch BattleMetrics server info by IP + ports, with name fallback.
#[tauri::command]
pub(crate) async fn fetch_battlemetrics_server(
    ip: String,
    port: i64,
    query_port: i64,
    name: String,
    state: State<'_, SharedState>,
) -> Result<BattleMetricsDto, String> {
    let bm_cache_key = format!("{}:{}:{}", ip, port, query_port);

    // Check cache first (read lock only)
    let token = {
        let s = state.read().await;
        if let Some((cached, fetched_at)) = s.bm_cache.peek(&bm_cache_key)
            && fetched_at.elapsed() < Duration::from_secs(BM_CACHE_TTL_SECS) {
                return Ok(cached.clone());
            }
        s.ctl
            .profile()
            .battlemetrics_api_key
            .clone()
            .ok_or_else(|| "No BattleMetrics API key configured".to_string())?
    };

    let client = insecure_client();

    // Search by IP only to get all servers on this IP, then match by port
    let search_url = format!(
        "https://api.battlemetrics.com/servers?filter[game]=dayz&filter[search]={ip}&page[size]=50"
    );
    let search_resp = client
        .get(&search_url)
        .bearer_auth(&token)
        .send()
        .await
        .cmd_err()?;
    let search_json: serde_json::Value = search_resp.json().await.cmd_err()?;

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
        let name_encoded = urlencoding::encode(&name);
        let name_search_url = format!(
            "https://api.battlemetrics.com/servers?filter[game]=dayz&filter[search]={name_encoded}&page[size]=10"
        );
        let name_resp = client
            .get(&name_search_url)
            .bearer_auth(&token)
            .send()
            .await
            .cmd_err()?;
        let name_json: serde_json::Value = name_resp.json().await.cmd_err()?;
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
    let entry: serde_json::Value = if ip_entry_valid {
        ip_entry.unwrap()
    } else if !name.is_empty() {
        search_by_name().await?
    } else if let Some(e) = ip_entry {
        e // Use IP result even if name doesn't match (no name to search with)
    } else {
        return Err("Server not found on BattleMetrics".to_string());
    };

    let bm_id = entry["id"]
        .as_str()
        .ok_or_else(|| "Missing BM server id".to_string())?
        .to_string();
    let attrs = &entry["attributes"];
    let bm_name = attrs["name"].as_str().unwrap_or("Unknown").to_string();
    let rank = attrs["rank"].as_i64();
    let status = attrs["status"].as_str().unwrap_or("unknown").to_string();
    let country = attrs["country"].as_str().map(|s| s.to_string());
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
    let query_status = attrs["queryStatus"].as_str().map(|s| s.to_string());
    let server_steam_id = attrs["serverSteamId"].as_str().map(|s| s.to_string());
    let created_at = attrs["createdAt"].as_str().map(|s| s.to_string());
    let players = attrs["players"].as_i64();
    let max_players = attrs["maxPlayers"].as_i64();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let stop = chrono_or_fallback(now);
    let start = chrono_or_fallback(now.saturating_sub(86400));

    let history_url = format!(
        "https://api.battlemetrics.com/servers/{bm_id}/player-count-history\
         ?start={start}&stop={stop}&resolution=60"
    );
    let history_resp = client
        .get(&history_url)
        .bearer_auth(&token)
        .send()
        .await
        .cmd_err()?;
    let history_json: serde_json::Value = history_resp.json().await.cmd_err()?;

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

    let result = BattleMetricsDto {
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
    };

    // Cache the result (write lock)
    {
        let mut s = state.write().await;
        s.bm_cache
            .put(bm_cache_key, (result.clone(), Instant::now()));
    }

    Ok(result)
}

/// Format a Unix timestamp (seconds) as an ISO 8601 string for BattleMetrics API queries.
fn chrono_or_fallback(unix_secs: u64) -> String {
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
