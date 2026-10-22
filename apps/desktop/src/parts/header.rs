//! The top of a view and its figures: `PageHeader`, `Figure`, and the rolling count
//! (`rolling.svelte.ts`) a figure, the title bar and the status bar share.

use egui::{
    Align, Color32, CornerRadius, Id, Layout, Pos2, Rect, Response, Sense, Stroke, Ui, UiBuilder,
    Vec2, Widget, WidgetInfo, WidgetType,
};

use super::text::{draw, ph};
use crate::i18n::format;
use crate::kit::paint::{self, alpha};
use crate::kit::{Side, TipExt};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// How long a count takes to roll to a new value (`rolling(…, 400)`).
const ROLL_SECS: f64 = 0.4;

#[derive(Clone, Copy)]
struct Roll {
    from: f64,
    to: f64,
    at: f64,
}

/// A count that rolls to its new value instead of jumping (`rolling()`): the first value
/// is shown as it is, and so is every value when motion is off (`animation_time` 0).
/// Whole numbers only: the steps between are rounded. Repaints only while it rolls.
pub fn rolling(ui: &Ui, id: Id, value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let now = ui.input(|i| i.time);
    let still = ui.style().animation_time <= 0.0;
    let prev = ui.data(|d| d.get_temp::<Roll>(id));
    let roll = match prev {
        None => Roll {
            from: value,
            to: value,
            at: now,
        },
        Some(r) if r.to == value => r,
        Some(r) => {
            // Start from where the roll is now, not where it was going.
            let shown = current(&r, now);
            Roll {
                from: if still { value } else { shown },
                to: value,
                at: now,
            }
        }
    };
    if prev.map(|p| (p.to, p.at)) != Some((roll.to, roll.at)) {
        ui.data_mut(|d| d.insert_temp(id, roll));
    }
    let shown = current(&roll, now);
    if shown == value {
        value
    } else {
        ui.ctx().request_repaint();
        shown.round()
    }
}

fn current(r: &Roll, now: f64) -> f64 {
    let k = ((now - r.at) / ROLL_SECS).clamp(0.0, 1.0);
    if k >= 1.0 {
        return r.to;
    }
    let e = 1.0 - (1.0 - k).powi(3);
    r.from + (r.to - r.from) * e
}

/// The figure's number written as `format::num` writes it, the default for `amount`.
fn num(n: f64) -> String {
    format::num(n)
}

/// `Figure`: one figure in a header, a stencilled label over a mono value. Clickable, it
/// is a shortcut to what it counts (a filter, a sort): a dotted underline on hover, and an
/// accent bar under it while `active`. A count given as `amount` rolls to new values.
#[must_use = "add it with ui.add(…)"]
pub struct Figure<'a> {
    label: &'a str,
    value: &'a str,
    amount: Option<f64>,
    format: fn(f64) -> String,
    tone: Option<Color32>,
    tip: &'a str,
    secondary: bool,
    active: bool,
    clickable: bool,
}

impl<'a> Figure<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            value: "",
            amount: None,
            format: num,
            tone: None,
            tip: "",
            secondary: false,
            active: false,
            clickable: false,
        }
    }

    /// A ready string ("54 ms", "62%").
    pub fn value(mut self, v: &'a str) -> Self {
        self.value = v;
        self
    }

    /// A count: it rolls to a new value instead of jumping. `None` shows a dash.
    pub fn amount(mut self, n: impl Into<Option<f64>>) -> Self {
        self.amount = Some(n.into().unwrap_or(f64::NAN));
        self
    }

    /// How an `amount` is written (`format::num` by default; `format::compact`…).
    pub fn format(mut self, f: fn(f64) -> String) -> Self {
        self.format = f;
        self
    }

    pub fn tone(mut self, c: Color32) -> Self {
        self.tone = Some(c);
        self
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }

    /// Gives way when the window is 1240px or narrower.
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }

    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// A shortcut to what it counts: `clicked()` on the response.
    pub fn clickable(mut self) -> Self {
        self.clickable = true;
        self
    }
}

