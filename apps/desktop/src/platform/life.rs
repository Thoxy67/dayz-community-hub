//! The window's life outside its frames: the tray, closing to it, what the launcher
//! does when DayZ starts and stops, and the system notifications.
//!
//! - **Decisions** are pure functions ([`on_close`], [`on_game_start`],
//!   [`on_game_stop`], [`on_tray_lost`], [`should_notify`]) of the settings
//!   ([`LifePrefs`], kept in the preferences as `dzch.life`) and of what the platform
//!   offers ([`Env`]): whether a tray is up, whether the window is on Wayland.
//! - **Doing it** ([`apply`]) is a viewport command: minimise, hide, restore, close.
//!   A hidden window is `ViewportCommand::Visible(false)` on Windows and X11; on
//!   Wayland a window cannot hide itself (winit ignores `set_visible` there, and the
//!   protocol has no un-minimise either), so it is minimised instead and stays in the
//!   task bar. Restoring it there is a best effort: un-minimise and an activation
//!   request, which most compositors honour as "demands attention". A Wayland window
//!   cannot tell either whether it is minimised (winit answers "no" there), so the
//!   launcher remembers it minimised it ([`Shared`]) until the window has the focus
//!   back: the tray's toggle then brings it back instead of minimising it again.
//!   Whether the window is a Wayland one is read from its handle ([`Life::platform`]),
//!   the environment being only the guess until then.
//! - **Never unreachable**: closing hides to the tray only while a tray host shows the
//!   icon. Without one (GNOME without the AppIndicator extension, a failed
//!   registration, a host that went away), closing quits as it always did, and a
//!   window hidden when the host leaves comes back at once. A second launch shows the
//!   window too (`main.rs`, through [`show`]).
//! - **Quitting for real** ([`quit`]: the tray's Quit, `app.quit`, a restart after an
//!   update or an import, "close the launcher" when DayZ starts) marks the close so it
//!   is not turned into a hide; the window then closes the normal way (preferences
//!   flushed in `on_exit`, the hub's writes given their moment in `main`).
//!
//! [`Life`] runs it, once per pass from `eframe::App::logic`, which eframe also calls
//! while the window is hidden or minimised (no egui pass then): it takes the hub's
//! events in itself on those passes, so the game stopping can bring a hidden window
//! back.

use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use dz_hub::HubEvent;
use dz_hub::features::steamcmd::ModProgressKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::notify::{self, Note};
use super::tray::{Tray, TrayMsg, TrayView};
use crate::app::Ctx;
use crate::i18n::{Locale, t, tf};
use crate::store::updater::UpdateState;

// ── the settings ────────────────────────────────────────────────────────────

/// What the launcher does when DayZ starts.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OnGameStart {
    Minimize,
    /// Hide to the tray (minimise when there is none).
    Tray,
    /// Close the launcher; the game is its own process and goes on.
    Close,
    /// Also what an unknown word in the file reads as (so it comes last).
    #[default]
    #[serde(other)]
    Nothing,
}

/// What the launcher does when DayZ stops.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OnGameStop {
    Restore,
    /// Restore and bring to the front.
    Front,
    /// Also what an unknown word in the file reads as (so it comes last).
    #[default]
    #[serde(other)]
    Nothing,
}

/// `dzch.life` in the preferences.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct LifePrefs {
    /// Closing the window hides it to the tray (while there is one).
    pub close_to_tray: bool,
    pub on_game_start: OnGameStart,
    pub on_game_stop: OnGameStop,
    /// System notifications while the window is not in front.
    pub notify: bool,
}

impl Default for LifePrefs {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            on_game_start: OnGameStart::Nothing,
            on_game_stop: OnGameStop::Nothing,
            notify: true,
        }
    }
}

// ── the decisions ───────────────────────────────────────────────────────────

/// What the platform offers now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Env {
    /// A tray host shows the launcher's icon.
    pub tray: bool,
    /// The window is a Wayland one (it cannot hide itself).
    pub wayland: bool,
}

/// What the window does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    Minimize,
    /// Out of sight, back from the tray.
    Hide,
    /// Back on screen, `front` also asks for the focus.
    Restore {
        front: bool,
    },
    Quit,
}

