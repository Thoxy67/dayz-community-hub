//! Facts about one thing: `Facts` (labelled figures in a tight grid), `DetailList` +
//! `DetailRow` (label beside value), `ModStatusList` (Workshop mods against what is
//! installed), `TransferProgress` (one transfer as it runs) and `CommandPreview` (a
//! command line as it will run).

use std::sync::Arc;

use egui::text::LayoutJob;
use egui::{
    Color32, CornerRadius, Galley, Pos2, Rect, Response, Sense, Stroke, StrokeKind, TextFormat, Ui,
    Vec2, Widget,
};

use super::sections::SectionCard;
use super::text::{clock, draw, ph, ph_trunc};
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Copy, Meter, MeterSize, Side, TipExt, Tone, panel};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// `text-2xs`'s line: 11px at 1.4.
const LINE_2XS: f32 = 15.4;

/// One labelled figure of [`Facts`].
#[derive(Clone, Copy, Debug)]
pub struct Fact<'a> {
    pub label: &'a str,
    pub value: &'a str,
    /// The value's colour (`fg` by default).
    pub tone: Option<Color32>,
    /// The hint over the value.
    pub tip: &'a str,
}

impl<'a> Fact<'a> {
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            tone: None,
            tip: "",
        }
    }

    pub fn tone(mut self, c: Color32) -> Self {
        self.tone = Some(c);
        self
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }
}

/// `Facts`: labelled figures in a tight grid (`gap-x-3 gap-y-1 text-2xs`): the label faint
/// on the left, the value on the right; two pairs per line by default, one when a
/// label would not fit whole.
#[must_use = "add it with ui.add(…)"]
pub struct Facts<'a> {
    items: &'a [Fact<'a>],
    columns: usize,
}

impl<'a> Facts<'a> {
    pub fn new(items: &'a [Fact<'a>]) -> Self {
        Self { items, columns: 2 }
    }

    /// 1 or 2 pairs per line.
    pub fn columns(mut self, n: usize) -> Self {
        self.columns = n.clamp(1, 2);
        self
    }
}

impl Widget for Facts<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let w = ui.available_width();
        let figs = font(Face::Sans, size::TEXT_2XS);
        let sans = font(Face::Sans, size::TEXT_2XS);
        let gap = 12.0;
        let value_full: Vec<f32> = self
            .items
            .iter()
            .map(|f| ph(ui, f.value, figs.clone()).size().x)
            .collect();
        let label_full: Vec<f32> = self
            .items
            .iter()
            .map(|f| ph(ui, f.label, sans.clone()).size().x)
            .collect();
        // The value columns are as wide as their widest value; the labels share the rest.
        let fit = |cols: usize| {
            let mut value_w = [0.0_f32; 2];
            for (i, v) in value_full.iter().enumerate() {
                if let Some(c) = value_w.get_mut(i % cols) {
                    *c = c.max(*v);
                }
            }
            let fixed: f32 = value_w.iter().take(cols).sum();
            let label_w = ((w - fixed - gap * (2 * cols - 1) as f32) / cols as f32).max(16.0);
            (value_w, label_w)
        };
        // Two pairs a line only while every label fits whole: else one pair a line.
        let mut cols = self.columns;
        let (mut value_w, mut label_w) = fit(cols);
        if cols > 1 && label_full.iter().any(|l| *l > label_w + 0.5) {
            cols = 1;
            (value_w, label_w) = fit(1);
        }
        let rows = self.items.len().div_ceil(cols);
        let h = rows as f32 * LINE_2XS + rows.saturating_sub(1) as f32 * 4.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        for (i, f) in self.items.iter().enumerate() {
            let (r, c) = (i / cols, i % cols);
            let vw = value_w.get(c).copied().unwrap_or(0.0);
            let before: f32 = value_w.iter().take(c).sum();
            let x = rect.left() + c as f32 * (label_w + 2.0 * gap) + before;
            let cy = rect.top() + r as f32 * (LINE_2XS + 4.0) + LINE_2XS / 2.0;
            let lg = ph_trunc(ui, f.label, sans.clone(), label_w);
            draw(ui.painter(), &lg, x, cy, tk.fg_faint);
            let vg = ph_trunc(ui, f.value, figs.clone(), vw.max(8.0));
            let vx = x + label_w + gap + vw - vg.size().x;
            draw(ui.painter(), &vg, vx, cy, f.tone.unwrap_or(tk.fg));
            if !f.tip.is_empty() {
                let vr = Rect::from_min_size(
                    Pos2::new(vx, cy - LINE_2XS / 2.0),
                    Vec2::new(vg.size().x, LINE_2XS),
                );
                let _ = ui
                    .interact(vr, resp.id.with(("fact", i)), Sense::hover())
                    .tip_kbd(f.tip, "", Side::Top);
            }
        }
        resp
    }
}

