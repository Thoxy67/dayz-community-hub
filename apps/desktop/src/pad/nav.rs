//! The pad's actions, acted on in the window (= `lib/gamepad/nav.ts`), the egui way.
//!
//! - **Directions** move egui's keyboard focus to the nearest focusable widget that way
//!   ([`super::spatial`]), among the widgets the last frame drew (egui's own widget
//!   rects), within the topmost dialog or popup, else the window. A move prefers to stay
//!   in its region (title bar, rail, status bar, the main area, a pane or a drawer). A
//!   widget scrolled out of sight counts when it is in the same region (the next field
//!   below the fold): an AccessKit `ScrollIntoView` brings it into view. A list walked
//!   row by row (`kit::VirtualList` with a cursor) is one stop: Up and Down inside it run
//!   the view's own list keys (`list.move`, `mods.move`), a held direction going 1, 3,
//!   then 10 rows at a time; Up past its first row, Left and Right leave it (sideways
//!   only to what lies beside the list: a pane, a drawer, a row's own buttons; never the
//!   title bar or the filters above). A heading registered sticky (`registry::sticky`)
//!   hides what is under it: a widget there scrolls into view (centred) when focused.
//! - **A** clicks what has the focus (an AccessKit click: exactly what Enter on a
//!   focused button does, without the key reaching anything else); a text field just
//!   takes the focus (and Steam's keyboard comes up in Steam's big-screen interface); on
//!   a server list's row it joins (`list.join`).
//! - **B** on a text field leaves the field downward (in a dialog too: the dialog stays);
//!   else it closes the topmost popup or dialog (an Escape for it); else it is Escape
//!   for the view (deselect, close the details).
//! - **LB/RB** go to the previous/next view and land there once it is drawn; **LT/RT**
//!   switch the tabs whose part of the window holds the focus (`registry::tabs`, which
//!   every `kit::TabStrip` fills: the server's details, the guide), else page a list (`list.page`) or scroll
//!   what holds the focus; the **right stick** scrolls the reading pane
//!   (`registry::reading_pane`).
//! - **Menu** opens the right-click menu of what has the focus (a list's row at its
//!   cursor, a friend's server: `kit::menu::context` registers them), at the row; with
//!   no such menu it opens the palette. **View** opens the palette (the shortcuts sheet
//!   is one of its actions).
//! - **X, Y, R3, L3** run the view's own commands when they apply (the registry's pad
//!   buttons: X favourite or remove, Y ping…), else the app's (L3 rejoin); in a dialog X
//!   presses its confirming button (the last of its footer).
//! - The search palette is driven with its own keys (arrows, Enter, Tab, Escape).
//! - With nothing focused, a press **lands**: on what had the focus last in this view,
//!   else where the view says (`registry::primary`), else the view's list at its row,
//!   else the first thing in reading order; in a dialog on its primary, else its safe
//!   button (the first of its footer), never on the close cross.
//! - **Keys** without a pad (a Steam Deck in Desktop Mode without Steam: its D-pad types
//!   arrows, A Enter, B Escape): while the pad drives the window (or on a Deck) and the
//!   palette does not have the keys, the arrows, Enter and Escape are these actions,
//!   taken before egui reads them ([`take_keys`]). In a text field being typed in only
//!   Escape is (B). Moved by the keys onto a text field, the pad rests on it without
//!   typing (A types), so the arrows go on moving.
//! - **While DayZ runs** the pad's presses are the game's (Linux readers keep reading
//!   unfocused, gamescope may never say): dropped until the window has been seen to
//!   lose the focus and get it back, or the pointer or the keyboard is used in it.
//!
//! Nothing runs while no pad is used: the hub's thread sleeps until a pad acts, and
//! [`frame`] only drains what it sent. The few frames a landing waits for (a view being
//! drawn, a popup opening) are asked for one by one and bounded.

use std::collections::HashMap;

use dz_hub::features::gamepad::PadAction;
use egui::accesskit::{self, Action};
use egui::{Context, CornerRadius, Event, Id, Key, LayerId, Modifiers, Order, Rect, Stroke};
use serde_json::{Value, json};

use super::registry::{self, Frame, ListSlot};
use super::spatial::{self, Dir};
use crate::app::Ctx;
use crate::commands::{self, PadButton, SHORTCUTS, Scope};
use crate::kit::osk::{Layout as OskLayout, Press};
use crate::prefs::{OskChoice, PadMode, Prefs};
use crate::store::Stores;
use crate::store::pad::InputMode;
use crate::theme::{TokensExt, size};
use crate::views::ViewId;

/// What the pad is on, besides egui's own focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    None,
    /// A widget (it also has egui's keyboard focus).
    Widget(Id),
    /// A list walked row by row, at its cursor.
    List(Id),
}

/// The controller's navigation state (kept in `store::pad::Pad`).
#[derive(Debug, Default)]
pub struct PadNav {
    focus: Focus,
    /// How many repeats in a row the held direction has fired.
    streak: u32,
    last: Option<PadAction>,
    /// The widget that had the focus last in each view, to come back to it.
    remembered: HashMap<ViewId, Id>,
    /// Frames left in which a lost focus is put back (after A or B).
    settle: u8,
    /// The settling waits for a popup a command may open before landing elsewhere.
    await_popup: bool,
    /// A view switched to with LB/RB: land in it once it is drawn (frames left, and the
    /// pass that switched: its widgets are the old view's).
    land_in: Option<(ViewId, u8, u64)>,
    /// After a page scroll: the height the focus was at, frames left to follow it.
    refocus: Option<(f32, u8)>,
    /// Give this widget the focus back at the end of the frame (B's Escape went by).
    restore: Option<Id>,
    was_pad: bool,
    /// DayZ was running last frame.
    game_was: bool,
    /// The game started and the window has not had the focus since: the pad is its.
    game_holds: bool,
    /// Since the game started, the window was seen without the focus.
    went_away: bool,
    /// The game's hold dropped a press and a toast said so (once per hold).
    told: bool,
    /// The arrows, Enter and Escape taken out of this frame's input ([`take_keys`]).
    keys: Vec<(PadAction, bool)>,
    /// The action being handled came from the keys, not a pad.
    by_keys: bool,
    /// A text field the pad is on without typing in it (egui's focus is elsewhere): the
    /// keys moved onto it, or B left it with nothing below. A types in it.
    rest: Option<Id>,
}

impl PadNav {
    pub fn focus(&self) -> Focus {
        self.focus
    }
}

/// Frames a landing waits for a view or a popup, at most.
const WAIT_FRAMES: u8 = 30;
/// What leaving a region sideways costs, in points of distance.
const ELSEWHERE: f32 = 400.0;
/// What leaving it up or down costs: only when it has nothing more that way.
const AWAY: f32 = 1e6;
/// AccessKit scrolls this far per request (egui's own constant).
const AK_SCROLL: f32 = 100.0;

/// Whether the window is narrow enough for the rail to fold on its own: below 1100
/// points with a mouse, 1200 with a controller (the list gets the room).
pub fn rail_folds(ctx: &Context, stores: &Stores, prefs: &Prefs) -> bool {
    let p = prefs.get();
    let fold = if stores.pad.mode(p.ui.pad_mode) == InputMode::Gamepad {
        1200.0
    } else {
        1100.0
    };
    ctx.content_rect().width() < fold
}

/// Whether the rail is drawn icon-only: the preference, or a narrow window
/// ([`rail_folds`]) unless the player opened the rail there.
pub fn rail_collapsed(ctx: &Context, stores: &Stores, prefs: &Prefs) -> bool {
    let p = prefs.get();
    p.ui.rail_collapsed || (!p.ui.rail_expanded && rail_folds(ctx, stores, prefs))
}

/// The collapse switch: on a narrow window it only says whether the player wants the
/// rail open there, so a wider window gets the rail back as it was.
pub fn toggle_rail(ctx: &Context, stores: &Stores, prefs: &mut Prefs) {
    let collapsed = rail_collapsed(ctx, stores, prefs);
    let folds = rail_folds(ctx, stores, prefs);
    let s = prefs.edit();
    if collapsed {
        s.ui.rail_collapsed = false;
        s.ui.rail_expanded = folds;
    } else if folds {
        s.ui.rail_expanded = false;
    } else {
        s.ui.rail_collapsed = true;
    }
}

/// A pad action by name, for the control channel and `ui_shot`: the action's own name
/// (`down`, `accept`, `prevTab`…) or the button's (`dpad_down`, `a`, `lb`, `rt`, `r3`…).
pub fn action_named(name: &str) -> Option<PadAction> {
    let n: String = name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect();
    Some(match n.as_str() {
        "up" | "dpadup" | "stickup" => PadAction::Up,
        "down" | "dpaddown" | "stickdown" => PadAction::Down,
        "left" | "dpadleft" | "stickleft" => PadAction::Left,
        "right" | "dpadright" | "stickright" => PadAction::Right,
        "accept" | "a" | "cross" => PadAction::Accept,
        "back" | "b" | "circle" => PadAction::Back,
        "primary" | "x" | "square" => PadAction::Primary,
        "secondary" | "y" | "triangle" => PadAction::Secondary,
        "menu" | "start" | "options" => PadAction::Menu,
        "view" | "select" | "share" => PadAction::View,
        "prevtab" | "lb" | "l1" => PadAction::PrevTab,
        "nexttab" | "rb" | "r1" => PadAction::NextTab,
        "pageup" | "lt" | "l2" => PadAction::PageUp,
        "pagedown" | "rt" | "r2" => PadAction::PageDown,
        "scrollup" | "rsup" => PadAction::ScrollUp,
        "scrolldown" | "rsdown" => PadAction::ScrollDown,
        "leftstick" | "l3" => PadAction::LeftStick,
        "rightstick" | "r3" => PadAction::RightStick,
        _ => return None,
    })
}

// ── the frame ──────────────────────────────────────────────────────────────

