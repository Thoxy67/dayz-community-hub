//! Shortcuts shown: `PadGlyph` (a controller button as the pad in hand prints it),
//! `ShortcutList` (every shortcut by where it works, keyboard or controller) and
//! `PadLegend` (the controller's, without the keyboard tab).

use std::sync::Arc;

use egui::{Color32, CornerRadius, Galley, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

pub use dz_hub::features::gamepad::PadKind;

use super::text::{draw, ph, ph_trunc};
use crate::i18n::t;
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{FieldSize, Kbd, Segment, Segmented};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// A controller button, by what Xbox prints on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PadButton {
    A,
    B,
    X,
    Y,
    Lb,
    Rb,
    Lt,
    Rt,
    Menu,
    View,
    /// The left stick pressed in.
    L3,
    /// The right stick pressed in.
    R3,
    Dpad,
    /// The right stick pushed up or down.
    Rs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FaceSet {
    Letters,
    Shapes,
    Nintendo,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShoulderSet {
    Xbox,
    Sony,
    Nintendo,
}

fn face_set(kind: PadKind) -> FaceSet {
    match kind {
        PadKind::PlayStation => FaceSet::Shapes,
        PadKind::Nintendo => FaceSet::Nintendo,
        _ => FaceSet::Letters,
    }
}

fn shoulder_set(kind: PadKind) -> ShoulderSet {
    match kind {
        PadKind::PlayStation | PadKind::SteamDeck => ShoulderSet::Sony,
        PadKind::Nintendo => ShoulderSet::Nintendo,
        _ => ShoulderSet::Xbox,
    }
}

/// What the pad in hand prints on `b` (a face letter or shape, a shoulder's name, a
/// stick's), or `None` for the buttons drawn as icons.
pub fn pad_text(b: PadButton, kind: PadKind) -> Option<&'static str> {
    let face = face_set(kind);
    let sh = shoulder_set(kind);
    let pick = |x: &'static str, s: &'static str, n: &'static str| match sh {
        ShoulderSet::Xbox => x,
        ShoulderSet::Sony => s,
        ShoulderSet::Nintendo => n,
    };
    let f = |l: &'static str, s: &'static str, n: &'static str| match face {
        FaceSet::Letters => l,
        FaceSet::Shapes => s,
        FaceSet::Nintendo => n,
    };
    Some(match b {
        PadButton::A => f("A", "✕", "B"),
        PadButton::B => f("B", "○", "A"),
        PadButton::X => f("X", "□", "Y"),
        PadButton::Y => f("Y", "△", "X"),
        PadButton::Lb => pick("LB", "L1", "L"),
        PadButton::Rb => pick("RB", "R1", "R"),
        PadButton::Lt => pick("LT", "L2", "ZL"),
        PadButton::Rt => pick("RT", "R2", "ZR"),
        PadButton::L3 => pick("LS", "L3", "LS"),
        PadButton::R3 => pick("RS", "R3", "RS"),
        PadButton::Rs => "R",
        PadButton::Menu if kind == PadKind::Nintendo => "+",
        PadButton::View if kind == PadKind::Nintendo => "−",
        PadButton::Menu | PadButton::View | PadButton::Dpad => return None,
    })
}

/// A face button's colour: Xbox's by letter, PlayStation's by shape, Nintendo's plain.
fn face_tone(b: PadButton, kind: PadKind, t: &Tokens) -> Color32 {
    match (face_set(kind), b) {
        (FaceSet::Nintendo, _) => t.fg,
        (FaceSet::Letters, PadButton::A) => t.ok,
        (FaceSet::Shapes, PadButton::A) => t.info,
        (_, PadButton::B) => t.err,
        (FaceSet::Letters, PadButton::X) => t.info,
        (FaceSet::Shapes, PadButton::X) => t.mods,
        (FaceSet::Letters, PadButton::Y) => t.warn,
        (FaceSet::Shapes, PadButton::Y) => t.ok,
        _ => t.fg_muted,
    }
}

/// A capsule's outline as points, for a dashed border.
fn capsule(r: Rect) -> Vec<Pos2> {
    let rad = r.height() / 2.0;
    let mut pts = Vec::with_capacity(40);
    let (lc, rc) = (
        Pos2::new(r.left() + rad, r.center().y),
        Pos2::new(r.right() - rad, r.center().y),
    );
    for i in 0..=16 {
        let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
        pts.push(rc + Vec2::angled(a) * rad);
    }
    for i in 0..=16 {
        let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * i as f32 / 16.0;
        pts.push(lc + Vec2::angled(a) * rad);
    }
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    pts
}

