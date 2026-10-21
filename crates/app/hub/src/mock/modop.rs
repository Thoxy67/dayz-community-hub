//! A pretend mod operation (mock.ts' `modOpSteps`): a plausible SteamCMD
//! run (log in, each mod with live progress lines, one failure, a summary)
//! or a download through the Steam client, as timed steps. They go through
//! the real operation slot (`steamcmd::op`), so `mod_op_state`,
//! `mod_op_attach`, `cancel_mod_operation` and `mod_op_dismiss` answer as
//! they do for a real one.

use std::sync::Arc;
use std::time::Duration;

use crate::features::steamcmd::op::{self, ModOpVia};
use crate::features::steamcmd::{ModOpType, ModProgressEvent, ModProgressKind, QueuedModDto};
use crate::{Hub, HubEvent};

use super::World;
use super::generate::{LAUNCHER_DIR, STEAM_ROOT};

/// Steps with the time each is due, ms after the start.
struct Steps {
    t: u64,
    total: usize,
    out: Vec<(u64, ModProgressEvent)>,
}

impl Steps {
    fn ev(&self, kind: ModProgressKind) -> ModProgressEvent {
        ModProgressEvent {
            kind,
            current: 0,
            total: self.total,
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

    fn at(&mut self, dt: u64, ev: ModProgressEvent) {
        self.t += dt;
        self.out.push((self.t, ev));
    }

    fn log(&mut self, dt: u64, line: String) {
        let ev = ModProgressEvent {
            log_line: Some(line),
            ..self.ev(ModProgressKind::LogLine)
        };
        self.at(dt, ev);
    }

    fn progress(&mut self, dt: u64, line: String) {
        let ev = ModProgressEvent {
            log_line: Some(line),
            ..self.ev(ModProgressKind::LogProgress)
        };
        self.at(dt, ev);
    }

    fn about(&mut self, dt: u64, kind: ModProgressKind, current: usize, id: u64, name: String) {
        let ev = ModProgressEvent {
            current,
            mod_id: id,
            name,
            ..self.ev(kind)
        };
        self.at(dt, ev);
    }

    fn finished(&mut self, dt: u64, ok: usize, failed: usize) {
        let ev = ModProgressEvent {
            ok,
            failed,
            ..self.ev(ModProgressKind::Finished)
        };
        self.at(dt, ev);
    }
}

/// `done` of `total` as SteamCMD prints it.
fn percent(done: u64, total: u64) -> String {
    format!("{:.2}", done as f64 / total.max(1) as f64 * 100.0)
}

fn steps(
    world: &World,
    kind: ModOpType,
    list: &[u64],
    via: ModOpVia,
) -> Vec<(u64, ModProgressEvent)> {
    let mut s = Steps {
        t: 0,
        total: list.len(),
        out: Vec::new(),
    };
    let named = |id: u64| {
        world.mods.iter().find(|m| m.id == id).map_or_else(
            || (format!("Workshop {id}"), 300_000_000),
            |m| (m.name.clone(), m.size),
        )
    };
    // First, as the backend does: every mod the operation works on, named.
    let queue = if kind == ModOpType::Login {
        Vec::new()
    } else {
        list.iter()
            .map(|&id| QueuedModDto {
                id,
                name: named(id).0,
            })
            .collect()
    };
    let queued = ModProgressEvent {
        queue,
        ..s.ev(ModProgressKind::Queued)
    };
    s.at(0, queued);

    if via == ModOpVia::Steamworks {
        // What dz-steamworks reports: subscribe, Steam downloads, installed.
        s.log(200, "Connected to Steam as DayZ (app 221100)".into());
        for id in list {
            s.log(40, format!("Subscribing to {id}"));
        }
        for id in list {
            s.log(80, format!("Subscribed {id}"));
        }
        let mut ok = 0;
        for (i, &id) in list.iter().enumerate() {
            let (name, total) = named(id);
            s.about(120, ModProgressKind::Starting, i + 1, id, name.clone());
            s.log(10, format!("Downloading item {id} ({name}) through Steam"));
            s.log(
                40,
                format!("{id}: subscribed, needs update, downloading, download pending"),
            );
            for k in 1..=12u64 {
                let done = total * k / 12;
                s.progress(
                    90,
                    format!(
                        "Steam: item {id} downloading, progress: {} ({done} / {total})",
                        percent(done, total)
                    ),
                );
            }
            if i == 1 && list.len() > 2 {
                let error =
                    "Download failed: access denied: the item is private, friends-only or hidden";
                s.log(80, format!("{id} failed: {error}"));
                s.about(
                    10,
                    ModProgressKind::Failed,
                    i + 1,
                    id,
                    format!("{name} ({error})"),
                );
            } else {
                ok += 1;
                s.log(60, format!("{id}: subscribed, installed"));
                s.log(
                    10,
                    format!(
                        "Installed {id} at {STEAM_ROOT}/steamapps/workshop/content/221100/{id}"
                    ),
                );
                s.about(10, ModProgressKind::Done, i + 1, id, name);
            }
        }
        s.log(150, "Disconnected from Steam".into());
        s.finished(20, ok, list.len() - ok);
        return s.out;
    }

    s.log(
        300,
        format!(
            "Redirecting stderr to '{LAUNCHER_DIR}/steamcmd-home/.steam/steamcmd/logs/stderr.txt'"
        ),
    );
    s.log(100, "[  0%] Checking for available updates...".into());
    s.log(100, "[----] Verifying installation...".into());
    s.log(
        100,
        "Steam Console Client (c) Valve Corporation - version 1726604893".into(),
    );
    s.log(
        100,
        "Logging in user 'survivor_42' to Steam Public...OK".into(),
    );
    s.log(80, "Waiting for client config...OK".into());
    s.log(80, "Waiting for user info...OK".into());
    let logged_in = s.ev(ModProgressKind::LoggedIn);
    s.at(10, logged_in);
    if kind == ModOpType::Login {
        s.log(200, "Unloading Steam API...OK".into());
        s.total = 1;
        s.finished(50, 1, 0);
        return s.out;
    }
    let mut ok = 0;
    for (i, &id) in list.iter().enumerate() {
        let (name, total) = named(id);
        s.about(120, ModProgressKind::Starting, i + 1, id, name.clone());
        s.log(60, format!("Downloading item {id} ..."));
        for k in 1..=12u64 {
            let done = total * k / 12;
            s.progress(
                90,
                format!(
                    "Update state (0x61) downloading, progress: {} ({done} / {total})",
                    percent(done, total)
                ),
            );
        }
        if i == 1 && list.len() > 2 {
            s.log(80, format!("ERROR! Download item {id} failed (Failure)."));
            s.about(20, ModProgressKind::Failed, i + 1, id, name);
        } else {
            ok += 1;
            s.log(
                80,
                format!(
                    "Success. Downloaded item {id} to \"{LAUNCHER_DIR}/steamcmd-content/steamapps/workshop/content/221100/{id}\" ({total} bytes)"
                ),
            );
            s.about(20, ModProgressKind::Done, i + 1, id, name);
        }
    }
    s.log(200, "Unloading Steam API...OK".into());
    s.finished(50, ok, list.len() - ok);
    s.out
}

impl World {
    /// Start a pretend mod operation: the mods asked for, else those with an
    /// update, else the first three. Its steps arrive as
    /// `HubEvent::ModProgress`.
    pub(crate) async fn start_mod_operation(
        &self,
        hub: &Hub,
        kind: ModOpType,
        mod_ids: Option<Vec<u64>>,
    ) -> Result<(), String> {
        let state = hub.state()?;
        let mut list = mod_ids.unwrap_or_else(|| {
            self.mods
                .iter()
                .filter(|m| m.update_available)
                .map(|m| m.id)
                .collect()
        });
        if list.is_empty() {
            list = self.mods.iter().take(3).map(|m| m.id).collect();
        }
        let mut s = state.write().await;
        if s.mod_op_abort.is_some() {
            return Err("A mod operation is already running".into());
        }
        let via = if kind != ModOpType::Login && s.ctl.downloads_through_steam() {
            ModOpVia::Steamworks
        } else {
            ModOpVia::Steamcmd
        };
        let steps = steps(self, kind, &list, via);
        let sink: op::StepSink = {
            let hub = hub.clone();
            Arc::new(move |ev| hub.emit(HubEvent::ModProgress(Box::new(ev))))
        };
        let id = op::begin(kind, via, sink);
        let done = state.clone();
        let handle = tokio::spawn(async move {
            let start = tokio::time::Instant::now();
            for (at, ev) in steps {
                tokio::time::sleep_until(start + Duration::from_millis(at)).await;
                op::emit(id, ev);
            }
            let mut s = done.write().await;
            s.mod_op_abort = None;
            s.pty_input_tx = None;
        });
        s.mod_op_abort = Some(handle.abort_handle());
        Ok(())
    }
}
