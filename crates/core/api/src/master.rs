//! DayZ's servers from Steam's master through the Web API
//! (`IGameServersService/GetServerList`, which needs a key).
//!
//! Faster than the Steam client's list and not rationed (about two seconds a
//! request, many at once), but capped the same, at 10 000 a request. A
//! request that hits the cap is split by how the servers' names start
//! (`\name_match\a*`): farms register thousands of servers under a few
//! similar names, so the honest servers they crowd out of a whole map come
//! back letter by letter. Measured on 2026-10-08: empty modded Chernarus
//! servers, one request, 10 000 rows; split by first letter, 58 349 rows,
//! 6 696 of them on addresses with fewer than 20.
//!
//! The split takes the characters the capped answer's names actually start
//! with, whatever the script (Latin, Cyrillic, Chinese, emoji, symbols), and
//! adds a *rest*: the same request with those starts excluded
//! (`\nor\N\name_match\a*…`), which holds every name starting otherwise
//! and is split the same way when it is capped too. Checked live: a request,
//! its `b*` part and its rest share no server and miss none, two `\nor`
//! included. `name_match` ignores case for ASCII letters only (`П*` and
//! `п*` differ), so only those are folded.
//!
//! It is rationed all the same, and says so badly: past a burst it answers
//! every request `200` with no servers, exactly as for a filter that
//! matches nothing. [`web_throttled`] tells the two apart.

use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};

/// Steam's ceiling for one request.
pub const WEB_LIST_CAP: usize = 10_000;
/// How deep a capped request is split: one character, then two.
pub const MAX_SPLIT_DEPTH: usize = 2;
/// Excluded starts past which a rest is not split again.
const MAX_EXCLUDED: usize = 400;

const LIST_URL: &str = "https://api.steampowered.com/IGameServersService/GetServerList/v1/";

/// One request: DayZ, the master's own player count, more filters, and,
/// once split, how the names start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebPartition {
    /// What Steam's master counts: `Some(true)` no player (`\noplayers\1`),
    /// `Some(false)` players (`\empty\1`, "not empty").
    pub empty: Option<bool>,
    /// More filters, already written (`\map\enoch`).
    #[serde(default)]
    pub extra: String,
    /// The start of the names asked for.
    #[serde(default)]
    pub prefix: String,
    /// Starts left out: the parts split off before this rest.
    #[serde(default)]
    pub excluded: Vec<String>,
}

/// The character as `name_match` compares it: ASCII letters fold.
fn fold(c: char) -> char {
    c.to_ascii_lowercase()
}

/// `name`'s character after `prefix`, if `name` starts with it.
fn next_char(name: &str, prefix: &str) -> Option<char> {
    let mut chars = name.chars();
    for p in prefix.chars() {
        if fold(chars.next()?) != fold(p) {
            return None;
        }
    }
    chars.next().map(fold)
}

impl WebPartition {
    pub fn new(empty: Option<bool>, extra: impl Into<String>) -> Self {
        Self {
            empty,
            extra: extra.into(),
            prefix: String::new(),
            excluded: Vec::new(),
        }
    }

    /// The `filter` parameter.
    pub fn filter(&self) -> String {
        let players = match self.empty {
            Some(true) => "\\noplayers\\1",
            Some(false) => "\\empty\\1",
            None => "",
        };
        use std::fmt::Write;
        let mut out = format!("\\appid\\221100{players}{}", self.extra);
        if !self.prefix.is_empty() {
            let _ = write!(out, "\\name_match\\{}*", self.prefix);
        }
        if !self.excluded.is_empty() {
            let _ = write!(out, "\\nor\\{}", self.excluded.len());
            for e in &self.excluded {
                let _ = write!(out, "\\name_match\\{e}*");
            }
        }
        out
    }

