//! The undecorated window's resize edges: a few points along each side and a larger
//! square at each corner show the resize cursor, and a press there hands the resize to
//! the window manager (`ViewportCommand::BeginResize`). None while maximised or
//! full-screen. Read from the raw pointer at the end of the frame, so it wins over what
//! is drawn under the edge.
//!
//! A move or resize handed to the window manager ([`hand_over`]) takes the pointer: the
//! button's release goes to the window manager, not to the window (Wayland, Windows).
//! egui would then think the button held for good (no resize cursor, no new drag), so
//! the next frame is given the release ([`release_after_hand_over`]).

use egui::viewport::ResizeDirection;
use egui::{Context, CursorIcon, Id, PointerButton, Pos2, RawInput, Rect, ViewportCommand};

use crate::app::Ctx;

/// How thick an edge is, and how far a corner reaches along its sides (points).
const EDGE: f32 = 4.0;
const CORNER: f32 = 10.0;

fn handed_over_id() -> Id {
    Id::new("dzch.window.handed_over")
}

/// Hand the pointer to the window manager: a move (`StartDrag`) or a resize
/// (`BeginResize`).
pub fn hand_over(ctx: &Context, cmd: ViewportCommand) {
    ctx.send_viewport_cmd(cmd);
    ctx.data_mut(|d| d.insert_temp(handed_over_id(), true));
}

/// Before a frame: after a hand-over, the primary button's release the window will not
/// get (unless this input already has it).
pub fn release_after_hand_over(ctx: &Context, raw: &mut RawInput) {
    let handed = ctx.data_mut(|d| d.remove_temp::<bool>(handed_over_id()));
    if handed != Some(true) {
        return;
    }
    let released = raw.events.iter().any(|e| {
        matches!(
            e,
            egui::Event::PointerButton {
                button: PointerButton::Primary,
                pressed: false,
                ..
            }
        )
    });
    let down = ctx.input(|i| i.pointer.primary_down());
    if released || !down {
        return;
    }
    let pos = ctx.input(|i| i.pointer.latest_pos()).unwrap_or_default();
    raw.events.push(egui::Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: ctx.input(|i| i.modifiers),
    });
}

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
        hand_over(ctx, ViewportCommand::BeginResize(d));
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

    /// After a hand-over, a button the window manager kept is released; without one,
    /// nothing is made up.
    #[test]
    fn a_hand_over_releases_the_button() {
        let ctx = Context::default();
        let press = |pressed| egui::Event::PointerButton {
            pos: pos2(5.0, 5.0),
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut raw = RawInput {
            events: vec![egui::Event::PointerMoved(pos2(5.0, 5.0)), press(true)],
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| hand_over(ui.ctx(), ViewportCommand::StartDrag));
        out.textures_delta.clear();
        assert!(ctx.input(|i| i.pointer.primary_down()));
        raw = RawInput::default();
        release_after_hand_over(&ctx, &mut raw);
        assert_eq!(raw.events, [press(false)]);
        let mut out = ctx.run_ui(raw, |_| {});
        out.textures_delta.clear();
        assert!(!ctx.input(|i| i.pointer.primary_down()));
        // Once only.
        let mut raw = RawInput::default();
        release_after_hand_over(&ctx, &mut raw);
        assert!(raw.events.is_empty());
    }
}
