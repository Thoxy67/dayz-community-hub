//! `ui/input`: the one text field, a number field and a text area wearing it.
//!
//! As tall as a control (`Sm`, 28) or as a control inside a row (`Xs`, 22), always on the
//! shell's ground (`bg-bg`) so it reads as a well cut into the panel, `rounded-sm`, a
//! border that strengthens under the pointer and turns accent while typing. A search
//! field wears the glass on the left and, once filled, a button that empties it.

use egui::{
    Align, Color32, CornerRadius, Id, Rect, Response, RichText, Sense, Shape, Stroke, StrokeKind,
    TextEdit, Ui, Vec2, Widget,
};

use super::paint::{self, alpha, fade};
use crate::icons::lucide;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// How tall a field is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldSize {
    /// `h-control text-xs px-2`.
    #[default]
    Sm,
    /// `h-control-sm text-2xs px-1.5`.
    Xs,
}

impl FieldSize {
    fn height(self) -> f32 {
        match self {
            FieldSize::Sm => size::control(),
            FieldSize::Xs => size::control_sm(),
        }
    }

    fn text(self) -> f32 {
        match self {
            FieldSize::Sm => size::TEXT_XS,
            FieldSize::Xs => size::TEXT_2XS,
        }
    }

    fn pad(self) -> f32 {
        match self {
            FieldSize::Sm => 8.0,
            FieldSize::Xs => 6.0,
        }
    }
}

/// What a field holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputKind {
    #[default]
    Text,
    /// The glass on the left, a clear button once filled.
    Search,
    /// Dots for letters.
    Password,
    /// Mono figures (`type="number"`'s face), for addresses and ports.
    Mono,
}

/// The field's frame, painted under a borderless `TextEdit`.
fn frame(
    painter: &egui::Painter,
    idx: egui::layers::ShapeIdx,
    rect: Rect,
    t: &Tokens,
    hover: f32,
    focus: f32,
    en: bool,
) {
    let border = paint::lerp(
        paint::lerp(t.border, t.border_strong, hover),
        alpha(t.accent, 0.6),
        focus,
    );
    painter.set(
        idx,
        Shape::Rect(egui::epaint::RectShape::new(
            rect,
            CornerRadius::same(size::RADIUS_SM),
            fade(t.bg, en),
            Stroke::new(1.0, fade(border, en)),
            StrokeKind::Inside,
        )),
    );
}

/// `Input::new(&mut query).search().placeholder("Search…")`. Returns the `TextEdit`'s
/// response (`changed()`, `lost_focus()`); clearing a search counts as a change.
#[must_use = "add it with ui.add(…)"]
pub struct Input<'a> {
    value: &'a mut String,
    size: FieldSize,
    kind: InputKind,
    placeholder: &'a str,
    clear_label: &'a str,
    width: Option<f32>,
    enabled: bool,
    id_salt: Option<Id>,
    char_limit: usize,
    error: bool,
    numeric: bool,
}

impl<'a> Input<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            size: FieldSize::Sm,
            kind: InputKind::Text,
            placeholder: "",
            clear_label: "",
            width: None,
            enabled: true,
            id_salt: None,
            char_limit: usize::MAX,
            error: false,
            numeric: false,
        }
    }

    /// The value is a number (a port, a coordinate): a controller's on-screen keyboard
    /// opens on its digits. The arrows still move the caret (`NumberInput` steps).
    pub fn numeric(mut self, on: bool) -> Self {
        self.numeric = on;
        self
    }

    /// A wrong value: the border turns red (`[&_input]:border-err/60`).
    pub fn error(mut self, on: bool) -> Self {
        self.error = on;
        self
    }

    pub fn size(mut self, s: FieldSize) -> Self {
        self.size = s;
        self
    }

    pub fn kind(mut self, k: InputKind) -> Self {
        self.kind = k;
        self
    }

    /// `type="search"`.
    pub fn search(self) -> Self {
        self.kind(InputKind::Search)
    }

    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    /// The accessible name of the clear button (`common.clearSearch`).
    pub fn clear_label(mut self, l: &'a str) -> Self {
        self.clear_label = l;
        self
    }

    /// A fixed width; by default the field takes the width on offer.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// A stable id (for `request_focus` from a shortcut).
    pub fn id_salt(mut self, salt: impl egui::AsId) -> Self {
        self.id_salt = Some(Id::new(salt));
        self
    }

    pub fn char_limit(mut self, n: usize) -> Self {
        self.char_limit = n;
        self
    }
}

