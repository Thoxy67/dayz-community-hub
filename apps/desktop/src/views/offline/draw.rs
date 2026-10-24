//! The offline view's drawing: the header, the status strip, the mission cards (a grid,
//! `auto-fill minmax(16rem, 1fr)`) and the aside ("How it works", the mods missions load,
//! "Maintenance").
//! Clicks become commands pushed on `run`, which the view runs once the state is unlocked.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Color32, CornerRadius, Layout, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
    pos2, vec2,
};
use serde_json::{Value, json};

use super::{Mission, OfflineState, Tone};
use crate::i18n::{format, t, tf};
use crate::icons::{game, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Button, IconButton, Input, Meter, MeterSize, Panel, Size, SkeletonRows, Spinner, Tag, TipExt,
    Toggle, Variant, panel, topo,
};
use crate::parts::{Empty, Figure, PageHeader};
use crate::store::mods::Mods;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

type Run = Vec<(&'static str, Value)>;

/// The aside's width (`18rem`).
const ASIDE_W: f32 = 288.0;
/// A card's least width (`16rem`) and the grid's gap (`gap-2.5`).
const CARD_MIN: f32 = 256.0;
const GAP: f32 = 10.0;
/// The banner's height (`h-24`).
const BANNER_H: f32 = 96.0;
/// One fact row of a card (`text-2xs`, its line) and the gap between rows (`gap-y-1`).
const FACT_H: f32 = 15.4;
const FACT_GAP: f32 = 4.0;
/// The card's foot: `py-1.5` around a control, its top border.
const FOOT_H: f32 = 6.0 + size::CONTROL + 6.0 + 1.0;

pub(super) fn show(ui: &mut Ui, st: &mut OfflineState, mods: &Mods, run: &mut Run) {
    let tk = ui.tokens();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, CornerRadius::ZERO, tk.panel);
    let mut v = ui.new_child(
        UiBuilder::new()
            .max_rect(full)
            .layout(Layout::top_down(Align::Min)),
    );
    v.spacing_mut().item_spacing = Vec2::ZERO;
    header(&mut v, st, run);
    if !st.status.is_empty() {
        status_strip(&mut v, st);
    }
    let rest = v.available_rect_before_wrap();
    if !rest.is_positive() {
        return;
    }
    let aside_w = ASIDE_W.min((rest.width() - 1.0) * 0.5).max(0.0);
    let main = Rect::from_min_max(rest.min, pos2(rest.right() - aside_w - 1.0, rest.bottom()));
    let aside = Rect::from_min_max(pos2(rest.right() - aside_w, rest.top()), rest.max);
    v.painter().vline(
        main.right() + 0.5,
        rest.y_range(),
        Stroke::new(1.0, tk.border),
    );

    let mut mui = v.new_child(
        UiBuilder::new()
            .max_rect(main)
            .id_salt("offline-missions")
            .layout(Layout::top_down(Align::Min)),
    );
    mui.set_clip_rect(main.intersect(ui.clip_rect()));
    missions(&mut mui, st, run);

    let mut aui = v.new_child(
        UiBuilder::new()
            .max_rect(aside)
            .id_salt("offline-aside")
            .layout(Layout::top_down(Align::Min)),
    );
    aui.set_clip_rect(aside.intersect(ui.clip_rect()));
    aside_panels(&mut aui, st, mods, run);
}

