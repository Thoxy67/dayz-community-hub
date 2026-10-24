//! What the frame tells the controller beyond the widgets egui knows: the lists walked
//! row by row, parts of the window a move prefers to stay in, things drawn over others,
//! values attached to a part of the screen, and the panes the right stick scrolls.
//!
//! Each frame fills a fresh set while it draws; the controller reads the set the last
//! frame left (what is on screen), the same way egui answers clicks from the last
//! frame's widget rects. Nothing here costs anything when no one reads it: a few small
//! vectors, cleared once per frame.

use egui::{Context, Id, LayerId, Rect, Ui};
use serde_json::Value;

use crate::theme::size;

/// A list a controller walks row by row (`kit::VirtualList` with a cursor).
#[derive(Clone, Debug)]
pub struct ListSlot {
    pub id: Id,
    pub layer: LayerId,
    /// Where its rows are drawn (below its header).
    pub viewport: Rect,
    pub count: usize,
    pub row_height: f32,
    /// The row the keys and the pad are on (the selection).
    pub cursor: Option<usize>,
    /// That row is the first one a move can reach: Up leaves the list from it.
    pub at_top: bool,
    /// The cursor row's rect, when it is drawn.
    pub cursor_rect: Option<Rect>,
    /// The first row on screen.
    pub first_visible: usize,
}

/// Something drawn over the widgets of its layer drawn before it (a drawer).
#[derive(Clone, Copy, Debug)]
pub struct Cover {
    pub id: Id,
    pub layer: LayerId,
    pub rect: Rect,
}

/// One frame's worth.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub lists: Vec<ListSlot>,
    pub regions: Vec<Rect>,
    pub covers: Vec<Cover>,
    pub tags: Vec<(Rect, Value)>,
    /// Widgets inside reading panes: (widget id, the pane's visible rect).
    pub anchors: Vec<(Id, Rect)>,
    /// Widgets that take Left and Right themselves (sliders).
    pub sliders: Vec<Id>,
    /// Number fields: Left and Right step them, the on-screen keyboard has digits.
    pub numbers: Vec<Id>,
    /// Text fields holding a number they do not step: the on-screen keyboard has digits.
    pub digits: Vec<Id>,
    /// Disabled controls carrying a hint: reachable in gamepad mode, so the hint (why it
    /// is disabled) can be read.
    pub tipped: Vec<Id>,
    /// Disabled buttons of the kit: no stop of their own (unless tipped), but a dialog's
    /// footer counts them, so X never confirms through the button before a disabled one.
    pub dormant: Vec<Id>,
    /// Widgets that take Up and Down themselves (a stacked split's handle).
    pub vsliders: Vec<Id>,
    /// Widgets holding the pad's directions until B lets them go (a grabbed handle).
    pub held: Vec<Id>,
    /// Widgets with a context menu the pad's Menu opens: (widget id, its rect).
    pub menus: Vec<(Id, Rect)>,
    /// Where a press lands in a view or a dialog with nothing focused yet.
    pub primaries: Vec<Id>,
    /// Headings drawn over the rows scrolled under them.
    pub stickies: Vec<Cover>,
    /// Strips of tabs LT and RT switch.
    pub tabs: Vec<Tabs>,
    /// Dialogs' close crosses: a press never lands on one.
    pub crosses: Vec<Id>,
}

/// A strip of tabs the pad switches ([`tabs`]).
#[derive(Clone, Debug)]
pub struct Tabs {
    pub layer: LayerId,
    /// The strip: LT and RT switch it while the focus is in the pane it heads (the
    /// smallest part of the window around it).
    pub strip: Rect,
    /// The strip's id: tab `i`'s widget is `kit::tabs::tab_id(base, i)` (a click shows it).
    pub base: Id,
    /// How many tabs.
    pub count: usize,
    /// The tab shown now.
    pub current: Option<usize>,
}

#[derive(Clone, Debug, Default)]
struct Store {
    pass: u64,
    cur: Frame,
    prev: Frame,
}

fn store_id() -> Id {
    Id::unique("dzch.pad.registry")
}

/// Bounds, so a view that registers in a loop cannot grow the set without end.
const MAX_ITEMS: usize = 512;

fn with_cur(ctx: &Context, f: impl FnOnce(&mut Frame)) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Store>(store_id());
        rotate(s, pass);
        f(&mut s.cur);
    });
}

