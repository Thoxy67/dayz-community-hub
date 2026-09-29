//! Looking a server up on DayZ Metrics (dayzmetrics.com): its rank, uptime,
//! restart and wipe schedules, whether its population looks fake, and the
//! last 24 hours of player counts and pings.
//!
//! The site has no published API; these are the JSON endpoints its own pages
//! use, read without a key. The app asks only when a server's panel opens and
//! caches the answers (see the shell's `features/metrics`), so it costs the
//! site no more than a visitor would.
//!
//! The server list does not carry addresses, so a server is found by
//! searching its IP and then reading the candidates' details until one has
//! the same game or query port; see [`lookup`].

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The site, for links to a server's page.
pub const SITE: &str = "https://dayzmetrics.com";
const API: &str = "https://dayzmetrics.com/api";
/// The error for a server the site does not know.
pub const NOT_LISTED: &str = "Not listed on DayZ Metrics";
/// Candidates read before giving up: an IP rarely hosts more servers.
const MAX_CANDIDATES: usize = 10;

// ── what the site sends ──────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
#[serde(default)]
struct SearchPage {
    servers: Vec<SearchItem>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct SearchItem {
    id: u64,
    avg_players_7d: Option<f64>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RawRestart {
    period_hours: Option<f64>,
    last_restart: Option<String>,
    next_restart: Option<String>,
    confidence: Option<String>,
    slots_utc: Vec<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RawWipe {
    last: Option<String>,
    last_source: Option<String>,
    days_since: Option<f64>,
    next: Option<String>,
    next_source: Option<String>,
    days_until: Option<f64>,
    period_days: Option<f64>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RawCurrent {
    players: Option<f64>,
    max_players: Option<f64>,
    queue: Option<f64>,
    status: Option<String>,
}

/// A server's page, as `/api/server/<id>` sends it. Every field is optional
/// on this side: a field the site renames or drops costs that figure, not the
/// whole panel.
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RawDetail {
    id: u64,
    ip: String,
    query_port: Option<u32>,
    game_port: Option<u32>,
    name: String,
    map: Option<String>,
    version: Option<String>,
    country: Option<String>,
    first_seen: Option<String>,
    last_seen: Option<String>,
    current: Option<RawCurrent>,
    rank_pos: Option<f64>,
    rank_score: Option<f64>,
    rank_alive: Option<f64>,
    rank_demand: Option<f64>,
    peak_7d: Option<f64>,
    uptime_7d: Option<f64>,
    wow_pct: Option<f64>,
    ping_lo: Option<f64>,
    ping_hi: Option<f64>,
    ping_jitter: Option<f64>,
    ping_stability: Option<f64>,
    time_accel: Option<f64>,
    night_time_accel: Option<f64>,
    restart: Option<RawRestart>,
    wipe: Option<RawWipe>,
    is_fake: bool,
    fake_reasons: Vec<Value>,
    behavior_verdict: Option<String>,
    behavior_score: Option<f64>,
    flagged: bool,
    mimics_official: bool,
    discord: Option<String>,
    website: Option<String>,
    links: Vec<Value>,
    notices: Vec<Value>,
    playstyle: Option<Value>,
    vanilla_band: Option<String>,
    vanilla_score: Option<f64>,
    mod_count: Option<f64>,
    mod_total_bytes: Option<f64>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RawPoint {
    t: String,
    players: Option<f64>,
    ping: Option<f64>,
}

// ── what the app gets ────────────────────────────────────────────────────────

/// When the server restarts, as the site has measured it.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct RestartSchedule {
    pub period_hours: Option<f64>,
    /// ISO 8601.
    pub last_restart: Option<String>,
    /// ISO 8601.
    pub next_restart: Option<String>,
    /// "high" | "medium" | "low", as the site rates its own guess.
    pub confidence: Option<String>,
    /// Restart times of day, "HH:MM" UTC.
    pub slots_utc: Vec<String>,
}

/// When the server wipes: the last one and the next, each announced or guessed.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct WipeSchedule {
    /// "YYYY-MM-DD".
    pub last: Option<String>,
    /// "announced" | "detected" | …
    pub last_source: Option<String>,
    pub days_since: Option<f64>,
    /// "YYYY-MM-DD".
    pub next: Option<String>,
    /// "announced" | "predicted" | …
    pub next_source: Option<String>,
    pub days_until: Option<f64>,
    pub period_days: Option<f64>,
}

/// A link the server lists on its page.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct MetricsLink {
    pub label: String,
    pub url: String,
}

/// What DayZ Metrics knows about a server, for the server panel's Stats tab.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct ServerMetrics {
    /// The site's id for the server.
    pub id: u64,
    /// Its page on dayzmetrics.com.
    pub url: String,
    pub name: String,
    pub map: Option<String>,
    pub version: Option<String>,
    /// "online" | "offline" | …
    pub status: Option<String>,
    /// ISO 3166-1 alpha-2.
    pub country: Option<String>,
    pub players: Option<f64>,
    pub max_players: Option<f64>,
    pub queue: Option<f64>,
    /// Position in the site's ranking (1 is the top).
    pub rank_pos: Option<f64>,
    pub rank_score: Option<f64>,
    /// How reliably it is up, 0–1, as the ranking weighs it.
    pub rank_alive: Option<f64>,
    /// How sought after it is, 0–1, as the ranking weighs it.
    pub rank_demand: Option<f64>,
    pub avg_players_7d: Option<f64>,
    pub peak_7d: Option<f64>,
    /// Percent of the last seven days it answered.
    pub uptime_7d: Option<f64>,
    /// Players week over week, percent (+12 means 12 % more than last week).
    pub wow_pct: Option<f64>,
    /// ISO 8601.
    pub first_seen: Option<String>,
    /// ISO 8601.
    pub last_seen: Option<String>,
    /// The site's own pings to it, milliseconds.
    pub ping_lo: Option<f64>,
    pub ping_hi: Option<f64>,
    pub ping_jitter: Option<f64>,
    pub ping_stability: Option<f64>,
    /// In-game time multiplier, by day and by night.
    pub time_accel: Option<f64>,
    pub night_time_accel: Option<f64>,
    pub restart: Option<RestartSchedule>,
    pub wipe: Option<WipeSchedule>,
    /// The site judges the player count to be padded (bots, fake slots).
    pub is_fake: bool,
    pub fake_reasons: Vec<String>,
    /// "real" | "suspicious" | "fake" | …: how the count behaves over time.
    pub behavior_verdict: Option<String>,
    pub behavior_score: Option<f64>,
    /// Reported by players and flagged by the site.
    pub flagged: bool,
    /// Named to look like an official server without being one.
    pub mimics_official: bool,
    pub discord: Option<String>,
    pub website: Option<String>,
    pub links: Vec<MetricsLink>,
    pub notices: Vec<String>,
    pub playstyle: Option<String>,
    /// "Vanilla" … "Heavily Modded", and the score behind it (0–100).
    pub vanilla_band: Option<String>,
    pub vanilla_score: Option<f64>,
    pub mod_count: Option<f64>,
    /// Bytes to download for every mod the server runs.
    pub mod_total_bytes: Option<f64>,
    /// The last 24 hours: (unix seconds, players), every five minutes.
    pub player_history: Vec<(i64, f64)>,
    /// The last 24 hours: (unix seconds, ms), where the site measured it.
    pub ping_history: Vec<(i64, f64)>,
}

/// What finding a server once is worth remembering: its id, and the one
/// figure only the search carries.
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub id: u64,
    pub avg_players_7d: Option<f64>,
}

