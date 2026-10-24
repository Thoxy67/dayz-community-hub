//! `ui/segmented`: a few choices of which exactly one holds, side by side in one frame
//! (`rounded-sm border bg-bg`, hairlines between), the chosen one lit
//! (`bg-accent/15 text-accent`). Arrows move the choice.

use egui::{
    Color32, CornerRadius, Id, Rect, Response, Sense, Ui, Vec2, Widget, WidgetInfo, WidgetType,
};

use super::input::FieldSize;
use super::paint::{self, alpha, fade};
use super::tooltip::TipExt;
use crate::icons::Icon;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// One choice.
#[derive(Clone, Debug)]
pub struct Segment<'a, V> {
    pub value: V,
    pub label: &'a str,
    pub icon: Option<Icon>,
    /// A longer word for the hint, when the label is an icon or an abbreviation.
    pub title: &'a str,
    pub disabled: bool,
}

impl<'a, V> Segment<'a, V> {
    pub fn new(value: V, label: &'a str) -> Self {
        Self {
            value,
            label,
            icon: None,
            title: "",
            disabled: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

/// The widget of segment `i` of the control whose response id is `base` (a view makes
/// one the pad's landing spot).
pub fn segment_id(base: Id, i: usize) -> Id {
    base.with(("seg", i))
}

/// `Segmented::new(&mut mode, &segments)`; `changed()` when the choice moved.
#[must_use = "add it with ui.add(…)"]
pub struct Segmented<'a, V: PartialEq + Clone> {
    value: &'a mut V,
    options: &'a [Segment<'a, V>],
    size: FieldSize,
    icon_only: bool,
    fill: bool,
}

impl<'a, V: PartialEq + Clone> Segmented<'a, V> {
    pub fn new(value: &'a mut V, options: &'a [Segment<'a, V>]) -> Self {
        Self {
            value,
            options,
            size: FieldSize::Sm,
            icon_only: false,
            fill: false,
        }
    }

    pub fn size(mut self, s: FieldSize) -> Self {
        self.size = s;
        self
    }

    /// Draw the icons alone; the labels become the hints and the accessible names.
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Stretch to the width on offer: each choice keeps its natural width and the spare
    /// room is shared equally (short of room, the widths shrink in proportion).
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }
}

impl<V: PartialEq + Clone> Widget for Segmented<'_, V> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let (h, text) = match self.size {
            FieldSize::Sm => (size::control(), size::TEXT_XS),
            FieldSize::Xs => (size::control_sm(), size::TEXT_2XS),
        };
        let f = font(Face::Sans, text);
        // Widths first: each item's natural width, plus a share of the spare room when
        // filling, so a short word ("Tout") is never cut to pay for a long one.
        let natural: Vec<f32> = self
            .options
            .iter()
            .map(|o| {
                if self.icon_only {
                    h
                } else {
                    let tw = paint::galley(ui, o.label, f.clone(), t.fg).size().x;
                    16.0 + tw + o.icon.map_or(0.0, |_| size::ICON + 4.0)
                }
            })
            .collect();
        let total_natural: f32 = natural.iter().sum::<f32>() + 2.0;
        let w = if self.fill {
            ui.available_width()
        } else {
            total_natural
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let base = resp.id;
        let p = ui.painter().clone();
        paint::plate(&p, rect, size::RADIUS_SM, t.bg, t.border);
        let widths = fill_widths(&natural, w - 2.0);
        let mut x = rect.left() + 1.0;
        let mut moved: Option<V> = None;
        let mut focus_idx: Option<usize> = None;
        let current = self.options.iter().position(|o| o.value == *self.value);
        for (i, o) in self.options.iter().enumerate() {
            let iw = if self.fill {
                widths.get(i).copied().unwrap_or(h)
            } else {
                natural.get(i).copied().unwrap_or(h)
            };
            let r = Rect::from_min_size(egui::pos2(x, rect.top() + 1.0), Vec2::new(iw, h - 2.0));
            x += iw;
            let en = !o.disabled;
            let id = segment_id(base, i);
            let sense = if en { Sense::click() } else { Sense::hover() };
            let ir = ui.interact(r, id, sense);
            let label = o.label;
            let checked = o.value == *self.value;
            ir.widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, en, checked, label));
            if ir.clicked() && !checked {
                moved = Some(o.value.clone());
            }
            if ir.has_focus() {
                focus_idx = Some(i);
            }
            let hover = paint::ease(ui, id.with("hover"), en && ir.hovered());
            let on = paint::ease(ui, id.with("on"), checked);
            let fill = paint::lerp(
                paint::lerp(Color32::TRANSPARENT, t.raised, hover),
                alpha(t.accent, 0.15),
                on,
            );
            // The plate's corners are the frame's; the inner ones are square.
            let first = i == 0;
            let last = i + 1 == self.options.len();
            let cr = CornerRadius {
                nw: if first { 3 } else { 0 },
                sw: if first { 3 } else { 0 },
                ne: if last { 3 } else { 0 },
                se: if last { 3 } else { 0 },
            };
            p.rect_filled(r, cr, fill);
            if !last {
                p.vline(r.right(), r.y_range(), egui::Stroke::new(1.0, t.border));
            }
            let ink = fade(
                paint::lerp(paint::lerp(t.fg_muted, t.fg, hover), t.accent, on),
                en,
            );
            let g = (!self.icon_only)
                .then(|| paint::galley_trunc(ui, o.label, f.clone(), ink, (iw - 16.0).max(0.0)));
            let content = o.icon.map_or(0.0, |_| size::ICON)
                + g.as_ref().map_or(0.0, |g| g.size().x)
                + if o.icon.is_some() && g.is_some() {
                    4.0
                } else {
                    0.0
                };
            let mut cx = r.center().x - content / 2.0;
            if let Some(icon) = o.icon {
                icon.paint(
                    &p,
                    Rect::from_min_size(
                        egui::pos2(cx, r.center().y - size::ICON / 2.0),
                        Vec2::splat(size::ICON),
                    ),
                    ink,
                );
                cx += size::ICON + 4.0;
            }
            if let Some(g) = &g {
                paint::text_at(&p, g, cx, r.center().y, ink);
            }
            if paint::keyboard_focus(ui, &ir) {
                paint::focus_ring(&p, r, size::RADIUS_XS, &t);
            }
            let tip = if !o.title.is_empty() {
                o.title
            } else if self.icon_only {
                o.label
            } else {
                ""
            };
            let _ = ir
                .on_hover_cursor(if en {
                    egui::CursorIcon::PointingHand
                } else {
                    egui::CursorIcon::NotAllowed
                })
                .tip(tip);
        }
        if let (Some(_), Some(cur)) = (focus_idx, current) {
            let (l, r) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                )
            });
            let step = |from: usize, fwd: bool| -> Option<usize> {
                let mut k = from;
                loop {
                    k = if fwd { k + 1 } else { k.checked_sub(1)? };
                    let o = self.options.get(k)?;
                    if !o.disabled {
                        return Some(k);
                    }
                }
            };
            let next = if l {
                step(cur, false)
            } else if r {
                step(cur, true)
            } else {
                None
            };
            if let Some(k) = next
                && let Some(o) = self.options.get(k)
            {
                moved = Some(o.value.clone());
                ui.memory_mut(|m| m.request_focus(segment_id(base, k)));
            }
        }
        if let Some(v) = moved {
            *self.value = v;
            resp.mark_changed();
        }
        resp
    }
}

/// The filled widths: `natural` plus an equal share of what is left of `inner`, or, when
/// `inner` is short, `natural` scaled down to it.
fn fill_widths(natural: &[f32], inner: f32) -> Vec<f32> {
    let sum: f32 = natural.iter().sum();
    if natural.is_empty() || sum <= 0.0 {
        return natural.to_vec();
    }
    let spare = inner - sum;
    if spare >= 0.0 {
        let share = spare / natural.len() as f32;
        natural.iter().map(|w| w + share).collect()
    } else {
        let k = (inner / sum).max(0.0);
        natural.iter().map(|w| w * k).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::fill_widths;

    #[test]
    fn filling_keeps_natural_widths_and_shares_the_rest() {
        let w = fill_widths(&[40.0, 100.0], 200.0);
        assert_eq!(w, vec![70.0, 130.0]);
        let w = fill_widths(&[40.0, 160.0], 100.0);
        assert_eq!(w, vec![20.0, 80.0]);
        assert!(fill_widths(&[], 100.0).is_empty());
    }
}