/// Each frame, before the window's keys and the views: the pad's actions, then the
/// landings that wait for a frame.
pub fn frame(cx: &mut Ctx) {
    let setting = cx.prefs.get().ui.pad_mode;
    // What the kit reads while drawing: hints follow the pad's focus (a disabled
    // control's too); the on-screen keyboard's typing reaches its field.
    let pad_now = cx.stores.pad.mode(setting) == InputMode::Gamepad;
    crate::kit::tooltip::set_pad_mode(cx.egui, pad_now);
    let resting = match cx.stores.pad.nav.focus {
        Focus::Widget(id) if pad_now => Some(id),
        _ => None,
    };
    crate::kit::tooltip::set_pad_focus(cx.egui, resting);
    let actions = std::mem::take(&mut cx.stores.pad.queue);
    // Never: pad input is ignored entirely.
    if setting == PadMode::Never {
        crate::kit::osk::begin_frame(cx.egui);
        cx.stores.pad.nav.was_pad = false;
        return;
    }
    // The game's hold reads this frame's keys first: a key typed here means the player
    // is back (keys only reach the window that has the focus).
    let keys = std::mem::take(&mut cx.stores.pad.nav.keys);
    let actions = if game_holds_pad(cx, !keys.is_empty()) {
        if !actions.is_empty() && !cx.stores.pad.nav.told {
            cx.stores.pad.nav.told = true;
            crate::kit::toast::say_topic(
                cx.egui,
                "pad.game",
                crate::i18n::t("pad", "gameHolds"),
                crate::kit::ToastTone::Neutral,
            );
        }
        Vec::new()
    } else {
        actions
    };
    for (a, repeat, by_keys) in actions
        .into_iter()
        .map(|(a, r)| (a, r, false))
        .chain(keys.into_iter().map(|(a, r)| (a, r, true)))
    {
        cx.stores.pad.used_pad();
        cx.stores.pad.nav.by_keys = by_keys;
        handle(cx, a, repeat);
    }
    // After the keys were read as pad actions: the keyboard's own arrows and erasing
    // reach its field as keys, which must not be read as the pad's in turn.
    crate::kit::osk::begin_frame(cx.egui);
    let pad = cx.stores.pad.mode(setting) == InputMode::Gamepad;
    if cx.stores.pad.nav.was_pad && !pad {
        // The pointer took over: a button keeping the keyboard focus would hold the
        // views' keys back. A text field keeps it (the player may be typing).
        if !cx.egui.text_edit_focused() {
            drop_focus(cx.egui);
        }
        cx.stores.pad.nav.focus = Focus::None;
        cx.stores.pad.nav.rest = None;
    }
    cx.stores.pad.nav.was_pad = pad;
    if !pad {
        let n = &mut cx.stores.pad.nav;
        n.settle = 0;
        n.await_popup = false;
        n.land_in = None;
        n.refocus = None;
        return;
    }
    waits(cx);
}

/// Whether the pad's presses are DayZ's now: from the game's start until the window has
/// been seen to lose the focus and get it back, or is used with the pointer or the keys.
fn game_holds_pad(cx: &mut Ctx, typed: bool) -> bool {
    let running = cx.stores.game.running;
    let (focused, used) = cx.egui.input(|i| {
        (
            i.viewport().focused.unwrap_or(i.focused),
            typed
                || i.pointer.any_pressed()
                || i.events
                    .iter()
                    .any(|e| matches!(e, Event::Key { pressed: true, .. } | Event::Text(_))),
        )
    });
    let n = &mut cx.stores.pad.nav;
    n.game_holds = game_step(
        GameWatch {
            running,
            was_running: n.game_was,
            holds: n.game_holds,
            went_away: &mut n.went_away,
        },
        focused,
        used,
    );
    n.game_was = running;
    if !n.game_holds {
        n.told = false;
    }
    n.game_holds
}

/// What [`game_holds_pad`] reads and keeps.
struct GameWatch<'a> {
    running: bool,
    was_running: bool,
    holds: bool,
    went_away: &'a mut bool,
}

/// One frame of [`game_holds_pad`], without a window (tested).
fn game_step(w: GameWatch<'_>, focused: bool, used: bool) -> bool {
    if !w.running {
        *w.went_away = false;
        return false;
    }
    let mut holds = w.holds;
    if !w.was_running {
        // The game just started: it has the pad until the player is back here.
        holds = true;
        *w.went_away = false;
    }
    if holds {
        if !focused {
            *w.went_away = true;
        } else if *w.went_away || used {
            holds = false;
        }
    }
    holds
}

/// The arrows, Enter and Escape as pad actions, taken out of the window's input before
/// egui reads it (it would move its own focus with the arrows and drop it on Escape):
/// while the pad drives the window (or on a Steam Deck, whose controller types them
/// without Steam), when the palette does not want them and no modifier is held. In a
/// text field only Escape is (B: done typing), unless the on-screen keyboard is up (it
/// takes them all). Called from the frame's raw input hook; [`frame`] handles them.
pub fn take_keys(cx: &mut Ctx, raw: &mut egui::RawInput) {
    let setting = cx.prefs.get().ui.pad_mode;
    let drives = cx.stores.pad.mode(setting) == InputMode::Gamepad || cx.stores.pad.steam_deck;
    if setting == PadMode::Never || !drives || palette(cx) {
        return;
    }
    let escape_only = cx.egui.text_edit_focused() && !crate::kit::osk::is_open(cx.egui);
    strip_keys(&mut raw.events, escape_only, &mut cx.stores.pad.nav.keys);
}

/// [`take_keys`]'s part on the events (tested): the keys that are pad actions leave
/// `events`, their presses join `keys` (16 a frame at most).
fn strip_keys(events: &mut Vec<Event>, escape_only: bool, keys: &mut Vec<(PadAction, bool)>) {
    events.retain(|e| match key_action(e, escape_only) {
        Some((a, repeat, pressed)) => {
            if pressed && keys.len() < 16 {
                keys.push((a, repeat));
            }
            false
        }
        None => true,
    });
}

/// The pad action a key event is (with its repeat and pressed flags), for
/// [`take_keys`]; `None` leaves it to the window.
fn key_action(e: &Event, escape_only: bool) -> Option<(PadAction, bool, bool)> {
    let Event::Key {
        key,
        pressed,
        repeat,
        modifiers,
        ..
    } = e
    else {
        return None;
    };
    if !modifiers.is_none() {
        return None;
    }
    let a = match key {
        Key::Escape => PadAction::Back,
        _ if escape_only => return None,
        Key::ArrowUp => PadAction::Up,
        Key::ArrowDown => PadAction::Down,
        Key::ArrowLeft => PadAction::Left,
        Key::ArrowRight => PadAction::Right,
        Key::Enter => PadAction::Accept,
        _ => return None,
    };
    Some((a, *repeat, *pressed))
}

/// The landings that wait for the next frames.
fn waits(cx: &mut Ctx) {
    let n = &mut cx.stores.pad.nav;
    if let Some((view, left, pass)) = n.land_in {
        if view != cx.nav.view || left == 0 {
            n.land_in = None;
        } else if cx.egui.cumulative_pass_nr() <= pass {
            cx.egui.request_repaint();
        } else {
            n.land_in = Some((view, left - 1, pass));
            let snap = Snap::take(cx);
            if !snap.overlay && snap.cands.iter().any(|c| c.visible() && snap.in_main(c)) {
                cx.stores.pad.nav.land_in = None;
                land(cx, &snap);
            } else {
                cx.egui.request_repaint();
            }
        }
    }
    let n = &mut cx.stores.pad.nav;
    if n.settle > 0 {
        n.settle -= 1;
        cx.egui.request_repaint();
        let snap = Snap::take(cx);
        // A popup's first frame only measures it: nothing to land on yet. After a view's
        // command that may open one, the landing waits for it (to the last frame).
        let n = &mut cx.stores.pad.nav;
        let wait = n.await_popup && !snap.overlay && n.settle > 0;
        if !wait && current(cx, &snap).is_none() && land(cx, &snap) {
            cx.stores.pad.nav.settle = 0;
        }
        if cx.stores.pad.nav.settle == 0 || snap.overlay {
            cx.stores.pad.nav.await_popup = false;
        }
    }
    let n = &mut cx.stores.pad.nav;
    if let Some((y, left)) = n.refocus {
        // The scroll animates: follow as soon as the focus is out of sight.
        if left == 0 || follow_scroll(cx, y) {
            cx.stores.pad.nav.refocus = None;
        } else {
            cx.stores.pad.nav.refocus = Some((y, left - 1));
            cx.egui.request_repaint();
        }
    }
}

/// One action from a pad.
pub fn handle(cx: &mut Ctx, action: PadAction, repeat: bool) {
    let n = &mut cx.stores.pad.nav;
    n.streak = if repeat && n.last == Some(action) {
        n.streak.saturating_add(1)
    } else {
        0
    };
    n.last = Some(action);
    cx.egui.request_repaint();
    // The on-screen keyboard, while it is up, takes every button.
    if crate::kit::osk::is_open(cx.egui) {
        if let Some(p) = osk_press(action) {
            crate::kit::osk::press(cx.egui, p);
        }
        return;
    }
    match action {
        PadAction::Up => move_focus(cx, Dir::Up),
        PadAction::Down => move_focus(cx, Dir::Down),
        PadAction::Left => move_focus(cx, Dir::Left),
        PadAction::Right => move_focus(cx, Dir::Right),
        PadAction::Accept => accept(cx),
        PadAction::Back => back(cx),
        PadAction::PrevTab => switch_view(cx, -1),
        PadAction::NextTab => switch_view(cx, 1),
        PadAction::PageUp => page(cx, -1),
        PadAction::PageDown => page(cx, 1),
        PadAction::ScrollUp => scroll(cx, -1),
        PadAction::ScrollDown => scroll(cx, 1),
        PadAction::Primary => command(cx, PadButton::X),
        PadAction::Secondary => command(cx, PadButton::Y),
        PadAction::Menu => command(cx, PadButton::Menu),
        PadAction::View => command(cx, PadButton::View),
        PadAction::LeftStick => command(cx, PadButton::L3),
        PadAction::RightStick => command(cx, PadButton::R3),
    }
}

// ── what the last frame drew ───────────────────────────────────────────────

/// A place the pad can go: a widget, or a list as one stop.
#[derive(Clone, Copy, Debug)]
struct Cand {
    id: Id,
    /// The part on screen (empty when scrolled out of sight).
    rect: Rect,
    /// The whole of it.
    full: Rect,
    layer: LayerId,
    list: bool,
    enabled: bool,
}

impl Cand {
    fn visible(&self) -> bool {
        self.rect.width() >= 1.0 && self.rect.height() >= 1.0
    }

    fn fully_visible(&self) -> bool {
        self.visible()
            && (self.rect.height() + 0.5 >= self.full.height())
            && (self.rect.width() + 0.5 >= self.full.width())
    }

    /// The rect to measure from or to.
    fn shown(&self) -> Rect {
        if self.visible() { self.rect } else { self.full }
    }
}

/// The scope a press acts in, and what is in it.
struct Snap {
    /// The scope is a dialog, a popup, the palette or the setup wizard.
    overlay: bool,
    /// The scope is a dialog with a veil (`egui::Modal`), not a popup.
    modal: bool,
    /// The first two regions are the title bar and the status bar.
    bars: bool,
    cands: Vec<Cand>,
    /// Every clickable widget of the overlay, enabled or not (its footer).
    overlay_all: Vec<Cand>,
    lists: Vec<ListSlot>,
    regions: Vec<Rect>,
    main: Rect,
    reg: Frame,
}

