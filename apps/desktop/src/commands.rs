//! Everything a user can trigger, by id (= `lib/shortcuts/registry.ts`,
//! `lib/shortcuts/actions.ts` and `shell/keys.ts`).
//!
//! - [`SHORTCUTS`]: every shortcut the window answers to, keyboard and controller, in
//!   one list, with the Svelte registry's ids, scopes, keys, pad buttons and words. The
//!   window's `global` and `views` ones carry command ids (`palette.toggle`,
//!   `view.servers`…); the scoped ones (`list.*`, `mods.*`, `news.*`, `palette.*`,
//!   `dialog.*`, `pad.*`) are the views' and the shell's to handle with [`scoped`].
//! - [`all`]: the commands ([`CommandSpec`]). The keyboard, the command palette, the
//!   shortcuts sheet, the pads, a toast's button and the control channel all run them
//!   with [`run`] (or [`run_line`]: `id {json args}`); buttons call the same id.
//! - [`handle_keys`]: each frame, the window's own keys, with the rules of `keys.ts`: a
//!   key without Ctrl (or an F-key) never fires while a text field has the focus; Esc is
//!   the top-most thing's (dialogs and popups take it); behind an open dialog nothing
//!   but Ctrl+K answers.
//!
//! A view adds its commands by appending to [`COMMANDS`] (keep ids unique: a test checks).

use egui::{Context, Key, KeyboardShortcut, Modifiers};
use serde_json::Value;

use dz_hub::features::gamepad::PadAction;

use crate::app::Ctx;
use crate::i18n::{Locale, t};
use crate::prefs::{GlyphChoice, PadMode};
use crate::store::ConfirmRequest;
use crate::theme::Scheme;
use crate::views::ViewId;

// ── shortcuts ───────────────────────────────────────────────────────────────

/// A controller button, by what Xbox prints on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    A,
    B,
    X,
    Y,
    Lb,
    Rb,
    Lt,
    Rt,
    Menu,
    View,
    L3,
    R3,
    Dpad,
    Rs,
}

/// Where a shortcut works (= the registry's `Scope`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Views,
    List,
    Saved,
    Mods,
    News,
    Palette,
    Dialog,
    Pad,
    Servers,
    Connect,
    Offline,
    Options,
}

impl Scope {
    /// The heading's words key in the `keys` dictionary.
    pub fn label(self) -> &'static str {
        match self {
            Scope::Global => "groupGlobal",
            Scope::Views => "groupViews",
            Scope::List | Scope::Servers => "groupList",
            Scope::Saved => "groupSaved",
            Scope::Mods => "groupMods",
            Scope::News => "groupNews",
            Scope::Palette => "groupPalette",
            Scope::Dialog => "groupDialog",
            Scope::Pad => "groupPad",
            Scope::Connect | Scope::Offline | Scope::Options => "groupViews",
        }
    }
}

/// One key combination. Letters and punctuation ignore Shift unless Ctrl is held (`?`
/// is Shift+/ on one layout and its own key on another); digits also match by their
/// physical key (AZERTY types `&` for Ctrl+1). Ctrl and ⌘ are the same.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Combo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

impl Combo {
    pub const fn key(key: Key) -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn ctrl(key: Key) -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: false,
            key,
        }
    }

    pub const fn shift(key: Key) -> Self {
        Self {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        }
    }

    /// As egui writes shortcuts (for menus).
    pub fn shortcut(self) -> KeyboardShortcut {
        let mut m = Modifiers::NONE;
        if self.ctrl {
            m |= Modifiers::COMMAND;
        }
        if self.shift {
            m |= Modifiers::SHIFT;
        }
        if self.alt {
            m |= Modifiers::ALT;
        }
        KeyboardShortcut::new(m, self.key)
    }

    fn printable(self) -> bool {
        self.key != Key::Space && key_name(self.key).chars().count() == 1
    }

    /// The chips the lists draw: "Ctrl", "K".
    pub fn chips(self) -> Vec<&'static str> {
        let mut v = Vec::with_capacity(3);
        if self.ctrl {
            v.push("Ctrl");
        }
        if self.alt {
            v.push("Alt");
        }
        if self.shift {
            v.push("Shift");
        }
        v.push(key_name(self.key));
        v
    }

    /// Written for a tooltip: "Ctrl+K".
    pub fn text(self) -> String {
        self.chips().join("+")
    }

    /// For a `Kbd` chip: "Ctrl K".
    pub fn kbd(self) -> String {
        self.chips().join(" ")
    }

    /// Whether a key press is this combination.
    pub fn matches(self, key: Key, physical: Option<Key>, m: Modifiers) -> bool {
        if (m.ctrl || m.command || m.mac_cmd) != self.ctrl || m.alt != self.alt {
            return false;
        }
        // The zoom keys: `+` is a shifted key on many layouts, so Shift does not count.
        let zoom_key = matches!(self.key, Key::Plus | Key::Equals | Key::Minus);
        if !zoom_key && (self.shift || !self.printable() || self.ctrl) && m.shift != self.shift {
            return false;
        }
        if key == self.key {
            return true;
        }
        is_digit(self.key) && physical == Some(self.key)
    }
}

fn is_digit(k: Key) -> bool {
    matches!(
        k,
        Key::Num0
            | Key::Num1
            | Key::Num2
            | Key::Num3
            | Key::Num4
            | Key::Num5
            | Key::Num6
            | Key::Num7
            | Key::Num8
            | Key::Num9
    )
}

/// A key as the lists write it: "Esc", "PgUp", "↑", "K", ",".
pub fn key_name(k: Key) -> &'static str {
    match k {
        Key::Escape => "Esc",
        Key::Delete => "Del",
        Key::Space => "Space",
        Key::PageUp => "PgUp",
        Key::PageDown => "PgDn",
        Key::ArrowUp => "↑",
        Key::ArrowDown => "↓",
        Key::ArrowLeft => "←",
        Key::ArrowRight => "→",
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::Home => "Home",
        Key::End => "End",
        Key::Questionmark => "?",
        Key::Slash => "/",
        Key::Comma => ",",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::Minus => "−",
        other => other.symbol_or_name(),
    }
}

/// One shortcut (= the registry's `Shortcut`).
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    /// For `global` and `views`, the command it runs.
    pub id: &'static str,
    pub scope: Scope,
    /// Each a combination; several are alternatives.
    pub keys: &'static [Combo],
    /// Shown as one row "↑ / ↓" rather than two: the keys of one gesture.
    pub pair: bool,
    pub pad: &'static [PadButton],
    /// The view a `views` entry goes to; its label is the view's own name.
    pub view: Option<ViewId>,
    /// The words, in the `keys` dictionary (not for a `views` entry).
    pub label: Option<&'static str>,
    /// Shorter words for the controller's hint bar, in the `pad` dictionary.
    pub hint: Option<&'static str>,
}

impl Shortcut {
    /// Its words in the current language.
    pub fn words(&self) -> &'static str {
        match (self.view, self.label) {
            (Some(v), _) => v.label(),
            (None, Some(l)) => t("keys", l),
            (None, None) => self.id,
        }
    }

    /// The first combination, for a tooltip: "Ctrl+K".
    pub fn key_text(&self) -> Option<String> {
        self.keys.first().map(|c| c.text())
    }
}

const fn sc(
    id: &'static str,
    scope: Scope,
    keys: &'static [Combo],
    label: &'static str,
) -> Shortcut {
    Shortcut {
        id,
        scope,
        keys,
        pair: false,
        pad: &[],
        view: None,
        label: Some(label),
        hint: None,
    }
}

const fn pad(mut s: Shortcut, pad: &'static [PadButton], hint: Option<&'static str>) -> Shortcut {
    s.pad = pad;
    s.hint = hint;
    s
}

const fn pair(mut s: Shortcut) -> Shortcut {
    s.pair = true;
    s
}

const fn view(id: &'static str, v: ViewId, keys: &'static [Combo]) -> Shortcut {
    Shortcut {
        id,
        scope: Scope::Views,
        keys,
        pair: false,
        pad: &[],
        view: Some(v),
        label: None,
        hint: None,
    }
}

use PadButton as P;
use Scope as S;

