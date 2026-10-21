//! Favourites (= `features/favorites/View.svelte`): the servers the player starred, with
//! their live line, head-count and details, on the shared saved list ([`super::saved`]).
//! The header's figures: how many are saved, how many the current list has, the players
//! on them, the best and average ping.

use egui::Ui;
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::parts::{Figure, RowCache};
use crate::theme::TokensExt;
use crate::views::View;
use crate::views::saved::{self, EmptyWords, Kind, Stats, cmds};

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
        saved::show(
            ui,
            cx,
            KIND,
            &mut self.cache,
            figures,
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