impl Snap {
    fn take(cx: &Ctx) -> Self {
        let ctx = cx.egui;
        let reg = registry::last(ctx);
        let screen = ctx.content_rect();
        let layers: Vec<(LayerId, Vec<egui::WidgetRect>)> = ctx.viewport(|v| {
            v.prev_pass
                .widgets
                .layers()
                .map(|(l, ws)| (*l, ws.to_vec()))
                .collect()
        });
        let (order, visible, allowed): (Vec<LayerId>, Vec<LayerId>, Vec<LayerId>) =
            ctx.memory(|m| {
                let order: Vec<LayerId> = m.layer_ids().collect();
                let vis = m.areas().visible_layer_ids();
                let mut v: Vec<LayerId> = Vec::new();
                let mut a: Vec<LayerId> = Vec::new();
                for (l, _) in &layers {
                    if vis.contains(l) || l.order == Order::Background {
                        v.push(*l);
                    }
                    if m.allows_interaction(*l) {
                        a.push(*l);
                    }
                }
                (order, v, a)
            });
        let clicky = |l: &LayerId| {
            layers
                .iter()
                .find(|(x, _)| x == l)
                .is_some_and(|(_, ws)| ws.iter().any(|w| w.sense.senses_click()))
        };
        // The topmost dialog or popup: a layer above the window's with something to click.
        let top = order
            .iter()
            .rev()
            .find(|l| {
                matches!(l.order, Order::Middle | Order::Foreground)
                    && visible.contains(l)
                    && allowed.contains(l)
                    && clicky(l)
            })
            .copied();
        let in_scope = |l: &LayerId| match top {
            Some(t) => *l == t,
            None => {
                !matches!(
                    l.order,
                    Order::Middle | Order::Foreground | Order::Tooltip | Order::Debug
                ) && allowed.contains(l)
            }
        };
        let lists: Vec<ListSlot> = reg
            .lists
            .iter()
            .filter(|s| in_scope(&s.layer) && s.count > 0)
            .cloned()
            .collect();
        let veil =
            |r: Rect| r.width() >= screen.width() * 0.9 && r.height() >= screen.height() * 0.6;
        let mut cands = Vec::new();
        let mut overlay_all = Vec::new();
        let modal = top.is_some_and(|t| {
            layers
                .iter()
                .any(|(l, ws)| *l == t && ws.iter().any(|w| w.sense.senses_click() && veil(w.rect)))
        });
        for (layer, ws) in &layers {
            if !in_scope(layer) {
                continue;
            }
            // Headings drawn over the rows scrolled under them: those rows are out of
            // sight where they are covered, and a heading is no stop itself.
            let stickies: Vec<(Id, Rect)> = reg
                .stickies
                .iter()
                .filter(|c| c.layer == *layer)
                .map(|c| (c.id, c.rect))
                .collect();
            let covers: Vec<(usize, Rect)> = reg
                .covers
                .iter()
                .filter(|c| c.layer == *layer)
                .filter_map(|c| ws.iter().position(|w| w.id == c.id).map(|i| (i, c.rect)))
                .collect();
            for (i, w) in ws.iter().enumerate() {
                // Clickable things, not the background an area senses for itself; and
                // the kit's disabled buttons, which sense hover only.
                let clickable = w.sense.is_focusable() && w.sense.senses_click();
                // (A kit button disables itself, not its `Ui`: egui still calls it enabled.)
                let dormant = reg.dormant.contains(&w.id) || reg.tipped.contains(&w.id);
                let disabled_button = dormant && !w.sense.senses_click();
                if !(clickable || disabled_button) || w.id == layer.id.with("move") {
                    continue;
                }
                if !w.rect.is_finite() || !w.interact_rect.is_finite() || veil(w.rect) {
                    continue;
                }
                let center = w.rect.center();
                if covers
                    .iter()
                    .any(|(ci, r)| *ci == i || (i < *ci && r.contains(center)))
                    || stickies.iter().any(|(id, _)| *id == w.id)
                {
                    continue;
                }
                let in_list = lists.iter().any(|s| {
                    s.layer == *layer
                        && (s.viewport.contains(center)
                            || (w.interact_rect.area() <= 0.0
                                && s.viewport.x_range().contains(center.x)))
                });
                if in_list {
                    continue;
                }
                let shown = under_stickies(w.interact_rect, &stickies);
                let c = Cand {
                    id: w.id,
                    rect: shown,
                    full: w.rect,
                    layer: *layer,
                    list: false,
                    enabled: w.enabled && !disabled_button,
                };
                if top.is_some() && c.visible() {
                    overlay_all.push(c);
                }
                // A disabled control carrying a hint stays a stop (its hint says why).
                if c.enabled || reg.tipped.contains(&w.id) {
                    cands.push(c);
                }
            }
        }
        for s in &lists {
            let mut shown = s.viewport;
            for c in reg.covers.iter().filter(|c| c.layer == s.layer) {
                if c.rect.intersects(shown) && c.rect.right() >= shown.right() {
                    shown.max.x = shown.max.x.min(c.rect.left());
                }
            }
            cands.push(Cand {
                id: s.id,
                rect: shown,
                full: s.viewport,
                layer: s.layer,
                list: true,
                enabled: true,
            });
        }
        let rail_w = if rail_collapsed(ctx, cx.stores, cx.prefs) {
            size::RAIL_COLLAPSED
        } else {
            size::RAIL
        };
        let top_y = screen.top() + size::TITLEBAR;
        let bottom_y = screen.bottom() - crate::shell::STATUSBAR_HEIGHT;
        let titlebar = Rect::from_min_max(screen.min, egui::pos2(screen.right(), top_y));
        let statusbar = Rect::from_min_max(egui::pos2(screen.left(), bottom_y), screen.max);
        let rail = Rect::from_min_max(
            egui::pos2(screen.left(), top_y),
            egui::pos2(screen.left() + rail_w, bottom_y),
        );
        // The pad hints' panel takes the foot of the main area (beside the rail).
        let main_bottom = if crate::shell::pad_hints_shown(cx) {
            bottom_y - crate::shell::PAD_HINTS_HEIGHT
        } else {
            bottom_y
        };
        let main = Rect::from_min_max(
            egui::pos2(rail.right(), top_y),
            egui::pos2(screen.right(), main_bottom),
        );
        let mut regions = vec![titlebar, statusbar, rail, main];
        regions.extend(reg.regions.iter().copied());
        Self {
            overlay: top.is_some(),
            modal,
            bars: true,
            cands,
            overlay_all,
            lists,
            regions,
            main,
            reg,
        }
    }

    /// The smallest region holding `p`.
    fn region_of(&self, p: egui::Pos2) -> Option<usize> {
        if self.overlay {
            return Some(usize::MAX);
        }
        self.regions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.contains(p))
            .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
            .map(|(i, _)| i)
    }

    fn in_main(&self, c: &Cand) -> bool {
        self.main.contains(c.shown().center())
    }

    fn list(&self, id: Id) -> Option<&ListSlot> {
        self.lists.iter().find(|s| s.id == id)
    }

    /// The list of the view on screen (in the main area).
    fn view_list(&self) -> Option<&ListSlot> {
        self.lists
            .iter()
            .find(|s| !self.overlay && self.main.contains(s.viewport.center()))
    }

    fn cand(&self, id: Id) -> Option<Cand> {
        self.cands.iter().find(|c| c.id == id).copied()
    }
}

/// What of a widget's on-screen part `r` shows past the sticky headings drawn over it
/// (`Rect::NOTHING` when it is all under one). The heading's own rect is skipped.
fn under_stickies(r: Rect, stickies: &[(Id, Rect)]) -> Rect {
    let mut shown = r;
    for (_, s) in stickies {
        if !s.intersects(shown) {
            continue;
        }
        if s.top() <= shown.top() && s.bottom() >= shown.bottom() {
            return Rect::NOTHING;
        }
        if s.top() <= shown.top() {
            shown.min.y = s.bottom();
        } else if s.bottom() >= shown.bottom() {
            shown.max.y = s.top();
        }
    }
    shown
}

/// Where the pad is now: the focused widget when it is in the scope, else the list it
/// walks.
fn current(cx: &Ctx, snap: &Snap) -> Option<Cand> {
    if let Some(id) = cx.egui.memory(|m| m.focused())
        && let Some(c) = snap.cand(id).filter(|c| !c.list)
    {
        return Some(c);
    }
    match cx.stores.pad.nav.focus {
        Focus::List(id) => snap.cand(id).filter(|c| c.list),
        // A disabled control egui gives no focus to: the pad rests on it.
        Focus::Widget(id) if snap.reg.tipped.contains(&id) => {
            snap.cand(id).filter(|c| !c.list && !c.enabled)
        }
        // A text field it rests on without typing.
        Focus::Widget(id) if cx.stores.pad.nav.rest == Some(id) => {
            snap.cand(id).filter(|c| !c.list)
        }
        _ => None,
    }
}

// ── synthetic input ────────────────────────────────────────────────────────

fn key(ctx: &Context, k: Key, shift: bool) {
    let modifiers = if shift {
        Modifiers::SHIFT
    } else {
        Modifiers::NONE
    };
    ctx.input_mut(|i| {
        for pressed in [true, false] {
            i.events.push(Event::Key {
                key: k,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            });
        }
    });
}

/// An AccessKit request aimed at a widget, taken when the widget is drawn this frame.
fn ak(ctx: &Context, id: Id, action: Action) {
    ctx.input_mut(|i| {
        i.events
            .push(Event::AccessKitActionRequest(accesskit::ActionRequest {
                action,
                target_tree: accesskit::TreeId::ROOT,
                target_node: id.accesskit_id(),
                data: None,
            }));
    });
}

fn drop_focus(ctx: &Context) {
    ctx.memory_mut(|m| {
        if let Some(f) = m.focused() {
            m.surrender_focus(f);
        }
    });
}

fn is_text(ctx: &Context, id: Id) -> bool {
    egui::text_edit::TextEditState::load(ctx, id).is_some()
}

// ── focus ──────────────────────────────────────────────────────────────────

fn focus_widget(cx: &mut Ctx, snap: &Snap, c: &Cand) {
    // Moved by the keys onto a text field: it would take the arrows, so the pad rests on
    // it and A types (a pad has no arrows to lose).
    let rest = cx.stores.pad.nav.by_keys && is_text(cx.egui, c.id);
    if rest {
        drop_focus(cx.egui);
    } else {
        cx.egui.memory_mut(|m| m.request_focus(c.id));
    }
    if !c.fully_visible() {
        ak(cx.egui, c.id, Action::ScrollIntoView);
    }
    let n = &mut cx.stores.pad.nav;
    n.rest = rest.then_some(c.id);
    n.focus = Focus::Widget(c.id);
    if !snap.overlay {
        n.remembered.insert(cx.nav.view, c.id);
    }
}

/// The list kinds whose rows the views' own commands walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListKind {
    /// `list.*`: the browser, favourites, history.
    Servers,
    /// `mods.*`.
    Mods,
}

fn list_kind(v: ViewId) -> Option<ListKind> {
    match v {
        ViewId::Servers | ViewId::Favorites | ViewId::History => Some(ListKind::Servers),
        ViewId::Mods => Some(ListKind::Mods),
        _ => None,
    }
}

