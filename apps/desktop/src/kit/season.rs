//! The seasonal effects' drawing, from the tokens only: snow resting on an edge, falling
//! flakes, a drifting fog, a candle's glow, a spider on its thread. Stateless: each takes
//! the effect clock (seconds that run only while the effects move), so a still frame is
//! the same call with the clock stopped.
//!
//! What a frame shows ([`Fx`]) is decided once a frame by `shell::seasons` and kept in
//! egui's memory, so a part (a card's top edge, the title) can ask for its share
//! ([`snow_on`], [`fx`]).

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{
    Color32, Context, CornerRadius, Id, Painter, Pos2, Rangef, Rect, Shadow, Shape, Stroke,
};

use crate::kit::paint::{alpha, lerp};
use crate::theme::{Ambient, Scheme, Tokens};

/// What the seasonal effects do this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fx {
    /// The worn preset's effect.
    pub ambient: Ambient,
    /// Drawn at all: a seasonal preset, the effects' setting on, a view on screen.
    pub on: bool,
    /// Moving this frame (focused, visible, no game, motion allowed).
    pub moving: bool,
    /// Reduced motion: one still frame, the spider kept up.
    pub still: bool,
    /// The effect clock, in seconds: runs only while moving.
    pub t: f64,
    /// The frame's input time, to step the clock.
    pub last: f64,
    /// The spider: when the pointer came over the rail, and how far down it is (0–1).
    pub hover_since: Option<f64>,
    pub drop: f32,
}

fn key() -> Id {
    Id::unique("dzch.season")
}

/// The effects of this frame.
pub fn fx(ctx: &Context) -> Fx {
    ctx.data(|d| d.get_temp::<Fx>(key())).unwrap_or_default()
}

pub fn set_fx(ctx: &Context, fx: Fx) {
    ctx.data_mut(|d| d.insert_temp(key(), fx));
}

/// The worn preset's effect (the theme store, when the theme changes).
pub fn set_ambient(ctx: &Context, ambient: Ambient) {
    let mut f = fx(ctx);
    if f.ambient != ambient {
        f.ambient = ambient;
        f.drop = 0.0;
        f.hover_since = None;
        set_fx(ctx, f);
    }
}

/// Snow resting on the top edge of `rect` (a card), when winter is worn.
pub fn snow_on(painter: &Painter, rect: Rect, tk: &Tokens) {
    let f = fx(painter.ctx());
    if f.on && f.ambient == Ambient::Winter {
        let inset = f32::from(crate::theme::size::RADIUS_MD) + 2.0;
        let x = Rangef::new(rect.left() + inset, rect.right() - inset);
        snow_cap(painter, x, rect.top(), tk);
    }
}

