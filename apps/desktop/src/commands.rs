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
        if (self.shift || !self.printable() || self.ctrl) && m.shift != self.shift {
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
    // ── mods ──
    cmd!(
        "mods.check_updates",
        ("keys", "checkMods"),
        "mods",
        |cx, a| {
            if arg_bool(a, "open") == Some(true) {
                cx.go(ViewId::Mods);
            }
            let key = cx.stores.profile.has_api_key();
            cx.stores.mods.check_updates(true, key);
        }
    ),
    cmd!("mods.refresh", ("mods", "refreshTitle"), "mods", |cx, _| {
        let key = cx.stores.profile.has_api_key();
        cx.stores.mods.refresh(key);
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
