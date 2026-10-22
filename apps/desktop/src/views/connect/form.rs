//! The Connection panel: address, password, the ports folded away, Query and Connect.

use egui::{Id, Key, Rect, Response, Ui, Vec2};

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint;
use crate::kit::{
    Button, Disclosure, IconButton, Input, InputKind, Panel, Size, Spinner, TipExt, Variant,
};
use crate::store::direct::Resolved;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What the form asked for this frame, done once it is drawn.
#[derive(Default)]
struct Asked {
    query: bool,
    join: bool,
}

/// Enter (not Shift+Enter) left the field: join.
fn entered(ui: &Ui, r: &Response) -> bool {
    r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
}

/// `text-2xs font-medium text-fg`, then a faint tail. The form's own hint (`title`) shows
/// over its labels.
fn label(ui: &mut Ui, text: &str, faint: &str) -> Response {
    let tk = ui.tokens();
    let mut job = egui::text::LayoutJob::default();
    let f = font(Face::SansMedium, size::TEXT_2XS);
    job.append(text, 0.0, egui::TextFormat::simple(f, tk.fg));
    if !faint.is_empty() {
        job.append(
            &format!(" {faint}"),
            0.0,
            egui::TextFormat::simple(font(Face::Sans, size::TEXT_2XS), tk.fg_faint),
        );
    }
    let g = ui.painter().layout_job(job);
    let (r, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), egui::Sense::hover());
    ui.painter().galley(
        egui::pos2(r.left(), r.center().y - g.size().y / 2.0),
        g,
        tk.fg,
    );
    resp.tip(t("connect", "intro"))
}

/// A small line under a field (`text-3xs`).
fn note(ui: &mut Ui, text: &str, color: egui::Color32, icon: Option<crate::icons::Icon>) {
    let w = ui.available_width();
    let x0 = if icon.is_some() { 16.0 } else { 0.0 };
    let g = paint::galley_wrap(ui, text, font(Face::Sans, size::TEXT_3XS), color, w - x0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y.max(12.0)), egui::Sense::hover());
    if let Some(i) = icon {
        i.paint(
            ui.painter(),
            Rect::from_center_size(egui::pos2(r.left() + 6.0, r.center().y), Vec2::splat(12.0)),
            color,
        );
    }
    ui.painter()
        .galley(egui::pos2(r.left() + x0, r.top()), g, color);
}

/// Keep digits only (a `type="number"` field).
fn digits(s: &mut String) {
    s.retain(|c| c.is_ascii_digit());
}