fn header(ui: &mut Ui, st: &OfflineState, out: &mut Run) {
    let tk = ui.tokens();
    let run = std::cell::RefCell::new(Run::new());
    let installed = st.installed();
    let busy = st.busy();
    let (loading, installing) = (st.loading, st.installing);
    let (n, maps) = (st.missions.len(), st.map_count());
    PageHeader::new(t("offline", "title")).show(
        ui,
        |ui| {
            let (word, tone) = if installed {
                (t("offline", "installed"), tk.ok)
            } else {
                (t("offline", "notInstalled"), tk.fg_faint)
            };
            ui.add(Figure::new(t("offline", "state")).value(word).tone(tone));
            ui.add(
                Figure::new(t("offline", "missions"))
                    .amount(n as f64)
                    .tone(tk.accent),
            );
            ui.add(
                Figure::new(t("offline", "maps"))
                    .amount(maps as f64)
                    .tone(tk.map)
                    .secondary(),
            );
        },
        |ui| {
            if ui
                .add(
                    Button::new(t("servers", "refresh"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(loading)
                        .enabled(!busy)
                        .tip(t("offline", "refreshTitle")),
                )
                .clicked()
            {
                run.borrow_mut().push(("offline.refresh", Value::Null));
            }
            // No file manager in Game Mode.
            if crate::platform::deck::folders()
                && ui
                    .add(
                        Button::new(t("offline", "explore"))
                            .icon(lucide::FOLDER_OPEN)
                            .tip(t("offline", "exploreTitle")),
                    )
                    .clicked()
            {
                run.borrow_mut()
                    .push(("offline.open_missions_dir", Value::Null));
            }
            let label = if installing {
                t("offline", "installing")
            } else {
                t("offline", "installUpdate")
            };
            if ui
                .add(
                    Button::new(label)
                        .icon(lucide::DOWNLOAD)
                        .spinner(installing)
                        .variant(Variant::Accent)
                        .enabled(!busy),
                )
                .clicked()
            {
                run.borrow_mut().push(("offline.install", Value::Null));
            }
        },
        None::<fn(&mut Ui)>,
    );
    out.extend(run.into_inner());
}

/// The strip under the header (`role=status`), tinted by its tone.
fn status_strip(ui: &mut Ui, st: &OfflineState) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 29.0), Sense::hover());
    let (fill, line, ink) = match st.tone {
        Tone::Neutral => (alpha(tk.raised, 0.4), tk.border, tk.fg_muted),
        Tone::Ok => (alpha(tk.ok, 0.08), alpha(tk.ok, 0.3), tk.ok),
        Tone::Warn => (alpha(tk.warn, 0.08), alpha(tk.warn, 0.3), tk.warn),
        Tone::Err => (alpha(tk.err, 0.08), alpha(tk.err, 0.3), tk.err),
    };
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::ZERO, fill);
    p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, line));
    let cy = rect.center().y - 0.5;
    let x = rect.left() + size::PAD;
    let ir = Rect::from_min_size(
        pos2(x, cy - size::ICON_SM / 2.0),
        Vec2::splat(size::ICON_SM),
    );
    if st.installing || st.loading {
        Spinner::paint(ui, ir, ink);
    } else {
        let icon = match st.tone {
            Tone::Ok => lucide::CIRCLE_CHECK,
            Tone::Neutral => lucide::INFO,
            Tone::Warn | Tone::Err => lucide::CIRCLE_ALERT,
        };
        icon.paint(p, ir, ink);
    }
    let sweep_w = if st.installing { 160.0 + 8.0 } else { 0.0 };
    let tx = ir.right() + 8.0;
    let g = paint::galley_trunc(
        ui,
        &st.status,
        font(Face::Sans, size::TEXT_2XS),
        ink,
        (rect.right() - size::PAD - sweep_w - tx).max(0.0),
    );
    paint::text_at(p, &g, tx, cy, ink);
    if st.installing {
        let mr = Rect::from_min_size(
            pos2(rect.right() - size::PAD - 160.0, cy - 2.0),
            vec2(160.0, 4.0),
        );
        let mut c = ui.new_child(
            UiBuilder::new()
                .max_rect(mr)
                .layout(Layout::left_to_right(Align::Center)),
        );
        c.add(
            Meter::new(0.0)
                .indeterminate()
                .size(MeterSize::Sm)
                .width(160.0),
        );
    }
}

// ── the missions ────────────────────────────────────────────────────────────