/// `PadGlyph`: a controller button as the pad in hand prints it (`size-4.5`): Xbox letters
/// by default, PlayStation shapes on a Sony pad, Nintendo's swapped letters on a Switch.
#[must_use = "add it with ui.add(…)"]
pub struct PadGlyph {
    button: PadButton,
    kind: PadKind,
}

impl PadGlyph {
    pub fn new(button: PadButton, kind: PadKind) -> Self {
        Self { button, kind }
    }
}

const GLYPH: f32 = 18.0;

impl Widget for PadGlyph {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let b = self.button;
        let text = pad_text(b, self.kind);
        let g = text.map(|s| ph(ui, s, font(Face::MonoSemibold, size::TEXT_3XS)));
        let round_face = matches!(b, PadButton::A | PadButton::B | PadButton::X | PadButton::Y)
            || matches!(b, PadButton::Menu | PadButton::View);
        let stick = matches!(b, PadButton::L3 | PadButton::R3 | PadButton::Rs);
        let w = if round_face || b == PadButton::Dpad {
            GLYPH
        } else if stick {
            let extra = if b == PadButton::Rs { 10.0 } else { 0.0 };
            (g.as_ref().map_or(0.0, |g| g.size().x) + extra + 10.0).max(GLYPH)
        } else {
            (g.as_ref().map_or(0.0, |g| g.size().x) + 10.0).max(24.0)
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, GLYPH), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let p = ui.painter();
        let ink = if matches!(b, PadButton::A | PadButton::B | PadButton::X | PadButton::Y) {
            face_tone(b, self.kind, &tk)
        } else {
            tk.fg_muted
        };
        if round_face {
            p.circle_filled(rect.center(), GLYPH / 2.0, tk.raised);
            p.circle_stroke(
                rect.center(),
                GLYPH / 2.0 - 0.5,
                Stroke::new(1.0, tk.border_strong),
            );
        } else if stick {
            p.rect_filled(rect, CornerRadius::same(9), tk.raised);
            p.add(egui::Shape::dashed_line(
                &capsule(rect.shrink(0.5)),
                Stroke::new(1.0, tk.border_strong),
                2.0,
                2.0,
            ));
        } else {
            paint::plate(p, rect, size::RADIUS_SM, tk.raised, tk.border_strong);
        }
        let icon = |i: Icon, px: f32| {
            i.paint(
                p,
                Rect::from_center_size(rect.center(), Vec2::splat(px)),
                tk.fg_muted,
            );
        };
        match (b, &g) {
            (PadButton::Dpad, _) => icon(lucide::MOVE, 12.0),
            (PadButton::Menu, None) => icon(lucide::MENU, 10.0),
            (PadButton::View, None) => icon(lucide::COPY, 10.0),
            (PadButton::Rs, Some(g)) => {
                let x = rect.center().x - (g.size().x + 10.0) / 2.0;
                draw(p, g, x, rect.center().y, ink);
                lucide::MOVE_VERTICAL.paint(
                    p,
                    Rect::from_min_size(
                        Pos2::new(x + g.size().x, rect.center().y - 5.0),
                        Vec2::splat(10.0),
                    ),
                    ink,
                );
            }
            // PlayStation's shapes are drawn: the faces lack ✕ and △.
            (PadButton::A | PadButton::B | PadButton::X | PadButton::Y, _)
                if face_set(self.kind) == FaceSet::Shapes =>
            {
                let c = rect.center();
                let st = Stroke::new(1.3, ink);
                match b {
                    PadButton::A => {
                        let d = Vec2::splat(3.2);
                        p.line_segment([c - d, c + d], st);
                        p.line_segment([c + Vec2::new(-3.2, 3.2), c + Vec2::new(3.2, -3.2)], st);
                    }
                    PadButton::B => {
                        p.circle_stroke(c, 3.6, st);
                    }
                    PadButton::X => {
                        p.rect_stroke(
                            Rect::from_center_size(c, Vec2::splat(6.6)),
                            CornerRadius::ZERO,
                            st,
                            egui::StrokeKind::Middle,
                        );
                    }
                    _ => {
                        let pts = vec![
                            c + Vec2::new(0.0, -3.8),
                            c + Vec2::new(3.6, 2.6),
                            c + Vec2::new(-3.6, 2.6),
                        ];
                        p.add(egui::Shape::closed_line(pts, st));
                    }
                }
            }
            (_, Some(g)) => {
                draw(
                    p,
                    g,
                    rect.center().x - g.size().x / 2.0,
                    rect.center().y,
                    ink,
                );
            }
            _ => {}
        }
        resp
    }
}