impl Widget for Figure<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        if self.secondary && ui.ctx().content_rect().width() <= 1240.0 {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }
        let tk = ui.tokens();
        let id = ui.next_auto_id();
        let shown = match self.amount {
            Some(n) if n.is_nan() => format::NONE.to_owned(),
            Some(n) => (self.format)(rolling(ui, id.with("roll"), n)),
            None => self.value.to_owned(),
        };
        // `label-stencil`: display 700 (the 800 instance), text-2xs, tracked 0.12em.
        let label = paint::tracked(
            ui,
            self.label,
            font(Face::DisplayBold, size::TEXT_2XS),
            Color32::PLACEHOLDER,
            0.12,
            true,
            None,
        );
        let value = ph(ui, &shown, font(Face::MonoSemibold, size::TEXT_SM));
        let w = label.size().x.max(value.size().x);
        // label (11px, leading 1) · gap-0.5 · value (13px, leading 1.5)
        let h = 11.0 + 2.0 + 19.5;
        let sense = if self.clickable {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), sense);
        if ui.is_rect_visible(rect) {
            let hover = self.clickable && resp.hovered();
            let lc = if self.active {
                tk.accent
            } else if hover {
                tk.fg_muted
            } else {
                tk.fg_faint
            };
            let p = ui.painter();
            draw(p, &label, rect.left(), rect.top() + 5.5, lc);
            let vc = self.tone.unwrap_or(tk.fg);
            let vy = rect.top() + 13.0 + 9.75;
            draw(p, &value, rect.left(), vy, vc);
            if hover {
                // decoration-dotted underline-offset-3
                let uy = vy + 7.5;
                let mut x = rect.left();
                while x < rect.left() + value.size().x {
                    p.hline(
                        Rangef::new(x, (x + 1.0).min(rect.left() + value.size().x)),
                        uy,
                        Stroke::new(1.0, vc),
                    );
                    x += 2.0;
                }
            }
            if self.clickable && self.active {
                p.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left(), rect.bottom() + 2.0),
                        Vec2::new(rect.width(), 2.0),
                    ),
                    CornerRadius::same(1),
                    tk.accent,
                );
            }
        }
        let (active, label_s) = (self.active, self.label);
        resp.widget_info(|| {
            if self.clickable {
                WidgetInfo::selected(WidgetType::Button, true, active, label_s)
            } else {
                WidgetInfo::labeled(WidgetType::Label, true, label_s)
            }
        });
        let resp = if self.clickable {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        };
        if self.tip.is_empty() {
            resp
        } else {
            resp.tip_kbd(self.tip, "", Side::Bottom)
        }
    }
}

use egui::Rangef;

/// `PageHeader`: the top of every view: its name in the display face, its figures, its
/// actions on the right, and its toolbar (search, filters) under. Below 760px of its own
/// width the figures take a line of their own under the name and the actions.
pub struct PageHeader<'a> {
    title: &'a str,
}

impl<'a> PageHeader<'a> {
    pub fn new(title: &'a str) -> Self {
        Self { title }
    }