/// Move the list's cursor by `delta` rows (0 brings it into view).
fn list_move(cx: &mut Ctx, delta: i64) {
    match list_kind(cx.nav.view) {
        Some(ListKind::Servers) => {
            let _ = cx.run("list.move", &json!({ "delta": delta }));
        }
        Some(ListKind::Mods) => {
            let dir = if delta < 0 { "up" } else { "down" };
            let n = delta.unsigned_abs().clamp(1, 200);
            for _ in 0..n {
                let _ = cx.run("mods.move", &json!({ "dir": dir }));
            }
        }
        None => {}
    }
}

fn enter_list(cx: &mut Ctx, s: &ListSlot) {
    drop_focus(cx.egui);
    cx.stores.pad.nav.focus = Focus::List(s.id);
    cx.stores.pad.nav.rest = None;
    match s.cursor {
        // Entering lands on the first row on screen.
        None => match list_kind(cx.nav.view) {
            Some(ListKind::Servers) => {
                let d = i64::try_from(s.first_visible)
                    .unwrap_or(0)
                    .saturating_add(1);
                list_move(cx, d);
            }
            Some(ListKind::Mods) => list_move(cx, 1),
            None => {}
        },
        // Its row, brought back into view.
        Some(_) if s.cursor_rect.is_none() && list_kind(cx.nav.view) == Some(ListKind::Servers) => {
            list_move(cx, 0);
        }
        Some(_) => {}
    }
}

fn focus(cx: &mut Ctx, snap: &Snap, c: &Cand) {
    if c.list {
        if let Some(s) = snap.list(c.id).cloned() {
            enter_list(cx, &s);
        }
    } else {
        focus_widget(cx, snap, c);
    }
}

/// Put the focus somewhere when nothing in the scope has it: what had it last in this
/// view, else where the view or the dialog says (`registry::primary`), else the view's
/// list at its row, else a dialog's safe button, else the first thing in reading order
/// (never a dialog's close cross).
fn land(cx: &mut Ctx, snap: &Snap) -> bool {
    if !snap.overlay {
        let mem = cx.stores.pad.nav.remembered.get(&cx.nav.view).copied();
        if let Some(c) = mem.and_then(|id| snap.cand(id)).filter(Cand::visible) {
            focus_widget(cx, snap, &c);
            return true;
        }
    }
    if let Some(c) = primary_of(snap) {
        focus(cx, snap, &c);
        return true;
    }
    if !snap.overlay
        && let Some(s) = snap.view_list().cloned()
    {
        enter_list(cx, &s);
        return true;
    }
    if snap.modal
        && let Some(c) = dialog_safe(snap)
    {
        focus_widget(cx, snap, &c);
        return true;
    }
    let cross = if snap.modal { close_cross(snap) } else { None };
    let pick = |within: &dyn Fn(&Cand) -> bool| {
        snap.cands
            .iter()
            .filter(|c| c.visible() && Some(c.id) != cross && within(c))
            .min_by(|a, b| {
                let (x, y) = (a.shown(), b.shown());
                if (x.top() - y.top()).abs() > 8.0 {
                    x.top().total_cmp(&y.top())
                } else {
                    x.left().total_cmp(&y.left())
                }
            })
            .copied()
    };
    let first = if snap.overlay {
        pick(&|_| true)
    } else {
        pick(&|c| snap.in_main(c)).or_else(|| pick(&|_| true))
    };
    match first {
        Some(c) => {
            focus(cx, snap, &c);
            true
        }
        None => false,
    }
}

/// The first primary the scope's view or dialog registered that is on screen in it.
fn primary_of(snap: &Snap) -> Option<Cand> {
    snap.reg
        .primaries
        .iter()
        .filter_map(|id| snap.cand(*id))
        .find(|c| c.visible() && (snap.overlay || snap.in_main(c)))
}

/// A dialog's close cross: the one `kit::Dialog` registered, else a small square button
/// on its top row, at the right end.
fn close_cross(snap: &Snap) -> Option<Id> {
    if let Some(c) = snap
        .overlay_all
        .iter()
        .find(|c| snap.reg.crosses.contains(&c.id))
    {
        return Some(c.id);
    }
    let top = snap
        .overlay_all
        .iter()
        .map(|c| c.rect.top())
        .min_by(f32::total_cmp)?;
    snap.overlay_all
        .iter()
        .filter(|c| {
            let r = c.rect;
            (r.top() - top).abs() < 8.0
                && (r.width() - r.height()).abs() < 4.0
                && r.width() <= size::CONTROL + 4.0
        })
        .max_by(|a, b| a.rect.right().total_cmp(&b.rect.right()))
        .filter(|_| snap.overlay_all.len() > 1)
        .map(|c| c.id)
}

/// The best place `dir` leads to from `from` (in its region first).
fn pick(snap: &Snap, from: &Cand, from_rect: Rect, dir: Dir) -> Option<Cand> {
    let home = snap.region_of(from_rect.center());
    let home_rect = home.and_then(|i| snap.regions.get(i)).copied();
    // A widget out of sight belongs to the focus's region when it lies beyond that
    // region's visible edge, in line with it (a field below the fold of its pane).
    let beyond_home = |c: &Cand| match home_rect {
        _ if snap.overlay => true,
        Some(h) => {
            let p = c.full.center();
            h.x_range().contains(p.x)
                && snap.region_of(egui::pos2(p.x, p.y.clamp(h.top() + 1.0, h.bottom() - 1.0)))
                    == home
        }
        None => false,
    };
    // The title bar and the status bar (regions 0 and 1) are reached up and down only:
    // Right from a list's row goes to the pane beside it, or nowhere.
    let bar = |i: Option<usize>| snap.bars && !snap.overlay && matches!(i, Some(0 | 1));
    let sideways_from_page = !dir.vertical() && !bar(home);
    let mut best: Option<(f32, Cand)> = None;
    for c in &snap.cands {
        if c.id == from.id || (!from.list && from.full.contains_rect(c.full)) {
            continue;
        }
        let to = c.shown();
        if sideways_from_page && bar(snap.region_of(to.center())) {
            continue;
        }
        // Sideways from a list's row: only what lies beside the list (a pane, a drawer),
        // not the filters above it.
        if from.list && !dir.vertical() && !from.full.y_range().intersects(to.y_range()) {
            continue;
        }
        let Some(k) = spatial::cost(from_rect, to, dir) else {
            continue;
        };
        // Out of sight only within the region the focus is in (the next field below
        // the fold of its pane), never behind something.
        let same = if c.visible() {
            snap.region_of(to.center()) == home
        } else if c.layer == from.layer && beyond_home(c) {
            true
        } else {
            continue;
        };
        let away = if same {
            0.0
        } else if dir.vertical() {
            AWAY
        } else {
            ELSEWHERE
        };
        let k = k + away;
        if best.as_ref().is_none_or(|(b, _)| k < *b) {
            best = Some((k, *c));
        }
    }
    best.map(|(_, c)| c)
}

// ── the actions ────────────────────────────────────────────────────────────

fn palette(cx: &Ctx) -> bool {
    cx.stores.dialogs.palette
}

fn move_focus(cx: &mut Ctx, dir: Dir) {
    if palette(cx) {
        let k = match dir {
            Dir::Up => Key::ArrowUp,
            Dir::Down => Key::ArrowDown,
            Dir::Left => Key::ArrowLeft,
            Dir::Right => Key::ArrowRight,
        };
        key(cx.egui, k, false);
        return;
    }
    let snap = Snap::take(cx);
    let Some(from) = current(cx, &snap) else {
        land(cx, &snap);
        return;
    };
    let mut from_rect = from.shown();
    if from.list
        && let Some(s) = snap.list(from.id).cloned()
    {
        if let Some(r) = s.cursor_rect {
            // Only the part of the row that shows: a drawer over the list's side is
            // where Right goes from it.
            let covers: Vec<Rect> = snap
                .reg
                .covers
                .iter()
                .filter(|c| c.layer == s.layer)
                .map(|c| c.rect)
                .collect();
            let shown = uncovered(r, &covers);
            from_rect = if shown.is_positive() { shown } else { r };
        }
        // Up and Down inside a list go through its rows, drawn or not.
        let stride = spatial::stride(cx.stores.pad.nav.streak);
        match dir {
            Dir::Down => {
                if s.cursor.is_none() {
                    enter_list(cx, &s);
                } else {
                    list_move(cx, stride);
                }
                return;
            }
            Dir::Up if s.cursor.is_none() => {
                enter_list(cx, &s);
                return;
            }
            Dir::Up if !s.at_top => {
                list_move(cx, -stride);
                return;
            }
            // Up from the first row leaves by the spatial rules, from its row.
            _ => {}
        }
    } else if !dir.vertical() && snap.reg.sliders.contains(&from.id) {
        // A slider takes Left and Right itself.
        key(
            cx.egui,
            if dir == Dir::Left {
                Key::ArrowLeft
            } else {
                Key::ArrowRight
            },
            false,
        );
        return;
    } else if dir.vertical() && snap.reg.vsliders.contains(&from.id) {
        // A stacked split's grabbed handle takes Up and Down.
        key(
            cx.egui,
            if dir == Dir::Up {
                Key::ArrowUp
            } else {
                Key::ArrowDown
            },
            false,
        );
        return;
    } else if !dir.vertical() && snap.reg.numbers.contains(&from.id) {
        // A number field: Left and Right step it (its Down and Up keys).
        let k = if dir == Dir::Left {
            Key::ArrowDown
        } else {
            Key::ArrowUp
        };
        key(cx.egui, k, false);
        return;
    }
    if let Some(to) = pick(&snap, &from, from_rect, dir) {
        focus(cx, &snap, &to);
    }
}

/// The on-screen keyboard's part of a pad button, while it is up.
fn osk_press(a: PadAction) -> Option<Press> {
    Some(match a {
        PadAction::Up => Press::Up,
        PadAction::Down => Press::Down,
        PadAction::Left => Press::Left,
        PadAction::Right => Press::Right,
        PadAction::Accept => Press::Type,
        PadAction::Back => Press::Close,
        PadAction::Primary => Press::Backspace,
        PadAction::Secondary => Press::Shift,
        PadAction::PrevTab => Press::CaretLeft,
        PadAction::NextTab => Press::CaretRight,
        PadAction::Menu => Press::Done,
        _ => return None,
    })
}

/// Which keyboard a text field picked by the pad brings up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Keyboard {
    Steam,
    Builtin,
    None,
}

/// The setting's keyboard where the window runs (tested): Auto takes Steam's inside its
/// big-screen interface, where it surely answers, and the built-in one elsewhere; Steam
/// takes Steam's whenever Steam started the window or runs the Deck (its desktop
/// keyboard answers `steam://open/keyboard` too), the built-in one without Steam.
fn keyboard(choice: OskChoice, steam_ui: bool, under_steam: bool) -> Keyboard {
    match choice {
        OskChoice::None => Keyboard::None,
        OskChoice::Builtin => Keyboard::Builtin,
        OskChoice::Auto if steam_ui => Keyboard::Steam,
        OskChoice::Steam if steam_ui || under_steam => Keyboard::Steam,
        OskChoice::Auto | OskChoice::Steam => Keyboard::Builtin,
    }
}

