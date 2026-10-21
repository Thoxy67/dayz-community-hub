//! The app as a whole (= `stores/app.svelte.ts`): whether start-up finished, the setup
//! wizard, the window's focus, and what is refreshed when the player comes back. (The
//! view on screen is [`crate::app::Nav`]'s.)
//!
//! Start-up: the wizard shows at once on a first launch while the server list loads
//! behind it; a cached list shows immediately and is refreshed in the background. The
//! list goes first: nothing waits on the network or the disk before it.

use std::time::{Duration, Instant};

use dz_hub::Hub;
use dz_hub::features::servers::InitResult;

use super::mods::Mods;
use super::profile::Profile;
use super::servers::Servers;
use super::{Bus, Signal};
use crate::rt::{self, Task};

/// Away this long and the server list is refreshed on return.
const AWAY_SERVERS: Duration = Duration::from_secs(5 * 60);
/// Away this long and mods are checked for updates on return.
const AWAY_MODS: Duration = Duration::from_secs(10 * 60);
/// Mods are checked for updates this often while the window is in use.
const MODS_EVERY: Duration = Duration::from_secs(30 * 60);

pub struct AppState {
    hub: Hub,
    bus: Bus,
    pub initialized: bool,
    pub init_error: Option<String>,
    pub show_wizard: bool,
    pub maximized: bool,
    pub focused: bool,
    /// The hub had already started (a second window in the same process).
    pub resumed: bool,
    away_at: Option<Instant>,
    started: bool,
    first: Option<Task<bool>>,
    init: Option<Task<Result<InitResult, String>>>,
    profile_seen: bool,
    mods_seen: bool,
    checked_once: bool,
    next_mods_check: Option<Instant>,
}

impl AppState {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            initialized: false,
            init_error: None,
            show_wizard: false,
            maximized: false,
            focused: true,
            resumed: false,
            away_at: None,
            started: false,
            first: None,
            init: None,
            profile_seen: false,
            mods_seen: false,
            checked_once: false,
            next_mods_check: None,
        }
    }

    /// Start-up, once (again after a failure: "Retry").
    pub fn init(&mut self) {
        if self.started && self.init_error.is_none() {
            return;
        }
        self.started = true;
        self.init_error = None;
        let hub = self.hub.clone();
        self.first = Some(rt::spawn(async move { hub.check_first_launch().await }));
    }

    /// The window lost the player's attention.
    pub fn away(&mut self) {
        if self.away_at.is_none() {
            self.away_at = Some(Instant::now());
        }
        self.focused = false;
    }

    /// The window is back: refresh what went stale meanwhile.
    pub fn back(&mut self, servers: &mut Servers, mods: &mut Mods, profile: &Profile) {
        self.focused = true;
        let Some(at) = self.away_at.take() else {
            return;
        };
        let gone = at.elapsed();
        if gone >= AWAY_SERVERS && self.initialized {
            servers.refresh(true);
        }
        if gone >= AWAY_MODS && self.initialized {
            mods.check_updates(false, profile.has_api_key());
        }
    }

    pub fn poll(&mut self, _signals: &mut [Signal]) -> bool {
        let mut changed = false;
        if let Some(t) = &mut self.first
            && let Some(first) = t.poll()
        {
            self.first = None;
            changed = true;
            if first {
                self.show_wizard = true;
                self.initialized = true;
            }
            let hub = self.hub.clone();
            self.init = Some(rt::spawn(async move { hub.initialize().await }));
        }
        changed
    }

    /// The answer to `initialize`, taken in by [`super::Stores::poll`] since it starts
    /// work in every store.
    pub fn take_init(&mut self) -> Option<Result<InitResult, String>> {
        let r = self.init.as_mut()?.poll()?;
        self.init = None;
        Some(r)
    }

    pub fn init_failed(&mut self, e: String) {
        self.init_error = Some(e);
    }

    pub fn init_done(&mut self, r: &InitResult) {
        self.resumed = r.resumed;
        self.initialized = true;
        self.next_mods_check = Some(Instant::now() + MODS_EVERY);
    }

    /// The profile was read: what needed it goes on.
    pub fn profile_loaded(
        &mut self,
        mods: &mut Mods,
        profile: &mut Profile,
        _c: &mut super::connect::Connect,
    ) {
        if !self.profile_seen && self.initialized {
            self.profile_seen = true;
            profile.load_avatar();
            self.maybe_check(mods, profile);
        }
    }

    /// The mods on disk were read.
    pub fn mods_loaded(&mut self, mods: &mut Mods, profile: &Profile) {
        if !self.mods_seen {
            self.mods_seen = true;
            self.maybe_check(mods, profile);
        }
    }

    /// The Workshop check needs the Steam API key from the profile; a resumed hub
    /// keeps the last answer, which the disk read shows.
    fn maybe_check(&mut self, mods: &mut Mods, profile: &Profile) {
        if self.profile_seen && self.mods_seen && !self.checked_once {
            self.checked_once = true;
            if !self.resumed {
                mods.check_updates(false, profile.has_api_key());
            }
        }
    }

    /// The list's first load: follow what the hub is doing.
    pub fn servers_loaded(&mut self, servers: &mut Servers) {
        servers.resume();
    }

    /// Mods are checked for updates every half hour while the window is in use; not
    /// while it sits in the background (`back` checks on return).
    pub fn tick(&mut self, mods: &mut Mods, profile: &Profile, _servers: &mut Servers) -> bool {
        let Some(due) = self.next_mods_check else {
            return false;
        };
        let now = Instant::now();
        if now < due {
            self.bus.ctx().request_repaint_after(due - now);
            return false;
        }
        self.next_mods_check = Some(now + MODS_EVERY);
        if self.focused {
            mods.check_updates(true, profile.has_api_key());
        }
        false
    }
}
