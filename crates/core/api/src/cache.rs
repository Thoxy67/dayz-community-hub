//! The server list on disk, between two starts: a binary file (postcard)
//! behind a magic and a version, with every mod written once in a table and
//! each server naming its mods by their place in it.
//!
//! The list is about ten thousand servers holding some 150 000 mod entries,
//! of which only about 13 000 differ. As JSON it was 11 MB, parsed at every
//! start and written every 30 s while a Steam listing ran; this file is a
//! fraction of that and reads several times faster, and a read shares one
//! name per mod as the network parse does (see `names`). JSON stays the
//! network's format only; an older JSON cache is read when no binary one
//! exists yet, or when an older build wrote it after the binary one.
//!
//! The file is only a speed-up: one that is missing, cut short, of another
//! version or otherwise unreadable is no cache, never an error.

use crate::{Mod, Server, ServerList, SocketAddrV4};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use std::path::Path;
use std::sync::Arc;

/// The first bytes of the file: what it is, then the layout's version.
const MAGIC: &[u8; 4] = b"DZSL";
/// Raised whenever [`Record`] or [`File`] change shape: a file of another
/// version is not read (it would decode as garbage or fail halfway).
const VERSION: u16 = 1;

/// On-disk cache envelope for the server list.
#[derive(Debug)]
pub struct ServerListCache {
    /// Unix timestamp (seconds) when the launcher list in it was fetched.
    /// Older caches hold when they were last written, which is no earlier.
    pub fetched_at: u64,
    /// Servers the launcher list itself had then (before Steam's and the
    /// kept ones); absent from older caches.
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

/// The file after its header. `T` is the mod table and `R` a server's
/// record: borrowed from the list to write it, owned to read it.
#[derive(Serialize, Deserialize)]
struct File<T, R> {
    fetched_at: u64,
    launcher_count: Option<u32>,
    status: i64,
    /// Every distinct mod (name and workshop id), once.
    mods: T,
    servers: Vec<R>,
}

/// One server, its address as numbers and its mods as places in the table.
/// `S` is how a text is held and `M` how the places are.
#[derive(Serialize, Deserialize)]
struct Record<S, M> {
    ip: [u8; 4],
    query_port: u16,
    game_port: u16,
    name: S,
    map: S,
    players: i64,
    max_players: i64,
    environment: S,
    password: bool,
    version: S,
    vac: bool,
    battl_eye: Option<bool>,
    first_person_only: bool,
    time: S,
    mods: M,
    official: bool,
}

/// The file as read: every text owned, the places as numbers.
type ReadFile = File<Vec<(String, i64)>, Record<String, Vec<u32>>>;

/// A server's mods as their places in the table, written without a vector
/// of them.
struct Places<'a> {
    mods: &'a [Mod],
    table: &'a rustc_hash::FxHashMap<(&'a str, i64), u32>,
}

impl Serialize for Places<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut seq = s.serialize_seq(Some(self.mods.len()))?;
        for m in self.mods {
            let place = self
                .table
                .get(&(&*m.name, m.steam_workshop_id))
                .ok_or_else(|| serde::ser::Error::custom("a mod missing from the table"))?;
            seq.serialize_element(place)?;
        }
        seq.end()
    }
}

/// The file's bytes: the header, then the list with its mod table.
fn encode(list: &ServerList, fetched_at: u64, launcher_count: Option<u32>) -> Vec<u8> {
    let mut table: rustc_hash::FxHashMap<(&str, i64), u32> = Default::default();
    let mut mods: Vec<(&str, i64)> = Vec::new();
    for m in list.result.iter().flat_map(|s| &s.mods) {
        let key = (&*m.name, m.steam_workshop_id);
        table.entry(key).or_insert_with(|| {
            mods.push(key);
            (mods.len() - 1) as u32
        });
    }
    let servers = list
        .result
        .iter()
        .map(|s| Record {
            ip: s.ip().octets(),
            query_port: s.query_port(),
            game_port: s.game_port,
            name: s.name.as_str(),
            map: s.map.as_str(),
            players: s.players,
            max_players: s.max_players,
            environment: s.environment.as_str(),
            password: s.password,
            version: s.version.as_str(),
            vac: s.vac,
            battl_eye: s.battl_eye,
            first_person_only: s.first_person_only,
            time: s.time.as_str(),
            mods: Places {
                mods: &s.mods,
                table: &table,
            },
            official: s.official,
        })
        .collect();
    let file = File {
        fetched_at,
        launcher_count,
        status: list.status,
        mods,
        servers,
    };
    let mut out = Vec::with_capacity(4 << 20);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    // Empty if it failed, which writing into a vector cannot.
    postcard::to_extend(&file, out).unwrap_or_default()
}