fn missions(ui: &mut Ui, st: &mut OfflineState, run: &mut Run) {
    let tk = ui.tokens();
    if !st.loaded {
        ui.add(SkeletonRows::new(6));
        return;
    }
    let busy = st.busy();
    if st.missions.is_empty() {
        let mut c = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.max_rect().shrink(size::PAD))
                .layout(Layout::top_down(Align::Min)),
        );
        if let Some(err) = &st.load_error {
            // The folder could not be read: say why, and offer to read it again.
            Empty::new()
                .icon(lucide::CIRCLE_ALERT)
                .title(t("offline", "loadFailed"))
                .error(err)
                .show(&mut c, |ui| {
                    if ui
                        .add(
                            Button::new(t("servers", "refresh"))
                                .icon(lucide::REFRESH_CW)
                                .enabled(!busy),
                        )
                        .clicked()
                    {
                        run.push(("offline.refresh", Value::Null));
                    }
                });
            return;
        }
        let hint = tf(
            "offline",
            "noMissionsHint",
            &[("installButton", &t("offline", "installButton"))],
        );
        Empty::new()
            .icon(game::CAMPING_TENT)
            .title(t("offline", "noMissions"))
            .text(&hint)
            .show(&mut c, |ui| {
                if ui
                    .add(
                        Button::new(t("offline", "installButton"))
                            .icon(lucide::DOWNLOAD)
                            .variant(Variant::Accent)
                            .enabled(!busy),
                    )
                    .clicked()
                {
                    run.push(("offline.install", Value::Null));
                }
            });
        return;
    }
    let mut hovered: Option<String> = None;
    egui::ScrollArea::vertical()
        .id_salt("offline-cards")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(size::PAD as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    let g = panel::stencil(ui, t("offline", "doubleclickHint"), tk.fg_faint, None);
                    let (hr, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 11.0), Sense::hover());
                    paint::text_at(ui.painter(), &g, hr.left(), hr.center().y, tk.fg_faint);
                    ui.add_space(8.0);
                    let w = ui.available_width().max(1.0);
                    let cols = (((w + GAP) / (CARD_MIN + GAP)).floor() as usize).max(1);
                    let card_w = ((w - GAP * (cols - 1) as f32) / cols as f32).max(1.0);
                    let card_h = BANNER_H + 16.0 + FACT_H * 3.0 + FACT_GAP * 2.0 + FOOT_H;
                    let rows = st.missions.len().div_ceil(cols);
                    let total = rows as f32 * card_h + rows.saturating_sub(1) as f32 * GAP;
                    let (grid, _) = ui.allocate_exact_size(vec2(w, total), Sense::hover());
                    for (i, m) in st.missions.iter().enumerate() {
                        let (r, c) = (i / cols, i % cols);
                        let rect = Rect::from_min_size(
                            pos2(
                                grid.left() + c as f32 * (card_w + GAP),
                                grid.top() + r as f32 * (card_h + GAP),
                            ),
                            vec2(card_w, card_h),
                        );
                        if !ui.is_rect_visible(rect) {
                            continue;
                        }
                        let save = st.saves.get(&m.id).map(|s| (s.bytes, s.written));
                        if card(ui, m, save, busy, rect, run) {
                            hovered = Some(m.id.clone());
                        }
                    }
                });
        });
    if hovered.is_some() {
        st.focused = hovered;
    }
}

