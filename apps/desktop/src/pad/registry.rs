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
}

#[derive(Clone, Debug, Default)]
struct Store {
    pass: u64,
    cur: Frame,
    prev: Frame,
}

fn store_id() -> Id {
    Id::new("dzch.pad.registry")
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

/// Marks the scroll area `ui` is in as a reading pane (a server's details, an article):
/// the right stick scrolls it. Call inside the area's content, after the content.
pub fn reading_pane(ui: &mut Ui) {
    let id = ui.id().with("dzch.pad.anchor");
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
