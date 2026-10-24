//! Friends (= `features/friends/View.svelte`): the account's Steam friends, who plays
//! DayZ and where (the server as every server list shows it, one click to join, show it
//! in the list or ask it for its details), who is in another game, online, away,
//! offline. The friends store ([`crate::store::friends`]) keeps them fresh; its search and
//! selection live there too, so the commands reach them.
//!
//! The groups are worked out once per answer, search and server list ([`model`]), not per
//! frame; the list draws only the rows on screen.

pub mod list;
pub mod model;

use std::sync::Arc;

use dz_api::SocketAddrV4;
use dz_hub::features::friends::FriendsSource;
use egui::{Rect, Ui, UiBuilder, Vec2};

use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::{Button, IconButton, Input, SkeletonRows, Tag, TipExt, Variant};
use crate::parts::{Empty, Figure, MasterDetail, PageHeader, RowCache};
use crate::store::friends::Selected;
use crate::theme::{TokensExt, font, fonts::Face, size};
use crate::views::{View, ViewId};
use model::Model;

/// How often "x ago" is written again while the view shows.
const TICK: std::time::Duration = std::time::Duration::from_secs(15);

/// Now, in Unix seconds.
pub(crate) fn now_unix() -> i64 {
    crate::store::now_ms() / 1000
}

/// What a click in the list asks for, done once the list is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Select(Selected),
    Join(String, u16),
    Message(String),
    Profile(String),
    ShowInList(String, u16),
    /// ip, game port, query port.
    Direct(String, u16, u16),
    /// name, ip, port, favourite now.
    Favorite(String, String, u16, bool),
    Ping(String, i64),
    Players(String, i64),
    /// ip, game port, query port, name.
    CopyLink(String, u16, u16, String),
    CopyAddress(String),
    /// ip, game port, query port, name.
    Shortcut(String, u16, u16, String),
    /// ip, exclude (true) or include.
    Exclude(String, bool),
}

pub struct FriendsView {
    model: Model,
    /// What `model` was built from: the store's answer, the search, the server list.
    built: Option<(u64, String, usize)>,
    cache: RowCache,
}

impl FriendsView {
    pub fn new() -> Self {
        Self {
            model: Model::default(),
            built: None,
            cache: RowCache::new(),
        }
    }

    fn rebuild(&mut self, cx: &Ctx) {
        let f = &cx.stores.friends;
        let snap = cx.stores.servers.snapshot();
        let key = (f.generation, f.query.clone(), Arc::as_ptr(snap) as usize);
        if self.built.as_ref() == Some(&key) {
            return;
        }
        let servers = &cx.stores.servers;
        let server_of = |addr: SocketAddrV4| {
            servers
                .row(addr)
                .map(|r| (r.server.name.clone(), r.map.to_string()))
        };
        self.model = Model::build(f.list(), &f.query, &server_of);
        self.built = Some(key);
    }
}

impl Default for FriendsView {
    fn default() -> Self {
        Self::new()
    }
}

/// Steam's own pages for a friend, through the Steam client (Valve's "Steam browser
/// protocol": `steam://url/SteamIDPage/<id>` the profile, `steam://friends/message/<id>`
/// a chat).
pub fn open_steam(cx: &Ctx, url: &str) {
    if let Err(e) = crate::platform::deck::open(url) {
        cx.stores.bus.err(e.to_string());
    }
}

pub fn message_url(steam_id: &str) -> String {
    format!("steam://friends/message/{steam_id}")
}

pub fn profile_url(steam_id: &str) -> String {
    format!("steam://url/SteamIDPage/{steam_id}")
}

fn apply(cx: &mut Ctx, act: Act) {
    match act {
        Act::Select(s) => cx.stores.friends.selected = Some(s),
        Act::Join(ip, port) => cx.join_address(ip, port, None),
        Act::Message(id) => open_steam(cx, &message_url(&id)),
        Act::Profile(id) => open_steam(cx, &profile_url(&id)),
        Act::ShowInList(ip, port) => cx.go_focus(ViewId::Servers, format!("server:{ip}:{port}")),
        Act::Direct(ip, game, query) => {
            cx.stores
                .connect
                .open_in_direct(ip, game, Some(query), None)
        }
        Act::Favorite(name, ip, port, _) => cx.stores.profile.toggle_favorite(&name, &ip, port),
        Act::Ping(ip, port) => {
            let timeout = cx.stores.profile.ping_settings().2;
            cx.stores.servers.ping_one(&ip, port, timeout);
        }
        Act::Players(ip, port) => {
            let s = &mut *cx.stores;
            s.server_data.refresh_a2s(&s.servers, &ip, port);
        }
        Act::CopyLink(ip, game, query, name) => {
            cx.stores
                .connect
                .copy_link(cx.egui, &ip, game, Some(query), Some(&name));
        }
        Act::CopyAddress(addr) => {
            cx.egui.copy_text(addr.clone());
            cx.stores
                .bus
                .ok(format!("{} · {addr}", t("common", "copied")));
        }
        Act::Shortcut(ip, game, query, name) => {
            let args = serde_json::json!({
                "ip": ip, "port": query, "gamePort": game, "name": name,
            });
            let _ = cx.run("list.shortcut", &args);
        }
        Act::Exclude(ip, on) => {
            if on {
                cx.stores.profile.exclude_ip(&ip);
            } else {
                cx.stores.profile.unexclude_ip(&ip);
            }
        }
    }
}

