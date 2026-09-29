//! The public server list (dayzsalauncher.com), the official servers (from
//! Steam's master server), the on-disk cache, and the Steam player count.

mod index;
mod official;

pub use index::ServerIndex;
pub use official::{
    fetch_official_servers, merge_official, name_looks_official, tags_say_official,
};

use dz_common::{Error, Result};
use serde::{Deserialize, Serialize};

/// The server list as the launcher API returns it.
///
/// Only the fields the app reads are kept: serde ignores the rest, which
/// saves parsing and holding a dozen unused strings for each of the ~18 000
/// servers.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerList {
    pub status: i64,
    pub result: Vec<Server>,
}

/// One server of the list.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub game_port: i64,
    /// The A2S query endpoint (its port is the query port).
    pub endpoint: Endpoint,
    pub name: String,
    pub map: String,
    pub players: i64,
    pub max_players: i64,
    /// "w" for Windows, "l" for Linux.
    pub environment: String,
    pub password: bool,
    pub version: String,
    pub vac: bool,
    pub battl_eye: Option<bool>,
    pub first_person_only: bool,
    /// In-game time, "HH:MM".
    pub time: String,
    pub mods: Vec<Mod>,
    /// Bohemia's own server, on the public hive (see `official`). The
    /// launcher list never says so; set when the official list is merged in.
    #[serde(default)]
    pub official: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub ip: String,
    pub port: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mod {
    pub name: String,
    pub steam_workshop_id: i64,
}

impl Server {
    /// A server known only by its address, for a direct connect to a server
    /// that is not in the list.
    pub fn unlisted(ip: &str, game_port: i64) -> Self {
        Self {
            endpoint: Endpoint {
                ip: ip.to_owned(),
                port: game_port,
            },
            name: format!("{ip}:{game_port}"),
            game_port,
            ..Default::default()
        }
    }

    /// Workshop mod IDs as `u64`.
    pub fn mod_ids(&self) -> Vec<u64> {
        self.mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect()
    }

    /// The query address, "ip:port".
    pub fn query_addr(&self) -> String {
        format!("{}:{}", self.endpoint.ip, self.endpoint.port)
    }
}

/// Keep the first server for each (ip, query port) and drop the repeats the
/// API sometimes lists.
pub fn dedup_servers(mut servers: Vec<Server>) -> Vec<Server> {
    let keep: Vec<bool> = {
        let mut seen: rustc_hash::FxHashSet<(&str, i64)> =
            rustc_hash::FxHashSet::with_capacity_and_hasher(servers.len(), Default::default());
        servers
            .iter()
            .map(|s| seen.insert((s.endpoint.ip.as_str(), s.endpoint.port)))
            .collect()
    };
    let mut keep = keep.into_iter();
    servers.retain(|_| keep.next().unwrap_or(false));
    servers
}

/// Response from the Steam GetNumberOfCurrentPlayers endpoint.
#[derive(Debug, Deserialize)]
struct SteamPlayerCountResponse {
    response: SteamPlayerCountInner,
}

#[derive(Debug, Deserialize)]
struct SteamPlayerCountInner {
    player_count: Option<u32>,
}

/// The number of players currently in DayZ (appid 221100) on Steam.
pub async fn fetch_steam_player_count(client: &reqwest::Client) -> Result<u32> {
    let resp = client
        .get("https://api.steampowered.com/ISteamUserStats/GetNumberOfCurrentPlayers/v1/")
        .query(&[("appid", "221100")])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .json::<SteamPlayerCountResponse>()
        .await?;
    Ok(resp.response.player_count.unwrap_or(0))
}

/// On-disk cache envelope for the server list.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerListCache {
    /// Unix timestamp (seconds) when the list was fetched.
    pub fetched_at: u64,
    pub list: ServerList,
}

/// Load the cached server list. `None` if the file is missing, unreadable, or
/// fails to parse. The file is multi-MB JSON, so it is read asynchronously and
/// parsed on the blocking pool.
pub async fn load_server_list_cache(cache_path: &std::path::Path) -> Option<ServerListCache> {
    let data = tokio::fs::read(cache_path).await.ok()?;
    tokio::task::spawn_blocking(move || serde_json::from_slice(&data).ok())
        .await
        .ok()
        .flatten()
}

/// Serialization-only view of the cache that borrows the list instead of cloning it.
#[derive(Serialize)]
struct ServerListCacheRef<'a> {
    fetched_at: u64,
    list: &'a ServerList,
}

/// Persist a freshly-fetched server list. Serialized from a reference (no
/// clone of the list) on the blocking pool, then written asynchronously.
/// Failures are ignored: the cache only speeds up the next start.
pub async fn save_server_list_cache(
    cache_path: &std::path::Path,
    list: std::sync::Arc<ServerList>,
) {
    let fetched_at = dz_common::time::now_secs();
    let data = tokio::task::spawn_blocking(move || {
        serde_json::to_vec(&ServerListCacheRef {
            fetched_at,
            list: &list,
        })
    })
    .await;
    if let Ok(Ok(data)) = data {
        if let Some(parent) = cache_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(cache_path, data).await;
    }
}

/// Fetch the full server list from the DayZSA Launcher API.
///
/// The response is multi-MB JSON: the bytes are read off the socket, then
/// deserialized on the blocking pool so a refresh does not stall the runtime.
pub async fn fetch_servers(client: &reqwest::Client) -> Result<ServerList> {
    let bytes = client
        .get("https://dayzsalauncher.com/api/v1/launcher/servers/dayz")
        // Generous: the list is several MB, but it must not hang startup.
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let list = tokio::task::spawn_blocking(move || serde_json::from_slice::<ServerList>(&bytes))
        .await
        .map_err(|e| Error::Other(format!("server list parse task failed: {e}")))?
        .map_err(|e| Error::Other(format!("the server list could not be read: {e}")))?;
    if list.result.is_empty() {
        return Err(Error::Other("the launcher API returned no servers".into()));
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(ip: &str, qport: i64, gport: i64, name: &str) -> Server {
        Server {
            endpoint: Endpoint {
                ip: ip.into(),
                port: qport,
            },
            game_port: gport,
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn dedup_keeps_first_occurrence() {
        let list = vec![
            server("1.1.1.1", 27016, 2302, "a"),
            server("1.1.1.1", 27016, 2302, "b"),
            server("1.1.1.1", 27017, 2402, "c"),
        ];
        let out = dedup_servers(list);
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{"status":0,"result":[{"gamePort":2302,"sponsor":true,"endpoint":{"ip":"1.2.3.4","port":27016},"name":"n","map":"chernarusplus","players":1,"maxPlayers":60,"environment":"w","password":false,"version":"1.28","vac":true,"battlEye":true,"firstPersonOnly":false,"time":"12:00","mods":[],"shard":"x"}]}"#;
        let list: ServerList = serde_json::from_str(json).unwrap();
        assert_eq!(list.result[0].endpoint.port, 27016);
    }
}
