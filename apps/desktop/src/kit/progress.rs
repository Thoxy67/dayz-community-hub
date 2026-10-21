//! Things that move while something is on its way: `ui/spinner`, `ui/skeleton`
//! (Skeleton, SkeletonRows) and `ui/meter`.
//!
//! These repaint while they are on screen (a spinner turns once a second, the shimmer
//! crosses in 1.4s, the indeterminate band in 1.6s), and stop when they are not drawn.

use egui::{
    Color32, CornerRadius, Mesh, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, Widget,
};

use super::Tone;
use super::paint::alpha;
use crate::theme::{Tokens, TokensExt, motion, size};

/// `ui/spinner`: lucide's loader-circle turning (`size-icon animate-spin text-fg-faint`).
#[must_use = "add it with ui.add(…)"]
pub struct Spinner {
    size: f32,
    color: Option<Color32>,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            size: size::ICON,
            color: None,
        }
    }

    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }

    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }

    /// Paints one in `rect` (for a spinner inside another widget).
    pub fn paint(ui: &Ui, rect: Rect, color: Color32) {
        let time = ui.input(|i| i.time);
        let turn = (time.fract() as f32) * std::f32::consts::TAU;
        let s = rect.width().min(rect.height()) / 24.0;
        let c = rect.center();
        let r = 9.0 * s;
        // The arc runs from 3 o'clock round to 288°, as loader-circle draws it.
        const N: usize = 40;
        let sweep = 288.0_f32.to_radians();
        let pts: Vec<Pos2> = (0..=N)
            .map(|i| {
                let a = turn + sweep * i as f32 / N as f32;
                c + Vec2::angled(a) * r
            })
            .collect();
        ui.painter()
            .add(Shape::line(pts, Stroke::new(2.0 * s, color)));
        ui.ctx().request_repaint();
    }
}

impl Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        if ui.is_rect_visible(rect) {
            let c = self.color.unwrap_or_else(|| ui.tokens().fg_faint);
            Self::paint(ui, rect, c);
        }
        resp
    }
}

/// The light that crosses a skeleton: `via-fg/6`, 1.4s ease-in-out, -100% → 100%.
fn shimmer(ui: &Ui, rect: Rect, t: &Tokens) {
    let time = ui.input(|i| i.time);
    let k = ((time / 1.4).fract()) as f32;
    let e = if k < 0.5 {
        2.0 * k * k
    } else {
        1.0 - (-2.0 * k + 2.0).powi(2) / 2.0
    };
    let x0 = rect.left() + rect.width() * (e * 2.0 - 1.0);
    let band = Rect::from_min_size(Pos2::new(x0, rect.top()), rect.size());
    let mid = alpha(t.fg, 0.06);
    let mut mesh = Mesh::default();
    let cols = [
        (band.left(), Color32::TRANSPARENT),
        (band.center().x, mid),
        (band.right(), Color32::TRANSPARENT),
    ];
    for (x, c) in cols {
        mesh.colored_vertex(Pos2::new(x, band.top()), c);
        mesh.colored_vertex(Pos2::new(x, band.bottom()), c);
    }
    for i in 0..2u32 {
        let b = i * 2;
        mesh.add_triangle(b, b + 1, b + 2);
        mesh.add_triangle(b + 1, b + 3, b + 2);
    }
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .add(Shape::mesh(mesh));
    ui.ctx().request_repaint();
}

/// `ui/skeleton`: the shape of something still on its way, `rounded-full bg-raised`
/// with the shimmer. `Skeleton::new(96.0, 8.0)`; a width of `f32::INFINITY` fills.
#[must_use = "add it with ui.add(…)"]
pub struct Skeleton {
    size: Vec2,
    radius: Option<u8>,
    opacity: f32,
}

impl Skeleton {
    pub fn new(w: f32, h: f32) -> Self {
        Self {
            size: Vec2::new(w, h),
            radius: None,
            opacity: 1.0,
        }
    }

    /// A corner radius other than fully round (`rounded-sm` for a picture).
    pub fn radius(mut self, r: u8) -> Self {
        self.radius = Some(r);
        self
    }

    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = o.clamp(0.0, 1.0);
        self
    }

    /// Paints one in `rect`.
    pub fn paint(ui: &Ui, rect: Rect, radius: Option<u8>, opacity: f32) {
        let t = ui.tokens();
        let r = radius.unwrap_or_else(|| (rect.height() / 2.0).min(255.0) as u8);
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(r),
            t.raised.gamma_multiply(opacity),
        );
        shimmer(ui, rect, &t);
    }
}

impl Widget for Skeleton {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = if self.size.x.is_finite() {
            self.size.x
        } else {
            ui.available_width()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, self.size.y), Sense::hover());
        if ui.is_rect_visible(rect) {
            Self::paint(ui, rect, self.radius, self.opacity);
        }
        resp
    }
}