/// One mission's card; true while the pointer is over it.
fn card(
    ui: &mut Ui,
    m: &Mission,
    save: Option<(u64, Option<i64>)>,
    busy: bool,
    rect: Rect,
    run: &mut Run,
) -> bool {
    let tk = ui.tokens();
    let id = ui.id().with(("offline-card", &m.id));
    let resp = ui.interact(rect, id, Sense::click());
    // A controller's X launches the card its focus is on.
    crate::pad::tag(ui.ctx(), rect, json!({ "mission": m.id }));
    let hovered = ui.rect_contains_pointer(rect);
    if resp.double_clicked() {
        run.push(("offline.launch", json!({ "mission": m.id })));
    }
    let radius = CornerRadius::same(size::RADIUS_MD);
    let hover = paint::ease(ui, id.with("hover"), hovered);
    let border = paint::lerp(tk.border, tk.border_strong, hover);
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, radius, tk.bg);

    // The map's name set big over contour lines: a survey sheet's label.
    let banner = Rect::from_min_size(rect.min, vec2(rect.width(), BANNER_H));
    p.rect_filled(
        banner,
        CornerRadius {
            nw: size::RADIUS_MD,
            ne: size::RADIUS_MD,
            sw: 0,
            se: 0,
        },
        alpha(tk.raised, 0.5),
    );
    topo::paint_tinted(
        &p.with_clip_rect(banner.intersect(p.clip_rect())),
        banner,
        alpha(tk.map, 0.35),
        0.9,
    );
    p.hline(
        banner.x_range(),
        banner.bottom() - 0.5,
        Stroke::new(1.0, tk.border),
    );
    let inset = Rect::from_min_max(
        pos2(banner.left() + 12.0, banner.top()),
        pos2(banner.right() - 12.0, banner.bottom() - 8.0),
    );
    let mut tag_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(inset)
            .layout(Layout::right_to_left(Align::Max)),
    );
    let tag = tag_ui.add(
        Tag::new(m.kind.label())
            .tone(crate::kit::Tone::Accent)
            .filled(),
    );
    let name_w = (tag.rect.left() - 8.0 - inset.left()).max(10.0);
    let name = paint::tracked(
        ui,
        &m.map,
        font(Face::DisplayBold, size::TEXT_2XL),
        tk.fg,
        0.02,
        true,
        Some(name_w),
    );
    // `leading-none`: the line box is the font size; its bottom on the inset's.
    let fs = size::TEXT_2XL;
    let top = inset.bottom() - fs - (name.size().y - fs) / 2.0 + 1.0;
    p.galley(pos2(inset.left(), top), name, tk.fg);

    // The facts (the kind is the banner's tag, not a line of its own).
    let small = font(Face::Sans, size::TEXT_2XS);
    let labels = [
        t("offline", "terrain"),
        t("offline", "save"),
        t("offline", "folder"),
    ];
    let label_w = labels
        .iter()
        .map(|l| paint::galley(ui, l, small.clone(), tk.fg_faint).size().x)
        .fold(0.0_f32, f32::max);
    let x0 = rect.left() + 12.0;
    let x1 = x0 + label_w + 12.0;
    let right = rect.right() - 12.0;
    let row_cy = |i: usize| banner.bottom() + 8.0 + i as f32 * (FACT_H + FACT_GAP) + FACT_H / 2.0;
    for (i, l) in labels.iter().enumerate() {
        let g = paint::galley(ui, l, small.clone(), tk.fg_faint);
        paint::text_at(&p, &g, x0, row_cy(i), tk.fg_faint);
    }
    // Terrain: the map icon and the area.
    let ir = Rect::from_min_size(pos2(x1, row_cy(0) - 6.0), Vec2::splat(12.0));
    lucide::MAP.paint(&p, ir, tk.map);
    let g = paint::galley(ui, m.terrain.unwrap_or(format::NONE), small.clone(), tk.map);
    paint::text_at(&p, &g, ir.right() + 4.0, row_cy(0), tk.map);
    // Save.
    let (text, ink) = match save {
        Some((bytes, written)) => (
            tf(
                "offline",
                "saveLine",
                &[
                    ("size", &format::bytes(bytes as f64)),
                    (
                        "when",
                        &written.map_or_else(|| "—".to_owned(), format::relative),
                    ),
                ],
            ),
            tk.fg_muted,
        ),
        None => (t("offline", "noSave").to_owned(), tk.fg_faint),
    };
    let g = paint::galley_trunc(ui, &text, small, ink, (right - x1).max(0.0));
    paint::text_at(&p, &g, x1, row_cy(1), ink);
    // Folder: mono, cut, the whole id on hover, selectable.
    let fr = Rect::from_min_max(
        pos2(x1, row_cy(2) - FACT_H / 2.0),
        pos2(right.max(x1), row_cy(2) + FACT_H / 2.0),
    );
    let mut fui = ui.new_child(
        UiBuilder::new()
            .max_rect(fr)
            .id_salt(("offline-folder", &m.id))
            .layout(Layout::left_to_right(Align::Center)),
    );
    fui.add(
        egui::Label::new(
            egui::RichText::new(&m.id)
                .font(font(Face::Mono, size::TEXT_2XS))
                .color(tk.fg_muted),
        )
        .truncate()
        .selectable(true),
    )
    .tip(&m.id);

    // The foot: launch, the folder, clear the save, remove.
    let foot = Rect::from_min_max(pos2(rect.left(), rect.bottom() - FOOT_H), rect.max);
    p.hline(
        foot.x_range(),
        foot.top() + 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.7)),
    );
    let mut f = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_max(
                pos2(foot.left() + 8.0, foot.top() + 1.0),
                pos2(foot.right() - 8.0, foot.bottom()),
            ))
            .id_salt(("offline-foot", &m.id))
            .layout(Layout::left_to_right(Align::Center)),
    );
    f.spacing_mut().item_spacing.x = 4.0;
    let play_w = (f.available_width() - 3.0 * (size::CONTROL + 4.0)).max(size::CONTROL);
    let mission = json!({ "mission": m.id });
    let launch = f.add(
        Button::new(t("offline", "launch"))
            .icon(lucide::PLAY)
            .variant(Variant::Play)
            .width(play_w),
    );
    // A controller lands on the first card's Launch on screen.
    crate::pad::primary(f.ctx(), launch.id);
    if launch.clicked() {
        run.push(("offline.launch", mission.clone()));
    }
    if crate::platform::deck::folders()
        && f.add(IconButton::new(
            lucide::FOLDER_OPEN,
            t("offline", "openFolder"),
        ))
        .clicked()
    {
        run.push(("offline.open_dir", mission.clone()));
    }
    if f.add(
        IconButton::new(lucide::ERASER, t("offline", "clearSave")).enabled(!busy && save.is_some()),
    )
    .clicked()
    {
        run.push(("offline.clear_save", mission.clone()));
    }
    if f.add(
        IconButton::new(lucide::TRASH_2, t("offline", "removeMission"))
            .variant(Variant::Danger)
            .enabled(!busy),
    )
    .clicked()
    {
        run.push(("offline.remove_mission", mission));
    }

    // The keyboard's focus on the card: its border becomes the focus ring.
    let stroke = if paint::keyboard_focus(ui, &resp) {
        Stroke::new(2.0, tk.accent)
    } else {
        Stroke::new(1.0, border)
    };
    ui.painter()
        .rect_stroke(rect, radius, stroke, StrokeKind::Inside);
    hovered
}

