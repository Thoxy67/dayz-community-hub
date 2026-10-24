//! The window (= `App.svelte`): the frame loop, the event pump, the view host.
//!
//! # The context views and the shell receive
//!
//! ```ignore
//! pub struct Ctx<'a> {
//!     pub egui: &'a egui::Context,
//!     pub hub: &'a dz_hub::Hub,              // every former Tauri command, async
//!     pub stores: &'a mut store::Stores,     // app-wide state (servers, profile, mods…)
//!     pub prefs: &'a mut prefs::Prefs,       // the interface's own preferences
//!     pub nav: &'a mut Nav,                  // the view on screen, a part asked for
//! }
//! cx.run("servers.refresh", &json!({}))      // a command by id (commands.rs)
//! cx.go(ViewId::Mods); cx.go_focus(ViewId::Settings, "apis");
//! cx.join_row(row, None); cx.join_address(ip, port, None); cx.paste(text);
//! ```
//!
//! A view never blocks: it calls a store's action (which spawns the hub call) or
//! `rt::spawn`s its own, and reads the answer on a later frame.
//!
//! # One frame
//!
//! 1. Input the app itself reacts to: the window's focus (`away`/`back`, the pads), the
//!    pointer (leaves pad mode), a paste outside text fields (an address or a `dzch://`
//!    link opens it).
//! 2. Hub events into the stores, then into every opened view.
//! 3. What async flows sent back ([`crate::store::Effect`]): questions, navigation,
//!    closures run with the context.
//! 4. The stores take finished answers in ([`Stores::poll`]).
//! 5. Theme, zoom, reduced motion, the preferences' debounced write, window geometry.
//! 6. The window's own keys ([`commands::handle_keys`]).
//! 7. Draw: title bar, status bar, rail ([`crate::shell`] hooks), the main area (the
//!    start-up screen, else the current view with its rise), the overlays, the toasts.
//!
//! Nothing here repaints on a timer: hub events, finished tasks, debounces and running
//! animations ask for the frames they need, and an idle window draws nothing.

use std::sync::{Arc, Mutex, PoisonError, mpsc};

use dz_hub::features::browser::ServerRow;
use dz_hub::{EventSink, Hub, HubEvent};
use egui::{Context, Event, Frame, UiBuilder};

use crate::commands;
use crate::i18n::{self, Locale, t, tf};
use crate::kit::{self, Button, Spinner, Variant, toast};
use crate::prefs::{Prefs, WindowState};
use crate::shell;
use crate::store::{Bus, Effect, Stores};
use crate::theme::{self, TokensExt, fonts::Face, motion, size};
use crate::views::{self, View, ViewId};

/// The view on screen, and a part of it that was asked for.
#[derive(Debug, Default)]
pub struct Nav {
    pub view: ViewId,
    /// A view asked to open a sub-part: `settings#ping`, `mods#search:<name>`. The view
    /// reads it in `on_show` ([`Nav::take_focus`]).
    pub focus: Option<String>,
    /// Ctrl+F / `/` asked the current view to focus its search field.
    pub focus_search: bool,
    /// When the current view came on screen (egui time), for its rise.
    shown_at: Option<f64>,
    /// The view changed: its `on_show` runs before it draws.
    pending_show: bool,
}

impl Nav {
    pub fn go(&mut self, view: ViewId, focus: Option<String>) {
        if view != self.view {
            self.shown_at = None;
        }
        self.view = view;
        self.focus = focus;
        self.pending_show = true;
    }

    /// The part asked for, once.
    pub fn take_focus(&mut self) -> Option<String> {
        self.focus.take()
    }

    /// Whether the search field should take the focus now (once).
    pub fn take_focus_search(&mut self) -> bool {
        std::mem::take(&mut self.focus_search)
    }
}

/// What views, the shell and commands work with. See the module docs.
pub struct Ctx<'a> {
    pub egui: &'a Context,
    pub hub: &'a Hub,
    pub stores: &'a mut Stores,
    pub prefs: &'a mut Prefs,
    pub nav: &'a mut Nav,
}