/// The cache in `data`, when it is a whole file of this version.
fn decode(data: &[u8]) -> Option<ServerListCache> {
    let body = data.strip_prefix(MAGIC)?;
    let (version, body) = body.split_first_chunk::<2>()?;
    if u16::from_le_bytes(*version) != VERSION {
        return None;
    }
    let (file, rest): (ReadFile, _) = postcard::take_from_bytes(body).ok()?;
    if !rest.is_empty() {
        return None;
    }
    let table: Vec<Mod> = file
        .mods
        .into_iter()
        .map(|(name, steam_workshop_id)| Mod {
            name: name.into(),
            steam_workshop_id,
        })
        .collect();
    // Pushed into a vector of their own: collected in place, the servers
    // would keep the records' far larger allocation.
    let mut result = Vec::with_capacity(file.servers.len());
    for r in file.servers {
        let mods = r
            .mods
            .iter()
            .map(|&i| table.get(i as usize).cloned())
            .collect::<Option<Vec<Mod>>>()?;
        result.push(Arc::new(Server {
            game_port: r.game_port,
            endpoint: SocketAddrV4::new(r.ip.into(), r.query_port),
            name: r.name,
            map: r.map,
            players: r.players,
            max_players: r.max_players,
            environment: r.environment,
            password: r.password,
            version: r.version,
            vac: r.vac,
            battl_eye: r.battl_eye,
            first_person_only: r.first_person_only,
            time: r.time,
            mods,
            official: r.official,
        }));
    }
    // Read as a list from the launcher is: the servers without an IPv4
    // query address leave (one may have come in with the official list).
    Some(ServerListCache {
        fetched_at: file.fetched_at,
        launcher_count: file.launcher_count,
        list: ServerList::from(crate::ReadList {
            status: file.status,
            result,
        }),
    })
}

/// The JSON cache older versions wrote.
#[derive(Deserialize)]
struct JsonCache {
    fetched_at: u64,
    #[serde(default)]
    launcher_count: Option<u32>,
    list: ServerList,
}

/// Load the cached server list: the binary file at `path` or the JSON one
/// older versions wrote at `legacy`, whichever holds the later launcher
/// list. Both are there only when an older build ran after this one (the
/// JSON is deleted once a binary cache is written), and then the JSON may
/// be the newer. `None` if neither is there whole and readable. The files
/// are read asynchronously and decoded on the blocking pool.
pub async fn load_server_list_cache(path: &Path, legacy: &Path) -> Option<ServerListCache> {
    let binary = match tokio::fs::read(path).await {
        Ok(data) => tokio::task::spawn_blocking(move || decode(&data))
            .await
            .ok()
            .flatten(),
        Err(_) => None,
    };
    let json = match tokio::fs::read(legacy).await {
        Ok(data) => {
            tokio::task::spawn_blocking(move || serde_json::from_slice::<JsonCache>(&data).ok())
                .await
                .ok()
                .flatten()
                .map(|old| ServerListCache {
                    fetched_at: old.fetched_at,
                    launcher_count: old.launcher_count,
                    list: old.list,
                })
        }
        Err(_) => None,
    };
    match (binary, json) {
        (Some(binary), Some(json)) if json.fetched_at > binary.fetched_at => Some(json),
        (binary, json) => binary.or(json),
    }
}

