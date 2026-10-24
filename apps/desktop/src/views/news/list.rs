//! The list of articles (the view's `<aside>`): a thumbnail, the meta line (latest,
//! category, age), the title on two lines at most, the excerpt on one.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CornerRadius, FontId, Galley, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui,
    Vec2,
};
use serde_json::{Value, json};

use super::{Item, NewsState};
use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, SkeletonRows, TipExt};
use crate::parts::Empty;
use crate::theme::{TokensExt, font, fonts::Face, size};

const PAD_Y: f32 = 10.0;
const THUMB: Vec2 = Vec2::new(80.0, 56.0);
const GAP: f32 = 12.0;
/// The excerpt's line (`text-2xs`, leading 1.5).
const EXCERPT_H: f32 = 16.5;

pub fn show(ui: &mut Ui, cx: &Ctx, st: &mut NewsState, run: &mut Vec<(&'static str, Value)>) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    // `bg-bg/40 border-r`.
    ui.painter()
        .rect_filled(rect, CornerRadius::ZERO, alpha(tk.bg, 0.4));
    ui.painter().vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, tk.border),
    );
    let inner = Rect::from_min_max(rect.min, Pos2::new(rect.right() - 1.0, rect.bottom()));
    let mut body = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let feed = st.feed.value.clone();
    if st.loading() && feed.as_ref().is_none_or(|f| f.items.is_empty()) {
        body.add(SkeletonRows::new(9).media());
        return;
    }
    let shown = st.shown();
    let current = st.current();
    let latest_badge = st.category.is_none();
    egui::ScrollArea::vertical()
        .id_salt("news-list-scroll")
        .auto_shrink([false, false])
        .show(&mut body, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let Some(feed) = feed.as_ref() else {
                empty(ui, st, run);
                return;
            };
            if shown.is_empty() {
                empty(ui, st, run);
                return;
            }
            for (pos, idx) in shown.iter().enumerate() {
                let Some(item) = feed.items.get(*idx) else {
                    continue;
                };
                let on = current == Some(*idx);
                let r = row(ui, cx, st, item, on, pos == 0 && latest_badge);
                // A controller lands on the article shown, else the first on screen.
                if on || (current.is_none() && pos == 0) {
                    crate::pad::primary(ui.ctx(), r.id);
                }
                if r.clicked() {
                    run.push(("news.select", json!({ "index": pos })));
                }
            }
        });
}

