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

use serde::Deserialize;

/// Steam's ceiling for one request.
pub const WEB_LIST_CAP: usize = 10_000;
/// How deep a capped request is split: one character, then two.
pub const MAX_SPLIT_DEPTH: usize = 2;
/// Excluded starts past which a rest is not split again.
const MAX_EXCLUDED: usize = 400;

const LIST_URL: &str = "https://api.steampowered.com/IGameServersService/GetServerList/v1/";

/// One request: DayZ, the master's own player count, more filters, and,
/// once split, how the names start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPartition {
    /// What Steam's master counts: `Some(true)` no player (`\noplayers\1`),
    /// `Some(false)` players (`\empty\1`, "not empty").
    pub empty: Option<bool>,
    /// More filters, already written (`\map\enoch`).
    pub extra: String,
    /// The start of the names asked for.
    pub prefix: String,
    /// Starts left out: the parts split off before this rest.
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
        let mut name = String::new();
        if !self.prefix.is_empty() {
            name.push_str(&format!("\\name_match\\{}*", self.prefix));
        }
        if !self.excluded.is_empty() {
            name.push_str(&format!("\\nor\\{}", self.excluded.len()));
            for e in &self.excluded {
                name.push_str(&format!("\\name_match\\{e}*"));
            }
        }
        format!("\\appid\\221100{players}{}{name}", self.extra)
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
        let mut starts: Vec<String> = Vec::new();
        for name in names {
            let Some(c) = next_char(name, &self.prefix) else {
                continue;
            };
            // The filter's own syntax cannot be asked for: left to the rest.
            if c == '*' || c == '\\' {
                continue;
            }
            let start = format!("{}{c}", self.prefix);
            if !self.excluded.contains(&start) && !starts.contains(&start) {
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

/// One server as the master lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterAddr {
    pub ip: String,
    pub name: String,
    pub query_port: u16,
    pub game_port: u16,
}

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
}

fn parse(body: &[u8]) -> Result<Vec<MasterAddr>, String> {
    let r: Response =
        serde_json::from_slice(body).map_err(|e| format!("unreadable Steam server list: {e}"))?;
    Ok(r.response
        .servers
        .into_iter()
        .filter_map(|r| {
            let (ip, port) = r.addr.rsplit_once(':')?;
            let query_port: u16 = port.parse().ok().filter(|p| *p != 0)?;
            Some(MasterAddr {
                ip: ip.to_owned(),
                name: r.name,
                query_port,
                game_port: if r.gameport == 0 {
                    query_port
                } else {
                    r.gameport
                },
            })
        })
        .collect())
}

/// One request's servers.
pub async fn fetch_web_partition(
    client: &reqwest::Client,
    key: &str,
    part: &WebPartition,
) -> Result<Vec<MasterAddr>, String> {
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
) -> Result<Vec<MasterAddr>, String> {
    let resp = client
        .get(LIST_URL)
        // The key goes as a query parameter, never into an error message.
        .query(&[("key", key), ("limit", limit), ("filter", filter)])
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "Steam did not answer in time".to_string()
            } else {
                format!("Steam unreachable: {}", e.without_url())
            }
        })?;
    match resp.status().as_u16() {
        200..=299 => {}
        401 | 403 => return Err("Steam rejected the API key".into()),
        429 => return Err("Steam asks to slow down".into()),
        code => return Err(format!("Steam answered HTTP {code}")),
    }
    let body = resp
        .bytes()
        .await
        .map_err(|e| format!("Steam server list cut short: {}", e.without_url()))?;
    tokio::task::spawn_blocking(move || parse(&body))
        .await
        .map_err(|e| format!("server list parse task failed: {e}"))?
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

    #[test]
    fn rows_read_their_ports() {
        let body = br#"{"response":{"servers":[
            {"addr":"1.2.3.4:27016","gameport":2302},
            {"addr":"1.2.3.4:27017"},
            {"addr":"bad"}]}}"#;
        let r = parse(body).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[0].query_port, r[0].game_port), (27016, 2302));
        assert_eq!(r[1].game_port, 27017);
        assert!(parse(br#"{"response":{}}"#).unwrap().is_empty());
    }
}
