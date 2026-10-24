//! Who this is: the launcher's mark and name over the contour texture, its version and
//! licence, the feature pills (from `xl` up), and the project's links under a rule.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, CornerRadius, Galley, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use serde_json::Value;

use super::{AUTHOR, Run, VERSION};
use crate::i18n::t;
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::topo;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// A pill's tone, read from the tokens worn.
type ToneOf = fn(&Tokens) -> Color32;

/// The feature pills: icon, words key, tone.
const FEATURES: [(Icon, &str, ToneOf); 7] = [
    (lucide::SERVER, "featBrowser", |t| t.info),
    (lucide::PUZZLE, "featMods", |t| t.mods),
    (lucide::CHART_LINE, "featStats", |t| t.ok),
    (lucide::ROCKET, "featLaunch", |t| t.accent),
    (lucide::WIFI_OFF, "featOffline", |t| t.warn),
    (lucide::SHARE_2, "featShare", |t| t.info),
    (lucide::GAMEPAD_2, "featController", |t| t.fg_muted),
];

/// The links under the hero: icon, words key, command.
const LINKS: [(Icon, &str, &str); 5] = [
    (lucide::GIT_BRANCH, "sourceRepo", "about.open_repo"),
    (lucide::TAG, "linkReleases", "about.open_releases"),
    (lucide::BUG, "linkBug", "about.report_bug"),
    (lucide::BOOK_OPEN, "linkWiki", "about.open_wiki"),
    (lucide::SCALE, "linkLicense", "about.open_license"),
];

/// The pills are shown from this window width (`max-xl:hidden`).
const XL: f32 = 1280.0;
const LOGO: f32 = 64.0;
const PILLS_MAX: f32 = 448.0;
const PILL_GAP: f32 = 6.0;
/// The meta line's box (the heart's inline-flex row).
const META_H: f32 = 16.5;
/// Where the title's galley sits in its `leading-none` box.
const TITLE_DY: f32 = -5.5;

struct Pill {
    icon: Icon,
    tone: Color32,
    text: Arc<Galley>,
    w: f32,
}

