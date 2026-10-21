//! Whether DayZ is running, for the rail's "close the game" block (=
//! `stores/game.svelte.ts`). Asked at start, then every few seconds from a launch until
//! the game has come and gone: a process scan is cheap, but not worth running all day.
//! The next check is a `request_repaint_after`, not a timer thread.

use std::time::{Duration, Instant};

use dz_hub::Hub;

use crate::rt::{self, Task};

/// How long after a launch the game may take to appear.
const START_WINDOW: Duration = Duration::from_secs(3 * 60);
const EVERY: Duration = Duration::from_secs(5);

pub struct Game {
    hub: Hub,
    ctx: egui::Context,
    pub running: bool,
    pub closing: bool,
    /// When the game was seen to start after a launch from here; `None` when unknown.
    pub since: Option<Instant>,
    watch_until: Option<Instant>,
    next: Option<Instant>,
    check: Option<Task<bool>>,
    kill: Option<Task<Result<u32, String>>>,
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
            next: None,
            check: None,
            kill: None,
        }
    }

    /// At start: the game may already be open.
    pub fn init(&mut self) {
        self.check_now();
    }

    /// A launch was handed to Steam: watch for the game.
    pub fn launched(&mut self) {
        self.watch_until = Some(Instant::now() + START_WINDOW);
        self.schedule();
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

    fn check_now(&mut self) {
        if self.check.is_some() {
            return;
        }
        self.next = None;
        let hub = self.hub.clone();
        self.check = Some(rt::spawn(async move {
            hub.game_running().await.unwrap_or(false)
        }));
    }

    fn schedule(&mut self) {
        self.next = Some(Instant::now() + EVERY);
        self.ctx.request_repaint_after(EVERY);
    }

    pub fn poll(&mut self) -> bool {
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
            let was = self.running;
            self.running = now;
            // Only a start this window watched for has a known time.
            if !now {
                self.since = None;
            } else if !was && self.watch_until.is_some() {
                self.since = Some(Instant::now());
            }
            let watching = self.watch_until.is_some_and(|u| Instant::now() < u);
            if now || watching {
                self.schedule();
            } else {
                self.watch_until = None;
            }
            changed |= was != now;
        }
        if let Some(due) = self.next {
            let now = Instant::now();
            if now >= due {
                self.check_now();
            } else {
                self.ctx.request_repaint_after(due - now);
            }
        }
        changed
    }
}
