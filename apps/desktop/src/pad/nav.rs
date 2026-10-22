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
//!   then 10 rows at a time; Up past its first row, Left and Right leave it.
//! - **A** clicks what has the focus (an AccessKit click: exactly what Enter on a
//!   focused button does, without the key reaching anything else); a text field just
//!   takes the focus (and Steam's keyboard comes up in Steam's big-screen interface); on
//!   a server list's row it joins (`list.join`).
//! - **B** closes the topmost popup or dialog (an Escape for it); on a text field it
//!   leaves the field downward; else it is Escape for the view (deselect, close the
//!   details).
//! - **LB/RB** go to the previous/next view and land there once it is drawn; **LT/RT**
//!   page a list (`list.page`) or scroll what holds the focus; the **right stick**
//!   scrolls the reading pane (`registry::reading_pane`).
//! - **X, Y, R3, L3, Menu, View** run the view's own commands when they apply (the
//!   registry's pad buttons: X favourite, Y ping…), else the app's (Menu the palette,
//!   View the shortcuts, L3 rejoin); in a dialog X presses its confirming button (the
//!   last of its footer).
//! - The search palette is driven with its own keys (arrows, Enter, Tab, Escape).
//! - With nothing focused, a press **lands**: on what had the focus last in this view,
//!   else the view's list at its row, else the first thing in reading order.
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
use crate::prefs::{PadMode, Prefs};
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
    /// A view switched to with LB/RB: land in it once it is drawn (frames left, and the
    /// pass that switched: its widgets are the old view's).
    land_in: Option<(ViewId, u8, u64)>,
    /// After a page scroll: the height the focus was at, frames left to follow it.
    refocus: Option<(f32, u8)>,
    /// Give this widget the focus back at the end of the frame (B's Escape went by).
    restore: Option<Id>,
    was_pad: bool,
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