// ── requests ─────────────────────────────────────────────────────────────────

async fn get<T: DeserializeOwned>(
    client: &reqwest::Client,
    path: &str,
    query: &[(&str, &str)],
) -> Result<T, String> {
    let resp = client
        .get(format!("{API}{path}"))
        .query(query)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "DayZ Metrics did not answer in time".to_string()
            } else {
                format!("DayZ Metrics unreachable: {}", e.without_url())
            }
        })?;
    match resp.status().as_u16() {
        200..=299 => {}
        404 => return Err(NOT_LISTED.into()),
        429 => return Err("DayZ Metrics is busy, try again in a minute".into()),
        code => return Err(format!("DayZ Metrics answered HTTP {code}")),
    }
    resp.json()
        .await
        .map_err(|e| format!("Unreadable DayZ Metrics response: {}", e.without_url()))
}

async fn detail(client: &reqwest::Client, id: u64) -> Result<RawDetail, String> {
    get(client, &format!("/server/{id}"), &[]).await
}

/// How well a server's page fits the address asked for: 2 for the same game
/// port, 1 for the same query port, 0 for another server.
fn fit(d: &RawDetail, ip: &str, game_port: u32, query_port: u32) -> u8 {
    if d.ip != ip {
        0
    } else if d.game_port == Some(game_port) {
        2
    } else if d.query_port == Some(query_port) {
        1
    } else {
        0
    }
}