/// Steam started the window (a game or a shortcut of its library): it sets these.
fn started_by_steam() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        ["SteamGameId", "SteamAppId", "SteamOverlayGameId"]
            .iter()
            .any(|k| std::env::var_os(k).is_some_and(|v| !v.is_empty()))
    })
}

fn keyboard_here(cx: &Ctx) -> Keyboard {
    let pad = &cx.stores.pad;
    keyboard(
        cx.prefs.get().ui.pad_keyboard,
        pad.steam_ui,
        pad.steam_deck || started_by_steam(),
    )
}

/// Whether a text field picked by the pad brings a keyboard up (the setting is not None).
pub fn has_keyboard(cx: &Ctx) -> bool {
    keyboard_here(cx) != Keyboard::None
}

/// Bring a keyboard up over the text field `id` (it has the focus): Steam's, or the
/// window's own on the language's letters, or a number field's digits.
fn open_keyboard(cx: &mut Ctx, id: Id, number: bool) {
    match keyboard_here(cx) {
        Keyboard::Steam => {
            let _ = crate::platform::deck::detached("steam://open/keyboard");
        }
        Keyboard::Builtin => {
            let layout = if number {
                OskLayout::Digits
            } else {
                OskLayout::for_locale(crate::i18n::locale().code())
            };
            crate::kit::osk::open(cx.egui, id, layout);
        }
        Keyboard::None => {}
    }
}

fn accept(cx: &mut Ctx) {
    if palette(cx) {
        key(cx.egui, Key::Enter, false);
        return;
    }
    let snap = Snap::take(cx);
    let Some(c) = current(cx, &snap) else {
        land(cx, &snap);
        return;
    };
    if c.list {
        let cursor = snap.list(c.id).and_then(|s| s.cursor);
        match (cursor, list_kind(cx.nav.view)) {
            (None, _) => {
                if let Some(s) = snap.list(c.id).cloned() {
                    enter_list(cx, &s);
                }
            }
            // A on the server under the cursor joins it (the row is selected already).
            (Some(_), Some(ListKind::Servers)) => {
                let _ = cx.run("list.join", &Value::Null);
            }
            (Some(_), _) => {}
        }
    } else if is_text(cx.egui, c.id) {
        cx.stores.pad.nav.rest = None;
        cx.egui.memory_mut(|m| m.request_focus(c.id));
        // A pad has no keys: a keyboard comes up over the window (the setting's).
        let number = snap.reg.numbers.contains(&c.id) || snap.reg.digits.contains(&c.id);
        open_keyboard(cx, c.id, number);
    } else {
        ak(cx.egui, c.id, Action::Click);
    }
    // A click can open a popup or a dialog, or redraw the focus away: put it back.
    cx.stores.pad.nav.settle = 4;
}

fn back(cx: &mut Ctx) {
    if palette(cx) {
        key(cx.egui, Key::Escape, false);
        return;
    }
    let snap = Snap::take(cx);
    let cur = current(cx, &snap);
    // A grabbed handle: let it go, the focus stays on it.
    if cur.is_some_and(|c| snap.reg.held.contains(&c.id)) {
        key(cx.egui, Key::Escape, false);
        return;
    }
    // A text field being typed in: done typing, in a dialog too (the dialog stays). The
    // focus goes to what is below it (a search's results, the dialog's next field or its
    // footer); with the keys, or with nothing below, the pad rests on the field (A types
    // again, the next B goes on: a dialog or a popup closes).
    let typing =
        |c: &Cand| !c.list && is_text(cx.egui, c.id) && cx.egui.memory(|m| m.has_focus(c.id));
    if let Some(c) = cur.filter(typing) {
        let below = if cx.stores.pad.nav.by_keys {
            None
        } else {
            pick(&snap, &c, c.shown(), Dir::Down)
        };
        drop_focus(cx.egui);
        match below {
            Some(to) => focus(cx, &snap, &to),
            None => {
                let n = &mut cx.stores.pad.nav;
                n.rest = Some(c.id);
                n.focus = Focus::Widget(c.id);
            }
        }
        return;
    }
    if snap.overlay {
        key(cx.egui, Key::Escape, false);
        cx.stores.pad.nav.settle = 4;
        return;
    }
    // The view's own Escape: the lists clear their selection, the details close. The
    // views read keys only while nothing is focused; the focus comes back at the end.
    if let Some(c) = cur.filter(|c| !c.list) {
        drop_focus(cx.egui);
        // A text field the pad rests on stays without egui's focus (it would take the
        // keys' arrows).
        if cx.stores.pad.nav.rest != Some(c.id) {
            cx.stores.pad.nav.restore = Some(c.id);
        }
    }
    key(cx.egui, Key::Escape, false);
    cx.stores.pad.nav.settle = 4;
}

fn switch_view(cx: &mut Ctx, delta: i32) {
    if palette(cx) {
        // The palette's sections, with Tab and Shift+Tab.
        key(cx.egui, Key::Tab, delta < 0);
        return;
    }
    let snap = Snap::take(cx);
    if snap.overlay {
        return;
    }
    let next = cx.nav.view.step(delta);
    let id = if delta < 0 { "view.prev" } else { "view.next" };
    if cx.run(id, &Value::Null).is_err() {
        return;
    }
    drop_focus(cx.egui);
    let n = &mut cx.stores.pad.nav;
    n.focus = Focus::None;
    n.land_in = Some((next, WAIT_FRAMES, cx.egui.cumulative_pass_nr()));
}

/// Where the pad's focus is, for what it is in: a list's cursor row, else the widget.
fn focus_point(snap: &Snap, c: &Cand) -> egui::Pos2 {
    if c.list
        && let Some(r) = snap.list(c.id).and_then(|s| s.cursor_rect)
    {
        return r.center();
    }
    c.shown().center()
}

/// The strip of tabs heading the pane `c` is in: the one in the smallest part of the
/// window that holds both (in a dialog, its own).
fn pane_tabs<'a>(snap: &'a Snap, c: &Cand) -> Option<&'a registry::Tabs> {
    let at = focus_point(snap, c);
    let home = snap.region_of(at);
    snap.reg
        .tabs
        .iter()
        .filter(|t| t.layer == c.layer && snap.region_of(t.strip.center()) == home)
        .min_by(|a, b| {
            let d = |t: &registry::Tabs| (t.strip.center().y - at.y).abs();
            d(a).total_cmp(&d(b))
        })
}

/// The tab LT (`delta` < 0) or RT presses from where `c` is, going round.
fn tab_target(snap: &Snap, c: &Cand, delta: i8) -> Option<Id> {
    let t = pane_tabs(snap, c)?;
    let n = t.count;
    if n < 2 {
        return None;
    }
    let cur = t.current.unwrap_or(0);
    let next = if delta < 0 {
        cur.checked_sub(1).unwrap_or(n - 1)
    } else {
        (cur + 1) % n
    };
    Some(crate::kit::tabs::tab_id(t.base, next))
}

/// How many AccessKit scroll requests make most of a page of `h` points.
fn page_steps(h: f32) -> usize {
    let n = (h * 0.85 / AK_SCROLL).round();
    if n.is_finite() {
        (n as usize).clamp(1, 40)
    } else {
        1
    }
}

fn page(cx: &mut Ctx, delta: i8) {
    if palette(cx) {
        key(
            cx.egui,
            if delta > 0 {
                Key::PageDown
            } else {
                Key::PageUp
            },
            false,
        );
        return;
    }
    let snap = Snap::take(cx);
    let cur = current(cx, &snap);
    // Tabs heading the pane the focus is in: the tab before or after.
    if let Some(c) = cur
        && let Some(id) = tab_target(&snap, &c, delta)
    {
        ak(cx.egui, id, Action::Click);
        return;
    }
    // The list the focus is in; with nothing focused, the view's own list.
    let list = match cur {
        Some(c) if c.list => snap.list(c.id).cloned(),
        None if !snap.overlay => snap.view_list().cloned(),
        _ => None,
    };
    if let Some(s) = list {
        if cur.is_none() {
            cx.stores.pad.nav.focus = Focus::List(s.id);
        }
        match list_kind(cx.nav.view) {
            Some(ListKind::Servers) => {
                let _ = cx.run("list.page", &json!({ "down": delta > 0 }));
            }
            Some(ListKind::Mods) => {
                let rows = (s.viewport.height() / s.row_height.max(1.0)).floor() as i64;
                list_move(cx, i64::from(delta) * rows.saturating_sub(1).max(1));
            }
            None => {}
        }
        return;
    }
    // Scroll what holds the focus (or the view's main scroller) by most of a page.
    let target = cur.or_else(|| {
        snap.cands
            .iter()
            .find(|c| c.visible() && (snap.overlay || snap.in_main(c)))
            .copied()
    });
    let Some(t) = target else {
        return;
    };
    let h = if snap.overlay {
        cx.egui.content_rect().height() * 0.6
    } else {
        snap.main.height()
    };
    let action = if delta > 0 {
        Action::ScrollDown
    } else {
        Action::ScrollUp
    };
    for _ in 0..page_steps(h) {
        ak(cx.egui, t.id, action);
    }
    // The focus follows what is on screen, so the next move starts from there.
    if cur.is_some() {
        cx.stores.pad.nav.refocus = Some((t.shown().center().y, WAIT_FRAMES));
    }
}

/// After a page scroll: a focus scrolled out of sight goes to what is nearest its
/// height on screen, in its region. True when there is nothing left to follow.
fn follow_scroll(cx: &mut Ctx, y: f32) -> bool {
    let snap = Snap::take(cx);
    let Some(c) = current(cx, &snap) else {
        return true;
    };
    if c.list {
        return true;
    }
    if c.visible() {
        return false;
    }
    let home = snap.region_of(egui::pos2(c.full.center().x, y));
    let near = snap
        .cands
        .iter()
        .filter(|o| {
            o.fully_visible() && !o.list && o.id != c.id && snap.region_of(o.rect.center()) == home
        })
        .min_by(|a, b| {
            (a.rect.center().y - y)
                .abs()
                .total_cmp(&(b.rect.center().y - y).abs())
        })
        .copied();
    if let Some(o) = near {
        focus_widget(cx, &snap, &o);
    }
    true
}

/// The right stick: scroll what is being read (a reading pane), else the scroller
/// around the focus, else the view's main one.
fn scroll(cx: &mut Ctx, dir: i8) {
    let snap = Snap::take(cx);
    if snap.overlay {
        return;
    }
    let action = if dir > 0 {
        Action::ScrollDown
    } else {
        Action::ScrollUp
    };
    let pane = snap
        .reg
        .anchors
        .iter()
        .find(|(_, r)| r.intersects(snap.main) && r.area() > 0.0)
        .map(|(id, _)| *id);
    let target = pane.or_else(|| {
        current(cx, &snap)
            .filter(|c| !c.list)
            .or_else(|| {
                snap.cands
                    .iter()
                    .find(|c| c.visible() && !c.list && snap.in_main(c))
                    .copied()
            })
            .map(|c| c.id)
    });
    if let Some(id) = target {
        ak(cx.egui, id, action);
    }
}

