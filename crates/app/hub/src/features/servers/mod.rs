//! The server list: loading it (from cache or the API), refreshing it, and
//! the counters the title bar shows.

mod dto;
pub(crate) mod refresh;
pub(crate) mod sources;
mod steam;

pub use dto::{AppStatsDto, InitResult, ModDto, ServerDto};
pub(crate) use dto::{mods_to_dto, server_to_dto};

use crate::paths;
use dz_api::ServerList;
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

/// Create the controller and load the server list: from the on-disk cache
/// when there is one (the window refreshes it in the background), else from
/// the API. The profile and the cache are read at the same time.
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
    let cache_path = paths::server_list_cache_path();

    let (ctl, cache) = tokio::join!(
        DayzCtl::new(&profile_path),
        dz_api::load_server_list_cache(&cache_path)
    );
    let ctl = ctl.map_err(|e| format!("Could not load the profile: {e}"))?;
    crate::features::stats::open(&ctl.profile().history);

    // The cache holds the official servers merged last time; a cold start
    // fetches both lists at once.
    let (list, from_cache, list_error) = match cache {
        Some(cache) if !cache.list.result.is_empty() => {
            (steam::with_steam(dedup(cache.list)), true, None)
        }
        _ => {
            let key = ctl.profile().steam_api_key.clone();
            dzsa_starts();
            let (community, official) = tokio::join!(
                dz_api::fetch_servers(ctl.http_client()),
                fetch_official(ctl.http_client(), key.as_deref())
            );
            dzsa_done(&community);
            match community {
                Ok(list) => {
                    let list = steam::with_steam(dedup(with_official(list, official)));
                    save_cache_in_background(Arc::clone(&list));
                    (list, false, None)
                }
                Err(e) => (Arc::default(), false, Some(e.to_string())),
            }
        }
    };

    refresh::initialized(match &list_error {
        Some(e) => Err(e.clone()),
        None => Ok((!from_cache).then(crate::features::ping::now_ms)),
    });
    let server_count = list.result.len();
    let mut app_state = AppState::new(ctl);
    app_state.set_servers(Arc::clone(&list));
    hub.set_state(Arc::new(RwLock::new(app_state)));
    crate::features::browser::list_replaced(list);
    // The cached list too.
    steam::comb(hub.clone());
    // A cached start refreshes in the background, which starts it too; a
    // cold one does not.
    if !from_cache {
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
    let ctl = DayzCtl::new(world.profile_path())
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

/// One server with its mods, by query port.
async fn get_server_details(hub: &Hub, ip: String, port: i64) -> Result<ServerDto, String> {
    let state = hub.state()?;
    let state = state.read().await;
    let server = state
        .find_by_query_port(&ip, port)
        .ok_or_else(|| "Server not found".to_string())?;
    let mut dto = server_to_dto(server);
    // The map's one spelling, as the rows show it.
    if let Some(r) = crate::features::browser::live::store()
        .snapshot()
        .lookup(&ip, port)
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
    let (client, key) = {
        let s = state.read().await;
        (
            s.ctl.http_client().clone(),
            s.ctl.profile().steam_api_key.clone(),
        )
    };
    dzsa_starts();
    let (community, official) = tokio::join!(
        dz_api::fetch_servers(&client),
        fetch_official(&client, key.as_deref())
    );
    dzsa_done(&community);
    let fresh = dedup(with_official(community.cmd_err()?, official));
    let list = {
        let mut s = state.write().await;
        // What the list had and no longer lists stays while it answers.
        let list = steam::refreshed(&s.servers, fresh);
        s.set_servers(Arc::clone(&list));
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
        steam::start(hub.clone());
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
pub(super) fn save_cache_in_background(list: Arc<ServerList>) {
    tokio::spawn(async move {
        dz_api::save_server_list_cache(&paths::server_list_cache_path(), list).await;
    });
}

impl Hub {
    /// Whether this is the first launch (no profile yet), asked before
    /// `initialize` so the wizard can show at once.
    pub async fn check_first_launch(&self) -> bool {
        mocked!(self, w => !w.profile_path().exists() || crate::mock::switch("wizard"));
        first_launch().await
    }

    /// Create the controller and load the server list (see the module).
    /// Called when the window starts; a second call finds the state as it
    /// is, with the work running on it, and says `resumed`.
    pub async fn initialize(&self) -> Result<InitResult, String> {
        initialize(self).await
    }

    /// One server with its mods, by query port.
    pub async fn get_server_details(&self, ip: String, port: i64) -> Result<ServerDto, String> {
        get_server_details(self, ip, port).await
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