/// Find the server at `ip` among the site's search results: the candidates'
/// pages are read one by one (gently), and an exact game-port match stops the
/// search; a query-port match is kept in case none does.
async fn resolve(
    client: &reqwest::Client,
    ip: &str,
    game_port: u32,
    query_port: u32,
) -> Result<(Resolved, RawDetail), String> {
    let page: SearchPage = get(
        client,
        "/servers",
        &[("q", ip), ("fakes", "show"), ("limit", "50")],
    )
    .await?;
    let mut fallback: Option<(Resolved, RawDetail)> = None;
    for item in page.servers.iter().take(MAX_CANDIDATES) {
        let d = match detail(client, item.id).await {
            Ok(d) => d,
            // One unreadable candidate does not end the search.
            Err(e) if e == NOT_LISTED => continue,
            Err(e) => return Err(e),
        };
        let found = Resolved {
            id: item.id,
            avg_players_7d: item.avg_players_7d,
        };
        match fit(&d, ip, game_port, query_port) {
            2 => return Ok((found, d)),
            1 if fallback.is_none() => fallback = Some((found, d)),
            _ => {}
        }
    }
    fallback.ok_or_else(|| NOT_LISTED.to_string())
}

/// Everything the site knows about the server at `ip`. `known` is an earlier
/// [`Resolved`] for the same address, which skips the search; if the site now
/// has another server under that id, the search runs again.
pub async fn lookup(
    client: &reqwest::Client,
    ip: &str,
    game_port: u32,
    query_port: u32,
    known: Option<Resolved>,
) -> Result<(Resolved, ServerMetrics), String> {
    let known_detail = match known {
        Some(r) => match detail(client, r.id).await {
            Ok(d) if fit(&d, ip, game_port, query_port) > 0 => Some((r, d)),
            Ok(_) | Err(_) => None,
        },
        None => None,
    };
    let (resolved, raw) = match known_detail {
        Some(found) => found,
        None => resolve(client, ip, game_port, query_port).await?,
    };
    // The history is a nicety: the rest of the page stands without it.
    let points: Vec<RawPoint> = get(
        client,
        &format!("/server/{}/timeseries", resolved.id),
        &[("range", "24h")],
    )
    .await
    .unwrap_or_default();
    Ok((resolved, build(resolved, raw, &points)))
}

// ── shaping ──────────────────────────────────────────────────────────────────

/// A value the site sends as either a plain string or an object with one of
/// a few likely text fields.
fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => [
            "text", "message", "reason", "title", "label", "name", "code",
        ]
        .iter()
        .find_map(|k| o.get(*k).and_then(Value::as_str).map(str::to_string)),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn link_of(v: &Value) -> Option<MetricsLink> {
    match v {
        Value::String(url) => Some(MetricsLink {
            label: url.clone(),
            url: url.clone(),
        }),
        Value::Object(o) => {
            let url = o
                .get("url")
                .or_else(|| o.get("href"))?
                .as_str()?
                .to_string();
            let label = ["label", "title", "name", "type", "kind"]
                .iter()
                .find_map(|k| o.get(*k).and_then(Value::as_str))
                .unwrap_or(&url)
                .to_string();
            Some(MetricsLink { label, url })
        }
        _ => None,
    }
}

