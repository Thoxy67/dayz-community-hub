//! The invented world of [`HubMode::Mock`](crate::HubMode): the port of
//! the Svelte interface's `lib/ipc/mock.ts`, the pretend backend its
//! screenshots (`var/reference/*.png`) were taken on.
//!
//! The same generator with the same seed (42, the same linear congruential
//! steps in `f64`, as JavaScript computes them) makes the same 9 500
//! servers, 24 mods, profile, articles and play log, so a screenshot of the
//! egui window shows what the reference shows.
//!
//! Where it can, the mock seeds the real state and lets the real commands
//! answer: the server list (filtered, sorted and paged by the real
//! `servers_query`), the pings (in the live store), the profile and the play
//! log (written into a data directory of its own, then loaded as for real),
//! the sources' status and the refresh. Only what would touch the network,
//! Steam, the game or the user's files answers from fixtures (`fixtures`),
//! and the long-running things (the ping scan, a refresh, a mod operation,
//! an update's download) play timed steps instead.
//!
//! The data directory is a fresh one under the system's temporary directory
//! (removed when the hub goes), unless `HubConfig::data_dir` names one: the
//! world's `profile.json` and `sessions.json` are written into it either
//! way. Nothing goes over the network.

mod commands;
pub(crate) mod fixtures;
mod generate;
mod modop;
mod news;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use dz_api::{Server, ServerList, SocketAddrV4};
use rustc_hash::FxHashMap;

use crate::features::browser::live::{self, Live};
use crate::state::AppState;

pub(crate) use generate::{MockMod, MockServer};

/// A switch of the mock, for screenshots of states its world does not show by itself:
/// `DZCH_MOCK=wizard,steamcmd-missing` (comma-separated). `wizard` makes this a first
/// launch (the setup wizard opens), `steamcmd-missing` hides SteamCMD from detection;
/// `update-none`, `update-error`, `update-package`, `update-dev` set the updater's answers;
/// `offline-empty` installs no offline mission, `offline-installing` never ends its install;
/// `stats-empty` writes no play session.
pub(crate) fn switch(name: &str) -> bool {
    std::env::var("DZCH_MOCK")
        .map(|v| v.split(',').any(|s| s.trim() == name))
        .unwrap_or(false)
}

/// A ping at or above this is a timeout in the mock (`pingOf(k) > 270`).
const PING_TIMEOUT_ABOVE: u32 = 270;
/// What a timed-out ping shows (mock.ts' `9999`).
const PING_TIMEOUT_MS: u32 = 9999;

/// Workshop ids of the mods servers run that are not on disk, and of the
/// ones on disk the mock's 24 installed mods leave out (see
/// [`World::server_list`]).
const MISSING_BASE: u64 = 2_910_000_000;
const EXTRA_BASE: u64 = 2_920_000_000;

/// The world: everything mock.ts invents, made once per hub.
pub(crate) struct World {
    /// Unix seconds when it was made (mock.ts' `now`).
    pub now: i64,
    pub servers: Vec<MockServer>,
    pub mods: Vec<MockMod>,
    pub articles: Vec<crate::features::news::ArticleDto>,
    pub play_log: Vec<dz_profile::Session>,
    data_dir: PathBuf,
    /// Made by the world, so removed with it.
    owned: bool,
}

impl World {
    /// Make the world and write its profile and play log into `data_dir`
    /// (a fresh temporary directory when `None`).
    pub(crate) fn create(data_dir: Option<PathBuf>) -> Result<World, String> {
        let now = dz_common::time::now_secs() as i64;
        let generated = generate::generate(now);
        let (data_dir, owned) = match data_dir {
            Some(d) => (d, false),
            None => (fresh_dir(), true),
        };
        let world = World {
            now,
            servers: generated.servers,
            mods: generated.mods,
            articles: generated.articles,
            // `stats-empty`: nobody has played yet (the stats view's empty state).
            play_log: if switch("stats-empty") {
                Vec::new()
            } else {
                generated.play_log
            },
            data_dir,
            owned,
        };
        world.write_files()?;
        Ok(world)
    }

    /// The data directory the world lives in.
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// `<data dir>/profile.json`.
    pub(crate) fn profile_path(&self) -> PathBuf {
        self.data_dir.join("profile.json")
    }

