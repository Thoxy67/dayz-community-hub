//! The server list: loading it (from cache or the API), refreshing it, and
//! the counters the title bar shows.

mod dto;
mod memory;
pub(crate) mod refresh;
pub(crate) mod sources;
mod steam;

pub use dto::{AppStatsDto, InitResult, ModDto, ServerDto};
pub(crate) use dto::{mods_to_dto, server_to_dto};

use crate::paths;
use dz_api::{ServerList, SocketAddrV4};
use dz_game::DayzCtl;
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, RwLock};

use crate::Hub;
use crate::error::ResultExt;
use crate::state::{AppState, SharedState};

/// Whether this is the first launch (no profile yet), asked before
/// `initialize` so the wizard can show at once.
async fn first_launch() -> bool {
    !tokio::fs::try_exists(paths::profile_path())
        .await
        .unwrap_or(false)
}

/// This process, as a token: its id and when it started. A reload keeps
/// the process, so it keeps the token; a new launch has another.
static SESSION: LazyLock<String> =
    LazyLock::new(|| format!("{}-{}", std::process::id(), crate::features::ping::now_ms()));

/// One `initialize` at a time: a page reloaded during the first one sends
/// it again (tauri-apps/tauri#14154), and the second must find the first's
/// state rather than build its own beside it.
static INIT: Mutex<()> = Mutex::const_new(());
/// The load [`Hub::prewarm`] started, for the window's first `initialize`.
type Prewarm = tokio::task::JoinHandle<Result<InitResult, String>>;
static PREWARM: std::sync::Mutex<Option<Prewarm>> = std::sync::Mutex::new(None);

/// A cached launcher list fetched less than this ago is not fetched again
/// at start (a restart, one after an update included): the list is shown
/// as fetched then, and the ping scan brings its counts up to date. As long
/// as the window leaves a list unrefreshed while it is away.
const LIST_FRESH: u64 = 5 * 60;

/// When the launcher list on hand was fetched, and its size: written with
/// every save of the cache.
static LAUNCHER: std::sync::Mutex<dz_api::CacheStamp> = std::sync::Mutex::new(dz_api::CacheStamp {
    fetched_at: 0,
    launcher_count: None,
});

pub(crate) fn launcher() -> dz_api::CacheStamp {
    LAUNCHER.lock().map(|s| *s).unwrap_or_default()
}

fn set_launcher(stamp: dz_api::CacheStamp) {
    if let Ok(mut s) = LAUNCHER.lock() {
        *s = stamp;
    }
}