fn build(resolved: Resolved, d: RawDetail, points: &[RawPoint]) -> ServerMetrics {
    let current = d.current.unwrap_or_default();
    let mut player_history = Vec::with_capacity(points.len());
    let mut ping_history = Vec::with_capacity(points.len());
    for p in points {
        let Some(t) = unix_secs(&p.t) else { continue };
        if let Some(n) = p.players {
            player_history.push((t, n));
        }
        if let Some(ms) = p.ping {
            ping_history.push((t, ms));
        }
    }
    ServerMetrics {
        id: resolved.id,
        url: format!("{SITE}/server/{}", resolved.id),
        name: d.name,
        map: d.map,
        version: d.version,
        status: current.status,
        country: d.country,
        players: current.players,
        max_players: current.max_players,
        queue: current.queue,
        rank_pos: d.rank_pos,
        rank_score: d.rank_score,
        rank_alive: d.rank_alive,
        rank_demand: d.rank_demand,
        avg_players_7d: resolved.avg_players_7d,
        peak_7d: d.peak_7d,
        uptime_7d: d.uptime_7d,
        wow_pct: d.wow_pct,
        first_seen: d.first_seen,
        last_seen: d.last_seen,
        ping_lo: d.ping_lo,
        ping_hi: d.ping_hi,
        ping_jitter: d.ping_jitter,
        ping_stability: d.ping_stability,
        time_accel: d.time_accel,
        night_time_accel: d.night_time_accel,
        restart: d.restart.map(|r| RestartSchedule {
            period_hours: r.period_hours,
            last_restart: r.last_restart,
            next_restart: r.next_restart,
            confidence: r.confidence,
            slots_utc: r.slots_utc,
        }),
        wipe: d.wipe.map(|w| WipeSchedule {
            last: w.last,
            last_source: w.last_source,
            days_since: w.days_since,
            next: w.next,
            next_source: w.next_source,
            days_until: w.days_until,
            period_days: w.period_days,
        }),
        is_fake: d.is_fake,
        fake_reasons: d.fake_reasons.iter().filter_map(text_of).collect(),
        behavior_verdict: d.behavior_verdict,
        behavior_score: d.behavior_score,
        flagged: d.flagged,
        mimics_official: d.mimics_official,
        discord: d.discord.filter(|s| !s.is_empty()),
        website: d.website.filter(|s| !s.is_empty()),
        links: d.links.iter().filter_map(link_of).collect(),
        notices: d.notices.iter().filter_map(text_of).collect(),
        playstyle: d.playstyle.as_ref().and_then(text_of),
        vanilla_band: d.vanilla_band,
        vanilla_score: d.vanilla_score,
        mod_count: d.mod_count,
        mod_total_bytes: d.mod_total_bytes,
        player_history,
        ping_history,
    }
}

