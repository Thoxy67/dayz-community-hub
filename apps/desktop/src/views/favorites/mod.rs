//! Favourites (= `features/favorites/View.svelte`): the servers the player starred, with
//! their live line, head-count and details, on the shared saved list ([`super::saved`]).
//! The header's figures: how many are saved, how many the current list has, the players
//! on them, the best and average ping.
//!
//! A favourite also carries the player's note, labels and group, and a character name
//! and launch arguments of its own ([`edit`]: the edit and "groups and labels" dialogs).
//! The groups are sections of the list, folded from their heading.

pub mod edit;

use egui::{Color32, Ui};
use serde_json::{Value, json};

use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::lucide;
use crate::kit::{IconButton, Side};
use crate::parts::{Figure, RowCache};
use crate::store::ConfirmRequest;
use crate::store::profile::{FavoriteEdit, LabelColor};
use crate::theme::{Tokens, TokensExt};
use crate::views::View;
use crate::views::saved::{self, EmptyWords, Kind, Stats, cmds};

/// A label's colour in the theme.
pub fn label_color(tk: &Tokens, c: LabelColor) -> Color32 {
    match c {
        LabelColor::Accent => tk.accent,
        LabelColor::Ok => tk.ok,
        LabelColor::Warn => tk.warn,
        LabelColor::Err => tk.err,
        LabelColor::Info => tk.info,
        LabelColor::Map => tk.map,
        LabelColor::Mods => tk.mods,
        LabelColor::Neutral => tk.fg_muted,
    }
}

/// A label colour's name, for the picker's hints.
pub fn label_color_name(c: LabelColor) -> &'static str {
    t(
        "favorites",
        match c {
            LabelColor::Accent => "colorAccent",
            LabelColor::Ok => "colorOk",
            LabelColor::Warn => "colorWarn",
            LabelColor::Err => "colorErr",
            LabelColor::Info => "colorInfo",
            LabelColor::Map => "colorMap",
            LabelColor::Mods => "colorMods",
            LabelColor::Neutral => "colorNeutral",
        },
    )
}

const KIND: Kind = Kind::Favorites;

pub struct FavoritesView {
    cache: RowCache,
}

impl FavoritesView {
    pub fn new() -> Self {
        Self {
            cache: RowCache::new(),
        }
    }
}

impl Default for FavoritesView {
    fn default() -> Self {
        Self::new()
    }
}

/// "N ms", or the dash.
fn ms(v: Option<u32>) -> String {
    v.map_or_else(|| format::NONE.to_string(), |p| format!("{p} ms"))
}

fn figures(ui: &mut Ui, s: &Stats) {
    let tk = ui.tokens();
    ui.add(
        Figure::new(t("favorites", "statSaved"))
            .amount(s.total as f64)
            .tone(tk.accent),
    );
    ui.add(Figure::new(t("favorites", "statOnline")).value(&format!("{}/{}", s.listed, s.total)));
    ui.add(
        Figure::new(t("favorites", "statPlayers"))
            .amount(s.players as f64)
            .tone(tk.ok),
    );
    ui.add(Figure::new(t("favorites", "statBestPing")).value(&ms(s.best_ping)));
    ui.add(Figure::new(t("favorites", "statAvgPing")).value(&ms(s.avg_ping)));
}

impl View for FavoritesView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        saved::show_with_actions(
            ui,
            cx,
            KIND,
            &mut self.cache,
            figures,
            |ui, _, run| {
                if ui
                    .add(
                        IconButton::new(lucide::LAYERS, t("favorites", "organize"))
                            .side(Side::Bottom),
                    )
                    .clicked()
                {
                    run.push(("favorites.organize", Value::Null));
                }
            },
            EmptyWords {
                icon: lucide::STAR,
                title: t("favorites", "noFavorites"),
                hint: t("favorites", "noFavoritesHint"),
            },
        );
    }
}