// ── the aside ───────────────────────────────────────────────────────────────

fn aside_panels(ui: &mut Ui, st: &mut OfflineState, mods: &Mods, run: &mut Run) {
    let tk = ui.tokens();
    let disabled = st.busy() || !st.installed();
    ui.painter()
        .rect_filled(ui.max_rect(), CornerRadius::ZERO, tk.panel);
    egui::ScrollArea::vertical()
        .id_salt("offline-aside-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            Panel::new("offline-how")
                .title(t("offline", "howItWorks"))
                .no_scroll()
                .show(ui, |ui| how_it_works(ui, &tk, run));
            let (sep, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep, CornerRadius::ZERO, tk.border);
            Panel::new("offline-mods")
                .title(t("offline", "modsTitle"))
                .no_scroll()
                .show(ui, |ui| mods_panel(ui, &tk, st, mods, run));
            let (sep, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep, CornerRadius::ZERO, tk.border);
            Panel::new("offline-maintenance")
                .title(t("offline", "maintenance"))
                .no_scroll()
                .show(ui, |ui| maintenance(ui, &tk, disabled, run));
        });
}

/// Text wrapped at `width` with its own line height (`leading-relaxed`, `leading-snug`).
fn para(
    ui: &Ui,
    text: &str,
    size: f32,
    line: f32,
    color: Color32,
    width: f32,
) -> Arc<egui::Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font(Face::Sans, size),
            color,
            line_height: Some(line),
            ..Default::default()
        },
    );
    // A browser lets a line's trailing space hang past the edge; egui counts it.
    job.wrap.max_width = (width + 3.0).max(1.0);
    ui.painter().layout_job(job)
}

