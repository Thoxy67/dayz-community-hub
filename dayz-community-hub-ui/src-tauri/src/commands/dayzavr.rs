use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dayz_community_hub_core::api::{Endpoint, Server};
use dayz_community_hub_core::dayzavr::{self, DayzavrServer};
use dayz_community_hub_core::system;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, PeerConnectionOptions, Session,
    SessionOptions,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::helpers::spawn_launch;
use crate::state::{SharedState, insecure_client};

/// Active-install bookkeeping. `gen` is bumped on every new install and on
/// cancel, so a previous install's polling loop can detect it's been superseded
/// and exit cleanly (fixes "can't restart after stopping").
#[derive(Default)]
pub(crate) struct DayzavrInstallInner {
    generation: u64,
    session: Option<Arc<Session>>,
}

pub(crate) struct DayzavrInstallState(pub Mutex<DayzavrInstallInner>);

impl DayzavrInstallState {
    pub(crate) fn new() -> Self {
        Self(Mutex::new(DayzavrInstallInner::default()))
    }
}

/// Progress streamed to the UI during a mod install.
#[derive(Clone, Default, Serialize)]
pub(crate) struct DayzavrInstallProgress {
    pub status: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    /// Download speed in MiB/s.
    pub download_mbps: f64,
    /// Upload speed in MiB/s.
    pub upload_mbps: f64,
    /// Connected (live) peers actively transferring.
    pub peers_live: u32,
    /// Peers currently being connected to.
    pub peers_connecting: u32,
    /// Total peers discovered from trackers (sources).
    pub peers_seen: u32,
    /// Human-readable ETA, when known.
    pub eta: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}

fn send_status(ch: &Channel<DayzavrInstallProgress>, status: &str, done: bool, err: Option<String>) {
    let _ = ch.send(DayzavrInstallProgress {
        status: status.to_string(),
        done,
        error: err,
        ..Default::default()
    });
}