impl Ctx<'_> {
    /// Run a command by id ([`commands::run`]).
    pub fn run(&mut self, id: &str, args: &serde_json::Value) -> Result<(), String> {
        commands::run(self, id, args)
    }

    /// Run `id` or `id {json}` ([`commands::run_line`]).
    pub fn run_line(&mut self, line: &str) -> Result<(), String> {
        commands::run_line(self, line)
    }

    pub fn go(&mut self, view: ViewId) {
        self.nav.go(view, None);
        self.egui.request_repaint();
    }

    pub fn go_focus(&mut self, view: ViewId, focus: impl Into<String>) {
        self.nav.go(view, Some(focus.into()));
        self.egui.request_repaint();
    }

    /// Switch the language and keep the choice.
    pub fn set_locale(&mut self, l: Locale) {
        i18n::set_locale(l);
        crate::views::settings::integrations::push_words(self.hub);
        let code = Some(l.code().to_string());
        if self.prefs.get().locale != code {
            self.prefs.edit().locale = code;
        }
        self.egui.request_repaint();
    }

    /// Join a server from a list (password, mods check, launch).
    pub fn join_row(&mut self, row: ServerRow, password: Option<String>) {
        let snap = self.stores.cli_snapshot();
        self.stores.connect.server(row, password, snap);
    }

    /// Join by address: the list's entry if there is one, else straight to it.
    pub fn join_address(&mut self, ip: String, port: u16, password: Option<String>) {
        let snap = self.stores.cli_snapshot();
        self.stores.connect.address(ip, port, password, snap);
    }

    /// The player answered the mods dialog.
    pub fn answer_connect(&mut self, fetch: bool) {
        if let Some((f, launch)) = self.stores.connect.answer(fetch) {
            let via = self.stores.profile.via_steam();
            self.stores.mods.start(f.kind, f.args, Some(launch), via);
        }
    }

    /// Text pasted outside a text field: a `dzch://` link opens it; an address opens
    /// Direct Connect with it (copied from Discord, a website, a friend).
    pub fn paste(&mut self, text: &str) {
        let text = text.trim();
        if text.starts_with("dzch://") {
            let snap = self.stores.cli_snapshot();
            self.stores.connect.open_dzch(text.to_string(), snap);
            return;
        }
        if let Some((host, port)) = crate::store::connect::pasted_address(text) {
            self.stores.connect.open_in_direct(host, port, None, None);
            self.stores
                .bus
                .info(tf("connect", "pastedAddress", &[("address", &text)]));
        }
    }

    /// Pick a `.dzch` file and open it.
    pub fn pick_dzch(&mut self) {
        let bus = self.stores.bus.clone();
        crate::rt::fire(async move {
            let pick = crate::parts::file_picker::Pick::file(t("connect", "openDzchTitle"))
                .filter(t("connect", "dzchFilter"), &["dzch"]);
            let Some(f) = crate::parts::file_picker::pick(pick).await else {
                return;
            };
            let path = f.to_string_lossy().into_owned();
            bus.run(move |cx| {
                let snap = cx.stores.cli_snapshot();
                cx.stores.connect.open_dzch(path, snap);
            });
        });
    }

    /// Close the mod operation's dialog.
    pub fn mods_dismiss(&mut self) {
        self.stores.mods_dismiss();
    }

    /// Cancel the mod operation and close its dialog.
    pub fn mods_cancel(&mut self) {
        self.stores.mods_cancel();
    }
}

/// The pipe the hub's events come down: the sink handed to [`Hub::new`], the receiving
/// end for [`HubApp::new`], and who to wake: the window of the moment once it exists
/// (events before that wait in the channel; the first frame drains them), `main`
/// between windows ([`crate::rt::Waker`]).
pub struct EventPipe {
    pub sink: EventSink,
    pub rx: mpsc::Receiver<HubEvent>,
    pub wake: crate::rt::Waker,
}

impl EventPipe {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<HubEvent>();
        let wake = crate::rt::Waker::default();
        let w = wake.clone();
        let sink: EventSink = Arc::new(move |e| {
            let _ = tx.send(e);
            w.repaint_after(EVENT_WAKE);
        });
        Self { sink, rx, wake }
    }
}

/// What outlives a window closed into the tray (Wayland, where a window cannot hide:
/// `platform::life`): the hub's events, the preferences, the tray and the window's
/// life, the control channel, and the events the next window needs. Handed back by
/// the window as it closes ([`HubApp::hand_back_to`]), to `main`, which waits with it
/// ([`Kept::wait`]) and makes the next window from it ([`HubApp::resume`]).
pub struct Kept {
    pub events: mpsc::Receiver<HubEvent>,
    pub prefs: Prefs,
    pub life: crate::platform::life::Life,
    pub control: Option<crate::control::Control>,
    /// Events that came while there was no window and that the next one needs.
    pub replay: Vec<HubEvent>,
    /// The view the last window showed: the next one opens on it.
    pub view: Option<ViewId>,
}

/// Where a closing window leaves what it kept.
pub type KeptSlot = Arc<Mutex<Option<Kept>>>;

impl Kept {
    pub fn new(events: mpsc::Receiver<HubEvent>, prefs: Prefs) -> Self {
        Self {
            events,
            prefs,
            life: crate::platform::life::Life::new(),
            control: None,
            replay: Vec::new(),
            view: None,
        }
    }

    /// Without a window: wait for what asks for one (the tray, a second launch, the
    /// game stopping, the control channel's `window.show`) or for a quit, keeping up
    /// with the hub meanwhile (the game's state, the notifications). Blocks: `main`'s
    /// thread has no frame to keep. True when a window is wanted.
    pub fn wait(&mut self) -> bool {
        use crate::platform::life::Away;
        let Some(waker) = self.life.session().map(|s| s.waker().clone()) else {
            return false;
        };
        self.life.went_away();
        if let Some(c) = &mut self.control {
            c.forget_window();
        }
        loop {
            let mut next =
                self.life
                    .away_pass(&self.prefs.get().life, &self.events, &mut self.replay);
            if let Some(c) = &mut self.control {
                next = next.and(c.away());
            }
            match next {
                Away::Stay => {}
                Away::Show => return true,
                Away::Quit => return false,
            }
            // Every source wakes this (the hub's events, the tray, a second launch,
            // a control request, a task done); the timeout is only a safety net.
            waker.wait(std::time::Duration::from_secs(5));
        }
    }
}

impl Default for EventPipe {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether motion is off (the preference): views appear without their rise.
pub fn reduced_motion(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::unique("dzch.reduced_motion")))
        .unwrap_or(false)
}

/// How soon a hub event wakes the window. Not zero: egui runs two passes for a repaint
/// asked for at once, and a short delay also folds the events that come in a burst (a
/// refresh sends dozens a second) into one frame.
const EVENT_WAKE: std::time::Duration = std::time::Duration::from_millis(4);
/// Events taken in per frame at most; the rest wait for the next (which is asked for).
const EVENTS_PER_FRAME: usize = 2048;
/// How far a view rises as it comes on screen.
const RISE_PX: f32 = 4.0;

