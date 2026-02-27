use dayz_community_hub_core::ctl::ModOperation;
use tauri::{State, ipc::Channel};

use crate::convert::mod_progress_to_event;
use crate::dto::ModProgressEvent;
use crate::helpers::find_server_in;
use crate::state::SharedState;

/// Start a background mod operation. Streams progress via Channel.
#[tauri::command]
pub(crate) async fn start_mod_operation(
    op_type: String,
    ip: Option<String>,
    port: Option<i64>,
    mod_id: Option<u64>,
    mod_name: Option<String>,
    mod_ids: Option<Vec<u64>>,
    mod_names: Option<Vec<String>>,
    on_progress: Channel<ModProgressEvent>,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    // For operations that require a filesystem scan, do it BEFORE acquiring the
    // lock so we don't block other IPC commands during the workshop directory walk.
    let pre_scan: Option<Vec<(u64, String)>> = match op_type.as_str() {
        "update_stale" => {
            let (update_cache, ctl_clone) = {
                let s = state.lock().await;
                (s.mod_update_cache.clone(), s.ctl.clone_for_launch())
            };
            let stale = tokio::task::spawn_blocking(move || ctl_clone.get_installed_mods())
                .await
                .map_err(|e| format!("Task join error: {}", e))?
                .unwrap_or_default()
                .into_iter()
                .filter(|m| {
                    update_cache
                        .get(&m.id)
                        .map(|&r| r > m.local_updated)
                        .unwrap_or(false)
                })
                .map(|m| (m.id, m.name.clone()))
                .collect::<Vec<_>>();
            Some(stale)
        }
        "update_selected" => {
            let ids = mod_ids.clone().unwrap_or_default();
            if let Some(ref names) = mod_names {
                let id_names = ids
                    .iter()
                    .zip(names.iter())
                    .map(|(&id, name)| (id, name.clone()))
                    .collect::<Vec<_>>();
                Some(id_names)
            } else {
                let ctl_clone = { state.lock().await.ctl.clone_for_launch() };
                let id_names = tokio::task::spawn_blocking(move || ctl_clone.get_installed_mods())
                    .await
                    .map_err(|e| format!("Task join error: {}", e))?
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|m| ids.contains(&m.id))
                    .map(|m| (m.id, m.name.clone()))
                    .collect::<Vec<_>>();
                Some(id_names)
            }
        }
        "install_manual" => {
            let ids = mod_ids.clone().unwrap_or_default();
            let names = mod_names.clone().unwrap_or_default();
            let id_names = ids
                .iter()
                .zip(names.iter())
                .map(|(&id, name)| (id, name.clone()))
                .collect::<Vec<_>>();
            Some(id_names)
        }
        _ => None,
    };

    let mut rx = {
        let mut state = state.lock().await;
        let op = match op_type.as_str() {
            "install_server" | "update_server" => {
                let ip = ip.ok_or("ip required")?;
                let port = port.ok_or("port required")?;
                let server = find_server_in(&state.servers, &ip, port)
                    .cloned()
                    .ok_or_else(|| "Server not found".to_string())?;
                if op_type == "install_server" {
                    ModOperation::InstallOnly { server }
                } else {
                    ModOperation::UpdateThenLaunch {
                        server,
                        password: None,
                    }
                }
            }
            "update_all" => ModOperation::UpdateAll,
            "update_stale" => ModOperation::UpdateStale {
                stale_mods: pre_scan.unwrap_or_default(),
            },
            "update_one" => ModOperation::UpdateOne {
                mod_id: mod_id.ok_or("mod_id required")?,
                name: mod_name.unwrap_or_default(),
            },
            "update_selected" => ModOperation::UpdateSelected {
                mods: pre_scan.unwrap_or_default(),
            },
            "install_manual" => ModOperation::InstallManual {
                mods: pre_scan.unwrap_or_default(),
            },
            other => return Err(format!("Unknown operation: {}", other)),
        };
        let (rx, pty_input_tx, handle) = state
            .ctl
            .start_mod_operation(op)
            .map_err(|e| e.to_string())?;
        state.pty_input_tx = Some(pty_input_tx);
        state.mod_op_abort = Some(handle.abort_handle());
        rx
    };

    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Some(msg) => {
                    let evt = mod_progress_to_event(&msg);
                    let is_finished = evt.kind == "finished";
                    let _ = on_progress.send(evt);
                    if is_finished {
                        break;
                    }
                }
                None => break,
            }
        }
        let mut st = state_clone.lock().await;
        st.pty_input_tx = None;
        st.mod_op_abort = None;
    });

    Ok(())
}

/// Send input (password or Steam Guard code) to the running steamcmd PTY.
#[tauri::command]
pub(crate) async fn send_steamcmd_input(
    input: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    let state = state.lock().await;
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
pub(crate) async fn cancel_mod_operation(state: State<'_, SharedState>) -> Result<(), String> {
    let mut state = state.lock().await;
    if let Some(abort) = state.mod_op_abort.take() {
        abort.abort();
    }
    state.pty_input_tx = None;
    Ok(())
}
