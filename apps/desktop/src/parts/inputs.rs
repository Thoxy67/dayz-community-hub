//! Fields that know the app: `SecretInput` (a key or a password, hidden until asked),
//! `PathInput` (a path typed or browsed for), `ColorToken` (one of a theme's twelve
//! colours) and `LanguagePicker` (the five languages as flags).

use egui::{
    Color32, CornerRadius, Id, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextEdit, Ui, Vec2,
};

use super::text::{draw, ph, ph_trunc};
use crate::i18n::{Locale, t};
use crate::icons::{flag, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Input, InputKind, Side, TipExt};
use crate::theme::{Oklch, TokensExt, font, fonts::Face, oklch, rgb, size};

/// `SecretInput`: a key or a password, hidden until the eye beside it is pressed. Returns
/// the field's response.
#[must_use = "add it with ui.add(…)"]
pub struct SecretInput<'a> {
    value: &'a mut String,
    placeholder: &'a str,
    id_salt: Option<Id>,
}

impl<'a> SecretInput<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            placeholder: "",
            id_salt: None,
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    pub fn id_salt(mut self, salt: impl egui::AsIdSalt) -> Self {
        self.id_salt = Some(Id::unique(salt));
        self
    }
}

impl egui::Widget for SecretInput<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let id = self.id_salt.unwrap_or_else(|| ui.next_auto_id());
        let shown_id = id.with("shown");
        let shown: bool = ui.data(|d| d.get_temp(shown_id)).unwrap_or(false);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let w = (ui.available_width() - size::CONTROL - 8.0).max(40.0);
            let kind = if shown {
                InputKind::Mono
            } else {
                InputKind::Password
            };
            let field = ui.add(
                Input::new(self.value)
                    .kind(kind)
                    .placeholder(self.placeholder)
                    .width(w)
                    .id_salt(id.with("field")),
            );
            let (r, eye) = ui.allocate_exact_size(Vec2::splat(size::CONTROL), Sense::click());
            let h = paint::ease(ui, eye.id.with("hover"), eye.hovered());
            if h > 0.0 {
                ui.painter().rect_filled(
                    r,
                    CornerRadius::same(size::RADIUS_SM),
                    alpha(tk.raised, h),
                );
            }
            let icon = if shown { lucide::EYE_OFF } else { lucide::EYE };
            icon.paint(
                ui.painter(),
                Rect::from_center_size(r.center(), Vec2::splat(size::ICON_SM)),
                paint::lerp(tk.fg_faint, tk.fg, h),
            );
            let label = if shown {
                t("settings", "hide")
            } else {
                t("settings", "show")
            };
            if eye
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .tip_kbd(label, "", Side::Top)
                .clicked()
            {
                ui.data_mut(|d| d.insert_temp(shown_id, !shown));
            }
            field
        })
        .inner
    }
}

/// What a [`PathInput`] answers: the field's response, and whether Browse was pressed
/// (the caller opens the system picker, off the frame, and writes the path back).
pub struct PathResponse {
    pub field: Response,
    pub browse: bool,
}

/// `PathInput`: a file or folder path, typed (mono) or chosen with the system's picker.
pub struct PathInput<'a> {
    value: &'a mut String,
    placeholder: &'a str,
    id_salt: Option<Id>,
}

impl<'a> PathInput<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            placeholder: "",
            id_salt: None,
        }
    }

    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    pub fn id_salt(mut self, salt: impl egui::AsIdSalt) -> Self {
        self.id_salt = Some(Id::unique(salt));
        self
    }

    pub fn show(self, ui: &mut Ui) -> PathResponse {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let label = t("settings", "browse");
            let bw = Button::new(label).icon(lucide::FOLDER_OPEN);
            // The button's width: its label, its icon, its padding.
            let lw = ph(ui, label, font(Face::SansMedium, size::TEXT_XS))
                .size()
                .x;
            let button_w = lw + size::ICON_SM + 6.0 + 22.0;
            let w = (ui.available_width() - button_w - 8.0).max(40.0);
            let mut input = Input::new(self.value)
                .kind(InputKind::Mono)
                .placeholder(self.placeholder)
                .width(w);
            if let Some(id) = self.id_salt {
                input = input.id_salt(id);
            }
            let field = ui.add(input);
            let browse = ui.add(bw).clicked();
            PathResponse { field, browse }
        })
        .inner
    }
}

// ---------------------------------------------------------------------------------------

/// `oklch(62.0% 0.170 32)`, as the theme editor writes a colour.
pub fn oklch_text(c: Oklch) -> String {
    format!("oklch({:.1}% {:.3} {})", c[0] * 100.0, c[1], c[2].round())
}

