//! The guide, one subject at a time instead of a wall of text, and what the window is built
//! with, quietly at the end.

use egui::{
    Align, CornerRadius, Frame, Layout, Margin, Rect, RichText, Sense, Shape, Stroke, StrokeKind,
    Ui, UiBuilder, Vec2, pos2, vec2,
};

use super::update::{icon, stencil, words};
use super::{AboutState, EXAMPLES, Guide, Run, STACK};
use crate::i18n::{fill, t};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Copy, FieldSize, Tab, TabStrip};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// Two columns from this window width (`lg:grid-cols-…`).
const LG: f32 = 1024.0;

pub fn show(ui: &mut Ui, st: &mut AboutState) {
    let tk = ui.tokens();
    let slot = ui.painter().add(Shape::Noop);
    let top = ui.cursor().min;
    let w = ui.available_width();
    let inner = (w - 2.0 * size::PAD).max(0.0);
    let r = ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        let head = Frame::new()
            .inner_margin(Margin {
                left: size::PAD as i8,
                right: size::PAD as i8,
                top: 4,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(inner);
                ui.spacing_mut().interact_size.y = 0.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    icon(ui, lucide::BOOK_OPEN, size::ICON_SM, tk.accent);
                    stencil(ui, t("about", "guide"));
                    ui.add_space(8.0);
                    let tabs = [
                        Tab::new(Guide::Start, t("about", "quickstart")).icon(lucide::LIST_CHECKS),
                        Tab::new(Guide::Mods, t("about", "modWorkflow")).icon(lucide::PUZZLE),
                        Tab::new(Guide::Share, t("about", "sharing")).icon(lucide::SHARE_2),
                        Tab::new(Guide::Offline, t("about", "srcOffline")).icon(lucide::WIFI_OFF),
                        Tab::new(Guide::Tips, t("about", "tips")).icon(lucide::LIGHTBULB),
                    ];
                    ui.add(TabStrip::new("about.guide", &mut st.guide, &tabs).size(FieldSize::Xs));
                });
            })
            .response
            .rect;
        Frame::new()
            .inner_margin(Margin::symmetric(size::PAD as i8, 12))
            .show(ui, |ui| {
                ui.set_width(inner);
                body(ui, st.guide);
            });
        head
    });
    let head = r.inner;
    let rect = Rect::from_min_size(top, vec2(w, r.response.rect.height()));
    let radius = CornerRadius::same(size::RADIUS_MD);
    ui.painter().set(
        slot,
        Shape::Vec(vec![
            Shape::rect_filled(rect, radius, alpha(tk.bg, 0.6)),
            Shape::hline(
                rect.x_range(),
                head.bottom() - 0.5,
                Stroke::new(1.0, alpha(tk.border, 0.6)),
            ),
            Shape::rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, tk.border),
                StrokeKind::Inside,
            ),
        ]),
    );
}

