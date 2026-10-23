//! The public server list (dayzsalauncher.com), the official servers (from
//! Steam's master server), the servers Steam's client lists, the on-disk
//! cache, and the Steam player count.

mod addr;
mod index;
mod master;
mod official;
mod steam;

pub use addr::{Ipv4Addr, NO_ADDR, SocketAddrV4, addr, is_addressed, lenient_ip, nonzero_port};
pub use index::ServerIndex;
pub use master::{
    MAX_SPLIT_DEPTH, MasterAddr, PLAN_MARGIN, PlannedPart, WEB_LIST_CAP, WebError, WebPartition,
    compact_plan, fetch_web_partition, planned_partitions, web_partitions, web_throttled,
};
pub use official::{
    fetch_official_servers, merge_official, name_looks_official, tags_say_official,
};
pub use steam::{FARM_ADDRESS, FarmCounter, SteamListed, SteamRow, merge_steam, merged_steam};

use dz_common::{Error, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::sync::Arc;

/// The server list as the launcher API returns it.
///
/// Only the fields the app reads are kept: serde ignores the rest, which
/// saves parsing and holding a dozen unused strings for each of the ~18 000
/// servers.
///
/// Each server sits behind an `Arc`: a list put together from another (the
/// Steam servers merged in, a refresh keeping what it lost) shares the
/// servers instead of copying every string of every one of them.
///
/// A server without an IPv4 query address (see `addr`) is left out as the
/// list is read, the launcher's and the cache's alike.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "ReadList")]
pub struct ServerList {
    pub status: i64,
    pub result: Vec<Arc<Server>>,
}

/// [`ServerList`] as read, before the servers without an address leave.
#[derive(Deserialize)]
struct ReadList {
    #[serde(default)]
    status: i64,
    #[serde(default)]
    result: Vec<Arc<Server>>,
}

impl From<ReadList> for ServerList {
    fn from(mut read: ReadList) -> Self {
        let before = read.result.len();
        read.result.retain(|s| is_addressed(&s.endpoint));
        let left_out = before - read.result.len();
        if left_out > 0 {
            eprintln!("server list: {left_out} servers without an IPv4 query address left out");
        }
        Self {
            status: read.status,
            result: read.result,
        }
    }
}

/// One server of the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    /// The port to join on, 0 when unknown.
    #[serde(deserialize_with = "addr::port_or_zero")]
    pub game_port: u16,
    /// The A2S query address (its port is the query port), written as the
    /// list has it: `{"ip": "1.2.3.4", "port": 27016}`.
    #[serde(with = "addr::endpoint")]
    pub endpoint: SocketAddrV4,
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
pub struct Mod {
    pub name: String,
    pub steam_workshop_id: i64,
}

impl Default for Server {
    /// No server: no address, nothing known.
    fn default() -> Self {
        Self {
            game_port: 0,
            endpoint: NO_ADDR,
            name: String::new(),
            map: String::new(),
            players: 0,
            max_players: 0,
            environment: String::new(),
            password: false,
            version: String::new(),
            vac: false,
            battl_eye: None,
            first_person_only: false,
            time: String::new(),
            mods: Vec::new(),
            official: false,
        }
    }
}

impl Server {
    /// A server known only by its address to join, for a direct connect to
    /// a server that is not in the list.
    pub fn unlisted(join: SocketAddrV4) -> Self {
        Self {
            endpoint: join,
            name: join.to_string(),
            game_port: join.port(),
            ..Default::default()
        }
    }

    /// The query address's IP.
    pub fn ip(&self) -> Ipv4Addr {
        *self.endpoint.ip()
    }

    /// The query port.
    pub fn query_port(&self) -> u16 {
        self.endpoint.port()
    }

    /// The address to join (ip, game port), when the game port is known.
    pub fn join_addr(&self) -> Option<SocketAddrV4> {
        (self.game_port != 0).then(|| SocketAddrV4::new(self.ip(), self.game_port))
    }

    /// Workshop mod IDs as `u64`.
    pub fn mod_ids(&self) -> Vec<u64> {
        self.mods
            .iter()
            .map(|m| m.steam_workshop_id as u64)
            .collect()
    }
}

/// Keep the first server for each (ip, query port) and drop the repeats the
/// API sometimes lists.
pub fn dedup_servers<S: Borrow<Server>>(mut servers: Vec<S>) -> Vec<S> {
    let keep: Vec<bool> = {
        let mut seen: rustc_hash::FxHashSet<SocketAddrV4> =
            rustc_hash::FxHashSet::with_capacity_and_hasher(servers.len(), Default::default());
        servers
            .iter()
            .map(|s| seen.insert(s.borrow().endpoint))
            .collect()
    };
    let mut keep = keep.into_iter();
    servers.retain(|_| keep.next().unwrap_or(false));
    servers
}

/// The servers that claim the same address to join (ip, game port) under
/// different query ports: one server listed twice (one entry stale), or one
/// stating a wrong game port. Each group is that address to join and the
/// query addresses claiming it, in list order. Servers on one address with
/// the same name but their own game ports are not here: they are separate
/// servers (hosts give many customers the same default name).
pub fn join_conflicts<S: Borrow<Server>>(servers: &[S]) -> Vec<(SocketAddrV4, Vec<SocketAddrV4>)> {
    // Every (join, query) pair in one vector, sorted by the address to join
    // (stable: list order within each), then read in runs: no table of a
    // vector per server for the few that conflict.
    let mut pairs: Vec<(SocketAddrV4, SocketAddrV4)> = servers
        .iter()
        .filter_map(|s| {
            let s = s.borrow();
            Some((s.join_addr()?, s.endpoint))
        })
        .collect();
    pairs.sort_by_key(|&(join, _)| join);
    pairs
        .chunk_by(|a, b| a.0 == b.0)
        .filter(|run| run.len() > 1)
        .filter_map(|run| Some((run.first()?.0, run.iter().map(|&(_, q)| q).collect())))
        .collect()
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
    /// Unix timestamp (seconds) when the launcher list in it was fetched.
    /// Older caches hold when they were last written, which is no earlier.
    pub fetched_at: u64,
    /// Servers the launcher list itself had then (before Steam's and the
    /// kept ones); absent from older caches.
    #[serde(default)]
    pub launcher_count: Option<u32>,
    pub list: ServerList,
}

/// When the launcher list in a cache was fetched, and how many servers it
/// had, for [`save_server_list_cache`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStamp {
    pub fetched_at: u64,
    pub launcher_count: Option<u32>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    launcher_count: Option<u32>,
    list: &'a ServerList,
}