/// Nothing in the list (and no error, which the reader tells): say so, offer a refresh.
fn empty(ui: &mut Ui, st: &NewsState, run: &mut Vec<(&'static str, Value)>) {
    if st.feed.error.is_some() {
        return;
    }
    let loading = st.loading();
    Empty::new()
        .icon(lucide::NEWSPAPER)
        .title(t("news", "empty"))
        .compact()
        .show(ui, |ui| {
            if ui
                .add(
                    Button::new(t("news", "refresh"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(loading)
                        .enabled(!loading),
                )
                .clicked()
            {
                run.push(("news.refresh", Value::Null));
            }
        });
}

/// Text cut after `rows` lines with an ellipsis (`line-clamp-N`).
fn clamp(ui: &Ui, text: &str, font: FontId, line_h: f32, width: f32, rows: usize) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            font_id: font,
            color: Color32::PLACEHOLDER,
            line_height: Some(line_h),
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width: width.max(1.0),
        max_rows: rows,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    let g = paint::layout_top(ui, job.clone());
    if !g.elided {
        return g;
    }
    // Cut at a word as the browser does, without the space egui leaves before the `…`.
    let kept: String = g
        .rows
        .iter()
        .flat_map(|r| r.glyphs.iter().map(|x| x.chr))
        .filter(|c| *c != '…')
        .collect();
    let fmt = job
        .sections
        .first()
        .map(|x| x.format.clone())
        .unwrap_or_default();
    let mut cut = LayoutJob::single_section(format!("{}…", kept.trim_end()), fmt);
    cut.wrap = job.wrap;
    paint::layout_top(ui, cut)
}

fn row(
    ui: &mut Ui,
    cx: &Ctx,
    st: &NewsState,
    item: &Item,
    on: bool,
    latest: bool,
) -> egui::Response {
    let tk = ui.tokens();
    let w = ui.available_width();
    let a = &item.a;
    let text_x = size::PAD + THUMB.x + GAP;
    let text_w = (w - text_x - size::PAD).max(20.0);
    let title_c = if on { tk.fg } else { tk.fg_muted };
    let title = clamp(
        ui,
        &a.title,
        font(Face::SansSemibold, size::TEXT_XS),
        size::TEXT_XS * 1.375,
        text_w,
        2,
    );
    let excerpt = a.excerpt.as_deref().filter(|e| !e.is_empty()).map(|e| {
        clamp(
            ui,
            e,
            font(Face::Sans, size::TEXT_2XS),
            EXCERPT_H,
            text_w,
            1,
        )
    });
    // meta (11: the stencil's leading 1) · mt-0.5 · title · mt-0.5 · excerpt
    let meta_h = 11.0;
    let text_h = meta_h + 2.0 + title.size().y + excerpt.as_ref().map_or(0.0, |_| 2.0 + EXCERPT_H);
    let h = PAD_Y * 2.0 + text_h.max(THUMB.y) + 1.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let p = ui.painter();
    let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered() && !on);
    if on {
        p.rect_filled(rect, CornerRadius::ZERO, tk.raised);
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + 8.0),
                Pos2::new(rect.left() + 2.0, rect.bottom() - 8.0),
            ),
            CornerRadius::same(1),
            tk.accent,
        );
    } else if hover > 0.0 {
        p.rect_filled(rect, CornerRadius::ZERO, alpha(tk.raised, 0.5 * hover));
    }
    p.hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.5)),
    );
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, rect.shrink(3.0), size::RADIUS_XS, &tk);
    }

    // The thumbnail.
    let thumb = Rect::from_min_size(
        Pos2::new(rect.left() + size::PAD, rect.top() + PAD_Y),
        THUMB,
    );
    let radius = CornerRadius::same(size::RADIUS_SM);
    p.rect_filled(thumb, radius, tk.raised);
    let drawn = a
        .image_url
        .as_deref()
        .and_then(|u| st.images.want(cx.hub, u))
        .is_some_and(|path| crate::parts::image::paint(p, thumb.shrink(1.0), &path, radius));
    if !drawn {
        lucide::IMAGE.paint(
            p,
            Rect::from_center_size(thumb.center(), Vec2::splat(size::ICON)),
            alpha(tk.fg_faint, 0.5),
        );
    }
    p.rect_stroke(
        thumb,
        radius,
        Stroke::new(1.0, tk.border),
        StrokeKind::Inside,
    );

    // The meta line: LATEST · CATEGORY ………… age.
    let x0 = rect.left() + text_x;
    let right = rect.right() - size::PAD;
    let mut y = rect.top() + PAD_Y;
    let cy = y + meta_h / 2.0;
    let age = paint::galley(
        ui,
        &format::relative(item.ts),
        font(Face::Sans, size::TEXT_3XS),
        tk.fg_faint,
    );
    let age_rect = Rect::from_min_size(
        Pos2::new(right - age.size().x, cy - age.size().y / 2.0),
        age.size(),
    );
    p.galley(age_rect.min, Arc::clone(&age), tk.fg_faint);
    let mut x = x0;
    if latest {
        let g = paint::tracked(
            ui,
            t("news", "latest"),
            // The meta line's face: one family beside the category and the age.
            font(Face::SansSemibold, size::TEXT_3XS),
            tk.accent,
            0.06,
            true,
            None,
        );
        p.galley(
            Pos2::new(x, cy - g.size().y / 2.0),
            Arc::clone(&g),
            tk.accent,
        );
        x += g.size().x + 6.0;
    }
    if let Some(c) = a.category.as_deref().filter(|c| !c.is_empty()) {
        let max = (age_rect.left() - 6.0 - x).max(0.0);
        let g = paint::tracked(
            ui,
            c,
            font(Face::Sans, size::TEXT_3XS),
            tk.fg_faint,
            0.06,
            true,
            Some(max),
        );
        p.galley(Pos2::new(x, cy - g.size().y / 2.0), g, tk.fg_faint);
    }
    // `title={date(a.date)}` on the age.
    let age_hit = ui.interact(age_rect, resp.id.with("age"), Sense::hover());
    let _ = age_hit.tip(&format::date_str(&a.date));

    y += meta_h + 2.0;
    p.galley_with_override_text_color(Pos2::new(x0, y), Arc::clone(&title), title_c);
    y += title.size().y;
    if let Some(g) = excerpt {
        y += 2.0 + (EXCERPT_H - g.size().y) / 2.0;
        p.galley_with_override_text_color(Pos2::new(x0, y), g, tk.fg_faint);
    }
    resp
}