pub struct HubApp {
    hub: Hub,
    stores: Stores,
    prefs: Prefs,
    nav: Nav,
    /// Views made on first visit, then kept.
    views: Vec<(ViewId, Box<dyn View>)>,
    events: mpsc::Receiver<HubEvent>,
    effects: mpsc::Receiver<Effect>,
    started: bool,
    zoom: f32,
    focused: Option<bool>,
    reduced: Option<bool>,
    /// The control channel (`--control`, `ui_shot`).
    control: Option<crate::control::Control>,
    /// Minimised now: eframe's save keeps the last geometry ([`eframe::App::save`]).
    minimized: bool,
    /// The corners last asked of Windows (`platform::window::round_corners`).
    #[cfg_attr(not(windows), allow(dead_code))]
    corners: Option<bool>,
    /// The last geometry eframe saved while the window was not minimised.
    saved_window: Option<String>,
    /// The tray, closing to it, the game's start and stop, notifications.
    life: crate::platform::life::Life,
    /// `logic` took the hub's events in for this pass (the window out of sight): the
    /// pass's `ui`, when it runs, does not take them in again.
    pumped: bool,
    /// Events that came while there was no window, taken in before the channel's.
    replay: Vec<HubEvent>,
    /// Where this window leaves what outlives it when it closes (`main`'s; none in
    /// tests and screenshots).
    hand_back: Option<KeptSlot>,
}

fn cx<'a>(
    egui: &'a Context,
    hub: &'a Hub,
    stores: &'a mut Stores,
    prefs: &'a mut Prefs,
    nav: &'a mut Nav,
) -> Ctx<'a> {
    Ctx {
        egui,
        hub,
        stores,
        prefs,
        nav,
    }
}

impl HubApp {
    /// The window over `hub`, its events arriving on `events`.
    pub fn new(ctx: &Context, hub: Hub, events: mpsc::Receiver<HubEvent>, prefs: Prefs) -> Self {
        ctx.set_fonts(theme::fonts::definitions());
        theme::apply(ctx, theme::Tokens::default());
        crate::parts::pad::install_chips();
        kit::install(ctx);
        views::install();
        // Ctrl+0 is the statistics view's, not egui's zoom reset.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let (bus, effects) = Bus::new(ctx.clone());
        let stores = Stores::new(&hub, bus);
        i18n::set_locale(i18n::initial_locale(prefs.get().locale.as_deref()));
        crate::views::settings::integrations::push_words(&hub);
        Self {
            hub,
            stores,
            prefs,
            nav: Nav {
                pending_show: true,
                ..Nav::default()
            },
            views: Vec::new(),
            events,
            effects,
            started: false,
            zoom: 1.0,
            focused: None,
            reduced: None,
            control: None,
            minimized: false,
            corners: None,
            saved_window: None,
            life: crate::platform::life::Life::new(),
            pumped: false,
            replay: Vec::new(),
            hand_back: None,
        }
    }

    /// A window made from what the previous one kept (or what `main` gathered for
    /// the first); it leaves it in `slot` when it closes.
    pub fn resume(ctx: &Context, hub: Hub, kept: Kept, slot: KeptSlot) -> Self {
        let Kept {
            events,
            prefs,
            mut life,
            control,
            replay,
            view,
        } = kept;
        let mut app = Self::new(ctx, hub, events, prefs);
        // The real window goes by the local date for the seasonal themes (tests and
        // screenshots have none unless they set one).
        app.stores
            .theme
            .set_clock(crate::store::theme::Clock::Local);
        if let Some(v) = view {
            app.nav.go(v, None);
        }
        life.new_window();
        app.life = life;
        app.control = control;
        app.replay = replay;
        app.hand_back = Some(slot);
        app
    }

    /// The window's life (to start the tray in it, once).
    pub fn life_mut(&mut self) -> &mut crate::platform::life::Life {
        &mut self.life
    }

    /// Whether this window is closing into the tray (Wayland), not quitting.
    fn closing_away(&self) -> bool {
        self.life.session().is_some_and(|s| s.closing_away())
    }

    /// Leave what outlives the window in its slot (once: at exit, else when dropped).
    fn hand_back_to(&mut self) {
        let Some(slot) = self.hand_back.take() else {
            return;
        };
        let (_, dead) = mpsc::channel();
        let mut control = self.control.take();
        if let Some(c) = &mut control {
            c.forget_window();
        }
        let kept = Kept {
            events: std::mem::replace(&mut self.events, dead),
            prefs: std::mem::take(&mut self.prefs),
            life: std::mem::take(&mut self.life),
            control,
            replay: std::mem::take(&mut self.replay),
            view: Some(self.nav.view),
        };
        *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(kept);
    }

    /// Take requests from the control channel ([`crate::control`]).
    pub fn set_control(&mut self, control: crate::control::Control) {
        self.control = Some(control);
    }

    /// The tray icon `main` put up.
    pub fn set_tray(&mut self, tray: crate::platform::tray::Tray) {
        self.life.set_tray(tray);
    }

    /// The window geometry eframe restored (its storage's `window`), kept in case the
    /// window is minimised before the first save.
    pub fn set_saved_window(&mut self, saved: Option<String>) {
        self.saved_window = saved;
    }