fn place(ui: &mut Ui, g: Arc<egui::Galley>, x: f32, color: Color32) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), g.size().y), Sense::hover());
    ui.painter().galley(pos2(x, r.top()), g, color);
}

fn how_it_works(ui: &mut Ui, tk: &Tokens, run: &mut Run) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            let line = size::TEXT_2XS * 1.625;
            let x = ui.cursor().left();
            let g = para(
                ui,
                t("offline", "intro"),
                size::TEXT_2XS,
                line,
                tk.fg_muted,
                w,
            );
            place(ui, g, x, tk.fg_muted);
            // `space-y-2.5` gives way to the children's `m-0`: no gap after the intro and
            // the steps (Tailwind 4 puts it on their bottom margins).
            for (i, key) in ["step1", "step2", "step3"].iter().enumerate() {
                if i > 0 {
                    ui.add_space(6.0);
                }
                let g = para(
                    ui,
                    t("offline", key),
                    size::TEXT_2XS,
                    line,
                    tk.fg_muted,
                    w - 24.0,
                );
                let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), Sense::hover());
                let dot = Rect::from_min_size(pos2(r.left(), r.top() + 1.0), Vec2::splat(16.0));
                let p = ui.painter();
                p.circle_filled(dot.center(), 8.0, alpha(tk.accent, 0.15));
                let n = paint::galley(
                    ui,
                    &(i + 1).to_string(),
                    font(Face::Mono, size::TEXT_3XS),
                    tk.accent,
                );
                paint::text_center(p, &n, dot, tk.accent);
                p.galley(pos2(r.left() + 24.0, r.top()), g, tk.fg_muted);
            }
            // The project's page, as a link card.
            let (r, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
            let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let p = ui.painter();
            paint::plate(
                p,
                r,
                size::RADIUS_SM,
                tk.bg,
                paint::lerp(tk.border, tk.border_strong, hover),
            );
            let ir = Rect::from_min_size(
                pos2(r.left() + 8.0, r.top() + 8.0),
                Vec2::splat(size::ICON_SM),
            );
            lucide::EXTERNAL_LINK.paint(p, ir, tk.accent);
            let tx = ir.right() + 8.0;
            let title = paint::galley(
                ui,
                t("offline", "project"),
                font(Face::SansMedium, size::TEXT_2XS),
                tk.accent,
            );
            p.galley(pos2(tx, r.top() + 6.0), title, tk.accent);
            let url = paint::galley_trunc(
                ui,
                "github.com/Arkensor/DayZCommunityOfflineMode",
                font(Face::Mono, size::TEXT_3XS),
                tk.fg_faint,
                (r.right() - 8.0 - tx).max(0.0),
            );
            p.galley(pos2(tx, r.top() + 22.0), url, tk.fg_faint);
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(p, r, size::RADIUS_SM, tk);
            }
            let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                run.push(("offline.open_project", Value::Null));
            }
        });
}