/// Out of the way into the tray: hidden, or minimised where a window cannot hide.
pub fn to_tray(env: Env) -> Act {
    if env.wayland {
        Act::Minimize
    } else {
        Act::Hide
    }
}

/// The window was asked to close (its button, Alt+F4, the window manager).
pub fn on_close(p: &LifePrefs, env: Env, quitting: bool) -> Act {
    if quitting || !p.close_to_tray || !env.tray {
        Act::Quit
    } else {
        to_tray(env)
    }
}

/// DayZ started.
pub fn on_game_start(p: &LifePrefs, env: Env) -> Act {
    match p.on_game_start {
        OnGameStart::Nothing => Act::Nothing,
        OnGameStart::Minimize => Act::Minimize,
        OnGameStart::Tray if env.tray => to_tray(env),
        OnGameStart::Tray => Act::Minimize,
        OnGameStart::Close => Act::Quit,
    }
}

/// DayZ stopped.
pub fn on_game_stop(p: &LifePrefs) -> Act {
    match p.on_game_stop {
        OnGameStop::Nothing => Act::Nothing,
        OnGameStop::Restore => Act::Restore { front: false },
        OnGameStop::Front => Act::Restore { front: true },
    }
}

/// The tray host went away: a hidden window would have no way back.
pub fn on_tray_lost(hidden: bool) -> Act {
    if hidden {
        Act::Restore { front: true }
    } else {
        Act::Nothing
    }
}

/// Whether the window is out of sight: hidden, minimised as the window system says,
/// or (Wayland, where it says nothing) minimised by the launcher and not given the
/// focus since.
pub fn out_of_sight(
    hidden: bool,
    reported_minimized: Option<bool>,
    ours: bool,
    wayland: bool,
) -> bool {
    hidden || reported_minimized == Some(true) || (wayland && ours)
}

/// Where a minimise by the launcher stands, from the focus: it is over once the window
/// lost the focus and got it back (whoever brought it back).
fn follow_focus(state: u8, focused: bool) -> u8 {
    match state {
        MIN_ASKED if !focused => MIN_AWAY,
        MIN_AWAY if focused => MIN_NO,
        s => s,
    }
}

/// The tray icon was clicked: away when on screen, back when not.
pub fn on_tray_toggle(on_screen: bool, env: Env) -> Act {
    if on_screen {
        to_tray(env)
    } else {
        Act::Restore { front: true }
    }
}

/// Whether to tell the system: never while the window is in front.
pub fn should_notify(enabled: bool, focused: bool, hidden: bool) -> bool {
    enabled && (hidden || !focused)
}

/// Which choices the "when DayZ starts" setting offers: hiding to the tray only
/// while there is one.
pub fn start_choices(tray: bool) -> &'static [OnGameStart] {
    if tray {
        &[
            OnGameStart::Nothing,
            OnGameStart::Minimize,
            OnGameStart::Tray,
            OnGameStart::Close,
        ]
    } else {
        &[
            OnGameStart::Nothing,
            OnGameStart::Minimize,
            OnGameStart::Close,
        ]
    }
}

/// Whether this process likely draws a Wayland window (winit's first choice when its
/// socket is named): the guess until the window's handle says ([`Life::platform`]).
pub fn wayland_guess() -> bool {
    cfg!(target_os = "linux")
        && ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]
            .iter()
            .any(|v| std::env::var_os(v).is_some_and(|s| !s.is_empty()))
}

// ── the shared state, and doing it ──────────────────────────────────────────

/// Not minimised by the launcher.
const MIN_NO: u8 = 0;
/// Minimised by the launcher; the window still had the focus when last seen.
const MIN_ASKED: u8 = 1;
/// Minimised by the launcher, and the window has lost the focus since.
const MIN_AWAY: u8 = 2;