    /// `stats` adds `Figure`s, `actions` the buttons (in reading order), `toolbar` the
    /// second row, if any.
    pub fn show(
        self,
        ui: &mut Ui,
        stats: impl FnOnce(&mut Ui),
        actions: impl FnOnce(&mut Ui),
        toolbar: Option<impl FnOnce(&mut Ui)>,
    ) -> Response {
        let tk = ui.tokens();
        let ground = ui.painter().add(egui::Shape::Noop);
        let full = ui.available_rect_before_wrap();
        let id = ui.next_auto_id().with("page-header");
        let inner = Rect::from_min_max(
            Pos2::new(full.left() + size::PAD, full.top() + 10.0),
            Pos2::new(full.right() - size::PAD, full.bottom()),
        );
        let narrow = full.width() < 760.0;

        // The actions' width is last frame's: placed in reading order from the right.
        let actions_w: f32 = ui.data(|d| d.get_temp(id.with("aw"))).unwrap_or(0.0);
        // The name row is as tall as its tallest cell (`items-end`): the name (30px,
        // `leading-none`), the figures, the actions; last frame's heights.
        let cells_h: f32 = ui.data(|d| d.get_temp(id.with("ch"))).unwrap_or(0.0);
        let row_h = cells_h.max(30.0);
        let title_g = paint::tracked(
            ui,
            self.title,
            font(Face::DisplayBold, size::TEXT_2XL),
            Color32::PLACEHOLDER,
            0.02,
            true,
            Some((inner.width() - actions_w - 24.0).max(40.0)),
        );
        let title_rect = Rect::from_min_size(
            Pos2::new(inner.left(), inner.top() + row_h - 30.0),
            Vec2::new(title_g.size().x, 30.0),
        );
        ui.painter().galley(
            Pos2::new(
                title_rect.left(),
                title_rect.bottom() - title_g.size().y + title_g.size().y * 0.12 - 1.0,
            ),
            title_g.clone(),
            tk.fg,
        );

        // Actions, right-aligned, bottoms on the row's.
        let ax = inner.right() - actions_w;
        let mut aui = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    Pos2::new(ax, inner.top()),
                    Pos2::new(inner.right() + 400.0, inner.top() + row_h),
                ))
                .layout(Layout::left_to_right(Align::Max))
                .id_salt("actions"),
        );
        aui.spacing_mut().item_spacing.x = 4.0;
        actions(&mut aui);
        let used = aui.min_rect().width().max(0.0);
        let actions_h = aui.min_rect().height().max(0.0);
        if (used - actions_w).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id.with("aw"), used));
            ui.ctx().request_discard("page header actions width");
        }

        // Figures: beside the name, or under it when narrow.
        let (sx, sy, sw) = if narrow {
            (inner.left(), inner.top() + row_h + 8.0, inner.width())
        } else {
            let x = title_rect.right() + 24.0;
            (x, inner.top(), (inner.right() - used - 24.0 - x).max(0.0))
        };
        // The figures' rows are each one figure tall (last frame's, at least 30): egui
        // gives every wrapped row the first row's height, so a first row as tall as the
        // name row would make the block taller each frame once the figures wrap. The
        // block sits on the row's bottom (`items-end`), at last frame's height.
        let band: f32 = ui.data(|d| d.get_temp(id.with("fb"))).unwrap_or(30.0);
        let block: f32 = ui.data(|d| d.get_temp(id.with("sh"))).unwrap_or(band);
        let top = if narrow {
            sy
        } else {
            sy + (row_h - block).max(0.0)
        };
        let mut sui = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    Pos2::new(sx, top),
                    Vec2::new(sw.max(1.0), band),
                ))
                .layout(Layout::left_to_right(Align::Max).with_main_wrap(true))
                .id_salt("stats"),
        );
        sui.spacing_mut().item_spacing = Vec2::new(20.0, 4.0);
        stats(&mut sui);
        let measured = sui.min_rect().height().max(0.0);
        let one_row = ((measured + 4.0) / (band + 4.0)).round() <= 1.0;
        if one_row && measured > 0.0 && (measured.max(30.0) - band).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id.with("fb"), measured.max(30.0)));
            ui.ctx().request_discard("page header figure height");
        }
        if (measured - block).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id.with("sh"), measured));
            ui.ctx().request_discard("page header figures height");
        }
        let stats_h = if narrow { 0.0 } else { measured };
        let cells = actions_h.max(stats_h);
        if (cells - cells_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(id.with("ch"), cells));
            ui.ctx().request_discard("page header row height");
        }
        let stats_bottom = if sui.min_rect().height() > 0.0 {
            sui.min_rect().bottom()
        } else {
            sy
        };
        let mut bottom = (inner.top() + row_h).max(if narrow { stats_bottom } else { 0.0 });

        if let Some(toolbar) = toolbar {
            let mut tui = ui.new_child(
                UiBuilder::new()
                    .max_rect(Rect::from_min_max(
                        Pos2::new(inner.left(), bottom + 8.0),
                        Pos2::new(inner.right(), full.bottom().max(bottom + 400.0)),
                    ))
                    .layout(Layout::top_down(Align::Min))
                    .id_salt("toolbar"),
            );
            toolbar(&mut tui);
            bottom = tui.min_rect().bottom().max(bottom + 8.0);
        }
        // `pb-2` and the 1px `border-b` under it, on whole pixels as the browser snaps it.
        let rect = Rect::from_min_max(full.min, Pos2::new(full.right(), (bottom + 9.0).floor()));
        let resp = ui.allocate_rect(rect, Sense::hover());
        ui.painter().set(
            ground,
            egui::Shape::rect_filled(rect, CornerRadius::ZERO, alpha(tk.bg, 0.4)),
        );
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, tk.border),
        );
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolls_ease_out_and_land() {
        let r = Roll {
            from: 0.0,
            to: 100.0,
            at: 0.0,
        };
        assert_eq!(current(&r, 0.0), 0.0);
        assert!(current(&r, 0.2) > 50.0);
        assert_eq!(current(&r, 0.4), 100.0);
        assert_eq!(current(&r, 9.0), 100.0);
    }
}