impl Widget for Input<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let s = self.size;
        let h = s.height();
        let w = self.width.unwrap_or_else(|| ui.available_width()).max(h);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let bg = ui.painter().add(Shape::Noop);
        let search = self.kind == InputKind::Search;
        let filled = !self.value.is_empty();
        let (pl, pr) = match (search, s) {
            (true, FieldSize::Sm) => (24.0, 24.0),
            (true, FieldSize::Xs) => (22.0, 20.0),
            (false, _) => (s.pad(), s.pad()),
        };
        let inner = Rect::from_min_max(
            egui::pos2(rect.left() + pl, rect.top()),
            egui::pos2(rect.right() - pr, rect.bottom()),
        );
        let face = if self.kind == InputKind::Mono {
            Face::Mono
        } else {
            Face::Sans
        };
        let f = font(face, s.text());
        let mut edit = TextEdit::singleline(&mut *self.value)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .font(f.clone())
            .text_color(t.fg)
            .vertical_align(Align::Center)
            .desired_width(inner.width())
            .password(self.kind == InputKind::Password)
            .char_limit(self.char_limit)
            .hint_text(RichText::new(self.placeholder).font(f).color(t.fg_faint));
        if let Some(id) = self.id_salt {
            edit = edit.id(id);
        }
        // A child that does not allocate in `ui`: a scope here would add the row's
        // spacing a second time after the field.
        let mut field = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        if !self.enabled {
            field.disable();
        }
        let mut resp = field.put(inner, edit);
        if self.numeric {
            crate::pad::registry::digits(ui.ctx(), resp.id);
        }
        let hover = paint::ease(
            ui,
            resp.id.with("hover"),
            self.enabled && ui.rect_contains_pointer(rect),
        );
        let focus = paint::ease(ui, resp.id.with("focus"), resp.has_focus());
        let mut ft = t;
        if self.error {
            ft.border = alpha(t.err, 0.6);
            ft.border_strong = ft.border;
        }
        frame(ui.painter(), bg, rect, &ft, hover, focus, self.enabled);
        if search {
            let ir = Rect::from_center_size(
                egui::pos2(rect.left() + 6.0 + size::ICON_SM / 2.0, rect.center().y),
                Vec2::splat(size::ICON_SM),
            );
            lucide::SEARCH.paint(ui.painter(), ir, fade(t.fg_faint, self.enabled));
            if filled && self.enabled {
                let cr = Rect::from_center_size(
                    egui::pos2(rect.right() - 4.0 - 8.0, rect.center().y),
                    Vec2::splat(16.0),
                );
                let clear = ui.interact(cr, resp.id.with("clear"), Sense::click());
                let label = self.clear_label;
                clear.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                });
                let ch = paint::ease(ui, clear.id, clear.hovered());
                let p = ui.painter();
                p.rect_filled(
                    cr,
                    CornerRadius::same(size::RADIUS_XS),
                    paint::lerp(Color32::TRANSPARENT, t.raised, ch),
                );
                lucide::X.paint(p, cr.shrink(1.0), paint::lerp(t.fg_faint, t.fg, ch));
                if clear.clicked() {
                    self.value.clear();
                    resp.request_focus();
                    resp.mark_changed();
                }
            }
        }
        resp
    }
}