    /// Before a frame: the control channel's synthetic input joins the window's.
    pub fn control_input(&mut self, raw: &mut egui::RawInput) {
        if let Some(c) = &mut self.control {
            c.input_hook(raw);
        }
    }

    pub fn hub(&self) -> &Hub {
        &self.hub
    }

    pub fn stores(&self) -> &Stores {
        &self.stores
    }

    pub fn stores_mut(&mut self) -> &mut Stores {
        &mut self.stores
    }

    pub fn prefs(&self) -> &Prefs {
        &self.prefs
    }

    pub fn nav(&self) -> &Nav {
        &self.nav
    }

    /// The context, outside a frame (tests, `ui_shot`, the control channel).
    pub fn with_ctx<R>(&mut self, egui: &Context, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let mut c = cx(
            egui,
            &self.hub,
            &mut self.stores,
            &mut self.prefs,
            &mut self.nav,
        );
        f(&mut c)
    }

    /// Write what the preferences have pending (at exit).
    pub fn flush(&mut self) {
        self.prefs.flush();
    }

    fn start(&mut self, ctx: &Context) {
        self.started = true;
        self.stores.app.init();
        self.stores.theme.sync(ctx, &self.prefs);
        // The interface's icons at their usual sizes, rasterised on a worker: the first
        // frames only upload them.
        crate::icons::prewarm(
            ctx,
            crate::icons::lucide::ALL,
            &[12.0, size::ICON_SM, size::ICON, size::ICON_LG],
        );
    }

