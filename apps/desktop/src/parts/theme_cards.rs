//! Themes shown, not described: `ThemePreview` (a theme drawn small in its own colours) and
//! `ThemeSwatches` (every preset as a small window, plus "follow the system").

use std::sync::OnceLock;

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::text::{draw, ph_trunc};
use crate::i18n::t;
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Side, TipExt};
use crate::theme::presets::{PRESETS, Preset};
use crate::theme::{Scheme, Tokens, TokensExt, font, fonts::Face, size};

/// A pill (`rounded-full`) from `x`, `w` wide, `h` tall, centred on `cy`.
fn pill(p: &egui::Painter, x: f32, cy: f32, w: f32, h: f32, c: Color32) {
    let r = Rect::from_min_size(Pos2::new(x, cy - h / 2.0), Vec2::new(w.max(0.0), h));
    p.rect_filled(r, CornerRadius::same((h / 2.0) as u8), c);
}

/// `ThemePreview`: a theme drawn small in its own colours in `rect` (72px tall in the
/// swatches): a title bar with the accent mark, a rail, two server rows (the second
/// selected) each with a ping signal, a players bar and a map name, and the play button.
/// `round` rounds its corners (`rounded-sm`).
pub fn theme_preview(p: &egui::Painter, rect: Rect, tk: &Tokens, round: bool) {
    let radius = if round { size::RADIUS_SM } else { 0 };
    let p = p.with_clip_rect(rect.intersect(p.clip_rect()));
    p.rect_filled(rect, CornerRadius::same(radius), tk.bg);
    let border = tk.border;
    // Title bar.
    let tb = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 14.0));
    p.hline(tb.x_range(), tb.bottom() - 0.5, Stroke::new(1.0, border));
    let cy = tb.center().y;
    p.rect_filled(
        Rect::from_min_size(Pos2::new(tb.left() + 6.0, cy - 3.0), Vec2::splat(6.0)),
        CornerRadius::same(size::RADIUS_XS),
        tk.accent,
    );
    pill(&p, tb.left() + 16.0, cy, 32.0, 4.0, tk.fg);
    pill(&p, tb.right() - 6.0 - 16.0, cy, 16.0, 4.0, tk.fg_faint);
    // Rail.
    let body = Rect::from_min_max(Pos2::new(rect.left(), tb.bottom()), rect.max);
    let rail_w = body.width() / 5.0;
    let rail = Rect::from_min_size(body.min, Vec2::new(rail_w, body.height()));
    p.vline(rail.right() - 0.5, rail.y_range(), Stroke::new(1.0, border));
    let inner = rail_w - 8.0;
    let mut y = rail.top() + 6.0 + 2.0;
    for (w, c) in [
        (1.0, tk.accent),
        (2.0 / 3.0, tk.fg_muted),
        (0.75, tk.fg_muted),
    ] {
        pill(&p, rail.left() + 4.0, y, inner * w, 4.0, c);
        y += 8.0;
    }
    // List.
    let list = Rect::from_min_max(Pos2::new(rail.right(), body.top()), body.max);
    p.rect_filled(list, CornerRadius::ZERO, tk.panel);
    let rows = [(tk.ok, 3, 0.70, false), (tk.warn, 2, 0.35, true)];
    for (i, (ping, bars, fill, mods)) in rows.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(list.left(), list.top() + i as f32 * 16.0),
            Vec2::new(list.width(), 16.0),
        );
        if i == 1 {
            p.rect_filled(r, CornerRadius::ZERO, alpha(tk.accent, 0.12));
            p.rect_filled(
                Rect::from_min_size(r.min, Vec2::new(2.0, r.height())),
                CornerRadius::ZERO,
                tk.accent,
            );
        }
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, border));
        let cy = r.center().y;
        let mut x = r.left() + 4.0;
        for b in 1..=3 {
            let h = b as f32 * 2.0;
            let c = if b <= *bars { *ping } else { border };
            p.rect_filled(
                Rect::from_min_size(Pos2::new(x, cy + 3.0 - h), Vec2::new(1.0, h)),
                CornerRadius::ZERO,
                c,
            );
            x += 2.0;
        }
        x += 3.0;
        pill(&p, x, cy, 16.0, 4.0, tk.raised);
        p.rect_filled(
            Rect::from_min_size(Pos2::new(x, cy - 2.0), Vec2::new(16.0 * fill, 4.0)),
            CornerRadius::same(2),
            tk.accent,
        );
        x += 16.0 + 4.0;
        let tail = 4.0 + 12.0 + if *mods { 4.0 + 6.0 } else { 0.0 } + 4.0;
        let name_w = (r.right() - tail - x).max(0.0);
        pill(&p, x, cy, name_w, 4.0, tk.fg);
        x += name_w + 4.0;
        pill(&p, x, cy, 12.0, 4.0, tk.map);
        if *mods {
            pill(&p, x + 16.0, cy, 6.0, 4.0, tk.mods);
        }
    }
    let fy = list.bottom() - 4.0 - 5.0;
    pill(&p, list.left() + 4.0, fy, 8.0, 4.0, tk.err);
    pill(&p, list.left() + 16.0, fy, 8.0, 4.0, tk.info);
    let play = Rect::from_min_size(
        Pos2::new(list.right() - 4.0 - 28.0, fy - 5.0),
        Vec2::new(28.0, 10.0),
    );
    p.rect_filled(play, CornerRadius::same(size::RADIUS_XS), tk.accent);
    pill(
        &p,
        play.center().x - 6.0,
        play.center().y,
        12.0,
        2.0,
        tk.accent_fg,
    );
    p.rect_stroke(
        rect,
        CornerRadius::same(radius),
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
}

