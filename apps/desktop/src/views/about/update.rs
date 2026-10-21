//! The update card (= `UpdateCard.svelte`): the launcher's own version against the newest
//! release, and installing it, as `store::updater` walks idle → checking → up to date |
//! available | error, then downloading → done → restart.

use std::time::Duration;

use dz_hub::features::updater::UpdateKind;
use egui::{
    Align, CornerRadius, Frame, Layout, Margin, RichText, Sense, Shadow, Stroke, Ui, Vec2, vec2,
};
use serde_json::Value;

use super::{AboutState, Run, VERSION};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Meter, MeterSize, Size, Spinner, Tag, Variant, panel};
use crate::store::updater::{UpdateState, Updater};
use crate::theme::{TokensExt, font, fonts::Face, size};

pub fn show(ui: &mut Ui, cx: &mut Ctx, st: &mut AboutState, run: &mut Run) {
    let tk = ui.tokens();
    let up = &cx.stores.updater;
    let now = ui.input(|i| i.time);
    let glow = now < st.glow_until;
    if glow {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64((st.glow_until - now).max(0.0)));
    }
    let lit = paint::ease_for(ui, ui.id().with("about.update.glow"), glow, 0.3);
    let rest = if up.state == UpdateState::Available {
        alpha(tk.ok, 0.5)
    } else {
        tk.border
    };
    let shadow = if lit > 0.0 {
        let s = paint::glow(&tk);
        Shadow {
            color: alpha(s.color, lit * f32::from(s.color.a()) / 255.0),
            ..s
        }
    } else {
        Shadow::NONE
    };
    let resp = Frame::new()
        .fill(alpha(tk.bg, 0.6))
        .stroke(Stroke::new(1.0, paint::lerp(rest, tk.accent, lit)))
        .corner_radius(CornerRadius::same(size::RADIUS_MD))
        .shadow(shadow)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            header(ui, up, run);
            Frame::new()
                .inner_margin(Margin::symmetric(size::PAD as i8, 12))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    body(ui, up, run);
                });
        });
    if std::mem::take(&mut st.center_update) {
        ui.scroll_to_rect(resp.response.rect, Some(Align::Center));
    }
}

fn header(ui: &mut Ui, up: &Updater, run: &mut Run) {
    let tk = ui.tokens();
    let r = Frame::new()
        .inner_margin(Margin::symmetric(size::PAD as i8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.set_min_height(size::CONTROL_SM);
                ui.spacing_mut().item_spacing.x = 8.0;
                stencil(ui, t("about", "updates"));
                let v = format!("{} {VERSION}", t("about", "version"));
                ui.add(Tag::new(&v));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let checking = up.state == UpdateState::Checking;
                    let busy = checking || up.state == UpdateState::Downloading;
                    let b = Button::new(t("about", "checkUpdates"))
                        .icon(lucide::REFRESH_CW)
                        .spinner(checking)
                        .variant(Variant::Ghost)
                        .size(Size::Xs)
                        .enabled(!busy);
                    if ui.add(b).clicked() {
                        run.push(("updates.check", Value::Null));
                    }
                });
            });
        });
    let rect = r.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        Stroke::new(1.0, alpha(tk.border, 0.6)),
    );
}

fn body(ui: &mut Ui, up: &Updater, run: &mut Run) {
    let tk = ui.tokens();
    match up.state {
        UpdateState::Idle | UpdateState::Checking => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add(Spinner::new().size(size::ICON).color(tk.fg_faint));
                words(
                    ui,
                    t("about", "updatesChecking"),
                    Face::Sans,
                    size::TEXT_XS,
                    tk.fg_muted,
                );
            });
        }
        UpdateState::UpToDate => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                icon(ui, lucide::CIRCLE_CHECK, size::ICON, tk.ok);
                words(
                    ui,
                    t("about", "updatesUpToDate"),
                    Face::Sans,
                    size::TEXT_SM,
                    tk.ok,
                );
            });
            ui.add_space(2.0);
            let latest = tf("about", "updatesLatest", &[("version", &VERSION)]);
            words(ui, &latest, Face::Sans, size::TEXT_2XS, tk.fg_faint);
        }
        UpdateState::Available if up.info.is_some() => available(ui, up, run),
        UpdateState::Downloading | UpdateState::Available => {
            let version = up.info.as_ref().map_or("", |i| i.version.as_str());
            let label = tf("about", "updatesDownloading", &[("version", &version)]);
            words(ui, &label, Face::Sans, size::TEXT_XS, tk.fg);
            ui.add_space(8.0);
            ui.add(
                Meter::new(up.percent() as f32 / 100.0)
                    .size(MeterSize::Md)
                    .width(ui.available_width()),
            );
            ui.add_space(4.0);
            let got = if up.total > 0 {
                format!(
                    "{} / {}",
                    format::bytes(up.received as f64),
                    format::bytes(up.total as f64)
                )
            } else {
                format::bytes(up.received as f64)
            };
            ui.horizontal(|ui| {
                words(ui, &got, Face::Mono, size::TEXT_2XS, tk.fg_faint);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let pct = format!("{}%", up.percent());
                    words(ui, &pct, Face::Mono, size::TEXT_2XS, tk.fg_faint);
                });
            });
        }
        UpdateState::Done => {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let b = Button::new(t("about", "updatesRestart"))
                        .icon(lucide::ROTATE_CW)
                        .variant(Variant::Play)
                        .size(Size::Lg);
                    if ui.add(b).clicked() {
                        run.push(("updates.restart", Value::Null));
                    }
                    ui.add_space(12.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            icon(ui, lucide::PACKAGE_CHECK, size::ICON, tk.ok);
                            words(
                                ui,
                                t("about", "updatesDone"),
                                Face::Sans,
                                size::TEXT_SM,
                                tk.ok,
                            );
                        });
                        ui.add_space(2.0);
                        words(
                            ui,
                            t("about", "updatesDoneHint"),
                            Face::Sans,
                            size::TEXT_2XS,
                            tk.fg_faint,
                        );
                    });
                });
            });
        }
        UpdateState::Error => {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.add_space(0.0);
                ui.vertical(|ui| {
                    ui.add_space(2.0);
                    icon(ui, lucide::TRIANGLE_ALERT, size::ICON_SM, tk.err);
                });
                ui.add(
                    egui::Label::new(
                        RichText::new(&up.error)
                            .font(font(Face::Mono, size::TEXT_2XS))
                            .color(tk.err),
                    )
                    .wrap()
                    .selectable(true),
                );
            });
            ui.add_space(8.0);
            let b = Button::new(t("about", "updatesRetry")).icon(lucide::REFRESH_CW);
            if ui.add(b).clicked() {
                run.push(("updates.check", Value::Null));
            }
        }
    }
}