fn body(ui: &mut Ui, guide: Guide) {
    let tk = ui.tokens();
    let a = |k: &str| t("about", k);
    match guide {
        Guide::Start => {
            let list: Vec<String> = [1, 2, 3]
                .iter()
                .map(|i| {
                    format!(
                        "{} — {}",
                        a(&format!("qsStep{i}Title")),
                        a(&format!("qsStep{i}Body"))
                    )
                })
                .collect();
            steps(ui, &list.iter().map(String::as_str).collect::<Vec<_>>());
        }
        Guide::Mods => {
            let desc = fill(a("steamcmdDesc"), &[("notFound", &a("steamcmdNotFound"))]);
            let job = rich(
                &[
                    (a("steamcmdWhat"), Face::SansMedium, tk.fg),
                    (" ", Face::Sans, tk.fg_muted),
                    (&desc, Face::Sans, tk.fg_muted),
                ],
                size::TEXT_XS,
            );
            ui.add(egui::Label::new(job).wrap());
            ui.add_space(8.0);
            steps(ui, &[a("modStep1"), a("modStep2"), a("modStep3")]);
            ui.add_space(12.0);
            words(
                ui,
                a("modDownloaderTip"),
                Face::Sans,
                size::TEXT_2XS,
                tk.fg_faint,
            );
        }
        Guide::Offline => steps(ui, &[a("offStep1"), a("offStep2"), a("offStep3")]),
        Guide::Share => columns(ui, share_left, share_right),
        Guide::Tips => columns(
            ui,
            |ui| {
                let tk = ui.tokens();
                words(
                    ui,
                    a("tipAuthTitle"),
                    Face::SansMedium,
                    size::TEXT_XS,
                    tk.fg,
                );
                ui.add_space(6.0);
                steps(
                    ui,
                    &[
                        a("tipAuthStep1"),
                        a("tipAuthStep2"),
                        a("tipAuthStep3"),
                        a("tipAuthStep4"),
                    ],
                );
            },
            |ui| {
                let tk = ui.tokens();
                words(
                    ui,
                    a("tipPerfTitle"),
                    Face::SansMedium,
                    size::TEXT_XS,
                    tk.fg,
                );
                ui.add_space(4.0);
                words(ui, a("tipPerfDesc"), Face::Sans, size::TEXT_XS, tk.fg_muted);
            },
        ),
    }
}

fn share_left(ui: &mut Ui) {
    let tk = ui.tokens();
    let a = |k: &str| t("about", k);
    let desc = fill(a("sharingDesc"), &[("url", &"dzch://"), ("file", &".dzch")]);
    words(ui, &desc, Face::Sans, size::TEXT_XS, tk.fg_muted);
    ui.add_space(10.0);
    Frame::new()
        .fill(tk.bg)
        .stroke(Stroke::new(1.0, tk.border))
        .corner_radius(CornerRadius::same(size::RADIUS_SM))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            for (key, url) in EXAMPLES {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.allocate_ui_with_layout(
                        vec2(96.0, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(96.0);
                            words(ui, a(key), Face::Sans, size::TEXT_2XS, tk.fg_faint);
                        },
                    );
                    // Wrapped, not cut: the end of a long link is the part worth reading.
                    ui.add(
                        Copy::new(url)
                            .wrap()
                            .max_width(ui.available_width())
                            .labels(t("common", "copy"), t("common", "copied"))
                            .color(ui.tokens().fg_muted),
                    );
                });
            }
        });
    ui.add_space(8.0);
    for (term, key) in [
        ("qport", "sharingParamQport"),
        ("name", "sharingParamName"),
        ("password", "sharingParamPassword"),
        ("mods", "sharingParamMods"),
    ] {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.allocate_ui_with_layout(vec2(88.0, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(88.0);
                words(ui, term, Face::Mono, size::TEXT_2XS, tk.accent);
            });
            words(ui, a(key), Face::Sans, size::TEXT_2XS, tk.fg_muted);
        });
        ui.add_space(4.0);
    }
}

fn share_right(ui: &mut Ui) {
    let tk = ui.tokens();
    let a = |k: &str| t("about", k);
    words(
        ui,
        a("sharingFiles"),
        Face::SansMedium,
        size::TEXT_2XS,
        tk.fg,
    );
    ui.add_space(6.0);
    let files = fill(
        a("sharingFilesDesc"),
        &[("button", &a("sharingFilesButton"))],
    );
    words(ui, &files, Face::Sans, size::TEXT_2XS, tk.fg_muted);
    ui.add_space(12.0);
    words(
        ui,
        a("sharingFromDc"),
        Face::SansMedium,
        size::TEXT_2XS,
        tk.fg,
    );
    ui.add_space(6.0);
    steps(
        ui,
        &[
            a("sharingDcStep1"),
            a("sharingDcStep2"),
            a("sharingDcStep3"),
        ],
    );
}