/// A colour typed: `oklch(L% C H)` (or `L` 0–1), or `#rrggbb`.
pub fn parse_color(s: &str) -> Option<Oklch> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        let v = u32::from_str_radix(hex, 16).ok()?;
        let c = match hex.len() {
            6 => Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
            3 => {
                let d = |n: u32| ((n & 0xf) * 17) as u8;
                Color32::from_rgb(d(v >> 8), d(v >> 4), d(v))
            }
            _ => return None,
        };
        return Some(oklch(c));
    }
    let inner = s.strip_prefix("oklch(")?.strip_suffix(')')?;
    let mut parts = inner.split_whitespace();
    let l = parts.next()?;
    let l = match l.strip_suffix('%') {
        Some(p) => p.parse::<f32>().ok()? / 100.0,
        None => l.parse::<f32>().ok()?,
    };
    let c: f32 = parts.next()?.parse().ok()?;
    let h: f32 = parts.next()?.trim_end_matches("deg").parse().ok()?;
    if parts.next().is_some() || !(0.0..=1.0).contains(&l) || !(0.0..=0.5).contains(&c) {
        return None;
    }
    Some([l, c, h.rem_euclid(360.0)])
}

/// `ColorToken`: one of the theme's twelve colours: the swatch opens a colour picker, and
/// the exact OKLCH value beside it can be typed (`#hex` too). `changed()` when it moved.
pub struct ColorToken<'a> {
    value: &'a mut Oklch,
    label: &'a str,
}

impl<'a> ColorToken<'a> {
    pub fn new(label: &'a str, value: &'a mut Oklch) -> Self {
        Self { value, label }
    }
}

impl egui::Widget for ColorToken<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let w = ui.available_width();
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, 28.0 + 12.0), Sense::hover());
        ui.painter().rect_filled(rect, CornerRadius::ZERO, tk.bg);
        let cy = rect.center().y;
        let sw = Rect::from_min_size(Pos2::new(rect.left() + 8.0, cy - 14.0), Vec2::splat(28.0));
        let srgb = rgb(*self.value);
        let swatch = ui.interact(sw, resp.id.with("swatch"), Sense::click());
        let label = self.label;
        swatch.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        ui.painter()
            .rect_filled(sw, CornerRadius::same(size::RADIUS_SM), srgb);
        ui.painter().rect_stroke(
            sw,
            CornerRadius::same(size::RADIUS_SM),
            Stroke::new(1.0, tk.border_strong),
            StrokeKind::Inside,
        );
        let swatch = swatch
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(self.label, "", Side::Top);
        let mut picked = srgb;
        let mut picker_moved = false;
        let mut lch = *self.value;
        let popup_id = egui::Popup::default_response_id(&swatch);
        let was_open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        let popup = egui::Popup::from_toggle_button_response(&swatch)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .frame(paint::float_frame(&tk, 8));
        crate::kit::menu::clear_of_bars(popup, ui.ctx(), egui::RectAlign::BOTTOM_START, 4.0).show(
            |ui| {
                // Only a move counts: shown, the picker's own round trip can nudge the
                // colour by a unit, which would rewrite the token on opening it.
                picker_moved = egui::color_picker::color_picker_color32(
                    ui,
                    &mut picked,
                    egui::color_picker::Alpha::Opaque,
                );
                // The same colour as three sliders, which arrows and a controller move
                // (the square above is for a pointer).
                lch_sliders(ui, &mut lch);
            },
        );
        // Closed by Escape (a controller's B): the focus goes back to the swatch, not to
        // whatever the window would land on next.
        if was_open
            && !egui::Popup::is_id_open(ui.ctx(), popup_id)
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
        {
            swatch.request_focus();
        }
        if lch != *self.value {
            *self.value = lch;
            resp.mark_changed();
        } else if picker_moved && picked != srgb {
            *self.value = oklch(picked);
            resp.mark_changed();
        }
        let lx = sw.right() + 8.0;
        let lg = ph_trunc(ui, self.label, font(Face::Sans, size::TEXT_2XS), 112.0);
        draw(ui.painter(), &lg, lx, cy, tk.fg_muted);
        // The value, typed: kept as a draft while the field has focus, read on leaving it.
        let fx = lx + 112.0 + 8.0;
        let field = Rect::from_min_max(
            Pos2::new(fx, cy - size::CONTROL_SM / 2.0),
            Pos2::new(rect.right() - 8.0, cy + size::CONTROL_SM / 2.0),
        );
        let draft_id = resp.id.with("draft");
        let edit_id = resp.id.with("edit");
        let focused = ui.memory(|m| m.has_focus(edit_id));
        let mut draft: String = if focused {
            ui.data(|d| d.get_temp(draft_id))
                .unwrap_or_else(|| oklch_text(*self.value))
        } else {
            oklch_text(*self.value)
        };
        let bg = ui.painter().add(egui::Shape::Noop);
        // egui 0.37 makes a text edit at least `interact_size.y` tall: the field is smaller.
        let interact_h = std::mem::replace(&mut ui.spacing_mut().interact_size.y, 0.0);
        let edit = ui.put(
            field.shrink2(Vec2::new(4.0, 0.0)),
            TextEdit::singleline(&mut draft)
                .id(edit_id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .font(font(Face::Mono, size::TEXT_2XS))
                .text_color(tk.fg)
                .align(egui::Align2::LEFT_CENTER),
        );
        ui.spacing_mut().interact_size.y = interact_h;
        let hover = ui.rect_contains_pointer(field);
        let border = if edit.has_focus() {
            alpha(tk.accent, 0.6)
        } else if hover {
            tk.border
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().set(
            bg,
            egui::Shape::rect_stroke(
                field,
                CornerRadius::same(size::RADIUS_XS),
                Stroke::new(1.0, border),
                StrokeKind::Inside,
            ),
        );
        if edit.has_focus() {
            ui.data_mut(|d| d.insert_temp(draft_id, draft.clone()));
        }
        if edit.lost_focus() {
            ui.data_mut(|d| d.remove::<String>(draft_id));
            if let Some(c) = parse_color(&draft)
                && c != *self.value
            {
                *self.value = c;
                resp.mark_changed();
            }
        }
        resp
    }
}

