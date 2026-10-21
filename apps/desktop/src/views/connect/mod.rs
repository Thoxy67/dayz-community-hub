//! Direct Connect (= `features/connect/View.svelte`): the form on the left (address,
//! password, ports, query and join), the extra launch arguments folded under it, and on
//! the right what the server says (the shared server detail), a spinner while it is
//! asked, or the recent servers as cards. Once a server is shown, the recent servers
//! fold into a list under the form.
//!
//! The form is a store ([`crate::store::direct`]) so it survives leaving the view and the
//! commands reach it; this module only draws it and turns clicks into store calls.

mod args;
mod form;
mod recent;
mod summary;

use egui::{Pos2, Rect, Ui, UiBuilder, Vec2};

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::{Alert, Button, Panel, Spinner, Tone, signal_color, signal_level};
use crate::parts::{Figure, PageHeader};
use crate::store::direct::{Direct, ModStatus, Resolved};
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::View;

/// `grid-cols-[25rem_…]`: the form's column.
const LEFT_W: f32 = 400.0;

pub struct ConnectView;

impl ConnectView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConnectView {
    fn default() -> Self {
        Self::new()
    }
}

/// Arriving from elsewhere ("open in Direct Connect", a pasted address): fill and ask.
fn take_prefill(cx: &mut Ctx) {
    if let Some(p) = cx.stores.connect.prefill.take() {
        cx.stores
            .direct_load(&p.ip, p.port, p.query_port, p.password);
    }
}

impl View for ConnectView {
    fn on_show(&mut self, cx: &mut Ctx) {
        take_prefill(cx);
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        take_prefill(cx);
        cx.stores.direct_poll();
        let r = cx.stores.direct_resolved();
        // A favourite's saved password fills an empty field; the ports open on a bad port.
        cx.stores.direct.fill_from_favorite(r.favorite.as_ref());
        if r.port_error {
            cx.stores.direct.ports_open = true;
        }

        ui.spacing_mut().item_spacing.y = 0.0;
        header(ui, cx, &r);

        let tk = ui.tokens();
        let body = ui.available_rect_before_wrap();
        ui.painter().rect_filled(body, 0.0, tk.border);
        let left = Rect::from_min_max(
            body.min,
            Pos2::new((body.left() + LEFT_W).min(body.right()), body.bottom()),
        );
        let right = Rect::from_min_max(Pos2::new(left.right() + 1.0, body.top()), body.max);
        let detail = cx.stores.direct.detail_target(&r);
        let busy = cx.stores.direct.querying;
        let failed = !cx.stores.direct.error.is_empty();

        // The form, the arguments, then the recent servers (once the right is taken).
        let mut lui = ui.new_child(
            UiBuilder::new()
                .id_salt("connect-left")
                .max_rect(left)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        lui.set_clip_rect(left.intersect(ui.clip_rect()));
        lui.spacing_mut().item_spacing = Vec2::new(0.0, 1.0);
        form::show(&mut lui, cx, &r);
        args::section(&mut lui, cx, &r);
        let rest = lui.available_rect_before_wrap();
        if detail.is_some() || busy || failed {
            let mut pui = lui.new_child(UiBuilder::new().max_rect(rest));
            Panel::new("connect-recent")
                .title(t("connect", "recent"))
                .show(&mut pui, |ui| recent::list(ui, cx, false));
        } else {
            lui.painter().rect_filled(rest, 0.0, tk.panel);
        }

        // What the server says.
        let mut rui = ui.new_child(
            UiBuilder::new()
                .id_salt("connect-right")
                .max_rect(right)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        rui.set_clip_rect(right.intersect(ui.clip_rect()));
        rui.painter().rect_filled(right, 0.0, tk.panel);
        rui.spacing_mut().item_spacing = Vec2::ZERO;
        if failed {
            let e = cx.stores.direct.error.clone();
            Alert::new(
                Tone::Err,
                lucide::TRIANGLE_ALERT,
                t("connect", "queryFailed"),
            )
            .text(&e)
            .banner()
            .show(&mut rui, |_| {});
        }
        if let Some((ip, port)) = detail {
            let rest = rui.available_rect_before_wrap();
            let mut dui = rui.new_child(
                UiBuilder::new()
                    .id_salt(("connect-detail", &ip, port))
                    .max_rect(rest)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            if !crate::views::server_detail(&mut dui, cx, &ip, port, &r.name) {
                summary::show(&mut dui, cx, &r, &ip, port);
            }
        } else if busy {
            querying(&mut rui);
        } else if !failed {
            let rest = rui.available_rect_before_wrap();
            let mut cui = rui.new_child(UiBuilder::new().max_rect(rest));
            egui::ScrollArea::vertical()
                .id_salt("connect-cards")
                .auto_shrink([false, false])
                .show(&mut cui, |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::same(size::PAD as i8))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            recent::heading(ui);
                            recent::list(ui, cx, true);
                        });
                });
        }
        ui.allocate_rect(body, egui::Sense::hover());
    }
}