    /// The same request as the Steam client's filters (`dz_steamworks`),
    /// which take the same keys in order, but `hasplayers` for "not empty".
    pub fn pairs(&self) -> Vec<(String, String)> {
        let filter = self.filter();
        let tokens: Vec<&str> = filter.split('\\').skip(1).collect();
        tokens
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|[k, _]| *k != "appid")
            .map(|[k, v]| match *k {
                "empty" => ("hasplayers".to_owned(), "1".to_owned()),
                k => (k.to_owned(), (*v).to_owned()),
            })
            .collect()
    }

    /// Short words for the progress line: "empty · map enoch · d*".
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        match self.empty {
            Some(true) => parts.push("empty".to_owned()),
            Some(false) => parts.push("with players".to_owned()),
            None => {}
        }
        if self.extra.starts_with("\\nor\\") {
            parts.push("other maps".to_owned());
        } else if !self.extra.is_empty() {
            parts.push(self.extra.trim_start_matches('\\').replace('\\', " "));
        }
        match (self.prefix.is_empty(), self.excluded.is_empty()) {
            (true, true) => {}
            (false, true) => parts.push(format!("{}*", self.prefix)),
            (true, false) => parts.push("rest".to_owned()),
            (false, false) => parts.push(format!("{}* rest", self.prefix)),
        }
        parts.join(" · ")
    }

    /// The requests that replace this one when it hit the cap, from the
    /// `names` it answered: one per character those names go on with after
    /// the prefix, and the rest. None when split as deep as it goes.
    pub fn split<'a>(&self, names: impl IntoIterator<Item = &'a str>) -> Vec<WebPartition> {
        if self.prefix.chars().count() >= MAX_SPLIT_DEPTH || self.excluded.len() >= MAX_EXCLUDED {
            return Vec::new();
        }
        // Ten thousand names, a handful of starts: each character is looked
        // at once, and a start is written out only the first time it comes.
        let mut seen: FxHashSet<char> = FxHashSet::default();
        let excluded: FxHashSet<&str> = self.excluded.iter().map(String::as_str).collect();
        let mut starts: Vec<String> = Vec::new();
        for name in names {
            let Some(c) = next_char(name, &self.prefix) else {
                continue;
            };
            // The filter's own syntax cannot be asked for: left to the rest.
            if c == '*' || c == '\\' || !seen.insert(c) {
                continue;
            }
            let mut start = String::with_capacity(self.prefix.len() + c.len_utf8());
            start.push_str(&self.prefix);
            start.push(c);
            if !excluded.contains(start.as_str()) {
                starts.push(start);
            }
        }
        if starts.is_empty() {
            return Vec::new();
        }
        let mut parts: Vec<WebPartition> = starts
            .iter()
            .map(|start| WebPartition {
                prefix: start.clone(),
                excluded: Vec::new(),
                ..self.clone()
            })
            .collect();
        let mut excluded = self.excluded.clone();
        excluded.extend(starts);
        parts.push(WebPartition {
            excluded,
            ..self.clone()
        });
        parts
    }
}

/// The partitions of a full listing: servers with players, empty ones on
/// each of `maps`, and empty ones on every other map.
pub fn web_partitions(maps: &[String]) -> Vec<WebPartition> {
    let mut parts = vec![WebPartition::new(Some(false), "")];
    parts.extend(
        maps.iter()
            .map(|m| WebPartition::new(Some(true), format!("\\map\\{m}"))),
    );
    if !maps.is_empty() {
        let nor: String = maps.iter().map(|m| format!("\\map\\{m}")).collect();
        parts.push(WebPartition::new(
            Some(true),
            format!("\\nor\\{}{nor}", maps.len()),
        ));
    }
    parts
}

// ── the plan a listing leaves ──────────────────────────────────────────

/// Below this many servers, parts of one request are asked as one again: a
/// fifth of the cap left for the master to grow before the next listing
/// (which splits again what overflows).
pub const PLAN_MARGIN: usize = WEB_LIST_CAP * 4 / 5;