/// The safe button of the open dialog: the first enabled one of its footer (dialogs put
/// the safe button first), when its last row holds several buttons side by side below
/// the rest (a footer, not the last field of a form).
fn dialog_safe(snap: &Snap) -> Option<Cand> {
    let bottom = snap
        .overlay_all
        .iter()
        .map(|c| c.rect.bottom())
        .max_by(f32::total_cmp)?;
    let row: Vec<&Cand> = snap
        .overlay_all
        .iter()
        .filter(|c| (c.rect.bottom() - bottom).abs() <= 6.0)
        .collect();
    let row_top = row.iter().map(|c| c.rect.top()).min_by(f32::total_cmp)?;
    let above = snap
        .overlay_all
        .iter()
        .any(|c| c.rect.bottom() <= row_top + 1.0);
    if row.len() < 2 || !above {
        return None;
    }
    row.into_iter()
        .filter(|c| c.enabled)
        .min_by(|a, b| a.rect.left().total_cmp(&b.rect.left()))
        .copied()
}

/// The button that confirms the open dialog: the last of its footer (dialogs put the
/// safe button first and the one that acts last), when it is enabled.
fn dialog_confirm(snap: &Snap) -> Option<Cand> {
    let last = snap.overlay_all.iter().max_by(|a, b| {
        let (x, y) = (a.rect, b.rect);
        if (x.bottom() - y.bottom()).abs() > 6.0 {
            x.bottom().total_cmp(&y.bottom())
        } else {
            x.right().total_cmp(&y.right())
        }
    })?;
    last.enabled.then_some(*last)
}

/// The context menu of what has the pad's focus: its own, else the one of the row it
/// is in (a list's row at its cursor). Opened at the row; true when there was one.
fn open_row_menu(cx: &mut Ctx, snap: &Snap) -> bool {
    let Some(c) = current(cx, snap) else {
        return false;
    };
    let at = if c.list {
        let Some(s) = snap.list(c.id).cloned() else {
            return false;
        };
        match (s.cursor, s.cursor_rect) {
            (_, Some(r)) => r,
            // The row is scrolled away: bring it back, the next press opens its menu.
            (Some(_), None) => {
                list_move(cx, 0);
                return true;
            }
            (None, None) => return false,
        }
    } else {
        c.shown()
    };
    let menus = &snap.reg.menus;
    let target = menus
        .iter()
        .find(|(id, _)| *id == c.id)
        .or_else(|| {
            menus
                .iter()
                .filter(|(_, r)| r.contains(at.center()))
                .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
        })
        .map(|(id, _)| *id);
    let Some(id) = target else {
        return false;
    };
    registry::open_menu(cx.egui, id);
    cx.stores.pad.nav.settle = 4;
    true
}

/// What a view gives a button now (`actions.svelte.ts`'s per-view handlers): the
/// command and its arguments, when it applies.
fn view_command(
    view: ViewId,
    b: PadButton,
    row: bool,
    tag: Option<Value>,
) -> Option<(&'static str, Value)> {
    use PadButton as P;
    use ViewId as V;
    let none = Value::Null;
    Some(match (view, b) {
        // Favourites and history: X takes the row out of the list.
        (V::Favorites | V::History, P::X) if row => ("saved.remove", none),
        (V::Servers, P::X) if row => ("list.fav", none),
        (V::Servers, P::X) => ("servers.filter.more_popover", none),
        // A friend's server under the focus: X joins it.
        (V::Friends, P::X) => ("connect.address", tag.filter(|t| t.get("ip").is_some())?),
        (V::Servers | V::Favorites | V::History, P::Y) if row => ("list.ping", none),
        (V::Servers, P::Y) => ("servers.refresh", none),
        (V::Servers, P::R3) => ("servers.toggle_detail", none),
        (V::Favorites, P::R3) => ("favorites.toggle_detail", none),
        (V::History, P::R3) => ("history.toggle_detail", none),
        (V::Mods, P::X) if row => ("mods.tick", none),
        (V::Mods, P::Y) => ("mods.update", none),
        (V::News, P::X) => ("news.open", none),
        (V::Connect, P::X) => ("connect.query", none),
        (V::Connect, P::Y) => ("connect.join", none),
        (V::Offline, P::X) => ("offline.launch", tag?),
        (V::Offline, P::Y) => ("offline.install", none),
        (V::Options, P::X) => ("options.apply_recommended", none),
        (V::Options, P::Y) => ("options.active_only", none),
        _ => return None,
    })
}

/// The shortcut whose words the hint bar shows for a view command (the registry's pad
/// entry for it).
fn hint_id(command: &'static str) -> &'static str {
    match command {
        "servers.refresh" => "servers.refresh_pad",
        "servers.filter.more_popover" => "servers.filters_pad",
        "connect.address" => "friends.join",
        "servers.toggle_detail" | "favorites.toggle_detail" | "history.toggle_detail" => {
            "list.detail"
        }
        "options.apply_recommended" => "options.apply",
        "options.active_only" => "options.active",
        other => other,
    }
}

/// Whether a view command applies now, beyond being enabled (`actions.svelte.ts`'s
/// `when`: Direct Connect's X needs a valid address not being queried, its Y a profile
/// too, the mods' Y something to update).
fn applies(cx: &Ctx, id: &str) -> bool {
    let now = match id {
        "connect.query" => cx.stores.direct.valid() && !cx.stores.direct.querying,
        "connect.join" => cx.stores.direct.valid() && cx.stores.profile.data.is_some(),
        "mods.update" => crate::views::mods::can_update(cx),
        _ => true,
    };
    now && commands::find(id).is_some_and(|c| (c.enabled)(cx))
}

/// What `b` runs on the view now, with its arguments, when something applies.
fn applying(cx: &Ctx, snap: &Snap, b: PadButton) -> Option<(&'static str, Value)> {
    let cur = current(cx, snap);
    let row = snap.view_list().is_some_and(|s| s.cursor.is_some());
    let tag = cur.and_then(|c| {
        let p = c.shown().center();
        snap.reg
            .tags
            .iter()
            .find(|(r, _)| r.contains(p))
            .map(|(_, v)| v.clone())
    });
    view_command(cx.nav.view, b, row, tag).filter(|(id, _)| applies(cx, id))
}

/// Whether the pad rests on a text field without typing in it (A types there).
pub fn on_text(cx: &Ctx) -> bool {
    matches!(cx.stores.pad.nav.focus, Focus::Widget(id) if cx.stores.pad.nav.rest == Some(id))
}

/// What the hint bar says the view's buttons do now (`PadHints.svelte`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewHints {
    /// A joins the server under the list's cursor.
    pub join: bool,
    /// The shortcuts (their hint words) X, Y and R3 run now.
    pub x: Option<&'static str>,
    pub y: Option<&'static str>,
    pub r3: Option<&'static str>,
    /// Menu opens the actions of what has the focus (its right-click menu).
    pub menu: bool,
    /// LT and RT switch the tabs of the pane the focus is in (else they page).
    pub tabs: bool,
}

/// The view's buttons as they stand, from what the last frame drew. Only asked while a
/// controller drives the window.
pub fn view_hints(cx: &Ctx) -> ViewHints {
    let snap = Snap::take(cx);
    if snap.overlay {
        return ViewHints::default();
    }
    let join = current(cx, &snap).is_some_and(|c| {
        c.list
            && list_kind(cx.nav.view) == Some(ListKind::Servers)
            && snap.list(c.id).is_some_and(|s| s.cursor.is_some())
    });
    let hint = |b| applying(cx, &snap, b).map(|(id, _)| hint_id(id));
    let cur = current(cx, &snap);
    let menu = cur.is_some_and(|c| {
        let at = focus_point(&snap, &c);
        snap.reg
            .menus
            .iter()
            .any(|(id, r)| *id == c.id || r.contains(at))
    });
    ViewHints {
        join,
        x: hint(PadButton::X),
        y: hint(PadButton::Y),
        r3: hint(PadButton::R3),
        menu,
        tabs: cur.is_some_and(|c| tab_target(&snap, &c, 1).is_some()),
    }
}

fn command(cx: &mut Ctx, b: PadButton) {
    if palette(cx) {
        // The palette: View (or Menu) closes it again, Y brings a keyboard up over its
        // field.
        if matches!(b, PadButton::Menu | PadButton::View) {
            let _ = cx.run("palette.toggle", &Value::Null);
        } else if b == PadButton::Y
            && let Some(id) = cx
                .egui
                .memory(|m| m.focused())
                .filter(|id| is_text(cx.egui, *id))
        {
            open_keyboard(cx, id, false);
        }
        return;
    }
    let snap = Snap::take(cx);
    if snap.overlay {
        // In a dialog X confirms it, wherever the focus is; Menu closes the menu it
        // opened.
        if b == PadButton::X
            && let Some(c) = dialog_confirm(&snap)
        {
            ak(cx.egui, c.id, Action::Click);
            cx.stores.pad.nav.settle = 4;
        } else if b == PadButton::Menu && !snap.modal {
            key(cx.egui, Key::Escape, false);
            cx.stores.pad.nav.settle = 4;
        }
        return;
    }
    // Menu: the right-click menu of what has the focus (the list's row at its cursor),
    // else the palette.
    if b == PadButton::Menu {
        if !open_row_menu(cx, &snap) {
            let _ = cx.run("palette.toggle", &Value::Null);
        }
        return;
    }
    // A toast's button, while one is up: L3 presses it (it rejoins otherwise).
    if b == PadButton::L3
        && let Some(line) = crate::kit::toast::take_action(cx.egui)
    {
        let _ = cx.run_line(&line);
        return;
    }
    if let Some((id, args)) = applying(cx, &snap, b) {
        let _ = cx.run(id, &args);
        // It may open a popup (the filters) or a dialog (a question): land in it (the
        // command takes a frame to reach the view, the popup one more to be measured).
        cx.stores.pad.nav.settle = 8;
        cx.stores.pad.nav.await_popup = current(cx, &snap).is_none();
        return;
    }
    // The app's own: the palette, the shortcuts, rejoining.
    if let Some(s) = SHORTCUTS
        .iter()
        .find(|s| s.scope == Scope::Global && s.pad.contains(&b))
    {
        let _ = cx.run(s.id, &Value::Null);
    }
}

// ── drawing ────────────────────────────────────────────────────────────────