/// Two equal columns from `lg` up (`gap-4`), one under the other below.
fn columns(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let w = ui.available_width();
    if ui.ctx().content_rect().width() < LG {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            left(ui);
            ui.add_space(16.0);
            right(ui);
        });
        return;
    }
    let cw = ((w - 16.0) / 2.0).max(0.0);
    let origin = ui.cursor().min;
    let mut h = 0.0_f32;
    for (i, f) in [
        Box::new(left) as Box<dyn FnOnce(&mut Ui)>,
        Box::new(right) as Box<dyn FnOnce(&mut Ui)>,
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            pos2(origin.x + i as f32 * (cw + 16.0), origin.y),
            vec2(cw, f32::INFINITY),
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min))
                .id_salt(("about.col", i)),
        );
        child.set_width(cw);
        child.spacing_mut().item_spacing = Vec2::ZERO;
        f(&mut child);
        h = h.max(child.min_rect().height());
    }
    ui.allocate_rect(Rect::from_min_size(origin, vec2(w, h)), Sense::hover());
}

/// Numbered steps: a circled number (`size-5 rounded-full border-accent/50`), then the
/// words (`text-xs leading-relaxed text-fg-muted`).
fn steps(ui: &mut Ui, list: &[&str]) {
    let tk = ui.tokens();
    for (i, text) in list.iter().enumerate() {
        if i > 0 {
            ui.add_space(8.0);
        }
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
            let p = ui.painter();
            p.circle_stroke(r.center(), 9.5, Stroke::new(1.0, alpha(tk.accent, 0.5)));
            let n = paint::galley(
                ui,
                &(i + 1).to_string(),
                font(Face::Mono, size::TEXT_3XS),
                tk.accent,
            );
            paint::text_center(p, &n, r, tk.accent);
            ui.vertical(|ui| {
                ui.add_space(2.0);
                let job = rich(&[(text, Face::Sans, tk.fg_muted)], size::TEXT_XS);
                ui.add(egui::Label::new(job).wrap());
            });
        });
    }
}

/// Words in several faces on lines `leading-relaxed` (1.625) apart.
fn rich(parts: &[(&str, Face, egui::Color32)], px: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (text, face, color) in parts {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id: font(*face, px),
                color: *color,
                line_height: Some(px * 1.625),
                ..Default::default()
            },
        );
    }
    job
}

/// "Built with" and the stack's chips; the licence at the end.
pub fn footer(ui: &mut Ui, run: &mut Run) {
    let tk = ui.tokens();
    let w = ui.available_width();
    ui.horizontal_wrapped(|ui| {
        ui.set_width(w);
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        icon(ui, lucide::LAYERS, 14.0, tk.accent);
        words(
            ui,
            t("about", "builtWith"),
            Face::Sans,
            size::TEXT_2XS,
            tk.fg_faint,
        );
        ui.add_space(4.0);
        for (i, (name, _)) in STACK.iter().enumerate() {
            let g = paint::galley(ui, name, font(Face::Mono, size::TEXT_3XS), tk.fg_muted);
            let (r, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 12.0, g.size().y + 4.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, true, *name));
            let hover = paint::ease(ui, resp.id.with("h"), resp.hovered());
            paint::plate(
                ui.painter(),
                r,
                size::RADIUS_SM,
                alpha(tk.panel, 0.7),
                paint::lerp(tk.border, tk.border_strong, hover),
            );
            paint::text_center(ui.painter(), &g, r, paint::lerp(tk.fg_muted, tk.fg, hover));
            if paint::keyboard_focus(ui, &resp) {
                paint::focus_ring(ui.painter(), r, size::RADIUS_SM, &tk);
            }
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                run.push(("about.open_stack_link", serde_json::json!({ "index": i })));
            }
        }
        let end = format!(
            "{} · {} · {}",
            t("about", "openSource"),
            t("about", "licenseMit"),
            t("about", "forgejo")
        );
        let g = paint::galley(ui, &end, font(Face::Sans, size::TEXT_2XS), tk.fg_faint);
        let room = ui.available_size_before_wrap().x;
        if room > g.size().x {
            ui.add_space(room - g.size().x - 6.0);
        }
        ui.label(
            RichText::new(end)
                .font(font(Face::Sans, size::TEXT_2XS))
                .color(tk.fg_faint),
        );
    });
    ui.add_space(8.0);
}