/// A list that is loading: `count` rows the height of real ones (`h-row border-b
/// border/50 px-pad`), a line and a shorter one under it, a square in front when the
/// rows have a picture. Widths vary from row to row so it reads as a list.
#[must_use = "add it with ui.add(…)"]
pub struct SkeletonRows {
    count: usize,
    media: bool,
}

impl SkeletonRows {
    pub fn new(count: usize) -> Self {
        Self {
            count,
            media: false,
        }
    }

    pub fn media(mut self) -> Self {
        self.media = true;
        self
    }
}

const WIDTHS: [f32; 6] = [0.6, 0.4, 0.5, 2.0 / 3.0, 1.0 / 3.0, 0.75];

impl Widget for SkeletonRows {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let w = ui.available_width();
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(w, size::ROW * self.count as f32), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        for i in 0..self.count {
            let row = Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + size::ROW * i as f32),
                Vec2::new(w, size::ROW),
            );
            if !ui.is_rect_visible(row) {
                continue;
            }
            ui.painter().hline(
                row.x_range(),
                row.bottom() - 0.5,
                Stroke::new(1.0, alpha(t.border, 0.5)),
            );
            let mut x = row.left() + size::PAD;
            if self.media {
                Skeleton::paint(
                    ui,
                    Rect::from_min_size(Pos2::new(x, row.center().y - 16.0), Vec2::splat(32.0)),
                    Some(size::RADIUS_SM),
                    1.0,
                );
                x += 32.0 + 12.0;
            }
            let room = (row.right() - size::PAD - x).max(0.0);
            let a = WIDTHS.get(i % WIDTHS.len()).copied().unwrap_or(0.5);
            let b = WIDTHS.get((i + 3) % WIDTHS.len()).copied().unwrap_or(0.5);
            let top = row.center().y - (10.0 + 6.0 + 8.0) / 2.0;
            Skeleton::paint(
                ui,
                Rect::from_min_size(Pos2::new(x, top), Vec2::new(room * a, 10.0)),
                None,
                1.0,
            );
            Skeleton::paint(
                ui,
                Rect::from_min_size(Pos2::new(x, top + 16.0), Vec2::new(room * b, 8.0)),
                None,
                0.6,
            );
        }
        resp
    }
}

/// How thick a meter is: `Xs` 2, `Sm` 4, `Md` 6.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeterSize {
    Xs,
    #[default]
    Sm,
    Md,
}

/// `ui/meter`: a bar some fraction full (`rounded-full bg-raised`), the fill easing to
/// its value over `--duration-base`; `indeterminate` sweeps a third-wide band.
/// `Meter::new(0.4).tone(Tone::Ok)`; the width on offer, or `.width(w)`.
#[must_use = "add it with ui.add(…)"]
pub struct Meter {
    fraction: f32,
    tone: Tone,
    size: MeterSize,
    animate: bool,
    indeterminate: bool,
    width: Option<f32>,
}

impl Meter {
    /// `value / max`, clamped to 0–1.
    pub fn new(fraction: f32) -> Self {
        let f = if fraction.is_finite() {
            fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            fraction: f,
            tone: Tone::Accent,
            size: MeterSize::Sm,
            animate: true,
            indeterminate: false,
            width: None,
        }
    }

    /// Accent, Ok, Warn, Err, Muted, Info (`in`); `out` is Warn.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    pub fn size(mut self, s: MeterSize) -> Self {
        self.size = s;
        self
    }

    pub fn animate(mut self, on: bool) -> Self {
        self.animate = on;
        self
    }

    pub fn indeterminate(mut self) -> Self {
        self.indeterminate = true;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

impl Widget for Meter {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let h = match self.size {
            MeterSize::Xs => 2.0,
            MeterSize::Sm => 4.0,
            MeterSize::Md => 6.0,
        };
        let w = self.width.unwrap_or_else(|| ui.available_width());
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let r = CornerRadius::same((h / 2.0) as u8);
        let fill = match self.tone {
            Tone::Ok => t.ok,
            Tone::Warn => t.warn,
            Tone::Err => t.err,
            Tone::Muted | Tone::Neutral => t.fg_faint,
            Tone::Info => t.info,
            Tone::Accent => t.accent,
        };
        let p = ui.painter();
        p.rect_filled(rect, r, t.raised);
        if self.indeterminate {
            let k = ((ui.input(|i| i.time) / 1.6).fract()) as f32;
            let bw = rect.width() / 3.0;
            let x = rect.left() + bw * (-1.0 + 4.0 * k);
            let band = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(bw, h));
            p.with_clip_rect(rect.intersect(ui.clip_rect()))
                .rect_filled(band, r, fill);
            ui.ctx().request_repaint();
        } else {
            let f = if self.animate {
                ui.ctx()
                    .animate_value_with_time(resp.id.with("fill"), self.fraction, motion::BASE)
            } else {
                self.fraction
            };
            if f > 0.0 {
                let fr = Rect::from_min_size(rect.min, Vec2::new(rect.width() * f, h));
                p.rect_filled(fr, r, fill);
            }
        }
        resp
    }
}