/// One request a listing asked that did not need splitting, and how many
/// servers it answered: what the next listing asks straight away rather than
/// hitting the cap again to find the same splits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedPart {
    pub part: WebPartition,
    #[serde(default)]
    pub rows: u32,
}

impl WebPartition {
    /// Whether `other` was split from the same request of a listing (the
    /// same player filter and filters).
    pub fn same_base(&self, other: &WebPartition) -> bool {
        self.empty == other.empty && self.extra == other.extra
    }

    /// This request before any split.
    pub fn base(&self) -> WebPartition {
        WebPartition::new(self.empty, self.extra.clone())
    }
}

/// Whether `parts`, all split from one request, are every name exactly
/// once below `prefix`: one part asks for `prefix` (the whole of it, or
/// its rest), and each start that rest leaves out is itself such a set one
/// character deeper. A plan read from disk is checked so: one that does
/// not hold is not used (the request is asked whole and split again).
fn covers(parts: &[&WebPartition], prefix: &str, depth: usize) -> bool {
    if depth > MAX_SPLIT_DEPTH {
        return false;
    }
    let mut level = parts.iter().filter(|p| p.prefix == prefix);
    let (Some(rest), None) = (level.next(), level.next()) else {
        return false;
    };
    let mut accounted = 1;
    for start in &rest.excluded {
        // One character deeper than `prefix`, and not asked twice.
        let deeper = start
            .strip_prefix(prefix)
            .is_some_and(|c| c.chars().count() == 1);
        if !deeper || rest.excluded.iter().filter(|e| *e == start).count() > 1 {
            return false;
        }
        let below: Vec<&WebPartition> = parts
            .iter()
            .copied()
            .filter(|p| p.prefix.starts_with(start.as_str()))
            .collect();
        if !covers(&below, start, depth + 1) {
            return false;
        }
        accounted += below.len();
    }
    // Nothing else below `prefix`: a part under no excluded start but its
    // own is asked twice or for nothing.
    accounted == parts.len()
}

/// The requests of a listing over `bases` ([`web_partitions`]), with the
/// splits an earlier listing found: a base the plan has a sound set of
/// parts for is asked as those parts, any other whole. Saved parts of a
/// base no longer asked are dropped.
pub fn planned_partitions(bases: Vec<WebPartition>, plan: &[PlannedPart]) -> Vec<WebPartition> {
    let mut out = Vec::with_capacity(bases.len().max(plan.len()));
    for base in bases {
        let parts: Vec<&WebPartition> = plan
            .iter()
            .map(|p| &p.part)
            .filter(|p| p.same_base(&base))
            .collect();
        if !parts.is_empty() && covers(&parts, "", 0) {
            out.extend(parts.into_iter().cloned());
        } else {
            out.push(base);
        }
    }
    out
}

/// The plan to keep from what a listing asked (`done`: every request that
/// did not need splitting, with its servers), with as few requests as the
/// cap allows, each kept under [`PLAN_MARGIN`]:
///
/// - the parts of a start (or of a whole base) that fit in one request
///   together become that request;
/// - otherwise its small starts go back into its rest (a rest leaves out
///   only the starts asked apart), the biggest kept apart until the rest
///   fits, and each of those is planned the same way one character deeper.
///
/// A plan only ever saves requests, never servers: the parts still cover
/// every name exactly once, and a part that has grown is split by the next
/// listing as it would be anyway. Parts of one base whose set does not
/// hold together are left as they were.
pub fn compact_plan(done: Vec<PlannedPart>) -> Vec<PlannedPart> {
    // Bases in the order they were first asked.
    let mut order: Vec<WebPartition> = Vec::new();
    let mut by_base: FxHashMap<(Option<bool>, String), Vec<PlannedPart>> = FxHashMap::default();
    for p in done {
        let key = (p.part.empty, p.part.extra.clone());
        if !by_base.contains_key(&key) {
            order.push(p.part.base());
        }
        by_base.entry(key).or_default().push(p);
    }
    let mut out = Vec::new();
    for base in order {
        let Some(parts) = by_base.remove(&(base.empty, base.extra.clone())) else {
            continue;
        };
        let refs: Vec<&WebPartition> = parts.iter().map(|p| &p.part).collect();
        if covers(&refs, "", 0) {
            let all: Vec<&PlannedPart> = parts.iter().collect();
            compact_below(&base, &all, "", &mut out);
        } else {
            out.extend(parts);
        }
    }
    out
}

