//! Drawing helpers the widgets share, and that `parts/` may use too: text laid out the
//! way the CSS set it (one line cut with an ellipsis, tracked capitals), the
//! shadows (`--shadow-pop`, `--glow-accent-soft`, `--shadow-glow`), the focus ring
//! (`:focus-visible`), colour easing.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CornerRadius, FontId, Frame, Galley, Id, Margin, Painter, Rect, Shadow, Stroke,
    StrokeKind, TextFormat, Ui,
};

use crate::theme::{Tokens, motion, with_alpha};

/// Opacity of a disabled control (`disabled:opacity-40`).
pub const DISABLED: f32 = 0.4;

/// One line of text, never wrapped.
pub fn galley(ui: &Ui, text: &str, font: FontId, color: Color32) -> Arc<Galley> {
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

/// One line of text, cut with `…` past `max_width` (`truncate`).
pub fn galley_trunc(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    tracked(ui, text, font, color, 0.0, false, Some(max_width))
}

/// Text wrapped at `width`.
pub fn galley_wrap(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    ui.painter()
        .layout(text.to_owned(), font, color, width.max(1.0))
}

/// One line with letter spacing (`tracking`, in em) and optionally upper-cased
/// (`uppercase tracking-[0.08em]`), cut at `max_width` when given.
pub fn tracked(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    tracking_em: f32,
    upper: bool,
    max_width: Option<f32>,
) -> Arc<Galley> {
    let spacing = font.size * tracking_em;
    let text = if upper {
        text.to_uppercase()
    } else {
        text.to_owned()
    };
    let mut job = LayoutJob::single_section(
        text,
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width: max_width.map_or(f32::INFINITY, |w| w.max(1.0)),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// Draws `g` with its left edge at `x`, centred vertically on `cy`.
pub fn text_at(painter: &Painter, g: &Arc<Galley>, x: f32, cy: f32, color: Color32) {
    let pos = egui::pos2(x, cy - g.size().y / 2.0);
    painter.galley_with_override_text_color(pos, Arc::clone(g), color);
}

/// Draws `g` centred in `rect`.
pub fn text_center(painter: &Painter, g: &Arc<Galley>, rect: Rect, color: Color32) {
    let pos = rect.center() - g.size() / 2.0;
    painter.galley_with_override_text_color(pos, Arc::clone(g), color);
}

/// An eased 0→1 for a state (hover, open, checked), `--duration-fast`, ease-out.
pub fn ease(ui: &Ui, id: Id, on: bool) -> f32 {
    ease_for(ui, id, on, motion::FAST)
}

/// [`ease`] with another duration.
pub fn ease_for(ui: &Ui, id: Id, on: bool, secs: f32) -> f32 {
    ui.ctx()
        .animate_bool_with_time_and_easing(id, on, secs, emath_ease_out)
}

fn emath_ease_out(t: f32) -> f32 {
    egui::emath::easing::cubic_out(t)
}

/// `a` → `b` by `t`.
pub fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    a.lerp_to_gamma(b, t.clamp(0.0, 1.0))
}

/// `c/N` in Tailwind: the colour at `a` opacity.
pub fn alpha(c: Color32, a: f32) -> Color32 {
    with_alpha(c, a)
}

/// A colour at the disabled opacity when `enabled` is false.
pub fn fade(c: Color32, enabled: bool) -> Color32 {
    if enabled {
        c
    } else {
        c.gamma_multiply(DISABLED)
    }
}

/// `filter: brightness(f)`.
pub fn bright(c: Color32, f: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let m = |v: u8| (f32::from(v) * f).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(m(r), m(g), m(b), a)
}

/// `--shadow-pop`: the drop under everything that floats.
pub fn shadow_pop() -> Shadow {
    Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(96),
    }
}

/// `--glow-accent-soft`: `0 0 8px accent/28%`.
pub fn glow_soft(t: &Tokens) -> Shadow {
    Shadow {
        offset: [0, 0],
        blur: 8,
        spread: 0,
        color: alpha(t.accent, 0.28),
    }
}

/// `--shadow-glow`'s halo: `0 0 14px -2px accent/40%` (its 1px ring is drawn by the caller).
pub fn glow(t: &Tokens) -> Shadow {
    Shadow {
        offset: [0, 0],
        blur: 14,
        spread: 0,
        color: alpha(t.accent, 0.40),
    }
}

/// The plate of anything that floats: overlay ground, border, `rounded-md`, `shadow-pop`.
pub fn float_frame(t: &Tokens, pad: i8) -> Frame {
    Frame::new()
        .fill(t.overlay)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(crate::theme::size::RADIUS_MD))
        .inner_margin(Margin::same(pad))
        .shadow(shadow_pop())
}

/// `:focus-visible`: a 2px accent outline, 1px off the edge.
pub fn focus_ring(painter: &Painter, rect: Rect, radius: u8, t: &Tokens) {
    painter.rect_stroke(
        rect.expand(1.0),
        CornerRadius::same(radius.saturating_add(1)),
        Stroke::new(2.0, t.accent),
        StrokeKind::Outside,
    );
}

/// Whether `r` holds the keyboard focus (the ring is for the keyboard, not a click).
pub fn keyboard_focus(ui: &Ui, r: &egui::Response) -> bool {
    r.has_focus() && !ui.input(|i| i.pointer.any_down())
}

/// A rounded rect filled and edged in one shape (border inside, as CSS `border-box`).
pub fn plate(painter: &Painter, rect: Rect, radius: u8, fill: Color32, border: Color32) {
    painter.rect(
        rect,
        CornerRadius::same(radius),
        fill,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
}

/// The accent tick that marks every heading: `h-2.5 w-0.5 rounded-full bg-accent/60`.
pub fn tick(painter: &Painter, x: f32, cy: f32, t: &Tokens) {
    let r = Rect::from_center_size(egui::pos2(x + 1.0, cy), egui::vec2(2.0, 10.0));
    painter.rect_filled(r, CornerRadius::same(1), alpha(t.accent, 0.6));
}