// ---------------------------------------------------------------------------------------

/// One fact of a [`DetailList`]: a plain value, as figures and ids are.
#[derive(Clone, Copy, Debug)]
pub struct DetailRow<'a> {
    pub label: &'a str,
    /// `None` shows a dash.
    pub value: Option<&'a str>,
    pub tone: Option<Color32>,
    /// The hint (the value itself by default).
    pub tip: &'a str,
}

impl<'a> DetailRow<'a> {
    pub fn new(label: &'a str, value: impl Into<Option<&'a str>>) -> Self {
        Self {
            label,
            value: value.into(),
            tone: None,
            tip: "",
        }
    }

    pub fn tone(mut self, c: Color32) -> Self {
        self.tone = Some(c);
        self
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }
}

/// The rows of a [`DetailList`] as they are added.
pub struct DetailRows<'u> {
    ui: &'u mut Ui,
    label_w: f32,
    widest: f32,
    first: bool,
}

impl DetailRows<'_> {
    fn label(&mut self, label: &str) -> (f32, f32) {
        let tk = self.ui.tokens();
        if !self.first {
            self.ui.add_space(8.0);
        }
        self.first = false;
        let g = ph(self.ui, label, font(Face::Sans, size::TEXT_XS));
        self.widest = self.widest.max(g.size().x);
        let top = self.ui.cursor().top();
        let left = self.ui.cursor().left();
        draw(self.ui.painter(), &g, left, top + 8.7, tk.fg_faint);
        (left + self.label_w + 16.0, top)
    }

    /// A plain fact.
    pub fn add(&mut self, row: DetailRow<'_>) -> &mut Self {
        let tk = self.ui.tokens();
        let (x, top) = self.label(row.label);
        let room = (self.ui.max_rect().right() - x).max(8.0);
        let text = row.value.unwrap_or(format::NONE);
        let g = ph_trunc(self.ui, text, font(Face::Sans, size::TEXT_XS), room);
        let r = Rect::from_min_size(Pos2::new(x, top), Vec2::new(room, 17.4));
        draw(
            self.ui.painter(),
            &g,
            x,
            top + 8.7,
            row.tone.unwrap_or(tk.fg),
        );
        let resp = self.ui.allocate_rect(
            Rect::from_min_max(Pos2::new(self.ui.cursor().left(), top), r.max),
            Sense::hover(),
        );
        let tip = if row.tip.is_empty() { text } else { row.tip };
        let _ = resp.tip_kbd(tip, "", Side::Top);
        self
    }

    /// Anything richer beside the label (a copy button, a meter).
    pub fn custom(&mut self, label: &str, value: impl FnOnce(&mut Ui)) -> &mut Self {
        let (x, top) = self.label(label);
        let right = self.ui.max_rect().right();
        let left = self.ui.cursor().left();
        let mut c = self.ui.new_child(
            egui::UiBuilder::new()
                .max_rect(Rect::from_min_max(
                    Pos2::new(x, top),
                    Pos2::new(right, top + 17.4),
                ))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        c.set_min_height(17.4);
        value(&mut c);
        let bottom = c.min_rect().bottom().max(top + 17.4);
        self.ui.allocate_rect(
            Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom)),
            Sense::hover(),
        );
        self
    }
}