/// Create the controller and load the server list: from the on-disk cache
/// when there is one (the window refreshes it in the background, unless it
/// was fetched less than [`LIST_FRESH`] ago), else from the API. The profile and the cache are read at the same time.
///
/// Called when the window mounts. When the window reloads (F5, a crash
/// recovery, the "Retry" after a failed start) the state already exists: it
/// is kept as it is, with the work running on it (the scan, a refresh, a mod
/// download and its SteamCMD input), and `resumed` tells the window so.
async fn initialize(hub: &Hub) -> Result<InitResult, String> {
    let _once = INIT.lock().await;
    if let Some(existing) = hub.try_state() {
        let s = existing.read().await;
        let server_count = s.servers.result.len();
        let r = refresh::snapshot();
        return Ok(InitResult {
            server_count,
            from_cache: r.loaded_at.is_none(),
            is_first_launch: first_launch().await,
            // Only while there is still no list to show.
            list_error: if server_count == 0 { r.error } else { None },
            resumed: true,
            session: SESSION.clone(),
        });
    }
    #[cfg(feature = "mock")]
    if let Some(world) = hub.mock() {
        return initialize_mock(hub, world).await;
    }
    let profile_path = paths::profile_path();
    let is_first_launch = !tokio::fs::try_exists(&profile_path).await.unwrap_or(false);
    let (cache_path, legacy_cache) = (
        paths::server_list_cache_path(),
        paths::legacy_server_list_cache_path(),
    );

    let memory_path = memory::path();
    // The play log is read as soon as the profile is, beside the cache.
    let profile = async {
        let ctl = DayzCtl::new(&profile_path, crate::net::download().clone()).await?;
        let history = ctl.profile().history.clone();
        if let Err(e) =
            tokio::task::spawn_blocking(move || crate::features::stats::open(&history)).await
        {
            eprintln!("[stats] the play log was not read: {e}");
        }
        Ok::<_, dz_common::Error>(ctl)
    };
    let (ctl, cache, memory) = tokio::join!(
        profile,
        dz_api::load_server_list_cache(&cache_path, &legacy_cache),
        memory::load(&memory_path)
    );
    let ctl = ctl.map_err(|e| format!("Could not load the profile: {e}"))?;

    // The cache holds the official servers merged last time; a cold start
    // fetches both lists at once.
    let now = dz_common::time::now_secs();
    let mut fresh = false;
    let (list, from_cache, list_error) = match cache {
        Some(cache) if !cache.list.result.is_empty() => {
            set_launcher(dz_api::CacheStamp {
                fetched_at: cache.fetched_at,
                launcher_count: cache.launcher_count,
            });
            fresh =
                cache.fetched_at <= now + 60 && now.saturating_sub(cache.fetched_at) < LIST_FRESH;
            let list = dedup(cache.list);
            steam::remember(memory, &list);
            (steam::with_steam(list), true, None)
        }
        _ => {
            let key = ctl.profile().steam_api_key.clone();
            dzsa_starts();
            let (community, official) = tokio::join!(
                dz_api::fetch_servers(crate::net::api()),
                fetch_official(crate::net::api(), key.as_deref())
            );
            dzsa_done(&community);
            match community {
                Ok(list) => {
                    let list = dedup(with_official(list, official));
                    steam::remember(memory, &list);
                    let list = steam::with_steam(list);
                    save_cache_in_background(Arc::clone(&list));
                    (list, false, None)
                }
                Err(e) => (Arc::default(), false, Some(e.to_string())),
            }
        }
    };

    let stamp = launcher();
    refresh::initialized(match &list_error {
        Some(e) => Err(e.clone()),
        None if fresh => Ok(Some(
            i64::try_from(stamp.fetched_at.saturating_mul(1000)).unwrap_or(i64::MAX),
        )),
        None => Ok((!from_cache).then(crate::features::ping::now_ms)),
    });
    if fresh {
        // The window takes the list as fetched then and starts no refresh.
        let count = stamp
            .launcher_count
            .unwrap_or(u32::try_from(list.result.len()).unwrap_or(u32::MAX));
        sources::update(|s| {
            s.dzsa.phase = sources::Phase::Done;
            s.dzsa.count = count;
            s.dzsa.at = i64::try_from(stamp.fetched_at).ok();
        });
    }
    let server_count = list.result.len();
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(Arc::clone(&list));
    hub.set_state(Arc::new(RwLock::new(app_state)));
    crate::features::browser::list_replaced(list);
    // The cached list too.
    steam::comb(hub.clone());
    // Steam's listing: a cached start's background refresh starts it; a
    // cold start and a fresh cache (no refresh) start it here. Either way
    // it is skipped if it finished minutes ago, before a restart.
    if !from_cache || fresh {
        steam::start(hub.clone());
    }

    Ok(InitResult {
        server_count,
        from_cache,
        is_first_launch,
        list_error,
        resumed: false,
        session: SESSION.clone(),
    })
}

/// `initialize` in the mock world: its profile and play log from its own
/// data directory, its server list, every server pinged, the mods on disk
/// and the sources as mock.ts showed them. Nothing is fetched.
#[cfg(feature = "mock")]
async fn initialize_mock(hub: &Hub, world: &crate::mock::World) -> Result<InitResult, String> {
    let ctl = DayzCtl::new(world.profile_path(), crate::net::download().clone())
        .await
        .map_err(|e| format!("Could not load the profile: {e}"))?;
    crate::features::stats::open_in(world.data_dir(), &ctl.profile().history);
    let list = Arc::new(world.server_list());
    let server_count = list.result.len();
    refresh::mock_loaded(server_count as u32);
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(Arc::clone(&list));
    world.seed_state(&mut app_state);
    app_state.profile_changed();
    hub.set_state(Arc::new(RwLock::new(app_state)));
    world.seed_pings();
    world.seed_sources();
    Ok(InitResult {
        server_count,
        from_cache: false,
        is_first_launch: false,
        list_error: None,
        resumed: false,
        session: SESSION.clone(),
    })
}

