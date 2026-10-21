//! Mod downloads through steamcmd, and finding steamcmd itself.

pub(crate) mod detect;
pub(crate) mod op;

use crate::{Hub, HubEvent};
use dz_game::ModOperation;
use dz_game::mods::InstalledMod;
use dz_steamcmd::ModProgress;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{ResultExt, spawn_blocking_mapped};
use crate::state::{SharedState, mutate_profile};

/// What a [`ModProgressEvent`] reports.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModProgressKind {
    /// Sent first, once: every mod the operation will work on, in order,
    /// named (`queue`). Empty for a login, or when there is nothing to do.
    Queued,
    /// SteamCMD logged in; its login is cached from now on.
    LoggedIn,
    SteamGuardMobileRequired,
    PasswordRequired,
    LogLine,
    LogProgress,
    Starting,
    Done,
    Failed,
    Finished,
}

/// One step of a mod operation, as the progress dialog receives it.
#[derive(Serialize, Clone, Debug)]
pub struct ModProgressEvent {
    pub kind: ModProgressKind,
    pub current: usize,
    pub total: usize,
    pub mod_id: u64,
    pub name: String,
    pub ok: usize,
    pub failed: usize,
    pub hint: Option<String>,
    pub log_line: Option<String>,
    /// Only on `queued`: the operation's mods with their names; empty on
    /// every other step.
    pub queue: Vec<QueuedModDto>,
    /// The step's number in its operation, from 1: a window that attached
    /// (`mod_op_attach`) drops the steps its snapshot already holds.
    pub seq: u32,
}

/// A mod an operation will work on, by the name it is best known by: its
/// files' (meta.cpp), the Workshop's title, a listed server's name for it,
/// else its id.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct QueuedModDto {
    pub id: u64,
    pub name: String,
}

impl ModProgressEvent {
    /// An event of `kind` with every other field empty.
    fn of(kind: ModProgressKind) -> Self {
        Self {
            kind,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
            queue: Vec::new(),
            seq: 0,
        }
    }

    /// A step about one mod: which, and how far the operation has got.
    fn about(
        kind: ModProgressKind,
        current: usize,
        total: usize,
        mod_id: u64,
        name: String,
    ) -> Self {
        Self {
            current,
            total,
            mod_id,
            name,
            ..Self::of(kind)
        }
    }
}