/// `DetailList`: facts about one thing, label beside value (`gap-x-4 gap-y-2 text-xs`).
/// `DetailList::new("id").show(ui, |rows| { rows.add(DetailRow::new("Map", "enoch")); })`.
pub struct DetailList {
    id: egui::Id,
}

impl DetailList {
    pub fn new(id_salt: impl egui::AsIdSalt) -> Self {
        Self {
            id: egui::Id::unique(id_salt),
        }
    }

    pub fn show(self, ui: &mut Ui, rows: impl FnOnce(&mut DetailRows<'_>)) {
        // The label column is as wide as the widest label: last frame's.
        let key = self.id.with("label-w");
        let label_w: f32 = ui.data(|d| d.get_temp(key)).unwrap_or(0.0);
        let widest = ui
            .vertical(|ui| {
                let mut r = DetailRows {
                    ui,
                    label_w,
                    widest: 0.0,
                    first: true,
                };
                rows(&mut r);
                r.widest
            })
            .inner;
        if (widest - label_w).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(key, widest));
            ui.ctx().request_discard("detail list label width");
        }
    }
}

// ---------------------------------------------------------------------------------------

/// A Workshop mod's standing here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModState {
    Missing,
    /// On disk, behind the Workshop.
    Stale,
    Installed,
}

/// One mod of a [`ModStatusList`], built once when the data changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ModRef {
    pub id: u64,
    pub name: String,
    pub state: ModState,
    /// Bytes on disk (0 when not installed or not known).
    pub size: u64,
    id_text: String,
    size_text: String,
}

impl ModRef {
    /// A mod; an empty name reads "Workshop {id}".
    pub fn new(id: u64, name: &str, state: ModState, size: u64) -> Self {
        let name = if name.is_empty() {
            format!("Workshop {id}")
        } else {
            name.to_owned()
        };
        Self {
            id,
            name,
            state,
            size,
            id_text: id.to_string(),
            size_text: if size > 0 {
                format::bytes(size as f64)
            } else {
                String::new()
            },
        }
    }

    /// Missing first, then stale, then the rest, each by name: the list's order.
    pub fn sort(mods: &mut [ModRef]) {
        mods.sort_by(|a, b| a.state.cmp(&b.state).then_with(|| a.name.cmp(&b.name)));
    }

    /// Its Workshop page.
    pub fn workshop_url(&self) -> String {
        format!(
            "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
            self.id
        )
    }
}

/// `ModStatusList`: Workshop mods against what is installed: each says whether it is on
/// disk, missing or behind, with its size, its id to copy and its Workshop page, under a
/// summary line. Pass them sorted ([`ModRef::sort`]). Answers the mod whose page to open.
pub struct ModStatusList<'a> {
    items: &'a [ModRef],
    summary: bool,
}

impl<'a> ModStatusList<'a> {
    pub fn new(items: &'a [ModRef]) -> Self {
        Self {
            items,
            summary: true,
        }
    }

