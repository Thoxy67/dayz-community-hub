//! Watches DayZ run and writes each stretch down as a session.
//!
//! Launching only hands Steam a command line, so the game is found by a
//! process scan (`DayzGame::running_pids`): every 5 s while a launch is
//! expected, every 30 s otherwise, and not at all while Steam says it is not
//! running. Once the game's processes are known, only they are looked at,
//! every 2 s, and one full scan confirms the game is gone when they are
//! (BattlEye's launcher hands over to the game). The tracker is the app's
//! one watcher of the game: it tells the window when the game starts or
//! stops ([`HubEvent::GameRunning`]). A launch from the launcher says what
//! is being played ([`expect`]); DayZ started any other way is recorded as
//! played on an unknown server.
//!
//! The open session's end is pushed forward as the game is seen running
//! and written once a minute, so a launcher that is killed loses a minute
//! at most; one that was closed while the game ran resumes the session if
//! the game still runs, or ends it where it was last seen.

use crate::{Hub, HubEvent};
use std::sync::Mutex;
use std::time::Duration;

use dz_profile::{PlayLog, Session, SessionKind};

use super::PlaySessionsChanged;

/// How long a launch waits for the game to appear before it is forgotten.
const EXPECT_FOR: i64 = 10 * 60;
/// A gap longer than this between two sightings means the launcher was not
/// watching: the session ends at the last sighting, not now.
const MAX_GAP: i64 = 90;
/// How often an open session's progress reaches the disk.
const WRITE_EVERY: i64 = 60;
/// What DayZ started outside the launcher is called.
const UNKNOWN_NAME: &str = "DayZ";
/// How often the game's known processes are looked at while it runs.
const WATCH_EVERY: Duration = Duration::from_secs(2);
/// How often the game is looked for while a launch is expected.
const EXPECTING_EVERY: Duration = Duration::from_secs(5);
/// How often otherwise.
const IDLE_EVERY: Duration = Duration::from_secs(30);

/// What a launch is about to play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expected {
    pub kind: SessionKind,
    pub name: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub map: Option<String>,
}

static PENDING: Mutex<Option<(Expected, i64)>> = Mutex::new(None);
static WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// A launch was handed to Steam: the game that appears next plays this.
pub(crate) fn expect(e: Expected) {
    if let Ok(mut p) = PENDING.lock() {
        *p = Some((e, dz_common::time::now_secs() as i64));
    }
    WAKE.notify_one();
}

/// What one look at the game changed in the log.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Nothing,
    /// The open session runs on: its end moved.
    Progress,
    /// A session started or ended.
    Changed,
}

fn open_session(e: Option<Expected>, now: i64) -> Session {
    let e = e.unwrap_or(Expected {
        kind: SessionKind::Unknown,
        name: UNKNOWN_NAME.into(),
        ip: None,
        port: None,
        map: None,
    });
    Session {
        kind: e.kind,
        name: e.name,
        ip: e.ip,
        port: e.port,
        map: e.map,
        start: now,
        end: now,
        open: true,
        measured: true,
    }
}

/// Bring the log up to date with one look at the game. `pending` is the
/// launch waiting for it, taken when a session starts.
pub(crate) fn step(
    log: &mut PlayLog,
    running: bool,
    now: i64,
    pending: &mut Option<Expected>,
) -> Step {
    match (running, log.open_mut()) {
        (true, Some(open)) => {
            if pending.is_some() {
                // A launch while the game runs: the player moved on.
                open.end = now;
                open.open = false;
                log.sessions.push(open_session(pending.take(), now));
                Step::Changed
            } else {
                open.end = now;
                Step::Progress
            }
        }
        (true, None) => {
            log.sessions.push(open_session(pending.take(), now));
            Step::Changed
        }
        (false, Some(open)) => {
            if now - open.end <= MAX_GAP {
                open.end = now;
            }
            open.open = false;
            Step::Changed
        }
        (false, None) => Step::Nothing,
    }
}

/// One look at the game. With the processes it was last seen as, only they
/// are read; when they are gone (or none were known), the whole table is,
/// unless `idle` and Steam says it is not running (DayZ cannot run without
/// it). Returns whether the game runs, and the processes to look at next.
/// Blocking.
fn look(known: &[u32], idle: bool) -> (bool, Vec<u32>) {
    use dz_steamcmd::{DayzGame, SteamClient};
    if !known.is_empty() && DayzGame::any_alive(known) {
        return (true, known.to_vec());
    }
    if idle && known.is_empty() && SteamClient::known_running() == Some(false) {
        return (false, Vec::new());
    }
    // Processes that just went away may have handed over to one the kept
    // table has not seen yet: read it again.
    let pids = DayzGame::running_pids(!known.is_empty());
    (!pids.is_empty(), pids)
}

/// How long until the next look.
fn next_look(watching: bool, expecting: bool) -> Duration {
    if watching {
        WATCH_EVERY
    } else if expecting {
        EXPECTING_EVERY
    } else {
        IDLE_EVERY
    }
}