/// Persist the server list, with when its launcher list was fetched
/// (`stamp`; a zero time is taken as now). Serialized from a reference (no
/// clone of the list) and written on the blocking pool, through a file
/// beside the cache renamed over it, so a crash or a power cut mid-write
/// leaves the previous cache whole rather than a truncated one.
/// Failures are ignored: the cache only speeds up the next start.
pub async fn save_server_list_cache(
    cache_path: &std::path::Path,
    list: std::sync::Arc<ServerList>,
    stamp: CacheStamp,
) {
    let fetched_at = if stamp.fetched_at == 0 {
        dz_common::time::now_secs()
    } else {
        stamp.fetched_at
    };
    let path = cache_path.to_path_buf();
    let _ = tokio::task::spawn_blocking(move || {
        let data = serde_json::to_vec(&ServerListCacheRef {
            fetched_at,
            launcher_count: stamp.launcher_count,
            list: &list,
        })
        .map_err(std::io::Error::other)?;
        write_atomic(&path, &data)
    })
    .await;
}

/// Write `data` to `path` atomically: into a new file in the same directory
/// (so the rename stays on one file system), flushed, then renamed over
/// `path`, which replaces it on every platform. A reader sees the old file
/// or the new one, never half of it. The temporary file is removed if
/// anything fails. The one write of the workspace: [`dz_common::fs::write_atomic`].
pub fn write_atomic(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    dz_common::fs::write_atomic(path, data)
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

    fn server(ip: &str, qport: u16, gport: u16, name: &str) -> Server {
        Server {
            endpoint: SocketAddrV4::new(ip.parse().unwrap(), qport),
            game_port: gport,
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn servers_claiming_one_join_address_are_grouped() {
        let list = vec![
            server("1.1.1.1", 27019, 2502, "panc"),
            server("1.1.1.1", 2505, 2502, "panc"),
            server("1.1.1.1", 27020, 2602, "panc"),
            server("2.2.2.2", 27016, 2302, "a"),
        ];
        assert_eq!(
            join_conflicts(&list),
            vec![(
                "1.1.1.1:2502".parse::<SocketAddrV4>().unwrap(),
                vec![
                    "1.1.1.1:27019".parse().unwrap(),
                    "1.1.1.1:2505".parse().unwrap()
                ]
            )]
        );
    }

    #[test]
    fn dedup_keeps_first_occurrence() {
        let list = vec![
            server("1.1.1.1", 27016, 2302, "a"),
            server("1.1.1.1", 27016, 2302, "b"),
            server("1.1.1.1", 27017, 2402, "c"),
        ];
        let out = dedup_servers(list);
        let shared = dedup_servers(vec![
            Arc::new(server("1.1.1.1", 27016, 2302, "a")),
            Arc::new(server("1.1.1.1", 27016, 2302, "b")),
        ]);
        assert_eq!(shared.len(), 1);
        let names: Vec<_> = out.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["a", "c"]);
    }

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let dir = std::env::temp_dir().join(format!("dz-api-atomic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("nested").join("cache.json");
        write_atomic(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        write_atomic(&path, b"second, longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second, longer");
        let left: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from("cache.json")]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_failing_keeps_the_old_file() {
        let dir = std::env::temp_dir().join(format!("dz-api-atomic-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A directory in the way of the rename: the write fails, the
        // directory stays, and no temporary file is left.
        let path = dir.join("cache.json");
        std::fs::create_dir_all(path.join("inside")).unwrap();
        assert!(write_atomic(&path, b"data").is_err());
        assert!(path.join("inside").is_dir());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn the_cache_round_trips() {
        let dir = std::env::temp_dir().join(format!("dz-api-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("servers.json");
        let list = Arc::new(ServerList {
            status: 0,
            result: vec![Arc::new(server("1.1.1.1", 27016, 2302, "a"))],
        });
        let stamp = CacheStamp {
            fetched_at: 1_700_000_000,
            launcher_count: Some(1),
        };
        save_server_list_cache(&path, Arc::clone(&list), stamp).await;
        let back = load_server_list_cache(&path).await.unwrap();
        assert_eq!(back.list, *list);
        assert_eq!(back.fetched_at, 1_700_000_000);
        assert_eq!(back.launcher_count, Some(1));
        // A cache written before the count was kept still loads.
        std::fs::write(
            &path,
            br#"{"fetched_at":5,"list":{"status":0,"result":[]}}"#,
        )
        .unwrap();
        let old = load_server_list_cache(&path).await.unwrap();
        assert_eq!((old.fetched_at, old.launcher_count), (5, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let json = r#"{"status":0,"result":[{"gamePort":2302,"sponsor":true,"endpoint":{"ip":"1.2.3.4","port":27016},"name":"n","map":"chernarusplus","players":1,"maxPlayers":60,"environment":"w","password":false,"version":"1.28","vac":true,"battlEye":true,"firstPersonOnly":false,"time":"12:00","mods":[],"shard":"x"}]}"#;
        let list: ServerList = serde_json::from_str(json).unwrap();
        assert_eq!(list.result[0].query_port(), 27016);
    }

    #[test]
    fn servers_without_an_ipv4_query_address_are_left_out() {
        let entry = |ip: &str, port: i64, game: i64| {
            format!(
                r#"{{"gamePort":{game},"endpoint":{{"ip":{ip},"port":{port}}},"name":"n","map":"m","players":0,"maxPlayers":60,"environment":"w","password":false,"version":"1","vac":true,"battlEye":null,"firstPersonOnly":false,"time":"","mods":[]}}"#
            )
        };
        let json = format!(
            r#"{{"status":0,"result":[{},{},{},{},{}]}}"#,
            entry(r#""1.2.3.4""#, 27016, 2302),
            entry(r#""play.example.com""#, 27016, 2302),
            entry(r#""2001:db8::1""#, 27016, 2302),
            entry(r#""5.6.7.8""#, 0, 2302),
            entry(r#""5.6.7.8""#, 27017, -1),
        );
        let list: ServerList = serde_json::from_str(&json).unwrap();
        let kept: Vec<String> = list.result.iter().map(|s| s.endpoint.to_string()).collect();
        assert_eq!(kept, ["1.2.3.4:27016", "5.6.7.8:27017"]);
        // A game port that is no port reads as unknown, as 0 did.
        assert_eq!(list.result[1].game_port, 0);
        assert_eq!(list.result[1].join_addr(), None);
        assert_eq!(
            list.result[0].join_addr(),
            Some("1.2.3.4:2302".parse().unwrap())
        );
    }
}