/// A new version: what it is, its notes, and the button when this copy can install it.
fn available(ui: &mut Ui, up: &Updater, run: &mut Run) {
    let tk = ui.tokens();
    let Some(info) = up.info.as_ref() else {
        return;
    };
    // A package (deb, AUR) or a dev build is told of a new version but cannot replace
    // itself: the card says why instead of offering the button.
    let in_place = up.support.as_ref().is_none_or(|s| s.supported);
    let why_not = if up
        .support
        .as_ref()
        .is_some_and(|s| s.kind == UpdateKind::Dev)
    {
        t("about", "updatesDevBuild")
    } else {
        t("about", "updatesPackageManager")
    };
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        if in_place {
            let b = Button::new(t("about", "updatesInstall"))
                .icon(lucide::DOWNLOAD)
                .variant(Variant::Play)
                .size(Size::Lg);
            if ui.add(b).clicked() {
                run.push(("updates.install", Value::Null));
            }
            ui.add_space(12.0);
        }
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.vertical(|ui| {
                ui.add_space(2.0);
                icon(ui, lucide::DOWNLOAD, size::ICON_LG, tk.ok);
            });
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    words(
                        ui,
                        t("about", "updatesAvailable"),
                        Face::Sans,
                        size::TEXT_SM,
                        tk.fg,
                    );
                    let v = paint::tracked(
                        ui,
                        &format!("v{}", info.version),
                        font(Face::DisplayBold, size::TEXT_LG),
                        tk.ok,
                        0.02,
                        true,
                        None,
                    );
                    let (r, _) = ui.allocate_exact_size(v.size(), Sense::hover());
                    ui.painter().galley(r.min, v, tk.ok);
                });
                let mut meta =
                    format!("{} v{}", t("about", "updatesCurrent"), info.current_version);
                if let Some(d) = info.date.as_deref().filter(|d| !d.is_empty()) {
                    meta.push_str(" · ");
                    meta.push_str(&tf(
                        "about",
                        "updatesReleased",
                        &[("date", &format::date_str(d))],
                    ));
                }
                words(ui, &meta, Face::Mono, size::TEXT_2XS, tk.fg_faint);
                if let Some(notes) = info.body.as_deref().filter(|b| !b.trim().is_empty()) {
                    ui.add_space(8.0);
                    Frame::new()
                        .fill(tk.bg)
                        .stroke(Stroke::new(1.0, tk.border))
                        .corner_radius(CornerRadius::same(size::RADIUS_SM))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            egui::ScrollArea::vertical()
                                .id_salt("about.update.notes")
                                .max_height(160.0 - 16.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(notes)
                                                .font(font(Face::Mono, size::TEXT_2XS))
                                                .color(tk.fg_muted),
                                        )
                                        .wrap()
                                        .selectable(true),
                                    );
                                });
                        });
                }
            });
        });
    });
    if !in_place {
        ui.add_space(8.0);
        words(ui, why_not, Face::Sans, size::TEXT_XS, tk.fg_muted);
    }
}

/// A stencilled label in the flow (`label-stencil text-fg-muted`).
pub fn stencil(ui: &mut Ui, text: &str) {
    let tk = ui.tokens();
    let g = panel::stencil(ui, text, tk.fg_muted, None);
    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
    ui.painter().galley(r.min, g, tk.fg_muted);
}

/// Words that wrap.
pub fn words(ui: &mut Ui, s: &str, face: Face, px: f32, color: egui::Color32) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(s).font(font(face, px)).color(color)).wrap())
}

/// An icon, `px` square, in the flow.
pub fn icon(ui: &mut Ui, icon: Icon, px: f32, color: egui::Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(px, px), Sense::hover());
    icon.paint(ui.painter(), r, color);
    resp
}
