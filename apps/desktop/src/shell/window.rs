//! The undecorated window's resize edges: a few points along each side and a larger
//! square at each corner show the resize cursor, and a press there hands the resize to
//! the window manager (`ViewportCommand::BeginResize`). None while maximised or
//! full-screen. Read from the raw pointer at the end of the frame, so it wins over what
//! is drawn under the edge.

use egui::viewport::ResizeDirection;
use egui::{Context, CursorIcon, Pos2, Rect, ViewportCommand};

use crate::app::Ctx;

/// How thick an edge is, and how far a corner reaches along its sides (points).
const EDGE: f32 = 4.0;
const CORNER: f32 = 10.0;

/// The edge or corner under `p` in a window of `r`, if any.
pub fn direction(r: Rect, p: Pos2) -> Option<ResizeDirection> {
    if !r.expand(1.0).contains(p) {
        return None;
    }
    let left = p.x - r.left() <= EDGE;
    let right = r.right() - p.x <= EDGE;
    let top = p.y - r.top() <= EDGE;
    let bottom = r.bottom() - p.y <= EDGE;
    let near_left = p.x - r.left() <= CORNER;
    let near_right = r.right() - p.x <= CORNER;
    let near_top = p.y - r.top() <= CORNER;
    let near_bottom = r.bottom() - p.y <= CORNER;
    Some(match () {
        _ if (top && near_left) || (left && near_top) => ResizeDirection::NorthWest,
        _ if (top && near_right) || (right && near_top) => ResizeDirection::NorthEast,
        _ if (bottom && near_left) || (left && near_bottom) => ResizeDirection::SouthWest,
        _ if (bottom && near_right) || (right && near_bottom) => ResizeDirection::SouthEast,
        _ if top => ResizeDirection::North,
        _ if bottom => ResizeDirection::South,
        _ if left => ResizeDirection::West,
        _ if right => ResizeDirection::East,
        _ => return None,
    })
}

fn cursor(d: ResizeDirection) -> CursorIcon {
    match d {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

pub(super) fn resize_edges(ctx: &Context, cx: &Ctx) {
    let (maximized, fullscreen) = ctx.input(|i| {
        let v = i.viewport();
        (
            v.maximized.unwrap_or(cx.stores.app.maximized),
            v.fullscreen.unwrap_or(false),
        )
    });
    if maximized || fullscreen {
        return;
    }
    let r = ctx.content_rect();
    let (pos, pressed, dragging) = ctx.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.pointer.is_decidedly_dragging(),
        )
    });
    let Some(p) = pos else {
        return;
    };
    if dragging {
        return;
    }
    let Some(d) = direction(r, p) else {
        return;
    };
    ctx.set_cursor_icon(cursor(d));
    if pressed {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(d));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    #[test]
    fn edges_and_corners() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0));
        assert_eq!(direction(r, pos2(50.0, 50.0)), None);
        assert_eq!(direction(r, pos2(1.0, 50.0)), Some(ResizeDirection::West));
        assert_eq!(direction(r, pos2(99.0, 50.0)), Some(ResizeDirection::East));
        assert_eq!(direction(r, pos2(50.0, 0.5)), Some(ResizeDirection::North));
        assert_eq!(direction(r, pos2(50.0, 99.5)), Some(ResizeDirection::South));
        assert_eq!(
            direction(r, pos2(2.0, 8.0)),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(
            direction(r, pos2(95.0, 99.0)),
            Some(ResizeDirection::SouthEast)
        );
        assert_eq!(direction(r, pos2(200.0, 50.0)), None);
    }
}