impl View for FriendsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        // "x ago" re-ticks while the view shows; nothing else repaints on a timer.
        ui.ctx().request_repaint_after(TICK);
        self.rebuild(cx);
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        header(ui, cx);

        let tk = ui.tokens();
        let body = ui.available_rect_before_wrap();
        let mut bui = ui.new_child(
            UiBuilder::new()
                .max_rect(body)
                .id_salt("friends-body")
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        bui.set_clip_rect(body.intersect(ui.clip_rect()));
        ui.allocate_rect(body, egui::Sense::hover());
        let ui = &mut bui;
        let f = &cx.stores.friends;
        let failed = f
            .failed
            .clone()
            .or_else(|| f.data.as_ref().and_then(|d| d.error.clone()));
        let empty = f.list().is_empty();

        if f.data.is_none() && f.loading {
            ui.painter().rect_filled(body, 0.0, tk.panel);
            ui.add(SkeletonRows::new(10).media());
            return;
        }
        if let (Some(e), true) = (&failed, empty) {
            ui.painter().rect_filled(body, 0.0, tk.panel);
            let loading = f.loading;
            let (mut retry, mut settings) = (false, false);
            Empty::new()
                .icon(lucide::USER_X)
                .title(t("friends", "unavailable"))
                .error(e)
                .text(t("friends", "unavailableHint"))
                .show(ui, |ui| {
                    retry = ui
                        .add(
                            Button::new(t("friends", "retry"))
                                .icon(lucide::REFRESH_CW)
                                .spinner(loading)
                                .enabled(!loading),
                        )
                        .clicked();
                    settings = ui
                        .add(Button::new(t("friends", "openSettings")).variant(Variant::Accent))
                        .clicked();
                });
            if retry {
                let _ = cx.run("friends.refresh", &serde_json::Value::Null);
            }
            if settings {
                let _ = cx.run("view.settings.apis", &serde_json::Value::Null);
            }
            return;
        }
        if empty {
            ui.painter().rect_filled(body, 0.0, tk.panel);
            let hint = if f.data.is_some() {
                t("friends", "emptyHint")
            } else {
                ""
            };
            let loading = f.loading;
            let mut refresh = false;
            Empty::new()
                .icon(lucide::USERS)
                .title(t("friends", "empty"))
                .text(hint)
                .show(ui, |ui| {
                    refresh = refresh_button(ui, loading, Variant::Default)
                });
            if refresh {
                let _ = cx.run("friends.refresh", &serde_json::Value::Null);
            }
            return;
        }

        // Esc closes the details (as the detail's close button).
        if cx.stores.friends.selected.is_some()
            && !ui.ctx().egui_wants_keyboard_input()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && !egui::Popup::is_any_open(ui.ctx())
        {
            cx.stores.friends.selected = None;
        }

        let mut acts: Vec<Act> = Vec::new();
        let selected = cx.stores.friends.selected.clone();
        let cell = std::cell::RefCell::new(&mut *cx);
        let (model, cache) = (&self.model, &mut self.cache);
        let mut close = false;
        MasterDetail::new("friends-detail")
            .selected(selected.is_some())
            .initial(380.0)
            .range(300.0, 620.0)
            .breakpoint(980.0)
            .show(
                ui,
                |ui| {
                    if let Ok(cx) = cell.try_borrow() {
                        list::show(ui, &cx, model, cache, &mut acts);
                    }
                },
                |ui| {
                    let Ok(mut cx) = cell.try_borrow_mut() else {
                        return;
                    };
                    match &selected {
                        Some(s) => {
                            close = detail(ui, &mut cx, s);
                        }
                        None => Empty::new()
                            .icon(lucide::SERVER)
                            .title(t("servers", "selectServer"))
                            .show(ui, |_| {}),
                    }
                },
            );
        if close {
            cx.stores.friends.selected = None;
        }
        for a in acts {
            apply(cx, a);
        }
    }
}