pub fn show(ui: &mut Ui, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let wide = ui.ctx().content_rect().width() >= XL;

    // ── measure ──
    let title = title_galley(ui, &tk);
    let meta_font = font(Face::Sans, size::TEXT_2XS);
    let meta: Vec<Arc<Galley>> = [
        format!("v{VERSION}"),
        "·".to_owned(),
        t("about", "licenseMit").to_owned(),
        "·".to_owned(),
        t("about", "madeWith").to_owned(),
        format!("{} {AUTHOR}", t("about", "by")),
    ]
    .iter()
    .map(|s| paint::galley(ui, s, meta_font.clone(), tk.fg_faint))
    .collect();

    let pills: Vec<Pill> = if wide {
        FEATURES
            .iter()
            .map(|(icon, key, tone)| {
                let text = paint::galley(
                    ui,
                    t("about", key),
                    font(Face::Sans, size::TEXT_2XS),
                    tk.fg_muted,
                );
                let w = 8.0 + 14.0 + 6.0 + text.size().x + 8.0;
                Pill {
                    icon: *icon,
                    tone: tone(&tk),
                    text,
                    w,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    let pill_h = 24.0;
    let pills_room = PILLS_MAX.min((w - 48.0 - LOGO - 20.0 - 260.0).max(0.0));
    let rows = pill_rows(&pills, pills_room);
    let pills_w = rows
        .iter()
        .map(|r| row_width(&pills, r))
        .fold(0.0_f32, f32::max);
    let pills_h = if rows.is_empty() {
        0.0
    } else {
        rows.len() as f32 * pill_h + (rows.len() as f32 - 1.0) * PILL_GAP
    };

    let gap = if pills_w > 0.0 { 20.0 } else { 0.0 };
    let tagline = paint::galley_wrap(
        ui,
        t("about", "heroTagline"),
        font(Face::Sans, size::TEXT_SM),
        tk.fg_muted,
        (w - 48.0 - LOGO - 20.0 - pills_w - gap).max(80.0),
    );
    // The CSS line boxes: the title's (`leading-none`: 1), the tagline's (1.5), the meta's.
    let title_h = size::TEXT_3XL;
    let tag_lh = size::TEXT_SM * 1.5;
    let tag_rows = tagline.rows.len().max(1) as f32;
    let tag_h = tag_lh * tag_rows;
    let text_h = title_h + 6.0 + tag_h + 4.0 + META_H;
    let body_h = text_h.max(LOGO).max(pills_h) + 40.0;
    let nav_h = size::CONTROL + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, body_h + nav_h), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    // ── ground ──
    let p = ui.painter();
    let radius = CornerRadius::same(size::RADIUS_MD);
    p.rect_filled(rect, radius, tk.bg);
    topo::paint(p, rect.shrink(1.0), &tk, 0.55);
    let nav = Rect::from_min_max(pos2(rect.left(), rect.bottom() - nav_h), rect.max);
    p.rect_filled(
        nav.shrink(1.0),
        CornerRadius {
            nw: 0,
            ne: 0,
            sw: size::RADIUS_MD - 1,
            se: size::RADIUS_MD - 1,
        },
        alpha(tk.panel, 0.6),
    );
    p.hline(
        rect.x_range(),
        nav.top() + 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.7)),
    );
    p.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, tk.border),
        StrokeKind::Inside,
    );

    // ── who ──
    let cy = rect.top() + body_h / 2.0;
    let logo = Rect::from_min_size(pos2(rect.left() + 24.0, cy - LOGO / 2.0), Vec2::splat(LOGO));
    crate::shell::logo(ui, logo);
    let x = logo.right() + 20.0;
    let mut y = cy - text_h / 2.0;
    p.galley(pos2(x, y + TITLE_DY), title, tk.fg);
    y += title_h + 6.0;
    p.galley(
        pos2(x, y + (tag_h - tagline.size().y) / 2.0),
        tagline.clone(),
        tk.fg_muted,
    );
    y += tag_h + 4.0;
    let mcy = y + META_H / 2.0;
    let mut mx = x;
    for (i, g) in meta.iter().enumerate() {
        paint::text_at(p, g, mx, mcy, tk.fg_faint);
        mx += g.size().x;
        if i == 4 {
            // "Made with ♥ by …": a heart between the words.
            let r = Rect::from_center_size(pos2(mx + 4.0 + 6.0, mcy), Vec2::splat(12.0));
            lucide::HEART.paint(p, r, tk.err);
            mx += 4.0 + 12.0 + 4.0;
        } else {
            mx += 8.0;
        }
    }

    // ── what it does ──
    let right = rect.right() - 24.0;
    let mut py = cy - pills_h / 2.0;
    for row in &rows {
        let mut px = right - row_width(&pills, row);
        for i in row.clone() {
            let Some(pill) = pills.get(i) else { continue };
            let r = Rect::from_min_size(pos2(px, py), vec2(pill.w, pill_h));
            paint::plate(p, r, size::RADIUS_SM, alpha(tk.panel, 0.7), tk.border);
            let ir =
                Rect::from_center_size(pos2(r.left() + 8.0 + 7.0, r.center().y), Vec2::splat(14.0));
            pill.icon.paint(p, ir, pill.tone);
            paint::text_at(p, &pill.text, ir.right() + 6.0, r.center().y, tk.fg_muted);
            px += pill.w + PILL_GAP;
        }
        py += pill_h + PILL_GAP;
    }

    // ── where it lives ──
    let mut lx = nav.left() + 16.0;
    let ly = nav.top() + 6.0;
    for (icon, key, cmd) in LINKS {
        let label = t("about", key);
        let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_XS), tk.fg_muted);
        let bw = 8.0 + 14.0 + 6.0 + g.size().x + 8.0;
        let r = Rect::from_min_size(pos2(lx, ly), vec2(bw, size::CONTROL));
        let resp = ui
            .interact(r, ui.scope_id().with(("about.link", key)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        let hover = paint::ease(ui, resp.id.with("h"), resp.hovered());
        if hover > 0.0 {
            p.rect_filled(
                r,
                CornerRadius::same(size::RADIUS_SM),
                alpha(tk.raised, hover),
            );
        }
        let ink = paint::lerp(tk.fg_muted, tk.fg, hover);
        let ir =
            Rect::from_center_size(pos2(r.left() + 8.0 + 7.0, r.center().y), Vec2::splat(14.0));
        icon.paint(p, ir, ink);
        paint::text_at(p, &g, ir.right() + 6.0, r.center().y, ink);
        if paint::keyboard_focus(ui, &resp) {
            paint::focus_ring(p, r, size::RADIUS_SM, &tk);
        }
        if resp.clicked() {
            run.push((cmd, Value::Null));
        }
        lx += bw + 4.0;
    }
}

/// "DAYZ COMMUNITY HUB", "Community Hub" in the accent (`title-display text-3xl`).
fn title_galley(ui: &Ui, tk: &Tokens) -> Arc<Galley> {
    let f = font(Face::DisplayBold, size::TEXT_3XL);
    let spacing = f.size * 0.02;
    let mut job = LayoutJob::default();
    for (text, color) in [("DAYZ ", tk.fg), ("COMMUNITY HUB", tk.accent)] {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: f.clone(),
                color,
                extra_letter_spacing: spacing,
                line_height: Some(f.size * 0.95),
                ..Default::default()
            },
        );
    }
    paint::layout_top(ui, job)
}

/// The pills in rows no wider than `room` (each row keeps at least one pill).
fn pill_rows(pills: &[Pill], room: f32) -> Vec<std::ops::Range<usize>> {
    let mut rows = Vec::new();
    if room <= 0.0 {
        return rows;
    }
    let mut start = 0;
    let mut w = 0.0;
    for (i, p) in pills.iter().enumerate() {
        let add = if i == start { p.w } else { PILL_GAP + p.w };
        if i > start && w + add > room {
            rows.push(start..i);
            start = i;
            w = p.w;
        } else {
            w += add;
        }
    }
    if start < pills.len() {
        rows.push(start..pills.len());
    }
    rows
}

fn row_width(pills: &[Pill], row: &std::ops::Range<usize>) -> f32 {
    let n = row.len() as f32;
    pills
        .get(row.clone())
        .map_or(0.0, |r| r.iter().map(|p| p.w).sum::<f32>())
        + (n - 1.0).max(0.0) * PILL_GAP
}
