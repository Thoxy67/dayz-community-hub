//! App-wide state (the Svelte interface's `lib/stores`, ported): what more than one view reads.
//!
//! Each store keeps the state its Svelte namesake kept, with the same actions and derived
//! values, and reaches the hub through [`crate::rt`]: an action spawns the hub call and
//! returns at once; the answer lands in a [`crate::rt::Task`] the store polls each frame.
//! Each store has
//!
//! - `on_event(&mut self, &HubEvent)`: what the hub says unasked;
//! - `poll(&mut self) -> bool` (or with a few arguments): take finished answers in,
//!   true when something on screen changed;
//! - actions as methods, optimistic where the Svelte store was (favourites, launch
//!   options, the rail…): the screen changes at once, a failed write reloads.
//!
//! What a store cannot do alone goes through the [`Bus`]: a toast (from any thread), a
//! question to the player (`bus.confirm(req).await`, `bus.password(name).await`, which
//! the shell's dialog host draws from [`dialogs::Dialogs`]), or a closure run on the UI
//! thread with the whole [`crate::app::Ctx`] (`bus.run(|cx| …)`), which is how an async
//! flow (joining a server) comes back to the stores and the navigation.
//!
//! [`Stores`] routes between them: one store's answer that another reacts to is a
//! [`Signal`], handled in [`Stores::poll`].

pub mod app;
pub mod battleye;
pub mod connect;
pub mod dialogs;
pub mod direct;
pub mod friends;
pub mod game;
pub mod mods;
pub mod pad;
pub mod profile;
pub mod server_data;
pub mod servers;
pub mod sources;
pub mod theme;
pub mod updater;

use std::sync::mpsc;

use dz_hub::{Hub, HubEvent};
use tokio::sync::oneshot;

use crate::app::Ctx;
use crate::kit::ToastTone;
use crate::kit::toast;
use crate::views::ViewId;

pub use dialogs::{ConfirmRequest, PasswordAnswer};

/// A closure the UI thread runs with the whole context.
pub type RunFn = Box<dyn for<'a, 'b> FnOnce(&'a mut Ctx<'b>) + Send>;

/// What reaches the UI thread through the [`Bus`].
pub enum Effect {
    /// Ask "are you sure"; the answer goes back on the channel (dropped = no).
    Confirm(ConfirmRequest, oneshot::Sender<bool>),
    /// Ask a server's password; `None` when the player gives up.
    Password(String, oneshot::Sender<Option<PasswordAnswer>>),
    /// Show a view (and a part of it: `settings#apis`).
    Go(ViewId, Option<String>),
    /// Run on the UI thread.
    Run(RunFn),
}

/// How soon a [`Bus`] wakes the window: any delay above zero makes egui run one pass
/// for the wake instead of two, and merges what arrives within it.
const WAKE: std::time::Duration = std::time::Duration::from_millis(4);

/// The way back to the UI thread, from anywhere: cheap to clone, `Send`.
#[derive(Clone)]
pub struct Bus {
    tx: mpsc::Sender<Effect>,
    ctx: egui::Context,
}

impl Bus {
    pub fn new(ctx: egui::Context) -> (Bus, mpsc::Receiver<Effect>) {
        let (tx, rx) = mpsc::channel();
        (Bus { tx, ctx }, rx)
    }

    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    pub fn send(&self, e: Effect) {
        let _ = self.tx.send(e);
        self.ctx.request_repaint_after(WAKE);
    }

    /// Run `f` on the UI thread with the whole context, on the next frame.
    pub fn run(&self, f: impl for<'a, 'b> FnOnce(&'a mut Ctx<'b>) + Send + 'static) {
        self.send(Effect::Run(Box::new(f)));
    }

    pub fn go(&self, view: ViewId) {
        self.send(Effect::Go(view, None));
    }

    pub fn go_focus(&self, view: ViewId, focus: impl Into<String>) {
        self.send(Effect::Go(view, Some(focus.into())));
    }

    // ── toasts (= `say`) ────────────────────────────────────────────────────

    /// The next step of a story told in one toast (`"launch"`: starting, waiting for
    /// Steam, started): replaces the step before.
    pub fn step(&self, topic: &'static str, text: impl Into<String>, tone: ToastTone) {
        toast::say_topic(&self.ctx, topic, text, tone);
        self.ctx.request_repaint_after(WAKE);
    }

    pub fn info(&self, text: impl Into<String>) {
        toast::say(&self.ctx, text, ToastTone::Neutral);
        self.ctx.request_repaint_after(WAKE);
    }

    pub fn ok(&self, text: impl Into<String>) {
        toast::ok(&self.ctx, text);
        self.ctx.request_repaint_after(WAKE);
    }

    pub fn warn(&self, text: impl Into<String>) {
        toast::warn(&self.ctx, text);
        self.ctx.request_repaint_after(WAKE);
    }

    pub fn err(&self, text: impl Into<String>) {
        toast::err(&self.ctx, text);
        self.ctx.request_repaint_after(WAKE);
    }