// ---------------------------------------------------------------------------------------

/// A chip (a key's text), or a separator laid out, and its width.
type ComboPart<'a> = (Option<&'a str>, Option<Arc<Galley>>, f32);

/// Which shortcuts a [`ShortcutList`] shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ShortcutMode {
    #[default]
    Keys,
    Pad,
}

/// One shortcut: what it does, its key combinations ("Ctrl+K"), its pad buttons.
#[derive(Clone, Copy, Debug)]
pub struct ShortcutRow<'a> {
    pub label: &'a str,
    pub keys: &'a [&'a str],
    pub pad: &'a [PadButton],
    /// Its keys are a pair ("↑ / ↓"): a slash between them rather than "or".
    pub pair: bool,
}

/// Shortcuts by where they work, under a title.
#[derive(Clone, Copy, Debug)]
pub struct ShortcutGroup<'a> {
    pub title: &'a str,
    /// The views' keys: two columns of chips rather than long rows.
    pub views: bool,
    pub rows: &'a [ShortcutRow<'a>],
}

/// A combination split into the chips the lists draw: "Ctrl+K" is "Ctrl", "K".
pub fn chips(combo: &str) -> impl Iterator<Item = &str> {
    let whole = combo == "+";
    let mut it = combo.split('+');
    let mut once = whole.then_some(combo);
    std::iter::from_fn(move || if whole { once.take() } else { it.next() })
}

/// `ShortcutList`: every shortcut by where it works, the keyboard's or the controller's
/// in the glyphs of the pad in hand, with a tab between the two (`.tabs(false)` hides it).
pub struct ShortcutList<'a> {
    keys: &'a [ShortcutGroup<'a>],
    pad: &'a [ShortcutGroup<'a>],
    mode: &'a mut ShortcutMode,
    tabs: bool,
    kind: PadKind,
    columns: usize,
}

