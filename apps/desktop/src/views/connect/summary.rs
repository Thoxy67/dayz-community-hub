//! What the server said, drawn here while no shared server detail is installed
//! ([`crate::views::set_server_detail`]): its name and address, the facts the query
//! answered, and its mods against what is installed.

use egui::{Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::app::Ctx;
use crate::i18n::t;
use crate::kit::paint::{self, alpha};
use crate::kit::{signal_color, signal_level};
use crate::parts::{Fact, Facts, ModRef, ModState, ModStatusList, map_name};
use crate::store::direct::{Direct, ModStatus, Resolved};
use crate::theme::{TokensExt, font, fonts::Face, size};

pub fn show(ui: &mut Ui, cx: &mut Ctx, r: &Resolved, ip: &str, port: i64) {
    let tk = ui.tokens();
    let d = &cx.stores.direct;
    let a2s = d.a2s.clone();
    let listed = r.listed.clone();
    let name = if r.name.is_empty() {
        format!("{ip}:{}", r.game_port)
    } else {
        r.name.clone()
    };
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
    let map = a2s
        .as_ref()
        .map(|a| a.map.clone())
        .filter(|m| !m.is_empty())
        .or(listed.as_ref().map(|l| l.map.clone()))
        .unwrap_or_default();
    let version = a2s
        .as_ref()
        .map(|a| a.version.clone())
        .or(listed.as_ref().map(|l| l.version.clone()))
        .unwrap_or_default();
    let ping = cx.stores.servers.ping.get(&format!("{ip}:{port}")).copied();
    let ping_text = match ping {
        None => "—".to_string(),
        Some(p) if p >= crate::kit::PING_TIMEOUT_MS => "×".to_string(),
        Some(p) => format!("{p} ms"),
    };
    let game = r.game_port.to_string();
    let query = port.to_string();
    let mut mods: Vec<ModRef> = d
        .server_mods()
        .iter()
        .filter_map(|m| {
            let id = u64::try_from(m.id).ok()?;
            let state = match Direct::mod_status(&cx.stores.mods, m.id) {
                ModStatus::Missing => ModState::Missing,
                ModStatus::Stale => ModState::Stale,
                ModStatus::Installed => ModState::Installed,
            };
            let bytes = cx.stores.mods.by_id(id).map_or(0, |m| m.size);
            Some(ModRef::new(id, &m.name, state, bytes))
        })
        .collect();
    ModRef::sort(&mut mods);
    let loading = d.details_loading;

    let rest = ui.available_rect_before_wrap();
    let mut open = None;
    egui::ScrollArea::vertical()
        .id_salt("connect-summary")
        .auto_shrink([false, false])
        .max_height(rest.height())
        .show(ui, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(size::PAD as i8))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 10.0);
                    let w = ui.available_width();
                    // Name over the address, in the map's tone.
                    let ng =
                        paint::galley_wrap(ui, &name, font(Face::Display, size::TEXT_LG), tk.fg, w);
                    let ag = paint::galley(
                        ui,
                        &format!("{ip}:{}", r.game_port),
                        font(Face::Mono, size::TEXT_2XS),
                        tk.fg_faint,
                    );
                    let (hr, _) = ui.allocate_exact_size(
                        Vec2::new(w, ng.size().y + 4.0 + ag.size().y),
                        Sense::hover(),
                    );
                    let p = ui.painter();
                    p.galley(hr.min, ng.clone(), tk.fg);
                    p.galley(
                        Pos2::new(hr.left(), hr.top() + ng.size().y + 4.0),
                        ag,
                        tk.fg_faint,
                    );
                    let map_shown = map_name(&map);
                    let facts = [
                        Fact::new(t("connect", "players"), &players).tone(tk.ok),
                        Fact::new(t("connect", "ping"), &ping_text)
                            .tone(signal_color(&tk, signal_level(ping))),
                        Fact::new(t("connect", "map"), map_shown.as_ref()).tone(tk.map),
                        Fact::new(t("connect", "version"), &version),
                        Fact::new(t("connect", "gamePort"), &game),
                        Fact::new(t("connect", "queryPort"), &query),
                    ];
                    ui.add(Facts::new(&facts).columns(3));
                    let y = ui.cursor().min.y;
                    ui.painter().hline(
                        Rect::from_min_size(Pos2::new(hr.left(), y), Vec2::new(w, 1.0)).x_range(),
                        y,
                        Stroke::new(1.0, alpha(tk.border, 0.6)),
                    );
                    if loading && mods.is_empty() {
                        ui.label(
                            egui::RichText::new(t("connect", "loadingMods"))
                                .font(font(Face::Sans, size::TEXT_2XS))
                                .color(tk.fg_muted),
                        );
                    } else if mods.is_empty() {
                        ui.label(
                            egui::RichText::new(t("connect", "noMods"))
                                .font(font(Face::Sans, size::TEXT_2XS))
                                .color(tk.fg_muted),
                        );
                    } else {
                        open = ModStatusList::new(&mods).show(ui);
                    }
                });
        });
    if let Some(id) = open {
        let url = format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}");
        if let Err(e) = crate::platform::deck::open(&url) {
            cx.stores.bus.err(e.to_string());
        }
    }
}