/// A small integer hash to 0–1: each flake's fixed draw.
fn rnd(i: u32, k: u32) -> f32 {
    let mut x = i
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(k.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// The upper half of an ellipse centred on (`cx`, `y`), clipped to `xs`.
fn dome(cx: f32, y: f32, rx: f32, ry: f32, xs: Rangef) -> Vec<Pos2> {
    const N: usize = 10;
    (0..=N)
        .map(|k| {
            let a = std::f32::consts::PI * (1.0 - k as f32 / N as f32);
            let x = (cx + rx * a.cos()).clamp(xs.min, xs.max);
            Pos2::new(x, y - ry * a.sin())
        })
        .collect()
}

/// Snow resting on an edge at `y` across `xs`: a scalloped crust (two rows of bumps
/// out of step, as the proposal's CSS drew it) over a 2 px layer. On a light ground
/// its outline is drawn in the frost colour, or white on white would not show.
pub fn snow_cap(painter: &Painter, xs: Rangef, y: f32, tk: &Tokens) {
    if xs.span() < 8.0 {
        return;
    }
    let fill = alpha(tk.snow, 0.92);
    let mut domes = Vec::new();
    // Every 27 px a 10 × 7 bump, every 43 px a wider, flatter one.
    for (step, off, rx, ry) in [(27.0, 12.0, 10.0, 6.5), (43.0, 29.0, 16.0, 4.5)] {
        let mut cx = xs.min - (xs.min.rem_euclid(step)) + off - step;
        while cx - rx < xs.max {
            if cx + rx > xs.min {
                domes.push(dome(cx, y, rx, ry, xs));
            }
            cx += step;
        }
    }
    let base = Rect::from_x_y_ranges(xs, (y - 2.0)..=y);
    if tk.scheme == Scheme::Light {
        let line = Stroke::new(1.0, alpha(tk.frost, 0.85));
        for d in &domes {
            painter.add(Shape::line(d.clone(), line));
        }
        painter.hline(xs, y - 2.5, line);
    }
    for d in domes {
        painter.add(Shape::convex_polygon(d, fill, Stroke::NONE));
    }
    painter.rect_filled(base, CornerRadius::same(1), fill);
}

/// How many flakes a window of this size gets: about 100 at 1280 × 800.
pub fn flake_count(rect: Rect) -> u32 {
    let area = (rect.width() * rect.height()).max(0.0);
    ((area / (1280.0 * 800.0)) * 100.0).clamp(30.0, 220.0) as u32
}

/// Falling snow over `rect` at clock `t`: three depths (the nearer, the larger, faster
/// and brighter), a light wind and a sway. A flake ends where `floor(x)` says snow
/// rests (the status bar's top edge): none falls past it.
pub fn flakes(painter: &Painter, rect: Rect, t: f64, tk: &Tokens, floor: impl Fn(f32) -> f32) {
    let (w, h) = (
        f64::from(rect.width()) + 8.0,
        f64::from(rect.height()) + 8.0,
    );
    if w < 16.0 || h < 16.0 {
        return;
    }
    // Light schemes draw a little stronger: the icy blue is less bright than snow.
    let gain = if tk.scheme == Scheme::Light {
        1.1
    } else {
        0.75
    };
    for i in 0..flake_count(rect) {
        let layer = (rnd(i, 1) * 3.0).floor().min(2.0);
        let z = 0.15 + layer * 0.4 + rnd(i, 2) * 0.1;
        let r = 0.8 + z * 2.3;
        let v = f64::from(14.0 + z * 38.0);
        let wind = f64::from(6.0 * (0.5 + z));
        let phase = f64::from(rnd(i, 3) * std::f32::consts::TAU);
        let sway = (phase + t * f64::from(0.6 + z)).sin() * f64::from(3.0 + 3.0 * z);
        let y = (f64::from(rnd(i, 4)) * h + v * t).rem_euclid(h) - 4.0;
        let x = (f64::from(rnd(i, 5)) * w + wind * t + sway).rem_euclid(w) - 4.0;
        let p = Pos2::new(rect.left() + x as f32, rect.top() + y as f32);
        if p.y + r > floor(p.x) - 2.0 {
            continue;
        }
        let a = ((0.3 + z * 0.5) * gain).min(0.95);
        painter.circle_filled(p, r, alpha(tk.frost, a));
    }
}

/// An ellipse that fades from `a` at its centre to nothing at 70 % of its radii
/// (`radial-gradient(…, c, transparent 70%)`), as one mesh.
fn haze(mesh: &mut Mesh, c: Pos2, rx: f32, ry: f32, color: Color32, a: f32) {
    const SEG: u32 = 40;
    const RINGS: [(f32, f32); 4] = [(0.18, 0.82), (0.36, 0.52), (0.54, 0.2), (0.70, 0.0)];
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(Vertex {
        pos: c,
        uv: WHITE_UV,
        color: alpha(color, a),
    });
    for (f, k) in RINGS {
        for s in 0..SEG {
            let ang = s as f32 / SEG as f32 * std::f32::consts::TAU;
            mesh.vertices.push(Vertex {
                pos: Pos2::new(c.x + rx * f * ang.cos(), c.y + ry * f * ang.sin()),
                uv: WHITE_UV,
                color: alpha(color, a * k),
            });
        }
    }
    let ring = |r: u32, s: u32| base + 1 + r * SEG + (s % SEG);
    for s in 0..SEG {
        mesh.add_triangle(base, ring(0, s), ring(0, s + 1));
    }
    for r in 0..(RINGS.len() as u32 - 1) {
        for s in 0..SEG {
            mesh.add_triangle(ring(r, s), ring(r + 1, s), ring(r + 1, s + 1));
            mesh.add_triangle(ring(r, s), ring(r + 1, s + 1), ring(r, s + 1));
        }
    }
}

/// A violet fog along the bottom of `band`, drifting slowly sideways (28 s there and
/// back), faint enough that what is under it reads as before.
pub fn fog(painter: &Painter, band: Rect, t: f64, tk: &Tokens) {
    if band.width() < 16.0 || band.height() < 8.0 {
        return;
    }
    let (w, h) = (band.width(), band.height());
    let drift = ((t * std::f64::consts::TAU / 28.0).sin() as f32) * 0.05 * w;
    let slow = ((t * std::f64::consts::TAU / 41.0).sin() as f32) * 0.03 * w;
    let mut mesh = Mesh::default();
    for (cx, rx, ry, a, dx) in [
        (0.28, 0.62, 1.0, 0.17, drift),
        (0.74, 0.52, 0.86, 0.14, -drift * 0.8 + slow),
        (0.50, 0.40, 0.55, 0.08, slow - drift * 0.4),
    ] {
        haze(
            &mut mesh,
            Pos2::new(band.left() + cx * w + dx, band.bottom()),
            rx * w,
            ry * h,
            tk.info,
            a,
        );
    }
    painter.with_clip_rect(band).add(Shape::mesh(mesh));
}

/// A candle's irregular breath, 0.25–1: slow swells, quick wavers and a rare dip.
pub fn flicker(t: f64) -> f32 {
    let s = (t * 2.1).sin() * (t * 0.7 + 1.0).sin() * 0.18
        + (t * 9.7 + 0.4).sin() * 0.09
        + (t * 23.0).sin() * 0.05;
    let beat = (t * 1.7).floor();
    let dip = rnd(beat.rem_euclid(1.0e6) as u32, 7) > 0.8 && (t * 1.7).fract() < 0.12;
    let v = 0.68 + s as f32;
    (if dip { v * 0.5 } else { v }).clamp(0.25, 1.0)
}

/// A warm glow behind `rect` (the title's words), breathing like a candle; steady at
/// mid strength when still.
pub fn candle(painter: &Painter, rect: Rect, f: &Fx, tk: &Tokens) {
    let k = if f.still { 0.6 } else { flicker(f.t) };
    let glow = Shadow {
        offset: [0, 0],
        blur: 18,
        spread: 1,
        color: alpha(tk.accent, 0.42 * k),
    };
    painter.add(glow.as_shape(rect.expand2(egui::vec2(4.0, -1.0)), CornerRadius::same(8)));
}

/// A cobweb in a corner at `corner`, `size` long, opening towards `dir` (each axis
/// ±1: right and down is (1, 1)): threads from the corner and sagging rings between
/// them, faint.
pub fn cobweb(painter: &Painter, corner: Pos2, size: f32, dir: egui::Vec2, tk: &Tokens) {
    const SPOKES: [(f32, f32); 6] = [
        (0.0, 0.96),
        (0.3, 0.82),
        (0.62, 1.0),
        (0.95, 0.86),
        (1.25, 0.94),
        (std::f32::consts::FRAC_PI_2, 0.9),
    ];
    let spoke = Stroke::new(1.0, alpha(tk.fg, 0.24));
    let ring = Stroke::new(1.0, alpha(tk.fg, 0.17));
    let at = |ang: f32, len: f32| {
        Pos2::new(
            corner.x + dir.x * ang.cos() * len,
            corner.y + dir.y * ang.sin() * len,
        )
    };
    for (ang, k) in SPOKES {
        painter.line_segment([corner, at(ang, size * k)], spoke);
    }
    for f in [0.24, 0.42, 0.6, 0.78] {
        for w in SPOKES.windows(2) {
            let (Some(&(a0, k0)), Some(&(a1, k1))) = (w.first(), w.get(1)) else {
                continue;
            };
            let (p0, p1) = (
                at(a0, size * f * k0.min(1.0)),
                at(a1, size * f * k1.min(1.0)),
            );
            // Sagging towards the corner, as a thread under its own weight.
            let mid = Pos2::new((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
            let ctrl = corner + (mid - corner) * 0.86;
            let pts: Vec<Pos2> = (0..=6)
                .map(|i| {
                    let t = i as f32 / 6.0;
                    let u = 1.0 - t;
                    Pos2::new(
                        u * u * p0.x + 2.0 * u * t * ctrl.x + t * t * p1.x,
                        u * u * p0.y + 2.0 * u * t * ctrl.y + t * t * p1.y,
                    )
                })
                .collect();
            painter.add(Shape::line(pts, ring));
        }
    }
}

/// A small spider hanging `drop` points below `top` on its thread.
pub fn spider(painter: &Painter, top: Pos2, drop: f32, sway: f32, tk: &Tokens) {
    let ink = lerp(tk.fg, tk.bg, 0.22);
    let thread = Stroke::new(1.0, alpha(tk.fg, 0.4));
    let c = Pos2::new(top.x + sway, top.y + 6.0 + drop.max(0.0));
    painter.line_segment([top, Pos2::new(c.x, c.y - 7.0)], thread);
    let leg = Stroke::new(1.4, ink);
    for (dx0, dy0, kx, ky, dx1, dy1) in [
        (2.5, -2.0, 6.0, -5.5, 8.5, -2.0),
        (3.0, -0.5, 7.5, -2.5, 10.0, 1.0),
        (3.0, 1.0, 7.5, 2.0, 9.5, 5.0),
        (2.5, 2.5, 6.0, 5.5, 7.5, 8.5),
    ] {
        for side in [-1.0, 1.0] {
            let p = |x: f32, y: f32| Pos2::new(c.x + side * x, c.y + y);
            painter.add(Shape::line(vec![p(dx0, dy0), p(kx, ky), p(dx1, dy1)], leg));
        }
    }
    painter.add(Shape::ellipse_filled(c, egui::vec2(3.6, 4.6), ink));
    painter.circle_filled(Pos2::new(c.x, c.y - 5.6), 2.4, ink);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flakes_scale_with_the_window() {
        let r = |w, h| Rect::from_min_size(Pos2::ZERO, egui::vec2(w, h));
        assert_eq!(flake_count(r(1280.0, 800.0)), 100);
        assert!(flake_count(r(1024.0, 700.0)) < 100);
        assert_eq!(flake_count(r(10.0, 10.0)), 30);
        assert_eq!(flake_count(r(8000.0, 8000.0)), 220);
    }

    #[test]
    fn the_candle_stays_in_its_range() {
        for i in 0..2000 {
            let k = flicker(f64::from(i) * 0.037);
            assert!((0.25..=1.0).contains(&k), "{k}");
        }
        assert!((0.0..1.0).contains(&rnd(3, 4)));
    }
}
