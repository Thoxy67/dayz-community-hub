//! The article being read: the hero picture (or a band of contour lines), the category,
//! the title, the date, reading time and author, then the excerpt and the body (fetched
//! when the article is picked: a spinner until it is in, a retry if it fails) and an
//! update's patch notes, one platform at a time. Each article fades in as it is picked
//! and starts at the top.

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    Color32, CornerRadius, Mesh, Pos2, Rect, Sense, Stroke, TextFormat, Ui, Vec2, epaint::Vertex,
};
use serde_json::{Value, json};

use super::{Body, Item, NewsState};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Size, Spinner, Tag, Tone, Variant, topo};
use crate::parts::{Empty, HtmlAction, HtmlDoc, SafeHtml};
use crate::theme::{TokensExt, font, fonts::Face, motion, size};

/// `max-w-3xl`, `px-8`.
const COLUMN: f32 = 768.0;
const SIDE: f32 = 32.0;
/// `h-72` with a picture, `h-24` without; the title climbs `-mt-24` onto the picture.
const HERO_H: f32 = 288.0;
const BAND_H: f32 = 96.0;
const OVERLAP: f32 = 96.0;

pub fn show(ui: &mut Ui, cx: &Ctx, st: &mut NewsState, run: &mut Vec<(&'static str, Value)>) {
    let current = st.current();
    let feed = st.feed.value.clone();
    let item = current.and_then(|i| feed.as_ref()?.items.get(i));
    if let Some(err) = st
        .feed
        .error
        .clone()
        .filter(|_| feed.as_ref().is_none_or(|f| f.items.is_empty()))
    {
        failed(ui, st, &err, run);
        return;
    }
    let Some(item) = item else {
        if !st.loading() {
            Empty::new()
                .icon(lucide::NEWSPAPER)
                .title(t("news", "selectArticle"))
                .show(ui, |_| {});
        }
        return;
    };

    // A new article: from the top, fading in (`animate-fade-in`, 120 ms).
    let now = ui.input(|i| i.time);
    let fresh = st.shown_slug.as_deref() != Some(item.a.slug.as_str());
    if fresh {
        st.shown_slug = Some(item.a.slug.clone());
        st.shown_at = now;
    }
    let p = if crate::app::reduced_motion(ui.ctx()) {
        1.0
    } else {
        ((now - st.shown_at) as f32 / motion::FAST).clamp(0.0, 1.0)
    };
    if p < 1.0 {
        ui.ctx().request_repaint();
    }
    let mut area = egui::ScrollArea::vertical()
        .id_salt("news-reader-scroll")
        .auto_shrink([false, false]);
    if fresh {
        area = area.vertical_scroll_offset(0.0);
    }
    area.show(ui, |ui| {
        ui.multiply_opacity(1.0 - (1.0 - p).powi(3));
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        if let Some(i) = current {
            article(ui, cx, st, i, item, run);
        }
        // What a controller's right stick scrolls.
        crate::pad::reading_pane(ui);
    });
}

/// The load failed and there is nothing to read.
fn failed(ui: &mut Ui, st: &NewsState, err: &str, run: &mut Vec<(&'static str, Value)>) {
    let blocked = st.blocked();
    let hint = if blocked {
        t("news", "blockedHint")
    } else {
        t("news", "loadFailedHint")
    };
    Empty::new()
        .icon(lucide::CLOUD_OFF)
        .title(t("news", "loadFailedTitle"))
        .text(hint)
        .detail(err)
        .show(ui, |ui| {
            if blocked
                && ui
                    .add(
                        Button::new(t("news", "openSite"))
                            .icon(lucide::EXTERNAL_LINK)
                            .variant(Variant::Accent),
                    )
                    .clicked()
            {
                run.push(("news.open_site", Value::Null));
            }
            let retry = Button::new(t("news", "retry"))
                .icon(lucide::REFRESH_CW)
                .spinner(st.loading())
                .enabled(!st.loading())
                .variant(if blocked {
                    Variant::Default
                } else {
                    Variant::Accent
                });
            if ui.add(retry).clicked() {
                run.push(("news.refresh", Value::Null));
            }
        });
}

fn article(
    ui: &mut Ui,
    cx: &Ctx,
    st: &NewsState,
    idx: usize,
    item: &Item,
    run: &mut Vec<(&'static str, Value)>,
) {
    let tk = ui.tokens();
    let a = &item.a;
    let w = ui.available_width();
    let top = ui.cursor().top();
    let left = ui.max_rect().left();

    // The hero, once its picture is in; else the contour band.
    let hero = a
        .image_url
        .as_deref()
        .filter(|u| !u.is_empty())
        .and_then(|u| Some((u, st.images.texture(ui.ctx(), cx.hub, u)?)));
    let full_w = Rect::from_min_size(Pos2::new(left, top), Vec2::new(w, 0.0));
    if let Some((url, tex)) = &hero {
        let r = Rect::from_min_size(full_w.min, Vec2::new(w, HERO_H));
        let [tw, th] = tex.size().map(|v| v.max(1) as f32);
        let scale = (r.width() / tw).max(r.height() / th);
        let uv = Rect::from_center_size(
            Pos2::new(0.5, 0.5),
            Vec2::new(r.width() / scale / tw, r.height() / scale / th),
        );
        ui.painter().add(
            egui::epaint::RectShape::filled(r, CornerRadius::ZERO, Color32::WHITE)
                .with_texture(tex.id(), uv),
        );
        // `bg-linear-to-t from-panel via-panel/60 to-transparent`.
        gradient(
            ui.painter(),
            r,
            &[
                (0.0, alpha(tk.panel, 0.0)),
                (0.5, alpha(tk.panel, 0.6)),
                (1.0, tk.panel),
            ],
        );
        let hit = ui
            .interact(r, ui.scope_id().with("hero"), Sense::click())
            .on_hover_cursor(egui::CursorIcon::ZoomIn);
        if paint::keyboard_focus(ui, &hit) {
            paint::focus_ring(ui.painter(), r.shrink(3.0), size::RADIUS_SM, &tk);
        }
        let hit = crate::kit::TipExt::tip(hit, t("news", "zoomImage"));
        if hit.clicked() {
            run.push(("news.zoom_image", json!({ "src": url, "original": url })));
        }
        ui.add_space(HERO_H - OVERLAP);
    } else {
        let r = Rect::from_min_size(full_w.min, Vec2::new(w, BAND_H));
        topo::paint(&ui.painter().with_clip_rect(r), r, &tk, 0.7);
        ui.painter()
            .hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, tk.border));
        ui.add_space(BAND_H + 16.0);
    }

    // The column: `mx-auto max-w-3xl px-8`.
    let col_w = w.min(COLUMN);
    let inset = (w - col_w) / 2.0 + SIDE;
    let text_w = (col_w - 2.0 * SIDE).max(80.0);
    ui.horizontal_top(|ui| {
        ui.add_space(inset);
        ui.vertical(|ui| {
            ui.set_width(text_w);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let body_in = st.body(idx).and_then(|f| f.value.clone());
            head(ui, item, body_in.as_ref().map(|b| b.minutes), text_w, run);
            ui.add_space(20.0);
            body(ui, cx, st, idx, item, text_w, run);
            ui.add_space(48.0);
        });
    });
}

/// Category, title, the meta row (the reading time once the body is in).
fn head(
    ui: &mut Ui,
    item: &Item,
    minutes: Option<usize>,
    w: f32,
    run: &mut Vec<(&'static str, Value)>,
) {
    let tk = ui.tokens();
    let a = &item.a;
    if let Some(c) = a.category.as_deref().filter(|c| !c.is_empty()) {
        ui.add(Tag::new(c).tone(Tone::Accent));
        ui.add_space(8.0);
    }
    // `title-display text-3xl leading-[0.95]`.
    let s = size::TEXT_3XL;
    let mut job = LayoutJob::single_section(
        a.title.to_uppercase(),
        TextFormat {
            font_id: font(Face::DisplayBold, s),
            color: tk.fg,
            extra_letter_spacing: s * 0.02,
            line_height: Some(s * 0.95),
            ..Default::default()
        },
    );
    job.wrap = TextWrapping {
        max_width: w,
        ..Default::default()
    };
    let g = paint::layout_top(ui, job);
    // egui sets the capitals lower in a 0.95 line than the browser does: drawn 7px up,
    // the block 4px shorter, the meta row lands where the reference has it.
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, (g.size().y - 4.0).max(0.0)), Sense::hover());
    ui.painter().galley(r.min - Vec2::new(0.0, 7.0), g, tk.fg);
    ui.add_space(12.0);

    // `flex flex-wrap gap-x-4 gap-y-1 text-2xs text-fg-muted`, the button at the end.
    let date = format::date_str(&a.date);
    let read = minutes.map(|m| tf("news", "readTime", &[("minutes", &m)]));
    let by = a
        .author
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|au| tf("news", "by", &[("author", &au)]));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(16.0, 4.0);
        meta(ui, lucide::CALENDAR_DAYS, &date);
        if let Some(r) = &read {
            meta(ui, lucide::CLOCK, r);
        }
        if let Some(b) = &by {
            meta(ui, lucide::PEN_LINE, b);
        }
        let open = || {
            Button::new(t("news", "openBrowser"))
                .icon(lucide::EXTERNAL_LINK)
                .size(Size::Xs)
                .variant(Variant::Ghost)
        };
        // Its width is last frame's: with too little room left on the line, the button
        // takes the next one instead of being drawn over the author.
        let wid = ui.scope_id().with("open-w");
        let bw: f32 = ui.data(|d| d.get_temp(wid)).unwrap_or(0.0);
        if ui.available_size_before_wrap().x < bw {
            ui.end_row();
        }
        let drawn = ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = ui.add(open());
                if r.clicked() {
                    run.push(("news.open_article", Value::Null));
                }
                r.rect.width()
            })
            .inner;
        if (drawn - bw).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(wid, drawn));
            ui.ctx().request_discard("news open button width");
        }
    });
}