fn mod_progress_to_event(msg: &ModProgress) -> ModProgressEvent {
    use ModProgressKind as K;
    match msg {
        ModProgress::LoggedIn => ModProgressEvent::of(K::LoggedIn),
        ModProgress::SavedPasswordRefused => ModProgressEvent {
            log_line: Some("The saved password was refused and forgotten.".into()),
            ..ModProgressEvent::of(K::LogLine)
        },
        ModProgress::SteamGuardMobileRequired => ModProgressEvent::of(K::SteamGuardMobileRequired),
        ModProgress::PasswordRequired => ModProgressEvent::of(K::PasswordRequired),
        ModProgress::LogLine(line) => ModProgressEvent {
            log_line: Some(line.clone()),
            ..ModProgressEvent::of(K::LogLine)
        },
        ModProgress::LogProgress(line) => ModProgressEvent {
            log_line: Some(line.clone()),
            ..ModProgressEvent::of(K::LogProgress)
        },
        ModProgress::Starting {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent::about(K::Starting, *current, *total, *mod_id, name.clone()),
        ModProgress::Done {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent::about(K::Done, *current, *total, *mod_id, name.clone()),
        ModProgress::Failed {
            current,
            total,
            mod_id,
            name,
            error,
        } => ModProgressEvent::about(
            K::Failed,
            *current,
            *total,
            *mod_id,
            format!("{name} ({error})"),
        ),
        ModProgress::Finished {
            ok, failed, hint, ..
        } => ModProgressEvent {
            ok: *ok,
            failed: *failed,
            hint: hint.clone(),
            ..ModProgressEvent::of(K::Finished)
        },
    }
}

/// Which mod operation to run.
#[derive(serde::Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModOpType {
    /// Install what a listed server needs (`ip`, `port`).
    InstallServer,
    /// Re-download every mod a listed server uses (`ip`, `port`).
    UpdateServer,
    /// Re-download every installed mod.
    UpdateAll,
    /// Re-download the mods the last Workshop check found stale.
    UpdateStale,
    /// Re-download one mod (`mod_id`, `mod_name`).
    UpdateOne,
    /// Re-download these mods (`mod_ids`, optionally `mod_names`).
    UpdateSelected,
    /// Install these mods by Workshop id (`mod_ids`, `mod_names`).
    InstallManual,
    /// Download these mods again and check every file (`mod_ids`,
    /// optionally `mod_names`): slow, for a mod that does not load.
    Repair,
    /// Log SteamCMD in and quit, so later downloads use its cached login.
    Login,
}

/// Pair ids with names: the given ones, else (none given) the installed
/// mods', leaving out ids that are not installed.
fn resolve_names(
    ids: Vec<u64>,
    names: Option<Vec<String>>,
    installed: &[InstalledMod],
) -> Vec<(u64, String)> {
    if let Some(names) = names {
        return ids.into_iter().zip(names).collect();
    }
    installed
        .iter()
        .filter(|m| ids.contains(&m.id))
        .map(|m| (m.id, m.name.clone()))
        .collect()
}

/// A name that says nothing: empty, the id itself (the window passes ids as
/// names for a manual install), or what a `meta.cpp` without one reads as.
fn unnamed(id: u64, name: &str) -> bool {
    let name = name.trim();
    name.is_empty() || name == "Unknown" || name.parse::<u64>().is_ok_and(|n| n == id)
}

/// Names for the mods of `ids` that have none worth showing: the copy on
/// disk's, else the Workshop's title (when Steam was asked this run), else
/// the name a listed server gives it.
fn names_for(
    ids: &FxHashSet<u64>,
    installed: &[InstalledMod],
    list: &dz_api::ServerList,
) -> FxHashMap<u64, String> {
    let mut found: FxHashMap<u64, String> = installed
        .iter()
        .filter(|m| ids.contains(&m.id) && !unnamed(m.id, &m.name))
        .map(|m| (m.id, m.name.clone()))
        .collect();
    for &id in ids {
        if !found.contains_key(&id)
            && let Some(title) = crate::features::steamworks::workshop_title(id)
        {
            found.insert(id, title);
        }
    }
    if found.len() < ids.len() {
        'list: for s in &list.result {
            for m in &s.mods {
                let id = m.steam_workshop_id as u64;
                if ids.contains(&id) && !unnamed(id, &m.name) && !found.contains_key(&id) {
                    found.insert(id, m.name.trim().to_owned());
                    if found.len() == ids.len() {
                        break 'list;
                    }
                }
            }
        }
    }
    found
}

/// Give every mod of `op` that has no name worth showing the best one known,
/// so its progress steps are named too.
fn name_op(op: &mut ModOperation, installed: &[InstalledMod], list: &dz_api::ServerList) {
    let mut wanted = FxHashSet::default();
    let mut want = |id: u64, name: &mut String| {
        if unnamed(id, name) {
            wanted.insert(id);
        }
    };
    match op {
        ModOperation::InstallServer { server } | ModOperation::UpdateServer { server } => {
            for m in &mut server.mods {
                want(m.steam_workshop_id as u64, &mut m.name);
            }
        }
        ModOperation::UpdateStale { stale_mods: mods }
        | ModOperation::UpdateSelected { mods }
        | ModOperation::InstallManual { mods }
        | ModOperation::Repair { mods } => {
            for (id, name) in mods.iter_mut() {
                want(*id, name);
            }
        }
        ModOperation::UpdateOne { mod_id, name } => want(*mod_id, name),
        ModOperation::UpdateAll | ModOperation::Login => {}
    }
    if wanted.is_empty() {
        return;
    }
    let found = names_for(&wanted, installed, list);
    let fill = |id: u64, name: &mut String| {
        if unnamed(id, name) {
            *name = found.get(&id).cloned().unwrap_or_else(|| id.to_string());
        }
    };
    match op {
        ModOperation::InstallServer { server } | ModOperation::UpdateServer { server } => {
            for m in &mut server.mods {
                fill(m.steam_workshop_id as u64, &mut m.name);
            }
        }
        ModOperation::UpdateStale { stale_mods: mods }
        | ModOperation::UpdateSelected { mods }
        | ModOperation::InstallManual { mods }
        | ModOperation::Repair { mods } => {
            for (id, name) in mods.iter_mut() {
                fill(*id, name);
            }
        }
        ModOperation::UpdateOne { mod_id, name } => fill(*mod_id, name),
        ModOperation::UpdateAll | ModOperation::Login => {}
    }
}