/// One server with its mods, by query address.
async fn get_server_details(hub: &Hub, addr: SocketAddrV4) -> Result<ServerDto, String> {
    let state = hub.state()?;
    let state = state.read().await;
    let server = state
        .find_by_query_port(addr)
        .ok_or_else(|| "Server not found".to_string())?;
    let mut dto = server_to_dto(server);
    // The map's one spelling, as the rows show it.
    if let Some(r) = crate::features::browser::live::store()
        .snapshot()
        .lookup(addr)
    {
        r.map.clone_into(&mut dto.map);
    }
    Ok(dto)
}

/// Fetch the list from the API again (a step of `refresh::servers_refresh`).
/// Returns how many servers it holds; the browser re-queries on the
/// `servers-changed` that follows. Servers the list had and no longer lists
/// stay, unless they do not answer. Then Steam's servers are listed in the
/// background (at most every few minutes) and added as they answer.
async fn refresh_list(hub: &Hub, state: &SharedState) -> Result<u32, String> {
    // The mock's refresh takes three seconds and finds the same list.
    mocked!(hub => {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        crate::features::browser::live::store().touch();
        Ok(state.read().await.servers.result.len() as u32)
    });
    let key = state.read().await.ctl.profile().steam_api_key.clone();
    dzsa_starts();
    let client = crate::net::api();
    let (community, official) = tokio::join!(
        dz_api::fetch_servers(client),
        fetch_official(client, key.as_deref())
    );
    dzsa_done(&community);
    let fresh = dedup(with_official(community.cmd_err()?, official));
    let list = {
        // In order with the lists Steam's listing puts in (see LIST_WRITER),
        // so the list read here is the last one put in.
        let _order = steam::LIST_WRITER.lock().await;
        let previous = Arc::clone(&state.read().await.servers);
        // What the list had and no longer lists stays while it answers.
        let list = steam::refreshed(&previous, fresh);
        // The snapshot built off the state's lock and off this worker.
        crate::state::replace_servers(state, Arc::clone(&list)).await;
        list
    };
    save_cache_in_background(Arc::clone(&list));
    let count = list.result.len() as u32;
    crate::features::browser::list_replaced(list);
    steam::comb(hub.clone());
    steam::check_kept(hub.clone());
    steam::start(hub.clone());
    Ok(count)
}

/// Whether the servers Steam's master lists are added to the list. Turning
/// it off stops a listing under way; turning it on lists them now.
async fn set_steam_server_list(hub: &Hub, enabled: bool) -> Result<(), String> {
    let state = hub.state()?;
    crate::state::mutate_profile(&state, |s| {
        s.ctl.profile_mut().steam_server_list = enabled;
        Ok(())
    })
    .await?;
    // The mock's sources never list anything.
    mocked!(hub => Ok(()));
    if enabled {
        steam::resume(hub.clone());
    } else {
        steam::stop();
    }
    Ok(())
}

/// The title bar's counters.
async fn get_app_stats(hub: &Hub) -> Result<AppStatsDto, String> {
    let state = hub.state()?;
    let state = state.read().await;
    let mut stats = AppStatsDto {
        server_count: state.servers.result.len(),
        total_players: state.servers.result.iter().map(|s| s.players).sum(),
        player_name: state.ctl.profile().player.clone(),
        steam_login: state.ctl.steamcmd_login().map(str::to_string),
        has_steamcmd: state.ctl.has_steamcmd(),
    };
    // The mock's SteamCMD is there whatever this machine has.
    if hub.is_mock() {
        let p = state.ctl.profile();
        stats.steam_login = p.steamcmd_enabled.then(|| p.steam_login.clone()).flatten();
        stats.has_steamcmd = p.steamcmd_enabled;
    }
    Ok(stats)
}