/// Every shortcut, in the order the sheet and the About page list them.
pub const SHORTCUTS: &[Shortcut] = &[
    // ── everywhere ──
    pad(
        sc(
            "palette.toggle",
            S::Global,
            &[Combo::ctrl(Key::K)],
            "palette",
        ),
        &[P::Menu],
        Some("search"),
    ),
    sc(
        "view.focus_search",
        S::Global,
        &[Combo::ctrl(Key::F), Combo::key(Key::Slash)],
        "search",
    ),
    pad(
        sc(
            "sheet.toggle",
            S::Global,
            &[Combo::key(Key::Questionmark)],
            "sheet",
        ),
        &[P::View],
        Some("shortcuts"),
    ),
    sc(
        "servers.refresh",
        S::Global,
        &[Combo::ctrl(Key::R)],
        "refresh",
    ),
    sc(
        "mods.update_behind",
        S::Global,
        &[Combo::ctrl(Key::U)],
        "updateMods",
    ),
    pad(
        sc("rejoin", S::Global, &[Combo::ctrl(Key::L)], "rejoin"),
        &[P::L3],
        Some("rejoin"),
    ),
    // The interface's size, as browsers do (Ctrl+0 is the statistics view).
    sc(
        "ui.zoom_in",
        S::Global,
        &[Combo::ctrl(Key::Plus), Combo::ctrl(Key::Equals)],
        "zoomIn",
    ),
    sc(
        "ui.zoom_out",
        S::Global,
        &[Combo::ctrl(Key::Minus)],
        "zoomOut",
    ),
    pad(
        sc("back", S::Global, &[Combo::key(Key::Escape)], "back"),
        &[P::B],
        Some("back"),
    ),
    // ── views ──
    view("view.servers", ViewId::Servers, &[Combo::ctrl(Key::Num1)]),
    view(
        "view.favorites",
        ViewId::Favorites,
        &[Combo::ctrl(Key::Num2)],
    ),
    view("view.history", ViewId::History, &[Combo::ctrl(Key::Num3)]),
    view("view.friends", ViewId::Friends, &[Combo::ctrl(Key::Num4)]),
    view("view.connect", ViewId::Connect, &[Combo::ctrl(Key::Num5)]),
    view("view.offline", ViewId::Offline, &[Combo::ctrl(Key::Num6)]),
    view("view.mods", ViewId::Mods, &[Combo::ctrl(Key::Num7)]),
    view("view.options", ViewId::Options, &[Combo::ctrl(Key::Num8)]),
    view("view.news", ViewId::News, &[Combo::ctrl(Key::Num9)]),
    view("view.stats", ViewId::Stats, &[Combo::ctrl(Key::Num0)]),
    // Ctrl+, is where desktop apps keep their settings; F1 is help, the guide is in About.
    view(
        "view.settings",
        ViewId::Settings,
        &[Combo::ctrl(Key::Comma)],
    ),
    view("view.about", ViewId::About, &[Combo::key(Key::F1)]),
    pad(
        sc(
            "view.prev",
            S::Views,
            &[Combo::ctrl(Key::PageUp)],
            "viewPrev",
        ),
        &[P::Lb],
        Some("views"),
    ),
    pad(
        sc(
            "view.next",
            S::Views,
            &[Combo::ctrl(Key::PageDown)],
            "viewNext",
        ),
        &[P::Rb],
        None,
    ),
    // ── the server lists ──
    pad(
        pair(sc(
            "list.move",
            S::List,
            &[Combo::key(Key::ArrowUp), Combo::key(Key::ArrowDown)],
            "move",
        )),
        &[P::Dpad],
        None,
    ),
    pair(sc(
        "list.edges",
        S::List,
        &[Combo::key(Key::Home), Combo::key(Key::End)],
        "edges",
    )),
    pad(
        pair(sc(
            "list.page",
            S::List,
            &[Combo::key(Key::PageUp), Combo::key(Key::PageDown)],
            "page",
        )),
        &[P::Lt, P::Rt],
        None,
    ),
    pad(
        sc("list.join", S::List, &[Combo::key(Key::Enter)], "join"),
        &[P::A],
        Some("join"),
    ),
    pad(
        sc("list.fav", S::List, &[Combo::key(Key::F)], "favorite"),
        &[P::X],
        Some("favorite"),
    ),
    pad(
        sc("list.ping", S::List, &[Combo::key(Key::P)], "ping"),
        &[P::Y],
        Some("ping"),
    ),
    pad(
        sc("list.detail", S::List, &[Combo::key(Key::I)], "detail"),
        &[P::R3],
        Some("details"),
    ),
    sc("list.copy", S::List, &[Combo::key(Key::C)], "copyAddress"),
    sc("list.link", S::List, &[Combo::key(Key::L)], "copyLink"),
    sc("list.direct", S::List, &[Combo::key(Key::D)], "direct"),
    pad(
        sc(
            "list.close",
            S::List,
            &[Combo::key(Key::Escape)],
            "deselect",
        ),
        &[P::B],
        Some("back"),
    ),
    sc(
        "saved.remove",
        S::Saved,
        &[Combo::key(Key::Delete)],
        "remove",
    ),
    // ── mods ──
    pad(
        pair(sc(
            "mods.move",
            S::Mods,
            &[Combo::key(Key::ArrowUp), Combo::key(Key::ArrowDown)],
            "move",
        )),
        &[P::Dpad],
        None,
    ),
    pair(sc(
        "mods.edges",
        S::Mods,
        &[Combo::key(Key::Home), Combo::key(Key::End)],
        "edges",
    )),
    pad(
        sc("mods.tick", S::Mods, &[Combo::key(Key::Space)], "tick"),
        &[P::X],
        Some("tickMod"),
    ),
    pad(
        sc("mods.update", S::Mods, &[Combo::key(Key::U)], "updateMod"),
        &[P::Y],
        Some("update"),
    ),
    sc("mods.managed", S::Mods, &[Combo::key(Key::M)], "managed"),
    sc(
        "mods.delete",
        S::Mods,
        &[Combo::key(Key::Delete)],
        "deleteMod",
    ),
    pad(
        sc(
            "mods.close",
            S::Mods,
            &[Combo::key(Key::Escape)],
            "deselect",
        ),
        &[P::B],
        Some("back"),
    ),
    // ── news ──
    pad(
        pair(sc(
            "news.move",
            S::News,
            &[
                Combo::key(Key::ArrowUp),
                Combo::key(Key::ArrowDown),
                Combo::key(Key::J),
                Combo::key(Key::K),
            ],
            "article",
        )),
        &[P::Dpad],
        None,
    ),
    pad(
        sc("news.open", S::News, &[Combo::key(Key::O)], "openArticle"),
        &[P::X],
        Some("openArticle"),
    ),
    // ── a view's own controller buttons ──
    pad(
        sc("servers.refresh_pad", S::Servers, &[], "refresh"),
        &[P::Y],
        Some("refresh"),
    ),
    pad(
        sc("connect.query", S::Connect, &[], "query"),
        &[P::X],
        Some("query"),
    ),
    pad(
        sc(
            "connect.join",
            S::Connect,
            &[Combo::key(Key::Enter)],
            "join",
        ),
        &[P::Y],
        Some("join"),
    ),
    pad(
        sc("offline.launch", S::Offline, &[], "launch"),
        &[P::X],
        Some("launch"),
    ),
    pad(
        sc("offline.install", S::Offline, &[], "installMode"),
        &[P::Y],
        Some("installMode"),
    ),
    pad(
        sc("options.apply", S::Options, &[], "applyRecommended"),
        &[P::X],
        Some("applyRecommended"),
    ),
    pad(
        sc("options.active", S::Options, &[], "activeOnly"),
        &[P::Y],
        Some("activeOnly"),
    ),
    // ── the search palette ──
    pad(
        pair(sc(
            "palette.move",
            S::Palette,
            &[Combo::key(Key::ArrowUp), Combo::key(Key::ArrowDown)],
            "move",
        )),
        &[P::Dpad],
        None,
    ),
    pad(
        sc("palette.run", S::Palette, &[Combo::key(Key::Enter)], "run"),
        &[P::A],
        Some("select"),
    ),
    pad(
        pair(sc(
            "palette.section",
            S::Palette,
            &[Combo::key(Key::Tab), Combo::shift(Key::Tab)],
            "section",
        )),
        &[P::Lb, P::Rb],
        Some("sections"),
    ),
    pad(
        sc("palette.keyboard", S::Palette, &[], "keyboard"),
        &[P::Y],
        Some("keyboard"),
    ),
    pad(
        sc(
            "palette.close",
            S::Palette,
            &[Combo::key(Key::Escape)],
            "close",
        ),
        &[P::B, P::Menu],
        Some("back"),
    ),
    // ── dialogs ──
    pad(
        sc("dialog.confirm", S::Dialog, &[], "confirm"),
        &[P::X],
        Some("confirm"),
    ),
    pad(
        sc(
            "dialog.close",
            S::Dialog,
            &[Combo::key(Key::Escape)],
            "close",
        ),
        &[P::B],
        Some("back"),
    ),
    // ── the controller's own moves ──
    pad(
        sc("pad.select", S::Pad, &[], "press"),
        &[P::A],
        Some("select"),
    ),
    pad(sc("pad.scroll", S::Pad, &[], "scroll"), &[P::Rs], None),
];

/// The order the sheet and the About page list the keyboard groups in.
pub const KEY_GROUPS: &[Scope] = &[
    S::Global,
    S::Views,
    S::List,
    S::Saved,
    S::Mods,
    S::News,
    S::Palette,
    S::Dialog,
];

/// The order of the controller groups.
pub const PAD_GROUPS: &[Scope] = &[
    S::Pad,
    S::Global,
    S::Views,
    S::List,
    S::Mods,
    S::News,
    S::Servers,
    S::Connect,
    S::Offline,
    S::Options,
    S::Palette,
    S::Dialog,
];

/// The shortcut with this id.
pub fn shortcut(id: &str) -> Option<&'static Shortcut> {
    SHORTCUTS.iter().find(|s| s.id == id)
}

/// The shortcuts of a scope, in order.
pub fn in_scope(scope: Scope) -> impl Iterator<Item = &'static Shortcut> {
    SHORTCUTS.iter().filter(move |s| s.scope == scope)
}

/// The key that goes to a view ("Ctrl+1"), for the rail's tooltips and the palette.
pub fn view_key(v: ViewId) -> Option<String> {
    SHORTCUTS
        .iter()
        .find(|s| s.view == Some(v))
        .and_then(Shortcut::key_text)
}

/// A pressed key that belongs to `scope`: consumed and its shortcut id returned. Keys
/// without Ctrl are left to a focused text field. For the views' own list keys.
pub fn scoped(ctx: &Context, scope: Scope) -> Option<&'static str> {
    let typing = ctx.egui_wants_keyboard_input();
    let found = ctx.input(|i| {
        i.events.iter().enumerate().find_map(|(n, e)| {
            let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = e
            else {
                return None;
            };
            in_scope(scope).find_map(|s| {
                let c = s
                    .keys
                    .iter()
                    .find(|c| c.matches(*key, *physical_key, *modifiers))?;
                (!typing || c.ctrl || is_fkey(c.key)).then_some((n, s.id))
            })
        })
    });
    let (n, id) = found?;
    ctx.input_mut(|i| {
        if n < i.events.len() {
            i.events.remove(n);
        }
    });
    Some(id)
}

fn is_fkey(k: Key) -> bool {
    key_name(k).starts_with('F') && key_name(k).len() > 1 && k != Key::F
}

// ── commands ────────────────────────────────────────────────────────────────

/// One command.
pub struct CommandSpec {
    pub id: &'static str,
    /// Its words: (dictionary, key).
    pub label: (&'static str, &'static str),
    /// Where it belongs, for the palette's sections: a [`Scope`]-like name.
    pub group: &'static str,
    pub enabled: fn(&Ctx) -> bool,
    /// Runs it; arguments are a JSON object (hostile input: read with care).
    pub run: fn(&mut Ctx, &Value),
}

impl CommandSpec {
    /// Its words in the current language.
    pub fn words(&self) -> &'static str {
        t(self.label.0, self.label.1)
    }

    /// Its shortcut, if it has one.
    pub fn shortcut(&self) -> Option<&'static Shortcut> {
        shortcut(self.id)
    }

    /// Its keys as egui shortcuts (first = the one shown).
    pub fn shortcuts(&self) -> Vec<KeyboardShortcut> {
        self.shortcut()
            .map(|s| s.keys.iter().map(|c| c.shortcut()).collect())
            .unwrap_or_default()
    }
}

impl std::fmt::Debug for CommandSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandSpec").field("id", &self.id).finish()
    }
}

fn yes(_: &Ctx) -> bool {
    true
}

// argument readers
fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn arg_u64(a: &Value, k: &str) -> Option<u64> {
    a.get(k)
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
}

fn arg_u16(a: &Value, k: &str) -> Option<u16> {
    arg_u64(a, k).and_then(|v| u16::try_from(v).ok())
}

fn arg_bool(a: &Value, k: &str) -> Option<bool> {
    a.get(k).and_then(Value::as_bool)
}

/// The interface sizes Ctrl+ and Ctrl− step through.
pub const ZOOM_STEPS: [f32; 11] = [0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5];

/// The next size up (`dir` 1) or down (-1) from `now`.
pub fn zoom_next(now: f32, dir: i32) -> f32 {
    let next = if dir > 0 {
        ZOOM_STEPS.iter().find(|s| **s > now + 0.005)
    } else {
        ZOOM_STEPS.iter().rev().find(|s| **s < now - 0.005)
    };
    next.copied().unwrap_or(now)
}