/// At the end of the frame: the focus ring, in gamepad mode (`base.css`'s
/// `[data-input="gamepad"] :focus`: an accent outline inside the edge, 3 px in the pad
/// density, a soft inner band and a glow outside; a list's row is tinted as well and
/// ringed only where it shows, inside its list, clear of what covers the list).
pub fn paint(ctx: &Context, cx: &mut Ctx) {
    if let Some(id) = cx.stores.pad.nav.restore.take() {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    if cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) != InputMode::Gamepad {
        return;
    }
    if let Some(id) = ctx.memory(|m| m.focused()) {
        if let Some(w) = ctx.viewport(|v| v.this_pass.widgets.get(id).copied()) {
            // A bare text field's rect is its text: the ring goes round it, clear of it.
            let r = if is_text(ctx, id) {
                w.interact_rect.expand(size::ring() + 1.0)
            } else {
                w.interact_rect
            };
            ring(ctx, w.layer_id, r, false);
        }
        return;
    }
    // A disabled control with a hint, which egui gives no focus to, or a text field the
    // pad rests on.
    if let Focus::Widget(id) = cx.stores.pad.nav.focus
        && (cx.stores.pad.nav.rest == Some(id) || registry::current(ctx).tipped.contains(&id))
        && let Some(w) = ctx.viewport(|v| v.this_pass.widgets.get(id).copied())
    {
        let r = if is_text(ctx, id) {
            w.interact_rect.expand(size::ring() + 1.0)
        } else {
            w.interact_rect
        };
        ring(ctx, w.layer_id, r, false);
        return;
    }
    if let Focus::List(id) = cx.stores.pad.nav.focus {
        let reg = registry::current(ctx);
        if let Some(s) = reg.lists.iter().find(|s| s.id == id) {
            let covers: Vec<Rect> = reg
                .covers
                .iter()
                .filter(|c| c.layer == s.layer)
                .map(|c| c.rect)
                .collect();
            let shown = uncovered(s.viewport, &covers);
            match s.cursor_rect {
                Some(r) => ring(ctx, s.layer, r.intersect(shown), true),
                None => ring(ctx, s.layer, shown, false),
            }
        }
    }
}

/// What of `r` shows past the `covers` drawn over it (a pane over its right side, a
/// sticky band over its top or foot).
fn uncovered(r: Rect, covers: &[Rect]) -> Rect {
    let mut shown = r;
    for c in covers {
        if !c.intersects(shown) {
            continue;
        }
        let full_w = c.left() <= shown.left() + 0.5 && c.right() >= shown.right() - 0.5;
        if full_w && c.top() <= shown.top() {
            shown.min.y = shown.min.y.max(c.bottom());
        } else if full_w && c.bottom() >= shown.bottom() {
            shown.max.y = shown.max.y.min(c.top());
        } else if c.right() >= shown.right() {
            shown.max.x = shown.max.x.min(c.left());
        }
    }
    shown
}