fn rows_of(parts: &[&PlannedPart]) -> u64 {
    parts.iter().map(|p| u64::from(p.rows)).sum()
}

/// [`compact_plan`] for the `parts` below `prefix` (a set [`covers`] holds
/// for), into `out`.
fn compact_below(
    base: &WebPartition,
    parts: &[&PlannedPart],
    prefix: &str,
    out: &mut Vec<PlannedPart>,
) {
    let total = rows_of(parts);
    let Some(rest) = parts.iter().find(|p| p.part.prefix == prefix) else {
        return;
    };
    if total < PLAN_MARGIN as u64 || rest.part.excluded.is_empty() {
        out.push(PlannedPart {
            part: WebPartition {
                prefix: prefix.to_owned(),
                excluded: Vec::new(),
                ..base.clone()
            },
            rows: u32::try_from(total).unwrap_or(u32::MAX),
        });
        return;
    }
    // Each start the rest leaves out, with its parts and their servers.
    let mut starts: Vec<(&str, Vec<&PlannedPart>, u64)> = rest
        .part
        .excluded
        .iter()
        .map(|e| {
            let below: Vec<&PlannedPart> = parts
                .iter()
                .copied()
                .filter(|p| p.part.prefix.starts_with(e.as_str()))
                .collect();
            let n = rows_of(&below);
            (e.as_str(), below, n)
        })
        .collect();
    // The biggest apart until what is left fits in the rest.
    starts.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(b.0)));
    let mut left = total;
    let mut apart = Vec::new();
    for (start, below, n) in starts {
        if left < PLAN_MARGIN as u64 {
            break;
        }
        left -= n;
        apart.push((start, below));
    }
    for (start, below) in &apart {
        compact_below(base, below, start, out);
    }
    out.push(PlannedPart {
        part: WebPartition {
            prefix: prefix.to_owned(),
            excluded: apart.iter().map(|(s, _)| (*s).to_owned()).collect(),
            ..base.clone()
        },
        rows: u32::try_from(left).unwrap_or(u32::MAX),
    });
}

/// One server as the master lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterAddr {
    pub ip: String,
    pub name: String,
    pub query_port: u16,
    pub game_port: u16,
    /// The bots its last heartbeat counted. DayZ has none: a server
    /// reporting some is a farm's emulator padding its count.
    pub bots: u8,
}

/// Why a request to the master failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebError {
    /// The key was refused (401, 403).
    Key,
    /// Asked to slow down (429), for as long as `Retry-After` says if it does.
    RateLimited(Option<std::time::Duration>),
    /// No answer in time.
    Timeout,
    /// Steam could not be reached.
    Unreachable(String),
    /// Any other HTTP status.
    Http(u16),
    /// An answer cut short or unreadable.
    Body(String),
}

impl WebError {
    /// Worth asking again after a short wait: a lost or garbled answer, or a
    /// server-side error, not a refused key nor a request to slow down.
    pub fn transient(&self) -> bool {
        match self {
            Self::Timeout | Self::Unreachable(_) | Self::Body(_) => true,
            Self::Http(code) => *code >= 500,
            Self::Key | Self::RateLimited(_) => false,
        }
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key => f.write_str("Steam rejected the API key"),
            Self::RateLimited(_) => f.write_str("Steam asks to slow down"),
            Self::Timeout => f.write_str("Steam did not answer in time"),
            Self::Unreachable(e) => write!(f, "Steam unreachable: {e}"),
            Self::Http(code) => write!(f, "Steam answered HTTP {code}"),
            Self::Body(e) => f.write_str(e),
        }
    }
}