    pub fn summary(mut self, on: bool) -> Self {
        self.summary = on;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Option<u64> {
        let tk = ui.tokens();
        let mut open = None;
        ui.spacing_mut().item_spacing.y = 6.0;
        if self.summary {
            let (mut installed, mut missing, mut stale, mut bytes) = (0usize, 0usize, 0usize, 0u64);
            for m in self.items {
                match m.state {
                    ModState::Missing => missing += 1,
                    ModState::Stale => {
                        stale += 1;
                        installed += 1;
                    }
                    ModState::Installed => installed += 1,
                }
                bytes = bytes.saturating_add(m.size);
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(12.0, 2.0);
                let s = tf(
                    "detail",
                    "modsSummary",
                    &[
                        ("installed", &installed),
                        ("missing", &missing),
                        ("stale", &stale),
                    ],
                );
                text(ui, &s, font(Face::Sans, size::TEXT_2XS), tk.fg_muted);
                if bytes > 0 {
                    let s = tf(
                        "detail",
                        "modsLocalSize",
                        &[("size", &format::bytes(bytes as f64))],
                    );
                    text(ui, &s, font(Face::Sans, size::TEXT_2XS), tk.fg_faint);
                }
            });
        }
        let w = ui.available_width();
        let slot = ui.painter().add(egui::Shape::Noop);
        let start = ui.cursor().min;
        let row_h = 28.0;
        let n = self.items.len();
        ui.spacing_mut().item_spacing.y = 0.0;
        let out = egui::ScrollArea::vertical()
            .id_salt("mod-status")
            .max_height(288.0)
            .auto_shrink([false, true])
            .show_rows(ui, row_h, n, |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for i in range {
                    let Some(m) = self.items.get(i) else {
                        continue;
                    };
                    if mod_row(ui, m, i + 1 == n, w) {
                        open = Some(m.id);
                    }
                }
            });
        let rect = Rect::from_min_size(
            start,
            Vec2::new(w, out.inner_rect.height().min(n as f32 * row_h)),
        );
        ui.painter().set(
            slot,
            egui::Shape::rect_filled(rect, CornerRadius::same(size::RADIUS_SM), tk.bg),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(size::RADIUS_SM),
            Stroke::new(1.0, tk.border),
            StrokeKind::Inside,
        );
        open
    }
}

fn text(ui: &mut Ui, s: &str, f: egui::FontId, c: Color32) {
    let g = ph(ui, s, f);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    draw(ui.painter(), &g, r.left(), r.center().y, c);
}

/// One row of the mods list; true when its Workshop button was clicked.
fn mod_row(ui: &mut Ui, m: &ModRef, last: bool, w: f32) -> bool {
    let tk = ui.tokens();
    let (rect, row) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return false;
    }
    let cy = rect.center().y;
    if !last {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.5)),
        );
    }
    let (word, c) = match m.state {
        ModState::Missing => (t("detail", "modMissing"), tk.err),
        ModState::Stale => (t("detail", "modStale"), tk.warn),
        ModState::Installed => (t("detail", "modInstalled"), tk.ok),
    };
    let pill = Rect::from_min_size(
        Pos2::new(rect.left() + 8.0, cy - 6.5),
        Vec2::new(76.0, 13.0),
    );
    ui.painter()
        .rect_filled(pill, CornerRadius::same(size::RADIUS_XS), alpha(c, 0.12));
    let pg = paint::tracked(
        ui,
        word,
        font(Face::Sans, size::TEXT_3XS),
        Color32::PLACEHOLDER,
        0.0,
        true,
        Some(68.0),
    );
    ui.painter().galley(pill.center() - pg.size() / 2.0, pg, c);
    // From the right: the Workshop button, the copy, the download mark, the size.
    let mut right = rect.right() - 8.0;
    let wr = Rect::from_min_size(Pos2::new(right - 20.0, cy - 10.0), Vec2::splat(20.0));
    let wresp = ui.interact(wr, row.id.with("ws"), Sense::click());
    let h = paint::ease(ui, wresp.id.with("hover"), wresp.hovered());
    if h > 0.0 {
        ui.painter()
            .rect_filled(wr, CornerRadius::same(size::RADIUS_XS), alpha(tk.raised, h));
    }
    lucide::EXTERNAL_LINK.paint(
        ui.painter(),
        Rect::from_center_size(wr.center(), Vec2::splat(12.0)),
        paint::lerp(tk.fg_faint, tk.accent, h),
    );
    let clicked = wresp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .tip_kbd(t("detail", "openWorkshop"), "", Side::Top)
        .clicked();
    right = wr.left() - 8.0;
    let cg = ph(ui, &m.id_text, font(Face::Sans, size::TEXT_3XS));
    let cw = cg.size().x + 16.0;
    let mut cui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                Pos2::new(right - cw, rect.top()),
                Pos2::new(right, rect.bottom()),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if !row.contains_pointer() {
        cui.multiply_opacity(0.6);
    }
    cui.add(
        Copy::new(&m.id_text)
            .labels(t("detail", "copyId"), t("common", "copied"))
            .text_size(size::TEXT_3XS),
    );
    right -= cw + 8.0;
    if m.state == ModState::Missing {
        lucide::DOWNLOAD.paint(
            ui.painter(),
            Rect::from_min_size(Pos2::new(right - 12.0, cy - 6.0), Vec2::splat(12.0)),
            alpha(tk.err, 0.7),
        );
        right -= 12.0 + 8.0;
    }
    if !m.size_text.is_empty() {
        let sg = ph(ui, &m.size_text, font(Face::Sans, size::TEXT_3XS));
        right -= sg.size().x;
        draw(ui.painter(), &sg, right, cy, tk.fg_faint);
        right -= 8.0;
    }
    let nx = pill.right() + 8.0;
    let ng = ph_trunc(ui, &m.name, font(Face::Sans, size::TEXT_2XS), right - nx);
    draw(ui.painter(), &ng, nx, cy, tk.fg);
    if super::text::is_cut(&ng, &m.name) {
        let nr = Rect::from_min_size(Pos2::new(nx, cy - 8.0), Vec2::new(right - nx, 16.0));
        let _ = ui
            .interact(nr, row.id.with("name"), Sense::hover())
            .tip_kbd(&m.name, "", Side::Top);
    }
    clicked
}

