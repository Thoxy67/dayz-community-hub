//! `ui/topo`: contour lines, as on a survey map of Chernarus: the texture behind empty
//! states and the rail's foot. Decorative, in the border colour at half strength, and
//! never behind anything that has to be read.
//!
//! The ten paths of `topo.svelte` (viewBox 400×300, `xMidYMid slice`), parsed once into
//! cubic segments and drawn with the painter.

use std::sync::OnceLock;

use egui::epaint::CubicBezierShape;
use egui::{Color32, Painter, Pos2, Rect, Stroke};

use super::paint::alpha;
use crate::theme::Tokens;

const PATHS: [&str; 10] = [
    "M-10 250 C 60 200, 120 260, 190 220 S 320 150, 410 190",
    "M-10 226 C 70 176, 130 236, 196 196 S 318 128, 410 166",
    "M-10 202 C 80 150, 140 210, 202 172 S 316 106, 410 142",
    "M30 175 C 90 128, 150 186, 208 150 S 300 92, 360 104 S 404 96, 410 92",
    "M70 150 C 120 108, 168 160, 214 130 S 290 84, 330 90",
    "M120 128 C 150 100, 186 136, 220 114 S 270 84, 296 90 C 280 116, 230 146, 180 150 C 150 152, 128 142, 120 128 Z",
    "M168 118 C 184 106, 204 120, 226 106 C 244 96, 260 100, 262 104 C 248 122, 212 134, 190 132 C 176 131, 168 126, 168 118 Z",
    "M-10 60 C 40 40, 90 80, 150 56 S 250 10, 300 26 S 380 60, 410 44",
    "M-10 36 C 50 16, 100 56, 156 32 S 246 -10, 300 4",
    "M260 300 C 280 270, 330 262, 360 280 S 400 300, 410 296",
];

type Seg = [[f32; 2]; 4];

/// The paths as cubic segments in viewBox units. Only absolute M, C, S and Z occur.
fn segments() -> &'static [Seg] {
    static SEGS: OnceLock<Vec<Seg>> = OnceLock::new();
    SEGS.get_or_init(|| {
        let mut out = Vec::new();
        for d in PATHS {
            let nums: Vec<String> = d
                .replace(',', " ")
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            let mut it = nums.iter().peekable();
            let mut cur = [0.0_f32; 2];
            let mut start = cur;
            let mut last_ctrl: Option<[f32; 2]> = None;
            let mut cmd = 'M';
            let num = |s: Option<&String>| s.and_then(|s| s.parse::<f32>().ok());
            while let Some(tok) = it.peek().map(|s| s.as_str()) {
                let lead = tok.chars().next().unwrap_or(' ');
                if lead.is_ascii_alphabetic() {
                    cmd = lead;
                    let rest = tok.get(1..).unwrap_or("").to_owned();
                    it.next();
                    if cmd == 'Z' {
                        if cur != start {
                            out.push([cur, cur, start, start]);
                        }
                        cur = start;
                        last_ctrl = None;
                        continue;
                    }
                    if !rest.is_empty() {
                        // "M-10": the number is glued to the command.
                        let (Some(x), Some(y)) = (rest.parse::<f32>().ok(), num(it.next())) else {
                            break;
                        };
                        if cmd == 'M' {
                            cur = [x, y];
                            start = cur;
                        }
                        continue;
                    }
                }
                match cmd {
                    'M' => {
                        let (Some(x), Some(y)) = (num(it.next()), num(it.next())) else {
                            break;
                        };
                        cur = [x, y];
                        start = cur;
                    }
                    'C' => {
                        let v: Vec<f32> = (0..6).filter_map(|_| num(it.next())).collect();
                        let [a, b, c, d2, e, f] = v.as_slice() else {
                            break;
                        };
                        out.push([cur, [*a, *b], [*c, *d2], [*e, *f]]);
                        last_ctrl = Some([*c, *d2]);
                        cur = [*e, *f];
                    }
                    'S' => {
                        let v: Vec<f32> = (0..4).filter_map(|_| num(it.next())).collect();
                        let [c, d2, e, f] = v.as_slice() else { break };
                        let c1 =
                            last_ctrl.map_or(cur, |l| [2.0 * cur[0] - l[0], 2.0 * cur[1] - l[1]]);
                        out.push([cur, c1, [*c, *d2], [*e, *f]]);
                        last_ctrl = Some([*c, *d2]);
                        cur = [*e, *f];
                    }
                    _ => break,
                }
            }
        }
        out
    })
}

/// Paints the contours over `rect` (`opacity` 0.5 is the Svelte default).
pub fn paint(painter: &Painter, rect: Rect, t: &Tokens, opacity: f32) {
    let color = alpha(t.border, opacity * f32::from(t.border.a()) / 255.0);
    paint_color(painter, rect, color);
}

/// The contours in another ink (`<Topo class="text-map" opacity={0.28} />`): `color` at
/// `opacity`.
pub fn paint_tinted(painter: &Painter, rect: Rect, color: Color32, opacity: f32) {
    paint_color(
        painter,
        rect,
        alpha(color, opacity * f32::from(color.a()) / 255.0),
    );
}

fn paint_color(painter: &Painter, rect: Rect, color: Color32) {
    if !rect.is_positive() {
        return;
    }
    // xMidYMid slice: scale to cover, centred.
    let s = (rect.width() / 400.0).max(rect.height() / 300.0);
    let ox = rect.center().x - 200.0 * s;
    let oy = rect.center().y - 150.0 * s;
    let map = |p: [f32; 2]| Pos2::new(ox + p[0] * s, oy + p[1] * s);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    for seg in segments() {
        let pts = [map(seg[0]), map(seg[1]), map(seg[2]), map(seg[3])];
        painter.add(CubicBezierShape::from_points_stroke(
            pts,
            false,
            Color32::TRANSPARENT,
            Stroke::new(1.0, color),
        ));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_path_parses() {
        // 2+2+2+3+2+4+4+3+2+2 cubic segments; the two closed paths end where they start,
        // so their Z adds none.
        assert_eq!(super::segments().len(), 26);
    }
}
