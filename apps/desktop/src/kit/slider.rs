//! `ui/slider`: a number chosen along a line. `h-control-sm`, a 4px track
//! (`rounded-full bg-raised`), the range `bg-accent/70`, a 12px accent thumb with the soft
//! glow. Drag or click to set, arrows and Page Up/Down to step, Home/End for the ends.
//! `changed()` on every move; `drag_stopped()` (or a key) is the commit.

use egui::{CornerRadius, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget, WidgetInfo};

use super::paint::{self, alpha, fade};
use crate::theme::{TokensExt, size};

/// `Slider::new(&mut volume, 0.0..=1.0).step(0.05)`.
#[must_use = "add it with ui.add(…)"]
pub struct Slider<'a> {
    value: &'a mut f64,
    min: f64,
    max: f64,
    step: f64,
    enabled: bool,
    width: Option<f32>,
    label: &'a str,
}

impl<'a> Slider<'a> {
    pub fn new(value: &'a mut f64, range: std::ops::RangeInclusive<f64>) -> Self {
        let (a, b) = (*range.start(), *range.end());
        Self {
            value,
            min: a.min(b),
            max: a.max(b),
            step: 0.01,
            enabled: true,
            width: None,
            label: "",
        }
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs();
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// The accessible name.
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }

    fn snap(&self, v: f64) -> f64 {
        let v = if v.is_finite() {
            v.clamp(self.min, self.max)
        } else {
            self.min
        };
        if self.step > 0.0 {
            let k = ((v - self.min) / self.step).round();
            (self.min + k * self.step).clamp(self.min, self.max)
        } else {
            v
        }
    }
}

const THUMB: f32 = 12.0;

impl Widget for Slider<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let w = self
            .width
            .unwrap_or_else(|| ui.available_width())
            .max(THUMB * 2.0);
        let sense = if self.enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL_SM), sense);
        let span = (self.max - self.min).max(f64::EPSILON);
        let track = Rect::from_min_max(
            Pos2::new(rect.left() + THUMB / 2.0, rect.center().y - 2.0),
            Pos2::new(rect.right() - THUMB / 2.0, rect.center().y + 2.0),
        );
        let before = *self.value;
        if self.enabled {
            if let Some(pos) = resp.interact_pointer_pos()
                && (resp.dragged() || resp.clicked() || resp.drag_started())
            {
                let k =
                    f64::from(((pos.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0));
                *self.value = self.snap(self.min + k * span);
            }
            if resp.has_focus() {
                let step = if self.step > 0.0 {
                    self.step
                } else {
                    span / 100.0
                };
                let (l, r, pu, pd, home, end) = ui.input(|i| {
                    use egui::Key;
                    (
                        i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown),
                        i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp),
                        i.key_pressed(Key::PageUp),
                        i.key_pressed(Key::PageDown),
                        i.key_pressed(Key::Home),
                        i.key_pressed(Key::End),
                    )
                });
                let v = *self.value;
                let next = if l {
                    v - step
                } else if r {
                    v + step
                } else if pu {
                    v + step * 10.0
                } else if pd {
                    v - step * 10.0
                } else if home {
                    self.min
                } else if end {
                    self.max
                } else {
                    v
                };
                *self.value = self.snap(next);
            }
        }
        #[allow(clippy::float_cmp)]
        if *self.value != before {
            resp.mark_changed();
        }
        let v = *self.value;
        let label = self.label;
        resp.widget_info(|| WidgetInfo::slider(true, v, label));
        if ui.is_rect_visible(rect) {
            let en = self.enabled;
            let k = (((v - self.min) / span).clamp(0.0, 1.0)) as f32;
            let p = ui.painter();
            p.rect_filled(track, CornerRadius::same(2), fade(t.raised, en));
            let x = track.left() + track.width() * k;
            let range = Rect::from_min_max(track.min, Pos2::new(x, track.max.y));
            p.rect_filled(range, CornerRadius::same(2), fade(alpha(t.accent, 0.7), en));
            let thumb = Rect::from_center_size(Pos2::new(x, rect.center().y), Vec2::splat(THUMB));
            if en {
                p.add(paint::glow_soft(&t).as_shape(thumb, CornerRadius::same(6)));
            }
            p.circle(
                thumb.center(),
                THUMB / 2.0,
                fade(t.accent, en),
                Stroke::new(1.0, fade(t.accent, en)),
            );
            if paint::keyboard_focus(ui, &resp) {
                p.circle_stroke(
                    thumb.center(),
                    THUMB / 2.0 + 2.0,
                    Stroke::new(2.0, alpha(t.accent, 0.5)),
                );
            }
        }
        if self.enabled {
            let icon = if resp.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            };
            resp.on_hover_cursor(icon)
        } else {
            resp
        }
    }
}