/// The official servers, when a Steam API key lets us ask Steam's master
/// server for them. Failing to get them never fails the list: the community
/// servers still load, and the next refresh tries again.
async fn fetch_official(client: &reqwest::Client, key: Option<&str>) -> Vec<dz_api::Server> {
    let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else {
        return Vec::new();
    };
    dz_api::fetch_official_servers(client, key)
        .await
        .unwrap_or_else(|e| {
            eprintln!("official servers: {e}");
            Vec::new()
        })
}

fn dzsa_starts() {
    sources::update(|s| {
        s.dzsa.phase = sources::Phase::Running;
        s.dzsa.error = None;
    });
}

fn dzsa_done(r: &dz_common::Result<ServerList>) {
    if let Ok(list) = r {
        set_launcher(dz_api::CacheStamp {
            fetched_at: dz_common::time::now_secs(),
            launcher_count: u32::try_from(list.result.len()).ok(),
        });
    }
    sources::update(|s| match r {
        Ok(list) => {
            s.dzsa.phase = sources::Phase::Done;
            s.dzsa.count = list.result.len() as u32;
            s.dzsa.at = Some(sources::now());
        }
        Err(e) => {
            s.dzsa.phase = sources::Phase::Failed;
            s.dzsa.error = Some(e.to_string());
        }
    });
}

fn with_official(mut list: ServerList, official: Vec<dz_api::Server>) -> ServerList {
    dz_api::merge_official(&mut list.result, official);
    list
}

fn dedup(mut list: ServerList) -> Arc<ServerList> {
    list.result = dz_api::dedup_servers(std::mem::take(&mut list.result));
    Arc::new(list)
}

/// Write the cache without making the caller wait for a multi-MB write.
/// One refresh asks for it several times (the refresh, the comb, the kept
/// check, the listing as it goes and at its end): the writes are coalesced
/// by [`CACHE_WRITER`], so the cache is written once for a burst of them,
/// and always with the newest list. A cache only speeds up the next start,
/// so an exit within the wait loses nothing that matters. Once the binary
/// cache is written, the JSON one an older version left is deleted: the
/// binary one now holds the newer list.
pub(super) fn save_cache_in_background(list: Arc<ServerList>) {
    CACHE_WRITER.request((list, launcher()), |(list, stamp)| async move {
        if dz_api::save_server_list_cache(&paths::server_list_cache_path(), list, stamp).await {
            let _ = tokio::fs::remove_file(paths::legacy_server_list_cache_path()).await;
        }
    });
}

/// How long the cache writer waits for the asks to settle, and the longest
/// an ask waits while more keep coming.
const CACHE_SETTLE: std::time::Duration = std::time::Duration::from_secs(3);
const CACHE_MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

static CACHE_WRITER: Coalesced<(Arc<ServerList>, dz_api::CacheStamp)> = Coalesced::new();

/// One writer for something written whole: the newest value asked for is
/// written once the asks have settled ([`CACHE_SETTLE`] without one, or
/// [`CACHE_MAX_WAIT`] after the first), by one task at a time, so writes
/// never overlap and an older value never lands after a newer one.
struct Coalesced<T> {
    slot: std::sync::Mutex<Pending<T>>,
}

struct Pending<T> {
    value: Option<T>,
    /// When the first ask still unwritten came, and the last.
    first: Option<tokio::time::Instant>,
    last: Option<tokio::time::Instant>,
    /// A task is writing, or waiting to.
    writer: bool,
}

