//! DayZ's servers from Steam's master through the Web API
//! (`IGameServersService/GetServerList`, which needs a key).
//!
//! Faster than the Steam client's list and not rationed (about two seconds a
//! request, many at once), but capped the same, at 10 000 a request. A
//! request that hits the cap is split by the first letters of the servers'
//! names (`\name_match\a*`, case-insensitive): farms register thousands of
//! servers under a few similar names, so the honest servers they crowd out
//! of a whole map come back letter by letter. Measured on 2026-10-08:
//! empty modded Chernarus servers, one request, 10 000 rows; split by first
//! letter, 58 349 rows, 6 696 of them on addresses with fewer than 20.

use serde::Deserialize;

/// Steam's ceiling for one request.
pub const WEB_LIST_CAP: usize = 10_000;
/// How deep a capped request is split: one letter, then two.
pub const MAX_SPLIT_DEPTH: usize = 2;

const LIST_URL: &str = "https://api.steampowered.com/IGameServersService/GetServerList/v1/";
/// What a name may start with, for the split.
const NAME_STARTS: &str = "abcdefghijklmnopqrstuvwxyz0123456789[(|#-{<!.@";

/// One request: DayZ, the master's own player count, more filters, and a
/// name prefix once split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPartition {
    /// What Steam's master counts: `Some(true)` no player (`\noplayers\1`),
    /// `Some(false)` players (`\empty\1`, "not empty").
    pub empty: Option<bool>,
    /// More filters, already written (`\map\enoch`).
    pub extra: String,
    /// The start of the names asked for, once split.
    pub prefix: String,
}

impl WebPartition {
    pub fn new(empty: Option<bool>, extra: impl Into<String>) -> Self {
        Self {
            empty,
            extra: extra.into(),
            prefix: String::new(),
        }
    }

    /// The `filter` parameter.
    pub fn filter(&self) -> String {
        let players = match self.empty {
            Some(true) => "\\noplayers\\1",
            Some(false) => "\\empty\\1",
            None => "",
        };
        let name = if self.prefix.is_empty() {
            String::new()
        } else {
            format!("\\name_match\\{}*", self.prefix)
        };
        format!("\\appid\\221100{players}{}{name}", self.extra)
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
        if !self.prefix.is_empty() {
            parts.push(format!("{}*", self.prefix));
        }
        parts.join(" · ")
    }

    /// The requests that replace this one when it hit the cap, or none when
    /// it is split as deep as it goes.
    pub fn split(&self) -> Vec<WebPartition> {
        if self.prefix.chars().count() >= MAX_SPLIT_DEPTH {
            return Vec::new();
        }
        // Past the first letter, punctuation is rare enough to leave out.
        let starts: Vec<char> = if self.prefix.is_empty() {
            NAME_STARTS.chars().collect()
        } else {
            "abcdefghijklmnopqrstuvwxyz0123456789 ".chars().collect()
        };
        starts
            .into_iter()
            .map(|c| WebPartition {
                prefix: format!("{}{c}", self.prefix),
                ..self.clone()
            })
            .collect()
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
    let filter = part.filter();
    let resp = client
        .get(LIST_URL)
        // The key goes as a query parameter, never into an error message.
        .query(&[
            ("key", key),
            ("limit", "20000"),
            ("filter", filter.as_str()),
        ])
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
        let split = p.split();
        assert_eq!(
            split[3].filter(),
            "\\appid\\221100\\noplayers\\1\\map\\enoch\\name_match\\d*"
        );
        assert_eq!(split[3].label(), "empty · map enoch · d*");
        let deeper = split[3].split();
        assert_eq!(deeper[0].prefix, "da");
        assert!(deeper[0].split().is_empty(), "two letters deep at most");
        assert_eq!(
            WebPartition::new(Some(false), "").filter(),
            "\\appid\\221100\\empty\\1"
        );
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