/// Download/update only the mods a DayZavr server requires into
/// `<dayz_path>/!Workshop` via the mods-only torrent (selective download).
///
/// Only `@Mod/...` files are fetched (`only_files_regex`); the game-bundle
/// torrent is never touched. Intended for users who own DayZ on Steam.
#[tauri::command]
pub(crate) async fn install_dayzavr_mods(
    mods: Vec<String>,
    dayz_path: String,
    on_progress: Channel<DayzavrInstallProgress>,
    install: State<'_, DayzavrInstallState>,
) -> Result<(), String> {
    let dayz = PathBuf::from(&dayz_path);
    if !dayz.is_dir() {
        return Err(format!("DayZ folder not found: {dayz_path}"));
    }

    // Claim this install: bump the generation and drop any previous session so an
    // older polling loop exits. Our `my_gen` lets us detect if we get superseded.
    let my_gen = {
        let mut g = install.0.lock().await;
        g.generation += 1;
        g.session = None;
        g.generation
    };

    // Always fetch the live manifest and only download mods that are missing or
    // out of date — already-complete mods are skipped entirely. (Partial mods
    // resume: librqbit verifies existing pieces, fetches only what's missing.)
    send_status(&on_progress, "Checking installed mods…", false, None);
    let manifest = dayzavr::fetch_manifest(insecure_client())
        .await
        .map_err(|e| e.to_string())?;
    let to_download = dayzavr::mods_needing_download(&dayz, &manifest, &mods);
    if to_download.is_empty() {
        send_status(&on_progress, "Already up to date", true, None);
        let mut g = install.0.lock().await;
        if g.generation == my_gen {
            g.session = None;
        }
        return Ok(());
    }
    let regex =
        dayzavr::mods_only_regex(&to_download).ok_or_else(|| "No mods to install".to_string())?;

    send_status(&on_progress, "Fetching torrent…", false, None);
    let bytes = insecure_client()
        .get(format!("{}/DayZavr.torrent", dayzavr::MODS_BASE))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;

    send_status(&on_progress, "Connecting to swarm…", false, None);
    // Maximise peer discovery + throughput for the 129 GB public swarm:
    // - DHT ON (extra peer source) but persistence OFF (the on-disk DHT state
    //   init is what failed before, not DHT itself).
    // - UPnP ON so peers can also connect inbound (non-fatal if unsupported).
    // - No session persistence (one-shot install).
    // - Short connect timeout so the many dead/unreachable peers in the tracker
    //   set (1000+ "seen", few "live") free their slot fast and we cycle to the
    //   real seeders quickly.
    let session = Session::new_with_opts(
        dayz.clone(),
        SessionOptions {
            disable_dht: false,
            disable_dht_persistence: true,
            persistence: None,
            enable_upnp_port_forwarding: true,
            peer_opts: Some(PeerConnectionOptions {
                connect_timeout: Some(Duration::from_secs(4)),
                read_write_timeout: Some(Duration::from_secs(10)),
                keep_alive_interval: None,
            }),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    // Store our session only if we're still the current install.
    {
        let mut g = install.0.lock().await;
        if g.generation != my_gen {
            return Ok(()); // superseded before we even started
        }
        g.session = Some(session.clone());
    }

    // `output_folder` and `sub_folder` are mutually exclusive. The session's
    // default output folder is `<dayz>`, so `sub_folder = "!Workshop"` lands the
    // mods at `<dayz>/!Workshop/@Mod/...` (DayZ's mod folder).
    let resp = session
        .add_torrent(
            AddTorrent::from_bytes(bytes.to_vec()),
            Some(AddTorrentOptions {
                only_files_regex: Some(regex),
                sub_folder: Some("!Workshop".to_string()),
                overwrite: true,
                ..Default::default()
            }),
        )
        .await
        .map_err(|e| e.to_string())?;

    let handle = match resp {
        AddTorrentResponse::Added(_, h) => h,
        _ => {
            let mut g = install.0.lock().await;
            if g.generation == my_gen {
                g.session = None;
            }
            return Err("Unexpected torrent response".into());
        }
    };

    loop {
        // Superseded by a new install or cancelled?
        if install.0.lock().await.generation != my_gen {
            send_status(&on_progress, "Cancelled", true, None);
            return Ok(());
        }

        let st = handle.stats();
        let err = st.error.clone();
        let finished = st.finished || err.is_some();
        let live = st.live.as_ref();
        let _ = on_progress.send(DayzavrInstallProgress {
            status: format!("{:?}", st.state),
            downloaded_bytes: st.progress_bytes,
            total_bytes: st.total_bytes,
            uploaded_bytes: st.uploaded_bytes,
            download_mbps: live.map(|l| l.download_speed.mbps).unwrap_or(0.0),
            upload_mbps: live.map(|l| l.upload_speed.mbps).unwrap_or(0.0),
            peers_live: live.map(|l| l.snapshot.peer_stats.live as u32).unwrap_or(0),
            peers_connecting: live
                .map(|l| l.snapshot.peer_stats.connecting as u32)
                .unwrap_or(0),
            peers_seen: live.map(|l| l.snapshot.peer_stats.seen as u32).unwrap_or(0),
            eta: live.and_then(|l| l.time_remaining.as_ref().map(|t| t.to_string())),
            done: finished,
            error: err,
        });

        if finished {
            break;
        }
        tokio::time::sleep(Duration::from_millis(700)).await;
    }

    {
        let mut g = install.0.lock().await;
        if g.generation == my_gen {
            g.session = None;
        }
    }
    Ok(())
}

/// Cancel an in-flight DayZavr mod install (bumps the generation so the polling
/// loop exits and drops the session, stopping the torrent).
#[tauri::command]
pub(crate) async fn cancel_dayzavr_install(
    install: State<'_, DayzavrInstallState>,
) -> Result<(), String> {
    let mut g = install.0.lock().await;
    g.generation += 1;
    g.session = None;
    Ok(())
}

/// Remove all DayZavr-installed mods (folders re-signed with the DayZavr key)
/// from `<dayz_path>/!Workshop`. Returns the removed mod names.
#[tauri::command]
pub(crate) async fn clear_dayzavr_mods(dayz_path: String) -> Result<Vec<String>, String> {
    let dayz = PathBuf::from(&dayz_path);
    dayzavr::clear_installed_mods(&dayz).map_err(|e| e.to_string())
}

/// Names of DayZavr mod folders that are fully installed (complete per the live
/// manifest) under `<dayz_path>/!Workshop`. The UI uses this to only show "Play"
/// when every mod a server requires is present.
#[tauri::command]
pub(crate) async fn dayzavr_installed_mods(dayz_path: String) -> Result<Vec<String>, String> {
    let dayz = PathBuf::from(&dayz_path);
    if !dayz.is_dir() {
        return Ok(vec![]);
    }
    let manifest = dayzavr::fetch_manifest(insecure_client())
        .await
        .map_err(|e| e.to_string())?;
    Ok(dayzavr::installed_complete_mods(&dayz, &manifest))
}

/// Fetch the public DayZavr community server list.
#[tauri::command]
pub(crate) async fn fetch_dayzavr_servers() -> Result<Vec<DayzavrServer>, String> {
    dayzavr::fetch_servers(insecure_client())
        .await
        .map_err(|e| e.to_string())
}

/// Launch DayZ and connect to a DayZavr server, loading the server's mods from
/// `!Workshop` (where `install_dayzavr_mods` placed them). Reuses the normal
/// Steam launch path; the DayZavr mod set is passed as a `-mod=` extra arg.
#[tauri::command]
pub(crate) async fn launch_dayzavr_server(
    host: String,
    game_port: u16,
    password: Option<String>,
    mods: Vec<String>,
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    // DayZ's `-connect` wants an IP; resolve the regional hostname.
    let ip = tokio::net::lookup_host((host.as_str(), game_port))
        .await
        .map_err(|e| format!("Could not resolve {host}: {e}"))?
        .find(std::net::SocketAddr::is_ipv4)
        .map(|a| a.ip().to_string())
        .ok_or_else(|| format!("No address found for {host}"))?;

    let (ctl, dayz_path) = {
        let st = state.read().await;
        (
            st.ctl.clone_for_launch(),
            st.ctl.profile().dayzavr_dayz_path.clone(),
        )
    };

    // Symlink the mods from `!Workshop` into the DayZ root and launch with plain
    // `-mod=@ModName` — exactly how the normal (working) launch loads mods. This
    // avoids the `!Workshop\@ModName` path, which DayZ under Proton often fails to
    // resolve, leaving the client unmodded and BattlEye-kicked.
    let extra: Vec<String> = match (&dayz_path, mods.is_empty()) {
        (_, true) => vec![],
        (Some(p), false) => {
            let available = dayzavr::link_mods_to_root(std::path::Path::new(p), &mods);
            if available.is_empty() {
                return Err("No installed mods found to launch with".into());
            }
            vec![format!("-mod={}", available.join(";"))]
        }
        (None, false) => return Err("DayZ path not set".into()),
    };

    let server = Server {
        endpoint: Endpoint {
            ip,
            port: game_port as i64,
        },
        name: host.clone(),
        game_port: game_port as i64,
        mods: vec![],
        ..Default::default()
    };

    spawn_launch(app, ctl, server, password, extra);
    Ok(())
}

/// Try to auto-detect the DayZ install directory via Steam (libraries, Flatpak,
/// common paths). Returns the path string, or `None` if not found.
#[tauri::command]
pub(crate) async fn detect_dayz_path(state: State<'_, SharedState>) -> Result<Option<String>, String> {
    let hint = {
        let s = state.read().await;
        s.ctl.profile().steam_root.clone()
    };
    Ok(system::detect_dayz_path(hint.as_deref()).map(|p| p.to_string_lossy().into_owned()))
}