/// The checklist of installed mods offline missions load (`-mod=`), with its search.
fn mods_panel(ui: &mut Ui, tk: &Tokens, st: &mut OfflineState, mods: &Mods, run: &mut Run) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            let x = ui.cursor().left();
            let line = size::TEXT_3XS * 1.375;
            let g = para(
                ui,
                t("offline", "modsIntro"),
                size::TEXT_3XS,
                line,
                tk.fg_faint,
                w,
            );
            place(ui, g, x, tk.fg_faint);
            ui.add_space(8.0);
            if mods.installed.is_empty() {
                let key = if mods.loaded {
                    "modsNone"
                } else {
                    "modsLoading"
                };
                let g = para(ui, t("offline", key), size::TEXT_2XS, line, tk.fg_muted, w);
                place(ui, g, x, tk.fg_muted);
                return;
            }
            if mods.installed.len() > 6 {
                ui.add(
                    Input::new(&mut st.mods_query)
                        .search()
                        .size(crate::kit::FieldSize::Xs)
                        .placeholder(t("offline", "modsSearch"))
                        .clear_label(t("common", "clear"))
                        .width(w)
                        .id_salt("offline-mods-search"),
                );
                ui.add_space(6.0);
            }
            refresh_mod_rows(st, mods);
            let rows = &st.mods_rows;
            if rows.is_empty() {
                let g = para(
                    ui,
                    t("offline", "modsNoMatch"),
                    size::TEXT_2XS,
                    line,
                    tk.fg_faint,
                    w,
                );
                place(ui, g, x, tk.fg_faint);
            } else {
                // Its own height (at most ten rows): inside the aside's scroller a
                // shrinking one would collapse to its least height. Only the rows on
                // screen are laid out.
                let h = size::CONTROL_SM * (rows.len().min(10) as f32);
                egui::ScrollArea::vertical()
                    .id_salt("offline-mods-list")
                    .min_scrolled_height(h)
                    .max_height(h)
                    .auto_shrink([false, true])
                    .show_rows(ui, size::CONTROL_SM, rows.len(), |ui, range| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for id in &rows[range] {
                            let Some(m) = mods.by_id(*id) else {
                                continue;
                            };
                            let mut on = st.mods.contains(id);
                            let tip = format!("{} · {}", m.name, m.id);
                            let r = ui.push_id(m.id, |ui| {
                                ui.add(Toggle::new(&mut on, &m.name).compact().tip(&tip))
                            });
                            if r.inner.clicked() {
                                run.push(("offline.toggle_mod", json!({ "id": m.id, "on": on })));
                            }
                        }
                    });
            }
            ui.add_space(6.0);
            let chosen = st
                .mods
                .iter()
                .filter(|id| mods.by_id(**id).is_some())
                .count();
            let missing = st.mods.len() - chosen;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let text = if chosen == 0 {
                    t("offline", "modsNoneChosen").to_owned()
                } else {
                    tf("offline", "modsChosen", &[("count", &chosen)])
                };
                let g = paint::galley(ui, &text, font(Face::Sans, size::TEXT_2XS), tk.fg_muted);
                let (r, _) =
                    ui.allocate_exact_size(vec2(g.size().x, size::CONTROL_SM), Sense::hover());
                paint::text_at(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                if !st.mods.is_empty() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(t("offline", "modsClear"))
                                    .variant(Variant::Ghost)
                                    .size(Size::Xs),
                            )
                            .clicked()
                        {
                            run.push(("offline.clear_mods", Value::Null));
                        }
                    });
                }
            });
            if missing > 0 && mods.loaded {
                ui.add_space(4.0);
                let g = para(
                    ui,
                    &tf("offline", "modsMissing", &[("count", &missing)]),
                    size::TEXT_3XS,
                    line,
                    tk.warn,
                    w,
                );
                place(ui, g, x, tk.warn);
                // Each one can go on its own: "Clear" would drop the kept ones too.
                let label = t("offline", "modsUnchoose");
                for id in st.mods.iter().filter(|id| mods.by_id(**id).is_none()) {
                    ui.push_id(("offline-missing", *id), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let bw = size::CONTROL_SM;
                            let name = paint::galley_trunc(
                                ui,
                                &mods.name_of(*id),
                                font(Face::Sans, size::TEXT_2XS),
                                tk.fg_muted,
                                (w - bw - 6.0).max(0.0),
                            );
                            let (r, _) = ui.allocate_exact_size(
                                vec2((w - bw - 6.0).max(0.0), bw),
                                Sense::hover(),
                            );
                            paint::text_at(
                                ui.painter(),
                                &name,
                                r.left(),
                                r.center().y,
                                tk.fg_muted,
                            );
                            if ui
                                .add(
                                    IconButton::new(lucide::X, label)
                                        .size(Size::IconXs)
                                        .danger_hover(),
                                )
                                .clicked()
                            {
                                run.push(("offline.toggle_mod", json!({ "id": id, "on": false })));
                            }
                        });
                    });
                }
            }
        });
}

