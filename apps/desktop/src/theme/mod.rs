//! The tokens: every colour, size and measure the interface is made of, as
//! the Svelte interface's `styles/tokens.css` defined them.
//!
//! A theme is twelve raw colours ([`Raw`], in oklch); everything else is mixed
//! from them in [`Tokens::derive`] exactly as the CSS mixed them (`color-mix(in
//! oklch, …)`), so no preset can forget a colour. Views never name a colour:
//! they read `Tokens` (`ui.tokens()`), sizes from [`size`], type from [`font`].

pub mod fonts;
pub mod presets;

use egui::Color32;

/// Whether a theme is drawn dark or light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Dark,
    Light,
}

/// An oklch colour: lightness 0–1, chroma, hue in degrees.
pub type Oklch = [f32; 3];

/// The twelve colours a theme is made of.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Raw {
    pub bg: Oklch,
    pub panel: Oklch,
    pub raised: Oklch,
    pub fg: Oklch,
    pub accent: Oklch,
    pub accent_fg: Oklch,
    pub ok: Oklch,
    pub warn: Oklch,
    pub err: Oklch,
    pub info: Oklch,
    pub map: Oklch,
    pub mods: Oklch,
}

/// Every colour the interface uses, derived from a [`Raw`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    pub scheme: Scheme,
    // surfaces
    pub bg: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub sunken: Color32,
    pub overlay: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub scrim: Color32,
    /// The deepest ground: dialog veils, terminal output, the sparkline's well.
    pub plot: Color32,
    // text
    pub fg: Color32,
    pub fg_muted: Color32,
    pub fg_faint: Color32,
    // meaning
    pub accent: Color32,
    pub accent_fg: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub info: Color32,
    pub map: Color32,
    pub mods: Color32,
}

impl Tokens {
    pub fn derive(raw: &Raw, scheme: Scheme) -> Self {
        const BLACK: Oklch = [0.0, 0.0, 0.0];
        const WHITE: Oklch = [1.0, 0.0, 0.0];
        let (sunken, plot) = match scheme {
            Scheme::Dark => (mix(raw.bg, 0.70, BLACK), mix(raw.bg, 0.55, BLACK)),
            Scheme::Light => (mix(raw.bg, 0.55, WHITE), mix(raw.bg, 0.35, WHITE)),
        };
        Self {
            scheme,
            bg: rgb(raw.bg),
            panel: rgb(raw.panel),
            raised: rgb(raw.raised),
            sunken: rgb(sunken),
            overlay: rgb(mix(raw.raised, 0.88, raw.fg)),
            border: mix_partial(raw.raised, 0.72, raw.fg, 0.12),
            border_strong: mix_partial(raw.raised, 0.55, raw.fg, 0.30),
            scrim: with_alpha(rgb(raw.bg), 0.55),
            plot: rgb(plot),
            fg: rgb(raw.fg),
            fg_muted: rgb(mix(raw.fg, 0.68, raw.bg)),
            // Faint at half way: at 46% most dark presets fell under 3:1 on their panels.
            fg_faint: rgb(mix(raw.fg, 0.50, raw.bg)),
            accent: rgb(raw.accent),
            accent_fg: rgb(raw.accent_fg),
            ok: rgb(raw.ok),
            warn: rgb(raw.warn),
            err: rgb(raw.err),
            info: rgb(raw.info),
            map: rgb(raw.map),
            mods: rgb(raw.mods),
        }
    }

    pub fn is_dark(&self) -> bool {
        self.scheme == Scheme::Dark
    }

    /// `color-mix(in oklch, c pct%, transparent)`: a tint of a meaning colour.
    pub fn tint(c: Color32, pct: f32) -> Color32 {
        with_alpha(c, pct)
    }
}

impl Default for Tokens {
    fn default() -> Self {
        match presets::by_id(presets::DEFAULT_DARK) {
            Some(p) => Tokens::derive(&p.raw, p.scheme),
            None => Tokens::derive(&FALLBACK, Scheme::Dark),
        }
    }
}

