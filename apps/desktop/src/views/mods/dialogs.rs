//! The view's two dialogs: installing by Workshop id or link (`InstallDialog.svelte`)
//! and the review every update and install goes through before anything downloads
//! (`ReviewDialog.svelte`, `review.svelte.ts`).

use dz_hub::features::mods::InstalledModDto;
use egui::{CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use serde_json::{Value, json};

use super::model::{self, parse_workshop_ids};
use super::shared;
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::paint::alpha;
use crate::kit::{Button, Dialog, DialogSize, IconButton, Size, Tag, TextArea, Tone, Variant};
use crate::parts::{draw, ph, ph_trunc};
use crate::store::mods::Mods;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What is waiting for the go-ahead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewKind {
    UpdateStale,
    UpdateAll,
    UpdateSelected,
    Install,
}

/// One mod of a review: what is known of it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewMod {
    pub id: u64,
    pub name: String,
    pub size: Option<u64>,
    pub local_updated: Option<i64>,
    pub remote_updated: Option<i64>,
    pub installed: bool,
}

impl ReviewMod {
    fn of(m: &InstalledModDto) -> Self {
        Self {
            id: m.id,
            name: m.name.clone(),
            size: Some(m.size),
            local_updated: Some(m.local_updated),
            remote_updated: m.remote_updated,
            installed: true,
        }
    }
}

/// An update or install waiting for the player's go-ahead.
#[derive(Clone, Debug, PartialEq)]
pub struct Review {
    pub kind: ReviewKind,
    pub mods: Vec<ReviewMod>,
}

impl Review {
    pub fn stale(mods: &Mods) -> Option<Self> {
        let list: Vec<ReviewMod> = mods.stale().map(ReviewMod::of).collect();
        (!list.is_empty()).then_some(Self {
            kind: ReviewKind::UpdateStale,
            mods: list,
        })
    }

    pub fn all(mods: &Mods) -> Option<Self> {
        let list: Vec<ReviewMod> = mods.installed.iter().map(ReviewMod::of).collect();
        (!list.is_empty()).then_some(Self {
            kind: ReviewKind::UpdateAll,
            mods: list,
        })
    }

    pub fn selected(mods: &Mods, ids: &[u64]) -> Option<Self> {
        let list: Vec<ReviewMod> = mods
            .installed
            .iter()
            .filter(|m| ids.contains(&m.id))
            .map(ReviewMod::of)
            .collect();
        (!list.is_empty()).then_some(Self {
            kind: ReviewKind::UpdateSelected,
            mods: list,
        })
    }

    pub fn install(mods: &Mods, ids: &[u64]) -> Option<Self> {
        if ids.is_empty() {
            return None;
        }
        let list = ids
            .iter()
            .map(|id| match mods.by_id(*id) {
                Some(m) => ReviewMod::of(m),
                None => ReviewMod {
                    id: *id,
                    name: format!("Workshop {id}"),
                    size: None,
                    local_updated: None,
                    remote_updated: None,
                    installed: false,
                },
            })
            .collect();
        Some(Self {
            kind: ReviewKind::Install,
            mods: list,
        })
    }

    /// Start what it shows.
    pub fn confirm(self, cx: &mut Ctx) {
        let via = cx.stores.profile.via_steam();
        let ids: Vec<u64> = self.mods.iter().map(|m| m.id).collect();
        let m = &mut cx.stores.mods;
        match self.kind {
            ReviewKind::UpdateStale => m.update_stale(via),
            ReviewKind::UpdateAll => m.update_all(via),
            ReviewKind::UpdateSelected => m.update_many(ids, via),
            ReviewKind::Install => m.install(ids, via),
        }
    }
}