/// The mods `op` will download, in the order it will, as `dz_game`'s
/// operation works them out: a server's or a manual install's only those
/// not on disk, an update all every installed one.
fn queue_of(op: &ModOperation, installed: &[InstalledMod]) -> Vec<QueuedModDto> {
    let on_disk: FxHashSet<u64> = installed.iter().map(|m| m.id).collect();
    let q = |id: u64, name: &str| QueuedModDto {
        id,
        name: if name.trim().is_empty() {
            id.to_string()
        } else {
            name.to_owned()
        },
    };
    match op {
        ModOperation::InstallServer { server } => {
            // Its first entry names a mod listed twice, as the operation does.
            let mut ids: Vec<u64> = server.mod_ids();
            ids.retain(|id| !on_disk.contains(id));
            ids.iter()
                .map(|&id| {
                    let name = server
                        .mods
                        .iter()
                        .find(|m| m.steam_workshop_id as u64 == id)
                        .map_or("", |m| m.name.as_str());
                    q(id, name)
                })
                .collect()
        }
        ModOperation::UpdateServer { server } => server
            .mods
            .iter()
            .map(|m| q(m.steam_workshop_id as u64, &m.name))
            .collect(),
        ModOperation::UpdateAll => installed.iter().map(|m| q(m.id, &m.name)).collect(),
        ModOperation::UpdateOne { mod_id, name } => vec![q(*mod_id, name)],
        ModOperation::UpdateStale { stale_mods: mods }
        | ModOperation::UpdateSelected { mods }
        | ModOperation::Repair { mods } => mods.iter().map(|(id, n)| q(*id, n)).collect(),
        ModOperation::InstallManual { mods } => mods
            .iter()
            .filter(|(id, _)| !on_disk.contains(id))
            .map(|(id, n)| q(*id, n))
            .collect(),
        ModOperation::Login => Vec::new(),
    }
}

/// A `log_progress` line is held back this long after the last one sent:
/// SteamCMD rewrites its progress line on every read of its output, and the
/// window only ever shows the latest.
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// What a session says about SteamCMD's login, kept in the profile:
/// logged in, the account is remembered and a password saved by an earlier
/// version is forgotten (SteamCMD caches the login itself); asked for a
/// password, its cached login is gone.
async fn remember_login(state: &SharedState, msg: &ModProgress) {
    let _ = match msg {
        ModProgress::LoggedIn => {
            mutate_profile(state, |s| {
                let p = s.ctl.profile_mut();
                p.steamcmd_logged_in = p.steam_login.clone();
                if p.steam_password.take().is_some() {
                    s.ctl.rebuild_steamcmd();
                }
                Ok(())
            })
            .await
        }
        ModProgress::SavedPasswordRefused => {
            mutate_profile(state, |s| {
                s.ctl.profile_mut().steam_password = None;
                s.ctl.rebuild_steamcmd();
                Ok(())
            })
            .await
        }
        ModProgress::PasswordRequired => {
            mutate_profile(state, |s| {
                s.ctl.profile_mut().steamcmd_logged_in = None;
                Ok(())
            })
            .await
        }
        _ => Ok(()),
    };
}

