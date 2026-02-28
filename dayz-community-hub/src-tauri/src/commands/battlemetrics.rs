use tauri::State;

use crate::dto::BattleMetricsDto;
use crate::state::{SharedState, insecure_client};

/// Fetch BattleMetrics server info by IP + query port.
#[tauri::command]
pub(crate) async fn fetch_battlemetrics_server(
    ip: String,
    port: i64,
    state: State<'_, SharedState>,
) -> Result<BattleMetricsDto, String> {
    let bm_cache_key = format!("{}:{}", ip, port);
    const BM_TTL_SECS: u64 = 300;

    let token = {
        let s = state.lock().await;
        if let Some((cached, fetched_at)) = s.bm_cache.get(&bm_cache_key) {
            if fetched_at.elapsed().as_secs() < BM_TTL_SECS {
                return Ok(cached.clone());
            }
        }
        s.ctl
            .profile()
            .battlemetrics_api_key
            .clone()
            .ok_or_else(|| "No BattleMetrics API key configured".to_string())?
    };

    let client = insecure_client();

    let search_url = format!(
        "https://api.battlemetrics.com/servers?filter[game]=dayz&filter[search]={ip}:{port}&page[size]=5"
    );
    let search_resp = client
        .get(&search_url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let search_json: serde_json::Value = search_resp.json().await.map_err(|e| e.to_string())?;

    let data = search_json["data"]
        .as_array()
        .ok_or_else(|| "Unexpected BattleMetrics response".to_string())?;

    let entry = data
        .iter()
        .find(|e| {
            let attrs = &e["attributes"];
            let bm_ip = attrs["ip"].as_str().unwrap_or("");
            let bm_port = attrs["port"].as_i64().unwrap_or(0);
            bm_ip == ip && (bm_port == port || bm_port == port - 1 || bm_port == port + 1)
        })
        .or_else(|| data.first())
        .ok_or_else(|| "Server not found on BattleMetrics".to_string())?;

    let bm_id = entry["id"]
        .as_str()
        .ok_or_else(|| "Missing BM server id".to_string())?
        .to_string();
    let attrs = &entry["attributes"];
    let rank = attrs["rank"].as_i64();
    let status = attrs["status"].as_str().unwrap_or("unknown").to_string();
    let country = attrs["country"].as_str().map(|s| s.to_string());
    // BattleMetrics may return location as GeoJSON: {"type":"Point","coordinates":[lon,lat]}
    // or as a direct array [lon, lat] - handle both formats
    let location: Option<(f64, f64)> = attrs["location"]["coordinates"]
        .as_array()
        .or_else(|| attrs["location"].as_array())
        .and_then(|arr| {
            let lon = arr.get(0)?.as_f64()?;
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
        .map_err(|e| e.to_string())?;
    let history_json: serde_json::Value = history_resp.json().await.map_err(|e| e.to_string())?;

    let player_history: Vec<(i64, i64)> = history_json["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
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
    };

    state
        .lock()
        .await
        .bm_cache
        .insert(bm_cache_key, (result.clone(), std::time::Instant::now()));

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
