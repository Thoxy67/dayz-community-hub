//! `ui/separator` and `ui/split` (Split, Splitter).
//!
//! A split is a main area and a side pane with a handle between them. One pane keeps its
//! size and the other takes what is left; the size is remembered under the split's id
//! (egui memory, persisted with the window's state). Too narrow for both side by side
//! (`min + keep`), the panes stack, main on top, and the stacked height is remembered
//! apart. The handle: 4px of hit area, a 3×16 grip that brightens under the pointer and
//! turns accent with the keyboard; drag, arrows (8px, 32 with Shift), double-click or
//! Home to put it back.

use egui::{CornerRadius, Id, Pos2, Rect, Response, Sense, Ui, UiBuilder, Vec2, Widget};

use super::paint;
use crate::theme::TokensExt;

/// `ui/separator`: a hairline between two groups (`bg-border`); vertical ones are 16 tall.
#[must_use = "add it with ui.add(…)"]
pub struct Separator {
    vertical: bool,
}

impl Separator {
    pub fn horizontal() -> Self {
        Self { vertical: false }
    }

    pub fn vertical() -> Self {
        Self { vertical: true }
    }
}

impl Widget for Separator {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let size = if self.vertical {
            Vec2::new(1.0, 16.0)
        } else {
            Vec2::new(ui.available_width(), 1.0)
        };
        let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::ZERO, t.border);
        resp
    }
}

/// Which side the sized pane sits on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    Start,
    #[default]
    End,
}

/// `Split::new("servers.detail", 340.0).show(ui, |ui| list(ui), |ui| detail(ui))`.
#[must_use = "call show(…)"]
pub struct Split<'a> {
    id: Id,
    /// Side by side (true) or one above the other.
    vertical: bool,
    pane: Pane,
    initial: f32,
    min: f32,
    max: f32,
    keep: f32,
    label: &'a str,
}

impl<'a> Split<'a> {
    /// Side by side, the sized pane at the end, `initial` wide.
    pub fn new(id_salt: impl egui::AsId, initial: f32) -> Self {
        Self {
            id: Id::new(id_salt),
            vertical: true,
            pane: Pane::End,
            initial,
            min: 180.0,
            max: 640.0,
            keep: 340.0,
            label: "",
        }
    }

    /// One above the other (`orientation="horizontal"`).
    pub fn stacked(mut self) -> Self {
        self.vertical = false;
        self
    }

    pub fn pane(mut self, p: Pane) -> Self {
        self.pane = p;
        self
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    /// Pixels the main area never drops below; also where the panes stack.
    pub fn keep(mut self, keep: f32) -> Self {
        self.keep = keep;
        self
    }

    /// The handle's accessible name.
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }

    /// Lays both panes out in the whole space on offer. Each closure gets a `Ui` exactly
    /// its pane's size (clipped).
    pub fn show(self, ui: &mut Ui, main: impl FnOnce(&mut Ui), aside: impl FnOnce(&mut Ui)) {
        let full = ui.available_rect_before_wrap();
        ui.allocate_rect(full, Sense::hover());
        let stacked = self.vertical && full.width() > 0.0 && full.width() < self.min + self.keep;
        let side_by_side = self.vertical && !stacked;
        let key = if stacked {
            self.id.with("stacked")
        } else {
            self.id
        };
        let available = if side_by_side {
            full.width()
        } else {
            full.height()
        };
        let floor = if stacked { 96.0 } else { self.min };
        let roof = if stacked {
            floor.max(full.height() - 140.0)
        } else {
            self.max
        };
        let spare = if stacked { 140.0 } else { self.keep };
        let first = if stacked {
            (full.height() * 0.42).round()
        } else {
            self.initial
        };
        let ceiling = if available > 0.0 {
            floor.max(roof.min(available - spare))
        } else {
            roof
        };
        let saved: Option<f32> = ui.data_mut(|d| d.get_persisted(key));
        let size = saved.unwrap_or(first).clamp(floor, ceiling.max(floor));
        let at_start = self.pane == Pane::Start;
        const HANDLE: f32 = 4.0;

        let (sized, handle, flexible) = if side_by_side {
            if at_start {
                let a = Rect::from_min_size(full.min, Vec2::new(size, full.height()));
                let h = Rect::from_min_size(
                    Pos2::new(a.right(), full.top()),
                    Vec2::new(HANDLE, full.height()),
                );
                let b = Rect::from_min_max(Pos2::new(h.right(), full.top()), full.max);
                (a, h, b)
            } else {
                let a = Rect::from_min_max(Pos2::new(full.right() - size, full.top()), full.max);
                let h = Rect::from_min_size(
                    Pos2::new(a.left() - HANDLE, full.top()),
                    Vec2::new(HANDLE, full.height()),
                );
                let b = Rect::from_min_max(full.min, Pos2::new(h.left(), full.bottom()));
                (a, h, b)
            }
        } else if at_start {
            let a = Rect::from_min_size(full.min, Vec2::new(full.width(), size));
            let h = Rect::from_min_size(
                Pos2::new(full.left(), a.bottom()),
                Vec2::new(full.width(), HANDLE),
            );
            let b = Rect::from_min_max(Pos2::new(full.left(), h.bottom()), full.max);
            (a, h, b)
        } else {
            let a = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - size), full.max);
            let h = Rect::from_min_size(
                Pos2::new(full.left(), a.top() - HANDLE),
                Vec2::new(full.width(), HANDLE),
            );
            let b = Rect::from_min_max(full.min, Pos2::new(full.right(), h.top()));
            (a, h, b)
        };

        // Each pane is a region a controller's moves prefer to stay in.
        crate::pad::region(ui.ctx(), flexible);
        crate::pad::region(ui.ctx(), sized);
        let mut main_ui = ui.new_child(UiBuilder::new().id_salt((self.id, 0u8)).max_rect(flexible));
        main_ui.set_clip_rect(flexible.intersect(ui.clip_rect()));
        main(&mut main_ui);
        let mut aside_ui = ui.new_child(UiBuilder::new().id_salt((self.id, 1u8)).max_rect(sized));
        aside_ui.set_clip_rect(sized.intersect(ui.clip_rect()));
        aside(&mut aside_ui);

        let s = Splitter::new(self.id.with(("handle", stacked)), side_by_side)
            .label(self.label)
            .show(ui, handle);
        if s.reset {
            ui.data_mut(|d| d.remove::<f32>(key));
        } else if s.delta != 0.0 {
            let next = if at_start {
                size + s.delta
            } else {
                size - s.delta
            };
            let next = next.clamp(floor, ceiling.max(floor));
            ui.data_mut(|d| d.insert_persisted(key, next));
        }
    }

    /// The size remembered for a split (for a prefs store that saves it elsewhere).
    pub fn saved(ctx: &egui::Context, id_salt: impl egui::AsId) -> Option<f32> {
        ctx.data_mut(|d| d.get_persisted(Id::new(id_salt)))
    }

    /// Sets the remembered size (restoring it from a prefs store).
    pub fn restore(ctx: &egui::Context, id_salt: impl egui::AsId, size: f32) {
        if size.is_finite() {
            ctx.data_mut(|d| d.insert_persisted(Id::new(id_salt), size));
        }
    }
}