impl std::error::Error for WebError {}

#[derive(Deserialize)]
struct Response {
    response: Inner,
}
#[derive(Deserialize, Default)]
struct Inner {
    #[serde(default)]
    servers: Vec<Row>,
}
#[derive(Deserialize)]
struct Row {
    addr: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    gameport: u16,
    #[serde(default)]
    bots: u8,
}

fn parse(body: &[u8]) -> Result<Vec<MasterAddr>, WebError> {
    let r: Response = serde_json::from_slice(body)
        .map_err(|e| WebError::Body(format!("unreadable Steam server list: {e}")))?;
    Ok(r.response
        .servers
        .into_iter()
        .filter_map(|r| {
            let (ip, port) = r.addr.rsplit_once(':')?;
            let query_port: u16 = port.parse().ok().filter(|p| *p != 0)?;
            // The address's own string, cut at the colon: no second
            // allocation per row of up to twenty thousand.
            let ip_len = ip.len();
            let mut ip = r.addr;
            ip.truncate(ip_len);
            Some(MasterAddr {
                ip,
                name: r.name,
                query_port,
                game_port: if r.gameport == 0 {
                    query_port
                } else {
                    r.gameport
                },
                bots: r.bots,
            })
        })
        .collect())
}

/// One request's servers.
pub async fn fetch_web_partition(
    client: &reqwest::Client,
    key: &str,
    part: &WebPartition,
) -> Result<Vec<MasterAddr>, WebError> {
    fetch(client, key, &part.filter(), "20000").await
}

/// Whether the master is withholding answers: asked for one server with
/// players, of which there are always thousands, it gives none.
pub async fn web_throttled(client: &reqwest::Client, key: &str) -> bool {
    fetch(client, key, r"\appid\221100\empty\1", "1")
        .await
        .map_or(true, |rows| rows.is_empty())
}

async fn fetch(
    client: &reqwest::Client,
    key: &str,
    filter: &str,
    limit: &str,
) -> Result<Vec<MasterAddr>, WebError> {
    let resp = client
        .get(LIST_URL)
        // The key goes as a query parameter, never into an error message.
        .query(&[("key", key), ("limit", limit), ("filter", filter)])
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                WebError::Timeout
            } else {
                WebError::Unreachable(e.without_url().to_string())
            }
        })?;
    match resp.status().as_u16() {
        200..=299 => {}
        401 | 403 => return Err(WebError::Key),
        429 => return Err(WebError::RateLimited(retry_after(resp.headers()))),
        code => return Err(WebError::Http(code)),
    }
    let body = resp
        .bytes()
        .await
        .map_err(|e| WebError::Body(format!("Steam server list cut short: {}", e.without_url())))?;
    tokio::task::spawn_blocking(move || parse(&body))
        .await
        .map_err(|e| WebError::Body(format!("server list parse task failed: {e}")))?
}