impl<'a> ShortcutList<'a> {
    /// `keys` are the keyboard's groups, `pad` the controller's (each in its own order).
    pub fn new(
        keys: &'a [ShortcutGroup<'a>],
        pad: &'a [ShortcutGroup<'a>],
        mode: &'a mut ShortcutMode,
    ) -> Self {
        Self {
            keys,
            pad,
            mode,
            tabs: true,
            kind: PadKind::Xbox,
            columns: 1,
        }
    }

    /// Lays the groups out in this many columns (the `?` sheet's two; one below a
    /// 640-point window).
    pub fn columns(mut self, n: usize) -> Self {
        self.columns = n.clamp(1, 2);
        self
    }

    pub fn tabs(mut self, on: bool) -> Self {
        self.tabs = on;
        self
    }

    /// The pad in hand, for its glyphs.
    pub fn pad_kind(mut self, k: PadKind) -> Self {
        self.kind = k;
        self
    }

    pub fn show(self, ui: &mut Ui) {
        let tk = ui.tokens();
        ui.spacing_mut().item_spacing.y = 12.0;
        if self.tabs {
            let opts = [
                Segment::new(ShortcutMode::Keys, t("keys", "tabKeys")).icon(lucide::KEYBOARD),
                Segment::new(ShortcutMode::Pad, t("keys", "tabPad")).icon(lucide::GAMEPAD_2),
            ];
            ui.add(Segmented::new(self.mode, &opts).size(FieldSize::Xs).fill());
        }
        let keys_mode = *self.mode == ShortcutMode::Keys;
        let groups = if keys_mode { self.keys } else { self.pad };
        let has = |r: &ShortcutRow<'_>| {
            if keys_mode {
                !r.keys.is_empty()
            } else {
                !r.pad.is_empty()
            }
        };
        let shown: Vec<(&ShortcutGroup<'_>, Vec<&ShortcutRow<'_>>)> = groups
            .iter()
            .map(|g| (g, g.rows.iter().filter(|r| has(r)).collect::<Vec<_>>()))
            .filter(|(_, rows)| !rows.is_empty())
            .collect();
        // `columns-2 max-[640px]:columns-1`: groups flow down the first column, then the
        // second, never split, the columns as even as the groups allow.
        let two = self.columns > 1 && ui.ctx().content_rect().width() > 640.0;
        if two {
            let heights: Vec<f32> = shown
                .iter()
                .map(|(g, rows)| group_height(g, rows.len(), keys_mode))
                .collect();
            let split = balance(&heights, ui.spacing().item_spacing.y);
            let gap = 24.0;
            let w = ui.available_width();
            let cw = ((w - gap) / 2.0).max(0.0);
            let top = ui.cursor().min;
            let mut bottom = top.y;
            for (col, part) in [&shown[..split], &shown[split..]].into_iter().enumerate() {
                let rect = Rect::from_min_size(
                    Pos2::new(top.x + col as f32 * (cw + gap), top.y),
                    Vec2::new(cw, f32::INFINITY),
                );
                let mut c = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                c.spacing_mut().item_spacing.y = 12.0;
                for (g, rows) in part {
                    self.group(&mut c, g, rows, keys_mode, &tk);
                }
                bottom = bottom.max(c.min_rect().bottom());
            }
            ui.allocate_exact_size(Vec2::new(w, (bottom - top.y).max(0.0)), Sense::hover());
        } else {
            for (g, rows) in &shown {
                self.group(ui, g, rows, keys_mode, &tk);
            }
        }
        let note = if keys_mode {
            format!("{} {}", t("keys", "typingNote"), t("keys", "rightClick"))
        } else {
            t("keys", "padNote").to_owned()
        };
        let g = paint::galley_wrap(
            ui,
            &note,
            font(Face::Sans, size::TEXT_2XS),
            Color32::PLACEHOLDER,
            ui.available_width(),
        );
        let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
        ui.painter().galley(r.min, g, tk.fg_faint);
    }

    /// One group: its title, then its rows.
    fn group(
        &self,
        ui: &mut Ui,
        g: &ShortcutGroup<'_>,
        rows: &[&ShortcutRow<'_>],
        keys_mode: bool,
        tk: &Tokens,
    ) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let tg = paint::tracked(
                ui,
                g.title,
                font(Face::Mono, size::TEXT_3XS),
                Color32::PLACEHOLDER,
                0.08,
                true,
                None,
            );
            let (r, _) = ui.allocate_exact_size(Vec2::new(tg.size().x, 12.0), Sense::hover());
            draw(ui.painter(), &tg, r.left(), r.center().y, tk.fg_faint);
            ui.add_space(4.0);
            if g.views && keys_mode {
                let w = ui.available_width();
                let cw = (w - 12.0) / 2.0;
                for pair in rows.chunks(2) {
                    let (line, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
                    for (k, r) in pair.iter().enumerate() {
                        let cell = Rect::from_min_size(
                            Pos2::new(line.left() + k as f32 * (cw + 12.0), line.top()),
                            Vec2::new(cw, ROW_H),
                        );
                        self.key_row(ui, cell, r, true, tk);
                    }
                }
            } else {
                let n = rows.len();
                for (i, r) in rows.iter().enumerate() {
                    let w = ui.available_width();
                    let (line, _) = ui.allocate_exact_size(Vec2::new(w, ROW_H), Sense::hover());
                    if keys_mode {
                        self.key_row(ui, line, r, i + 1 < n, tk);
                    } else {
                        self.pad_row(ui, line, r, i + 1 < n, tk);
                    }
                }
            }
        });
    }

    /// Lays out a row's key combos from the right; returns their left edge.
    fn combos(ui: &Ui, right: f32, cy: f32, r: &ShortcutRow<'_>, tk: &Tokens) -> f32 {
        let small = font(Face::Sans, size::TEXT_3XS);
        let sep_text = if r.pair { "/" } else { t("keys", "or") };
        let sep = ph(ui, sep_text, small.clone());
        let plus = ph(ui, "+", small);
        // Measure, then paint left to right.
        let mut parts: Vec<ComboPart<'_>> = Vec::new();
        for (i, c) in r.keys.iter().enumerate() {
            if i > 0 {
                parts.push((None, Some(sep.clone()), sep.size().x));
            }
            for (j, chip) in chips(c).enumerate() {
                if j > 0 {
                    parts.push((None, Some(plus.clone()), plus.size().x));
                }
                parts.push((Some(chip), None, Kbd::width(ui, chip)));
            }
        }
        let n = parts.len();
        let total: f32 = parts.iter().map(|p| p.2).sum::<f32>() + 3.0 * n.saturating_sub(1) as f32;
        let mut x = right - total;
        let left = x;
        for (chip, g, w) in parts {
            match (chip, g) {
                (Some(c), _) => {
                    Kbd::paint_at(ui, c, x, cy);
                }
                (None, Some(g)) => draw(ui.painter(), &g, x, cy, tk.fg_faint),
                _ => {}
            }
            x += w + 3.0;
        }
        left
    }

    fn key_row(&self, ui: &Ui, line: Rect, r: &ShortcutRow<'_>, rule: bool, tk: &Tokens) {
        let cy = line.center().y;
        if rule {
            ui.painter().hline(
                line.x_range(),
                line.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.4)),
            );
        }
        let left = Self::combos(ui, line.right(), cy, r, tk);
        let g = ph_trunc(
            ui,
            r.label,
            font(Face::Sans, size::TEXT_2XS),
            left - 8.0 - line.left(),
        );
        draw(ui.painter(), &g, line.left(), cy, tk.fg_muted);
    }

    fn pad_row(&self, ui: &mut Ui, line: Rect, r: &ShortcutRow<'_>, rule: bool, tk: &Tokens) {
        let cy = line.center().y;
        if rule {
            ui.painter().hline(
                line.x_range(),
                line.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.4)),
            );
        }
        let mut c = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(line)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        c.spacing_mut().item_spacing.x = 4.0;
        for (i, b) in r.pad.iter().enumerate().rev() {
            c.add(PadGlyph::new(*b, self.kind));
            if i > 0 {
                let g = ph(&c, "/", font(Face::Sans, size::TEXT_3XS));
                let (sr, _) = c.allocate_exact_size(g.size(), Sense::hover());
                draw(c.painter(), &g, sr.left(), sr.center().y, tk.fg_faint);
            }
        }
        let left = c.min_rect().left();
        let g = ph_trunc(
            ui,
            r.label,
            font(Face::Sans, size::TEXT_2XS),
            left - 8.0 - line.left(),
        );
        draw(ui.painter(), &g, line.left(), cy, tk.fg_muted);
    }
}