/// Chernarus, should the preset table ever lose it.
const FALLBACK: Raw = Raw {
    bg: [0.183, 0.007, 135.0],
    panel: [0.217, 0.012, 129.0],
    raised: [0.266, 0.017, 127.0],
    fg: [0.914, 0.022, 98.0],
    accent: [0.788, 0.141, 85.0],
    accent_fg: [0.195, 0.019, 98.0],
    ok: [0.748, 0.141, 130.0],
    warn: [0.734, 0.144, 62.0],
    err: [0.619, 0.170, 32.0],
    info: [0.701, 0.063, 246.0],
    map: [0.770, 0.078, 88.0],
    mods: [0.747, 0.086, 173.0],
};

/// `color-mix(in oklch, a p, b)`: interpolate lightness, chroma and hue (shorter
/// arc). An achromatic side has no hue and takes the other's, as CSS does.
pub fn mix(a: Oklch, p: f32, b: Oklch) -> Oklch {
    let q = 1.0 - p;
    let (mut ha, mut hb) = (a[2], b[2]);
    if a[1] < 1e-4 {
        ha = hb;
    }
    if b[1] < 1e-4 {
        hb = ha;
    }
    let mut d = hb - ha;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    [
        a[0] * p + b[0] * q,
        a[1] * p + b[1] * q,
        (ha + d * q).rem_euclid(360.0),
    ]
}

/// `color-mix(in oklch, a pa, b pb)` with `pa + pb < 1`: the mix normalised, its
/// alpha the sum (CSS's rule for percentages that leave a gap).
fn mix_partial(a: Oklch, pa: f32, b: Oklch, pb: f32) -> Color32 {
    let sum = pa + pb;
    with_alpha(rgb(mix(a, pa / sum, b)), sum)
}

pub fn with_alpha(c: Color32, a: f32) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// oklch to 8-bit sRGB, clipped to the gamut (bottosson.github.io/posts/oklab).
pub fn rgb([l, c, h]: Oklch) -> Color32 {
    let (s, co) = h.to_radians().sin_cos();
    let (a, b) = (c * co, c * s);
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let enc = |x: f32| {
        let v = if x >= 0.003_130_8 {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        } else {
            12.92 * x
        };
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    };
    Color32::from_rgb(enc(r), enc(g), enc(b))
}

/// sRGB to oklch, for the custom theme's colour pickers.
pub fn oklch(c: Color32) -> Oklch {
    let lin = |v: u8| {
        let x = f32::from(v) / 255.0;
        if x >= 0.040_45 {
            ((x + 0.055) / 1.055).powf(2.4)
        } else {
            x / 12.92
        }
    };
    let (r, g, b) = (lin(c.r()), lin(c.g()), lin(c.b()));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let aa = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    [
        ll,
        aa.hypot(bb),
        bb.atan2(aa).to_degrees().rem_euclid(360.0),
    ]
}

/// Sizes, from the `--spacing-*`, `--text-*` and `--radius-*` tokens (CSS px = egui points).
pub mod size {
    pub const CONTROL_SM: f32 = 22.0;
    pub const CONTROL: f32 = 28.0;
    pub const CONTROL_LG: f32 = 34.0;
    pub const ROW: f32 = 44.0;
    pub const TITLEBAR: f32 = 34.0;
    pub const RAIL: f32 = 208.0;
    pub const RAIL_COLLAPSED: f32 = 52.0;
    pub const ICON_SM: f32 = 14.0;
    pub const ICON: f32 = 16.0;
    pub const ICON_LG: f32 = 20.0;
    /// The horizontal air inside a view, a panel, a toolbar.
    pub const PAD: f32 = 12.0;

    pub const TEXT_3XS: f32 = 10.0;
    pub const TEXT_2XS: f32 = 11.0;
    pub const TEXT_XS: f32 = 12.0;
    pub const TEXT_SM: f32 = 13.0;
    pub const TEXT_BASE: f32 = 14.0;
    pub const TEXT_LG: f32 = 17.0;
    /// Display sizes: only in the display face.
    pub const TEXT_XL: f32 = 22.0;
    pub const TEXT_2XL: f32 = 30.0;
    pub const TEXT_3XL: f32 = 44.0;