/// `Retry-After` in seconds (the form Steam would use; a date is ignored).
fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<std::time::Duration> {
    let secs: u64 = headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(std::time::Duration::from_secs(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_are_written_as_the_master_reads_them() {
        let p = WebPartition::new(Some(true), "\\map\\enoch");
        assert_eq!(p.filter(), "\\appid\\221100\\noplayers\\1\\map\\enoch");
        assert_eq!(
            WebPartition::new(Some(false), "").filter(),
            "\\appid\\221100\\empty\\1"
        );
    }

    #[test]
    fn a_capped_request_splits_by_the_starts_its_names_have_and_a_rest() {
        let p = WebPartition::new(Some(true), "\\map\\enoch");
        let split = p.split(["Dark", "dusk", "Попробуй", "*star", "🔥 Fire"]);
        let prefixes: Vec<&str> = split.iter().map(|s| s.prefix.as_str()).collect();
        // ASCII folds; Cyrillic and emoji stay as they are; `*` goes to the rest.
        assert_eq!(prefixes, ["d", "П", "🔥", ""]);
        assert_eq!(
            split[0].filter(),
            "\\appid\\221100\\noplayers\\1\\map\\enoch\\name_match\\d*"
        );
        let rest = &split[3];
        assert_eq!(
            rest.filter(),
            "\\appid\\221100\\noplayers\\1\\map\\enoch\\nor\\3\\name_match\\d*\\name_match\\П*\\name_match\\🔥*"
        );
        assert_eq!(rest.label(), "empty · map enoch · rest");
        // The rest splits again by what it answers, never by what it excludes.
        let again = rest.split(["dawn", "=FARM=", "Ёлка"]);
        let prefixes: Vec<&str> = again.iter().map(|s| s.prefix.as_str()).collect();
        assert_eq!(prefixes, ["=", "Ё", ""]);
        assert_eq!(again[2].excluded.len(), 5);
    }

    #[test]
    fn a_request_reads_as_the_clients_filters() {
        let p = WebPartition::new(Some(true), "\\nor\\1\\map\\enoch");
        let rest = &p.split(["Dark"])[1];
        let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
        assert_eq!(
            rest.pairs(),
            [
                pair("noplayers", "1"),
                pair("nor", "1"),
                pair("map", "enoch"),
                pair("nor", "1"),
                pair("name_match", "d*"),
            ]
        );
        assert_eq!(
            WebPartition::new(Some(false), "").pairs(),
            [pair("hasplayers", "1")]
        );
    }

    #[test]
    fn a_letter_splits_by_the_next_character_then_stops() {
        let d = WebPartition::new(Some(true), "").split(["Dark"]).remove(0);
        let split = d.split(["Dark", "DAWN", "Dusk", "Other"]);
        let prefixes: Vec<&str> = split.iter().map(|s| s.prefix.as_str()).collect();
        assert_eq!(prefixes, ["da", "du", "d"]);
        assert_eq!(split[2].label(), "empty · d* rest");
        assert!(
            split[0].split(["Dark"]).is_empty(),
            "two characters deep at most"
        );
        assert!(d.split(["Other"]).is_empty(), "nothing to split by");
    }

    #[test]
    fn a_listing_ends_with_every_other_map() {
        let p = web_partitions(&["enoch".into(), "banov".into()]);
        assert_eq!(p.len(), 4);
        assert_eq!(p[3].extra, "\\nor\\2\\map\\enoch\\map\\banov");
        assert_eq!(p[3].label(), "empty · other maps");
    }

    fn done(part: &WebPartition, rows: u32) -> PlannedPart {
        PlannedPart {
            part: part.clone(),
            rows,
        }
    }

    /// A base split twice: `d` split again into `da` and its rest.
    fn split_twice() -> (WebPartition, Vec<WebPartition>) {
        let base = WebPartition::new(Some(true), "\\map\\enoch");
        let top = base.split(["Dark", "Fire", "Alpha"]);
        let d = top[0].split(["Dark", "Dawn"]);
        let mut leaves: Vec<WebPartition> = d;
        leaves.extend(top[1..].iter().cloned());
        (base, leaves)
    }

    #[test]
    fn a_saved_plan_is_asked_as_it_was_split() {
        let (base, leaves) = split_twice();
        let plan: Vec<PlannedPart> = leaves.iter().map(|p| done(p, 9_000)).collect();
        let other = WebPartition::new(Some(false), "");
        let asked = planned_partitions(vec![other.clone(), base], &plan);
        assert_eq!(asked[0], other, "a base the plan lacks is asked whole");
        assert_eq!(&asked[1..], &leaves[..]);
        // Round trip through the disk.
        let json = serde_json::to_string(&plan).unwrap();
        let back: Vec<PlannedPart> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, plan);
    }

    #[test]
    fn a_plan_that_misses_or_repeats_a_part_is_not_used() {
        let (base, leaves) = split_twice();
        let plan = |parts: &[WebPartition]| -> Vec<PlannedPart> {
            parts.iter().map(|p| done(p, 9_000)).collect()
        };
        for broken in [
            leaves[1..].to_vec(),
            [leaves.clone(), vec![leaves[0].clone()]].concat(),
            leaves[..leaves.len() - 1].to_vec(),
            vec![WebPartition {
                prefix: "abc".into(),
                ..base.clone()
            }],
        ] {
            assert_eq!(
                planned_partitions(vec![base.clone()], &plan(&broken)),
                vec![base.clone()]
            );
        }
    }

    #[test]
    fn parts_that_fit_in_one_request_again_are_merged() {
        let (base, leaves) = split_twice();
        // Everything fits: the base alone.
        let small: Vec<PlannedPart> = leaves.iter().map(|p| done(p, 100)).collect();
        assert_eq!(compact_plan(small), vec![done(&base, 500)]);
        // Only `a` is big: the other starts go back into the rest.
        let rows = [100, 100, 100, 9_000, 500];
        let mixed: Vec<PlannedPart> = leaves.iter().zip(rows).map(|(p, n)| done(p, n)).collect();
        let kept = compact_plan(mixed);
        let shape: Vec<(&str, Vec<&str>, u32)> = kept
            .iter()
            .map(|p| {
                let ex = p.part.excluded.iter().map(String::as_str).collect();
                (p.part.prefix.as_str(), ex, p.rows)
            })
            .collect();
        assert_eq!(shape, [("a", vec![], 9_000), ("", vec!["a"], 800)]);
        // Still a sound plan.
        let asked = planned_partitions(vec![base.clone()], &kept);
        assert_eq!(asked.len(), 2);
        // Big starts stay apart, `d` planned a level deeper; `f` fits in the rest.
        let rows = [9_000, 100, 7_000, 9_000, 500];
        let big: Vec<PlannedPart> = leaves.iter().zip(rows).map(|(p, n)| done(p, n)).collect();
        let kept = compact_plan(big);
        let shape: Vec<(&str, usize)> = kept
            .iter()
            .map(|p| (p.part.prefix.as_str(), p.part.excluded.len()))
            .collect();
        assert_eq!(shape, [("da", 0), ("d", 1), ("a", 0), ("", 2)]);
        assert_eq!(planned_partitions(vec![base], &kept).len(), 4);
        // A set that does not hold is kept as it came.
        let broken = vec![done(&leaves[0], 5)];
        assert_eq!(compact_plan(broken.clone()), broken);
    }

    #[test]
    fn retry_after_reads_seconds_only() {
        let mut h = reqwest::header::HeaderMap::new();
        assert_eq!(retry_after(&h), None);
        h.insert(reqwest::header::RETRY_AFTER, "30".parse().unwrap());
        assert_eq!(retry_after(&h), Some(std::time::Duration::from_secs(30)));
        h.insert(
            reqwest::header::RETRY_AFTER,
            "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(retry_after(&h), None);
        assert!(!WebError::RateLimited(None).transient());
        assert!(!WebError::Key.transient());
        assert!(WebError::Http(502).transient() && !WebError::Http(404).transient());
    }

    #[test]
    fn rows_read_their_ports() {
        let body = br#"{"response":{"servers":[
            {"addr":"1.2.3.4:27016","gameport":2302,"bots":12,"players":12},
            {"addr":"1.2.3.4:27017"},
            {"addr":"bad"}]}}"#;
        let r = parse(body).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].query_port, r[0].game_port), (27016, 2302));
        assert_eq!((r[0].bots, r[1].bots), (12, 0));
        assert_eq!(r[1].game_port, 27017);
        assert!(parse(br#"{"response":{}}"#).unwrap().is_empty());
    }
}