/// `Spinner` + "Querying server…", centred.
fn querying(ui: &mut Ui) {
    let tk = ui.tokens();
    let rest = ui.available_rect_before_wrap();
    let text = t("connect", "queryingServer");
    let g = crate::kit::paint::galley(ui, text, font(Face::Sans, size::TEXT_XS), tk.fg_muted);
    let w = 16.0 + 8.0 + g.size().x;
    let x = rest.center().x - w / 2.0;
    let mut sui = ui.new_child(UiBuilder::new().max_rect(Rect::from_center_size(
        Pos2::new(x + 8.0, rest.center().y),
        Vec2::splat(16.0),
    )));
    sui.add(Spinner::new().size(16.0).color(tk.accent));
    crate::kit::paint::text_at(ui.painter(), &g, x + 24.0, rest.center().y, tk.fg_muted);
}

/// The title, the figures once the server is known, and the share actions.
fn header(ui: &mut Ui, cx: &mut Ctx, r: &Resolved) {
    let tk = ui.tokens();
    let d = &cx.stores.direct;
    let a2s = d.a2s.clone();
    let listed = r.listed.clone();
    let show_figures = a2s.is_some() || listed.is_some();
    let players = format!(
        "{}/{}",
        a2s.as_ref()
            .map(|a| i64::from(a.players))
            .or(listed.as_ref().map(|l| l.players))
            .unwrap_or(0),
        a2s.as_ref()
            .map(|a| i64::from(a.max_players))
            .or(listed.as_ref().map(|l| l.max_players))
            .unwrap_or(0)
    );
    let ping = match (r.query_port, &listed) {
        (Some(q), l) => {
            let ip = l.as_ref().map_or(r.ip.as_str(), |l| l.ip.as_str());
            cx.stores.servers.ping.get(&format!("{ip}:{q}")).copied()
        }
        _ => None,
    };
    let ping_text = match ping {
        None => "—".to_string(),
        Some(p) if p >= crate::kit::PING_TIMEOUT_MS => "×".to_string(),
        Some(p) => format!("{p} ms"),
    };
    let map = a2s
        .as_ref()
        .map(|a| a.map.clone())
        .filter(|m| !m.is_empty())
        .or(listed.as_ref().map(|l| l.map.clone()))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "—".to_string());
    let server_mods = d.server_mods();
    let missing = server_mods
        .iter()
        .filter(|m| Direct::mod_status(&cx.stores.mods, m.id) == ModStatus::Missing)
        .count();
    let mods_label = tf("connect", "modsCount", &[("count", &server_mods.len())]);
    let mods_value = if missing > 0 {
        format!("{missing} {}", t("connect", "modMissing").to_lowercase())
    } else {
        server_mods.len().to_string()
    };
    let valid = r.valid;
    let (mut open, mut copy, mut export) = (false, false, false);
    PageHeader::new(t("connect", "title")).show(
        ui,
        |ui| {
            if !show_figures {
                return;
            }
            ui.add(
                Figure::new(t("connect", "players"))
                    .value(&players)
                    .tone(tk.ok),
            );
            ui.add(
                Figure::new(t("connect", "ping"))
                    .value(&ping_text)
                    .tone(signal_color(&tk, signal_level(ping))),
            );
            ui.add(Figure::new(t("connect", "map")).value(&map).tone(tk.map));
            ui.add(
                Figure::new(&mods_label)
                    .value(&mods_value)
                    .tone(if missing > 0 { tk.warn } else { tk.mods }),
            );
        },
        |ui| {
            open = ui
                .add(
                    Button::new(t("connect", "openDzch"))
                        .icon(lucide::FILE_UP)
                        .tip(t("connect", "openDzchTitle")),
                )
                .clicked();
            copy = ui
                .add(
                    Button::new(t("connect", "copyUrl"))
                        .icon(lucide::LINK)
                        .enabled(valid)
                        .tip(t("connect", "copyUrlTitle")),
                )
                .clicked();
            export = ui
                .add(
                    Button::new(t("connect", "export"))
                        .icon(lucide::FILE_DOWN)
                        .enabled(valid)
                        .tip(t("connect", "exportTitle")),
                )
                .clicked();
        },
        None::<fn(&mut Ui)>,
    );
    if open {
        let _ = cx.run("connect.open_dzch", &serde_json::Value::Null);
    }
    if copy {
        let _ = cx.run("connect.copy_link", &serde_json::Value::Null);
    }
    if export {
        let _ = cx.run("connect.export_dzch", &serde_json::Value::Null);
    }
}

/// A five-pointed star filled with `fill` (lucide's star, `fill-warn`), its outline over it.
pub(super) fn filled_star(p: &egui::Painter, rect: Rect, color: egui::Color32) {
    let c = rect.center();
    let outer = rect.width() * 0.42;
    let inner = outer * 0.45;
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let a = std::f32::consts::PI * (i as f32) / 5.0 - std::f32::consts::FRAC_PI_2;
            let rr = if i % 2 == 0 { outer } else { inner };
            Pos2::new(
                c.x + rr * a.cos(),
                c.y + 0.04 * rect.height() + rr * a.sin(),
            )
        })
        .collect();
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, color);
    for p in &pts {
        mesh.colored_vertex(*p, color);
    }
    for i in 0..10u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
    }
    p.add(mesh);
    lucide::STAR.paint(p, rect, color);
}