/// Lightness, chroma and hue as sliders under the colour picker; the first is where a
/// controller lands in it.
fn lch_sliders(ui: &mut Ui, c: &mut Oklch) {
    let tk = ui.tokens();
    // As wide as the picker above.
    let w = ui.min_rect().width().max(size::ROW * 3.0);
    let rows: [(&str, f64, std::ops::RangeInclusive<f64>, f64); 3] = [
        (t("theme", "lightness"), f64::from(c[0]), 0.0..=1.0, 0.01),
        (t("theme", "chroma"), f64::from(c[1]), 0.0..=0.37, 0.005),
        (t("theme", "hue"), f64::from(c[2]), 0.0..=360.0, 1.0),
    ];
    ui.add_space(ui.spacing().item_spacing.y);
    for (i, (label, mut v, range, step)) in rows.into_iter().enumerate() {
        let lg = ph_trunc(ui, label, font(Face::Sans, size::TEXT_2XS), w);
        let (lr, _) = ui.allocate_exact_size(Vec2::new(w, lg.size().y), Sense::hover());
        draw(ui.painter(), &lg, lr.left(), lr.center().y, tk.fg_muted);
        let r = ui.add(
            crate::kit::Slider::new(&mut v, range)
                .step(step)
                .width(w)
                .label(label),
        );
        if i == 0 {
            crate::pad::primary(ui.ctx(), r.id);
        }
        if r.changed()
            && let Some(slot) = c.get_mut(i)
        {
            *slot = v as f32;
        }
    }
}

// ---------------------------------------------------------------------------------------

/// `LanguagePicker`: the five languages as flags with their own names, the current one lit.
/// Answers the one clicked; the caller switches (`i18n::set_locale`) and saves.
pub struct LanguagePicker {
    current: Locale,
    inline: bool,
}

impl LanguagePicker {
    pub fn new(current: Locale) -> Self {
        Self {
            current,
            inline: false,
        }
    }

