//! Play statistics: how long DayZ ran, on which servers and offline maps,
//! when. The tracker writes sessions into `sessions.json` beside the
//! profile (it travels with a profile export); the commands here work the
//! figures out of it for the stats view.

mod compute;
pub(crate) mod tracker;

use crate::{Hub, HubEvent};
use std::sync::{LazyLock, Mutex};

use dz_profile::{History, PlayLog, SESSIONS_FILE, Snapshot, Writer};
use serde::{Deserialize, Serialize};

pub use compute::{
    DayStatDto, MapStatDto, PlaceStatDto, PlayKind, PlayStatsDto, SessionDto, StatsRange,
};

/// The sessions changed (one started, ended, ran on, or was deleted).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlaySessionsChanged;

static LOG: Mutex<Option<PlayLog>> = Mutex::new(None);
static WRITER: LazyLock<Writer> = LazyLock::new(Writer::default);

/// Read the log from the data folder (or start it from `history`). Called
/// when the app initialises, and after a profile import replaced the file.
pub(crate) fn open(history: &[History]) {
    open_in(&crate::paths::data_dir(), history);
}

/// The same, from the data folder `dir`.
pub(crate) fn open_in(dir: &std::path::Path, history: &[History]) {
    let log = PlayLog::load(dir.join(SESSIONS_FILE), history);
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

/// A page of the sessions, newest first.
#[derive(Serialize, Clone, Debug)]
pub struct SessionsPage {
    /// Sessions matching the search.
    pub total: u32,
    pub rows: Vec<SessionDto>,
}

impl Hub {
    /// The figures over `range`. `utc_offset_min` is the player's offset from
    /// UTC in minutes, east positive (minus JavaScript's `getTimezoneOffset`).
    pub async fn play_stats(
        &self,
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

    /// Every session matching `search` (name, address, map) that started in
    /// `from..to` (Unix seconds, either open), newest first, `limit` of them
    /// from `offset`.
    pub async fn play_sessions(
        &self,
        search: String,
        from: Option<i64>,
        to: Option<i64>,
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
                .filter(|s| from.is_none_or(|t| s.start >= t) && to.is_none_or(|t| s.start < t))
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
    pub async fn delete_session(&self, start: i64, name: String) -> Result<bool, String> {
        let app = self.clone();
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
        app.emit(HubEvent::PlaySessionsChanged(PlaySessionsChanged));
        Ok(true)
    }
}