// ---------------------------------------------------------------------------------------

/// The captions of a [`TransferProgress`]'s figures, in the current language.
#[derive(Clone, Copy, Debug)]
pub struct TransferLabels<'a> {
    pub size: &'a str,
    pub speed: &'a str,
    pub eta: &'a str,
    pub elapsed: &'a str,
}

/// `TransferProgress`: one transfer as it runs: what it is, how far (percent and bytes),
/// how fast, how long left and so far. A figure not known yet is a dash.
pub struct TransferProgress<'a> {
    pub label: &'a str,
    pub name: &'a str,
    pub percent: Option<f64>,
    pub done: Option<u64>,
    pub total: Option<u64>,
    /// Bytes per second.
    pub speed: Option<f64>,
    /// Milliseconds left.
    pub eta: Option<f64>,
    /// Milliseconds so far.
    pub elapsed: Option<f64>,
    pub labels: TransferLabels<'a>,
}

impl Widget for TransferProgress<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let slot = ui.painter().add(egui::Shape::Noop);
        let r = egui::Frame::new()
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                let w = ui.available_width();
                let (top, _) = ui.allocate_exact_size(Vec2::new(w, 17.4), Sense::hover());
                let lg = panel::stencil(ui, self.label, Color32::PLACEHOLDER, Some(w - 60.0));
                draw(ui.painter(), &lg, top.left(), top.center().y, tk.fg_faint);
                let pct = self
                    .percent
                    .map_or_else(|| format::NONE.to_owned(), |p| format!("{p:.1}%"));
                let pg = ph(ui, &pct, font(Face::Sans, size::TEXT_XS));
                draw(
                    ui.painter(),
                    &pg,
                    top.right() - pg.size().x,
                    top.center().y,
                    tk.fg,
                );
                if !self.name.is_empty() {
                    ui.add_space(4.0);
                    let ng = ph_trunc(ui, self.name, font(Face::SansSemibold, size::TEXT_SM), w);
                    let (nr, resp) = ui.allocate_exact_size(Vec2::new(w, 19.5), Sense::hover());
                    draw(ui.painter(), &ng, nr.left(), nr.center().y, tk.fg);
                    if super::text::is_cut(&ng, self.name) {
                        let _ = resp.tip_kbd(self.name, "", Side::Top);
                    }
                }
                ui.add_space(8.0);
                let mut m = Meter::new((self.percent.unwrap_or(0.0) / 100.0) as f32)
                    .tone(Tone::Info)
                    .size(MeterSize::Md);
                if self.percent.is_none() {
                    m = m.indeterminate();
                }
                ui.add(m);
                ui.add_space(8.0);
                let dash = || format::NONE.to_owned();
                let size_v = match (self.done, self.total) {
                    (Some(d), Some(t)) if t > 0 => {
                        format!("{} / {}", format::bytes(d as f64), format::bytes(t as f64))
                    }
                    _ => dash(),
                };
                let speed_v = match self.speed {
                    Some(s) if s > 0.0 => format!("{}/s", format::bytes(s)),
                    _ => dash(),
                };
                let eta_v = self.eta.map_or_else(dash, clock);
                let el_v = self.elapsed.map_or_else(dash, clock);
                let figs = font(Face::Sans, size::TEXT_2XS);
                let pairs = [
                    (self.labels.size, size_v),
                    (self.labels.speed, speed_v),
                    (self.labels.eta, eta_v),
                    (self.labels.elapsed, el_v),
                ];
                let col = (w - 12.0) / 2.0;
                for (i, (l, v)) in pairs.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(4.0);
                    }
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, LINE_2XS), Sense::hover());
                    let lg = ph_trunc(ui, l, figs.clone(), col);
                    draw(ui.painter(), &lg, r.left(), r.center().y, tk.fg_faint);
                    let vg = ph_trunc(ui, v, figs.clone(), col);
                    draw(
                        ui.painter(),
                        &vg,
                        r.right() - vg.size().x,
                        r.center().y,
                        tk.fg,
                    );
                }
            });
        let rect = r.response.rect;
        ui.painter().set(
            slot,
            egui::Shape::rect_filled(rect, CornerRadius::same(size::RADIUS_MD), tk.panel),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(size::RADIUS_MD),
            Stroke::new(1.0, tk.border),
            StrokeKind::Inside,
        );
        r.response
    }
}