    /// A toast with a button running a command line (`"view.friends"`, or an id and
    /// its JSON arguments: `connect.address {"ip":"1.2.3.4","port":2302}`).
    pub fn say_with_action(
        &self,
        text: impl Into<String>,
        tone: ToastTone,
        label: impl Into<String>,
        command: impl Into<String>,
    ) {
        toast::say_with_action(&self.ctx, text, tone, label, command);
        self.ctx.request_repaint_after(WAKE);
    }

    // ── questions ───────────────────────────────────────────────────────────

    /// Ask "are you sure"; false when the player says no or the window goes.
    pub async fn confirm(&self, req: ConfirmRequest) -> bool {
        let (tx, rx) = oneshot::channel();
        self.send(Effect::Confirm(req, tx));
        rx.await.unwrap_or(false)
    }

    /// Ask the password to join `server` with.
    pub async fn password(&self, server: String) -> Option<PasswordAnswer> {
        let (tx, rx) = oneshot::channel();
        self.send(Effect::Password(server, tx));
        rx.await.ok().flatten()
    }

    /// Ask, then run `then` on the UI thread if the answer is yes.
    pub fn confirm_then(
        &self,
        req: ConfirmRequest,
        then: impl for<'a, 'b> FnOnce(&'a mut Ctx<'b>) + Send + 'static,
    ) {
        let bus = self.clone();
        crate::rt::fire(async move {
            if bus.confirm(req).await {
                bus.run(then);
            }
        });
    }
}

/// Something a store finished that others react to.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    /// The profile was read (or re-read).
    ProfileLoaded,
    /// The account settings were saved: the title bar's figures and the avatar follow.
    AccountSaved,
    /// SteamCMD logged in: the profile says so now.
    SteamcmdLoggedIn,
    /// The mods on disk were read (`ModsChanged` comes first when they moved).
    ModsLoaded,
    /// What the rows show about mods changed (read, checked, linked).
    ModsChanged,
    /// The mod operation's dialog was closed: the title bar's figures follow.
    ModOpDismissed,
    /// Every mod of an operation came through: what was waiting on it goes on.
    ModOpSucceeded(connect::Launch),
    /// The server list's maps and total were (re)loaded.
    ServersLoaded,
}

/// Every store.
pub struct Stores {
    pub bus: Bus,
    pub app: app::AppState,
    pub servers: servers::Servers,
    pub server_data: server_data::ServerData,
    pub profile: profile::Profile,
    pub mods: mods::Mods,
    pub connect: connect::Connect,
    /// The Direct Connect form.
    pub direct: direct::Direct,
    pub game: game::Game,
    pub battleye: battleye::BattlEye,
    pub dialogs: dialogs::Dialogs,
    pub friends: friends::Friends,
    pub updater: updater::Updater,
    pub sources: sources::Sources,
    pub pad: pad::Pad,
    pub theme: theme::Theme,
    /// Moves whenever a poll took something in or the hub said something other than
    /// pure progress ([`moves_views`]): what a view derives from the stores and caches
    /// across frames is made again when it moves (with the view's own key for what
    /// actions change at once).
    pub generation: u64,
    signals: Vec<Signal>,
}

impl Stores {
    pub fn new(hub: &Hub, bus: Bus) -> Self {
        Self {
            app: app::AppState::new(hub.clone(), bus.clone()),
            servers: servers::Servers::new(hub.clone(), bus.clone()),
            server_data: server_data::ServerData::new(hub.clone()),
            profile: profile::Profile::new(hub.clone(), bus.clone()),
            mods: mods::Mods::new(hub.clone(), bus.clone()),
            connect: connect::Connect::new(hub.clone(), bus.clone()),
            direct: direct::Direct::new(hub.clone(), bus.clone()),
            game: game::Game::new(hub.clone(), bus.ctx().clone()),
            battleye: battleye::BattlEye::new(hub.clone(), bus.clone()),
            dialogs: dialogs::Dialogs::default(),
            friends: friends::Friends::new(hub.clone(), bus.clone()),
            updater: updater::Updater::new(hub.clone()),
            sources: sources::Sources::new(hub.clone()),
            pad: pad::Pad::new(hub.clone()),
            theme: theme::Theme::default(),
            generation: 0,
            signals: Vec::new(),
            bus,
        }
    }

    /// What the hub said, to every store that listens.
    pub fn on_event(&mut self, ev: &HubEvent) {
        if moves_views(ev) {
            self.generation = self.generation.wrapping_add(1);
        }
        let bus = self.bus.clone();
        match ev {
            HubEvent::LaunchDone(name) => {
                bus.step(
                    "launch",
                    crate::i18n::tf("shell", "statusLaunched", &[("name", name)]),
                    ToastTone::Ok,
                );
                self.game.launched();
                self.profile.load();
            }
            HubEvent::LaunchError(error) => {
                bus.step(
                    "launch",
                    crate::i18n::tf("shell", "statusLaunchError", &[("error", error)]),
                    ToastTone::Err,
                );
            }
            HubEvent::CliArgs(args) => self.connect.cli(args.clone(), self.cli_snapshot()),
            _ => {}
        }
        self.servers.on_event(ev);
        self.game.on_event(ev);
        self.sources.on_event(ev);
        self.mods.on_event(ev, &mut self.signals);
        self.updater.on_event(ev);
        self.pad.on_event(ev);
    }