/// What threads other than the frame's (the single-instance listener, a restart, a
/// join's flow) and commands need to know, kept in the context's data.
#[derive(Debug)]
pub struct Shared {
    quitting: AtomicBool,
    hidden: AtomicBool,
    tray: AtomicBool,
    wayland: AtomicBool,
    /// `MIN_*`: whether the launcher minimised the window (what a Wayland window cannot
    /// be asked).
    minimized: AtomicU8,
    /// Notes from other threads, taken by the next pass.
    notes: Mutex<Vec<Note>>,
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            quitting: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            tray: AtomicBool::new(false),
            wayland: AtomicBool::new(wayland_guess()),
            minimized: AtomicU8::new(MIN_NO),
            notes: Mutex::new(Vec::new()),
        }
    }
}

impl Shared {
    pub fn env(&self) -> Env {
        Env {
            tray: self.tray.load(Ordering::SeqCst),
            wayland: self.wayland.load(Ordering::SeqCst),
        }
    }

    /// Whether the launcher minimised the window and it has not had the focus since.
    pub fn minimized_by_us(&self) -> bool {
        self.minimized.load(Ordering::SeqCst) != MIN_NO
    }

    fn take_notes(&self) -> Vec<Note> {
        let mut q = self
            .notes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        std::mem::take(&mut *q)
    }

    pub fn hidden(&self) -> bool {
        self.hidden.load(Ordering::SeqCst)
    }

    pub fn quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
}

/// The context's shared state (made on first use).
pub fn shared(ctx: &egui::Context) -> Arc<Shared> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<Shared>>(egui::Id::new("dzch.life"))
            .clone()
    })
}

/// Whether a tray host shows the launcher's icon now.
pub fn tray_available(ctx: &egui::Context) -> bool {
    shared(ctx).env().tray
}

/// Close the launcher for real (not into the tray).
pub fn quit(ctx: &egui::Context) {
    shared(ctx).quitting.store(true, Ordering::SeqCst);
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    ctx.request_repaint();
}

/// A join failed before the game could start (the server not found, its name not
/// resolved…): told by the next pass like the hub's `LaunchError`. Any thread.
pub fn note_launch_failed(ctx: &egui::Context, err: &str) {
    let note = Note {
        title: t("shell", "notifyLaunchFailed").to_string(),
        body: err.chars().take(240).collect(),
    };
    shared(ctx)
        .notes
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(note);
    ctx.request_repaint();
}

/// Show the window and bring it to the front (a second launch, the tray).
pub fn show(ctx: &egui::Context) {
    apply(ctx, Act::Restore { front: true });
}

/// Do `act` to the window. Callable from any thread.
pub fn apply(ctx: &egui::Context, act: Act) {
    use egui::ViewportCommand as V;
    let s = shared(ctx);
    match act {
        Act::Nothing => return,
        Act::Minimize => {
            s.minimized.store(MIN_ASKED, Ordering::SeqCst);
            ctx.send_viewport_cmd(V::Minimized(true));
        }
        Act::Hide => {
            s.hidden.store(true, Ordering::SeqCst);
            ctx.send_viewport_cmd(V::Visible(false));
        }
        Act::Restore { front } => {
            if s.hidden.swap(false, Ordering::SeqCst) {
                ctx.send_viewport_cmd(V::Visible(true));
            }
            // Un-minimising a window that is not minimised raises it on some window
            // managers: only when it is (or the launcher minimised it, which a Wayland
            // window cannot report).
            let ours = s.minimized.swap(MIN_NO, Ordering::SeqCst) != MIN_NO;
            let minimized = ours || ctx.input(|i| i.viewport().minimized) != Some(false);
            if minimized || front {
                ctx.send_viewport_cmd(V::Minimized(false));
            }
            if front {
                ctx.send_viewport_cmd(V::Focus);
            }
        }
        Act::Quit => {
            quit(ctx);
            return;
        }
    }
    ctx.request_repaint();
}