/// What a [`ThemeSwatches`] click picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemePick {
    /// Follow the system (a dark and a light preset).
    System,
    Preset(&'static str),
}

/// Every preset's tokens, derived once; dark ones first.
fn ordered() -> &'static [(&'static Preset, Tokens)] {
    static ALL: OnceLock<Vec<(&'static Preset, Tokens)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut v: Vec<_> = PRESETS
            .iter()
            .map(|p| (p, Tokens::derive(&p.raw, p.scheme)))
            .collect();
        v.sort_by_key(|(p, _)| p.scheme == Scheme::Light);
        v
    })
}

/// A preset's name as shown: the house themes' names are translated.
pub fn preset_name(p: &Preset) -> &'static str {
    match p.id {
        "chernarus" => t("theme", "presetChernarus"),
        "topo" => t("theme", "presetTopo"),
        "halloween" => t("theme", "presetHalloween"),
        "christmas" => t("theme", "presetChristmas"),
        "powder" => t("theme", "presetPowder"),
        _ => p.name,
    }
}

/// `ThemeSwatches`: every preset as a small window in its own colours, dark ones first,
/// after "follow the system" (drawn as its dark and light halves). Answers the one clicked.
pub struct ThemeSwatches<'a> {
    selected: Option<&'a str>,
    system: (Tokens, Tokens),
    system_label: &'a str,
    columns: usize,
}

impl<'a> ThemeSwatches<'a> {
    /// `selected` is the worn preset's id, `None` when following the system, whose pair of
    /// looks is `system` (dark, light).
    pub fn new(selected: Option<&'a str>, system: (Tokens, Tokens), system_label: &'a str) -> Self {
        Self {
            selected,
            system,
            system_label,
            columns: 4,
        }
    }