    /// One frame of the whole window, into `ui` (the root, or `ui_shot`'s harness).
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.start(&ctx);
        }
        self.input(&ctx);
        // Already done by this pass's `logic` while the window is out of sight (a
        // window hidden on X11 or Windows still gets whole passes).
        let pumped = std::mem::take(&mut self.pumped);
        if !pumped {
            self.pump(&ctx);
        }
        if let Some(mut control) = self.control.take() {
            control.frame(self, &ctx);
            self.control = Some(control);
        }
        if !pumped {
            self.stores.poll(self.nav.view);
        }
        self.dress(&ctx);
        self.track_window(&ctx);
        self.prefs.poll(&ctx);
        {
            let mut c = cx(
                &ctx,
                &self.hub,
                &mut self.stores,
                &mut self.prefs,
                &mut self.nav,
            );
            // The pads' actions first: what they send (an Escape, an arrow for the
            // palette) is the window's input like any key.
            crate::pad::frame(&mut c);
            commands::handle_keys(&mut c);
        }
        self.draw(ui, &ctx);
    }

    /// Step 1: the input the app itself reacts to.
    fn input(&mut self, ctx: &Context) {
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(i.focused));
        if self.focused != Some(focused) {
            let first = self.focused.is_none();
            self.focused = Some(focused);
            self.hub.set_focused(focused);
            if focused {
                if !first {
                    let s = &mut self.stores;
                    s.app.back(&mut s.servers, &mut s.mods, &s.profile);
                }
            } else {
                self.stores.app.away();
            }
        }
        // A finger on a touch screen also moves egui's pointer: it is no mouse, and
        // leaves the controller's density and zoom as they are (`Pad::used_touch`).
        let touched = ctx.input(|i| {
            i.any_touches() || i.events.iter().any(|e| matches!(e, Event::Touch { .. }))
        });
        let now = ctx.input(|i| i.time);
        if touched {
            self.stores.pad.used_touch(now);
        } else if ctx.input(|i| i.pointer.is_moving() || i.pointer.any_pressed()) {
            self.stores.pad.used_pointer_at(now);
        }
        if !ctx.egui_wants_keyboard_input() {
            let pasted = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    Event::Paste(s) => Some(s.clone()),
                    _ => None,
                })
            });
            if let Some(text) = pasted {
                let mut c = cx(
                    ctx,
                    &self.hub,
                    &mut self.stores,
                    &mut self.prefs,
                    &mut self.nav,
                );
                c.paste(&text);
            }
        }
    }

    /// Steps 2 and 3: hub events, then what async flows sent back.
    fn pump(&mut self, ctx: &Context) {
        let mut events = std::mem::take(&mut self.replay);
        while events.len() < EVENTS_PER_FRAME {
            match self.events.try_recv() {
                Ok(e) => events.push(e),
                Err(_) => break,
            }
        }
        if events.len() == EVENTS_PER_FRAME {
            ctx.request_repaint();
        }
        for e in &events {
            self.stores.on_event(e);
            self.life.on_event(e);
        }
        if !events.is_empty() && !self.views.is_empty() {
            let mut c = cx(
                ctx,
                &self.hub,
                &mut self.stores,
                &mut self.prefs,
                &mut self.nav,
            );
            for (_, v) in &mut self.views {
                for e in &events {
                    v.on_event(e, &mut c);
                }
            }
        }
        // Effects may send effects: a bounded number of rounds per frame.
        for _ in 0..8 {
            let mut any = false;
            while let Ok(e) = self.effects.try_recv() {
                any = true;
                match e {
                    Effect::Confirm(req, tx) => self.stores.dialogs.confirm(req, tx),
                    Effect::Password(server, tx) => self.stores.dialogs.ask_password(server, tx),
                    Effect::Go(v, focus) => self.nav.go(v, focus),
                    Effect::Run(f) => {
                        let mut c = cx(
                            ctx,
                            &self.hub,
                            &mut self.stores,
                            &mut self.prefs,
                            &mut self.nav,
                        );
                        f(&mut c);
                    }
                }
            }
            if !any {
                break;
            }
        }
    }

    /// Step 5: theme, zoom, reduced motion.
    fn dress(&mut self, ctx: &Context) {
        let themed = self.stores.theme.sync(ctx, &self.prefs);
        let p = self.prefs.get();
        let reduced = p.reduced_motion;
        if themed || self.reduced != Some(reduced) {
            self.reduced = Some(reduced);
            ctx.data_mut(|d| d.insert_temp(egui::Id::unique("dzch.reduced_motion"), reduced));
            ctx.all_styles_mut(|s| {
                s.animation_time = if reduced { 0.0 } else { motion::FAST };
            });
        }
        let pad = self.stores.pad.mode(p.ui.pad_mode) == crate::store::pad::InputMode::Gamepad;
        // Files and folders are picked in the window's own browser with a pad or in
        // Game Mode (the system's dialog is mouse-only, or not shown by gamescope).
        crate::parts::file_picker::set_in_app(pad || self.stores.pad.game_mode);
        // The pad density: taller controls, no text under 13 px; a mouse undoes it.
        if pad {
            crate::parts::pad::set_kind(ctx, self.stores.pad.kind(p.pad_glyphs));
        }
        if size::set_pad_density(pad) {
            ctx.all_styles_mut(|s| {
                s.spacing.interact_size.y = size::control();
                s.text_styles = crate::theme::fonts::text_styles();
            });
            ctx.request_repaint();
        }
        // Touch: a wider reach around each widget.
        let reach = if self.stores.pad.touch {
            size::TOUCH_REACH
        } else {
            size::POINTER_REACH
        };
        if (ctx.global_style().interaction.interact_radius - reach).abs() > f32::EPSILON {
            ctx.all_styles_mut(|s| s.interaction.interact_radius = reach);
        }
        // The density already enlarges what a controller drives by about the default pad
        // scale (115 %, the scale's least, `prefs::PAD_SCALE_DEFAULT`): the zoom is only
        // what the player asked beyond it.
        let zoom = p.scale
            * if pad {
                (p.ui.pad_scale / crate::prefs::PAD_SCALE_DEFAULT).clamp(1.0, 3.0)
            } else {
                1.0
            };
        if zoom.is_finite() && (zoom - self.zoom).abs() > 1e-3 {
            self.zoom = zoom;
            ctx.set_zoom_factor(zoom);
        }
    }

    /// Where the window is and how big, kept for the next start (not while minimised;
    /// a maximised window keeps the size it had before).
    fn track_window(&mut self, ctx: &Context) {
        let (outer, inner, maximized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            (v.outer_rect, v.inner_rect, v.maximized, v.minimized)
        });
        let maximized = maximized.unwrap_or(false);
        self.stores.app.maximized = maximized;
        self.minimized = minimized == Some(true);
        if minimized == Some(true) {
            return;
        }
        let Some(inner) = inner else {
            return;
        };
        let prev = self.prefs.get().window;
        let w = if maximized {
            match prev {
                Some(p) => WindowState {
                    maximized: true,
                    ..p
                },
                None => return,
            }
        } else {
            WindowState {
                pos: outer.map(|r| [r.min.x.round(), r.min.y.round()]),
                size: [inner.width().round(), inner.height().round()],
                maximized: false,
            }
        };
        if w.size.iter().any(|v| !v.is_finite() || *v < 100.0) {
            return;
        }
        if prev != Some(w) {
            // Kept without a write of its own (eframe's file is the one read
            // back first): it goes with the next preferences write or at exit.
            self.prefs.set_window(w);
            // A frame after eframe's save interval, so it writes the new geometry even
            // if the window then sits idle (`auto_save_interval`).
            ctx.request_repaint_after(std::time::Duration::from_millis(2_100));
        }
    }

    /// Step 7: the window's regions.
    fn draw(&mut self, outer: &mut egui::Ui, ctx: &Context) {
        let tk = outer.tokens();
        // The whole window, whatever margin the host's root Ui has (a test harness
        // wraps it in a panel).
        let full = ctx.content_rect();
        let mut root = outer.new_child(UiBuilder::new().id_salt("hubapp").max_rect(full));
        let ui = &mut root;
        let rail_w = if crate::pad::rail_collapsed(ctx, &self.stores, &self.prefs) {
            size::RAIL_COLLAPSED
        } else {
            size::RAIL
        };
        let mut c = cx(
            ctx,
            &self.hub,
            &mut self.stores,
            &mut self.prefs,
            &mut self.nav,
        );
        // The seasonal effects: whether they show and move this frame (the title's
        // candle reads it as the bar is drawn).
        shell::seasons::begin(ctx, &c);
        // Rounded corners while the window stands free on a desktop that shows through
        // it: the title bar holds the top two, the status bar the bottom two.
        let radius = window_radius(ctx, c.prefs.get().ui.rounded_corners);
        crate::kit::paint::set_window_radius(ctx, radius);
        // The window's ground under the panels, out to its last pixel: the window is
        // transparent, and the panels stop at the content rect, rounded to egui's grid,
        // which at a fractional scale leaves a strip of the window's edge bare.
        let window = ctx.viewport_rect().union(full);
        let ground = if radius > 0 {
            window
        } else {
            window.expand(2.0)
        };
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            ground,
            egui::CornerRadius::same(radius),
            tk.bg,
        );
        let (top, bottom) = (
            egui::CornerRadius {
                nw: radius,
                ne: radius,
                ..egui::CornerRadius::ZERO
            },
            egui::CornerRadius {
                sw: radius,
                se: radius,
                ..egui::CornerRadius::ZERO
            },
        );
        egui::Panel::top("shell.titlebar")
            .exact_size(size::TITLEBAR)
            .resizable(false)
            .frame(Frame::new().fill(tk.bg).corner_radius(top))
            .show(ui, |ui| shell::titlebar(ui, &mut c));
        let status = egui::Panel::bottom("shell.statusbar")
            .exact_size(shell::STATUSBAR_HEIGHT)
            .resizable(false)
            .frame(Frame::new().fill(tk.bg).corner_radius(bottom))
            .show(ui, |ui| shell::statusbar(ui, &mut c))
            .response
            .rect;
        let rail = egui::Panel::left("shell.rail")
            .exact_size(rail_w)
            .resizable(false)
            .frame(Frame::new().fill(tk.bg))
            .show(ui, |ui| shell::rail(ui, &mut c))
            .response
            .rect;
        // What the pad's buttons do, under the main area beside the rail: in pad mode
        // only, its height taken from the main area rather than drawn over it.
        let hints = shell::pad_hints_shown(&c).then(|| {
            egui::Panel::bottom("shell.padhints")
                .exact_size(shell::PAD_HINTS_HEIGHT)
                .resizable(false)
                .show_separator_line(false)
                .frame(Frame::new().fill(tk.panel))
                .show(ui, |ui| shell::pad_hints(ui, &mut c))
                .response
                .rect
        });
        let views = &mut self.views;
        egui::CentralPanel::default()
            .frame(Frame::new().fill(tk.panel))
            .show(ui, |ui| main_area(ui, &mut c, views));
        shell::seasons::paint(
            ctx,
            shell::seasons::Regions {
                window: full,
                rail,
                status,
                hints,
            },
        );
        // In pad mode the status bar and the pad hints' bar stay readable over the veil:
        // dialogs keep clear of them.
        let kept = if shell::pad_hints_shown(&c) {
            shell::STATUSBAR_HEIGHT + shell::PAD_HINTS_HEIGHT
        } else {
            0.0
        };
        crate::kit::dialog::set_bottom_inset(ctx, kept);
        shell::overlays(ctx, &mut c);
        // The file browser and the on-screen keyboard, over the dialogs.
        crate::parts::file_picker::show(&mut c);
        crate::parts::osk::show(&c);
        // The toasts, over everything (above the status bar and the pad hints); a
        // toast's button runs its command line.
        let hints_h = if shell::pad_hints_shown(&c) {
            shell::PAD_HINTS_HEIGHT
        } else {
            0.0
        };
        toast::set_inset(ctx, shell::STATUSBAR_HEIGHT + hints_h + 8.0);
        if let Some(line) = toast::host(ctx, t("common", "close")) {
            let _ = c.run_line(&line);
        }
        // The controller's focus ring, over what this frame drew.
        crate::pad::paint(ctx, &mut c);
        // A rounded window's edge, a hairline over everything: without the frame a
        // desktop draws around a window, its rounded corners would melt into a dark
        // background.
        if radius > 0 {
            ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::unique("dzch.window.edge"),
            ))
            .rect_stroke(
                window,
                egui::CornerRadius::same(radius),
                egui::Stroke::new(1.0, tk.border_strong),
                egui::StrokeKind::Inside,
            );
        }
    }
}