fn meta(ui: &mut Ui, icon: Icon, text: &str) {
    let tk = ui.tokens();
    let g = paint::galley(ui, text, font(Face::Sans, size::TEXT_2XS), tk.fg_muted);
    let h = size::CONTROL_SM;
    let (r, _) = ui.allocate_exact_size(Vec2::new(14.0 + 4.0 + g.size().x, h), Sense::hover());
    icon.paint(
        ui.painter(),
        Rect::from_center_size(Pos2::new(r.left() + 7.0, r.center().y), Vec2::splat(14.0)),
        tk.fg_muted,
    );
    paint::text_at(ui.painter(), &g, r.left() + 18.0, r.center().y, tk.fg_muted);
}

/// The excerpt as a lead, then the article.
fn body(
    ui: &mut Ui,
    cx: &Ctx,
    st: &NewsState,
    idx: usize,
    item: &Item,
    w: f32,
    run: &mut Vec<(&'static str, Value)>,
) {
    let tk = ui.tokens();
    let a = &item.a;
    if let Some(ex) = a.excerpt.as_deref().filter(|e| !e.is_empty()) {
        // `border-l-2 border-accent pl-3 text-sm leading-relaxed italic`.
        let mut job = LayoutJob::single_section(
            ex.to_owned(),
            TextFormat {
                font_id: font(Face::Sans, size::TEXT_SM),
                color: tk.fg_muted,
                italics: true,
                line_height: Some(size::TEXT_SM * 1.625),
                ..Default::default()
            },
        );
        job.wrap = TextWrapping {
            max_width: (w - 14.0).max(20.0),
            ..Default::default()
        };
        let g = paint::layout_top(ui, job);
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
        ui.painter().rect_filled(
            Rect::from_min_size(r.min, Vec2::new(2.0, r.height())),
            CornerRadius::ZERO,
            tk.accent,
        );
        ui.painter()
            .galley(Pos2::new(r.left() + 14.0, r.top()), g, tk.fg_muted);
        ui.add_space(20.0);
    }
    let fetch = st.body(idx);
    match fetch.and_then(|f| f.value.clone()) {
        Some(b) => {
            match &b.doc {
                Some(doc) => html(ui, cx, st, doc, run),
                None => {
                    ui.label(
                        egui::RichText::new(t("news", "noContent"))
                            .font(font(Face::Sans, size::TEXT_SM))
                            .color(tk.fg_muted),
                    );
                }
            }
            notes(ui, cx, st, &b, run);
        }
        None => match fetch.and_then(|f| f.error.clone()) {
            Some(err) => failed_body(ui, &err, fetch.is_some_and(|f| f.loading()), run),
            None => {
                // Asked for, or about to be.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.add(Spinner::new().size(14.0).color(tk.fg_muted));
                    ui.label(
                        egui::RichText::new(t("news", "loadingArticle"))
                            .font(font(Face::Sans, size::TEXT_SM))
                            .color(tk.fg_muted),
                    );
                });
                crate::kit::paint::animate(ui.ctx());
            }
        },
    }
}