    /// 3 to 6 per line.
    pub fn columns(mut self, n: usize) -> Self {
        self.columns = n.clamp(3, 6);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Option<ThemePick> {
        let tk = ui.tokens();
        let all = ordered();
        let n = all.len() + 1;
        let cols = self.columns;
        let gap = 6.0;
        let w = ui.available_width();
        let cw = ((w - gap * (cols - 1) as f32) / cols as f32).max(40.0);
        // p-1 · preview 72 · gap-1 · label line (text-3xs 12) · p-1
        let ch = 4.0 + 72.0 + 4.0 + 12.0 + 4.0;
        let rows = n.div_ceil(cols);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(w, rows as f32 * ch + rows.saturating_sub(1) as f32 * gap),
            Sense::hover(),
        );
        if !ui.is_rect_visible(rect) {
            return None;
        }
        let mut out = None;
        for i in 0..n {
            let r = Rect::from_min_size(
                Pos2::new(
                    rect.left() + (i % cols) as f32 * (cw + gap),
                    rect.top() + (i / cols) as f32 * (ch + gap),
                ),
                Vec2::new(cw, ch),
            );
            let (pick, label, tip, on) = match i.checked_sub(1).and_then(|k| all.get(k)) {
                None => (
                    ThemePick::System,
                    self.system_label,
                    "",
                    self.selected.is_none(),
                ),
                Some((p, _)) => (
                    ThemePick::Preset(p.id),
                    preset_name(p),
                    t("theme", p.blurb),
                    self.selected == Some(p.id),
                ),
            };
            let b = ui.interact(r, resp.id.with(i), Sense::click());
            let h = paint::ease(ui, b.id.with("hover"), b.hovered());
            let p = ui.painter();
            if on {
                p.add(paint::glow_soft(&tk).as_shape(r, CornerRadius::same(size::RADIUS_MD)));
                // The glow is a filled shape: the page's ground goes back under the tint.
                p.rect_filled(r, CornerRadius::same(size::RADIUS_MD), tk.bg);
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
            let pr = Rect::from_min_size(r.min + Vec2::splat(4.0), Vec2::new(cw - 8.0, 72.0));
            match i.checked_sub(1).and_then(|k| all.get(k)) {
                Some((_, look)) => theme_preview(p, pr, look, true),
                None => {
                    let half = (pr.width() - 1.0) / 2.0;
                    let pp = p.with_clip_rect(pr);
                    theme_preview(
                        &pp,
                        Rect::from_min_size(pr.min, Vec2::new(half, 72.0)),
                        &self.system.0,
                        false,
                    );
                    theme_preview(
                        &pp,
                        Rect::from_min_size(
                            Pos2::new(pr.left() + half + 1.0, pr.top()),
                            Vec2::new(half, 72.0),
                        ),
                        &self.system.1,
                        false,
                    );
                    let m = Rect::from_center_size(pr.center(), Vec2::splat(size::ICON));
                    p.rect_filled(m, CornerRadius::same(size::RADIUS_XS), tk.bg);
                    lucide::MONITOR.paint(p, m.shrink(2.0), tk.fg);
                }
            }
            let ly = pr.bottom() + 4.0 + 6.0;
            let room = cw - 8.0 - 4.0 - if on { 16.0 } else { 0.0 };
            let g = ph_trunc(ui, label, font(Face::Sans, size::TEXT_3XS), room);
            draw(p, &g, r.left() + 6.0, ly, tk.fg_muted);
            if on {
                lucide::CHECK.paint(
                    p,
                    Rect::from_min_size(
                        Pos2::new(r.right() - 6.0 - 12.0, ly - 6.0),
                        Vec2::splat(12.0),
                    ),
                    tk.accent,
                );
            }
            let b = b.on_hover_cursor(egui::CursorIcon::PointingHand);
            let b = if tip.is_empty() {
                b
            } else {
                b.tip_kbd(tip, "", Side::Top)
            };
            if b.clicked() {
                out = Some(pick);
            }
        }
        out
    }
}

/// `ThemePreview` as a widget: the width on offer, 72px tall.
#[must_use = "add it with ui.add(…)"]
pub struct ThemePreview<'a> {
    tokens: &'a Tokens,
}

impl<'a> ThemePreview<'a> {
    pub fn new(tokens: &'a Tokens) -> Self {
        Self { tokens }
    }
}

impl egui::Widget for ThemePreview<'_> {
    fn ui(self, ui: &mut Ui) -> egui::Response {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 72.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            theme_preview(ui.painter(), rect, self.tokens, true);
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_presets_first() {
        let all = ordered();
        assert_eq!(all.len(), PRESETS.len());
        let first_light = all.iter().position(|(p, _)| p.scheme == Scheme::Light);
        if let Some(i) = first_light {
            assert!(all.iter().skip(i).all(|(p, _)| p.scheme == Scheme::Light));
        }
    }
}