pub fn show(ui: &mut Ui, cx: &mut Ctx, r: &Resolved) {
    let mut asked = Asked::default();
    let is_fav = r.valid && r.favorite.is_some();
    let has_address = !cx.stores.direct.address.is_empty();
    let has_profile = cx.stores.profile.data.is_some();
    let d = &mut cx.stores.direct;
    let (mut clear, mut favorite) = (false, false);
    Panel::new("connect-form")
        .title(t("connect", "connection"))
        .no_scroll()
        .show_with(
            ui,
            |ui| {
                // Right to left: the eraser at the end, the star before it.
                clear = ui
                    .add(
                        IconButton::new(lucide::ERASER, t("common", "clear"))
                            .size(Size::IconXs)
                            .enabled(has_address),
                    )
                    .clicked();
                favorite = ui
                    .add(
                        IconButton::new(
                            lucide::STAR,
                            if is_fav {
                                t("connect", "alreadyFavorite")
                            } else {
                                t("connect", "addFavorite")
                            },
                        )
                        .size(Size::IconXs)
                        .active(is_fav)
                        .enabled(r.valid),
                    )
                    .clicked();
            },
            |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 12,
                        top: 10,
                        bottom: 12,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                        let tk = ui.tokens();

                        // The address.
                        let _ = label(ui, t("connect", "ipHostname"), "");
                        let addr = ui
                            .add(
                                Input::new(&mut d.address)
                                    .kind(InputKind::Mono)
                                    .placeholder(t("connect", "ipPlaceholder"))
                                    .error(d.touched && r.address_error)
                                    .id_salt("connect-address"),
                            )
                            .tip(t("connect", "addressHint"));
                        if addr.lost_focus() {
                            d.split_address();
                        }
                        asked.join |= entered(ui, &addr);
                        if d.touched && r.address_error {
                            note(ui, t("connect", "addressMissing"), tk.err, None);
                        }
                        ui.add_space(3.0);

                        // The password, with its eye.
                        let _ = label(
                            ui,
                            t("connect", "password"),
                            &format!("({})", t("connect", "optional")),
                        );
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            let w = ui.available_width() - size::CONTROL - 4.0;
                            let pw = ui.add(
                                Input::new(&mut d.password)
                                    .kind(if d.show_password {
                                        InputKind::Text
                                    } else {
                                        InputKind::Password
                                    })
                                    .placeholder(t("connect", "passwordPlaceholder"))
                                    .width(w)
                                    .id_salt("connect-password"),
                            );
                            if pw.changed() {
                                d.password_from_favorite = false;
                            }
                            asked.join |= entered(ui, &pw);
                            let (icon, tip) = if d.show_password {
                                (lucide::EYE_OFF, t("connect", "hidePassword"))
                            } else {
                                (lucide::EYE, t("connect", "showPassword"))
                            };
                            if ui
                                .add(IconButton::new(icon, tip).size(Size::Icon))
                                .clicked()
                            {
                                d.show_password = !d.show_password;
                            }
                        });
                        if d.password_from_favorite && !d.password.is_empty() {
                            note(
                                ui,
                                t("connect", "savedPassword"),
                                tk.warn,
                                Some(lucide::KEY_ROUND),
                            );
                        }
                        ui.add_space(3.0);

                        // The ports, shut unless one is wrong.
                        let ports = match (d.query_port.is_empty(), d.port.is_empty()) {
                            (false, _) => format!("{} / {}", d.port, d.query_port),
                            (true, false) => d.port.clone(),
                            (true, true) => "—".to_string(),
                        };
                        let summary = tf("connect", "portsSummary", &[("ports", &ports)]);
                        let did = Id::new("connect-ports");
                        let open = d.ports_open;
                        ui.data_mut(|m| m.insert_persisted(did, open));
                        let full = ui.available_width() + 24.0;
                        let mut wide =
                            ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                                ui.cursor().min - Vec2::new(12.0, 0.0),
                                Vec2::new(full, 400.0),
                            )));
                        wide.spacing_mut().item_spacing = Vec2::ZERO;
                        Disclosure::new("connect-ports", &summary).show(&mut wide, |ui| {
                            egui::Frame::new()
                                .inner_margin(egui::Margin {
                                    left: 12,
                                    right: 12,
                                    top: 4,
                                    bottom: 6,
                                })
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 4.0);
                                    let half = (ui.available_width() - 8.0) / 2.0;
                                    ui.horizontal_top(|ui| {
                                        ui.vertical(|ui| {
                                            ui.set_width(half);
                                            ui.spacing_mut().item_spacing.y = 4.0;
                                            let _ = label(
                                                ui,
                                                t("connect", "port"),
                                                t("connect", "portGame"),
                                            );
                                            let p = ui.add(
                                                Input::new(&mut d.port)
                                                    .kind(InputKind::Mono)
                                                    .char_limit(5)
                                                    .error(r.port_error)
                                                    .id_salt("connect-port"),
                                            );
                                            if p.changed() {
                                                digits(&mut d.port);
                                            }
                                            asked.join |= entered(ui, &p);
                                        });
                                        ui.vertical(|ui| {
                                            ui.set_width(half);
                                            ui.spacing_mut().item_spacing.y = 4.0;
                                            let _ =
                                                label(ui, t("connect", "queryPortOptional"), "");
                                            let q = ui.add(
                                                Input::new(&mut d.query_port)
                                                    .kind(InputKind::Mono)
                                                    .char_limit(5)
                                                    .placeholder("27016")
                                                    .id_salt("connect-query-port"),
                                            );
                                            if q.changed() {
                                                digits(&mut d.query_port);
                                            }
                                            asked.join |= entered(ui, &q);
                                        });
                                    });
                                    if r.port_error {
                                        note(ui, t("connect", "portInvalid"), tk.err, None);
                                    } else {
                                        note(ui, t("connect", "queryPortHint"), tk.fg_faint, None);
                                    }
                                });
                        });
                        let used = wide.min_rect().height();
                        d.ports_open = ui.data_mut(|m| m.get_persisted(did)).unwrap_or(open);
                        ui.add_space(used + 7.0);

                        // Query, Connect.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let w = ui.available_width() - 6.0;
                            let qw = (w * 158.0 / 368.0).floor();
                            // While a button is disabled, its hint says what is missing.
                            let missing = if !has_address {
                                Some(t("connect", "needAddress"))
                            } else if !r.valid {
                                Some(t("connect", "needValidAddress"))
                            } else {
                                None
                            };
                            let label = if d.querying {
                                t("connect", "querying")
                            } else {
                                t("connect", "query")
                            };
                            let mut b = Button::new(label)
                                .size(Size::Lg)
                                .width(qw)
                                .enabled(!d.querying && r.valid)
                                .tip(missing.unwrap_or(t("connect", "query")));
                            if !d.querying {
                                b = b.icon(lucide::SEARCH);
                            }
                            let q = ui.add(b);
                            if d.querying {
                                let g = paint::galley(
                                    ui,
                                    label,
                                    font(Face::SansMedium, size::TEXT_SM),
                                    tk.fg,
                                );
                                let x = q.rect.center().x - g.size().x / 2.0 - 10.0 - 6.0;
                                let sr = Rect::from_center_size(
                                    egui::pos2(x, q.rect.center().y),
                                    Vec2::splat(size::ICON_SM),
                                );
                                Spinner::paint(ui, sr, tk.fg_faint);
                            }
                            asked.query |= q.clicked();
                            asked.join |= ui
                                .add(
                                    Button::new(t("connect", "connect"))
                                        .icon(lucide::PLAY)
                                        .variant(Variant::Play)
                                        .size(Size::Lg)
                                        .width(w - qw)
                                        .enabled(r.valid && has_profile)
                                        .tip(missing.unwrap_or(if has_profile {
                                            t("connect", "enterToConnect")
                                        } else {
                                            t("connect", "needProfile")
                                        })),
                                )
                                .clicked();
                        });
                    });
            },
        );
    if clear {
        cx.stores.direct.clear();
    }
    if favorite {
        cx.stores.direct_favorite();
    }
    if asked.query {
        cx.stores.direct_query();
    }
    if asked.join {
        cx.stores.direct_join();
    }
}