/// A number typed in a field: mono figures, the arrows and the wheel step it (a
/// controller's Left and Right too), and what is typed is kept until the field is left
/// (then clamped). `NumberInput::new(&mut port)` over an `f64`; convert integers at the
/// call.
#[must_use = "add it with ui.add(…)"]
pub struct NumberInput<'a> {
    value: &'a mut f64,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    size: FieldSize,
    width: Option<f32>,
    enabled: bool,
}

impl<'a> NumberInput<'a> {
    pub fn new(value: &'a mut f64) -> Self {
        Self {
            value,
            min: f64::MIN,
            max: f64::MAX,
            step: 1.0,
            decimals: 0,
            size: FieldSize::Sm,
            width: None,
            enabled: true,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min.min(max);
        self.max = max.max(min);
        self
    }

    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs();
        self
    }

    pub fn decimals(mut self, n: usize) -> Self {
        self.decimals = n.min(8);
        self
    }

    pub fn size(mut self, s: FieldSize) -> Self {
        self.size = s;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

impl Widget for NumberInput<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let id = ui.next_auto_id().with("number");
        let shown = format!("{:.*}", self.decimals, *self.value);
        let mut text: String = ui
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| shown.clone());
        let mut input = Input::new(&mut text)
            .kind(InputKind::Mono)
            .size(self.size)
            .enabled(self.enabled)
            .id_salt(id);
        if let Some(w) = self.width {
            input = input.width(w);
        }
        let mut resp = ui.add(input);
        // A controller steps it with Left and Right, and types it on digits.
        crate::pad::registry::number(ui.ctx(), resp.id);
        let clamp = |v: f64| {
            if v.is_finite() {
                v.clamp(self.min, self.max)
            } else {
                self.min.max(0.0_f64.min(self.max))
            }
        };
        if resp.has_focus() {
            let (up, down) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                )
            });
            if up || down {
                let base: f64 = text.trim().parse().unwrap_or(*self.value);
                let next = clamp(base + if up { self.step } else { -self.step });
                *self.value = next;
                text = format!("{:.*}", self.decimals, next);
                resp.mark_changed();
            } else if resp.changed()
                && let Ok(v) = text.trim().parse::<f64>()
                && v.is_finite()
            {
                *self.value = clamp(v);
            }
            ui.data_mut(|d| d.insert_temp(id, text));
        } else {
            if resp.hovered() && self.enabled {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    *self.value =
                        clamp(*self.value + if scroll > 0.0 { self.step } else { -self.step });
                    resp.mark_changed();
                }
            }
            if resp.lost_focus() {
                *self.value = clamp(*self.value);
            }
            ui.data_mut(|d| d.remove::<String>(id));
        }
        resp
    }
}

/// A field of several lines, in the input's clothes: `TextArea::new(&mut notes).rows(4)`.
#[must_use = "add it with ui.add(…)"]
pub struct TextArea<'a> {
    value: &'a mut String,
    rows: usize,
    placeholder: &'a str,
    mono: bool,
    enabled: bool,
}

impl<'a> TextArea<'a> {
    pub fn new(value: &'a mut String) -> Self {
        Self {
            value,
            rows: 3,
            placeholder: "",
            mono: false,
            enabled: true,
        }
    }

    pub fn rows(mut self, n: usize) -> Self {
        self.rows = n.max(1);
        self
    }

    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

impl Widget for TextArea<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let bg = ui.painter().add(Shape::Noop);
        let f = font(
            if self.mono { Face::Mono } else { Face::Sans },
            size::TEXT_XS,
        );
        let edit = TextEdit::multiline(self.value)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
            .font(f)
            .text_color(t.fg)
            .desired_rows(self.rows)
            .desired_width(f32::INFINITY)
            .hint_text(RichText::new(self.placeholder).color(t.fg_faint));
        let resp = ui.add_enabled(self.enabled, edit);
        let hover = paint::ease(ui, resp.id.with("hover"), self.enabled && resp.hovered());
        let focus = paint::ease(ui, resp.id.with("focus"), resp.has_focus());
        frame(ui.painter(), bg, resp.rect, &t, hover, focus, self.enabled);
        resp
    }
}