// ---------------------------------------------------------------------------------------

/// `CommandPreview`: a command line as it will run, coloured (flag, `=`, value) and
/// wrapped between flags, never inside one, in a card with a copy button (it copies the
/// flags, not the program).
pub struct CommandPreview<'a> {
    title: &'a str,
    program: &'a str,
    flags: &'a [String],
    empty: &'a str,
    hint: &'a str,
    copy_label: &'a str,
}

impl<'a> CommandPreview<'a> {
    pub fn new(title: &'a str, flags: &'a [String], empty: &'a str, copy_label: &'a str) -> Self {
        Self {
            title,
            program: "",
            flags,
            empty,
            hint: "",
            copy_label,
        }
    }

    /// Shown first, dimmed; not part of what is copied.
    pub fn program(mut self, p: &'a str) -> Self {
        self.program = p;
        self
    }

    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let id = ui.next_auto_id().with("cmd-preview");
        let copied_id = id.with("copied");
        let now = ui.input(|i| i.time);
        let copied = ui
            .data(|d| d.get_temp::<f64>(copied_id))
            .is_some_and(|at| now - at < 1.4);
        if copied {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let card = SectionCard::new(self.title).icon(lucide::TERMINAL);
        let flags = self.flags;
        let copy_label = self.copy_label;
        card.show(
            ui,
            |ui| {
                let en = !flags.is_empty();
                let sense = if en { Sense::click() } else { Sense::hover() };
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(size::CONTROL_SM), sense);
                let h = paint::ease(ui, resp.id.with("hover"), en && resp.hovered());
                if h > 0.0 {
                    ui.painter().rect_filled(
                        r,
                        CornerRadius::same(size::RADIUS_SM),
                        alpha(tk.raised, h),
                    );
                }
                let ink = if copied {
                    tk.ok
                } else {
                    paint::fade(paint::lerp(tk.fg_faint, tk.fg, h), en)
                };
                let icon = if copied { lucide::CHECK } else { lucide::COPY };
                icon.paint(
                    ui.painter(),
                    Rect::from_center_size(r.center(), Vec2::splat(size::ICON_SM)),
                    ink,
                );
                let resp = resp.tip_kbd(copy_label, "", Side::Bottom);
                if resp.clicked() {
                    ui.ctx().copy_text(flags.join(" "));
                    ui.data_mut(|d| d.insert_temp(copied_id, now));
                }
            },
            |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let slot = ui.painter().add(egui::Shape::Noop);
                        let start = ui.cursor().min;
                        let w = ui.available_width();
                        let out = egui::ScrollArea::vertical()
                            .id_salt(id.with("scroll"))
                            .max_height(288.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.add_space(6.0);
                                self.flow(ui, w - 16.0);
                                ui.add_space(6.0);
                            });
                        let rect = Rect::from_min_size(
                            start,
                            Vec2::new(w, out.inner_rect.height().min(out.content_size.y)),
                        );
                        ui.painter().set(
                            slot,
                            egui::Shape::rect_filled(
                                rect,
                                CornerRadius::same(size::RADIUS_SM),
                                tk.plot,
                            ),
                        );
                        if !self.hint.is_empty() {
                            ui.add_space(6.0);
                            let g = paint::galley_wrap(
                                ui,
                                self.hint,
                                font(Face::Sans, size::TEXT_3XS),
                                Color32::PLACEHOLDER,
                                w,
                            );
                            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            ui.painter().galley(r.min, g, tk.fg_faint);
                        }
                    });
            },
        );
        ui.interact(Rect::NOTHING, id, Sense::hover())
    }

    /// The program and the flags, flowed: a flag goes to the next line whole.
    fn flow(&self, ui: &mut Ui, max_w: f32) {
        let tk = ui.tokens();
        let line_h = (size::TEXT_2XS * 1.625).round();
        let mut words: Vec<Arc<Galley>> = Vec::with_capacity(self.flags.len() + 2);
        let f = font(Face::Mono, size::TEXT_2XS);
        let fmt = |c: Color32| TextFormat {
            font_id: f.clone(),
            color: c,
            ..Default::default()
        };
        if !self.program.is_empty() {
            words.push(ui.painter().layout_no_wrap(
                self.program.to_owned(),
                f.clone(),
                tk.fg_faint,
            ));
        }
        for flag in self.flags {
            let mut job = LayoutJob::default();
            match flag.split_once('=') {
                Some((k, v)) => {
                    job.append(k, 0.0, fmt(tk.accent));
                    job.append("=", 0.0, fmt(tk.fg_faint));
                    job.append(v, 0.0, fmt(tk.ok));
                }
                None => job.append(flag, 0.0, fmt(tk.accent)),
            }
            words.push(ui.painter().layout_job(job));
        }
        let space = 6.6;
        let start = ui.cursor().min + Vec2::new(8.0, 0.0);
        let (mut x, mut y) = (0.0_f32, 0.0_f32);
        let mut placed: Vec<(Pos2, Arc<Galley>)> = Vec::with_capacity(words.len() + 1);
        for (i, g) in words.into_iter().enumerate() {
            let gw = g.size().x;
            if i > 0 {
                if x + space + gw > max_w {
                    x = 0.0;
                    y += line_h;
                } else {
                    x += space;
                }
            }
            placed.push((start + Vec2::new(x, y + (line_h - g.size().y) / 2.0), g));
            x += gw;
        }
        if self.flags.is_empty() {
            if !self.program.is_empty() {
                y += line_h;
            }
            let g = ui
                .painter()
                .layout_no_wrap(self.empty.to_owned(), f, tk.fg_faint);
            placed.push((start + Vec2::new(0.0, y + (line_h - g.size().y) / 2.0), g));
        }
        let h = y + line_h;
        let (r, _) = ui.allocate_exact_size(Vec2::new(max_w + 16.0, h), Sense::hover());
        let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
        for (pos, g) in placed {
            p.galley(pos, g, tk.fg_muted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mods_sort_missing_first() {
        let mut v = vec![
            ModRef::new(3, "Zed", ModState::Installed, 10),
            ModRef::new(1, "Bee", ModState::Stale, 10),
            ModRef::new(2, "Ant", ModState::Missing, 0),
            ModRef::new(4, "", ModState::Missing, 0),
        ];
        ModRef::sort(&mut v);
        let names: Vec<&str> = v.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Ant", "Workshop 4", "Bee", "Zed"]);
        assert!(v[0].workshop_url().ends_with("?id=2"));
    }
}