/// The window commands (`app.quit`, `window.hide`, `window.show`) and the settings'
/// (`settings.life.*`), registered in `commands.rs`. Arguments are hostile.
pub fn command(cx: &mut Ctx, what: &str, a: &Value) {
    let on = |cur: bool| a.get("on").and_then(Value::as_bool).unwrap_or(!cur);
    let word = a.get("action").and_then(Value::as_str);
    match what {
        "quit" => quit(cx.egui),
        "hide" => {
            let env = shared(cx.egui).env();
            if env.tray {
                apply(cx.egui, to_tray(env));
            } else {
                apply(cx.egui, Act::Minimize);
            }
        }
        "show" => show(cx.egui),
        "close_to_tray" => {
            let v = on(cx.prefs.get().life.close_to_tray);
            if v != cx.prefs.get().life.close_to_tray {
                cx.prefs.edit().life.close_to_tray = v;
            }
        }
        "notify" => {
            let v = on(cx.prefs.get().life.notify);
            if v != cx.prefs.get().life.notify {
                cx.prefs.edit().life.notify = v;
            }
        }
        "on_game_start" => {
            let v = match word {
                Some("nothing") => OnGameStart::Nothing,
                Some("minimize") => OnGameStart::Minimize,
                Some("tray") => OnGameStart::Tray,
                Some("close") => OnGameStart::Close,
                _ => return,
            };
            if v != cx.prefs.get().life.on_game_start {
                cx.prefs.edit().life.on_game_start = v;
            }
        }
        "on_game_stop" => {
            let v = match word {
                Some("nothing") => OnGameStop::Nothing,
                Some("restore") => OnGameStop::Restore,
                Some("front") => OnGameStop::Front,
                _ => return,
            };
            if v != cx.prefs.get().life.on_game_stop {
                cx.prefs.edit().life.on_game_stop = v;
            }
        }
        _ => {}
    }
}

// ── the driver ──────────────────────────────────────────────────────────────

/// The tray, the game's state as last seen, and what is waiting to be told.
#[derive(Default)]
pub struct Life {
    tray: Option<Tray>,
    /// Whether the tray was up on the previous pass.
    tray_was: bool,
    /// DayZ running, as the hub last said; `None` before its first look.
    game: Option<bool>,
    /// Starts and stops seen since the last pass, oldest first.
    game_turns: Vec<bool>,
    /// The running mod operation listed mods to work on (not a login).
    mod_op_has_mods: bool,
    /// The update version already told about.
    update_told: Option<String>,
    notes: Vec<Note>,
    /// The window, to ask for the pass that handles what an event queued.
    ctx: Option<egui::Context>,
    /// The window's handle was looked at (Wayland or not).
    handle_seen: bool,
    /// What the tray's words were last made from (on screen, language, last server).
    tray_key: Option<(bool, Locale, u64)>,
}

impl Life {
    pub fn new() -> Self {
        Self::default()
    }

    /// The tray `main` started (none in tests and screenshots).
    pub fn set_tray(&mut self, tray: Tray) {
        self.tray = Some(tray);
    }

    /// One of the hub's events (every one, as the stores get them).
    pub fn on_event(&mut self, e: &HubEvent) {
        match e {
            HubEvent::GameRunning(now) => {
                // The first is the tracker's first look: the game was already there,
                // or not; nothing started or stopped.
                if let Some(was) = self.game
                    && was != *now
                {
                    self.game_turns.push(*now);
                    self.wake();
                }
                self.game = Some(*now);
            }
            HubEvent::ModProgress(ev) => match ev.kind {
                ModProgressKind::Queued => self.mod_op_has_mods = !ev.queue.is_empty(),
                // A login lists no mods: nothing to tell when it ends. An operation
                // that stopped before any mod (SteamCMD not logged in, no network)
                // has a hint and no counts: told as failed. (The flag is taken
                // whatever the rest of the guard says.)
                ModProgressKind::Finished
                    if std::mem::take(&mut self.mod_op_has_mods)
                        && (ev.ok + ev.failed > 0 || ev.hint.is_some()) =>
                {
                    self.notes
                        .push(mod_note(ev.ok, ev.failed, ev.hint.as_deref()));
                    self.wake();
                }
                _ => {}
            },
            HubEvent::LaunchError(err) => {
                self.notes.push(Note {
                    title: t("shell", "notifyLaunchFailed").to_string(),
                    body: err.chars().take(240).collect(),
                });
                self.wake();
            }
            _ => {}
        }
    }

    /// The event came in a frame's pass, after this one's: ask for another.
    fn wake(&self) {
        if let Some(ctx) = &self.ctx {
            ctx.request_repaint();
        }
    }