/// Persist the server list, with when its launcher list was fetched
/// (`stamp`; a zero time is taken as now). Encoded from a reference (no
/// clone of the list) and written on the blocking pool, through a file
/// beside the cache renamed over it, so a crash or a power cut mid-write
/// leaves the previous cache whole rather than a truncated one. Whether it
/// was written; a failure costs only the next start's speed-up.
pub async fn save_server_list_cache(path: &Path, list: Arc<ServerList>, stamp: CacheStamp) -> bool {
    let fetched_at = if stamp.fetched_at == 0 {
        dz_common::time::now_secs()
    } else {
        stamp.fetched_at
    };
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let data = encode(&list, fetched_at, stamp.launcher_count);
        !data.is_empty() && crate::write_atomic(&path, &data).is_ok()
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(ip: &str, qport: u16, mods: &[(&str, i64)]) -> Arc<Server> {
        Arc::new(Server {
            endpoint: SocketAddrV4::new(ip.parse().unwrap(), qport),
            game_port: qport - 1,
            name: format!("server {ip}"),
            map: "chernarusplus".into(),
            players: 12,
            max_players: 60,
            environment: "w".into(),
            version: "1.28".into(),
            battl_eye: Some(true),
            time: "12:00".into(),
            mods: mods
                .iter()
                .map(|&(name, id)| Mod {
                    name: name.into(),
                    steam_workshop_id: id,
                })
                .collect(),
            official: ip == "2.2.2.2",
            ..Default::default()
        })
    }

    fn list() -> Arc<ServerList> {
        Arc::new(ServerList {
            status: 0,
            result: vec![
                server("1.1.1.1", 27016, &[("@CF", 1), ("@Trader", 2)]),
                server("2.2.2.2", 27017, &[]),
                server("3.3.3.3", 27018, &[("@Trader", 2), ("@CF", 1), ("@CF", 3)]),
            ],
        })
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dz-api-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_list_reads_back_as_written_with_one_name_per_mod() {
        let list = list();
        let back = decode(&encode(&list, 7, Some(3))).unwrap();
        assert_eq!(back.list, *list);
        assert_eq!((back.fetched_at, back.launcher_count), (7, Some(3)));
        let (a, b) = (&back.list.result[0].mods[0], &back.list.result[2].mods[1]);
        assert_eq!((&*a.name, a.steam_workshop_id), ("@CF", 1));
        assert!(Arc::ptr_eq(&a.name, &b.name));
        // One name, two workshop ids: two mods.
        assert_eq!(back.list.result[2].mods[2].steam_workshop_id, 3);
    }

    #[test]
    fn a_broken_or_foreign_file_is_no_cache() {
        let good = encode(&list(), 7, None);
        assert!(decode(&good).is_some());
        assert!(decode(&good[..good.len() - 1]).is_none());
        assert!(decode(&good[..5]).is_none());
        assert!(decode(b"").is_none());
        assert!(decode(br#"{"fetched_at":5}"#).is_none());
        let mut other = good.clone();
        other[4..6].copy_from_slice(&(VERSION + 1).to_le_bytes());
        assert!(decode(&other).is_none());
        let mut longer = good;
        longer.push(0);
        assert!(decode(&longer).is_none());
        // A mod place past the table's end.
        let one = ServerList {
            status: 0,
            result: vec![server("1.1.1.1", 27016, &[("@CF", 1)])],
        };
        let mut bad = encode(&one, 7, None);
        let at = bad.len() - 2;
        assert_eq!(bad[at], 0, "the server's one mod place");
        bad[at] = 9;
        assert!(decode(&bad).is_none());
    }

    #[tokio::test]
    async fn the_cache_round_trips_and_falls_back_to_the_old_json() {
        let dir = dir("cache");
        let (path, legacy) = (dir.join("servers.bin"), dir.join("servers.json"));
        assert!(load_server_list_cache(&path, &legacy).await.is_none());
        // Only the JSON an older version wrote: it is read.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            &legacy,
            br#"{"fetched_at":5,"list":{"status":0,"result":[]}}"#,
        )
        .unwrap();
        let old = load_server_list_cache(&path, &legacy).await.unwrap();
        assert_eq!((old.fetched_at, old.launcher_count), (5, None));
        // The binary file, once written, comes first.
        let list = list();
        let stamp = CacheStamp {
            fetched_at: 1_700_000_000,
            launcher_count: Some(3),
        };
        assert!(save_server_list_cache(&path, Arc::clone(&list), stamp).await);
        let back = load_server_list_cache(&path, &legacy).await.unwrap();
        assert_eq!(back.list, *list);
        assert_eq!(
            (back.fetched_at, back.launcher_count),
            (1_700_000_000, Some(3))
        );
        // A JSON an older build wrote after the binary file wins.
        std::fs::write(
            &legacy,
            br#"{"fetched_at":1800000000,"list":{"status":0,"result":[]}}"#,
        )
        .unwrap();
        let newer = load_server_list_cache(&path, &legacy).await.unwrap();
        assert_eq!(newer.fetched_at, 1_800_000_000);
        assert!(newer.list.result.is_empty());
        // A broken binary file is no cache, and the JSON is read instead.
        std::fs::write(
            &legacy,
            br#"{"fetched_at":5,"list":{"status":0,"result":[]}}"#,
        )
        .unwrap();
        std::fs::write(&path, b"DZSL\x01\x00garbage").unwrap();
        let old = load_server_list_cache(&path, &legacy).await.unwrap();
        assert_eq!(old.fetched_at, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_server_without_a_query_address_is_left_out_as_the_file_is_read() {
        let mut no_ip = (*server("1.1.1.1", 27016, &[("@CF", 1)])).clone();
        no_ip.endpoint = SocketAddrV4::new(crate::Ipv4Addr::UNSPECIFIED, 27016);
        let mut no_port = (*server("2.2.2.2", 27017, &[])).clone();
        no_port.endpoint = SocketAddrV4::new("2.2.2.2".parse().unwrap(), 0);
        let mut servers = list().result.clone();
        servers.extend([Arc::new(no_ip), Arc::new(no_port)]);
        let written = ServerList {
            status: 0,
            result: servers,
        };
        let back = decode(&encode(&written, 7, None)).unwrap();
        assert_eq!(back.list, *list());
    }
}
