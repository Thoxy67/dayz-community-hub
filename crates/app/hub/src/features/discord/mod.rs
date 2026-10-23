//! Discord Rich Presence: what the player does ("Playing on <server>", "In the
//! launcher", "Offline mode", since when) shown on their Discord profile, when
//! the profile's `discord_presence` is on (off by default).
//!
//! A pure-Rust client of Discord's local IPC ([`socket`], [`frame`]): one
//! background task, started by `Hub::start` (never in Mock mode), connects when
//! the presence is wanted, says the activity again whenever it changes, answers
//! Discord's pings, and clears it (then lets go) when the presence is turned
//! off. A Discord that is not running, or starts later, is looked for again
//! quietly, less and less often; nothing is ever shown to the player about it
//! but the settings row's state. When the launcher exits the activity is
//! cleared ([`Hub::discord_quit`], a short wait), and the connection closes.
//!
//! What is played comes from the play tracker (`stats::tracker`, through
//! [`set_playing`]); the words come from the window, in its language
//! ([`Hub::set_discord_words`]).

mod frame;
mod socket;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use dz_profile::SessionKind;
use serde::Serialize;
use serde_json::{Value, json};

use crate::Hub;

/// The Discord application the presence speaks as (its "Application ID", from
/// <https://discord.com/developers/applications>). Its name is what Discord
/// shows as the game ("Playing DayZ Community Hub"). Empty: no application
/// was made yet, and the presence is unavailable (the settings say so).
pub const DISCORD_APP_ID: &str = "";

/// The art key, uploaded to the application's Rich Presence assets, shown as
/// the large picture. Empty: no picture.
const LARGE_IMAGE: &str = "";

/// Discord refuses a text field longer than this (and shorter than 2).
const MAX_TEXT: usize = 128;

/// The first wait before looking for Discord again, and the longest.
const RETRY_MIN: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(120);
/// How long a write or the handshake's answer may take.
const IO_FOR: Duration = Duration::from_secs(5);
/// How long an activity Discord refused waits before it is offered again.
const REFUSED_FOR: Duration = Duration::from_secs(60);

/// Whether this build has a Discord application to speak as.
pub fn presence_available() -> bool {
    !DISCORD_APP_ID.trim().is_empty()
}

/// The presence's words, in the window's language. `playing_on` holds `{name}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresenceWords {
    pub launcher: String,
    pub offline: String,
    pub playing: String,
    pub playing_on: String,
}

impl Default for PresenceWords {
    fn default() -> Self {
        Self {
            launcher: "In the launcher".into(),
            offline: "Offline mode".into(),
            playing: "Playing DayZ".into(),
            playing_on: "Playing on {name}".into(),
        }
    }
}

/// Where the presence is, for the settings row.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiscordStatus {
    /// The build has an application id ([`DISCORD_APP_ID`]).
    pub available: bool,
    /// Connected to a running Discord, the activity shown.
    pub connected: bool,
}

/// What is being played, as the play tracker sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Playing {
    pub kind: SessionKind,
    pub name: String,
    /// Unix seconds.
    pub start: i64,
}

/// What the activity is made of.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct Inputs {
    enabled: bool,
    show_server: bool,
    /// `None`: the English ones.
    words: Option<PresenceWords>,
    playing: Option<Playing>,
    /// When the launcher started (Unix seconds).
    since: i64,
}

/// One activity as Discord is told it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Activity {
    details: String,
    start: i64,
}

impl Activity {
    fn to_json(&self) -> Value {
        let mut a = json!({
            "details": self.details,
            "timestamps": { "start": self.start },
            "instance": false,
        });
        if !LARGE_IMAGE.is_empty()
            && let Some(o) = a.as_object_mut()
        {
            o.insert(
                "assets".into(),
                json!({ "large_image": LARGE_IMAGE, "large_text": "DayZ Community Hub" }),
            );
        }
        a
    }
}