    /// Whether the window is out of sight (hidden, or minimised): the frame's own
    /// work (the hub's events) is then done from here.
    pub fn out_of_sight(&self, ctx: &egui::Context) -> bool {
        let s = shared(ctx);
        out_of_sight(
            s.hidden(),
            ctx.input(|i| i.viewport().minimized),
            s.minimized_by_us(),
            s.env().wayland,
        )
    }

    /// Read from the window's handle whether it is a Wayland one (winit may have
    /// fallen back to X11 whatever the environment says). Once, from `logic`.
    pub fn platform(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
        if self.handle_seen {
            return;
        }
        if let Ok(h) = frame.window_handle() {
            self.handle_seen = true;
            let wayland = matches!(h.as_raw(), RawWindowHandle::Wayland(_));
            shared(ctx).wayland.store(wayland, Ordering::SeqCst);
        }
    }

    /// One pass, from `eframe::App::logic` (before the frame, and alone while the
    /// window is out of sight).
    pub fn frame(&mut self, cx: &mut Ctx) {
        let ctx = cx.egui.clone();
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }
        let s = shared(&ctx);
        let p = cx.prefs.get().life;
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(i.focused));
        let m = s.minimized.load(Ordering::SeqCst);
        let next = follow_focus(m, focused);
        if next != m {
            // Only from the state seen: a minimise asked meanwhile is kept.
            let _ = s
                .minimized
                .compare_exchange(m, next, Ordering::SeqCst, Ordering::SeqCst);
        }

        // The tray: whether it is up, and what was picked in it.
        let away = self.out_of_sight(&ctx);
        if let Some(tray) = &mut self.tray {
            tray.poll(&ctx, away);
        }
        let up = self.tray.as_ref().is_some_and(Tray::available);
        s.tray.store(up, Ordering::SeqCst);
        if self.tray_was && !up {
            apply(&ctx, on_tray_lost(s.hidden()));
        }
        self.tray_was = up;
        let picked: Vec<TrayMsg> = self
            .tray
            .as_ref()
            .map(|t| std::iter::from_fn(|| t.try_recv()).take(16).collect())
            .unwrap_or_default();
        let env = s.env();
        for m in picked {
            match m {
                TrayMsg::Toggle => apply(&ctx, on_tray_toggle(self.on_screen(&ctx), env)),
                TrayMsg::Quit => quit(&ctx),
                TrayMsg::Rejoin => {
                    show(&ctx);
                    let _ = cx.run("rejoin", &Value::Null);
                }
            }
        }

        // A close asked for (the title bar, Alt+F4, the window manager).
        close_asked(&ctx, &p, &s);

        // DayZ started or stopped.
        for now in std::mem::take(&mut self.game_turns) {
            let act = if now {
                on_game_start(&p, env)
            } else {
                on_game_stop(&p)
            };
            apply(&ctx, act);
        }

        // A launcher update was found.
        let u = &cx.stores.updater;
        if u.state == UpdateState::Available
            && let Some(info) = &u.info
            && self.update_told.as_deref() != Some(info.version.as_str())
        {
            self.update_told = Some(info.version.clone());
            self.notes.push(Note {
                title: t("shell", "notifyUpdate").to_string(),
                body: tf("shell", "notifyUpdateBody", &[("version", &info.version)]),
            });
        }

        // What to tell the system, unless the window is in front.
        self.notes.extend(s.take_notes());
        if !self.notes.is_empty() {
            let tell = should_notify(p.notify, focused, self.out_of_sight(&ctx));
            for n in std::mem::take(&mut self.notes) {
                if tell {
                    notify::send(n);
                }
            }
        }

        // The tray's words follow the language, the window and the last server; made
        // again only when one of them moved.
        if self.tray.is_some() {
            let on_screen = self.on_screen(&ctx);
            let last = cx.stores.profile.history.first().map(|h| h.name.as_str());
            let mut h = rustc_hash::FxHasher::default();
            last.hash(&mut h);
            let key = (on_screen, crate::i18n::locale(), h.finish());
            if self.tray_key != Some(key)
                && let Some(tray) = &mut self.tray
            {
                self.tray_key = Some(key);
                tray.set_view(tray_view(on_screen, last));
            }
        }
    }

    fn on_screen(&self, ctx: &egui::Context) -> bool {
        !self.out_of_sight(ctx)
    }
}