type Run = Vec<(&'static str, Value)>;

/// Both dialogs, while the mods view is on screen.
pub(super) fn show(ctx: &egui::Context, cx: &mut Ctx) {
    let s = shared(ctx);
    let mut run: Run = Vec::new();
    {
        let mut st = s.lock();
        let st = &mut *st;
        install(
            ctx,
            cx,
            &mut st.install_open,
            &mut st.install_text,
            &mut run,
        );
        review(ctx, cx, &mut st.review, &mut run);
    }
    for (id, a) in run {
        if let Err(e) = cx.run(id, &a) {
            cx.stores.bus.err(e);
        }
    }
}

fn install(ctx: &egui::Context, cx: &Ctx, open: &mut bool, text: &mut String, run: &mut Run) {
    let mods = &cx.stores.mods;
    let ids = parse_workshop_ids(text);
    let label = if ids.len() > 1 {
        tf("mods", "installMods", &[("count", &ids.len())])
    } else {
        t("mods", "installMod").to_string()
    };
    let found = tf("mods", "installParsed", &[("count", &ids.len())]);
    Dialog::new("mods-install", t("mods", "installModalTitle"))
        .description(t("mods", "installModalDesc"))
        .size(DialogSize::Md)
        .close_label(t("mods", "close"))
        .show(
            ctx,
            open,
            |ui, _| {
                let tk = ui.tokens();
                let w = ui.available_width();
                let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
                let a = ph(
                    ui,
                    t("mods", "workshopIdsLabel"),
                    font(Face::SansMedium, size::TEXT_2XS),
                );
                draw(ui.painter(), &a, r.left(), r.center().y, tk.fg);
                let b = ph(
                    ui,
                    t("mods", "workshopIdsHint"),
                    font(Face::Sans, size::TEXT_2XS),
                );
                draw(
                    ui.painter(),
                    &b,
                    r.right() - b.size().x,
                    r.center().y,
                    tk.fg_faint,
                );
                ui.add(TextArea::new(text).rows(6).mono().placeholder(
                    "1559212036\nhttps://steamcommunity.com/sharedfiles/filedetails/?id=1564026768",
                ));
                // What was read from the text, once there is text: no blank line under
                // an empty field.
                if text.trim().is_empty() {
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.set_min_height(24.0);
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    if ids.is_empty() {
                        let g = ph(
                            ui,
                            t("mods", "noValidDetected"),
                            font(Face::Sans, size::TEXT_2XS),
                        );
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(g.size().x, 18.0), Sense::hover());
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.err);
                    } else if !ids.is_empty() {
                        let g = ph(ui, &found, font(Face::Sans, size::TEXT_2XS));
                        let (r, _) = ui
                            .allocate_exact_size(Vec2::new(g.size().x + 4.0, 18.0), Sense::hover());
                        draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                        for id in ids.iter().take(12) {
                            let s = id.to_string();
                            let have = mods.by_id(*id);
                            let tip = have.map_or("", |m| m.name.as_str());
                            ui.add(
                                Tag::new(&s)
                                    .tone(if have.is_some() {
                                        Tone::Neutral
                                    } else {
                                        Tone::Accent
                                    })
                                    .tip(tip),
                            );
                        }
                        if ids.len() > 12 {
                            let more = format!("+{}", ids.len() - 12);
                            let g = ph(ui, &more, font(Face::Sans, size::TEXT_2XS));
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::new(g.size().x, 18.0), Sense::hover());
                            draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_faint);
                        }
                    }
                });
            },
            |ui, open| {
                if ui
                    .add(Button::new(t("mods", "cancel")).variant(Variant::Ghost))
                    .clicked()
                {
                    *open = false;
                }
                if ui
                    .add(
                        Button::new(&label)
                            .icon(lucide::DOWNLOAD)
                            .variant(Variant::Accent)
                            .enabled(!ids.is_empty()),
                    )
                    .clicked()
                {
                    run.push(("mods.install", json!({ "ids": ids })));
                }
            },
        );
}

