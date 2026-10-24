//! Whether DayZ is running, for the rail's "close the game" block (=
//! `stores/game.svelte.ts`). Asked once at start and once after closing the game; the
//! hub's play tracker, the app's one watcher of the game, says the rest
//! (`HubEvent::GameRunning`), launched from here or not.
//!
//! Also the installed game's version (`1.30.164200`), which server rows compare theirs
//! with: read at start and asked again with BattlEye's state (the hub reads the
//! executable only when it changed).
//!
//! While it runs, the session the hub's tracker has open (the server played, its start),
//! asked when the game starts and again when the play log changes.

use std::time::{Duration, Instant};

use dz_hub::features::stats::SessionDto;
use dz_hub::{Hub, HubEvent};

use crate::rt::{self, Task};

/// How long after a launch the game may take to appear, for its start to be timed.
const START_WINDOW: Duration = Duration::from_secs(3 * 60);

pub struct Game {
    hub: Hub,
    ctx: egui::Context,
    pub running: bool,
    pub closing: bool,
    /// When the game was seen to start after a launch from here; `None` when unknown.
    pub since: Option<Instant>,
    watch_until: Option<Instant>,
    check: Option<Task<bool>>,
    kill: Option<Task<Result<u32, String>>>,
    /// The installed game's version; `None` until known (or DayZ is not found).
    pub version: Option<String>,
    version_task: Option<Task<Option<String>>>,
    /// What is being played while the game runs (`None` when unknown or stopped).
    pub session: Option<SessionDto>,
    session_task: Option<Task<Option<SessionDto>>>,
}

impl Game {
    pub fn new(hub: Hub, ctx: egui::Context) -> Self {
        Self {
            hub,
            ctx,
            running: false,
            closing: false,
            since: None,
            watch_until: None,
            check: None,
            kill: None,
            version: None,
            version_task: None,
            session: None,
            session_task: None,
        }
    }

    /// At start: the game may already be open.
    pub fn init(&mut self) {
        self.check_now();
        self.read_version();
    }

    /// Ask for the installed game's version (cheap when it has not changed).
    pub fn read_version(&mut self) {
        if self.version_task.is_some() {
            return;
        }
        let hub = self.hub.clone();
        self.version_task = Some(rt::spawn(
            async move { hub.game_version().await.ok().flatten() },
        ));
    }

    /// A launch was handed to Steam: the game that appears soon was started from here.
    pub fn launched(&mut self) {
        self.watch_until = Some(Instant::now() + START_WINDOW);
    }

    /// What the hub said: the tracker saw the game start or stop.
    pub fn on_event(&mut self, ev: &HubEvent) {
        match ev {
            HubEvent::GameRunning(now) => {
                self.set_running(*now);
                self.ctx.request_repaint();
            }
            HubEvent::PlaySessionsChanged(_) if self.running => self.read_session(),
            _ => {}
        }
    }

    /// Kill the game (the rail asks first).
    pub fn close(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        let hub = self.hub.clone();
        self.kill = Some(rt::spawn(async move { hub.kill_game().await }));
    }

    /// Ask the hub what is being played (the tracker's open session).
    fn read_session(&mut self) {
        if self.session_task.is_some() {
            return;
        }
        let hub = self.hub.clone();
        self.session_task = Some(rt::spawn(
            async move { hub.play_current().await.ok().flatten() },
        ));
    }

    /// How long the game has run, in seconds: from the tracker's session when it was
    /// measured, else from when it was seen to start after a launch from here.
    pub fn played_secs(&self) -> Option<i64> {
        if let Some(s) = self.session.as_ref().filter(|s| s.measured) {
            let now = dz_common::time::now_secs() as i64;
            return Some((now - s.start).max(0));
        }
        self.since
            .map(|s| i64::try_from(s.elapsed().as_secs()).unwrap_or(0))
    }

    fn check_now(&mut self) {
        if self.check.is_some() {
            return;
        }
        let hub = self.hub.clone();
        self.check = Some(rt::spawn(async move {
            hub.game_running().await.unwrap_or(false)
        }));
    }

    /// The game is running or not, as just learnt. Only a start soon after a launch from
    /// here has a known time.
    fn set_running(&mut self, now: bool) -> bool {
        let was = self.running;
        self.running = now;
        let watching = self.watch_until.is_some_and(|u| Instant::now() < u);
        if !now {
            self.since = None;
            self.session = None;
        } else if !was {
            self.read_session();
            if watching {
                self.since = Some(Instant::now());
                self.watch_until = None;
            }
        }
        if !watching {
            self.watch_until = None;
        }
        was != now
    }

    pub fn poll(&mut self) -> bool {
        let mut version_changed = false;
        if let Some(t) = &mut self.version_task
            && let Some(v) = t.poll()
        {
            self.version_task = None;
            version_changed = self.version != v;
            self.version = v;
        }
        let mut session_changed = false;
        if let Some(t) = &mut self.session_task
            && let Some(s) = t.poll()
        {
            self.session_task = None;
            // An answer that lands after the game stopped is stale.
            let s = s.filter(|_| self.running);
            session_changed = self.session != s;
            self.session = s;
        }
        self.poll_running() || version_changed || session_changed
    }

    fn poll_running(&mut self) -> bool {
        let mut changed = false;
        if let Some(t) = &mut self.kill
            && let Some(_r) = t.poll()
        {
            // Whatever happened, the next check says where the game stands.
            self.kill = None;
            self.closing = false;
            self.check_now();
            changed = true;
        }
        if let Some(t) = &mut self.check
            && let Some(now) = t.poll()
        {
            self.check = None;
            changed |= self.set_running(now);
        }
        changed
    }
}