/// A shortcut row's height.
const ROW_H: f32 = 25.0;

/// How tall a group draws: its title, then its rows (the views' keys two by two).
fn group_height(g: &ShortcutGroup<'_>, rows: usize, keys_mode: bool) -> f32 {
    let lines = if g.views && keys_mode {
        rows.div_ceil(2)
    } else {
        rows
    };
    12.0 + 4.0 + lines as f32 * ROW_H
}

/// Where the second column starts: the split that keeps the taller column shortest
/// (`gap` between groups).
fn balance(heights: &[f32], gap: f32) -> usize {
    let col = |hs: &[f32]| hs.iter().sum::<f32>() + gap * hs.len().saturating_sub(1) as f32;
    // Ties go to the first column, as a browser fills it first.
    (0..=heights.len())
        .rev()
        .min_by(|a, b| {
            let ha = col(&heights[..*a]).max(col(&heights[*a..]));
            let hb = col(&heights[..*b]).max(col(&heights[*b..]));
            ha.total_cmp(&hb)
        })
        .unwrap_or(0)
}

/// `PadLegend`: what a controller's buttons do, in the glyphs of the one plugged in: the
/// controller's shortcuts without the keyboard tab.
pub fn pad_legend(ui: &mut Ui, pad: &[ShortcutGroup<'_>], kind: PadKind) {
    let mut mode = ShortcutMode::Pad;
    ShortcutList::new(&[], pad, &mut mode)
        .tabs(false)
        .pad_kind(kind)
        .show(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_per_maker() {
        assert_eq!(pad_text(PadButton::A, PadKind::Xbox), Some("A"));
        assert_eq!(pad_text(PadButton::A, PadKind::PlayStation), Some("✕"));
        assert_eq!(pad_text(PadButton::A, PadKind::Nintendo), Some("B"));
        assert_eq!(pad_text(PadButton::Lb, PadKind::SteamDeck), Some("L1"));
        assert_eq!(pad_text(PadButton::Lt, PadKind::Nintendo), Some("ZL"));
        assert_eq!(pad_text(PadButton::Menu, PadKind::Nintendo), Some("+"));
        assert_eq!(pad_text(PadButton::Menu, PadKind::Xbox), None);
        assert_eq!(pad_text(PadButton::R3, PadKind::PlayStation), Some("R3"));
    }

    #[test]
    fn columns_balance() {
        assert_eq!(balance(&[], 12.0), 0);
        assert_eq!(balance(&[100.0], 12.0), 1);
        assert_eq!(balance(&[100.0, 100.0], 12.0), 1);
        assert_eq!(balance(&[300.0, 50.0, 50.0, 50.0, 50.0], 12.0), 1);
        assert_eq!(balance(&[50.0, 50.0, 50.0, 200.0], 12.0), 3);
    }

    #[test]
    fn chips_split() {
        assert_eq!(chips("Ctrl+K").collect::<Vec<_>>(), ["Ctrl", "K"]);
        assert_eq!(chips("+").collect::<Vec<_>>(), ["+"]);
        assert_eq!(chips("F5").collect::<Vec<_>>(), ["F5"]);
    }
}