    pub const RADIUS_XS: u8 = 2;
    pub const RADIUS_SM: u8 = 4;
    pub const RADIUS_MD: u8 = 6;
    pub const RADIUS_LG: u8 = 10;
}

/// Durations (`--duration-*`), in seconds. Motion is subtle: ≤ 6 px, ease-out, never a bounce.
pub mod motion {
    pub const FAST: f32 = 0.120;
    pub const BASE: f32 = 0.180;
    pub const SLOW: f32 = 0.240;
}

pub use fonts::font;

/// The tokens worn now, kept in egui's memory so any `Ui` can read them.
pub trait TokensExt {
    fn tokens(&self) -> Tokens;
}

impl TokensExt for egui::Context {
    fn tokens(&self) -> Tokens {
        self.data(|d| d.get_temp::<Tokens>(egui::Id::NULL))
            .unwrap_or_default()
    }
}

impl TokensExt for egui::Ui {
    fn tokens(&self) -> Tokens {
        self.ctx().tokens()
    }
}

/// Wear `t`: store it for [`TokensExt`] and dress egui's own widgets with it.
pub fn apply(ctx: &egui::Context, t: Tokens) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, t));
    let mut v = if t.is_dark() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = t.panel;
    v.window_fill = t.raised;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.window_corner_radius = egui::CornerRadius::same(size::RADIUS_LG);
    v.menu_corner_radius = egui::CornerRadius::same(size::RADIUS_MD);
    v.extreme_bg_color = t.sunken;
    v.faint_bg_color = t.bg;
    v.code_bg_color = t.sunken;
    v.hyperlink_color = t.accent;
    v.warn_fg_color = t.warn;
    v.error_fg_color = t.err;
    v.override_text_color = Some(t.fg);
    v.selection.bg_fill = with_alpha(t.accent, 0.35);
    v.selection.stroke = egui::Stroke::new(1.0, t.accent);
    v.text_cursor.stroke = egui::Stroke::new(1.5, t.accent);
    let r = egui::CornerRadius::same(size::RADIUS_SM);
    for (w, fill, stroke) in [
        (&mut v.widgets.noninteractive, t.panel, t.border),
        (&mut v.widgets.inactive, t.raised, t.border),
        (&mut v.widgets.hovered, t.overlay, t.border_strong),
        (&mut v.widgets.active, t.overlay, t.accent),
        (&mut v.widgets.open, t.raised, t.border_strong),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.bg_stroke = egui::Stroke::new(1.0, stroke);
        w.corner_radius = r;
        w.fg_stroke.color = t.fg;
        w.expansion = 0.0;
    }
    v.widgets.noninteractive.fg_stroke.color = t.fg_muted;
    v.popup_shadow = egui::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(128),
    };
    v.window_shadow = v.popup_shadow;
    // The theme is ours, not the OS's: both of egui's slots wear it.
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(10.0, 4.0);
        s.spacing.interact_size.y = size::CONTROL;
        s.animation_time = motion::FAST;
        s.text_styles = fonts::text_styles();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chernarus_bg_matches_the_browser() {
        // oklch(18.3% 0.007 135) is #111310 in Chromium (var/reference).
        let c = rgb([0.183, 0.007, 135.0]);
        assert!((i32::from(c.r()) - 0x11).abs() <= 1, "{c:?}");
        assert!((i32::from(c.g()) - 0x13).abs() <= 1, "{c:?}");
        assert!((i32::from(c.b()) - 0x10).abs() <= 1, "{c:?}");
    }

    #[test]
    fn oklch_round_trips() {
        let c = Color32::from_rgb(0xd4, 0xa0, 0x3c);
        assert_eq!(rgb(oklch(c)), c);
    }

    #[test]
    fn every_preset_derives() {
        for p in presets::PRESETS {
            let t = Tokens::derive(&p.raw, p.scheme);
            assert_ne!(t.fg, t.bg, "{}", p.id);
        }
        assert!(presets::by_id("mocha").is_some());
    }
}
