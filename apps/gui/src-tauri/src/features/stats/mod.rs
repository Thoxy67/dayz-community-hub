//! Play statistics: how long DayZ ran, on which servers and offline maps,
//! when. The tracker writes sessions into `sessions.json` beside the
//! profile (it travels with a profile export); the commands here work the
//! figures out of it for the stats view.

mod compute;
pub(crate) mod tracker;

use std::sync::{LazyLock, Mutex};

use dz_profile::{History, PlayLog, SESSIONS_FILE, Snapshot, Writer};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_specta::Event;

pub use compute::{PlayStatsDto, SessionDto, StatsRange};

/// The sessions changed (one started, ended, ran on, or was deleted).
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct PlaySessionsChanged;

static LOG: Mutex<Option<PlayLog>> = Mutex::new(None);
static WRITER: LazyLock<Writer> = LazyLock::new(Writer::default);

/// Read the log from the data folder (or start it from `history`). Called
/// when the app initialises, and after a profile import replaced the file.
pub(crate) fn open(history: &[History]) {
    let log = PlayLog::load(dz_profile::default_data_dir().join(SESSIONS_FILE), history);
    if let Ok(mut l) = LOG.lock() {
        *l = Some(log);
    }
}

/// Run `f` on the log, if it is open.
pub(crate) fn with_log<R>(f: impl FnOnce(&mut PlayLog) -> R) -> Option<R> {
    LOG.lock().ok()?.as_mut().map(f)
}

/// Write a snapshot of the log, in the order snapshots were taken.
pub(crate) async fn save(snapshot: Snapshot) {
    let snapshot = WRITER.stamp(snapshot);
    if let Err(e) = WRITER.write(snapshot).await {
        eprintln!("[stats] sessions not saved: {e}");
    }
}

fn now() -> i64 {
    dz_common::time::now_secs() as i64
}

/// The figures over `range`. `utc_offset_min` is the player's offset from
/// UTC in minutes, east positive (minus JavaScript's `getTimezoneOffset`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn play_stats(
    range: StatsRange,
    utc_offset_min: i32,
) -> Result<PlayStatsDto, String> {
    let sessions = with_log(|l| l.sessions.clone()).unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        compute::stats(&sessions, now(), range, i64::from(utc_offset_min) * 60)
    })
    .await
    .map_err(|e| e.to_string())
}

/// A page of the sessions, newest first.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct SessionsPage {
    /// Sessions matching the search.
    pub total: u32,
    pub rows: Vec<SessionDto>,
}

/// Every session matching `search` (name, address, map), newest first,
/// `limit` of them from `offset`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn play_sessions(
    search: String,
    offset: u32,
    limit: u32,
) -> Result<SessionsPage, String> {
    let needle = search.trim().to_lowercase();
    let now = now();
    Ok(with_log(|l| {
        let hits: Vec<&dz_profile::Session> = l
            .sessions
            .iter()
            .rev()
            .filter(|s| {
                needle.is_empty()
                    || s.name.to_lowercase().contains(&needle)
                    || s.ip.as_deref().is_some_and(|ip| ip.contains(&needle))
                    || s.map
                        .as_deref()
                        .is_some_and(|m| m.to_lowercase().contains(&needle))
            })
            .collect();
        SessionsPage {
            total: hits.len() as u32,
            rows: hits
                .into_iter()
                .skip(offset as usize)
                .take(limit as usize)
                .map(|s| compute::dto(s, now))
                .collect(),
        }
    })
    .unwrap_or(SessionsPage {
        total: 0,
        rows: Vec::new(),
    }))
}

/// Forget one session (not the one running). True when it was found.
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_session(
    app: AppHandle,
    start: i64,
    name: String,
) -> Result<bool, String> {
    let snapshot = with_log(|l| {
        let before = l.sessions.len();
        l.sessions
            .retain(|s| s.open || s.start != start || s.name != name);
        (l.sessions.len() != before).then(|| l.snapshot())
    })
    .flatten();
    let Some(snapshot) = snapshot else {
        return Ok(false);
    };
    save(snapshot).await;
    let _ = PlaySessionsChanged.emit(&app);
    Ok(true)
}
