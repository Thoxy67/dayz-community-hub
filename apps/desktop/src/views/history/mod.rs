//! History (= `features/history/View.svelte`): every server joined, newest first, one click
//! back into any of them, on the shared saved list ([`super::saved`]) with its extra
//! "last played" column. The header's figures: how many were played, the last session,
//! this week's, how many the current list has, the players on them, the best ping; then
//! the tools and "Clear all history".
//!
//! The relative times ("3 minutes ago", "this week") move on: while the view is on screen
//! it repaints once a minute, and on nothing else of its own.

use std::time::Duration;

use egui::Ui;
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::lucide;
use crate::kit::{Button, Variant};
use crate::parts::{Figure, RowCache};
use crate::theme::TokensExt;
use crate::views::View;
use crate::views::saved::{self, EmptyWords, Kind, Run, Stats, cmds};

const KIND: Kind = Kind::History;

pub struct HistoryView {
    cache: RowCache,
}

impl HistoryView {
    pub fn new() -> Self {
        Self {
            cache: RowCache::new(),
        }
    }
}

impl Default for HistoryView {
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
        Figure::new(t("history", "statEntries"))
            .amount(s.total as f64)
            .tone(tk.accent),
    );
    let (last, last_tip) = match s.last_ts {
        Some(ts) => (format::relative(ts), format::date_time(ts)),
        None => (format::NONE.to_string(), String::new()),
    };
    ui.add(
        Figure::new(t("history", "statLastPlayed"))
            .value(&last)
            .tip(&last_tip),
    );
    ui.add(
        Figure::new(t("history", "statThisWeek"))
            .amount(s.this_week as f64)
            .secondary(),
    );
    ui.add(
        Figure::new(t("favorites", "statOnline"))
            .value(&format!("{}/{}", s.listed, s.total))
            .secondary(),
    );
    ui.add(
        Figure::new(t("favorites", "statPlayers"))
            .amount(s.players as f64)
            .tone(tk.ok),
    );
    ui.add(
        Figure::new(t("favorites", "statBestPing"))
            .value(&ms(s.best_ping))
            .secondary(),
    );
}

/// After the tools, where the browser has Refresh: "Clear all history", while there is any.
fn actions(ui: &mut Ui, s: &Stats, run: &mut Run) {
    if s.total == 0 {
        return;
    }
    let b = Button::new(t("history", "clearAll"))
        .icon(lucide::TRASH_2)
        .variant(Variant::Danger)
        .tip(t("history", "clearAllTitle"));
    if ui.add(b).clicked() {
        run.push(("history.clear", Value::Null));
    }
}

impl View for HistoryView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        saved::show_with_actions(
            ui,
            cx,
            KIND,
            &mut self.cache,
            figures,
            actions,
            EmptyWords {
                icon: lucide::HISTORY,
                title: t("history", "noHistory"),
                hint: t("history", "noHistoryHint"),
            },
        );
        // "N minutes ago" and "this week" move on by themselves.
        if !cx.stores.profile.history.is_empty() {
            ui.ctx().request_repaint_after(Duration::from_secs(60));
        }
    }

    fn on_show(&mut self, cx: &mut Ctx) {
        // The rail's Rejoin card opens the history on the last server played.
        if cx.nav.focus.as_deref() == Some("last") {
            cx.nav.take_focus();
            let s = saved::shared(cx.egui, KIND);
            s.lock().focus_last = true;
            cx.egui.request_repaint();
        }
    }
}

// ── commands (`commands.rs`) ────────────────────────────────────────────────

/// `history.clear`: the whole history removed, after asking.
pub fn clear(cx: &mut Ctx, _: &Value) {
    cx.stores.profile.clear_history();
}

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