/// Make the checklist's rows again when the mods, the search or the choice moved:
/// the chosen first, in the order they load, then the others by name.
fn refresh_mod_rows(st: &mut OfflineState, mods: &Mods) {
    let needle = st.mods_query.trim().to_lowercase();
    let fresh = st.mods_rows_key.as_ref().is_some_and(|(rev, q, chosen)| {
        *rev == mods.revision && *q == needle && *chosen == st.mods
    });
    if fresh {
        return;
    }
    st.mods_rows = mod_rows(
        mods.installed.iter().map(|m| (m.id, m.name.as_str())),
        &st.mods,
        &needle,
    );
    st.mods_rows_key = Some((mods.revision, needle, st.mods.clone()));
}

/// The installed mods (`(id, name)`) matching `needle` (lowercase): the `chosen`
/// first, in their order, then the others by name.
fn mod_rows<'a>(
    installed: impl Iterator<Item = (u64, &'a str)>,
    chosen: &[u64],
    needle: &str,
) -> Vec<u64> {
    let rank: rustc_hash::FxHashMap<u64, usize> =
        chosen.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut rows: Vec<(usize, String, u64)> = installed
        .filter_map(|(id, name)| {
            let name = name.to_lowercase();
            let hit = needle.is_empty() || name.contains(needle) || id.to_string().contains(needle);
            hit.then(|| (rank.get(&id).copied().unwrap_or(usize::MAX), name, id))
        })
        .collect();
    rows.sort_unstable();
    rows.into_iter().map(|(_, _, id)| id).collect()
}

fn maintenance(ui: &mut Ui, tk: &Tokens, disabled: bool, run: &mut Run) {
    egui::Frame::new()
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let w = ui.available_width();
            let x = ui.cursor().left();
            let line = size::TEXT_3XS * 1.375;
            let items = [
                (
                    "offline.clear_saves",
                    lucide::ERASER,
                    "clearSaves",
                    "clearSavesTitle",
                    Variant::Default,
                ),
                (
                    "offline.remove_all",
                    lucide::TRASH_2,
                    "removeAll",
                    "removeAllTitle",
                    Variant::Danger,
                ),
            ];
            for (i, (cmd, icon, label, caption, variant)) in items.into_iter().enumerate() {
                if i > 0 {
                    ui.add_space(8.0);
                }
                if ui
                    .add(
                        Button::new(t("offline", label))
                            .icon(icon)
                            .variant(variant)
                            .width(w)
                            .start()
                            .enabled(!disabled),
                    )
                    .clicked()
                {
                    run.push((cmd, Value::Null));
                }
                ui.add_space(4.0);
                let g = para(
                    ui,
                    t("offline", caption),
                    size::TEXT_3XS,
                    line,
                    tk.fg_faint,
                    w,
                );
                place(ui, g, x, tk.fg_faint);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::mod_rows;

    #[test]
    fn the_checklist_puts_the_chosen_first_then_the_rest_by_name() {
        let installed = [(1, "Zeta"), (2, "alpha"), (3, "CF"), (4, "Beta")];
        assert_eq!(
            mod_rows(installed.into_iter(), &[3, 1], ""),
            vec![3, 1, 2, 4]
        );
        assert_eq!(mod_rows(installed.into_iter(), &[], "ta"), vec![4, 1]);
        assert_eq!(mod_rows(installed.into_iter(), &[], "3"), vec![3]);
    }
}