/// The window's corner radius this frame: rounded only on a window that shows the
/// desktop through it (`platform::window::see_through`), turned on, neither maximised
/// nor full screen.
fn window_radius(ctx: &Context, on: bool) -> u8 {
    // A window as large as its screen is maximised whatever the system says (a
    // compositor may not report it).
    let free = ctx.input(|i| {
        let v = i.viewport();
        let screen_sized = match (v.inner_rect, v.monitor_size) {
            (Some(r), Some(m)) => r.width() >= m.x - 2.0 && r.height() >= m.y - 64.0,
            _ => false,
        };
        v.maximized != Some(true) && v.fullscreen != Some(true) && !screen_sized
    });
    if on && free && crate::platform::window::see_through() {
        size::RADIUS_LG
    } else {
        0
    }
}

/// The main area: the start-up screen, else the current view (made on first visit,
/// then kept), rising into place.
fn main_area(ui: &mut egui::Ui, c: &mut Ctx, views: &mut Vec<(ViewId, Box<dyn View>)>) {
    if !c.stores.app.initialized {
        init_screen(ui, c);
        return;
    }
    // Under the setup wizard the view is hidden: not drawn at all.
    if shell::wizard_covers(c) {
        return;
    }
    let id = c.nav.view;
    let i = match views.iter().position(|(v, _)| *v == id) {
        Some(i) => i,
        None => {
            views.push((id, views::create(id)));
            views.len() - 1
        }
    };
    let Some((_, view)) = views.get_mut(i) else {
        return;
    };
    if c.nav.pending_show {
        c.nav.pending_show = false;
        view.on_show(c);
    }
    let now = ui.input(|inp| inp.time);
    let shown_at = *c.nav.shown_at.get_or_insert(now);
    let p = if reduced_motion(ui.ctx()) {
        1.0
    } else {
        ((now - shown_at) as f32 / motion::BASE).clamp(0.0, 1.0)
    };
    let eased = 1.0 - (1.0 - p).powi(3);
    if p < 1.0 {
        ui.ctx().request_repaint();
    }
    let rect = ui
        .max_rect()
        .translate(egui::vec2(0.0, (1.0 - eased) * RISE_PX));
    let mut child = ui.new_child(UiBuilder::new().id_salt(("view", id.id())).max_rect(rect));
    child.set_clip_rect(ui.max_rect());
    if eased < 1.0 {
        child.set_opacity(eased);
    }
    view.ui(&mut child, c);
}