fn review(ctx: &egui::Context, cx: &Ctx, pending: &mut Option<Review>, out: &mut Run) {
    let run = std::cell::RefCell::new(Vec::<(&'static str, Value)>::new());
    let mut open = pending.is_some();
    // The dialog fades out with what it showed: kept for that moment.
    let shown_id = egui::Id::new("mods-review-last");
    if let Some(p) = pending.as_ref() {
        ctx.data_mut(|d| d.insert_temp(shown_id, std::sync::Arc::new(p.clone())));
    }
    let Some(p) = ctx.data(|d| d.get_temp::<std::sync::Arc<Review>>(shown_id)) else {
        return;
    };
    let install = p.kind == ReviewKind::Install;
    let count = p.mods.len();
    let total: u64 = p.mods.iter().filter_map(|m| m.size).sum();
    let title = if install {
        tf("mods", "confirmTitleInstall", &[("count", &count)])
    } else {
        tf("mods", "confirmTitleUpdate", &[("count", &count)])
    };
    let desc = if p.kind == ReviewKind::UpdateAll {
        t("mods", "reviewUpdateAll")
    } else {
        ""
    };
    let size_tag = if total > 0 {
        tf(
            "mods",
            "reviewSize",
            &[("size", &format::bytes(total as f64))],
        )
    } else {
        t("mods", "reviewUnknownSize").to_string()
    };
    let go = if install {
        tf("mods", "confirmInstall", &[("count", &count)])
    } else {
        tf("mods", "confirmUpdate", &[("count", &count)])
    };
    let via_steam = cx.stores.profile.via_steam();
    let closed = Dialog::new("mods-review", &title)
        .description(desc)
        .size(DialogSize::Md)
        .close_label(t("mods", "confirmCancel"))
        .show(
            ctx,
            &mut open,
            |ui, _| {
                let tk = ui.tokens();
                ui.add(Tag::new(&size_tag));
                ui.add_space(2.0);
                review_list(ui, &p, install, &mut run.borrow_mut());
                ui.add_space(4.0);
                let (icon, ink, text) = if via_steam {
                    (lucide::INFO, tk.fg_faint, t("mods", "confirmWarningSteam"))
                } else {
                    (lucide::TRIANGLE_ALERT, tk.warn, t("mods", "confirmWarning"))
                };
                let w = ui.available_width();
                let g = crate::kit::paint::galley_wrap(
                    ui,
                    text,
                    font(Face::Sans, size::TEXT_2XS),
                    ink,
                    w - 20.0,
                );
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(w, g.size().y.max(14.0)), Sense::hover());
                icon.paint(
                    ui.painter(),
                    Rect::from_min_size(Pos2::new(r.left(), r.top() + 1.0), Vec2::splat(14.0)),
                    ink,
                );
                ui.painter()
                    .galley(Pos2::new(r.left() + 20.0, r.top()), g, ink);
            },
            |ui, open| {
                if ui
                    .add(Button::new(t("mods", "confirmCancel")).variant(Variant::Ghost))
                    .clicked()
                {
                    *open = false;
                }
                let icon = if install {
                    lucide::DOWNLOAD
                } else {
                    lucide::REFRESH_CW
                };
                if ui
                    .add(Button::new(&go).icon(icon).variant(Variant::Accent))
                    .clicked()
                {
                    run.borrow_mut().push(("mods.review_confirm", Value::Null));
                }
            },
        );
    if closed
        && pending.is_some()
        && !run
            .borrow()
            .iter()
            .any(|(id, _)| *id == "mods.review_confirm")
    {
        *pending = None;
    }
    out.extend(run.into_inner());
}

/// The mods a review touches: name, id, dates, size; "already installed" and how much
/// newer; a link to each Workshop page. A bounded scroller (`max-h-80`).
fn review_list(ui: &mut Ui, p: &Review, install: bool, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    // Each row measured: the meta line wraps in the room left of the link.
    let inner_w = w - 2.0 - 16.0;
    let rows: Vec<ReviewRow> = p
        .mods
        .iter()
        .map(|m| ReviewRow::new(ui, m, install, inner_w))
        .collect();
    let total: f32 = rows.iter().map(|r| r.height).sum();
    let h = total.min(320.0);
    let (box_r, _) = ui.allocate_exact_size(Vec2::new(w, h + 2.0), Sense::hover());
    ui.painter().rect(
        box_r,
        CornerRadius::same(size::RADIUS_SM),
        tk.bg,
        Stroke::new(1.0, tk.border),
        StrokeKind::Inside,
    );
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(box_r.shrink(1.0)));
    egui::ScrollArea::vertical()
        .id_salt("mods-review-list")
        .max_height(h)
        .auto_shrink([false, false])
        .show(&mut c, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for (i, (m, row)) in p.mods.iter().zip(&rows).enumerate() {
                let (r, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), row.height),
                    Sense::hover(),
                );
                // Only what is on screen is drawn.
                if !ui.is_rect_visible(r) {
                    continue;
                }
                if i + 1 < p.mods.len() {
                    ui.painter().hline(
                        r.x_range(),
                        r.bottom() - 0.5,
                        Stroke::new(1.0, alpha(tk.border, 0.6)),
                    );
                }
                let inner = r.shrink2(Vec2::new(8.0, 6.0));
                let lr = Rect::from_center_size(
                    Pos2::new(inner.right() - size::CONTROL_SM / 2.0, inner.center().y),
                    Vec2::splat(size::CONTROL_SM),
                );
                // The tags sit on the name's line; the meta line below has the width.
                let mut right = lr.left() - 8.0;
                for (text, tone, tw) in &row.tags {
                    let tr = Rect::from_min_size(
                        Pos2::new(right - tw, inner.top()),
                        Vec2::new(*tw, 16.0),
                    );
                    ui.place(tr, Tag::new(text).tone(*tone));
                    right = tr.left() - 8.0;
                }
                let text_w = (right - inner.left()).max(8.0);
                let ng = ph_trunc(ui, &m.name, font(Face::SansMedium, size::TEXT_XS), text_w);
                draw(ui.painter(), &ng, inner.left(), inner.top() + 8.0, tk.fg);
                ui.painter().galley(
                    Pos2::new(inner.left(), inner.top() + 16.0),
                    row.meta.clone(),
                    tk.fg_faint,
                );
                if ui
                    .place(
                        lr,
                        IconButton::new(lucide::EXTERNAL_LINK, t("mods", "confirmOpenWorkshop"))
                            .size(Size::IconXs)
                            .variant(Variant::Ghost),
                    )
                    .clicked()
                {
                    run.push(("mods.open_workshop", json!({ "id": m.id })));
                }
            }
        });
}