/// "2026-09-28T14:05:00+00:00" (or with a fraction, or `Z`) to Unix seconds.
fn unix_secs(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[10] != b'T' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> Option<i64> { s.get(r)?.parse().ok() };
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    // Days from the civil calendar (Howard Hinnant's algorithm).
    let (y, mo) = if mo <= 2 {
        (y - 1, mo + 9)
    } else {
        (y, mo - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * mo + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let mut t = days * 86_400 + h * 3600 + mi * 60 + se;
    // A trailing "+HH:MM" / "-HH:MM" offset, after any fraction.
    let tail = &s[19..];
    if let Some(i) = tail.find(['+', '-']) {
        let off = &tail[i..];
        let sign = if off.starts_with('-') { -1 } else { 1 };
        let oh: i64 = off.get(1..3)?.parse().ok()?;
        let om: i64 = off.get(4..6).and_then(|m| m.parse().ok()).unwrap_or(0);
        t -= sign * (oh * 3600 + om * 60);
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DETAIL: &str = include_str!("../tests/fixtures/detail.json");
    const SEARCH: &str = include_str!("../tests/fixtures/search.json");
    const SERIES: &str = include_str!("../tests/fixtures/timeseries.json");

    fn raw() -> RawDetail {
        serde_json::from_str(DETAIL).expect("detail fixture parses")
    }

    #[test]
    fn search_page_parses() {
        let page: SearchPage = serde_json::from_str(SEARCH).unwrap();
        assert_eq!(page.servers.len(), 1);
        assert_eq!(page.servers[0].id, 160_155);
    }

    #[test]
    fn detail_parses_and_builds() {
        let points: Vec<RawPoint> = serde_json::from_str(SERIES).unwrap();
        let m = build(
            Resolved {
                id: 160_155,
                avg_players_7d: Some(49.9),
            },
            raw(),
            &points,
        );
        assert_eq!(m.url, "https://dayzmetrics.com/server/160155");
        assert_eq!(m.rank_pos, Some(8.0));
        assert_eq!(m.status.as_deref(), Some("online"));
        assert_eq!(m.behavior_verdict.as_deref(), Some("real"));
        assert!(!m.is_fake);
        let r = m.restart.expect("restart schedule");
        assert_eq!(r.period_hours, Some(3.0));
        assert_eq!(r.slots_utc.len(), 8);
        let w = m.wipe.expect("wipe schedule");
        assert_eq!(w.next.as_deref(), Some("2026-10-17"));
        assert_eq!(w.next_source.as_deref(), Some("predicted"));
        assert_eq!(m.player_history.len(), points.len());
        assert_eq!(m.avg_players_7d, Some(49.9));
    }

    #[test]
    fn unknown_or_missing_fields_do_not_fail_the_page() {
        let d: RawDetail =
            serde_json::from_str(r#"{"id":1,"ip":"1.2.3.4","surprise":{"x":1},"links":[{"url":"https://a.b","type":"discord"},"https://c.d"],"fake_reasons":["pad",{"reason":"bots"}]}"#)
                .unwrap();
        let m = build(
            Resolved {
                id: 1,
                avg_players_7d: None,
            },
            d,
            &[],
        );
        assert_eq!(m.links.len(), 2);
        assert_eq!(m.links[0].label, "discord");
        assert_eq!(m.fake_reasons, ["pad", "bots"]);
        assert!(m.restart.is_none());
    }

    #[test]
    fn ports_decide_the_match() {
        let d = raw();
        assert_eq!(fit(&d, "185.207.214.106", 2302, 0), 2);
        assert_eq!(fit(&d, "185.207.214.106", 9999, 2305), 1);
        assert_eq!(fit(&d, "185.207.214.106", 9999, 9999), 0);
        // A search by IP also finds IPs that merely contain it.
        assert_eq!(fit(&d, "85.207.214.106", 2302, 2305), 0);
    }

    #[test]
    fn timestamps_read_as_utc() {
        assert_eq!(unix_secs("2023-11-14T22:13:20+00:00"), Some(1_700_000_000));
        assert_eq!(
            unix_secs("2023-11-14T22:13:20.123456+00:00"),
            Some(1_700_000_000)
        );
        assert_eq!(unix_secs("2023-11-14T22:13:20Z"), Some(1_700_000_000));
        assert_eq!(unix_secs("2023-11-15T00:13:20+02:00"), Some(1_700_000_000));
        assert_eq!(unix_secs("nope"), None);
    }

    /// Against the live site, once, by hand: `cargo test -p dz-dayzmetrics -- --ignored`.
    #[tokio::test]
    #[ignore = "reaches dayzmetrics.com"]
    async fn live_lookup() {
        let client = reqwest::Client::builder()
            .user_agent("DayZ-Community-Hub/test (+https://git.thoxy.xyz/thoxy/dayz-community-hub)")
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .unwrap();
        let (r, m) = lookup(&client, "185.207.214.106", 2302, 2305, None)
            .await
            .unwrap();
        println!(
            "{r:?} {} rank {:?} points {}",
            m.name,
            m.rank_pos,
            m.player_history.len()
        );
        assert_eq!(r.id, m.id);
        assert!(!m.player_history.is_empty());
    }
}