/// What a drag on a [`Splitter`] asked for this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct SplitterOutput {
    /// Pixels along the axis of movement.
    pub delta: f32,
    /// Double-click or Home: back to the default.
    pub reset: bool,
}

/// The handle alone, for a layout that sizes its panes itself.
pub struct Splitter<'a> {
    id: Id,
    vertical: bool,
    label: &'a str,
}

impl<'a> Splitter<'a> {
    /// `vertical`: a handle between two panes side by side (it moves left and right).
    pub fn new(id: Id, vertical: bool) -> Self {
        Self {
            id,
            vertical,
            label: "",
        }
    }

    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }

    /// Draws the handle in `rect` (4px across) and reads the gesture.
    pub fn show(self, ui: &mut Ui, rect: Rect) -> SplitterOutput {
        let t = ui.tokens();
        // The hit area reaches 4px past each side of the hairline.
        let hit = if self.vertical {
            rect.expand2(Vec2::new(4.0, 0.0))
        } else {
            rect.expand2(Vec2::new(0.0, 4.0))
        };
        let resp = ui.interact(hit, self.id, Sense::click_and_drag());
        let label = self.label;
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, label));
        let mut out = SplitterOutput::default();
        if resp.dragged() {
            let d = resp.drag_delta();
            out.delta = if self.vertical { d.x } else { d.y };
        }
        if resp.double_clicked() {
            out.reset = true;
        }
        // A controller grabs the handle with A (a click while it has the focus): then its
        // directions move it along its axis, held down they keep moving it, and B or A
        // lets it go.
        let held_id = self.id.with("pad-held");
        let mut held = ui.data(|d| d.get_temp::<bool>(held_id)).unwrap_or(false);
        // Not a click of the pointer (an AccessKit one: the pad's A, Enter).
        let pointer = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
        if resp.clicked() && !pointer && resp.has_focus() {
            held = !held;
        }
        if held
            && (!resp.has_focus()
                || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)))
        {
            held = false;
        }
        ui.data_mut(|d| d.insert_temp(held_id, held));
        if held {
            let ctx = ui.ctx();
            crate::pad::registry::held(ctx, self.id);
            if self.vertical {
                crate::pad::slider(ctx, self.id);
            } else {
                crate::pad::vslider(ctx, self.id);
            }
        }
        if resp.has_focus() {
            let (back, fwd, home, shift) = ui.input(|i| {
                use egui::Key;
                let (b, f) = if self.vertical {
                    (Key::ArrowLeft, Key::ArrowRight)
                } else {
                    (Key::ArrowUp, Key::ArrowDown)
                };
                (
                    i.key_pressed(b),
                    i.key_pressed(f),
                    i.key_pressed(Key::Home),
                    i.modifiers.shift,
                )
            });
            let step = if shift { 32.0 } else { 8.0 };
            if back {
                out.delta = -step;
            } else if fwd {
                out.delta = step;
            } else if home {
                out.reset = true;
            }
        }
        let focus =
            held || paint::keyboard_focus(ui, &resp) || (resp.has_focus() && resp.dragged());
        let hover = paint::ease(ui, self.id.with("hover"), resp.hovered() || resp.dragged());
        let grip_size = if self.vertical {
            Vec2::new(3.0, 16.0)
        } else {
            Vec2::new(16.0, 3.0)
        };
        let grip = Rect::from_center_size(rect.center(), grip_size);
        let c = if focus {
            t.accent
        } else {
            paint::lerp(t.border, t.border_strong, hover)
        };
        ui.painter().rect_filled(grip, CornerRadius::same(2), c);
        let _ = resp.on_hover_cursor(if self.vertical {
            egui::CursorIcon::ResizeColumn
        } else {
            egui::CursorIcon::ResizeRow
        });
        out
    }
}