fn rotate(s: &mut Store, pass: u64) {
    if s.pass != pass {
        // A pass with nothing registered leaves nothing behind for the next.
        s.prev = if s.pass.checked_add(1) == Some(pass) {
            std::mem::take(&mut s.cur)
        } else {
            Frame::default()
        };
        s.cur = Frame::default();
        s.pass = pass;
    }
}

/// What the last frame registered.
pub fn last(ctx: &Context) -> Frame {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Store>(store_id());
        rotate(s, pass);
        s.prev.clone()
    })
}

/// What this frame has registered so far (for drawing at its end).
pub fn current(ctx: &Context) -> Frame {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_default::<Store>(store_id());
        rotate(s, pass);
        s.cur.clone()
    })
}

/// A list the controller walks row by row (called by `kit::VirtualList`).
pub fn list(ctx: &Context, slot: ListSlot) {
    if !slot.viewport.is_finite() {
        return;
    }
    with_cur(ctx, |f| {
        if f.lists.len() < MAX_ITEMS {
            f.lists.push(slot);
        }
    });
}

/// A part of the window a move prefers to stay in (a pane, a drawer, an index).
pub fn region(ctx: &Context, rect: Rect) {
    if !rect.is_finite() || rect.area() <= 0.0 {
        return;
    }
    with_cur(ctx, |f| {
        if f.regions.len() < MAX_ITEMS {
            f.regions.push(rect);
        }
    });
}

/// The widget `id` (in `layer`, at `rect`) is drawn over what its layer drew before it:
/// those widgets under it are out of the controller's reach, and so is the widget itself.
pub fn cover(ctx: &Context, id: Id, layer: LayerId, rect: Rect) {
    if !rect.is_finite() {
        return;
    }
    with_cur(ctx, |f| {
        if f.covers.len() < MAX_ITEMS {
            f.covers.push(Cover { id, layer, rect });
        }
    });
}

/// A value attached to a part of the screen: the view's X/Y commands get the value under
/// the controller's focus as their arguments (the mission of an offline card).
pub fn tag(ctx: &Context, rect: Rect, value: Value) {
    if !rect.is_finite() {
        return;
    }
    with_cur(ctx, |f| {
        if f.tags.len() < MAX_ITEMS {
            f.tags.push((rect, value));
        }
    });
}

/// The widget `id` takes Left and Right itself (a slider): a controller's Left and Right
/// reach it as arrow keys while it has the focus.
pub fn slider(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.sliders.len() < MAX_ITEMS {
            f.sliders.push(id);
        }
    });
}

/// The text field `id` holds a number: a controller's Left and Right step it (as the
/// arrow keys Down and Up), and the on-screen keyboard opens on its digits.
pub fn number(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.numbers.len() < MAX_ITEMS {
            f.numbers.push(id);
        }
    });
}

/// The text field `id` holds a number typed only (no stepping): the on-screen keyboard
/// opens on its digits, Left and Right still leave it.
pub fn digits(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.digits.len() < MAX_ITEMS {
            f.digits.push(id);
        }
    });
}

/// The disabled control `id` carries a hint: in gamepad mode the focus may rest on it,
/// and its hint shows (`kit::tooltip`).
pub fn tipped(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.tipped.len() < MAX_ITEMS {
            f.tipped.push(id);
        }
    });
}

/// The kit button `id` is disabled this frame (it senses hover only, so egui's own
/// widget list does not say it is a button).
pub fn dormant(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.dormant.len() < MAX_ITEMS {
            f.dormant.push(id);
        }
    });
}

/// The widget `id` takes Up and Down itself (a handle between stacked panes).
pub fn vslider(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.vsliders.len() < MAX_ITEMS {
            f.vsliders.push(id);
        }
    });
}

/// The widget `id` holds the pad's directions (a split handle grabbed with A): B lets it
/// go (an Escape it reads while it keeps the focus) instead of leaving the view.
pub fn held(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.held.len() < MAX_ITEMS {
            f.held.push(id);
        }
    });
}

/// The widget `id` (at `rect`) has a context menu (`kit::menu::context` says so): the
/// pad's Menu opens it when the focus is on it, or on the list row it is.
pub fn menu(ctx: &Context, id: Id, rect: Rect) {
    if !rect.is_finite() {
        return;
    }
    with_cur(ctx, |f| {
        if f.menus.len() < MAX_ITEMS {
            f.menus.push((id, rect));
        }
    });
}

