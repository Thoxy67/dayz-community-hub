//! Official DayZ servers, which the launcher list does not carry.
//!
//! The dayzsalauncher.com list has community servers only. The official
//! ones (Bohemia's, named like `4193 | EUROPE - DE`) come from Steam's master
//! server through the Web API, which needs the player's Steam API key.
//!
//! What makes a server official is in its gametags (A2S keywords): an
//! official server sits on the public hive, so it has a `shard…` tag and
//! neither `external` (hosted by someone else) nor `privHive` (its own
//! hive). Community servers that copy the official naming keep those two
//! tags; old-engine servers (0.4x, 0.5x) send no shard at all.

use serde::Deserialize;

use crate::{Endpoint, Server};

/// Steam's master server, filtered to DayZ servers without the community tags.
const STEAM_SERVER_LIST: &str =
    "https://api.steampowered.com/IGameServersService/GetServerList/v1/";
const FILTER: &str = "\\appid\\221100\\nor\\2\\gametype\\external\\gametype\\privHive";

/// Whether a server's gametags say it is official.
pub fn tags_say_official(gametype: &str) -> bool {
    let mut shard = false;
    for tag in gametype.split(',').map(str::trim) {
        if tag.eq_ignore_ascii_case("external") || tag.eq_ignore_ascii_case("privHive") {
            return false;
        }
        shard |= tag.len() > 5 && tag[..5].eq_ignore_ascii_case("shard");
    }
    shard
}

/// Whether a name follows the official pattern: four digits, then " | ".
pub fn name_looks_official(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 7 && b[..4].iter().all(u8::is_ascii_digit) && &b[4..7] == b" | "
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    response: ListInner,
}

#[derive(Debug, Default, Deserialize)]
struct ListInner {
    #[serde(default)]
    servers: Vec<SteamServer>,
}

/// One server as Steam's master lists it.
#[derive(Debug, Default, Deserialize)]
struct SteamServer {
    /// "ip:queryport".
    addr: String,
    #[serde(default)]
    gameport: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    map: String,
    #[serde(default)]
    players: i64,
    #[serde(default)]
    max_players: i64,
    #[serde(default)]
    version: String,
    #[serde(default)]
    secure: bool,
    #[serde(default)]
    os: String,
    #[serde(default)]
    gametype: String,
}

impl SteamServer {
    /// The app's model of this server, if its address reads and it is official.
    fn into_server(self) -> Option<Server> {
        if !tags_say_official(&self.gametype) {
            return None;
        }
        let (ip, port) = self.addr.rsplit_once(':')?;
        let query_port: i64 = port.parse().ok()?;
        let tags: Vec<&str> = self.gametype.split(',').map(str::trim).collect();
        let time = tags
            .iter()
            .rev()
            .find(|t| t.len() == 5 && t.as_bytes()[2] == b':')
            .map(|t| (*t).to_string())
            .unwrap_or_default();
        Some(Server {
            game_port: if self.gameport > 0 {
                self.gameport
            } else {
                query_port
            },
            endpoint: Endpoint {
                ip: ip.to_string(),
                port: query_port,
            },
            name: self.name,
            map: self.map,
            players: self.players,
            max_players: self.max_players,
            environment: if self.os.is_empty() {
                "w".into()
            } else {
                self.os
            },
            // The master does not say; official servers never have one.
            password: false,
            version: self.version,
            vac: self.secure,
            battl_eye: Some(tags.iter().any(|t| t.eq_ignore_ascii_case("battleye"))),
            first_person_only: tags.iter().any(|t| t.eq_ignore_ascii_case("no3rd")),
            time,
            mods: Vec::new(),
            official: true,
        })
    }
}

fn parse(body: &[u8]) -> Result<Vec<Server>, String> {
    let r: ListResponse =
        serde_json::from_slice(body).map_err(|e| format!("unreadable Steam server list: {e}"))?;
    Ok(r.response
        .servers
        .into_iter()
        .filter_map(SteamServer::into_server)
        .collect())
}

/// The official servers, from Steam's master server. Needs a Steam Web API key.
pub async fn fetch_official_servers(
    client: &reqwest::Client,
    key: &str,
) -> Result<Vec<Server>, String> {
    let resp = client
        .get(STEAM_SERVER_LIST)
        .query(&[("key", key), ("limit", "5000"), ("filter", FILTER)])
        .timeout(std::time::Duration::from_secs(20))
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
        code => return Err(format!("Steam answered HTTP {code}")),
    }
    let body = resp
        .bytes()
        .await
        .map_err(|e| format!("Steam server list cut short: {}", e.without_url()))?;
    tokio::task::spawn_blocking(move || parse(&body))
        .await
        .map_err(|e| format!("official list parse task failed: {e}"))?
}