impl<T: Send + 'static> Coalesced<T> {
    const fn new() -> Self {
        Self {
            slot: std::sync::Mutex::new(Pending {
                value: None,
                first: None,
                last: None,
                writer: false,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Pending<T>> {
        self.slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Ask for `value` to be written by `write`; a newer ask replaces it.
    fn request<F, Fut>(&'static self, value: T, write: F)
    where
        F: Fn(T) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        let now = tokio::time::Instant::now();
        {
            let mut p = self.lock();
            p.value = Some(value);
            p.first.get_or_insert(now);
            p.last = Some(now);
            if std::mem::replace(&mut p.writer, true) {
                return;
            }
        }
        tokio::spawn(async move {
            loop {
                // Wait for the asks to settle.
                let value = loop {
                    let due = {
                        let mut p = self.lock();
                        let (Some(first), Some(last)) = (p.first, p.last) else {
                            p.writer = false;
                            return;
                        };
                        let due = (last + CACHE_SETTLE).min(first + CACHE_MAX_WAIT);
                        if tokio::time::Instant::now() >= due {
                            p.first = None;
                            p.last = None;
                            match p.value.take() {
                                Some(v) => break v,
                                None => {
                                    p.writer = false;
                                    return;
                                }
                            }
                        }
                        due
                    };
                    tokio::time::sleep_until(due).await;
                };
                write(value).await;
            }
        });
    }
}

impl Hub {
    /// Whether this is the first launch (no profile yet), asked before
    /// `initialize` so the wizard can show at once.
    pub async fn check_first_launch(&self) -> bool {
        mocked!(self, w => !w.profile_path().exists() || crate::mock::switch("wizard"));
        if let Some(first) = self.take_first_launch() {
            return first;
        }
        first_launch().await
    }

    /// Start [`Hub::initialize`] now, before any window exists: the profile and
    /// the cached list load while the window and its GPU are set up. The
    /// window's own call then gets this first answer (not a `resumed` one).
    /// Needs a tokio runtime entered.
    pub fn prewarm(&self) {
        // Before `initialize` writes a default profile.
        self.note_first_launch();
        let hub = self.clone();
        let task = tokio::spawn(async move { initialize(&hub).await });
        if let Ok(mut p) = PREWARM.lock() {
            *p = Some(task);
        }
    }

    /// Create the controller and load the server list (see the module).
    /// Called when the window starts; a second call finds the state as it
    /// is, with the work running on it, and says `resumed`.
    pub async fn initialize(&self) -> Result<InitResult, String> {
        let early = PREWARM.lock().ok().and_then(|mut p| p.take());
        if let Some(task) = early
            && let Ok(r) = task.await
        {
            return r;
        }
        initialize(self).await
    }

    /// One server with its mods, by query address.
    pub async fn get_server_details(&self, addr: SocketAddrV4) -> Result<ServerDto, String> {
        get_server_details(self, addr).await
    }

    /// Whether the servers Steam's master lists are added to the list.
    pub async fn set_steam_server_list(&self, enabled: bool) -> Result<(), String> {
        set_steam_server_list(self, enabled).await
    }

    /// The title bar's counters.
    pub async fn get_app_stats(&self) -> Result<AppStatsDto, String> {
        get_app_stats(self).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::time::Duration;

    static WRITER: Coalesced<u32> = Coalesced::new();
    static WRITTEN: std::sync::Mutex<Vec<(u32, tokio::time::Instant)>> =
        std::sync::Mutex::new(Vec::new());

    fn ask(v: u32) {
        WRITER.request(v, |v| async move {
            // A slow write: asks meanwhile wait for the next one.
            tokio::time::sleep(Duration::from_secs(1)).await;
            WRITTEN
                .lock()
                .unwrap()
                .push((v, tokio::time::Instant::now()));
        });
    }

    #[tokio::test(start_paused = true)]
    async fn cache_writes_are_coalesced_ordered_and_the_newest_wins() {
        let t0 = tokio::time::Instant::now();
        // A burst: one write, of the last.
        for v in 1..=5 {
            ask(v);
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(
            WRITTEN
                .lock()
                .unwrap()
                .iter()
                .map(|w| w.0)
                .collect::<Vec<_>>(),
            [5]
        );
        // Asks that never settle are still written, at the longest wait.
        let t1 = tokio::time::Instant::now();
        for v in 10..50 {
            ask(v);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
        let written = WRITTEN.lock().unwrap().clone();
        assert!(written[0].1 - t0 < Duration::from_secs(8));
        assert!(written[1].1 - t1 <= CACHE_MAX_WAIT + Duration::from_secs(2));
        assert_eq!(written.last().unwrap().0, 49, "the newest lands last");
        assert!(written.windows(2).all(|w| w[0].0 < w[1].0), "{written:?}");
    }
}