fn ring(ctx: &Context, layer: LayerId, r: Rect, row: bool) {
    if !r.is_finite() || r.width() < 1.0 || r.height() < 1.0 {
        return;
    }
    let tk = ctx.tokens();
    let line = size::ring();
    let mut p = ctx.layer_painter(layer);
    let radius = CornerRadius::same(size::RADIUS_SM);
    let a = |c: egui::Color32, f: f32| crate::kit::paint::alpha(c, f);
    if row {
        // A row: inside its list only, so the glow never spills over a header or the
        // row beside a sticky band.
        p.set_clip_rect(r);
        p.rect_filled(r, radius, a(tk.accent, 0.10));
    } else {
        p.rect_stroke(
            r,
            radius,
            Stroke::new(line + 1.0, a(tk.accent, 0.3)),
            egui::StrokeKind::Outside,
        );
    }
    p.rect_stroke(
        r.shrink(line),
        radius,
        Stroke::new(2.0, a(tk.accent, 0.25)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        r,
        radius,
        Stroke::new(line, tk.accent),
        egui::StrokeKind::Inside,
    );
}

/// For `ui.state` (asked between frames): who drives, and where the pad is, as the
/// last frame drew it.
pub fn describe(ctx: &Context, cx: &Ctx) -> Value {
    let mode = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    let rect = |r: Rect| json!([r.left(), r.top(), r.width(), r.height()]);
    let focus = match (ctx.memory(|m| m.focused()), cx.stores.pad.nav.focus) {
        (Some(id), _) => {
            let w = ctx.viewport(|v| v.prev_pass.widgets.get(id).copied());
            json!({
                "kind": if is_text(ctx, id) { "text" } else { "widget" },
                "rect": w.map(|w| rect(w.interact_rect)),
            })
        }
        // A text field the pad rests on, not typing in it.
        (None, Focus::Widget(id)) if cx.stores.pad.nav.rest == Some(id) => {
            let w = ctx.viewport(|v| v.prev_pass.widgets.get(id).copied());
            json!({
                "kind": "text",
                "resting": true,
                "rect": w.map(|w| rect(w.interact_rect)),
            })
        }
        (None, Focus::List(id)) => {
            let s = registry::last(ctx).lists.into_iter().find(|s| s.id == id);
            json!({
                "kind": "list",
                "rect": s.as_ref().map(|s| rect(s.viewport)),
                "cursor": s.and_then(|s| s.cursor),
            })
        }
        _ => Value::Null,
    };
    json!({
        "mode": if mode { "gamepad" } else { "pointer" },
        "focus": focus,
        "pads": cx.stores.pad.pads.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_list_ring_stays_clear_of_covers() {
        let list = Rect::from_min_max(egui::pos2(0.0, 100.0), egui::pos2(800.0, 600.0));
        let pane = Rect::from_min_max(egui::pos2(500.0, 0.0), egui::pos2(900.0, 700.0));
        let head = Rect::from_min_max(egui::pos2(0.0, 90.0), egui::pos2(800.0, 130.0));
        let foot = Rect::from_min_max(egui::pos2(-10.0, 560.0), egui::pos2(810.0, 620.0));
        let away = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(50.0, 50.0));
        let r = uncovered(list, &[pane, head, foot, away]);
        assert_eq!(r.right(), 500.0);
        assert_eq!(r.top(), 130.0);
        assert_eq!(r.bottom(), 560.0);
        assert_eq!(r.left(), 0.0);
        assert_eq!(uncovered(list, &[]), list);
    }

    #[test]
    fn actions_by_name() {
        assert_eq!(action_named("dpad_down"), Some(PadAction::Down));
        assert_eq!(action_named("down"), Some(PadAction::Down));
        assert_eq!(action_named("A"), Some(PadAction::Accept));
        assert_eq!(action_named("prevTab"), Some(PadAction::PrevTab));
        assert_eq!(action_named("rt"), Some(PadAction::PageDown));
        assert_eq!(action_named("rs_down"), Some(PadAction::ScrollDown));
        assert_eq!(action_named("r3"), Some(PadAction::RightStick));
        assert_eq!(action_named("nope"), None);
    }

    #[test]
    fn view_buttons_follow_the_registry() {
        use PadButton as P;
        // Every view command named here is a command, and its button is the one the
        // registry (the hints, the sheet) gives the shortcut.
        let cases = [
            (ViewId::Servers, P::X, true, "list.fav", "list.fav"),
            (
                ViewId::Servers,
                P::X,
                false,
                "servers.filter.more_popover",
                "servers.filters_pad",
            ),
            (
                ViewId::Favorites,
                P::X,
                true,
                "saved.remove",
                "saved.remove",
            ),
            (ViewId::History, P::X, true, "saved.remove", "saved.remove"),
            (ViewId::Servers, P::Y, true, "list.ping", "list.ping"),
            (
                ViewId::Servers,
                P::Y,
                false,
                "servers.refresh",
                "servers.refresh_pad",
            ),
            (
                ViewId::Servers,
                P::R3,
                false,
                "servers.toggle_detail",
                "list.detail",
            ),
            (ViewId::Mods, P::X, true, "mods.tick", "mods.tick"),
            (ViewId::Mods, P::Y, false, "mods.update", "mods.update"),
            (ViewId::News, P::X, false, "news.open", "news.open"),
            (
                ViewId::Connect,
                P::X,
                false,
                "connect.query",
                "connect.query",
            ),
            (ViewId::Connect, P::Y, false, "connect.join", "connect.join"),
            (
                ViewId::Offline,
                P::Y,
                false,
                "offline.install",
                "offline.install",
            ),
            (
                ViewId::Options,
                P::X,
                false,
                "options.apply_recommended",
                "options.apply",
            ),
            (
                ViewId::Options,
                P::Y,
                false,
                "options.active_only",
                "options.active",
            ),
        ];
        for (v, b, row, cmd, sc) in cases {
            let got = view_command(v, b, row, None).map(|(id, _)| id);
            assert_eq!(got, Some(cmd), "{v:?} {b:?}");
            assert!(commands::find(cmd).is_some(), "{cmd} is not a command");
            assert_eq!(hint_id(cmd), sc, "{cmd}'s hint");
            let s = commands::shortcut(sc).expect("shortcut");
            assert!(s.pad.contains(&b), "{sc} is not on {b:?}");
        }
        // Without a row under the focus, X does nothing in the lists.
        assert_eq!(view_command(ViewId::Favorites, P::X, false, None), None);
        // Friends' X needs a server under the focus.
        assert_eq!(view_command(ViewId::Friends, P::X, false, None), None);
        let srv = json!({"ip": "1.2.3.4", "port": 2302});
        assert_eq!(
            view_command(ViewId::Friends, P::X, false, Some(srv.clone())),
            Some(("connect.address", srv))
        );
        assert!(commands::shortcut("friends.join").is_some_and(|s| s.pad.contains(&P::X)));
        // Offline's X needs the mission under the focus.
        assert_eq!(view_command(ViewId::Offline, P::X, false, None), None);
        let m = json!({"mission": "dayzOffline.chernarusplus"});
        assert_eq!(
            view_command(ViewId::Offline, P::X, false, Some(m.clone())),
            Some(("offline.launch", m))
        );
    }

    fn cand(n: u64, x: f32, y: f32, w: f32, h: f32) -> Cand {
        let r = Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h));
        Cand {
            id: Id::unique(n),
            rect: r,
            full: r,
            layer: LayerId::background(),
            list: false,
            enabled: true,
        }
    }

    fn snap(cands: Vec<Cand>, regions: Vec<Rect>) -> Snap {
        let main = regions.first().copied().unwrap_or(Rect::EVERYTHING);
        Snap {
            overlay: false,
            modal: false,
            bars: false,
            cands,
            overlay_all: Vec::new(),
            lists: Vec::new(),
            regions,
            main,
            reg: Frame::default(),
        }
    }

    #[test]
    fn moves_stay_in_their_region_first() {
        let main = Rect::from_min_max(egui::pos2(200.0, 0.0), egui::pos2(1000.0, 800.0));
        let rail = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 800.0));
        let from = cand(1, 220.0, 100.0, 100.0, 30.0);
        // Below in the rail, nearer; below in the main area, farther and off to the side.
        let rail_below = cand(2, 100.0, 140.0, 90.0, 30.0);
        let main_below = cand(3, 400.0, 400.0, 100.0, 30.0);
        let s = snap(vec![from, rail_below, main_below], vec![main, rail]);
        let to = pick(&s, &from, from.rect, Dir::Down).map(|c| c.id);
        assert_eq!(to, Some(Id::unique(3u64)), "down stays in the main area");
        // Left leaves the region: the rail is that way.
        let to = pick(&s, &from, from.rect, Dir::Left).map(|c| c.id);
        assert_eq!(to, Some(Id::unique(2u64)));
        // Nothing more down in the main area: down may leave it.
        let s = snap(vec![from, rail_below], vec![main, rail]);
        let to = pick(&s, &from, from.rect, Dir::Down).map(|c| c.id);
        assert_eq!(to, Some(Id::unique(2u64)));
    }

    #[test]
    fn a_field_below_the_fold_counts_in_its_region_only() {
        let main = Rect::from_min_max(egui::pos2(200.0, 0.0), egui::pos2(1000.0, 800.0));
        let status = Rect::from_min_max(egui::pos2(0.0, 800.0), egui::pos2(1000.0, 830.0));
        let from = cand(1, 300.0, 700.0, 200.0, 30.0);
        // Scrolled out of sight under the main area's edge: nothing of it shows.
        let mut hidden = cand(2, 300.0, 900.0, 200.0, 30.0);
        hidden.rect = Rect::NOTHING;
        let in_status = cand(3, 300.0, 805.0, 100.0, 20.0);
        let s = snap(vec![from, hidden, in_status], vec![main, status]);
        let to = pick(&s, &from, from.rect, Dir::Down).map(|c| c.id);
        assert_eq!(
            to,
            Some(Id::unique(2u64)),
            "the next field of the page, not the status bar"
        );
        // Out of sight beside the region: never.
        let mut aside = cand(4, 1100.0, 900.0, 50.0, 30.0);
        aside.rect = Rect::NOTHING;
        let s = snap(vec![from, aside], vec![main, status]);
        assert!(pick(&s, &from, from.rect, Dir::Down).is_none());
    }

    #[test]
    fn the_keyboard_follows_the_setting_and_steam() {
        use OskChoice as O;
        assert_eq!(keyboard(O::Auto, true, true), Keyboard::Steam);
        assert_eq!(keyboard(O::Auto, false, false), Keyboard::Builtin);
        // Auto keeps to the big-screen interface; Steam asks Steam wherever it runs.
        assert_eq!(keyboard(O::Auto, false, true), Keyboard::Builtin);
        assert_eq!(keyboard(O::Steam, false, true), Keyboard::Steam);
        assert_eq!(keyboard(O::Steam, true, false), Keyboard::Steam);
        assert_eq!(keyboard(O::Steam, false, false), Keyboard::Builtin);
        assert_eq!(keyboard(O::Builtin, true, true), Keyboard::Builtin);
        assert_eq!(keyboard(O::None, true, true), Keyboard::None);
        // Every pad button the keyboard knows; the others are dropped while it is up.
        assert_eq!(osk_press(PadAction::Accept), Some(Press::Type));
        assert_eq!(osk_press(PadAction::Menu), Some(Press::Done));
        assert_eq!(osk_press(PadAction::NextTab), Some(Press::CaretRight));
        assert_eq!(osk_press(PadAction::PageDown), None);
    }

    #[test]
    fn a_page_is_most_of_the_height() {
        assert_eq!(page_steps(700.0), 6);
        assert_eq!(page_steps(10.0), 1);
        assert_eq!(page_steps(f32::NAN), 1);
        assert_eq!(page_steps(1e9), 40);
    }

    #[test]
    fn the_game_holds_the_pad_until_the_player_is_back() {
        let mut away = false;
        let mut step = |running, was, holds, focused, used| {
            game_step(
                GameWatch {
                    running,
                    was_running: was,
                    holds,
                    went_away: &mut away,
                },
                focused,
                used,
            )
        };
        // Not running: never held.
        assert!(!step(false, false, false, true, false));
        // The game starts while the window still says it has the focus: held.
        assert!(step(true, false, false, true, false));
        assert!(step(true, true, true, true, false));
        // The window lost the focus, then got it back: the pad is the window's again.
        assert!(step(true, true, true, false, false));
        assert!(!step(true, true, true, true, false));
        // Used with the pointer or the keys while held: back at once.
        assert!(step(true, false, false, true, false));
        assert!(!step(true, true, true, true, true));
    }

    #[test]
    fn rows_under_a_sticky_heading_are_out_of_sight() {
        let head = (
            Id::unique(9u64),
            Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(400.0, 30.0)),
        );
        let under = Rect::from_min_size(egui::pos2(10.0, 105.0), egui::vec2(100.0, 20.0));
        assert!(!under_stickies(under, &[head]).is_positive());
        let half = Rect::from_min_size(egui::pos2(10.0, 120.0), egui::vec2(100.0, 30.0));
        let r = under_stickies(half, &[head]);
        assert_eq!(r.top(), 130.0);
        assert_eq!(r.bottom(), 150.0);
        let clear = Rect::from_min_size(egui::pos2(10.0, 200.0), egui::vec2(100.0, 30.0));
        assert_eq!(under_stickies(clear, &[head]), clear);
    }

    fn overlay(cands: Vec<Cand>) -> Snap {
        let mut s = snap(cands.clone(), vec![]);
        s.overlay = true;
        s.modal = true;
        s.overlay_all = cands;
        s
    }

    #[test]
    fn a_dialog_lands_on_its_safe_button_never_on_the_cross() {
        let cross = cand(1, 470.0, 10.0, 24.0, 24.0);
        let field = cand(2, 20.0, 60.0, 300.0, 28.0);
        let cancel = cand(3, 300.0, 200.0, 80.0, 28.0);
        let ok = cand(4, 390.0, 200.0, 80.0, 28.0);
        let s = overlay(vec![cross, field, cancel, ok]);
        assert_eq!(dialog_safe(&s).map(|c| c.id), Some(Id::unique(3u64)));
        assert_eq!(dialog_confirm(&s).map(|c| c.id), Some(Id::unique(4u64)));
        assert_eq!(close_cross(&s), Some(Id::unique(1u64)));
        // A lone button at the foot is no footer to land on.
        let s = overlay(vec![cross, field, ok]);
        assert!(dialog_safe(&s).is_none());
    }

    #[test]
    fn sideways_moves_never_reach_the_title_bar() {
        let title = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 40.0));
        let status = Rect::from_min_max(egui::pos2(0.0, 780.0), egui::pos2(1000.0, 800.0));
        let main = Rect::from_min_max(egui::pos2(0.0, 40.0), egui::pos2(1000.0, 780.0));
        let row = cand(1, 0.0, 60.0, 600.0, 30.0);
        let in_title = cand(2, 900.0, 5.0, 30.0, 30.0);
        let mut s = snap(vec![row, in_title], vec![title, status, main]);
        s.bars = true;
        assert!(pick(&s, &row, row.rect, Dir::Right).is_none());
        // Up still reaches it.
        assert_eq!(
            pick(&s, &row, row.rect, Dir::Up).map(|c| c.id),
            Some(Id::unique(2u64))
        );
        // A pane beside the row: Right goes there.
        let pane = cand(3, 700.0, 300.0, 200.0, 30.0);
        let mut s = snap(vec![row, in_title, pane], vec![title, status, main]);
        s.bars = true;
        assert_eq!(
            pick(&s, &row, row.rect, Dir::Right).map(|c| c.id),
            Some(Id::unique(3u64))
        );
    }

    #[test]
    fn lt_and_rt_go_round_the_tabs_of_the_pane() {
        let base = Id::unique("strip");
        let mut s = snap(
            vec![],
            vec![Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )],
        );
        s.reg.tabs.push(registry::Tabs {
            layer: LayerId::background(),
            strip: Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(300.0, 24.0)),
            base,
            count: 3,
            current: Some(0),
        });
        let at = cand(7, 20.0, 100.0, 100.0, 24.0);
        let tab = |i| Some(crate::kit::tabs::tab_id(base, i));
        assert_eq!(tab_target(&s, &at, 1), tab(1));
        assert_eq!(tab_target(&s, &at, -1), tab(2));
    }

    fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn keys_become_pad_actions_and_a_text_field_keeps_all_but_escape() {
        let down = key_event(Key::ArrowDown, true, Modifiers::NONE);
        assert_eq!(
            key_action(&down, false),
            Some((PadAction::Down, false, true))
        );
        assert_eq!(key_action(&down, true), None, "the field moves its caret");
        let esc = key_event(Key::Escape, true, Modifiers::NONE);
        assert_eq!(key_action(&esc, true), Some((PadAction::Back, false, true)));
        let enter = key_event(Key::Enter, false, Modifiers::NONE);
        assert_eq!(
            key_action(&enter, false),
            Some((PadAction::Accept, false, false)),
            "a release is taken out too, but acts on nothing"
        );
        let shift_down = key_event(Key::ArrowDown, true, Modifiers::SHIFT);
        assert_eq!(key_action(&shift_down, false), None);
        assert_eq!(key_action(&Event::Text("a".into()), false), None);
    }

    #[test]
    fn one_arrow_moves_one_stop_and_egui_never_sees_it() {
        // Before egui's pass reads the input: an arrow left in it would move egui's own
        // focus one more step.
        let mut raw = egui::RawInput::default();
        raw.events
            .push(key_event(Key::ArrowDown, true, Modifiers::NONE));
        raw.events
            .push(key_event(Key::ArrowDown, false, Modifiers::NONE));
        raw.events.push(Event::Text("x".into()));
        let mut keys = Vec::new();
        strip_keys(&mut raw.events, false, &mut keys);
        assert_eq!(keys, vec![(PadAction::Down, false)]);
        assert_eq!(raw.events, vec![Event::Text("x".into())]);
        // In a text field the arrows stay (the caret), Escape leaves.
        let mut events = vec![
            key_event(Key::ArrowLeft, true, Modifiers::NONE),
            key_event(Key::Escape, true, Modifiers::NONE),
        ];
        let mut keys = Vec::new();
        strip_keys(&mut events, true, &mut keys);
        assert_eq!(keys, vec![(PadAction::Back, false)]);
        assert_eq!(
            events,
            vec![key_event(Key::ArrowLeft, true, Modifiers::NONE)]
        );
    }

    #[test]
    fn the_registered_cross_is_the_dialog_s_cross_whatever_its_shape() {
        let mut s = snap(vec![], vec![]);
        s.overlay = true;
        let wide = cand(1, 300.0, 10.0, 80.0, 24.0);
        let ok = cand(2, 300.0, 200.0, 80.0, 24.0);
        s.overlay_all = vec![wide, ok];
        assert_eq!(close_cross(&s), None);
        s.reg.crosses.push(Id::unique(1u64));
        assert_eq!(close_cross(&s), Some(Id::unique(1u64)));
    }
}