/// Where SteamCMD keeps its things: the launcher's, never the Steam client's.
#[derive(Serialize, Clone, Debug)]
pub struct SteamcmdDirsDto {
    /// Its install directory, where mods download.
    pub content: String,
    /// The `HOME` it runs with (Linux), where its login is cached; null on
    /// Windows, where it keeps everything beside its executable.
    pub home: Option<String>,
}

impl Hub {
    /// Start a mod operation in the background; its progress goes out as
    /// `HubEvent::ModProgress`, starting with a `queued` step that names every mod it will
    /// work on and ending with a `finished` step. The backend keeps the
    /// operation's state meanwhile (`op`), which a reloaded window gets back
    /// with `mod_op_attach`.
    // One parameter per field, as the Tauri command took them.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_mod_operation(
        &self,
        op_type: ModOpType,
        ip: Option<String>,
        port: Option<i64>,
        mod_id: Option<u64>,
        mod_name: Option<String>,
        mod_ids: Option<Vec<u64>>,
        mod_names: Option<Vec<String>>,
    ) -> Result<(), String> {
        mocked!(self, w => w.start_mod_operation(self, op_type, mod_ids).await);
        let state = self.state()?;
        // One operation at a time: two steamcmd sessions fight over the login,
        // and the first would lose its input channel. Checked first, so a
        // refused start costs no disk scan.
        let (ctl, update_cache, list, downloads_through_steam) = {
            let s = state.read().await;
            if s.mod_op_abort.is_some() {
                return Err("A mod operation is already running".into());
            }
            (
                s.ctl.clone_for_task(),
                s.mod_update_cache.clone(),
                Arc::clone(&s.servers),
                s.ctl.downloads_through_steam(),
            )
        };
        // What is on disk, read once for everything below (which mods are
        // stale or missing, their names), off the runtime and outside the lock.
        let installed = if op_type == ModOpType::Login {
            Vec::new()
        } else {
            let ctl = ctl.clone_for_task();
            spawn_blocking_mapped(move || ctl.get_installed_mods())
                .await
                .unwrap_or_default()
        };

        let mut op = match op_type {
            ModOpType::InstallServer | ModOpType::UpdateServer => {
                let ip = ip.ok_or("ip required")?;
                let port = port.ok_or("port required")?;
                let server = state
                    .read()
                    .await
                    .find_by_query_port(&ip, port)
                    .cloned()
                    .ok_or_else(|| "Server not found".to_string())?;
                if op_type == ModOpType::InstallServer {
                    ModOperation::InstallServer { server }
                } else {
                    ModOperation::UpdateServer { server }
                }
            }
            ModOpType::UpdateAll => ModOperation::UpdateAll,
            ModOpType::UpdateStale => ModOperation::UpdateStale {
                stale_mods: installed
                    .iter()
                    .filter(|m| {
                        update_cache
                            .get(&m.id)
                            .is_some_and(|&r| r > m.local_updated)
                    })
                    .map(|m| (m.id, m.name.clone()))
                    .collect(),
            },
            ModOpType::UpdateOne => ModOperation::UpdateOne {
                mod_id: mod_id.ok_or("mod_id required")?,
                name: mod_name.unwrap_or_default(),
            },
            ModOpType::UpdateSelected => ModOperation::UpdateSelected {
                mods: resolve_names(mod_ids.unwrap_or_default(), mod_names, &installed),
            },
            ModOpType::Repair => ModOperation::Repair {
                mods: resolve_names(mod_ids.unwrap_or_default(), mod_names, &installed),
            },
            ModOpType::Login => ModOperation::Login,
            ModOpType::InstallManual => ModOperation::InstallManual {
                mods: mod_ids
                    .unwrap_or_default()
                    .into_iter()
                    .zip(mod_names.unwrap_or_default())
                    .collect(),
            },
        };
        name_op(&mut op, &installed, &list);
        drop(list);
        let queue = queue_of(&op, &installed);

        // Starting scans the workshop directory: off the runtime, and without
        // holding the state's lock.
        let (mut rx, pty_input_tx, handle) =
            spawn_blocking_mapped(move || ctl.start_mod_operation(op)).await?;
        let ours = pty_input_tx.clone();
        {
            let mut s = state.write().await;
            // Another start won the race while this one read the disk.
            if s.mod_op_abort.is_some() {
                handle.abort();
                return Err("A mod operation is already running".into());
            }
            s.pty_input_tx = Some(pty_input_tx);
            s.mod_op_abort = Some(handle.abort_handle());
        }
        // A login is always SteamCMD's (see `DayzCtl::start_mod_operation`).
        let via = if op_type != ModOpType::Login && downloads_through_steam {
            op::ModOpVia::Steamworks
        } else {
            op::ModOpVia::Steamcmd
        };
        let sink: op::StepSink = {
            let hub = self.clone();
            Arc::new(move |ev| hub.emit(HubEvent::ModProgress(Box::new(ev))))
        };
        let id = op::begin(op_type, via, sink);
        op::emit(
            id,
            ModProgressEvent {
                queue,
                ..ModProgressEvent::of(ModProgressKind::Queued)
            },
        );

        let state = state.clone();
        tokio::spawn(async move {
            // The latest progress line not sent yet, and when one last went.
            let mut held: Option<ModProgressEvent> = None;
            let mut last_progress: Option<Instant> = None;
            loop {
                let next = match &held {
                    // Wait for the next message only until the held line is due.
                    Some(_) => {
                        let due = last_progress.map_or(Duration::ZERO, |t| {
                            PROGRESS_EVERY.saturating_sub(t.elapsed())
                        });
                        match tokio::time::timeout(due, rx.recv()).await {
                            Ok(m) => m,
                            Err(_) => {
                                if let Some(evt) = held.take() {
                                    last_progress = Some(Instant::now());
                                    op::emit(id, evt);
                                }
                                continue;
                            }
                        }
                    }
                    None => rx.recv().await,
                };
                let Some(msg) = next else { break };
                remember_login(&state, &msg).await;
                let evt = mod_progress_to_event(&msg);
                if evt.kind == ModProgressKind::LogProgress {
                    if last_progress.is_some_and(|t| t.elapsed() < PROGRESS_EVERY) {
                        held = Some(evt);
                        continue;
                    }
                    held = None;
                    last_progress = Some(Instant::now());
                    op::emit(id, evt);
                    continue;
                }
                // Anything else goes after the progress line it followed.
                if let Some(h) = held.take() {
                    op::emit(id, h);
                }
                let finished = evt.kind == ModProgressKind::Finished;
                op::emit(id, evt);
                if finished {
                    break;
                }
            }
            // Cleared only if still this operation's: after a cancel, a new one
            // may have started before this loop saw its channel close.
            let mut st = state.write().await;
            if st
                .pty_input_tx
                .as_ref()
                .is_some_and(|tx| tx.same_channel(&ours))
            {
                st.pty_input_tx = None;
                st.mod_op_abort = None;
            }
        });

        Ok(())
    }

    /// Send input (password or Steam Guard code) to the running steamcmd PTY.
    pub async fn send_steamcmd_input(&self, input: String) -> Result<(), String> {
        mocked!(self => Ok(()));
        let state = self.state()?;
        let state = state.read().await;
        if let Some(ref tx) = state.pty_input_tx {
            tx.send(input)
                .map_err(|e| format!("Failed to send input to steamcmd: {e}"))?;
        } else {
            return Err("No active steamcmd session".into());
        }
        Ok(())
    }

    /// Cancel the running mod operation, and forget it.
    pub async fn cancel_mod_operation(&self) -> Result<(), String> {
        let state = self.state()?;
        let mut state = state.write().await;
        if let Some(abort) = state.mod_op_abort.take() {
            abort.abort();
        }
        state.pty_input_tx = None;
        op::clear();
        Ok(())
    }

    /// The mod operation as it stands, running or finished and not dismissed;
    /// null when there is none.
    pub async fn mod_op_state(&self) -> Option<op::ModOpSnapshot> {
        op::snapshot()
    }

    /// Follow the mod operation from a window that starts following it late:
    /// the operation as it stands. Its later steps arrive as
    /// `HubEvent::ModProgress`; those numbered at or below the snapshot's `seq`
    /// are already in it. `None` when there is no operation.
    pub async fn mod_op_attach(&self) -> Option<op::ModOpSnapshot> {
        op::attach()
    }

    /// The finished operation's dialog was closed: forget it. A running one is
    /// kept (cancel it to stop it).
    pub async fn mod_op_dismiss(&self) {
        op::dismiss();
    }

    /// Where SteamCMD downloads and keeps its login.
    pub async fn steamcmd_dirs(&self) -> Result<SteamcmdDirsDto, String> {
        mocked!(self => Ok(crate::mock::fixtures::steamcmd_dirs()));
        let state = self.state()?;
        let content = state.read().await.ctl.steamcmd_content_dir();
        Ok(SteamcmdDirsDto {
            content: content.to_string_lossy().into_owned(),
            home: cfg!(unix).then(|| {
                dz_common::paths::steamcmd_home_dir()
                    .to_string_lossy()
                    .into_owned()
            }),
        })
    }

    /// Open SteamCMD's install directory in the system file manager.
    /// Returns the folder: the window opens it.
    pub async fn open_steamcmd_dir(&self) -> Result<std::path::PathBuf, String> {
        mocked!(self, w => w.folder("steamcmd-content"));
        let state = self.state()?;
        let dir = state.read().await.ctl.steamcmd_content_dir();
        tokio::fs::create_dir_all(&dir).await.cmd_err()?;
        Ok(dir)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;

    fn on_disk(id: u64, name: &str) -> InstalledMod {
        InstalledMod {
            name: name.into(),
            id,
            local_updated: 0,
            size: 0,
            managed: false,
            source: dz_game::mods::ModSource::Launcher,
            path: Default::default(),
            other_copy: false,
        }
    }

    fn listed(mods: &[(i64, &str)]) -> dz_api::Server {
        dz_api::Server {
            mods: mods
                .iter()
                .map(|&(id, name)| dz_api::Mod {
                    name: name.into(),
                    steam_workshop_id: id,
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn a_manual_install_is_named_and_queues_only_what_is_missing() {
        let installed = vec![on_disk(1, "CF")];
        let list = dz_api::ServerList {
            status: 0,
            result: vec![listed(&[(2, "Expansion"), (3, "")])],
        };
        let mut op = ModOperation::InstallManual {
            mods: vec![(1, "1".into()), (2, "2".into()), (3, "3".into())],
        };
        name_op(&mut op, &installed, &list);
        let ModOperation::InstallManual { mods } = &op else {
            unreachable!()
        };
        assert_eq!(
            mods,
            &[(1, "CF".into()), (2, "Expansion".into()), (3, "3".into())]
        );
        let queue = queue_of(&op, &installed);
        assert_eq!(
            queue,
            [
                QueuedModDto {
                    id: 2,
                    name: "Expansion".into()
                },
                QueuedModDto {
                    id: 3,
                    name: "3".into()
                },
            ]
        );
    }

    #[test]
    fn a_servers_install_queues_its_missing_mods_in_its_order() {
        let installed = vec![on_disk(1, "CF")];
        let op = ModOperation::InstallServer {
            server: listed(&[(5, "B"), (1, "CF"), (4, "A")]),
        };
        let ids: Vec<u64> = queue_of(&op, &installed).iter().map(|q| q.id).collect();
        assert_eq!(ids, [5, 4]);
        assert!(queue_of(&ModOperation::Login, &installed).is_empty());
    }

    #[test]
    fn names_that_say_nothing() {
        assert!(unnamed(7, ""));
        assert!(unnamed(7, " 7 "));
        assert!(unnamed(7, "Unknown"));
        assert!(!unnamed(7, "8"));
        assert!(!unnamed(7, "@CF"));
    }
}