/// Before start-up finished: the topographic ground, a spinner and "Initializing…",
/// or the error with Retry.
fn init_screen(ui: &mut egui::Ui, c: &mut Ctx) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    kit::topo::paint(ui.painter(), rect, &tk, 0.6);
    let error = c.stores.app.init_error.clone();
    let mut retry = false;
    ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
        ui.with_layout(
            egui::Layout::centered_and_justified(egui::Direction::TopDown),
            |ui| {
                ui.vertical_centered(|ui| {
                    let block_h = if error.is_some() { 150.0 } else { 70.0 };
                    ui.add_space(((rect.height() - block_h) / 2.0).max(0.0));
                    ui.spacing_mut().item_spacing.y = 12.0;
                    match &error {
                        Some(e) => {
                            crate::icons::lucide::TRIANGLE_ALERT.show(ui, 32.0, tk.err);
                            ui.label(
                                egui::RichText::new(t("shell", "initFailed"))
                                    .font(theme::font(Face::Display, size::TEXT_XL))
                                    .color(tk.fg),
                            );
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(e)
                                        .font(theme::font(Face::Sans, size::TEXT_2XS))
                                        .color(tk.fg_muted),
                                )
                                .selectable(true),
                            );
                            if ui
                                .add(Button::new(t("shell", "initRetry")).variant(Variant::Accent))
                                .clicked()
                            {
                                retry = true;
                            }
                        }
                        None => {
                            ui.add(Spinner::new().size(28.0).color(tk.accent));
                            ui.label(
                                egui::RichText::new(t("shell", "initLoading").to_uppercase())
                                    .font(theme::font(Face::SansSemibold, size::TEXT_2XS))
                                    .color(tk.fg_muted),
                            );
                        }
                    }
                });
            },
        );
    });
    if retry {
        c.stores.app.init();
    }
}

impl eframe::App for HubApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        HubApp::ui(self, ui);
    }

    /// Before each frame, and alone while the window is hidden or minimised (no egui
    /// pass then): the hub's events are taken in here on those passes, so the window's
    /// life ([`crate::platform::life`]) follows the game and the tray out of sight.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.life.platform(ctx, frame);
        #[cfg(windows)]
        {
            let on = self.prefs.get().ui.rounded_corners;
            if self.corners != Some(on) {
                self.corners = Some(on);
                crate::platform::window::round_corners(frame, on);
            }
        }
        // `logic` always comes before the pass's `ui`: `ui` knows from this flag that
        // the events were taken in.
        self.pumped = self.started && self.life.out_of_sight(ctx);
        if self.pumped {
            self.pump(ctx);
            self.stores.poll(self.nav.view);
        }
        let mut c = cx(
            ctx,
            &self.hub,
            &mut self.stores,
            &mut self.prefs,
            &mut self.nav,
        );
        self.life.frame(&mut c);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        shell::release_after_hand_over(ctx, raw_input);
        self.control_input(raw_input);
        // Keys acting as the pad leave before egui reads them.
        self.with_ctx(ctx, |c| crate::pad::take_keys(c, raw_input));
    }

    fn on_exit(&mut self) {
        self.flush();
        if self.closing_away() {
            // Into the tray (Wayland): the launcher goes on; the pads speak to no
            // window meanwhile.
            self.hub.set_focused(false);
        } else {
            // The Discord activity is cleared (a short wait, only when one is shown).
            self.hub.discord_quit(std::time::Duration::from_millis(300));
        }
        self.hand_back_to();
    }

    /// The window's geometry is the only thing eframe keeps (`platform::window`); egui's
    /// own memory is not (the preferences hold the panes).
    /// Nothing behind the panels: a rounded corner shows the desktop (on an opaque
    /// window the panels cover everything anyway).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    /// eframe writes its file (off this thread) only when the geometry changed, so a
    /// short interval saves a move or resize soon after it, at no cost when idle.
    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(2)
    }

    /// eframe has just put the window's geometry in `storage`: a minimised window's
    /// (parked off screen, zero size on Windows) is replaced by the last one before.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        const KEY: &str = "window";
        if self.minimized {
            if let Some(w) = &self.saved_window {
                storage.set_string(KEY, w.clone());
            } else {
                storage.remove_string(KEY);
            }
        } else {
            self.saved_window = storage.get_string(KEY);
        }
    }
}