// ── commands (`commands.rs`) ────────────────────────────────────────────────

pub fn toggle_detail(cx: &mut Ctx, a: &Value) {
    cmds::toggle_detail(cx, KIND, a);
}

pub fn toggle_listed_only(cx: &mut Ctx, a: &Value) {
    cmds::toggle_listed_only(cx, KIND, a);
}

pub fn sort_by(cx: &mut Ctx, a: &Value) {
    cmds::sort_by(cx, KIND, a);
}

pub fn search(cx: &mut Ctx, a: &Value) {
    cmds::search(cx, KIND, a);
}

pub fn select(cx: &mut Ctx, a: &Value) {
    cmds::select(cx, KIND, a);
}

pub fn remove(cx: &mut Ctx, a: &Value) {
    cmds::remove(cx, KIND, a);
}

fn arg_str<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

/// `{name}` of a group or label: given and not empty.
fn arg_name(a: &Value) -> Option<String> {
    arg_str(a, "name")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// `favorites.edit {ip?, port?}`: the edit dialog for the favourite named (its saved,
/// query or game port), else the one picked in the list.
pub fn edit(cx: &mut Ctx, a: &Value) {
    match cmds::entry(cx, KIND, a) {
        Some(e) => edit::open_edit(cx, &e.ip, e.port),
        None => cx.stores.bus.warn(t("favorites", "notFavorite")),
    }
}

/// `favorites.organize`: the groups and labels dialog.
pub fn organize(cx: &mut Ctx, _: &Value) {
    edit::open_organize(cx);
}

/// `favorites.move_to_group {ip?, port?, group}`: the favourite named (else the one
/// picked) put in that group, or in none (`group` null or absent).
pub fn move_to_group(cx: &mut Ctx, a: &Value) {
    let Some(e) = cmds::entry(cx, KIND, a) else {
        cx.stores.bus.warn(t("favorites", "notFavorite"));
        return;
    };
    let group = arg_str(a, "group").map(String::from);
    let done = match &group {
        Some(g) => tf("favorites", "movedToGroup", &[("group", g)]),
        None => t("favorites", "ungroupedDone").to_string(),
    };
    cx.stores.profile.edit_favorites(
        FavoriteEdit::MoveToGroup {
            ip: e.ip,
            port: e.port,
            group,
        },
        Some(done),
    );
}

/// `favorites.group_toggle {name?}`: a group folded or unfolded; without a name, the
/// favourites in no group.
pub fn group_toggle(cx: &mut Ctx, a: &Value) {
    match arg_name(a) {
        Some(name) => {
            let Some(g) = cx.stores.profile.groups.iter().find(|g| g.name == name) else {
                return;
            };
            let collapsed = !g.collapsed;
            cx.stores
                .profile
                .edit_favorites(FavoriteEdit::CollapseGroup { name, collapsed }, None);
        }
        None => {
            let s = saved::shared(cx.egui, KIND);
            let mut st = s.lock();
            st.ungrouped_collapsed = !st.ungrouped_collapsed;
        }
    }
    cx.egui.request_repaint();
}

/// `favorites.group_move {name, delta}`: a group moved up (negative) or down.
pub fn group_move(cx: &mut Ctx, a: &Value) {
    let Some(name) = arg_name(a) else {
        return;
    };
    let delta = a
        .get("delta")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .clamp(-100, 100);
    let delta = i32::try_from(delta).unwrap_or(1);
    cx.stores
        .profile
        .edit_favorites(FavoriteEdit::MoveGroup { name, delta }, None);
}

/// `favorites.group_remove {name}`: a group deleted after asking; its favourites stay.
pub fn group_remove(cx: &mut Ctx, a: &Value) {
    let Some(name) = arg_name(a) else {
        return;
    };
    if !cx.stores.profile.groups.iter().any(|g| g.name == name) {
        return;
    }
    let req = ConfirmRequest::new(
        t("favorites", "removeGroupTitle"),
        tf("favorites", "removeGroupMessage", &[("name", &name)]),
    )
    .danger();
    cx.stores.bus.confirm_then(req, move |cx| {
        cx.stores
            .profile
            .edit_favorites(FavoriteEdit::RemoveGroup { name }, None);
    });
}

/// `favorites.label_remove {name}`: a label deleted after asking, and taken off every
/// favourite.
pub fn label_remove(cx: &mut Ctx, a: &Value) {
    let Some(name) = arg_name(a) else {
        return;
    };
    if cx.stores.profile.label(&name).is_none() {
        return;
    }
    let req = ConfirmRequest::new(
        t("favorites", "removeLabelTitle"),
        tf("favorites", "removeLabelMessage", &[("name", &name)]),
    )
    .danger();
    cx.stores.bus.confirm_then(req, move |cx| {
        cx.stores
            .profile
            .edit_favorites(FavoriteEdit::RemoveLabel { name }, None);
    });
}

/// The arguments the `favorites.*` commands take for a server shown at `ip` with these
/// ports (the server panel's).
pub fn args_for(ip: &str, port: i64) -> Value {
    json!({ "ip": ip, "port": port })
}

/// The server panel's "Your notes" section, for a favourite (nothing for another
/// server): its labels, note, character name and whether it has launch arguments of its
/// own, with an Edit button in the heading.
pub fn detail_notes(ui: &mut Ui, cx: &Ctx, ip: &str, ports: &[u16], run: &mut saved::Run) {
    let p = &cx.stores.profile;
    let Some(f) = p.favorite_of(ip, ports) else {
        return;
    };
    let args = args_for(&f.ip, i64::from(f.port));
    crate::parts::Section::new(t("favorites", "notesTitle"))
        .icon(lucide::MESSAGE_SQUARE)
        .show(
            ui,
            |ui| {
                if ui
                    .add(
                        crate::kit::Button::new(t("favorites", "editTitle"))
                            .icon(lucide::PENCIL)
                            .size(crate::kit::Size::Xs),
                    )
                    .clicked()
                {
                    run.push(("favorites.edit", args.clone()));
                }
            },
            |ui| {
                let tk = ui.tokens();
                let small = crate::theme::font(
                    crate::theme::fonts::Face::Sans,
                    crate::theme::size::TEXT_2XS,
                );
                let empty = f.note.is_empty()
                    && f.labels.is_empty()
                    && f.character.is_none()
                    && f.launch_args.is_empty()
                    && f.group.is_none();
                if empty {
                    edit::hint_line(ui, t("favorites", "notesEmpty"), tk.fg_faint);
                    return;
                }
                if !f.labels.is_empty() || f.group.is_some() {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::new(4.0, 4.0);
                        if let Some(g) = &f.group {
                            ui.add(crate::kit::Tag::new(g).icon(lucide::LAYERS));
                        }
                        for name in &f.labels {
                            if let Some(l) = p.label(name) {
                                edit::label_pill(ui, &l.name, label_color(&tk, l.color));
                            }
                        }
                    });
                }
                if !f.note.is_empty() {
                    edit::hint_line(ui, &f.note, tk.fg);
                }
                let mut facts: Vec<String> = Vec::new();
                if let Some(c) = &f.character {
                    facts.push(tf("favorites", "playsAs", &[("name", c)]));
                }
                if !f.launch_args.is_empty() {
                    facts.push(format!("{} · {}", t("favorites", "ownArgs"), f.launch_args));
                }
                for line in facts {
                    let g = crate::parts::ph_trunc(ui, &line, small.clone(), ui.available_width());
                    let (r, resp) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                    crate::parts::draw(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
                    if crate::parts::is_cut(&g, &line) {
                        let _ = crate::kit::TipExt::tip_kbd(resp, &line, "", Side::Top);
                    }
                }
            },
        );
}