static INPUTS: Mutex<Option<Inputs>> = Mutex::new(None);
static WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static CONNECTED: AtomicBool = AtomicBool::new(false);
static NONCE: AtomicU64 = AtomicU64::new(1);
/// The launcher is closing: the activity is cleared and the task ends.
static EXITING: AtomicBool = AtomicBool::new(false);
/// Set when the task has cleared the activity and ended.
static CLEARED: (Mutex<bool>, std::sync::Condvar) = (Mutex::new(false), std::sync::Condvar::new());

/// Change the inputs; the task is woken when they changed.
fn edit(f: impl FnOnce(&mut Inputs)) {
    let changed = {
        let Ok(mut g) = INPUTS.lock() else {
            return;
        };
        let i = g.get_or_insert_with(|| Inputs {
            show_server: true,
            since: dz_common::time::now_secs() as i64,
            ..Inputs::default()
        });
        let before = i.clone();
        f(i);
        *i != before
    };
    if changed {
        WAKE.notify_one();
    }
}

/// Note whether Discord is connected; the window hears of a change.
fn set_connected(hub: &Hub, on: bool) {
    if CONNECTED.swap(on, Ordering::Relaxed) != on {
        hub.emit(crate::HubEvent::DiscordConnected(on));
    }
}

fn inputs() -> Inputs {
    INPUTS
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

/// The play tracker's latest look: the open session, if any.
pub(crate) fn set_playing(p: Option<Playing>) {
    edit(|i| note_playing(i, p, dz_common::time::now_secs() as i64));
}

/// What is played now; back in the launcher after a game, "In the launcher"
/// counts from `now`.
fn note_playing(i: &mut Inputs, p: Option<Playing>, now: i64) {
    if i.playing.is_some() && p.is_none() {
        i.since = now;
    }
    i.playing = p;
}

/// A text Discord takes: at most [`MAX_TEXT`] characters, at least 2.
fn clamp_text(s: &str) -> String {
    let s = s.trim();
    let mut out: String = s.chars().take(MAX_TEXT).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}

/// The activity the inputs ask for, or `None` for no presence (always, when
/// the build has no application: `available`).
fn desired(i: &Inputs, available: bool) -> Option<Activity> {
    if !i.enabled || !available {
        return None;
    }
    let fallback = PresenceWords::default();
    let w = i.words.as_ref().unwrap_or(&fallback);
    let (details, start) = match &i.playing {
        None => (w.launcher.clone(), i.since),
        Some(p) => match p.kind {
            SessionKind::Offline => (w.offline.clone(), p.start),
            SessionKind::Server if i.show_server && !p.name.trim().is_empty() => {
                (w.playing_on.replace("{name}", p.name.trim()), p.start)
            }
            SessionKind::Server | SessionKind::Unknown => (w.playing.clone(), p.start),
        },
    };
    Some(Activity {
        details: clamp_text(&details),
        start,
    })
}

fn wanted() -> Option<Activity> {
    if EXITING.load(Ordering::Relaxed) {
        return None;
    }
    desired(&inputs(), presence_available())
}

/// Look after the presence for as long as the app runs.
pub(crate) fn spawn(hub: Hub) {
    if !presence_available() {
        return;
    }
    tokio::spawn(async move {
        load_settings(&hub).await;
        let connected = |on: bool| set_connected(&hub, on);
        let mut retry = RETRY_MIN;
        loop {
            if EXITING.load(Ordering::Relaxed) {
                break;
            }
            if wanted().is_none() {
                connected(false);
                WAKE.notified().await;
                continue;
            }
            let wait = match socket::connect().await {
                Some(stream) => {
                    let out =
                        session(stream, DISCORD_APP_ID.trim(), &WAKE, wanted, connected).await;
                    connected(false);
                    match out {
                        Ok(()) => None,
                        // It spoke with Discord, which closed or restarts: look again
                        // a little later.
                        Err(Lost { ready: true, .. }) => {
                            retry = RETRY_MIN;
                            Some((RETRY_MIN, false))
                        }
                        // Refused (an application Discord does not know) or cut short
                        // before it was ready: less and less often, as when absent.
                        Err(Lost { ready: false, why }) => {
                            eprintln!("{why}");
                            Some((retry, true))
                        }
                    }
                }
                // Discord is not running: look again later, less and less often.
                None => Some((retry, true)),
            };
            if let Some((wait, grow)) = wait {
                tokio::select! {
                    () = tokio::time::sleep(wait) => {}
                    () = WAKE.notified() => {}
                }
                if grow {
                    retry = (retry * 2).min(RETRY_MAX);
                }
            }
        }
        say_cleared();
    });
}

/// The profile's settings, once `initialize` has read it.
async fn load_settings(hub: &Hub) {
    let state = loop {
        if let Some(s) = hub.try_state() {
            break s;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    let (enabled, show) = {
        let s = state.read().await;
        let p = s.ctl.profile();
        (p.discord_presence, p.discord_show_server)
    };
    edit(|i| {
        i.enabled = enabled;
        i.show_server = show;
    });
}

/// Why a connection ended, and whether Discord had said it was ready first.
#[derive(Debug)]
struct Lost {
    ready: bool,
    why: String,
}

/// One connection: the handshake, then the activity `want` asks for whenever it
/// changes (`wake`), until it asks for none (the activity cleared: `Ok`) or the
/// connection is lost (`Err`). `connected` hears whether Discord shows it.
async fn session<S>(
    stream: S,
    app_id: &str,
    wake: &tokio::sync::Notify,
    want: impl Fn() -> Option<Activity>,
    connected: impl Fn(bool),
) -> Result<(), Lost>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + 'static,
{
    let early = |why: String| Lost { ready: false, why };
    let late = |why: String| Lost { ready: true, why };
    let io = |e: tokio::time::error::Elapsed| format!("Discord: {e}");
    let (mut rd, mut wr) = tokio::io::split(stream);
    tokio::time::timeout(
        IO_FOR,
        frame::write(&mut wr, frame::OP_HANDSHAKE, &frame::handshake(app_id)),
    )
    .await
    .map_err(io)
    .and_then(|r| r)
    .map_err(early)?;
    let (op, v) = tokio::time::timeout(IO_FOR, frame::read(&mut rd))
        .await
        .map_err(io)
        .and_then(|r| r)
        .map_err(early)?;
    if !frame::is_ready(op, &v) {
        return Err(early(format!("Discord refused the handshake: {v}")));
    }
    connected(true);

    // Reading in a task of its own: a read cut short by a change would lose
    // its half-read frame.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<(u32, Value), String>>(8);
    let reader = tokio::spawn(async move {
        loop {
            let f = frame::read(&mut rd).await;
            let end = f.is_err();
            if tx.send(f).await.is_err() || end {
                break;
            }
        }
    });
    let pid = std::process::id();
    let mut sent: Option<Activity> = None;
    // An activity Discord refused, not offered again before the instant.
    let mut refused: Option<(Activity, tokio::time::Instant)> = None;
    let out = loop {
        let Some(want) = want() else {
            let clear = frame::set_activity(pid, None, NONCE.fetch_add(1, Ordering::Relaxed));
            let _ =
                tokio::time::timeout(IO_FOR, frame::write(&mut wr, frame::OP_FRAME, &clear)).await;
            break Ok(());
        };
        let now = tokio::time::Instant::now();
        if refused.as_ref().is_some_and(|(_, until)| now >= *until) {
            refused = None;
        }
        let held = refused.as_ref().is_some_and(|(a, _)| *a == want);
        if sent.as_ref() != Some(&want) && !held {
            let set = frame::set_activity(
                pid,
                Some(want.to_json()),
                NONCE.fetch_add(1, Ordering::Relaxed),
            );
            match tokio::time::timeout(IO_FOR, frame::write(&mut wr, frame::OP_FRAME, &set)).await {
                Ok(Ok(())) => sent = Some(want),
                Ok(Err(e)) => break Err(late(e)),
                Err(e) => break Err(late(io(e))),
            }
        }
        let until = refused.as_ref().map(|(_, u)| *u);
        tokio::select! {
            () = wake.notified() => {}
            () = tokio::time::sleep_until(until.unwrap_or(now)), if until.is_some() => {}
            f = rx.recv() => match f {
                Some(Ok((frame::OP_PING, v))) => {
                    let w = frame::write(&mut wr, frame::OP_PONG, &v);
                    if let Err(e) = tokio::time::timeout(IO_FOR, w).await.map_err(io).and_then(|r| r) {
                        break Err(late(e));
                    }
                }
                Some(Ok((frame::OP_CLOSE, v))) => break Err(late(format!("Discord closed: {v}"))),
                Some(Ok((frame::OP_FRAME, v))) if frame::is_error(&v) => {
                    // The activity was refused: not shown, and offered again a
                    // while later (or at once when it changes).
                    eprintln!("Discord refused the activity: {v}");
                    connected(false);
                    refused = sent
                        .take()
                        .map(|a| (a, tokio::time::Instant::now() + REFUSED_FOR));
                }
                Some(Ok((frame::OP_FRAME, v))) if frame::is_activity_set(&v) => connected(true),
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(late(e)),
                None => break Err(late("Discord: the connection ended".into())),
            },
        }
    };
    reader.abort();
    out
}

/// Tell [`Hub::discord_quit`] the activity is cleared (or there is none).
fn say_cleared() {
    let (lock, cv) = &CLEARED;
    if let Ok(mut done) = lock.lock() {
        *done = true;
        cv.notify_all();
    }
}

impl Hub {
    /// Turn Discord Rich Presence on or off, and whether it names the server;
    /// saved in the profile, applied at once.
    pub async fn set_discord_presence(
        &self,
        enabled: bool,
        show_server: bool,
    ) -> Result<(), String> {
        let state = self.state()?;
        crate::state::mutate_profile(&state, |s| {
            let p = s.ctl.profile_mut();
            p.discord_presence = enabled;
            p.discord_show_server = show_server;
            Ok(())
        })
        .await?;
        edit(|i| {
            i.enabled = enabled;
            i.show_server = show_server;
        });
        Ok(())
    }

    /// The presence's words, in the window's language (at start and when the
    /// language changes). Never blocks.
    pub fn set_discord_words(&self, words: PresenceWords) {
        edit(|i| i.words = Some(words));
    }

    /// Where the presence is, for the settings row. Never blocks.
    pub fn discord_status(&self) -> DiscordStatus {
        DiscordStatus {
            // The task never runs on the invented world.
            available: presence_available() && !self.is_mock(),
            connected: CONNECTED.load(Ordering::Relaxed),
        }
    }

    /// The launcher is closing: clear the activity, waiting at most `wait` for
    /// it to be sent. Returns at once when nothing is shown.
    pub fn discord_quit(&self, wait: Duration) {
        EXITING.store(true, Ordering::Relaxed);
        WAKE.notify_one();
        if !CONNECTED.load(Ordering::Relaxed) {
            return;
        }
        let (lock, cv) = &CLEARED;
        if let Ok(done) = lock.lock() {
            let _ = cv.wait_timeout_while(done, wait, |done| !*done);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn on(playing: Option<Playing>) -> Inputs {
        Inputs {
            enabled: true,
            show_server: true,
            words: None,
            playing,
            since: 100,
        }
    }

    fn server(name: &str) -> Option<Playing> {
        Some(Playing {
            kind: SessionKind::Server,
            name: name.into(),
            start: 500,
        })
    }

    #[test]
    fn the_activity_says_where_the_player_is() {
        let a = desired(&on(None), true).unwrap();
        assert_eq!((a.details.as_str(), a.start), ("In the launcher", 100));
        let a = desired(&on(server("Namalsk PvE")), true).unwrap();
        assert_eq!(
            (a.details.as_str(), a.start),
            ("Playing on Namalsk PvE", 500)
        );
        let mut hidden = on(server("Namalsk PvE"));
        hidden.show_server = false;
        assert_eq!(desired(&hidden, true).unwrap().details, "Playing DayZ");
        assert_eq!(
            desired(&on(server("  ")), true).unwrap().details,
            "Playing DayZ"
        );
        let off = Some(Playing {
            kind: SessionKind::Offline,
            name: "dayzOffline.chernarusplus".into(),
            start: 700,
        });
        let a = desired(&on(off), true).unwrap();
        assert_eq!((a.details.as_str(), a.start), ("Offline mode", 700));
        let unknown = Some(Playing {
            kind: SessionKind::Unknown,
            name: "DayZ".into(),
            start: 800,
        });
        assert_eq!(desired(&on(unknown), true).unwrap().details, "Playing DayZ");
    }

    #[test]
    fn no_presence_when_off_or_without_an_application() {
        let mut i = on(server("A"));
        assert_eq!(desired(&i, false), None);
        i.enabled = false;
        assert_eq!(desired(&i, true), None);
    }

    #[test]
    fn the_words_are_the_windows() {
        let mut i = on(server("Ferme"));
        i.words = Some(PresenceWords {
            launcher: "Dans le lanceur".into(),
            offline: "Mode hors ligne".into(),
            playing: "Joue à DayZ".into(),
            playing_on: "Joue sur {name}".into(),
        });
        assert_eq!(desired(&i, true).unwrap().details, "Joue sur Ferme");
        i.playing = None;
        assert_eq!(desired(&i, true).unwrap().details, "Dans le lanceur");
    }

    #[test]
    fn texts_fit_what_discord_takes() {
        assert_eq!(clamp_text(&"é".repeat(300)).chars().count(), MAX_TEXT);
        assert_eq!(clamp_text("x"), "x ");
        assert_eq!(clamp_text("  ab  "), "ab");
    }

    #[test]
    fn an_activity_is_details_and_a_start() {
        let a = Activity {
            details: "In the launcher".into(),
            start: 1_700_000_000,
        }
        .to_json();
        assert_eq!(a["details"], "In the launcher");
        assert_eq!(a["timestamps"]["start"], 1_700_000_000i64);
        assert_eq!(a.get("assets").is_some(), !LARGE_IMAGE.is_empty());
    }

    #[test]
    fn back_in_the_launcher_counts_from_the_end_of_the_game() {
        let mut i = on(None);
        note_playing(&mut i, server("A"), 200);
        assert_eq!(i.since, 100, "a game does not move the launcher's start");
        note_playing(&mut i, server("A"), 300);
        assert_eq!(i.since, 100);
        note_playing(&mut i, None, 7_300);
        assert_eq!(i.since, 7_300);
        assert_eq!(desired(&i, true).unwrap().start, 7_300);
        note_playing(&mut i, None, 9_000);
        assert_eq!(i.since, 7_300, "still in the launcher: unchanged");
    }

    use std::sync::Arc;
    use tokio::io::DuplexStream;

    type Log = Arc<Mutex<Vec<bool>>>;
    type Want = Arc<Mutex<Option<Activity>>>;
    type Task = tokio::task::JoinHandle<Result<(), Lost>>;

    /// A session against a fake Discord: the activity it is asked for, what it
    /// said about the connection, its wake-up, and Discord's end of the pipe.
    fn fake_discord(
        want: Option<Activity>,
    ) -> (Want, Log, Arc<tokio::sync::Notify>, DuplexStream, Task) {
        let (ours, theirs) = tokio::io::duplex(64 * 1024);
        let want = Arc::new(Mutex::new(want));
        let log: Log = Arc::default();
        let wake = Arc::new(tokio::sync::Notify::new());
        let (w, l, n) = (want.clone(), log.clone(), wake.clone());
        let task = tokio::spawn(async move {
            session(
                ours,
                "123",
                &n,
                move || w.lock().unwrap().clone(),
                move |on| l.lock().unwrap().push(on),
            )
            .await
        });
        (want, log, wake, theirs, task)
    }

    async fn until(what: impl Fn() -> bool) {
        for _ in 0..200 {
            if what() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("never happened");
    }

    #[tokio::test]
    async fn a_refused_handshake_is_not_a_ready_connection() {
        let (_, log, _, mut discord, task) = fake_discord(Some(Activity {
            details: "In the launcher".into(),
            start: 1,
        }));
        let (op, hello) = frame::read(&mut discord).await.unwrap();
        assert_eq!(
            (op, hello["client_id"].as_str()),
            (frame::OP_HANDSHAKE, Some("123"))
        );
        let no =
            json!({ "evt": "ERROR", "data": { "code": 4000, "message": "Invalid Client ID" } });
        frame::write(&mut discord, frame::OP_FRAME, &no)
            .await
            .unwrap();
        let lost = task.await.unwrap().unwrap_err();
        assert!(!lost.ready, "{lost:?}");
        assert!(log.lock().unwrap().is_empty(), "never said connected");
    }

    #[tokio::test]
    async fn the_session_shows_answers_pings_takes_a_refusal_and_clears() {
        let (want, log, wake, mut discord, task) = fake_discord(Some(Activity {
            details: "Playing on A".into(),
            start: 5,
        }));
        let (op, _) = frame::read(&mut discord).await.unwrap();
        assert_eq!(op, frame::OP_HANDSHAKE);
        let ready = json!({ "cmd": "DISPATCH", "evt": "READY", "data": {} });
        frame::write(&mut discord, frame::OP_FRAME, &ready)
            .await
            .unwrap();

        let (op, set) = frame::read(&mut discord).await.unwrap();
        assert_eq!(
            (op, set["cmd"].as_str()),
            (frame::OP_FRAME, Some("SET_ACTIVITY"))
        );
        assert_eq!(set["args"]["activity"]["details"], "Playing on A");
        assert_eq!(*log.lock().unwrap(), vec![true]);

        frame::write(&mut discord, frame::OP_PING, &json!({ "n": 1 }))
            .await
            .unwrap();
        let (op, pong) = frame::read(&mut discord).await.unwrap();
        assert_eq!((op, pong), (frame::OP_PONG, json!({ "n": 1 })));

        // Refused: not connected any more, and the same activity is not sent again
        // at once.
        let no = json!({ "cmd": "SET_ACTIVITY", "evt": "ERROR", "data": { "code": 4000 } });
        frame::write(&mut discord, frame::OP_FRAME, &no)
            .await
            .unwrap();
        until(|| log.lock().unwrap().last() == Some(&false)).await;
        wake.notify_one();

        // A new activity is offered at once, and its answer says connected again.
        *want.lock().unwrap() = Some(Activity {
            details: "In the launcher".into(),
            start: 9,
        });
        wake.notify_one();
        let (_, set) = frame::read(&mut discord).await.unwrap();
        assert_eq!(set["args"]["activity"]["details"], "In the launcher");
        let ok = json!({ "cmd": "SET_ACTIVITY", "evt": null, "data": {} });
        frame::write(&mut discord, frame::OP_FRAME, &ok)
            .await
            .unwrap();
        until(|| log.lock().unwrap().last() == Some(&true)).await;

        // Turned off: the activity is cleared, the session ends well.
        *want.lock().unwrap() = None;
        wake.notify_one();
        let (op, clear) = frame::read(&mut discord).await.unwrap();
        assert_eq!(
            (op, clear["cmd"].as_str()),
            (frame::OP_FRAME, Some("SET_ACTIVITY"))
        );
        assert!(clear["args"].get("activity").is_none());
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn a_closed_pipe_after_ready_is_a_lost_ready_connection() {
        let (_, _, _, mut discord, task) = fake_discord(Some(Activity {
            details: "x ".into(),
            start: 1,
        }));
        frame::read(&mut discord).await.unwrap();
        let ready = json!({ "evt": "READY" });
        frame::write(&mut discord, frame::OP_FRAME, &ready)
            .await
            .unwrap();
        frame::read(&mut discord).await.unwrap();
        drop(discord);
        let lost = task.await.unwrap().unwrap_err();
        assert!(lost.ready, "{lost:?}");
    }
}