fn zoom_step(cx: &mut Ctx, dir: i32) {
    let now = cx.prefs.get().scale;
    set_zoom(cx, zoom_next(now, dir));
}

/// Set the interface size (kept in the preferences), and say it.
fn set_zoom(cx: &mut Ctx, s: f32) {
    let s = s.clamp(ZOOM_STEPS[0], ZOOM_STEPS[ZOOM_STEPS.len() - 1]);
    if (cx.prefs.get().scale - s).abs() < 1e-3 {
        return;
    }
    cx.prefs.edit().scale = s;
    let pct = (s * 100.0).round() as i32;
    cx.stores
        .bus
        .info(crate::i18n::tf("shell", "zoomNow", &[("percent", &pct)]));
}

fn arg_f32(a: &Value, k: &str) -> Option<f32> {
    a.get(k)
        .and_then(Value::as_f64)
        .map(|f| f as f32)
        .filter(|f| f.is_finite())
}

fn arg_ids(a: &Value) -> Vec<u64> {
    match (a.get("ids").and_then(Value::as_array), arg_u64(a, "id")) {
        (Some(v), _) => v.iter().filter_map(Value::as_u64).take(10_000).collect(),
        (None, Some(id)) => vec![id],
        _ => Vec::new(),
    }
}

fn arg_scheme(a: &Value) -> Option<Scheme> {
    match arg_str(a, "scheme")? {
        "dark" => Some(Scheme::Dark),
        "light" => Some(Scheme::Light),
        _ => None,
    }
}

macro_rules! go {
    ($v:expr) => {
        |cx: &mut Ctx, _: &Value| cx.go($v)
    };
}

macro_rules! cmd {
    ($id:literal, ($d:literal, $k:literal), $g:literal, $run:expr) => {
        CommandSpec {
            id: $id,
            label: ($d, $k),
            group: $g,
            enabled: yes,
            run: $run,
        }
    };
    ($id:literal, ($d:literal, $k:literal), $g:literal, $run:expr, $en:expr) => {
        CommandSpec {
            id: $id,
            label: ($d, $k),
            group: $g,
            enabled: $en,
            run: $run,
        }
    };
}