/// Watch the game for as long as the app runs.
pub(crate) fn spawn(app: Hub) {
    tokio::spawn(async move {
        let mut last_write = 0i64;
        let mut told: Option<bool> = None;
        let mut known: Vec<u32> = Vec::new();
        loop {
            let expecting = PENDING.lock().is_ok_and(|p| p.is_some());
            let idle = !expecting && told != Some(true);
            let prev = std::mem::take(&mut known);
            let (running, pids) = tokio::task::spawn_blocking(move || look(&prev, idle))
                .await
                .unwrap_or((false, Vec::new()));
            known = pids;
            if told != Some(running) {
                told = Some(running);
                app.emit(HubEvent::GameRunning(running));
            }
            let now = dz_common::time::now_secs() as i64;
            let mut pending = PENDING.lock().ok().and_then(|mut p| {
                // A launch that never showed up is forgotten.
                if p.as_ref().is_some_and(|(_, at)| now - at > EXPECT_FOR) {
                    *p = None;
                }
                p.as_ref().map(|(e, _)| e.clone())
            });
            let had_pending = pending.is_some();

            let outcome = super::with_log(|log| {
                let step = step(log, running, now, &mut pending);
                let write = step == Step::Changed
                    || (step == Step::Progress && now - last_write >= WRITE_EVERY);
                (step, write.then(|| log.snapshot()))
            });
            if had_pending
                && pending.is_none()
                && let Ok(mut p) = PENDING.lock()
            {
                *p = None;
            }
            // What Discord shows follows the open session (it wakes only on a change).
            let open = super::with_log(|log| {
                log.open_mut().map(|s| crate::features::discord::Playing {
                    kind: s.kind,
                    name: s.name.clone(),
                    start: s.start,
                })
            })
            .flatten();
            crate::features::discord::set_playing(open.filter(|_| running));
            if let Some((_, Some(snapshot))) = outcome {
                last_write = now;
                super::save(snapshot).await;
                app.emit(HubEvent::PlaySessionsChanged(PlaySessionsChanged));
            }

            let expecting = PENDING.lock().is_ok_and(|p| p.is_some());
            let wait = next_look(!known.is_empty(), expecting);
            tokio::select! {
                () = tokio::time::sleep(wait) => {}
                () = WAKE.notified() => {
                    // Steam takes a moment to start the game.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });
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

    fn server(name: &str) -> Expected {
        Expected {
            kind: SessionKind::Server,
            name: name.into(),
            ip: Some("1.2.3.4".into()),
            port: Some(2302),
            map: Some("enoch".into()),
        }
    }

    #[test]
    fn looks_are_close_together_only_while_something_is_happening() {
        assert_eq!(next_look(true, false), WATCH_EVERY);
        assert_eq!(next_look(true, true), WATCH_EVERY);
        assert_eq!(next_look(false, true), EXPECTING_EVERY);
        assert_eq!(next_look(false, false), IDLE_EVERY);
    }

    #[test]
    fn a_launch_becomes_a_session_that_runs_then_ends() {
        let mut log = PlayLog::default();
        let mut pending = Some(server("A"));
        assert_eq!(step(&mut log, false, 100, &mut pending), Step::Nothing);
        assert!(pending.is_some(), "kept until the game shows up");
        assert_eq!(step(&mut log, true, 130, &mut pending), Step::Changed);
        assert!(pending.is_none());
        assert_eq!(step(&mut log, true, 145, &mut pending), Step::Progress);
        assert_eq!(step(&mut log, false, 160, &mut pending), Step::Changed);
        let s = &log.sessions[0];
        assert_eq!(
            (s.name.as_str(), s.start, s.end, s.open),
            ("A", 130, 160, false)
        );
    }

    #[test]
    fn a_game_started_elsewhere_is_unknown_and_a_new_launch_splits() {
        let mut log = PlayLog::default();
        let mut pending = None;
        step(&mut log, true, 10, &mut pending);
        assert_eq!(log.sessions[0].kind, SessionKind::Unknown);
        pending = Some(server("B"));
        assert_eq!(step(&mut log, true, 70, &mut pending), Step::Changed);
        assert_eq!(log.sessions.len(), 2);
        assert!(!log.sessions[0].open && log.sessions[0].end == 70);
        assert!(log.sessions[1].open && log.sessions[1].name == "B");
    }

    #[test]
    fn a_session_the_launcher_stopped_watching_ends_where_it_was_last_seen() {
        let mut log = PlayLog::default();
        let mut pending = Some(server("A"));
        step(&mut log, true, 1_000, &mut pending);
        step(&mut log, true, 1_060, &mut pending);
        // The launcher was closed; it starts again an hour later, game gone.
        assert_eq!(step(&mut log, false, 4_660, &mut pending), Step::Changed);
        assert_eq!(log.sessions[0].end, 1_060);
        assert!(!log.sessions[0].open);
    }
}