    /// Close the mod operation's dialog; the disk is read again.
    pub fn mods_dismiss(&mut self) {
        self.mods.dismiss(&mut self.signals);
    }

    /// Cancel the mod operation and close its dialog.
    pub fn mods_cancel(&mut self) {
        self.mods.cancel(&mut self.signals);
    }

    /// What the connect flows need to know of the other stores, at the moment they start.
    pub fn cli_snapshot(&self) -> connect::Snapshot {
        connect::Snapshot::of(self.app.initialized, &self.profile, &self.mods)
    }

    /// Take finished answers in and route what one store tells others. True when
    /// something changed (the frame repaints anyway: tasks wake it).
    pub fn poll(&mut self, view: ViewId) -> bool {
        let mut changed = false;
        changed |= self.app.poll(&mut self.signals);
        if let Some(r) = self.app.take_init() {
            changed = true;
            match r {
                Ok(r) => self.started(&r),
                Err(e) => self.app.init_failed(e),
            }
        }
        changed |= self.servers.poll(&mut self.signals);
        changed |= self.server_data.poll(&mut self.servers);
        changed |= self.profile.poll(&mut self.signals);
        changed |= self.mods.poll(&mut self.signals);
        self.mods
            .follow_saved(self.servers.snapshot(), self.profile.written);
        if self.game.poll() {
            changed = true;
            crate::parts::server_row::set_game_version(
                self.bus.ctx(),
                self.game.version.as_deref(),
            );
            self.servers.set_game_version(self.game.version.as_deref());
        }
        changed |= self.battleye.poll();
        changed |= self.updater.poll();
        changed |= self.sources.poll();
        changed |= self.pad.poll();
        if self.app.initialized && !self.app.show_wizard {
            let focused = self.app.focused;
            let web = self.profile.has_web_api();
            changed |= self.friends.poll(focused, view == ViewId::Friends, web);
        }
        changed |= self.app.tick(&mut self.mods, &mut self.servers);
        while !self.signals.is_empty() {
            changed = true;
            let signals = std::mem::take(&mut self.signals);
            for s in signals {
                self.route(s);
            }
        }
        if changed {
            self.generation = self.generation.wrapping_add(1);
        }
        changed
    }

    /// `initialize` answered: start what every store loads, the list first.
    fn started(&mut self, r: &dz_hub::features::servers::InitResult) {
        self.servers.list_error.clone_from(&r.list_error);
        self.app.init_done(r);
        self.servers.load();
        self.servers.load_stats();
        self.servers.load_steam_players();
        self.sources.start();
        self.updater.resume();
        self.profile.load();
        self.mods.load();
        if r.resumed {
            self.mods.resume();
        }
        self.game.init();
        self.battleye.refresh();
        self.pad.start();
    }

    fn route(&mut self, s: Signal) {
        match s {
            Signal::ProfileLoaded => {
                self.app
                    .profile_loaded(&mut self.mods, &mut self.profile, &mut self.connect);
                let snap = self.cli_snapshot();
                self.connect.flush_cli(snap);
                // A join (`LaunchDone` reads the profile again) recorded its mods.
                if self.mods.loaded {
                    self.mods.load_saved();
                }
            }
            Signal::AccountSaved => {
                self.profile.load();
                self.profile.load_avatar();
                self.servers.load_stats();
            }
            Signal::SteamcmdLoggedIn => self.profile.load(),
            Signal::ModsLoaded => self.app.mods_loaded(&mut self.mods),
            Signal::ModsChanged => self.servers.touch(),
            Signal::ModOpDismissed => {
                // The check reads the disk again first; it needs no key.
                self.mods.check_updates(true);
                self.mods.load_steam();
                self.servers.load_stats();
            }
            Signal::ModOpSucceeded(launch) => self.connect.launch(launch),
            Signal::ServersLoaded => self.app.servers_loaded(&mut self.servers),
        }
    }
}

/// Whether `ev` can change what views derive from the stores and cache by
/// [`Stores::generation`]. Pure progress (a SteamCMD log line, a download's bytes, a
/// stick repeat) is read live by the dialogs and pads that show it, and must not
/// rebuild the saved lists or an open server panel several times a second.
fn moves_views(ev: &HubEvent) -> bool {
    use dz_hub::features::steamcmd::ModProgressKind;
    use dz_hub::features::updater::DownloadEvent;
    match ev {
        HubEvent::GamepadInput(_) => false,
        HubEvent::UpdateDownload(DownloadEvent::Progress { .. }) => false,
        HubEvent::ModProgress(p) => !matches!(
            p.kind,
            ModProgressKind::LogLine | ModProgressKind::LogProgress | ModProgressKind::Bytes
        ),
        _ => true,
    }
}

/// Now, as Unix milliseconds.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