/// Every command.
pub static COMMANDS: &[CommandSpec] = &[
    // ── everywhere ──
    cmd!("palette.toggle", ("keys", "palette"), "global", |cx, _| {
        let d = &mut cx.stores.dialogs;
        d.shortcuts = false;
        d.palette = !d.palette;
    }),
    cmd!(
        "view.focus_search",
        ("keys", "search"),
        "global",
        |cx, _| cx.nav.focus_search = true
    ),
    cmd!("sheet.toggle", ("keys", "sheet"), "global", |cx, _| {
        let d = &mut cx.stores.dialogs;
        d.palette = false;
        d.shortcuts = !d.shortcuts;
    }),
    cmd!("servers.refresh", ("keys", "refresh"), "global", |cx, _| cx
        .stores
        .servers
        .refresh(false)),
    cmd!(
        "mods.update_behind",
        ("keys", "updateMods"),
        "global",
        |cx, _| {
            if cx.stores.mods.stale_count() > 0 {
                cx.go(ViewId::Mods);
                let via = cx.stores.profile.via_steam();
                cx.stores.mods.update_stale(via);
            } else {
                cx.stores.bus.ok(t("mods", "upToDate"));
            }
        }
    ),
    cmd!("rejoin", ("keys", "rejoin"), "global", |cx, _| {
        let snap = cx.stores.cli_snapshot();
        cx.stores.connect.rejoin(snap);
    }),
    cmd!("app.retry", ("shell", "initRetry"), "global", |cx, _| cx
        .stores
        .app
        .init()),
    // ── views ──
    cmd!(
        "view.servers",
        ("nav", "servers"),
        "views",
        go!(ViewId::Servers)
    ),
    cmd!(
        "view.favorites",
        ("nav", "favorites"),
        "views",
        go!(ViewId::Favorites)
    ),
    cmd!(
        "view.history",
        ("nav", "history"),
        "views",
        go!(ViewId::History)
    ),
    cmd!(
        "view.friends",
        ("nav", "friends"),
        "views",
        go!(ViewId::Friends)
    ),
    cmd!(
        "view.connect",
        ("nav", "connect"),
        "views",
        go!(ViewId::Connect)
    ),
    cmd!(
        "view.offline",
        ("nav", "offline"),
        "views",
        go!(ViewId::Offline)
    ),
    cmd!(
        "view.mods",
        ("nav", "mods"),
        "views",
        |cx, a| match arg_str(a, "focus") {
            Some(f) => cx.go_focus(ViewId::Mods, f),
            None => cx.go(ViewId::Mods),
        }
    ),
    cmd!(
        "view.options",
        ("nav", "options"),
        "views",
        go!(ViewId::Options)
    ),
    cmd!("view.news", ("nav", "news"), "views", go!(ViewId::News)),
    cmd!("view.stats", ("nav", "stats"), "views", go!(ViewId::Stats)),
    cmd!(
        "view.settings",
        ("nav", "settings"),
        "views",
        |cx, a| match arg_str(a, "focus") {
            Some(f) => cx.go_focus(ViewId::Settings, f),
            None => cx.go(ViewId::Settings),
        }
    ),
    cmd!("view.about", ("nav", "about"), "views", go!(ViewId::About)),
    cmd!("view.prev", ("keys", "viewPrev"), "views", |cx, _| {
        let v = cx.nav.view.step(-1);
        cx.go(v);
    }),
    cmd!("view.next", ("keys", "viewNext"), "views", |cx, _| {
        let v = cx.nav.view.step(1);
        cx.go(v);
    }),
    cmd!(
        "view.settings.apis",
        ("friends", "openSettings"),
        "views",
        |cx, _| {
            cx.go_focus(ViewId::Settings, "apis");
        }
    ),
    // ── the shell ──
    cmd!(
        "rail.toggle_collapse",
        ("nav", "collapse"),
        "shell",
        |cx, _| {
            let s = cx.prefs.edit();
            s.ui.rail_collapsed = !s.ui.rail_collapsed;
        }
    ),
    cmd!(
        "rejoin.dismiss",
        ("progress", "dismiss"),
        "shell",
        |cx, _| {
            let last = cx
                .stores
                .profile
                .history
                .first()
                .map(|h| format!("{}:{}", h.ip, h.port));
            cx.prefs.edit().ui.dismissed_rejoin = last;
        },
        |cx| !cx.stores.profile.history.is_empty()
    ),
    cmd!(
        "game.close",
        ("nav", "killGame"),
        "shell",
        |cx, _| {
            let req = ConfirmRequest::new(t("nav", "killGameTitle"), t("nav", "killGameMessage"))
                .confirm_label(t("nav", "killGame"))
                .danger();
            cx.stores.bus.confirm_then(req, |cx| cx.stores.game.close());
        },
        |cx| cx.stores.game.running && !cx.stores.game.closing
    ),
    cmd!(
        "i18n.set_locale",
        ("shell", "langChange"),
        "shell",
        |cx, a| {
            if let Some(l) = arg_str(a, "locale").and_then(Locale::from_code) {
                cx.set_locale(l);
            }
        }
    ),
    cmd!("pad.set_mode", ("pad", "mode"), "settings", |cx, a| {
        let mode = match arg_str(a, "mode") {
            Some("auto") => PadMode::Auto,
            Some("always") => PadMode::Always,
            Some("never") => PadMode::Never,
            _ => return,
        };
        cx.prefs.edit().ui.pad_mode = mode;
    }),
    cmd!("ui.zoom_in", ("keys", "zoomIn"), "settings", |cx, _| {
        zoom_step(cx, 1);
    }),
    cmd!("ui.zoom_out", ("keys", "zoomOut"), "settings", |cx, _| {
        zoom_step(cx, -1);
    }),
    cmd!(
        "ui.zoom_reset",
        ("keys", "zoomReset"),
        "settings",
        |cx, _| {
            set_zoom(cx, 1.0);
        }
    ),
    cmd!(
        "ui.set_scale",
        ("keys", "zoomReset"),
        "settings",
        |cx, a| {
            if let Some(s) = arg_f32(a, "scale") {
                set_zoom(cx, s);
            }
        }
    ),
    cmd!("pad.set_scale", ("pad", "scale"), "settings", |cx, a| {
        if let Some(s) = arg_f32(a, "scale").filter(|s| (0.5..=3.0).contains(s)) {
            cx.prefs.edit().ui.pad_scale = s;
        }
    }),
    cmd!("pad.set_glyphs", ("pad", "glyphs"), "settings", |cx, a| {
        let g = match arg_str(a, "glyphs") {
            Some("auto") => GlyphChoice::Auto,
            Some("xbox") => GlyphChoice::Xbox,
            Some("playStation") => GlyphChoice::PlayStation,
            Some("nintendo") => GlyphChoice::Nintendo,
            Some("steamDeck") => GlyphChoice::SteamDeck,
            _ => return,
        };
        cx.prefs.edit().pad_glyphs = g;
    }),
    // ── the controller's own moves (`crate::pad`): as if a pad had sent them ──
    cmd!("pad.move", ("keys", "move"), "pad", |cx, a| {
        let action = match arg_str(a, "dir") {
            Some("up") => PadAction::Up,
            Some("left") => PadAction::Left,
            Some("right") => PadAction::Right,
            _ => PadAction::Down,
        };
        cx.stores
            .pad
            .inject(action, arg_bool(a, "repeat").unwrap_or(false));
    }),
    cmd!("pad.accept", ("keys", "press"), "pad", |cx, _| {
        cx.stores.pad.inject(PadAction::Accept, false);
    }),
    cmd!("pad.back", ("keys", "back"), "pad", |cx, _| {
        cx.stores.pad.inject(PadAction::Back, false);
    }),
    cmd!("pad.page", ("keys", "page"), "pad", |cx, a| {
        let down = arg_bool(a, "down").unwrap_or(true);
        let action = if down {
            PadAction::PageDown
        } else {
            PadAction::PageUp
        };
        cx.stores.pad.inject(action, false);
    }),
    cmd!("pad.press", ("keys", "press"), "pad", |cx, a| {
        if let Some(action) = arg_str(a, "button").and_then(crate::pad::action_named) {
            cx.stores
                .pad
                .inject(action, arg_bool(a, "repeat").unwrap_or(false));
        }
    }),
    // ── the theme ──
    cmd!("theme.use_preset", ("theme", "scheme"), "theme", |cx, a| {
        let id = arg_str(a, "id");
        cx.stores.theme.use_theme(cx.prefs, id);
    }),
    cmd!(
        "theme.follow_system",
        ("theme", "followSystem"),
        "theme",
        |cx, _| {
            cx.stores.theme.use_theme(cx.prefs, None);
        }
    ),
    cmd!(
        "theme.set_pair",
        ("theme", "followSystem"),
        "theme",
        |cx, a| {
            if let (Some(s), Some(id)) = (arg_scheme(a), arg_str(a, "id")) {
                cx.stores.theme.set_pair(cx.prefs, s, id);
            }
        }
    ),
    cmd!(
        "theme.duplicate",
        ("theme", "duplicate"),
        "theme",
        |cx, a| {
            let (from, name) = (arg_str(a, "from"), arg_str(a, "name"));
            cx.stores.theme.duplicate(cx.prefs, from, name);
        }
    ),
    cmd!("theme.rename", ("theme", "scheme"), "theme", |cx, a| {
        if let (Some(id), Some(name)) = (arg_str(a, "id"), arg_str(a, "name")) {
            cx.stores.theme.rename(cx.prefs, id, name);
        }
    }),
    cmd!("theme.set_scheme", ("theme", "scheme"), "theme", |cx, a| {
        if let Some(s) = arg_scheme(a) {
            cx.stores.theme.set_scheme(cx.prefs, s);
        }
    }),
    cmd!(
        "theme.reset_custom",
        ("theme", "resetHint"),
        "theme",
        |cx, a| {
            if let Some(id) = arg_str(a, "id") {
                cx.stores.theme.reset(cx.prefs, id);
            }
        }
    ),
    cmd!(
        "theme.remove_custom",
        ("theme", "remove"),
        "theme",
        |cx, a| {
            if let Some(id) = arg_str(a, "id") {
                cx.stores.theme.remove(cx.prefs, id);
            }
        }
    ),
    cmd!("theme.set_token", ("theme", "scheme"), "theme", |cx, a| {
        let Some(name) = arg_str(a, "name") else {
            return;
        };
        let v = match a.get("value") {
            Some(Value::String(s)) => crate::store::theme::parse_oklch(s),
            Some(Value::Array(v)) if v.len() == 3 => {
                let f: Vec<f32> = v
                    .iter()
                    .filter_map(Value::as_f64)
                    .map(|f| f as f32)
                    .collect();
                (f.len() == 3 && f.iter().all(|x| x.is_finite())).then(|| [f[0], f[1], f[2]])
            }
            _ => None,
        };
        if let Some(v) = v {
            cx.stores.theme.set_token(cx.prefs, name, v);
        }
    }),
    cmd!(
        "theme.export_clipboard",
        ("theme", "export"),
        "theme",
        |cx, a| {
            let json = cx.stores.theme.export_json(cx.prefs, arg_str(a, "id"));
            cx.egui.copy_text(json);
            cx.stores.bus.ok(t("settings", "themeCopied"));
        }
    ),
    // ── launch options (`views/options`) ──
    cmd!(
        "options.search",
        ("options", "searchPlaceholder"),
        "options",
        |cx, a| crate::views::options::command(cx, "search", a)
    ),
    cmd!(
        "options.active_only",
        ("options", "showActiveOnly"),
        "options",
        |cx, a| crate::views::options::command(cx, "active_only", a)
    ),
    cmd!(
        "options.toggle",
        ("options", "statActive"),
        "options",
        |cx, a| crate::views::options::command(cx, "toggle", a)
    ),
    cmd!(
        "options.set_value",
        ("options", "valuePlaceholder"),
        "options",
        |cx, a| crate::views::options::command(cx, "set_value", a)
    ),
    cmd!(
        "options.use_recommended",
        ("options", "useRecommended"),
        "options",
        |cx, a| crate::views::options::command(cx, "use_recommended", a)
    ),
    cmd!(
        "options.browse_folder",
        ("options", "browseFolder"),
        "options",
        |cx, a| crate::views::options::command(cx, "browse_folder", a)
    ),
    cmd!(
        "options.clear_value",
        ("options", "clearValue"),
        "options",
        |cx, a| crate::views::options::command(cx, "clear_value", a)
    ),
    cmd!(
        "options.apply_recommended",
        ("options", "recommendApply"),
        "options",
        |cx, a| crate::views::options::command(cx, "apply_recommended", a),
        |cx| crate::views::options::can_apply(cx)
    ),
    cmd!(
        "options.copy_command",
        ("options", "copyCommand"),
        "options",
        |cx, a| crate::views::options::command(cx, "copy_command", a)
    ),
    cmd!(
        "options.copy_prefix",
        ("options", "linuxPrefix"),
        "options",
        |cx, a| crate::views::options::command(cx, "copy_prefix", a)
    ),
    cmd!(
        "options.copy_linux_command",
        ("options", "linuxCommand"),
        "options",
        |cx, a| crate::views::options::command(cx, "copy_linux_command", a)
    ),
    cmd!(
        "options.linux_raw",
        ("options", "linuxRaw"),
        "options",
        |cx, a| crate::views::options::command(cx, "linux_raw", a)
    ),
    // ── settings (`views/settings`) ──
    cmd!(
        "settings.goto_section",
        ("settings", "onThisPage"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "goto_section", a)
    ),
    cmd!(
        "settings.account.save",
        ("settings", "save"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "account.save", a)
    ),
    cmd!(
        "settings.account.discard",
        ("settings", "discard"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "account.discard", a)
    ),
    cmd!(
        "settings.account.edit_name",
        ("settings", "ingameName"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "account.edit_name", a)
    ),
    cmd!(
        "settings.steam.set_downloader",
        ("settings", "modDownloads"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.set_downloader", a)
    ),
    cmd!(
        "settings.steam.test_steamworks",
        ("settings", "steamworksTest"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.test_steamworks", a)
    ),
    cmd!(
        "settings.steam.toggle_steam_server_list",
        ("settings", "steamServerList"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.toggle_steam_server_list", a)
    ),
    cmd!(
        "settings.steam.edit_login",
        ("settings", "username"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.edit_login", a)
    ),
    cmd!(
        "settings.steam.pick_steam_root",
        ("settings", "selectSteamRoot"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.pick_steam_root", a)
    ),
    cmd!(
        "settings.steam.pick_steamcmd",
        ("settings", "selectSteamcmd"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.pick_steamcmd", a)
    ),
    cmd!(
        "settings.steam.open_steamcmd_dir",
        ("settings", "openFolder"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.open_steamcmd_dir", a)
    ),
    cmd!(
        "settings.steam.detect_steamcmd",
        ("settings", "steamcmdDetect"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.detect_steamcmd", a)
    ),
    cmd!(
        "settings.steam.install_steamcmd",
        ("settings", "steamcmdInstallWin"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "steam.install_steamcmd", a)
    ),
    cmd!(
        "settings.apis.edit_key",
        ("settings", "apiKey"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "apis.edit_key", a)
    ),
    cmd!(
        "settings.apis.edit_steam_id",
        ("settings", "steamId"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "apis.edit_steam_id", a)
    ),
    cmd!(
        "settings.apis.detect_steam_id",
        ("settings", "detectSteamId"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "apis.detect_steam_id", a)
    ),
    cmd!(
        "settings.location.open_maps",
        ("settings", "openMaps"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "location.open_maps", a)
    ),
    cmd!(
        "settings.location.clear",
        ("settings", "clearLocation"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "location.clear", a)
    ),
    cmd!(
        "settings.location.detect_ip",
        ("settings", "autodetectIp"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "location.detect_ip", a)
    ),
    cmd!(
        "settings.location.apply_manual",
        ("settings", "apply"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "location.apply_manual", a)
    ),
    cmd!(
        "settings.ping.set_concurrency",
        ("about", "pingConcurrency"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "ping.set_concurrency", a)
    ),
    cmd!(
        "settings.ping.set_timeout_auto",
        ("about", "pingTimeoutAuto"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "ping.set_timeout_auto", a)
    ),
    cmd!(
        "settings.ping.set_timeout_manual",
        ("about", "pingTimeoutManual"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "ping.set_timeout_manual", a)
    ),
    cmd!(
        "settings.ping.set_retries",
        ("about", "pingMaxRetries"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "ping.set_retries", a)
    ),
    cmd!(
        "settings.ping.toggle_scope",
        ("about", "pingScanScope"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "ping.toggle_scope", a)
    ),
    cmd!(
        "settings.excluded.remove_ip",
        ("settings", "sectionExcluded"),
        "settings",
        |cx, a| crate::views::settings::command(cx, "excluded.remove_ip", a)
    ),
    cmd!(
        "theme.derive",
        ("theme", "derive"),
        "theme",
        crate::views::settings::cmd_theme_derive
    ),
    cmd!(
        "theme.import_file",
        ("theme", "import"),
        "theme",
        crate::views::settings::cmd_theme_import
    ),
    cmd!(
        "theme.export_file",
        ("theme", "export"),
        "theme",
        crate::views::settings::cmd_theme_export
    ),
    cmd!(
        "battleye.repair_thorough",
        ("settings", "battleyeRepair"),
        "settings",
        crate::views::settings::cmd_battleye_thorough,
        crate::views::settings::battleye_repairable
    ),
    // ── updates ──
    cmd!(
        "updates.check",
        ("about", "checkUpdates"),
        "about",
        |cx, _| cx.stores.updater.check()
    ),
    cmd!(
        "updates.install",
        ("about", "updatesInstall"),
        "about",
        |cx, _| cx.stores.updater.install(),
        |cx| cx.stores.updater.info.is_some()
            && cx
                .stores
                .updater
                .support
                .as_ref()
                .is_none_or(|s| s.supported)
    ),
    cmd!(
        "updates.restart",
        ("about", "updatesRestart"),
        "about",
        |cx, _| cx.stores.updater.restart()
    ),
    cmd!(
        "battleye.repair",
        ("settings", "battleyeRepair"),
        "settings",
        |cx, _| cx.stores.battleye.repair(false)
    ),
    // ── about ──
    cmd!(
        "about.open_repo",
        ("about", "sourceRepo"),
        "about",
        crate::views::about::cmd_open_repo
    ),
    cmd!(
        "about.open_releases",
        ("about", "linkReleases"),
        "about",
        crate::views::about::cmd_open_releases
    ),
    cmd!(
        "about.report_bug",
        ("about", "linkBug"),
        "about",
        crate::views::about::cmd_report_bug
    ),
    cmd!(
        "about.open_wiki",
        ("about", "linkWiki"),
        "about",
        crate::views::about::cmd_open_wiki
    ),
    cmd!(
        "about.open_license",
        ("about", "linkLicense"),
        "about",
        crate::views::about::cmd_open_license
    ),
    cmd!(
        "about.open_source_link",
        ("about", "dataSources"),
        "about",
        crate::views::about::cmd_open_source_link
    ),
    cmd!(
        "about.open_stack_link",
        ("about", "builtWith"),
        "about",
        crate::views::about::cmd_open_stack_link
    ),
    cmd!(
        "about.copy_system_info",
        ("about", "copyInfo"),
        "about",
        crate::views::about::cmd_copy_system_info
    ),
    cmd!(
        "about.shortcuts_mode",
        ("keys", "title"),
        "about",
        crate::views::about::cmd_shortcuts_mode
    ),
    cmd!(
        "about.guide_tab",
        ("about", "guide"),
        "about",
        crate::views::about::cmd_guide_tab
    ),
    cmd!(
        "about.copy_example",
        ("about", "sharing"),
        "about",
        crate::views::about::cmd_copy_example
    ),
    // ── the profile file ──
    cmd!(
        "profile.export",
        ("about", "profileExport"),
        "settings",
        |cx, a| {
            cx.stores
                .profile
                .export_to(arg_bool(a, "include_mods").unwrap_or(true));
        }
    ),
    cmd!(
        "profile.import",
        ("about", "profileImport"),
        "settings",
        |cx, _| cx.stores.profile.import_from()
    ),
    cmd!(
        "profile.reset",
        ("about", "profileReset"),
        "settings",
        |cx, _| cx.stores.profile.reset()
    ),
    // ── the first-launch setup (`views/setup`) ──
    cmd!("setup.open", ("setup", "welcomeTitle"), "setup", |cx, a| {
        crate::views::setup::command(cx, "open", a)
    }),
    cmd!(
        "setup.next",
        ("setup", "next"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "next", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.back",
        ("setup", "back"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "back", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.go_step",
        ("setup", "stepOf"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "go_step", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.choose_downloader",
        ("setup", "downloadsTitle"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "choose_downloader", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.pick_steam_library",
        ("setup", "selectSteamRoot"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "pick_steam_library", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.pick_steamcmd",
        ("setup", "selectSteamcmd"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "pick_steamcmd", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.copy_install_command",
        ("setup", "copyCommand"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "copy_install_command", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.download_steamcmd",
        ("setup", "steamcmdInstallAuto"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "download_steamcmd", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.rescan_steamcmd",
        ("setup", "rescan"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "rescan_steamcmd", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.rescan_game",
        ("setup", "rescan"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "rescan_game", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.check_steam",
        ("setup", "checkAgain"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "check_steam", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.edit_login",
        ("setup", "username"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "edit_login", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.edit_name",
        ("setup", "ingameName"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "edit_name", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.edit_api",
        ("setup", "steamApi"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "edit_api", a),
        crate::views::setup::shown
    ),
    cmd!(
        "setup.finish",
        ("setup", "goServers"),
        "setup",
        |cx, a| crate::views::setup::command(cx, "finish", a),
        crate::views::setup::shown
    ),
    // ── joining ──
    cmd!("connect.address", ("keys", "join"), "connect", |cx, a| {
        if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_u16(a, "port")) {
            let pw = arg_str(a, "password").map(str::to_string);
            cx.join_address(ip.to_string(), port, pw);
        }
    }),
    cmd!(
        "connect.open_dzch",
        ("connect", "openDzch"),
        "connect",
        |cx, a| {
            match arg_str(a, "path").or_else(|| arg_str(a, "url")) {
                Some(p) => {
                    let snap = cx.stores.cli_snapshot();
                    cx.stores.connect.open_dzch(p.to_string(), snap);
                }
                None => cx.pick_dzch(),
            }
        }
    ),
    cmd!(
        "connect.open_in_direct",
        ("keys", "direct"),
        "connect",
        |cx, a| {
            if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_u16(a, "port")) {
                let qp = arg_u16(a, "query_port");
                let pw = arg_str(a, "password").map(str::to_string);
                cx.stores
                    .connect
                    .open_in_direct(ip.to_string(), port, qp, pw);
            }
        }
    ),
    cmd!(
        "connect.copy_link",
        ("keys", "copyLink"),
        "connect",
        |cx, a| {
            if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_u16(a, "port")) {
                let (qp, name) = (arg_u16(a, "query_port"), arg_str(a, "name"));
                cx.stores.connect.copy_link(cx.egui, ip, port, qp, name);
            } else {
                // No address given: the Direct Connect form's.
                cx.stores.direct_copy_link(cx.egui);
            }
        }
    ),
    cmd!(
        "connect.paste",
        ("connect", "pastedAddress"),
        "connect",
        |cx, a| {
            if let Some(text) = arg_str(a, "text") {
                cx.paste(text);
            }
        }
    ),
    cmd!(
        "connect.answer_password",
        ("pad", "join"),
        "connect",
        |cx, a| {
            let answer = arg_str(a, "password").map(|p| crate::store::PasswordAnswer {
                password: p.to_string(),
                save: arg_bool(a, "save").unwrap_or(true),
            });
            cx.stores.dialogs.answer_password(answer);
        }
    ),
    cmd!(
        "connect.confirm_mods",
        ("connect", "connectModalInstallConnect"),
        "connect",
        |cx, a| {
            if arg_bool(a, "cancel") == Some(true) {
                cx.stores.connect.request = None;
            } else {
                cx.answer_connect(arg_bool(a, "fetch").unwrap_or(true));
            }
        }
    ),
    // ── Direct Connect's form (store/direct.rs) ──
    cmd!("connect.query", ("connect", "query"), "connect", |cx, _| {
        cx.stores.direct_query()
    }),
    cmd!(
        "connect.join",
        ("connect", "connect"),
        "connect",
        |cx, _| { cx.stores.direct_join() }
    ),
    cmd!(
        "connect.favorite",
        ("connect", "addFavorite"),
        "connect",
        |cx, _| cx.stores.direct_favorite()
    ),
    cmd!("connect.clear", ("common", "clear"), "connect", |cx, _| {
        cx.stores.direct.clear()
    }),
    cmd!(
        "connect.export_dzch",
        ("connect", "export"),
        "connect",
        |cx, _| cx.stores.direct_export()
    ),
    cmd!(
        "connect.set_address",
        ("connect", "ipHostname"),
        "connect",
        |cx, a| {
            if let Some(v) = arg_str(a, "address") {
                cx.stores.direct.address = v.chars().take(256).collect();
                cx.stores.direct.split_address();
            }
        }
    ),
    cmd!(
        "connect.password",
        ("connect", "password"),
        "connect",
        |cx, a| {
            if let Some(v) = arg_str(a, "password") {
                cx.stores.direct.password = v.chars().take(256).collect();
                cx.stores.direct.password_from_favorite = false;
            }
        }
    ),
    cmd!(
        "connect.toggle_password",
        ("connect", "showPassword"),
        "connect",
        |cx, _| {
            let d = &mut cx.stores.direct;
            d.show_password = !d.show_password;
        }
    ),
    cmd!(
        "connect.toggle_ports",
        ("connect", "portGame"),
        "connect",
        |cx, _| {
            let d = &mut cx.stores.direct;
            d.ports_open = !d.ports_open;
        }
    ),
    cmd!("connect.port", ("connect", "port"), "connect", |cx, a| {
        if let Some(p) = a.get("port") {
            let v = p
                .as_u64()
                .map_or_else(|| p.as_str().unwrap_or("").to_string(), |n| n.to_string());
            cx.stores.direct.port = v.chars().filter(char::is_ascii_digit).take(5).collect();
        }
    }),
    cmd!(
        "connect.query_port",
        ("connect", "queryPort"),
        "connect",
        |cx, a| {
            let v = arg_u16(a, "port")
                .map(|p| p.to_string())
                .unwrap_or_default();
            cx.stores.direct.query_port = v;
        }
    ),
    cmd!(
        "connect.toggle_args",
        ("connect", "extraMods"),
        "connect",
        |cx, _| {
            let d = &mut cx.stores.direct;
            d.args_open = !d.args_open;
        }
    ),
    cmd!(
        "connect.arg_toggle",
        ("connect", "enable"),
        "connect",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.direct.toggle_arg(id);
            }
        }
    ),
    cmd!(
        "connect.arg_up",
        ("connect", "moveUp"),
        "connect",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.direct.move_arg(id, -1);
            }
        }
    ),
    cmd!(
        "connect.arg_down",
        ("connect", "moveDown"),
        "connect",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.direct.move_arg(id, 1);
            }
        }
    ),
    cmd!(
        "connect.arg_remove",
        ("connect", "remove"),
        "connect",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.direct.remove_arg(id);
            }
        }
    ),
    cmd!(
        "connect.arg_add_mod",
        ("connect", "addMod"),
        "connect",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.direct_add_mod(id);
            }
        }
    ),
    cmd!(
        "connect.arg_add_custom",
        ("connect", "addArg"),
        "connect",
        |cx, a| {
            if let Some(v) = arg_str(a, "value") {
                let v: String = v.chars().take(512).collect();
                cx.stores.direct.add_custom(&v);
            }
        }
    ),
    cmd!(
        "connect.load_recent",
        ("connect", "recent"),
        "connect",
        |cx, a| {
            if let (Some(ip), Some(port)) = (arg_str(a, "ip"), arg_u16(a, "port")) {
                let qp = arg_u16(a, "query_port");
                let pw = arg_str(a, "password").map(str::to_string);
                cx.go(ViewId::Connect);
                cx.stores.direct_load(ip, port, qp, pw);
            }
        }
    ),
    // ── mods ──
    cmd!(
        "mods.check_updates",
        ("keys", "checkMods"),
        "mods",
        |cx, a| {
            if arg_bool(a, "open") == Some(true) {
                cx.go(ViewId::Mods);
            }
            cx.stores.mods.check_updates(true);
        }
    ),
    cmd!("mods.refresh", ("mods", "refreshTitle"), "mods", |cx, _| {
        cx.stores.mods.refresh();
    }),
    cmd!("mods.cleanup", ("mods", "cleanup"), "mods", |cx, _| cx
        .stores
        .mods
        .cleanup()),
    cmd!(
        "mods.open_workshop_dir",
        ("mods", "openFolderTitle"),
        "mods",
        |cx, _| {
            cx.stores.mods.open_workshop_dir();
        }
    ),
    cmd!(
        "mods.open_mod_dir",
        ("mods", "openModFolder"),
        "mods",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.mods.open_mod_dir(id);
            }
        }
    ),
    cmd!("mods.repair", ("mods", "repair"), "mods", |cx, a| {
        if let Some(id) = arg_u64(a, "id") {
            let via = cx.stores.profile.via_steam();
            cx.stores.mods.repair(id, via);
        }
    }),
    cmd!(
        "mods.toggle_managed",
        ("keys", "managed"),
        "mods",
        |cx, a| {
            if let Some(id) = arg_u64(a, "id") {
                cx.stores.mods.toggle_managed(id);
            }
        }
    ),
    cmd!("mods.delete", ("keys", "deleteMod"), "mods", |cx, a| {
        let ids = arg_ids(a);
        match ids.as_slice() {
            [id] => cx.stores.mods.remove(*id),
            [] => {}
            _ => cx.stores.mods.remove_many(ids),
        }
    }),
    cmd!(
        "mods.steamcmd_login",
        ("settings", "loginButton"),
        "mods",
        |cx, _| cx.stores.mods.login(),
        |cx| !cx.stores.mods.busy()
    ),
    cmd!("mods.op.cancel", ("progress", "cancel"), "mods", |cx, _| cx
        .mods_cancel()),
    cmd!(
        "mods.op.dismiss",
        ("progress", "dismiss"),
        "mods",
        |cx, _| cx.mods_dismiss()
    ),
    cmd!(
        "mods.op.minimise",
        ("progress", "minimise"),
        "mods",
        |cx, _| cx.stores.mods.op.minimised = true
    ),
    cmd!(
        "mods.op.send_password",
        ("pad", "confirm"),
        "mods",
        |cx, a| {
            if let Some(input) = arg_str(a, "input").or_else(|| arg_str(a, "password")) {
                cx.stores.mods.send_input(input.to_string());
            }
        }
    ),
    cmd!(
        "mods.reopen_op",
        ("progress", "kindInstall"),
        "mods",
        |cx, _| cx.stores.mods.op.minimised = false,
        |cx| cx.stores.mods.op.active
    ),
    // ── the mods view's own (`views/mods`) ──
    cmd!(
        "mods.update_stale",
        ("mods", "updateAll"),
        "mods",
        crate::views::mods::cmd_update_stale
    ),
    cmd!(
        "mods.update_all",
        ("mods", "redownloadAll"),
        "mods",
        crate::views::mods::cmd_update_all
    ),
    cmd!(
        "mods.update_one",
        ("mods", "update"),
        "mods",
        crate::views::mods::cmd_update_one
    ),
    cmd!(
        "mods.update",
        ("keys", "updateMod"),
        "mods",
        crate::views::mods::cmd_update
    ),
    cmd!(
        "mods.install_dialog",
        ("mods", "install"),
        "mods",
        crate::views::mods::cmd_install_dialog
    ),
    cmd!(
        "mods.install",
        ("mods", "installMod"),
        "mods",
        crate::views::mods::cmd_install
    ),
    cmd!(
        "mods.review_confirm",
        ("pad", "confirm"),
        "mods",
        crate::views::mods::cmd_review_confirm
    ),
    cmd!(
        "mods.more_menu",
        ("mods", "moreActions"),
        "mods",
        crate::views::mods::cmd_more_menu
    ),
    cmd!(
        "mods.search",
        ("mods", "searchPlaceholder"),
        "mods",
        crate::views::mods::cmd_search
    ),
    cmd!(
        "mods.filter",
        ("mods", "filterLabel"),
        "mods",
        crate::views::mods::cmd_filter
    ),
    cmd!(
        "mods.steam_retry",
        ("mods", "steamNoAnswer"),
        "mods",
        crate::views::mods::cmd_steam_retry
    ),
    cmd!(
        "mods.open_settings_steam",
        ("mods", "openSettings"),
        "mods",
        crate::views::mods::cmd_open_settings_steam
    ),
    cmd!(
        "mods.tick_all",
        ("mods", "selectAll"),
        "mods",
        crate::views::mods::cmd_tick_all
    ),
    cmd!(
        "mods.sort.name",
        ("mods", "colName"),
        "mods",
        crate::views::mods::cmd_sort_name
    ),
    cmd!(
        "mods.sort.status",
        ("mods", "colStatus"),
        "mods",
        crate::views::mods::cmd_sort_status
    ),
    cmd!(
        "mods.sort.managed",
        ("mods", "colInGame"),
        "mods",
        crate::views::mods::cmd_sort_managed
    ),
    cmd!(
        "mods.sort.size",
        ("mods", "colSize"),
        "mods",
        crate::views::mods::cmd_sort_size
    ),
    cmd!(
        "mods.sort.local",
        ("mods", "colDownloaded"),
        "mods",
        crate::views::mods::cmd_sort_local
    ),
    cmd!(
        "mods.group.fold",
        ("mods", "groupCollapse"),
        "mods",
        crate::views::mods::cmd_fold
    ),
    cmd!(
        "mods.tick",
        ("keys", "tick"),
        "mods",
        crate::views::mods::cmd_tick
    ),
    cmd!(
        "mods.select",
        ("keys", "detail"),
        "mods",
        crate::views::mods::cmd_select
    ),
    cmd!(
        "mods.close_detail",
        ("mods", "closeDetail"),
        "mods",
        crate::views::mods::cmd_close_detail
    ),
    cmd!(
        "mods.row_menu",
        ("mods", "moreActions"),
        "mods",
        crate::views::mods::cmd_row_menu
    ),
    cmd!(
        "mods.row_context_menu",
        ("mods", "moreActions"),
        "mods",
        crate::views::mods::cmd_row_menu
    ),
    cmd!(
        "mods.open_workshop",
        ("mods", "openWorkshop"),
        "mods",
        crate::views::mods::cmd_open_workshop
    ),
    cmd!(
        "mods.copy_id",
        ("mods", "copyId"),
        "mods",
        crate::views::mods::cmd_copy_id
    ),
    cmd!(
        "mods.tick_stale",
        ("mods", "selectAllUpdates"),
        "mods",
        crate::views::mods::cmd_tick_stale
    ),
    cmd!(
        "mods.bulk_update",
        ("mods", "update"),
        "mods",
        crate::views::mods::cmd_bulk_update
    ),
    cmd!(
        "mods.bulk_redownload",
        ("mods", "redownload"),
        "mods",
        crate::views::mods::cmd_bulk_redownload
    ),
    cmd!(
        "mods.bulk_link",
        ("mods", "linkAction"),
        "mods",
        crate::views::mods::cmd_bulk_link
    ),
    cmd!(
        "mods.bulk_unlink",
        ("mods", "unlinkAction"),
        "mods",
        crate::views::mods::cmd_bulk_unlink
    ),
    cmd!(
        "mods.bulk_delete",
        ("mods", "deleteSelectedTitle"),
        "mods",
        crate::views::mods::cmd_bulk_delete
    ),
    cmd!(
        "mods.clear_ticks",
        ("mods", "clearSelectionTitle"),
        "mods",
        crate::views::mods::cmd_clear_ticks
    ),
    cmd!(
        "mods.move",
        ("keys", "move"),
        "mods",
        crate::views::mods::cmd_move
    ),
    cmd!(
        "mods.edges",
        ("keys", "edges"),
        "mods",
        crate::views::mods::cmd_edges
    ),
    cmd!(
        "mods.op.toggle_log",
        ("progress", "showLog"),
        "mods",
        crate::views::mods::op::cmd_toggle_log
    ),
    cmd!(
        "mods.op.manual_login",
        ("progress", "dontTrust"),
        "mods",
        crate::views::mods::op::cmd_manual_login
    ),
    cmd!(
        "log.filter",
        ("common", "filterLines"),
        "mods",
        crate::views::mods::op::cmd_log_filter
    ),
    cmd!(
        "log.copy",
        ("common", "copy"),
        "mods",
        crate::views::mods::op::cmd_log_copy
    ),
    cmd!(
        "log.follow",
        ("common", "live"),
        "mods",
        crate::views::mods::op::cmd_log_follow
    ),
    // ── the server browser (`views/servers/cmds.rs`) ──
    cmd!(
        "servers.toggle_detail",
        ("servers", "toggleDetails"),
        "servers",
        crate::views::servers::cmds::toggle_detail
    ),
    cmd!(
        "servers.sort_by",
        ("servers", "colServer"),
        "servers",
        crate::views::servers::cmds::sort_by
    ),
    cmd!(
        "servers.sort_busiest",
        ("servers", "figSortPlayers"),
        "servers",
        crate::views::servers::cmds::sort_busiest
    ),
    cmd!(
        "servers.sort_closest",
        ("servers", "figSortPing"),
        "servers",
        crate::views::servers::cmds::sort_closest
    ),
    cmd!(
        "servers.toggle_hide_full",
        ("servers", "hideFull"),
        "servers",
        crate::views::servers::cmds::toggle_hide_full
    ),
    cmd!(
        "servers.toggle_hide_empty",
        ("servers", "hideEmpty"),
        "servers",
        crate::views::servers::cmds::toggle_hide_empty
    ),
    cmd!(
        "servers.toggle_modded_only",
        ("servers", "filterModsTitleAll"),
        "servers",
        crate::views::servers::cmds::toggle_modded_only
    ),
    cmd!(
        "servers.search",
        ("servers", "searchPlaceholder"),
        "servers",
        crate::views::servers::cmds::search
    ),
    cmd!(
        "servers.filter.cycle",
        ("servers", "moreFilters"),
        "servers",
        crate::views::servers::cmds::cycle
    ),
    cmd!(
        "servers.filter.cycle_1pp",
        ("servers", "filterFpTitleAll"),
        "servers",
        crate::views::servers::cmds::cycle_1pp
    ),
    cmd!(
        "servers.filter.cycle_password",
        ("servers", "filterPwdTitleAll"),
        "servers",
        crate::views::servers::cmds::cycle_password
    ),
    cmd!(
        "servers.filter.cycle_battleye",
        ("servers", "filterBeTitleAll"),
        "servers",
        crate::views::servers::cmds::cycle_battleye
    ),
    cmd!(
        "servers.filter.cycle_modded",
        ("servers", "filterModsTitleAll"),
        "servers",
        crate::views::servers::cmds::cycle_modded
    ),
    cmd!(
        "servers.filter.cycle_official",
        ("servers", "filterOfficialTitleAll"),
        "servers",
        crate::views::servers::cmds::cycle_official
    ),
    cmd!(
        "servers.filter.map",
        ("servers", "allMaps"),
        "servers",
        crate::views::servers::cmds::map
    ),
    cmd!(
        "servers.filter.more_popover",
        ("servers", "moreFilters"),
        "servers",
        crate::views::servers::cmds::more_popover
    ),
    cmd!(
        "servers.filter.max_ping",
        ("servers", "maxPing"),
        "servers",
        crate::views::servers::cmds::max_ping
    ),
    cmd!(
        "servers.filter.hide_empty",
        ("servers", "hideEmpty"),
        "servers",
        crate::views::servers::cmds::hide_empty
    ),
    cmd!(
        "servers.filter.hide_full",
        ("servers", "hideFull"),
        "servers",
        crate::views::servers::cmds::hide_full
    ),
    cmd!(
        "servers.filter.toggle_excluded",
        ("servers", "excludedReveal"),
        "servers",
        crate::views::servers::cmds::toggle_excluded
    ),
    cmd!(
        "servers.filter.remove_one",
        ("servers", "clearFilters"),
        "servers",
        crate::views::servers::cmds::remove_one
    ),
    cmd!(
        "servers.filter.clear",
        ("servers", "clearFilters"),
        "servers",
        crate::views::servers::cmds::clear
    ),
    cmd!(
        "servers.sources.details",
        ("sources", "title"),
        "servers",
        crate::views::servers::cmds::sources_details
    ),
    cmd!(
        "servers.row.select",
        ("keys", "detail"),
        "servers",
        crate::views::servers::cmds::select
    ),
    cmd!(
        "servers.row.show_mods",
        ("servers", "colMods"),
        "servers",
        crate::views::servers::cmds::show_mods
    ),
    cmd!(
        "servers.row.refresh_players",
        ("detail", "refreshLive"),
        "servers",
        crate::views::servers::cmds::refresh_live
    ),
    cmd!(
        "servers.row.copy_address",
        ("keys", "copyAddress"),
        "servers",
        crate::views::servers::cmds::copy_address
    ),
    cmd!(
        "servers.row.exclude_ip",
        ("servers", "colServer"),
        "servers",
        crate::views::servers::cmds::exclude
    ),
    cmd!(
        "list.join",
        ("keys", "join"),
        "servers",
        crate::views::servers::cmds::join
    ),
    cmd!(
        "list.fav",
        ("keys", "favorite"),
        "servers",
        crate::views::servers::cmds::favorite
    ),
    cmd!(
        "list.ping",
        ("keys", "ping"),
        "servers",
        crate::views::servers::cmds::ping
    ),
    cmd!(
        "list.copy",
        ("keys", "copyAddress"),
        "servers",
        crate::views::servers::cmds::copy_address
    ),
    cmd!(
        "list.link",
        ("keys", "copyLink"),
        "servers",
        crate::views::servers::cmds::copy_link
    ),
    cmd!(
        "list.direct",
        ("keys", "direct"),
        "servers",
        crate::views::servers::cmds::direct
    ),
    cmd!(
        "list.move",
        ("keys", "move"),
        "servers",
        crate::views::servers::cmds::list_move
    ),
    cmd!(
        "list.page",
        ("keys", "page"),
        "servers",
        crate::views::servers::cmds::list_page
    ),
    cmd!(
        "list.edges",
        ("keys", "edges"),
        "servers",
        crate::views::servers::cmds::list_edges
    ),
    cmd!(
        "list.close",
        ("keys", "deselect"),
        "servers",
        crate::views::servers::cmds::list_close
    ),
    // ── favourites (`views/favorites`, on the shared saved list `views/saved`) ──
    cmd!(
        "favorites.toggle_detail",
        ("servers", "toggleDetails"),
        "favorites",
        crate::views::favorites::toggle_detail
    ),
    cmd!(
        "favorites.toggle_listed_only",
        ("favorites", "onlineOnly"),
        "favorites",
        crate::views::favorites::toggle_listed_only
    ),
    cmd!(
        "favorites.sort_by",
        ("servers", "colServer"),
        "favorites",
        crate::views::favorites::sort_by
    ),
    cmd!(
        "favorites.search",
        ("favorites", "searchPlaceholder"),
        "favorites",
        crate::views::favorites::search
    ),
    cmd!(
        "favorites.select",
        ("keys", "detail"),
        "favorites",
        crate::views::favorites::select
    ),
    cmd!(
        "favorites.remove",
        ("servers", "removeFavorite"),
        "favorites",
        crate::views::favorites::remove
    ),
    // ── history (`views/history`, on the shared saved list `views/saved`) ──
    cmd!(
        "history.clear",
        ("history", "clearAll"),
        "history",
        crate::views::history::clear,
        |cx| !cx.stores.profile.history.is_empty()
    ),
    cmd!(
        "history.toggle_detail",
        ("servers", "toggleDetails"),
        "history",
        crate::views::history::toggle_detail
    ),
    cmd!(
        "history.toggle_listed_only",
        ("favorites", "onlineOnly"),
        "history",
        crate::views::history::toggle_listed_only
    ),
    cmd!(
        "history.sort_by",
        ("history", "colLastPlayed"),
        "history",
        crate::views::history::sort_by
    ),
    cmd!(
        "history.search",
        ("favorites", "searchPlaceholder"),
        "history",
        crate::views::history::search
    ),
    cmd!(
        "history.select",
        ("keys", "detail"),
        "history",
        crate::views::history::select
    ),
    cmd!(
        "history.remove",
        ("history", "remove"),
        "history",
        crate::views::history::remove
    ),
    cmd!("saved.remove", ("keys", "remove"), "favorites", |cx, a| {
        if let Some(k) = crate::views::saved::cmds::on_screen(cx) {
            crate::views::saved::cmds::remove(cx, k, a);
        }
    }),
    cmd!(
        "servers.detail.resize",
        ("detail", "title"),
        "servers",
        crate::views::servers::cmds::resize
    ),
    cmd!(
        "servers.detail.refresh_live",
        ("detail", "refreshLive"),
        "servers",
        crate::views::servers::cmds::refresh_live
    ),
    cmd!(
        "servers.detail.tab",
        ("detail", "sections"),
        "servers",
        crate::views::servers::cmds::tab
    ),
    cmd!(
        "servers.detail.copy_address",
        ("detail", "copyIp"),
        "servers",
        crate::views::servers::cmds::detail_copy_address
    ),
    cmd!(
        "servers.detail.download_and_join",
        ("servers", "connect"),
        "servers",
        crate::views::servers::cmds::download_and_join
    ),
    cmd!(
        "servers.detail.install_missing",
        ("detail", "filterMissing"),
        "servers",
        crate::views::servers::cmds::install_missing
    ),
    cmd!(
        "servers.detail.update_stale",
        ("detail", "filterStale"),
        "servers",
        crate::views::servers::cmds::update_stale
    ),
    cmd!(
        "servers.detail.search_players",
        ("detail", "searchPlayers"),
        "servers",
        crate::views::servers::cmds::search_players
    ),
    cmd!(
        "servers.detail.search_rules",
        ("detail", "searchRules"),
        "servers",
        crate::views::servers::cmds::search_rules
    ),
    cmd!(
        "servers.detail.mods_filter",
        ("detail", "filterAll"),
        "servers",
        crate::views::servers::cmds::mods_filter
    ),
    cmd!(
        "servers.detail.refresh_stats",
        ("detail", "dmRefresh"),
        "servers",
        crate::views::servers::cmds::refresh_stats
    ),
    cmd!(
        "servers.detail.stats_range",
        ("detail", "dmRange"),
        "servers",
        crate::views::servers::cmds::stats_range
    ),
    cmd!(
        "servers.detail.open_link",
        ("detail", "dmLinks"),
        "servers",
        crate::views::servers::cmds::open_link
    ),
    // ── friends, the scan ──
    cmd!(
        "friends.refresh",
        ("friends", "refresh"),
        "friends",
        |cx, _| {
            let shown = cx.nav.view == ViewId::Friends;
            cx.stores.friends.load(shown);
        }
    ),
    cmd!(
        "friends.search",
        ("friends", "search"),
        "friends",
        |cx, a| {
            // With `query`, search for it; without, put the cursor in the field.
            match arg_str(a, "query") {
                Some(q) => q.clone_into(&mut cx.stores.friends.query),
                None => cx.nav.focus_search = true,
            }
            if cx.nav.view != ViewId::Friends {
                cx.go(ViewId::Friends);
            }
        }
    ),
    cmd!(
        "friends.message",
        ("friends", "sendMessage"),
        "friends",
        |cx, a| {
            if let Some(id) = arg_str(a, "steam_id") {
                crate::views::friends::open_steam(cx, &crate::views::friends::message_url(id));
            }
        }
    ),
    cmd!(
        "friends.profile",
        ("friends", "openProfile"),
        "friends",
        |cx, a| {
            if let Some(id) = arg_str(a, "steam_id") {
                crate::views::friends::open_steam(cx, &crate::views::friends::profile_url(id));
            }
        }
    ),
    cmd!(
        "friends.select_server",
        ("friends", "details"),
        "friends",
        |cx, a| {
            // `{ip, port, name?}` shows that server's details; anything else closes them.
            cx.stores.friends.selected = match (arg_str(a, "ip"), arg_u16(a, "port")) {
                (Some(ip), Some(port)) => Some(crate::store::friends::Selected {
                    ip: ip.to_string(),
                    port,
                    name: arg_str(a, "name").unwrap_or_default().to_string(),
                }),
                _ => None,
            };
        }
    ),
    cmd!("servers.scan", ("nav", "sbRescan"), "servers", |cx, _| cx
        .stores
        .servers
        .start_scan()),
    cmd!(
        "servers.scan_pause",
        ("nav", "pauseScan"),
        "servers",
        |cx, _| {
            cx.stores.servers.toggle_scan_pause();
        }
    ),
    cmd!(
        "servers.scan_cancel",
        ("common", "cancel"),
        "servers",
        |cx, _| cx.stores.servers.cancel_scan()
    ),
    // ── the shell's own (`shell/mod.rs`) ──
    cmd!(
        "window.drag",
        ("nav", "navigation"),
        "shell",
        crate::shell::cmd_window_drag
    ),
    cmd!(
        "window.minimize_maximize_close",
        ("shell", "windowMaximize"),
        "shell",
        crate::shell::cmd_window
    ),
    cmd!(
        "settings.open_account",
        ("shell", "titlebarEditAccount"),
        "shell",
        |cx, _| cx.go_focus(ViewId::Settings, "account")
    ),
    cmd!(
        "settings.open_steam",
        ("nav", "sbSteamSettings"),
        "shell",
        |cx, _| cx.go_focus(ViewId::Settings, "steam")
    ),
    cmd!(
        "about.open_update",
        ("about", "updatesInstall"),
        "shell",
        |cx, _| cx.go_focus(ViewId::About, "update")
    ),
    cmd!(
        "theme.menu",
        ("theme", "change"),
        "shell",
        crate::shell::cmd_theme_menu
    ),
    cmd!(
        "steam.downloader_menu",
        ("nav", "sbDownloader"),
        "shell",
        crate::shell::cmd_steam_menu
    ),
    cmd!(
        "palette.actions",
        ("keys", "actions"),
        "shell",
        crate::shell::cmd_palette_actions
    ),
    cmd!(
        "palette.connect_address",
        ("keys", "address"),
        "shell",
        crate::shell::cmd_palette_address
    ),
    cmd!(
        "palette.navigate",
        ("keys", "footerMove"),
        "shell",
        crate::shell::cmd_palette_navigate
    ),
    cmd!(
        "field.escape",
        ("keys", "back"),
        "shell",
        crate::shell::cmd_field_escape
    ),
    // ── news ──
    cmd!(
        "news.refresh",
        ("news", "refresh"),
        "news",
        crate::views::news::cmd_refresh
    ),
    cmd!(
        "news.open_site",
        ("news", "openAllOnSite"),
        "news",
        crate::views::news::cmd_open_site
    ),
    cmd!(
        "news.filter_category",
        ("news", "allCategories"),
        "news",
        crate::views::news::cmd_filter_category
    ),
    cmd!(
        "news.select",
        ("news", "selectArticle"),
        "news",
        crate::views::news::cmd_select
    ),
    cmd!(
        "news.move",
        ("keys", "article"),
        "news",
        crate::views::news::cmd_move
    ),
    cmd!(
        "news.open",
        ("keys", "openArticle"),
        "news",
        crate::views::news::cmd_open_article,
        crate::views::news::can_open_article
    ),
    cmd!(
        "news.open_article",
        ("news", "openBrowser"),
        "news",
        crate::views::news::cmd_open_article,
        crate::views::news::can_open_article
    ),
    cmd!(
        "news.open_link",
        ("news", "openBrowser"),
        "news",
        crate::views::news::cmd_open_link
    ),
    cmd!(
        "news.zoom_image",
        ("news", "zoomImage"),
        "news",
        crate::views::news::cmd_zoom_image
    ),
    cmd!(
        "news.retry_article",
        ("news", "retry"),
        "news",
        crate::views::news::cmd_retry_article
    ),
    cmd!(
        "news.patch_notes",
        ("news", "patchNotes"),
        "news",
        crate::views::news::cmd_patch_notes
    ),
    cmd!(
        "news.close_lightbox",
        ("news", "close"),
        "news",
        crate::views::news::cmd_close_lightbox
    ),
    // ── offline (`views/offline`) ──
    cmd!(
        "offline.refresh",
        ("offline", "refreshTitle"),
        "offline",
        crate::views::offline::cmd_refresh,
        crate::views::offline::can_act
    ),
    cmd!(
        "offline.open_missions_dir",
        ("offline", "exploreTitle"),
        "offline",
        crate::views::offline::cmd_open_missions_dir
    ),
    cmd!(
        "offline.install",
        ("offline", "installUpdate"),
        "offline",
        crate::views::offline::cmd_install,
        crate::views::offline::can_act
    ),
    cmd!(
        "offline.launch",
        ("offline", "launch"),
        "offline",
        crate::views::offline::cmd_launch
    ),
    cmd!(
        "offline.open_dir",
        ("offline", "openFolder"),
        "offline",
        crate::views::offline::cmd_open_dir
    ),
    cmd!(
        "offline.clear_save",
        ("offline", "clearSave"),
        "offline",
        crate::views::offline::cmd_clear_save,
        crate::views::offline::can_act
    ),
    cmd!(
        "offline.remove_mission",
        ("offline", "removeMission"),
        "offline",
        crate::views::offline::cmd_remove_mission,
        crate::views::offline::can_act
    ),
    cmd!(
        "offline.open_project",
        ("offline", "project"),
        "offline",
        crate::views::offline::cmd_open_project
    ),
    cmd!(
        "offline.clear_saves",
        ("offline", "clearSaves"),
        "offline",
        crate::views::offline::cmd_clear_saves,
        crate::views::offline::can_maintain
    ),
    cmd!(
        "offline.remove_all",
        ("offline", "removeAll"),
        "offline",
        crate::views::offline::cmd_remove_all,
        crate::views::offline::can_maintain
    ),
    // ── stats (`views/stats`) ──
    cmd!(
        "stats.set_range",
        ("stats", "rangeLabel"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "set_range", a)
    ),
    cmd!(
        "stats.calendar_prev",
        ("stats", "prevYear"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "calendar_prev", a)
    ),
    cmd!(
        "stats.calendar_next",
        ("stats", "nextYear"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "calendar_next", a)
    ),
    cmd!(
        "stats.calendar_navigate",
        ("stats", "sectionActivity"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "calendar_navigate", a)
    ),
    cmd!(
        "stats.filter_day",
        ("stats", "pickHint"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "filter_day", a)
    ),
    cmd!(
        "stats.clear_day",
        ("stats", "filterRemove"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "clear_day", a)
    ),
    cmd!(
        "stats.filter_text",
        ("stats", "sectionPlaces"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "filter_text", a)
    ),
    cmd!(
        "stats.history_search",
        ("stats", "historySearch"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "history_search", a)
    ),
    cmd!(
        "stats.history_more",
        ("stats", "sectionHistory"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "history_more", a)
    ),
    cmd!(
        "stats.toggle_all_places",
        ("stats", "showFewerPlaces"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "toggle_all_places", a)
    ),
    cmd!(
        "stats.forget_session",
        ("stats", "deleteSession"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "forget_session", a)
    ),
    cmd!(
        "stats.week_navigate",
        ("stats", "sectionWhen"),
        "stats",
        |cx, a| crate::views::stats::command(cx, "week_navigate", a)
    ),
];

/// Every command.
pub fn all() -> &'static [CommandSpec] {
    COMMANDS
}

/// The command with this id.
pub fn find(id: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// Whether the catalog's command id is carried: registered, or a pattern whose every
/// instance is (`view.<id>`).
pub fn registered(id: &str) -> bool {
    if id == "view.<id>" {
        return ViewId::ALL
            .iter()
            .all(|v| find(&format!("view.{}", v.id())).is_some());
    }
    find(id).is_some()
}

/// Run a command by id. Errors: unknown, or not enabled now.
pub fn run(cx: &mut Ctx, id: &str, args: &Value) -> Result<(), String> {
    let c = find(id).ok_or_else(|| format!("no command \"{id}\""))?;
    if !(c.enabled)(cx) {
        return Err(format!("\"{id}\" is not available now"));
    }
    (c.run)(cx, args);
    cx.egui.request_repaint();
    Ok(())
}

/// Run `id` or `id {json}` (a toast's button, the control channel).
pub fn run_line(cx: &mut Ctx, line: &str) -> Result<(), String> {
    let line = line.trim();
    let (id, args) = match line.split_once(' ') {
        Some((id, rest)) => {
            let v: Value = serde_json::from_str(rest.trim()).map_err(|e| e.to_string())?;
            (id, v)
        }
        None => (line, Value::Null),
    };
    run(cx, id, &args)
}

/// The window's own keys, each frame, before the views draw: matched presses are
/// consumed so no widget sees them too.
pub fn handle_keys(cx: &mut Ctx) {
    let typing = cx.egui.egui_wants_keyboard_input();
    let overlay = cx.stores.dialogs.open()
        || cx.egui.any_popup_open()
        || cx.egui.memory(|m| m.top_modal_layer().is_some());
    let mut hits: Vec<(usize, &'static str)> = Vec::new();
    cx.egui.input(|i| {
        for (n, e) in i.events.iter().enumerate() {
            let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = e
            else {
                continue;
            };
            let found = SHORTCUTS
                .iter()
                .filter(|s| matches!(s.scope, Scope::Global | Scope::Views))
                .find_map(|s| {
                    s.keys
                        .iter()
                        .find(|c| c.matches(*key, *physical_key, *modifiers))
                        .map(|c| (s, *c))
                });
            let Some((s, combo)) = found else {
                continue;
            };
            // Ctrl+K toggles the palette from anywhere, over anything.
            if s.id == "palette.toggle" {
                hits.push((n, s.id));
                continue;
            }
            // Esc is whatever is open's (dialogs, popups, a text field).
            if s.id == "back" || overlay {
                continue;
            }
            // A key with no Ctrl is the field's while one is focused.
            if typing && !combo.ctrl && !is_fkey(combo.key) {
                continue;
            }
            hits.push((n, s.id));
        }
    });
    if hits.is_empty() {
        return;
    }
    let consumed: Vec<usize> = hits.iter().map(|(n, _)| *n).collect();
    cx.egui.input_mut(|i| {
        let mut n = 0;
        i.events.retain(|_| {
            let keep = !consumed.contains(&n);
            n += 1;
            keep
        });
    });
    for (_, id) in hits {
        let _ = run(cx, id, &Value::Null);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::i18n;
    use rustc_hash::FxHashSet;

    #[test]
    fn zoom_steps_and_keys() {
        assert_eq!(zoom_next(1.0, 1), 1.1);
        assert_eq!(zoom_next(1.0, -1), 0.9);
        assert_eq!(zoom_next(2.5, 1), 2.5);
        assert_eq!(zoom_next(0.67, -1), 0.67);
        // A size between steps goes to the next one.
        assert_eq!(zoom_next(1.2, 1), 1.25);
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::COMMAND | Modifiers::SHIFT;
        assert!(Combo::ctrl(Key::Plus).matches(Key::Plus, None, shift));
        assert!(Combo::ctrl(Key::Equals).matches(Key::Equals, None, ctrl));
        assert!(Combo::ctrl(Key::Minus).matches(Key::Minus, None, ctrl));
        assert!(!Combo::ctrl(Key::Minus).matches(Key::Minus, None, Modifiers::NONE));
    }

    #[test]
    fn ids_are_unique() {
        let mut seen = FxHashSet::default();
        for c in all() {
            assert!(seen.insert(c.id), "command {} twice", c.id);
        }
        let mut seen = FxHashSet::default();
        for s in SHORTCUTS {
            assert!(seen.insert(s.id), "shortcut {} twice", s.id);
        }
    }

    #[test]
    fn window_shortcuts_do_not_conflict() {
        let mut seen = FxHashSet::default();
        for s in SHORTCUTS
            .iter()
            .filter(|s| matches!(s.scope, Scope::Global | Scope::Views))
        {
            for c in s.keys {
                assert!(seen.insert(*c), "{} reuses {}", s.id, c.text());
            }
        }
        // Within each view scope, too.
        for scope in [S::List, S::Mods, S::News, S::Palette] {
            let mut seen = FxHashSet::default();
            for s in in_scope(scope) {
                for c in s.keys {
                    assert!(seen.insert(*c), "{} reuses {}", s.id, c.text());
                }
            }
        }
    }

    #[test]
    fn window_shortcuts_are_commands() {
        for s in SHORTCUTS
            .iter()
            .filter(|s| matches!(s.scope, Scope::Global | Scope::Views) && s.id != "back")
        {
            assert!(find(s.id).is_some(), "{} has no command", s.id);
        }
        for v in ViewId::ALL {
            assert!(find(&format!("view.{}", v.id())).is_some());
            assert!(view_key(v).is_some(), "{v:?}");
        }
        assert!(registered("view.<id>"));
    }

    #[test]
    fn words_exist() {
        for c in all() {
            assert!(i18n::has(c.label.0, c.label.1), "{}: {:?}", c.id, c.label);
        }
        for s in SHORTCUTS {
            if let Some(l) = s.label {
                assert!(i18n::has("keys", l), "{}: keys.{l}", s.id);
            }
            if let Some(h) = s.hint {
                assert!(i18n::has("pad", h), "{}: pad.{h}", s.id);
            }
        }
    }

    #[test]
    fn matching_follows_the_registry() {
        let m = Modifiers::NONE;
        let ctrl = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let q = Combo::key(Key::Questionmark);
        assert!(q.matches(Key::Questionmark, None, shift));
        assert!(q.matches(Key::Questionmark, None, m));
        let c1 = Combo::ctrl(Key::Num1);
        assert!(c1.matches(Key::Num1, Some(Key::Num1), ctrl));
        assert!(
            c1.matches(Key::A, Some(Key::Num1), ctrl),
            "AZERTY by the physical key"
        );
        assert!(!c1.matches(Key::Num1, None, m));
        assert!(!Combo::ctrl(Key::K).matches(Key::K, None, ctrl | shift));
        assert!(!Combo::key(Key::Escape).matches(Key::Escape, None, shift));
        assert_eq!(Combo::ctrl(Key::PageUp).text(), "Ctrl+PgUp");
        assert_eq!(Combo::ctrl(Key::Comma).text(), "Ctrl+,");
    }
}