    /// The settings page's look: the flag at the left of the name and its code, a check
    /// on the current one (`flex items-center gap-2 px-2.5 py-2`).
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Option<Locale> {
        if self.inline {
            return self.show_inline(ui);
        }
        let tk = ui.tokens();
        let w = ui.available_width();
        let gap = 6.0;
        let cw = ((w - 4.0 * gap) / 5.0).max(40.0);
        // `py-2.5`, the flag, `gap-1.5`, the name, and the 1px border at each side.
        let ch = 1.0 + 10.0 + 24.0 + 6.0 + 15.4 + 10.0 + 1.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, ch), Sense::hover());
        let mut out = None;
        for (i, l) in Locale::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + i as f32 * (cw + gap), rect.top()),
                Vec2::new(cw, ch),
            );
            let b = ui.interact(r, resp.id.with(i), Sense::click());
            let on = *l == self.current;
            let h = paint::ease(ui, b.id.with("hover"), b.hovered());
            let p = ui.painter();
            if on {
                p.add(paint::glow_soft(&tk).as_shape(r, CornerRadius::same(size::RADIUS_MD)));
                // The glow is a filled shape: the page's ground goes back under the tint.
                p.rect_filled(r, CornerRadius::same(size::RADIUS_MD), tk.bg);
                paint::plate(p, r, size::RADIUS_MD, alpha(tk.accent, 0.10), tk.accent);
            } else {
                paint::plate(
                    p,
                    r,
                    size::RADIUS_MD,
                    Color32::TRANSPARENT,
                    paint::lerp(tk.border, tk.border_strong, h),
                );
            }
            let icon = match l {
                Locale::En => flag::GB,
                Locale::Fr => flag::FR,
                Locale::De => flag::DE,
                Locale::Es => flag::ES,
                Locale::Ru => flag::RU,
            };
            icon.paint(
                p,
                Rect::from_min_size(
                    Pos2::new(r.center().x - 12.0, r.top() + 11.0),
                    Vec2::splat(24.0),
                ),
                Color32::WHITE,
            );
            let g = ph_trunc(ui, l.label(), font(Face::Sans, size::TEXT_2XS), cw - 16.0);
            let ink = if on {
                tk.fg
            } else {
                paint::lerp(tk.fg_muted, tk.fg, h)
            };
            draw(
                p,
                &g,
                r.center().x - g.size().x / 2.0,
                r.top() + 11.0 + 24.0 + 6.0 + 7.7,
                ink,
            );
            let b = b.on_hover_cursor(egui::CursorIcon::PointingHand);
            b.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, l.label())
            });
            if b.clicked() && !on {
                out = Some(*l);
            }
        }
        out
    }

    fn show_inline(self, ui: &mut Ui) -> Option<Locale> {
        let tk = ui.tokens();
        let w = ui.available_width();
        let gap = 8.0;
        let cw = ((w - 4.0 * gap) / 5.0).max(40.0);
        let ch = 8.0 + 16.0 + 13.0 + 8.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, ch), Sense::hover());
        let mut out = None;
        for (i, l) in Locale::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + i as f32 * (cw + gap), rect.top()),
                Vec2::new(cw, ch),
            );
            let b = ui.interact(r, resp.id.with(("inline", i)), Sense::click());
            let on = *l == self.current;
            let h = paint::ease(ui, b.id.with("hover"), b.hovered());
            let p = ui.painter();
            if on {
                paint::plate(p, r, size::RADIUS_MD, alpha(tk.accent, 0.08), tk.accent);
            } else {
                paint::plate(
                    p,
                    r,
                    size::RADIUS_MD,
                    alpha(tk.raised, 0.4 * h),
                    paint::lerp(tk.border, tk.border_strong, h),
                );
            }
            let icon = match l {
                Locale::En => flag::GB,
                Locale::Fr => flag::FR,
                Locale::De => flag::DE,
                Locale::Es => flag::ES,
                Locale::Ru => flag::RU,
            };
            let x = r.left() + 10.0;
            icon.paint(
                p,
                Rect::from_min_size(Pos2::new(x, r.center().y - 10.0), Vec2::splat(20.0)),
                Color32::WHITE,
            );
            let tx = x + 20.0 + 8.0;
            let check = if on { size::ICON_SM + 8.0 } else { 0.0 };
            let room = (r.right() - 10.0 - check - tx).max(10.0);
            let g = ph_trunc(ui, l.label(), font(Face::SansMedium, size::TEXT_XS), room);
            draw(p, &g, tx, r.top() + 8.0 + 8.0, tk.fg);
            let code = l.code().to_uppercase();
            let cg = ph_trunc(ui, &code, font(Face::Mono, size::TEXT_3XS), room);
            draw(p, &cg, tx, r.top() + 8.0 + 16.0 + 6.5, tk.fg_faint);
            if on {
                lucide::CHECK.paint(
                    p,
                    Rect::from_min_size(
                        Pos2::new(
                            r.right() - 10.0 - size::ICON_SM,
                            r.center().y - size::ICON_SM / 2.0,
                        ),
                        Vec2::splat(size::ICON_SM),
                    ),
                    tk.accent,
                );
            }
            let b = b.on_hover_cursor(egui::CursorIcon::PointingHand);
            b.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, l.label())
            });
            if b.clicked() && !on {
                out = Some(*l);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_parse() {
        let c = parse_color("oklch(62.0% 0.170 32)");
        assert_eq!(c, Some([0.62, 0.17, 32.0]));
        assert_eq!(parse_color("oklch(0.5 0.1 400)"), Some([0.5, 0.1, 40.0]));
        assert!(parse_color("#ff0000").is_some());
        assert!(parse_color("#f00").is_some());
        assert_eq!(parse_color("oklch(120% 0.1 3)"), None);
        assert_eq!(parse_color("red"), None);
        assert_eq!(oklch_text([0.62, 0.17, 32.0]), "oklch(62.0% 0.170 32)");
    }
}