    fn write_files(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| format!("Mock data directory {}: {e}", self.data_dir.display()))?;
        let mut profile = generate::profile(self);
        profile.path = self.profile_path();
        profile
            .snapshot()
            .and_then(|s| s.write_blocking())
            .map_err(|e| format!("Mock profile: {e}"))?;
        let log = dz_profile::PlayLog {
            sessions: self.play_log.clone(),
            path: self.data_dir.join(dz_profile::SESSIONS_FILE),
        };
        log.snapshot()
            .write_blocking()
            .map_err(|e| format!("Mock play log: {e}"))?;
        Ok(())
    }

    /// The server list as the real state holds it. Each server runs as many
    /// mods as mock.ts says, chosen so the real browser works out the same
    /// "missing" and "behind" counts mock.ts invented for its rows.
    pub(crate) fn server_list(&self) -> ServerList {
        let stale: Vec<&MockMod> = self.mods.iter().filter(|m| m.update_available).collect();
        let current: Vec<&MockMod> = self.mods.iter().filter(|m| !m.update_available).collect();
        let result = self
            .servers
            .iter()
            .map(|s| {
                let count = s.mods_count;
                let missing = s.mods_missing().min(count);
                let behind = s.mods_stale().min(count - missing).min(stale.len());
                let have = count - missing - behind;
                let mut mods = Vec::with_capacity(count);
                for k in 0..have {
                    mods.push(match current.get(k) {
                        Some(m) => mod_entry(m.id, &m.name),
                        None => {
                            let n = k - current.len();
                            mod_entry(EXTRA_BASE + n as u64, &format!("Server Pack {}", n + 1))
                        }
                    });
                }
                for m in stale.iter().take(behind) {
                    mods.push(mod_entry(m.id, &m.name));
                }
                for k in 0..missing {
                    mods.push(mod_entry(
                        MISSING_BASE + k as u64,
                        &format!("Workshop Mod {}", k + 1),
                    ));
                }
                std::sync::Arc::new(Server {
                    game_port: s.game_port,
                    endpoint: SocketAddrV4::new(s.ip, s.query_port),
                    name: s.name.clone(),
                    map: s.map.clone(),
                    players: s.players,
                    max_players: s.max_players,
                    environment: s.environment.to_string(),
                    password: s.password,
                    version: s.version.to_string(),
                    vac: true,
                    battl_eye: Some(s.battl_eye),
                    first_person_only: s.first_person_only,
                    time: s.time.clone(),
                    mods,
                    official: s.official,
                })
            })
            .collect();
        ServerList { status: 0, result }
    }

    /// What `initialize` adds to the real state: the mods on disk and what
    /// the Workshop said of them, so the browser counts missing and stale
    /// mods as the mock's rows did.
    pub(crate) fn seed_state(&self, state: &mut AppState) {
        let mut disk: FxHashMap<u64, i64> =
            self.mods.iter().map(|m| (m.id, m.local_updated)).collect();
        let most = self.servers.iter().map(|s| s.mods_count).max().unwrap_or(0);
        for n in 0..most {
            disk.insert(EXTRA_BASE + n as u64, self.now);
        }
        state.mods_on_disk = Some(disk);
        state.mod_update_cache = self
            .mods
            .iter()
            .filter_map(|m| m.remote_updated.map(|r| (m.id, r)))
            .collect();
    }

    /// Every server pinged, as mock.ts' rows always were.
    pub(crate) fn seed_pings(&self) {
        let store = live::store();
        for s in &self.servers {
            let addr = SocketAddrV4::new(s.ip, s.query_port);
            store.record(addr, live_of(addr));
        }
        store.touch();
    }

    /// The sources half way through a listing (mock.ts' `mockSources`).
    pub(crate) fn seed_sources(&self) {
        let sources = fixtures::sources(self.now);
        crate::features::servers::sources::update(|s| *s = sources);
    }

    /// The server at `ip` (mock.ts looks servers up by IP alone).
    pub(crate) fn server_by_ip(&self, ip: &str) -> Option<(usize, &MockServer)> {
        let ip: dz_api::Ipv4Addr = ip.trim().parse().ok()?;
        self.servers.iter().enumerate().find(|(_, s)| s.ip == ip)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if self.owned {
            let _ = std::fs::remove_dir_all(&self.data_dir);
        }
    }
}

fn mod_entry(id: u64, name: &str) -> dz_api::Mod {
    dz_api::Mod {
        name: name.into(),
        steam_workshop_id: id as i64,
    }
}

/// A directory of its own under the system's temporary directory.
fn fresh_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    std::env::temp_dir().join(format!(
        "dayz-community-hub-mock-{}-{nanos}",
        std::process::id()
    ))
}

/// mock.ts' `pingOf`: a round trip made up from the address, 20 to 279 ms.
pub(crate) fn ping_of(key: &str) -> u32 {
    let len = key.encode_utf16().count() as u64;
    let last = key.encode_utf16().last().map_or(0, u64::from);
    20 + ((len * 37 + last * 11) % 260) as u32
}

/// What a ping of `addr` comes to in the mock: above 270 ms, a timeout.
/// mock.ts hashed the address as text, so it is written out here.
pub(crate) fn live_of(addr: SocketAddrV4) -> Live {
    let ms = ping_of(&addr.to_string());
    let failed = ms > PING_TIMEOUT_ABOVE;
    Live {
        ms: Some(if failed { PING_TIMEOUT_MS } else { ms }),
        players: None,
        max_players: None,
        bots: None,
        failed,
    }
}

/// Unix ms as JavaScript's `toISOString()` writes it.
pub(crate) fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let sod = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z",
        sod / 3600,
        sod % 3600 / 60,
        sod % 60
    )
}

/// Unix seconds as a UTC date, `YYYY-MM-DD`.
pub(crate) fn iso_date(secs: i64) -> String {
    let (y, m, d) = civil(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The UTC hour of Unix seconds.
pub(crate) fn utc_hour(secs: i64) -> i64 {
    secs.rem_euclid(86_400) / 3600
}

/// The UTC weekday of Unix seconds, 0 Sunday (`getUTCDay`).
pub(crate) fn utc_weekday(secs: i64) -> i64 {
    (secs.div_euclid(86_400) + 4).rem_euclid(7)
}

/// Days since 1970-01-01 as a civil date (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}