/// A window that ends without `on_exit` (an error) still hands back what it kept.
impl Drop for HubApp {
    fn drop(&mut self) {
        if self.hand_back.is_some() {
            self.flush();
            self.hand_back_to();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::time::{Duration, Instant};

    use egui_kittest::Harness;

    use super::*;
    use dz_hub::{CliArgs, HubConfig, HubMode};

    /// The window on the mock hub, in a headless harness. The app is made on the first
    /// frame (its fonts take effect from the next), as `main` makes it before the first.
    fn mock_app(rt: &tokio::runtime::Runtime) -> Harness<'static, Option<HubApp>> {
        let pipe = EventPipe::new();
        let config = HubConfig {
            mode: HubMode::Mock,
            data_dir: None,
            args: CliArgs::none(),
        };
        let hub = Hub::new(config, pipe.sink.clone()).expect("mock hub");
        {
            let _g = rt.enter();
            hub.start();
        }
        let handle = rt.handle().clone();
        let EventPipe { rx, wake, .. } = pipe;
        let mut parts = Some((hub, rx, handle, wake));
        Harness::builder()
            .with_size(egui::vec2(1400.0, 860.0))
            .build_ui_state(
                move |ui, app: &mut Option<HubApp>| match app {
                    Some(a) => a.ui(ui),
                    None => {
                        if let Some((hub, rx, handle, wake)) = parts.take() {
                            crate::rt::Rt::install(handle, ui.ctx().clone());
                            wake.set(ui.ctx());
                            *app = Some(HubApp::new(ui.ctx(), hub, rx, Prefs::default()));
                        }
                    }
                },
                None,
            )
    }

    /// Step the harness until `done` holds (real time passes: the hub works on the
    /// runtime), or fail after `secs`.
    fn until(
        h: &mut Harness<'static, Option<HubApp>>,
        secs: u64,
        what: &str,
        done: impl Fn(&HubApp) -> bool,
    ) {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            h.step();
            if h.state().as_ref().is_some_and(&done) {
                return;
            }
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// One runtime for the process: `rt::Rt` is installed once and must stay alive.
    fn runtime() -> &'static tokio::runtime::Runtime {
        static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
        })
    }

    /// One test: the hub keeps process-wide state (the refresh, the sources, the mock's
    /// data directory), so two mock windows at once would share it.
    #[test]
    fn mock_window_starts_fills_the_stores_and_runs_commands() {
        let mut h = mock_app(runtime());
        until(&mut h, 60, "start-up", |a| a.stores().app.initialized);
        until(&mut h, 60, "the server list", |a| {
            a.stores().servers.total > 0 && !a.stores().servers.maps.is_empty()
        });
        until(&mut h, 60, "the profile", |a| {
            a.stores().profile.data.is_some()
        });
        until(&mut h, 60, "the mods", |a| a.stores().mods.loaded);
        let app = h.state().as_ref().unwrap();
        assert!(app.stores().app.init_error.is_none());
        // The deduplicated list, as the Svelte store counts it (not the sources' 19 107).
        assert_eq!(app.stores().servers.total, 9500);
        let order = app.stores().servers.order().expect("the browser's order");
        assert_eq!(order.total(), 9499, "one server's IP is excluded");
        // A server other views mention, found by address in the snapshot.
        let fav = app
            .stores()
            .profile
            .favorites
            .first()
            .cloned()
            .expect("the mock profile has favourites");
        let (ip, port) = (fav.ip.clone(), i64::from(fav.port));
        until(&mut h, 60, "a favourite's row", |a| {
            a.stores().servers.row_of(&ip, port).is_some()
        });

        let ctx = h.ctx.clone();
        let app = h.state_mut().as_mut().unwrap();
        app.with_ctx(&ctx, |cx| {
            cx.run("view.mods", &serde_json::Value::Null).unwrap();
            assert_eq!(cx.nav.view, ViewId::Mods);
            cx.run("view.next", &serde_json::Value::Null).unwrap();
            assert_eq!(cx.nav.view, ViewId::Options);
            cx.run_line(r#"view.settings {"focus":"apis"}"#).unwrap();
            assert_eq!(cx.nav.focus.as_deref(), Some("apis"));
            cx.run("rail.toggle_collapse", &serde_json::Value::Null)
                .unwrap();
            assert!(crate::pad::rail_collapsed(cx.egui, cx.stores, cx.prefs));
            cx.run("palette.toggle", &serde_json::Value::Null).unwrap();
            assert!(cx.stores.dialogs.palette);
            cx.run("sheet.toggle", &serde_json::Value::Null).unwrap();
            assert!(!cx.stores.dialogs.palette && cx.stores.dialogs.shortcuts);
            cx.stores.dialogs.shortcuts = false;
            cx.run_line(r#"i18n.set_locale {"locale":"fr"}"#).unwrap();
            assert_eq!(i18n::locale(), Locale::Fr);
            cx.run_line(r#"i18n.set_locale {"locale":"en"}"#).unwrap();
            assert!(cx.run("no.such.command", &serde_json::Value::Null).is_err());
            cx.paste("1.2.3.4:2302");
            assert_eq!(
                cx.stores.connect.prefill.as_ref().map(|p| p.port),
                Some(2302)
            );
        });
        // The paste's navigation lands on the next frame.
        h.run_steps(3);
        assert_eq!(h.state().as_ref().unwrap().nav().view, ViewId::Connect);

        // The window's edges and corners show the resize cursor, wherever they are.
        for (pos, want) in [
            (egui::pos2(1.0, 400.0), egui::CursorIcon::ResizeWest),
            (egui::pos2(1398.5, 400.0), egui::CursorIcon::ResizeEast),
            (egui::pos2(700.0, 1.0), egui::CursorIcon::ResizeNorth),
            (egui::pos2(700.0, 858.5), egui::CursorIcon::ResizeSouth),
            (egui::pos2(300.0, 858.5), egui::CursorIcon::ResizeSouth),
            (egui::pos2(1399.0, 859.0), egui::CursorIcon::ResizeSouthEast),
            (egui::pos2(1.0, 1.0), egui::CursorIcon::ResizeNorthWest),
        ] {
            h.hover_at(pos);
            h.run_steps(2);
            assert_eq!(h.output().platform_output.cursor_icon, want, "at {pos:?}");
        }
    }
}
