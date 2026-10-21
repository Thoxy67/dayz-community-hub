//! A whole theme from two colours, the window floor and the accent (= `lib/theme/derive.ts`):
//! the "Derive" button of the theme editor. Everything is made in OKLCH so a step in
//! lightness looks the same at every hue, and pushed until it reaches the contrast the
//! presets are held to (text 7:1, muted 4.5:1, faint 3:1, the accent's label 4.5:1,
//! the meaning colours 3:1 on the panel).

use crate::theme::{Oklch, Raw, Scheme, mix, rgb};

/// Two meaning colours closer than this (OKLab distance) are too alike.
const DISTINCT_MIN: f32 = 0.06;

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// A colour as written: lightness 0–1, chroma never negative.
fn fmt(c: Oklch) -> Oklch {
    [clamp(c[0], 0.0, 1.0), c[1].max(0.0), c[2].rem_euclid(360.0)]
}

/// WCAG relative luminance of the colour once drawn in sRGB.
fn luminance(c: Oklch) -> f32 {
    let px = rgb(fmt(c));
    let lin = |v: u8| {
        let x = f32::from(v) / 255.0;
        if x <= 0.040_45 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(px.r()) + 0.7152 * lin(px.g()) + 0.0722 * lin(px.b())
}

/// The WCAG contrast ratio of two colours.
pub fn wcag(a: Oklch, b: Oklch) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// Distance in OKLab.
fn delta_e(x: Oklch, y: Oklch) -> f32 {
    let (sx, cx) = x[2].to_radians().sin_cos();
    let (sy, cy) = y[2].to_radians().sin_cos();
    let (ax, bx) = (x[1] * cx, x[1] * sx);
    let (ay, by) = (y[1] * cy, y[1] * sy);
    ((x[0] - y[0]).powi(2) + (ax - ay).powi(2) + (bx - by).powi(2)).sqrt()
}

/// Move `c` in lightness (`dir` ±1) until it reaches `min` against every ground.
fn reach(c: Oklch, grounds: &[Oklch], min: f32, dir: f32) -> Oklch {
    let mut out = c;
    for _ in 0..40 {
        if grounds.iter().all(|g| wcag(fmt(out), *g) >= min) {
            break;
        }
        out[0] = clamp(out[0] + dir * 0.01, 0.0, 1.0);
    }
    out
}

/// The scheme and the twelve colours made from `bg` and `accent`.
pub fn derive(bg: Oklch, accent: Oklch) -> (Scheme, Raw) {
    let mut accent = accent;
    let dark = bg[0] < 0.6;
    let toward = if dark { 1.0 } else { -1.0 };

    let tint = (bg[1] * 1.4 + 0.004).min(0.06);
    let panel = [
        clamp(bg[0] + if dark { 0.035 } else { 0.05 }, 0.0, 0.995),
        tint,
        bg[2],
    ];
    let raised = [
        clamp(bg[0] + if dark { 0.085 } else { -0.04 }, 0.0, 1.0),
        tint * 1.3,
        bg[2],
    ];
    let grounds = [fmt(bg), fmt(panel)];

    let mut fg = reach(
        [if dark { 0.92 } else { 0.24 }, 0.02, bg[2]],
        &grounds,
        7.0,
        toward,
    );
    // Muted and faint text are mixed from fg and the floor; they pass too.
    for _ in 0..20 {
        let muted = mix(fmt(fg), 0.68, fmt(bg));
        let faint = mix(fmt(fg), 0.5, fmt(bg));
        if grounds
            .iter()
            .all(|g| wcag(muted, *g) >= 4.5 && wcag(faint, *g) >= 3.0)
        {
            break;
        }
        fg[0] = clamp(fg[0] + toward * 0.01, 0.0, 1.0);
    }

    // The accent's label: whichever end contrasts more.
    let ends = |a: Oklch| -> (Oklch, f32, f32) {
        let black = [0.18, a[1].min(0.03), a[2]];
        let white = [0.985, a[1].min(0.015), a[2]];
        let cb = wcag(black, a);
        let cw = wcag(white, a);
        if cb >= cw {
            (black, cb, 1.0)
        } else {
            (white, cw, -1.0)
        }
    };
    let mut label = ends(accent);
    for _ in 0..20 {
        if label.1 >= 4.5 {
            break;
        }
        accent[0] = clamp(accent[0] + label.2 * 0.01, 0.0, 1.0);
        label = ends(accent);
    }
    // A mark must also stand out from the panel.
    accent = reach(accent, &[fmt(panel)], 3.0, toward);

    let tone = |h: f32, c: f32| -> Oklch {
        let mut t = reach(
            [if dark { 0.74 } else { 0.52 }, c, h],
            &[fmt(panel)],
            3.0,
            toward,
        );
        for _ in 0..12 {
            if delta_e(fmt(t), fmt(accent)) >= DISTINCT_MIN {
                break;
            }
            t[0] = clamp(t[0] + toward * 0.02, 0.0, 1.0);
        }
        fmt(t)
    };

    let raw = Raw {
        bg: fmt(bg),
        panel: fmt(panel),
        raised: fmt(raised),
        fg: fmt(fg),
        accent: fmt(accent),
        accent_fg: fmt(label.0),
        ok: tone(140.0, 0.14),
        warn: tone(65.0, 0.145),
        err: tone(28.0, 0.17),
        info: tone(240.0, 0.08),
        map: tone(90.0, 0.08),
        mods: tone(175.0, 0.09),
    };
    (if dark { Scheme::Dark } else { Scheme::Light }, raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_themes_are_legible() {
        for (bg, accent) in [
            ([0.18, 0.01, 130.0], [0.79, 0.14, 85.0]),
            ([0.97, 0.01, 90.0], [0.55, 0.15, 250.0]),
            ([0.3, 0.0, 0.0], [0.6, 0.2, 30.0]),
        ] {
            let (scheme, raw) = derive(bg, accent);
            assert_eq!(scheme == Scheme::Dark, bg[0] < 0.6);
            assert!(wcag(raw.fg, raw.bg) >= 6.9, "{bg:?}");
            assert!(wcag(raw.accent_fg, raw.accent) >= 4.0, "{accent:?}");
            assert!(wcag(raw.ok, raw.panel) >= 2.9);
        }
    }
}