/// One row of the review, measured: its tags (text, tone, width), shown on the name's
/// line, the meta line laid out below, and the row's height.
struct ReviewRow {
    tags: Vec<(String, Tone, f32)>,
    meta: std::sync::Arc<egui::Galley>,
    height: f32,
}

impl ReviewRow {
    fn new(ui: &Ui, m: &ReviewMod, install: bool, inner_w: f32) -> Self {
        let days = match (m.local_updated, m.remote_updated) {
            (Some(l), r) if l > 0 => model::days_behind(l, r),
            _ => 0,
        };
        let mut tags: Vec<(String, Tone, f32)> = Vec::new();
        let mut add = |text: String, tone: Tone| {
            let g = ph(ui, &text, font(Face::Mono, size::TEXT_2XS));
            let tw = g.size().x + 12.0;
            tags.push((text, tone, tw));
        };
        if days > 0 {
            add(tf("mods", "confirmNewer", &[("days", &days)]), Tone::Warn);
        }
        if install && m.installed {
            add(t("mods", "installAlready").to_string(), Tone::Neutral);
        }
        let text_w = (inner_w - size::CONTROL_SM - 8.0).max(40.0);
        // `gap-x-3` between the parts; a part never splits across lines.
        let mut parts: Vec<String> = vec![format!("#{}", m.id)];
        if let Some(l) = m.local_updated.filter(|l| *l > 0) {
            parts.push(format!(
                "{} {}",
                t("mods", "confirmLocalDate"),
                format::date(l)
            ));
        }
        if let Some(r) = m.remote_updated {
            parts.push(format!(
                "{} {}",
                t("mods", "confirmRemoteDate"),
                format::date(r)
            ));
        }
        if let Some(s) = m.size.filter(|s| *s > 0) {
            parts.push(format::bytes(s as f64));
        }
        let parts: Vec<String> = parts
            .into_iter()
            .map(|p| p.replace(' ', "\u{a0}"))
            .collect();
        let meta = crate::kit::paint::galley_wrap(
            ui,
            &parts.join("   "),
            font(Face::Mono, size::TEXT_3XS),
            egui::Color32::PLACEHOLDER,
            text_w,
        );
        let height = 12.0 + 16.0 + meta.size().y;
        Self { tags, meta, height }
    }
}