/// A close was asked for (the title bar, Alt+F4, the window manager): cancelled and
/// turned into a trip to the tray only while one shows the icon and it is no real quit.
fn close_asked(ctx: &egui::Context, p: &LifePrefs, s: &Shared) {
    if ctx.input(|i| i.viewport().close_requested()) {
        match on_close(p, s.env(), s.quitting()) {
            Act::Quit => s.quitting.store(true, Ordering::SeqCst),
            act => {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                apply(ctx, act);
            }
        }
    }
}

/// The tray's words.
pub fn tray_view(on_screen: bool, last_server: Option<&str>) -> TrayView {
    let toggle = if on_screen { "trayHide" } else { "trayShow" };
    TrayView {
        tooltip: "DayZ Community Hub".to_string(),
        toggle: t("shell", toggle).to_string(),
        rejoin: last_server
            // A server's name is anyone's words: no line break or tab in a menu.
            .map(|n| {
                n.chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect::<String>()
            })
            .map(|n| n.trim().chars().take(48).collect::<String>())
            .filter(|n| !n.is_empty())
            .map(|n| tf("shell", "trayRejoin", &[("server", &n)])),
        quit: t("shell", "trayQuit").to_string(),
    }
}

fn mod_note(ok: usize, failed: usize, hint: Option<&str>) -> Note {
    if failed > 0 {
        Note {
            title: t("shell", "notifyModsFailed").to_string(),
            body: tf(
                "shell",
                "notifyModsFailedBody",
                &[("ok", &ok), ("failed", &failed)],
            ),
        }
    } else if let Some(hint) = hint {
        // Stopped before (or after) the mods, for a reason SteamCMD gave.
        Note {
            title: t("shell", "notifyModsFailed").to_string(),
            body: hint.trim().chars().take(240).collect(),
        }
    } else {
        Note {
            title: t("shell", "notifyModsDone").to_string(),
            body: tf("shell", "notifyModsDoneBody", &[("count", &ok)]),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const TRAY: Env = Env {
        tray: true,
        wayland: false,
    };
    const NO_TRAY: Env = Env {
        tray: false,
        wayland: false,
    };
    const WAYLAND: Env = Env {
        tray: true,
        wayland: true,
    };

    #[test]
    fn closing_hides_only_while_a_tray_shows_the_icon() {
        let p = LifePrefs::default();
        assert!(p.close_to_tray && p.notify);
        assert_eq!(on_close(&p, TRAY, false), Act::Hide);
        assert_eq!(on_close(&p, WAYLAND, false), Act::Minimize);
        // No tray host: the window must never be left invisible.
        assert_eq!(on_close(&p, NO_TRAY, false), Act::Quit);
        // Quit from the tray, a restart: a real close.
        assert_eq!(on_close(&p, TRAY, true), Act::Quit);
        let off = LifePrefs {
            close_to_tray: false,
            ..p
        };
        assert_eq!(on_close(&off, TRAY, false), Act::Quit);
    }

    #[test]
    fn the_game_starting_and_stopping() {
        let mut p = LifePrefs::default();
        assert_eq!(on_game_start(&p, TRAY), Act::Nothing);
        assert_eq!(on_game_stop(&p), Act::Nothing);
        p.on_game_start = OnGameStart::Minimize;
        assert_eq!(on_game_start(&p, NO_TRAY), Act::Minimize);
        p.on_game_start = OnGameStart::Tray;
        assert_eq!(on_game_start(&p, TRAY), Act::Hide);
        assert_eq!(on_game_start(&p, WAYLAND), Act::Minimize);
        // The tray went away since the choice: minimised, never hidden.
        assert_eq!(on_game_start(&p, NO_TRAY), Act::Minimize);
        p.on_game_start = OnGameStart::Close;
        assert_eq!(on_game_start(&p, NO_TRAY), Act::Quit);
        p.on_game_stop = OnGameStop::Restore;
        assert_eq!(on_game_stop(&p), Act::Restore { front: false });
        p.on_game_stop = OnGameStop::Front;
        assert_eq!(on_game_stop(&p), Act::Restore { front: true });
    }

    #[test]
    fn the_tray_and_its_loss() {
        assert_eq!(on_tray_lost(true), Act::Restore { front: true });
        assert_eq!(on_tray_lost(false), Act::Nothing);
        assert_eq!(on_tray_toggle(true, TRAY), Act::Hide);
        assert_eq!(on_tray_toggle(true, WAYLAND), Act::Minimize);
        assert_eq!(on_tray_toggle(false, TRAY), Act::Restore { front: true });
        // Wayland: minimised by the launcher, the window still says "not minimised";
        // the toggle brings it back.
        let away = out_of_sight(false, Some(false), true, true);
        assert!(away);
        assert_eq!(on_tray_toggle(!away, WAYLAND), Act::Restore { front: true });
        // Elsewhere the window's own answer counts.
        assert!(!out_of_sight(false, Some(false), true, false));
        assert!(out_of_sight(true, Some(false), false, false));
        assert!(out_of_sight(false, Some(true), false, false));
        assert!(start_choices(true).contains(&OnGameStart::Tray));
        assert!(!start_choices(false).contains(&OnGameStart::Tray));
    }

    #[test]
    fn a_minimise_is_over_once_the_focus_left_and_came_back() {
        // Still focused right after the ask: kept.
        assert_eq!(follow_focus(MIN_ASKED, true), MIN_ASKED);
        assert_eq!(follow_focus(MIN_ASKED, false), MIN_AWAY);
        assert_eq!(follow_focus(MIN_AWAY, false), MIN_AWAY);
        assert_eq!(follow_focus(MIN_AWAY, true), MIN_NO);
        assert_eq!(follow_focus(MIN_NO, false), MIN_NO);
    }

    /// One pass with a close asked for; whether it was cancelled.
    fn close_pass(ctx: &egui::Context, p: &LifePrefs) -> bool {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let mut out = ctx.run_ui(input, |ui| close_asked(ui.ctx(), p, &shared(ui.ctx())));
        // The font atlas the first pass made is nobody's to upload here.
        out.textures_delta.clear();
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| v.commands.contains(&egui::ViewportCommand::CancelClose))
    }

    #[test]
    fn a_close_is_cancelled_only_while_a_tray_shows_the_icon() {
        let ctx = egui::Context::default();
        let p = LifePrefs::default();
        let s = shared(&ctx);
        s.wayland.store(false, Ordering::SeqCst);
        assert!(!close_pass(&ctx, &p), "no tray: the window closes");
        assert!(s.quitting());
        s.quitting.store(false, Ordering::SeqCst);
        s.tray.store(true, Ordering::SeqCst);
        assert!(close_pass(&ctx, &p), "a tray: into it");
        assert!(s.hidden());
        s.hidden.store(false, Ordering::SeqCst);
        let off = LifePrefs {
            close_to_tray: false,
            ..p
        };
        assert!(!close_pass(&ctx, &off));
        s.quitting.store(false, Ordering::SeqCst);
        // A real quit (the tray's Quit, a restart) is never turned into a hide.
        quit(&ctx);
        assert!(!close_pass(&ctx, &p));
    }

    #[test]
    fn a_join_failed_elsewhere_is_queued_for_the_pass() {
        let ctx = egui::Context::default();
        note_launch_failed(&ctx, "Server not found");
        let notes = shared(&ctx).take_notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].body, "Server not found");
        assert!(shared(&ctx).take_notes().is_empty());
    }

    #[test]
    fn notifications_only_when_the_window_is_not_in_front() {
        assert!(!should_notify(true, true, false));
        assert!(should_notify(true, false, false));
        assert!(should_notify(true, true, true));
        assert!(!should_notify(false, false, true));
    }

    #[test]
    fn the_game_turns_follow_the_trackers_first_look() {
        let mut l = Life::new();
        l.on_event(&HubEvent::GameRunning(true));
        assert!(l.game_turns.is_empty(), "the first look is no start");
        l.on_event(&HubEvent::GameRunning(true));
        l.on_event(&HubEvent::GameRunning(false));
        l.on_event(&HubEvent::GameRunning(true));
        assert_eq!(l.game_turns, [false, true]);
        l.on_event(&HubEvent::LaunchError("no Steam".into()));
        assert_eq!(l.notes.len(), 1);
        assert_eq!(l.notes[0].body, "no Steam");
    }

    fn step(kind: ModProgressKind, queued: usize, ok: usize, failed: usize) -> HubEvent {
        step_hint(kind, queued, ok, failed, None)
    }

    fn step_hint(
        kind: ModProgressKind,
        queued: usize,
        ok: usize,
        failed: usize,
        hint: Option<&str>,
    ) -> HubEvent {
        use dz_hub::features::steamcmd::{ModProgressEvent, QueuedModDto};
        HubEvent::ModProgress(Box::new(ModProgressEvent {
            kind,
            current: 0,
            total: queued,
            mod_id: 0,
            name: String::new(),
            ok,
            failed,
            hint: hint.map(str::to_string),
            log_line: None,
            bytes: None,
            queue: (0..queued)
                .map(|i| QueuedModDto {
                    id: i as u64,
                    name: format!("@mod{i}"),
                })
                .collect(),
            seq: 1,
        }))
    }

    #[test]
    fn a_mod_operation_ending_is_told_but_a_login_is_not() {
        let mut l = Life::new();
        // A login: nothing queued, one "ok".
        l.on_event(&step(ModProgressKind::Queued, 0, 0, 0));
        l.on_event(&step(ModProgressKind::Finished, 0, 1, 0));
        assert!(l.notes.is_empty());
        l.on_event(&step(ModProgressKind::Queued, 3, 0, 0));
        l.on_event(&step(ModProgressKind::Finished, 0, 3, 0));
        l.on_event(&step(ModProgressKind::Queued, 2, 0, 0));
        l.on_event(&step(ModProgressKind::Finished, 0, 1, 1));
        assert_eq!(l.notes.len(), 2);
        assert_ne!(l.notes[0].title, l.notes[1].title, "done, then failed");
        assert!(l.notes[1].body.contains('1'));
        // Stopped before any mod (not logged in, no network): told as failed.
        l.on_event(&step(ModProgressKind::Queued, 2, 0, 0));
        l.on_event(&step_hint(
            ModProgressKind::Finished,
            0,
            0,
            0,
            Some("not logged in"),
        ));
        assert_eq!(l.notes.len(), 3);
        assert_eq!(l.notes[2].title, l.notes[1].title);
        assert_eq!(l.notes[2].body, "not logged in");
        // A login that failed with a hint: still nothing.
        l.on_event(&step(ModProgressKind::Queued, 0, 0, 0));
        l.on_event(&step_hint(
            ModProgressKind::Finished,
            0,
            0,
            0,
            Some("bad password"),
        ));
        assert_eq!(l.notes.len(), 3);
    }

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let p: LifePrefs = serde_json::from_str(r#"{"onGameStart": "tray"}"#).unwrap();
        assert_eq!(p.on_game_start, OnGameStart::Tray);
        assert!(p.close_to_tray && p.notify);
        let p = LifePrefs {
            close_to_tray: false,
            on_game_start: OnGameStart::Close,
            on_game_stop: OnGameStop::Front,
            notify: false,
        };
        let back: LifePrefs = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back, p);
        // A word from elsewhere (a newer version, a hand edit) is "nothing", and the
        // rest of the preferences survive it.
        let p: LifePrefs =
            serde_json::from_str(r#"{"onGameStop": "bogus", "notify": false}"#).unwrap();
        assert_eq!(p.on_game_stop, OnGameStop::Nothing);
        assert!(!p.notify);
    }

    #[test]
    fn the_trays_words() {
        let v = tray_view(true, Some("  "));
        assert!(v.rejoin.is_none());
        assert!(!v.toggle.is_empty() && !v.quit.is_empty());
        let v = tray_view(false, Some("My Server"));
        assert!(v.rejoin.unwrap().contains("My Server"));
        assert_ne!(tray_view(true, None).toggle, v.toggle);
        let v = tray_view(true, Some("A\nB\tC"));
        let r = v.rejoin.unwrap();
        assert!(r.contains("A B C") && !r.contains('\n') && !r.contains('\t'));
        assert!(tray_view(true, Some("\n\t")).rejoin.is_none());
    }
}
