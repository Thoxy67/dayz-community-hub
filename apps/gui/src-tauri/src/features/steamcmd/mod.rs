//! Mod downloads through steamcmd, and finding steamcmd itself.

pub(crate) mod detect;

use dz_game::ModOperation;
use dz_steamcmd::ModProgress;
use serde::Serialize;
use tauri::{State, ipc::Channel};

use crate::error::{ResultExt, spawn_blocking_mapped};
use crate::state::SharedState;

/// What a [`ModProgressEvent`] reports.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ModProgressKind {
    ShuttingDownSteam,
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
#[derive(Serialize, Clone, Debug, specta::Type)]
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
}

fn mod_progress_to_event(msg: &ModProgress) -> ModProgressEvent {
    match msg {
        ModProgress::ShuttingDownSteam => ModProgressEvent {
            kind: ModProgressKind::ShuttingDownSteam,
            current: 0,
            total: 0,
            mod_id: 0,
            name: "Closing Steam...".into(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::SteamGuardMobileRequired => ModProgressEvent {
            kind: ModProgressKind::SteamGuardMobileRequired,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::PasswordRequired => ModProgressEvent {
            kind: ModProgressKind::PasswordRequired,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::LogLine(line) => ModProgressEvent {
            kind: ModProgressKind::LogLine,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: Some(line.clone()),
        },
        ModProgress::LogProgress(line) => ModProgressEvent {
            kind: ModProgressKind::LogProgress,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: Some(line.clone()),
        },
        ModProgress::Starting {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: ModProgressKind::Starting,
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Done {
            current,
            total,
            mod_id,
            name,
        } => ModProgressEvent {
            kind: ModProgressKind::Done,
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: name.clone(),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Failed {
            current,
            total,
            mod_id,
            name,
            error,
        } => ModProgressEvent {
            kind: ModProgressKind::Failed,
            current: *current,
            total: *total,
            mod_id: *mod_id,
            name: format!("{} ({})", name, error),
            ok: 0,
            failed: 0,
            hint: None,
            log_line: None,
        },
        ModProgress::Finished {
            ok,
            failed,
            total: _,
            hint,
        } => ModProgressEvent {
            kind: ModProgressKind::Finished,
            current: 0,
            total: 0,
            mod_id: 0,
            name: String::new(),
            ok: *ok,
            failed: *failed,
            hint: hint.clone(),
            log_line: None,
        },
    }
}

/// Which mod operation to run.
#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
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
}

/// Pair ids with names, or with the installed mods' names when none are given.
async fn resolve_names(
    ids: Vec<u64>,
    names: Option<Vec<String>>,
    state: &SharedState,
) -> Result<Vec<(u64, String)>, String> {
    if let Some(names) = names {
        return Ok(ids.into_iter().zip(names).collect());
    }
    let ctl = state.read().await.ctl.clone_for_task();
    let installed = spawn_blocking_mapped(move || ctl.get_installed_mods())
        .await
        .unwrap_or_default();
    Ok(installed
        .into_iter()
        .filter(|m| ids.contains(&m.id))
        .map(|m| (m.id, m.name))
        .collect())
}

/// Start a mod operation in the background; its progress streams over
/// `on_progress` and ends with a `finished` step.
// One parameter per field: Tauri receives a command's arguments as one flat
// object.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn start_mod_operation(
    op_type: ModOpType,
    ip: Option<String>,
    port: Option<i64>,
    mod_id: Option<u64>,
    mod_name: Option<String>,
    mod_ids: Option<Vec<u64>>,
    mod_names: Option<Vec<String>>,
    on_progress: Channel<ModProgressEvent>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    // Anything that walks the workshop directory runs before the write lock,
    // so other commands are not held up by the scan.
    let op = match op_type {
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
        ModOpType::UpdateStale => {
            let (update_cache, ctl) = {
                let s = state.read().await;
                (s.mod_update_cache.clone(), s.ctl.clone_for_task())
            };
            let installed = spawn_blocking_mapped(move || ctl.get_installed_mods())
                .await
                .unwrap_or_default();
            let stale_mods = installed
                .into_iter()
                .filter(|m| {
                    update_cache
                        .get(&m.id)
                        .is_some_and(|&r| r > m.local_updated)
                })
                .map(|m| (m.id, m.name))
                .collect();
            ModOperation::UpdateStale { stale_mods }
        }
        ModOpType::UpdateOne => ModOperation::UpdateOne {
            mod_id: mod_id.ok_or("mod_id required")?,
            name: mod_name.unwrap_or_default(),
        },
        ModOpType::UpdateSelected => ModOperation::UpdateSelected {
            mods: resolve_names(mod_ids.unwrap_or_default(), mod_names, &state).await?,
        },
        ModOpType::InstallManual => ModOperation::InstallManual {
            mods: mod_ids
                .unwrap_or_default()
                .into_iter()
                .zip(mod_names.unwrap_or_default())
                .collect(),
        },
    };

    let mut rx = {
        let mut state = state.write().await;
        let (rx, pty_input_tx, handle) = state.ctl.start_mod_operation(op).cmd_err()?;
        state.pty_input_tx = Some(pty_input_tx);
        state.mod_op_abort = Some(handle.abort_handle());
        rx
    };

    let state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let evt = mod_progress_to_event(&msg);
            let finished = evt.kind == ModProgressKind::Finished;
            let _ = on_progress.send(evt);
            if finished {
                break;
            }
        }
        let mut st = state.write().await;
        st.pty_input_tx = None;
        st.mod_op_abort = None;
    });

    Ok(())
}

/// Send input (password or Steam Guard code) to the running steamcmd PTY.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_steamcmd_input(
    input: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let state = state.read().await;
    if let Some(ref tx) = state.pty_input_tx {
        tx.send(input)
            .map_err(|e| format!("Failed to send input to steamcmd: {}", e))?;
    } else {
        return Err("No active steamcmd session".into());
    }
    Ok(())
}

/// Cancel the running mod operation.
#[tauri::command]
#[specta::specta]
pub(crate) async fn cancel_mod_operation(state: State<'_, SharedState>) -> Result<(), String> {
    let mut state = state.write().await;
    if let Some(abort) = state.mod_op_abort.take() {
        abort.abort();
    }
    state.pty_input_tx = None;
    Ok(())
}