/// An article's HTML, its links and pictures answered.
fn html(
    ui: &mut Ui,
    cx: &Ctx,
    st: &NewsState,
    doc: &HtmlDoc,
    run: &mut Vec<(&'static str, Value)>,
) {
    let (ctx, hub, images) = (ui.ctx().clone(), cx.hub, st.images.clone());
    let action = SafeHtml::new(doc).zoomable().show(ui, |src| {
        let tex = images.texture(&ctx, hub, src)?;
        Some(egui::ImageSource::Texture(tex.sized()))
    });
    match action {
        Some(HtmlAction::Open(url)) => run.push(("news.open_link", json!({ "url": url }))),
        Some(HtmlAction::Image { shown, original }) => {
            // The original is asked for now, to swap in once it is here.
            let _ = st.images.want(cx.hub, &original);
            run.push((
                "news.zoom_image",
                json!({ "src": shown, "original": original }),
            ));
        }
        None => {}
    }
}

/// The body could not be fetched: why, and a retry.
fn failed_body(ui: &mut Ui, err: &str, loading: bool, run: &mut Vec<(&'static str, Value)>) {
    let tk = ui.tokens();
    ui.label(
        egui::RichText::new(t("news", "articleFailed"))
            .font(font(Face::Sans, size::TEXT_SM))
            .color(tk.fg),
    );
    ui.add_space(4.0);
    ui.add(
        egui::Label::new(
            egui::RichText::new(err)
                .font(font(Face::Mono, size::TEXT_2XS))
                .color(tk.fg_faint),
        )
        .wrap(),
    );
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let retry = Button::new(t("news", "retry"))
            .icon(lucide::REFRESH_CW)
            .size(Size::Sm)
            .spinner(loading)
            .enabled(!loading)
            .variant(Variant::Accent);
        if ui.add(retry).clicked() {
            run.push(("news.retry_article", Value::Null));
        }
        let open = Button::new(t("news", "openBrowser"))
            .icon(lucide::EXTERNAL_LINK)
            .size(Size::Sm);
        if ui.add(open).clicked() {
            run.push(("news.open_article", Value::Null));
        }
    });
}

