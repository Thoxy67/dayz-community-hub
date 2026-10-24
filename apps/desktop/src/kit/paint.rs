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

/// Text wrapped at `width` with a CSS line height `lh` (px, e.g. `text-xs` = 12 × 1.45):
/// rows are `lh` apart and the galley is `lines × lh` tall, as the browser lays it out.
/// The second value is the half-leading: add it to the top when drawing so the glyphs sit
/// mid-row where CSS puts them (the galley has them at the top of a taller row, as egui
/// 0.36 did: [`rows_at_top`]).
pub fn galley_lh(
    ui: &Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
    lh: f32,
) -> (Arc<Galley>, f32) {
    let natural = ui.fonts_mut(|f| f.row_height(&font));
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color,
            line_height: Some(lh.max(1.0)),
            ..Default::default()
        },
    );
    job.wrap.max_width = width.max(1.0);
    let g = layout_top(ui, job);
    (g, ((lh - natural) / 2.0).max(0.0))
}

/// `job` laid out with its glyphs where egui 0.36 put them in a row of a custom line
/// height: at the row's top, the leading below them (see [`rows_at_top`]).
pub fn layout_top(ui: &Ui, job: LayoutJob) -> Arc<Galley> {
    rows_at_top(ui.painter().layout_job(job))
}

/// `job` as a wrapping [`egui::Label`] lays it out in a vertical layout, with
/// [`rows_at_top`]: give it to `Label::new`.
pub fn label_top(ui: &Ui, mut job: LayoutJob) -> Arc<Galley> {
    job.wrap.max_width = ui.available_width();
    job.halign = ui.layout().horizontal_placement();
    job.justify = ui.layout().horizontal_justify();
    layout_top(ui, job)
}

/// Text in rows of a custom line height, moved back to where egui 0.36 drew it: since
/// 0.37 egui centres the glyphs in the row as CSS does (half the leading above), and the
/// views that draw such text were placed for the glyphs at the row's top. Each row moves
/// up by the shift egui applied to its first glyph (exactly, rounding included); a galley
/// without custom line heights comes back as it was.
pub fn rows_at_top(g: Arc<Galley>) -> Arc<Galley> {
    let ppp = g.pixels_per_point.max(f32::EPSILON);
    let round = |v: f32| (v * ppp).round() / ppp;
    // The jobs given here share one `valign` across their sections.
    let valign = g
        .job
        .sections
        .first()
        .map_or(0.0, |s| s.format.valign.to_factor());
    let shifts: Vec<f32> = g
        .rows
        .iter()
        .map(|r| {
            r.row.glyphs.first().map_or(0.0, |gl| {
                // epaint's placement before the centring term (0.36's whole formula).
                let at = gl.font_face_ascent
                    + valign * (r.row.size.y - gl.line_height)
                    + 0.5 * (gl.font_height - gl.font_face_height);
                round(at + 0.5 * (gl.line_height - gl.font_height)) - round(at)
            })
        })
        .collect();
    if shifts.iter().all(|s| *s == 0.0) {
        return g;
    }
    let lo = shifts.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = shifts.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut moved = (*g).clone();
    for (row, s) in moved.rows.iter_mut().zip(&shifts) {
        row.pos.y -= s;
    }
    let b = moved.mesh_bounds;
    moved.mesh_bounds = Rect::from_min_max(
        egui::pos2(b.min.x, b.min.y - hi),
        egui::pos2(b.max.x, b.max.y - lo),
    );
    Arc::new(moved)
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

/// Frames a looping decoration gets while the window has focus (about 30 a second) and
/// while it is behind another window (4 a second).
const LOOP_FOCUSED: std::time::Duration = std::time::Duration::from_millis(33);
const LOOP_AWAY: std::time::Duration = std::time::Duration::from_millis(250);

/// Asks for the next frame of a decorative loop (a spinner, a shimmer, a pulse) that
/// is on screen: about 30 a second with focus, 4 behind another window, never at the
/// display's rate. A spinner turning once a second looks the same at 30 frames, and a
/// download that lasts an hour no longer rebuilds the window 144 times a second while
/// the game or SteamCMD needs the machine. Only for decoration: a repaint that carries
/// state (a hub event, a debounce, a measured width) asks for its own.
pub fn animate(ctx: &egui::Context) {
    let focused = ctx.input(|i| i.viewport().focused.unwrap_or(i.focused));
    ctx.request_repaint_after(if focused { LOOP_FOCUSED } else { LOOP_AWAY });
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
/// A control the keyboard has just moved to is brought into view: egui's scroll areas
/// do not follow Tab by themselves.
pub fn keyboard_focus(ui: &Ui, r: &egui::Response) -> bool {
    let keyboard = r.has_focus() && !ui.input(|i| i.pointer.any_down());
    if keyboard && r.gained_focus() {
        r.scroll_to_me(None);
    }
    keyboard
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::theme::fonts::{Face, definitions};

    /// [`rows_at_top`] puts the glyphs exactly where egui 0.36 did: the baselines below
    /// were read from 0.36 for these sizes, line heights and pixel densities.
    #[test]
    fn rows_at_top_matches_egui_0_36() {
        // (pixels per point, size, line height, 0.36's baseline)
        let cases: [(f32, f32, f32, f32); 12] = [
            (1.0, 12.0, 17.4, 12.0),
            (1.0, 13.0, 17.4, 13.0),
            (1.0, 11.0, 15.125, 11.0),
            (1.0, 13.0, 15.125, 13.0),
            (1.25, 12.0, 17.4, 12.8),
            (1.25, 13.0, 17.4, 13.6),
            (1.25, 11.0, 15.125, 11.2),
            (1.25, 13.0, 15.125, 13.6),
            (1.5, 12.0, 17.4, 12.0),
            (1.5, 13.0, 17.4, 13.333_333),
            (1.5, 11.0, 15.125, 11.333_333),
            (1.5, 13.0, 15.125, 13.333_333),
        ];
        for (ppp, size, lh, want) in cases {
            let ctx = egui::Context::default();
            ctx.set_fonts(definitions());
            let mut raw = egui::RawInput::default();
            raw.viewports
                .entry(egui::ViewportId::ROOT)
                .or_default()
                .native_pixels_per_point = Some(ppp);
            for _ in 0..2 {
                ctx.run_ui(raw.clone(), |_| {}).textures_delta.clear();
            }
            let job = LayoutJob::single_section(
                "Identité".into(),
                TextFormat {
                    font_id: FontId::new(size, Face::Sans.family()),
                    line_height: Some(lh),
                    ..Default::default()
                },
            );
            let g = rows_at_top(ctx.fonts_mut(|f| f.layout_job(job)));
            let row = &g.rows[0];
            let y = row.pos.y + row.row.glyphs[0].pos.y;
            assert!(
                (y - want).abs() < 1e-3,
                "ppp {ppp} size {size} lh {lh}: {y} ≠ {want}"
            );
        }
    }
}