/// Whether the rail is drawn icon-only: the preference, or a controller driving a
/// window narrower than 1200 points (the list gets the room).
pub fn rail_collapsed(ctx: &Context, stores: &Stores, prefs: &Prefs) -> bool {
    let p = prefs.get();
    p.ui.rail_collapsed
        || (stores.pad.mode(p.ui.pad_mode) == InputMode::Gamepad
            && ctx.content_rect().width() < 1200.0)
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
    let actions = std::mem::take(&mut cx.stores.pad.queue);
    // Never: pad input is ignored entirely.
    if setting == PadMode::Never {
        cx.stores.pad.nav.was_pad = false;
        return;
    }
    for (a, repeat) in actions {
        cx.stores.pad.used_pad();
        handle(cx, a, repeat);
    }
    let pad = cx.stores.pad.mode(setting) == InputMode::Gamepad;
    if cx.stores.pad.nav.was_pad && !pad {
        // The pointer took over: a button keeping the keyboard focus would hold the
        // views' keys back. A text field keeps it (the player may be typing).
        if !cx.egui.text_edit_focused() {
            drop_focus(cx.egui);
        }
        cx.stores.pad.nav.focus = Focus::None;
    }
    cx.stores.pad.nav.was_pad = pad;
    if !pad {
        let n = &mut cx.stores.pad.nav;
        n.settle = 0;
        n.land_in = None;
        n.refocus = None;
        return;
    }
    waits(cx);
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
        // A popup's first frame only measures it: nothing to land on yet.
        if current(cx, &snap).is_none() && land(cx, &snap) {
            cx.stores.pad.nav.settle = 0;
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
        for (layer, ws) in &layers {
            if !in_scope(layer) {
                continue;
            }
            let covers: Vec<(usize, Rect)> = reg
                .covers
                .iter()
                .filter(|c| c.layer == *layer)
                .filter_map(|c| ws.iter().position(|w| w.id == c.id).map(|i| (i, c.rect)))
                .collect();
            for (i, w) in ws.iter().enumerate() {
                // Clickable things, not the background an area senses for itself.
                if !(w.sense.is_focusable() && w.sense.senses_click())
                    || w.id == layer.id.with("move")
                {
                    continue;
                }
                if !w.rect.is_finite() || !w.interact_rect.is_finite() || veil(w.rect) {
                    continue;
                }
                let center = w.rect.center();
                if covers
                    .iter()
                    .any(|(ci, r)| *ci == i || (i < *ci && r.contains(center)))
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
                let c = Cand {
                    id: w.id,
                    rect: w.interact_rect,
                    full: w.rect,
                    layer: *layer,
                    list: false,
                    enabled: w.enabled,
                };
                if top.is_some() && c.visible() {
                    overlay_all.push(c);
                }
                if w.enabled {
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
        let main = Rect::from_min_max(
            egui::pos2(rail.right(), top_y),
            egui::pos2(screen.right(), bottom_y),
        );
        let mut regions = vec![titlebar, statusbar, rail, main];
        regions.extend(reg.regions.iter().copied());
        Self {
            overlay: top.is_some(),
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

/// Where the pad is now: the focused widget when it is in the scope, else the list it
/// walks.
fn current(cx: &Ctx, snap: &Snap) -> Option<Cand> {
    if let Some(id) = cx.egui.memory(|m| m.focused())
        && let Some(c) = snap.cand(id).filter(|c| !c.list)
    {
        return Some(c);
    }
    if let Focus::List(id) = cx.stores.pad.nav.focus {
        return snap.cand(id).filter(|c| c.list);
    }
    None
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
    cx.egui.memory_mut(|m| m.request_focus(c.id));
    if !c.fully_visible() {
        ak(cx.egui, c.id, Action::ScrollIntoView);
    }
    let n = &mut cx.stores.pad.nav;
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
/// view, else the view's list at its row, else the first thing in reading order.
fn land(cx: &mut Ctx, snap: &Snap) -> bool {
    if !snap.overlay {
        let mem = cx.stores.pad.nav.remembered.get(&cx.nav.view).copied();
        if let Some(c) = mem.and_then(|id| snap.cand(id)).filter(Cand::visible) {
            focus_widget(cx, snap, &c);
            return true;
        }
        if let Some(s) = snap.view_list().cloned() {
            enter_list(cx, &s);
            return true;
        }
    }
    let pick = |within: &dyn Fn(&Cand) -> bool| {
        snap.cands
            .iter()
            .filter(|c| c.visible() && within(c))
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
    let mut best: Option<(f32, Cand)> = None;
    for c in &snap.cands {
        if c.id == from.id || (!from.list && from.full.contains_rect(c.full)) {
            continue;
        }
        let to = c.shown();
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
            from_rect = r;
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
    }
    if let Some(to) = pick(&snap, &from, from_rect, dir) {
        focus(cx, &snap, &to);
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
        cx.egui.memory_mut(|m| m.request_focus(c.id));
        // Game Mode has no keyboard: Steam's own comes up over the window.
        if cx.stores.pad.steam_ui {
            let _ = open::that_detached("steam://open/keyboard");
        }
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
    if snap.overlay {
        key(cx.egui, Key::Escape, false);
        cx.stores.pad.nav.settle = 4;
        return;
    }
    let cur = current(cx, &snap);
    // A text field: done typing. The focus goes to what is below it (a search's
    // results), else stays on the field, where A types again.
    if let Some(c) = cur.filter(|c| !c.list && is_text(cx.egui, c.id)) {
        if let Some(to) = pick(&snap, &c, c.shown(), Dir::Down) {
            drop_focus(cx.egui);
            focus(cx, &snap, &to);
        }
        return;
    }
    // The view's own Escape: the lists clear their selection, the details close. The
    // views read keys only while nothing is focused; the focus comes back at the end.
    if let Some(c) = cur.filter(|c| !c.list) {
        drop_focus(cx.egui);
        cx.stores.pad.nav.restore = Some(c.id);
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
        (V::Servers | V::Favorites | V::History, P::X) if row => ("list.fav", none),
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

/// What the hint bar says the view's buttons do now (`PadHints.svelte`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewHints {
    /// A joins the server under the list's cursor.
    pub join: bool,
    /// The shortcuts (their hint words) X, Y and R3 run now.
    pub x: Option<&'static str>,
    pub y: Option<&'static str>,
    pub r3: Option<&'static str>,
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
    ViewHints {
        join,
        x: hint(PadButton::X),
        y: hint(PadButton::Y),
        r3: hint(PadButton::R3),
    }
}

fn command(cx: &mut Ctx, b: PadButton) {
    if palette(cx) {
        // The palette: Menu closes it again, Y brings Steam's keyboard.
        if b == PadButton::Menu {
            let _ = cx.run("palette.toggle", &Value::Null);
        } else if b == PadButton::Y && cx.stores.pad.steam_ui {
            let _ = open::that_detached("steam://open/keyboard");
        }
        return;
    }
    let snap = Snap::take(cx);
    if snap.overlay {
        // In a dialog X confirms it, wherever the focus is.
        if b == PadButton::X
            && let Some(c) = dialog_confirm(&snap)
        {
            ak(cx.egui, c.id, Action::Click);
            cx.stores.pad.nav.settle = 4;
        }
        return;
    }
    if let Some((id, args)) = applying(cx, &snap, b) {
        let _ = cx.run(id, &args);
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
/// `[data-input="gamepad"] :focus`: a 2px accent outline inside the edge, a soft inner
/// band and a glow outside; a list's row is tinted as well).
pub fn paint(ctx: &Context, cx: &mut Ctx) {
    if let Some(id) = cx.stores.pad.nav.restore.take() {
        ctx.memory_mut(|m| m.request_focus(id));
    }
    if cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) != InputMode::Gamepad {
        return;
    }
    if let Some(id) = ctx.memory(|m| m.focused()) {
        if let Some(w) = ctx.viewport(|v| v.this_pass.widgets.get(id).copied()) {
            ring(ctx, w.layer_id, w.interact_rect, false);
        }
        return;
    }
    if let Focus::List(id) = cx.stores.pad.nav.focus
        && let Some(s) = registry::current(ctx)
            .lists
            .into_iter()
            .find(|s| s.id == id)
    {
        match s.cursor_rect {
            Some(r) => ring(ctx, s.layer, r.intersect(s.viewport), true),
            None => ring(ctx, s.layer, s.viewport, false),
        }
    }
}

fn ring(ctx: &Context, layer: LayerId, r: Rect, row: bool) {
    if !r.is_finite() || r.width() < 1.0 || r.height() < 1.0 {
        return;
    }
    let tk = ctx.tokens();
    let p = ctx.layer_painter(layer);
    let radius = CornerRadius::same(size::RADIUS_SM);
    let a = |c: egui::Color32, f: f32| crate::kit::paint::alpha(c, f);
    if row {
        p.rect_filled(r, radius, a(tk.accent, 0.08));
    }
    p.rect_stroke(
        r,
        radius,
        Stroke::new(3.0, a(tk.accent, 0.3)),
        egui::StrokeKind::Outside,
    );
    p.rect_stroke(
        r.shrink(2.0),
        radius,
        Stroke::new(2.0, a(tk.accent, 0.25)),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        r,
        radius,
        Stroke::new(2.0, tk.accent),
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
            id: Id::new(n),
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
        assert_eq!(to, Some(Id::new(3u64)), "down stays in the main area");
        // Left leaves the region: the rail is that way.
        let to = pick(&s, &from, from.rect, Dir::Left).map(|c| c.id);
        assert_eq!(to, Some(Id::new(2u64)));
        // Nothing more down in the main area: down may leave it.
        let s = snap(vec![from, rail_below], vec![main, rail]);
        let to = pick(&s, &from, from.rect, Dir::Down).map(|c| c.id);
        assert_eq!(to, Some(Id::new(2u64)));
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
            Some(Id::new(2u64)),
            "the next field of the page, not the status bar"
        );
        // Out of sight beside the region: never.
        let mut aside = cand(4, 1100.0, 900.0, 50.0, 30.0);
        aside.rect = Rect::NOTHING;
        let s = snap(vec![from, aside], vec![main, status]);
        assert!(pick(&s, &from, from.rect, Dir::Down).is_none());
    }

    #[test]
    fn a_page_is_most_of_the_height() {
        assert_eq!(page_steps(700.0), 6);
        assert_eq!(page_steps(10.0), 1);
        assert_eq!(page_steps(f32::NAN), 1);
        assert_eq!(page_steps(1e9), 40);
    }
}