/// Put the official servers into a list: a server already listed at the same
/// query address is marked official, the others are added.
pub fn merge_official(list: &mut Vec<Server>, official: Vec<Server>) {
    let mut at: rustc_hash::FxHashMap<(String, i64), usize> = list
        .iter()
        .enumerate()
        .map(|(i, s)| ((s.endpoint.ip.clone(), s.endpoint.port), i))
        .collect();
    for o in official {
        let key = (o.endpoint.ip.clone(), o.endpoint.port);
        match at.get(&key) {
            Some(&i) => list[i].official = true,
            None => {
                at.insert(key, list.len());
                list.push(o);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_classify_official_and_community() {
        assert!(tags_say_official(
            "battleye,shard000,lqs0,etm5.400000,entm2.170000,20:16"
        ));
        assert!(tags_say_official(
            "battleye,no3rd,shard001,lqs0,etm4.200000,entm4.000000,07:55"
        ));
        // Community, even when named like an official one.
        assert!(!tags_say_official(
            "battleye,external,privHive,shard123ABC,lqs0,etm6.000000,entm4.000000,mod,12:32"
        ));
        assert!(!tags_say_official("battleye,privHive,shard000,lqs0,12:00"));
        // Old-engine servers send the clock only.
        assert!(!tags_say_official("17:06"));
        assert!(!tags_say_official(""));
    }

    #[test]
    fn official_name_pattern() {
        assert!(name_looks_official("4193 | EUROPE - DE"));
        assert!(name_looks_official(
            "1363 | EUROPE - FR | BITTEROOT - VANILLA"
        ));
        assert!(!name_looks_official("DayZ 0.52 by JustaRandom"));
        assert!(!name_looks_official("419 | EU"));
        assert!(!name_looks_official("[EU] 4193 | x"));
    }

    #[test]
    fn steam_list_maps_to_servers() {
        let body = br#"{"response":{"servers":[
            {"addr":"5.62.99.21:11101","gameport":11100,"name":"4193 | EUROPE - DE","map":"enoch","players":9,"max_players":60,"bots":0,"version":"1.29.163709","secure":true,"os":"w","gametype":"battleye,shard000,lqs0,etm5.400000,entm2.170000,20:16"},
            {"addr":"5.62.99.20:10201","gameport":10200,"name":"4121 | EUROPE - DE | 1st Person Only","map":"chernarusplus","players":60,"max_players":60,"version":"1.29.163709","secure":true,"os":"l","gametype":"battleye,no3rd,shard001,lqs0,07:55"},
            {"addr":"134.255.252.219:27018","gameport":2314,"name":"DayZ 0.52","map":"dayzea","gametype":"17:06"},
            {"addr":"bad-address","gametype":"shard000"}
        ]}}"#;
        let servers = parse(body).unwrap();
        assert_eq!(servers.len(), 2);
        let a = &servers[0];
        assert_eq!(
            (a.endpoint.ip.as_str(), a.endpoint.port, a.game_port),
            ("5.62.99.21", 11101, 11100)
        );
        assert!(a.official && a.battl_eye == Some(true) && !a.first_person_only);
        assert_eq!(a.time, "20:16");
        assert_eq!(a.environment, "w");
        let b = &servers[1];
        assert!(b.first_person_only);
        assert_eq!(b.environment, "l");
    }

    #[test]
    fn merge_marks_known_and_adds_new() {
        let mut list = vec![Server {
            endpoint: Endpoint {
                ip: "1.1.1.1".into(),
                port: 27016,
            },
            name: "listed".into(),
            ..Default::default()
        }];
        let official = |ip: &str, port| Server {
            endpoint: Endpoint {
                ip: ip.into(),
                port,
            },
            official: true,
            ..Default::default()
        };
        merge_official(
            &mut list,
            vec![
                official("1.1.1.1", 27016),
                official("2.2.2.2", 27016),
                official("2.2.2.2", 27016),
            ],
        );
        assert_eq!(list.len(), 2);
        assert!(list[0].official && list[0].name == "listed");
        assert_eq!(list[1].endpoint.ip, "2.2.2.2");
    }

    /// Live check against Steam: `DZCH_STEAM_KEY=… cargo test -p dz-api -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_official_list() {
        let Ok(key) = std::env::var("DZCH_STEAM_KEY") else {
            return;
        };
        let servers = fetch_official_servers(&reqwest::Client::new(), &key)
            .await
            .unwrap();
        assert!(
            servers.len() > 50,
            "only {} official servers",
            servers.len()
        );
        assert!(servers.iter().all(|s| s.official));
        assert!(
            servers
                .iter()
                .filter(|s| name_looks_official(&s.name))
                .count()
                > 50
        );
    }
}