/// The widget `id` is a dialog's close cross (`kit::Dialog` says so): a press never
/// lands on it.
pub fn cross(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.crosses.len() < MAX_ITEMS {
            f.crosses.push(id);
        }
    });
}

/// The widget `id` is where a press lands when nothing in its view or dialog has the
/// focus yet: the view's first field, a dialog's safe button. Call after adding it.
pub fn primary(ctx: &Context, id: Id) {
    with_cur(ctx, |f| {
        if f.primaries.len() < MAX_ITEMS {
            f.primaries.push(id);
        }
    });
}

/// `rect` (in `layer`) is drawn over the pane's content scrolled under it (a sticky
/// heading, widget `id`): what is under it is not in sight, and it is no stop itself.
pub fn sticky(ctx: &Context, id: Id, layer: LayerId, rect: Rect) {
    if !rect.is_finite() {
        return;
    }
    with_cur(ctx, |f| {
        if f.stickies.len() < MAX_ITEMS {
            f.stickies.push(Cover { id, layer, rect });
        }
    });
}

/// A strip of tabs (`kit::TabStrip` does it): while the pad's focus is in the pane the
/// strip heads, LT and RT press the tab before or after `current` of its `count`.
pub fn tabs(
    ctx: &Context,
    layer: LayerId,
    strip: Rect,
    base: Id,
    count: usize,
    current: Option<usize>,
) {
    if !strip.is_finite() || count == 0 || count > 64 {
        return;
    }
    with_cur(ctx, |f| {
        if f.tabs.len() < MAX_ITEMS {
            f.tabs.push(Tabs {
                layer,
                strip,
                base,
                count,
                current,
            });
        }
    });
}

/// The context menu the pad opened (the widget it belongs to), kept while it is open.
fn pad_menu_id() -> Id {
    Id::unique("dzch.pad.menu")
}

/// The pad opens `target`'s context menu (it is shown at the widget, not the pointer).
pub fn open_menu(ctx: &Context, target: Id) {
    egui::Popup::open_id(ctx, target.with("popup"));
    ctx.data_mut(|d| d.insert_temp(pad_menu_id(), target));
}

/// Where `target`'s context menu shows when the pad opened it: a line down `rect` (the
/// widget's) near its start, so the menu opens below it, or above it when there is no
/// room below; `None` when the pointer did (it shows at the pointer).
pub fn menu_anchor(ctx: &Context, target: Id, rect: Rect) -> Option<Rect> {
    let opened: Option<Id> = ctx.data(|d| d.get_temp(pad_menu_id()));
    if opened != Some(target) {
        return None;
    }
    if !egui::Popup::is_id_open(ctx, target.with("popup")) {
        ctx.data_mut(|d| d.remove::<Id>(pad_menu_id()));
        return None;
    }
    let x = rect.left() + (rect.width() * 0.25).min(size::ROW * 3.0);
    Some(Rect::from_x_y_ranges(x..=x, rect.y_range()))
}

/// Marks the scroll area `ui` is in as a reading pane (a server's details, an article):
/// the right stick scrolls it. Call inside the area's content, after the content.
pub fn reading_pane(ui: &mut Ui) {
    let id = ui.scope_id().with("dzch.pad.anchor");
    let clip = ui.clip_rect();
    let at = ui.cursor().min;
    // A widget of no size where the content ends: AccessKit's scroll requests aimed at
    // it scroll the pane it is in.
    let _ = ui.interact(
        Rect::from_min_size(at, egui::Vec2::ZERO),
        id,
        egui::Sense::hover(),
    );
    if !clip.is_finite() {
        return;
    }
    with_cur(ui.ctx(), |f| {
        if f.anchors.len() < MAX_ITEMS {
            f.anchors.push((id, clip));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_frame_is_read_and_a_skipped_frame_leaves_nothing() {
        let mut s = Store::default();
        rotate(&mut s, 1);
        s.cur
            .regions
            .push(Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0)));
        rotate(&mut s, 2);
        assert_eq!(s.prev.regions.len(), 1);
        assert!(s.cur.regions.is_empty());
        rotate(&mut s, 2);
        assert_eq!(s.prev.regions.len(), 1, "same pass: no rotation");
        s.cur.regions.push(Rect::NOTHING);
        rotate(&mut s, 5);
        assert!(s.prev.regions.is_empty(), "passes were skipped");
    }
}