/// The selected server's details: the shared detail when the servers view installed it,
/// else its name, address and a close button. True when closed.
fn detail(ui: &mut Ui, cx: &mut Ctx, s: &Selected) -> bool {
    if crate::views::server_detail(ui, cx, &s.ip, i64::from(s.port), &s.name) {
        return false;
    }
    let tk = ui.tokens();
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, tk.bg);
    let mut close = false;
    let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 56.0));
    let mut hui = ui.new_child(
        UiBuilder::new()
            .max_rect(head.shrink2(Vec2::new(size::PAD, 8.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    let name = crate::kit::paint::galley_trunc(
        &hui,
        &s.name,
        font(Face::SansSemibold, size::TEXT_SM),
        tk.fg,
        (head.width() - 2.0 * size::PAD - 32.0).max(20.0),
    );
    let addr = format!("{}:{}", s.ip, s.port);
    let ag = crate::kit::paint::galley(&hui, &addr, font(Face::Mono, size::TEXT_2XS), tk.fg_muted);
    let x = hui.max_rect().left();
    let cy = head.center().y;
    hui.painter()
        .galley(egui::pos2(x, cy - name.size().y - 1.0), name, tk.fg);
    hui.painter()
        .galley(egui::pos2(x, cy + 1.0), ag, tk.fg_muted);
    hui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        close = ui
            .add(IconButton::new(lucide::X, t("common", "close")).kbd("Esc"))
            .clicked();
    });
    ui.painter().hline(
        rect.x_range(),
        head.bottom() - 0.5,
        egui::Stroke::new(1.0, tk.border),
    );
    close
}

fn refresh_button(ui: &mut Ui, loading: bool, variant: Variant) -> bool {
    ui.add(
        Button::new(t("friends", "refresh"))
            .icon(lucide::REFRESH_CW)
            .spinner(loading)
            .enabled(!loading)
            .variant(variant),
    )
    .clicked()
}

/// The title, the three figures, the source and freshness, Refresh, and the search.
fn header(ui: &mut Ui, cx: &mut Ctx) {
    let tk = ui.tokens();
    let f = &cx.stores.friends;
    let (in_dayz, online, total) = (f.in_dayz(), f.online(), f.list().len());
    let source = f.data.as_ref().and_then(|d| d.source);
    let updated = f.updated_at.map(|at| {
        let secs = i64::try_from(at.elapsed().as_secs()).unwrap_or(0);
        let when = format::relative_at(now_unix(), now_unix().saturating_sub(secs));
        tf("friends", "updated", &[("when", &when)])
    });
    let loading = f.loading;
    let mut query = f.query.clone();
    let focus = cx.nav.take_focus_search() && total > 0;
    let mut refresh = false;
    let toolbar = (total > 0).then_some(|ui: &mut Ui| {
        let w = ui.available_width().clamp(128.0, 320.0);
        let r = ui.add(
            Input::new(&mut query)
                .search()
                .placeholder(t("friends", "search"))
                .clear_label(t("servers", "clearSearch"))
                .width(w)
                .id_salt("friends-search"),
        );
        // Where a controller lands in the view.
        crate::pad::primary(ui.ctx(), r.id);
        if focus {
            r.request_focus();
        }
        // The Svelte row is a pixel taller than its field.
        ui.add_space(1.0);
    });
    PageHeader::new(t("nav", "friends")).show(
        ui,
        |ui| {
            ui.add(
                Figure::new(t("friends", "statInDayz"))
                    .amount(in_dayz as f64)
                    .tone(tk.accent),
            );
            ui.add(
                Figure::new(t("friends", "statOnline"))
                    .amount(online as f64)
                    .tone(tk.ok),
            );
            ui.add(Figure::new(t("friends", "statTotal")).amount(total as f64));
        },
        |ui| {
            if source == Some(FriendsSource::SteamClient) {
                ui.add(
                    Tag::new(t("friends", "sourceClient")).tip(t("friends", "sourceClientHint")),
                );
            }
            if let Some(u) = &updated {
                ui.add_space(4.0);
                let g =
                    crate::kit::paint::galley(ui, u, font(Face::Sans, size::TEXT_2XS), tk.fg_faint);
                // `items-center` with the button beside it.
                let (r, resp) = ui.allocate_exact_size(
                    Vec2::new(g.size().x + 4.0, size::CONTROL),
                    egui::Sense::hover(),
                );
                ui.painter().galley(
                    egui::pos2(r.left() + 2.0, r.center().y - g.size().y / 2.0),
                    g,
                    tk.fg_faint,
                );
                if source == Some(FriendsSource::WebApi) {
                    let _ = resp.tip(t("friends", "sourceWeb"));
                }
                ui.add_space(4.0);
            }
            refresh = refresh_button(ui, loading, Variant::Default);
        },
        toolbar,
    );
    if query != cx.stores.friends.query {
        cx.stores.friends.query = query;
    }
    if refresh {
        let _ = cx.run("friends.refresh", &serde_json::Value::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_links() {
        assert_eq!(message_url("7656"), "steam://friends/message/7656");
        assert_eq!(profile_url("7656"), "steam://url/SteamIDPage/7656");
    }
}
