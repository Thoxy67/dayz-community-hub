//! Controller navigation (= `lib/gamepad`): the hub's gamepad thread reads the pads
//! (gilrs) and sends actions; [`nav`] acts on them each frame over the widgets egui
//! drew, [`spatial`] picks where a direction leads, [`registry`] carries what the views
//! and the kit tell the pad beyond egui's widgets.
//!
//! | for | call |
//! |---|---|
//! | the frame loop | [`frame`]`(cx)` before the window's keys; [`paint`]`(ctx, cx)` after drawing |
//! | a list walked row by row | `kit::VirtualList::cursor(Some(i))` (registers itself) |
//! | a pane a move prefers to stay in | [`region`]`(ctx, rect)` (`kit::Split` does it for its panes) |
//! | something drawn over its layer | [`cover`]`(ctx, id, layer, rect)` (the narrow details drawer) |
//! | a value for the view's X/Y | [`tag`]`(ctx, rect, json)` (an offline card's mission) |
//! | what the right stick scrolls | [`reading_pane`]`(ui)` at the end of a scroll area's content |
//! | a widget that takes Left/Right | [`slider`]`(ctx, id)` (`kit::Slider` does it) |
//! | a pad action by name | [`action_named`]`("dpad_down")`, `store::pad::Pad::inject` |
//!
//! In gamepad mode the focused widget wears a ring (token `accent`) and the window is
//! drawn at the pad scale (`app.rs`), the rail collapses below 1200 points
//! ([`rail_collapsed`]) and its "close the game" block hides (`shell/rail.rs`).

pub mod nav;
pub mod registry;
pub mod spatial;

pub use nav::{
    Focus, PadNav, ViewHints, action_named, describe, frame, paint, rail_collapsed, view_hints,
};
pub use registry::{ListSlot, cover, list, reading_pane, region, slider, tag};