/// An update's patch notes: a chip per platform, the chosen one's notes under them.
fn notes(ui: &mut Ui, cx: &Ctx, st: &NewsState, b: &Body, run: &mut Vec<(&'static str, Value)>) {
    if b.notes.is_empty() {
        return;
    }
    let tk = ui.tokens();
    let chosen = st
        .platform
        .as_deref()
        .and_then(|p| b.notes.iter().position(|(n, _)| n == p))
        .or_else(|| b.notes.iter().position(|(n, _)| n.contains("Steam")))
        .unwrap_or(0);
    ui.add_space(40.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, tk.border));
    ui.add_space(24.0);
    let s = size::TEXT_XL;
    ui.label(
        egui::RichText::new(t("news", "patchNotes").to_uppercase())
            .font(font(Face::DisplayBold, s))
            .extra_letter_spacing(s * 0.02)
            .color(tk.fg),
    );
    ui.add_space(10.0);
    if b.notes.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for (i, (platform, _)) in b.notes.iter().enumerate() {
                if ui.add(Button::chip(platform, i == chosen)).clicked() {
                    run.push(("news.patch_notes", json!({ "platform": platform })));
                }
            }
        });
        ui.add_space(16.0);
    }
    if let Some((_, doc)) = b.notes.get(chosen) {
        html(ui, cx, st, doc, run);
    }
}

/// A vertical gradient over `r`: `stops` from the top (0) to the bottom (1).
fn gradient(painter: &egui::Painter, r: Rect, stops: &[(f32, Color32)]) {
    let mut mesh = Mesh::default();
    for (i, (at, c)) in stops.iter().enumerate() {
        let y = r.top() + r.height() * at.clamp(0.0, 1.0);
        let base = mesh.vertices.len() as u32;
        for x in [r.left(), r.right()] {
            mesh.vertices.push(Vertex {
                pos: Pos2::new(x, y),
                uv: egui::epaint::WHITE_UV,
                color: *c,
            });
        }
        if i > 0 {
            let prev = base - 2;
            mesh.add_triangle(prev, prev + 1, base);
            mesh.add_triangle(prev + 1, base + 1, base);
        }
    }
    painter.add(mesh);
}
