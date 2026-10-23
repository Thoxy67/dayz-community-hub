//! A download operation as it happens (= `ModOpDialog.svelte`): SteamCMD's or the Steam
//! client's (both report the same steps). The phase it is in, how far the current mod
//! and the whole batch have got (bytes, speed, time left), what has come through, what
//! it needs from the player (a Steam Guard approval, the password), and the downloader's
//! own log streaming beside it.
//!
//! Drawn by the shell over everything while the operation is active and not minimised
//! ([`host`], installed with [`crate::shell::set_mod_op_host`]). Closing it while it runs
//! only hides it: the operation carries on and the status bar keeps showing it; once
//! finished, closing it dismisses it.

use std::sync::Arc;

use dz_hub::features::steamcmd::ModOpType;
use egui::mutex::Mutex;
use egui::{
    Align, Color32, CornerRadius, Id, Layout, Margin, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};
use serde_json::{Value, json};

use super::model::{self, Progress};
use crate::app::Ctx;
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Input, InputKind, Meter, MeterSize, Size, Spinner, Tone, Variant};
use crate::parts::{LogPane, Rate, TransferLabels, TransferProgress, clock, draw, ph, ph_trunc};
use crate::rt::{self, Task};
use crate::store::mods::{ModOp, ModOpPhase, ModOpVia};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// SteamCMD waits about this long for the phone, per attempt.
const GUARD_S: f64 = 65.0;

/// What the dialog keeps between frames.
#[derive(Default)]
pub struct OpUi {
    /// The log beside the progress (kept for the session).
    hide_log: bool,
    password: String,
    reveal: bool,
    sending: bool,
    /// "I don't trust this": the manual login instead of the password field.
    distrust: bool,
    steamcmd_home: Option<String>,
    home_task: Option<Task<Result<Option<String>, String>>>,
    /// When the Steam Guard countdown (re)started (egui time).
    guard_from: Option<std::time::Instant>,
    /// The newest log line seen, to spot SteamCMD's "retrying".
    seen_rev: u64,
    phase: Option<ModOpPhase>,
    rate: Rate,
    rate_at: Option<u64>,
    speed: Option<f64>,
    /// The log as the pane reads it, kept in step with the operation's: new lines
    /// added, the live line rewritten, as many dropped at the front as the store drops.
    lines: Vec<String>,
    times: Vec<f64>,
    lines_rev: u64,
    /// The store's `log_dropped` that `lines` matches.
    lines_dropped: u64,
    lines_start: Option<std::time::Instant>,
    /// The clock stopped where the operation finished (ms), for the operation that
    /// started at `lines_start`.
    finished_ms: Option<f64>,
    /// The footer's buttons' width, last frame (the note wraps in what is left).
    buttons_w: f32,
}

type Shared = Arc<Mutex<OpUi>>;

fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.mods.op"), || {
            Arc::new(Mutex::new(OpUi::default()))
        })
        .clone()
    })
}

/// The operation's log pane.
const LOG_ID: &str = "mods-op-log";

/// `log.filter {"text"}`: filter the operation's log.
pub fn cmd_log_filter(cx: &mut Ctx, a: &Value) {
    let text = a.get("text").and_then(Value::as_str).unwrap_or("");
    LogPane::set_filter(cx.egui, LOG_ID, text);
}

/// `log.copy`: copy the operation's log, each line with its time.
pub fn cmd_log_copy(cx: &mut Ctx, _: &Value) {
    let op = &cx.stores.mods.op;
    let lines: Vec<String> = op.log.iter().map(|(_, l)| l.clone()).collect();
    let times: Vec<f64> = op.log.iter().map(|(at, _)| *at as f64).collect();
    LogPane::copy(cx.egui, LOG_ID, &lines, Some(&times));
}

/// `log.follow`: back to the end of the operation's log, following it.
pub fn cmd_log_follow(cx: &mut Ctx, _: &Value) {
    LogPane::follow(cx.egui, LOG_ID);
}

/// `mods.op.toggle_log`: show or hide the downloader's log.
pub fn cmd_toggle_log(cx: &mut Ctx, _: &Value) {
    let s = shared(cx.egui);
    let mut s = s.lock();
    s.hide_log = !s.hide_log;
}

/// The command a player types to log SteamCMD in by hand (with SteamCMD's own HOME on
/// Linux, where its login is cached).
fn manual_cmd(cx: &Ctx, home: Option<&str>) -> String {
    let login = cx
        .stores
        .profile
        .data
        .as_ref()
        .and_then(|p| p.steam_login.as_deref())
        .filter(|l| !l.is_empty())
        .unwrap_or("YOUR_USERNAME");
    match home {
        Some(h) => format!("HOME=\"{h}\" steamcmd +login {login} +quit"),
        None => format!("steamcmd +login {login} +quit"),
    }
}

/// `mods.op.manual_login`: copy the manual login command and show how to use it.
pub fn cmd_manual_login(cx: &mut Ctx, _: &Value) {
    let s = shared(cx.egui);
    let mut s = s.lock();
    let cmd = manual_cmd(cx, s.steamcmd_home.as_deref());
    cx.egui.copy_text(cmd);
    s.distrust = true;
}

/// Bring the pane's copy of the log in step with the operation's: only what changed is
/// copied (a few lines a step, where a whole copy was up to [`crate::store::mods::LOG_MAX`]).
fn sync_lines(st: &mut OpUi, op: &ModOp) {
    let same_op = st.lines_start == Some(op.started_at) && op.log_dropped >= st.lines_dropped;
    if same_op {
        let drop = usize::try_from(op.log_dropped - st.lines_dropped).unwrap_or(usize::MAX);
        let drop = drop.min(st.lines.len());
        st.lines.drain(..drop);
        st.times.drain(..drop);
    }
    if !same_op || st.lines.len() > op.log.len() {
        st.lines.clear();
        st.times.clear();
    }
    // The last line kept may be the live progress line, rewritten since.
    if let Some(i) = st.lines.len().checked_sub(1)
        && let Some((at, l)) = op.log.get(i)
        && st.lines[i] != *l
    {
        st.lines[i].clone_from(l);
        st.times[i] = *at as f64;
    }
    for (at, l) in op.log.iter().skip(st.lines.len()) {
        st.lines.push(l.clone());
        st.times.push(*at as f64);
    }
    st.lines_rev = op.log_rev;
    st.lines_dropped = op.log_dropped;
    st.lines_start = Some(op.started_at);
}

fn kind_label(k: ModOpType) -> &'static str {
    match k {
        ModOpType::Login => t("progress", "kindLogin"),
        ModOpType::Repair => t("progress", "kindRepair"),
        ModOpType::InstallServer | ModOpType::InstallManual => t("progress", "kindInstall"),
        _ => t("progress", "kindUpdate"),
    }
}

fn status_line(op: &ModOp) -> String {
    match op.phase {
        ModOpPhase::SteamGuardMobile => t("progress", "statusSteamGuard").into(),
        ModOpPhase::PasswordRequired => t("progress", "statusPassword").into(),
        ModOpPhase::Finished => {
            if op.kind == ModOpType::Login {
                return if op.ok > 0 {
                    t("progress", "statusLoggedIn")
                } else {
                    t("progress", "statusLoginFailed")
                }
                .into();
            }
            if op.hint.is_some() {
                return if op.via == ModOpVia::Steamworks {
                    t("progress", "statusSteamFailed")
                } else {
                    t("progress", "statusLoginFailed")
                }
                .into();
            }
            if op.failed == 0 {
                let k = if op.ok == 1 {
                    "statusDone"
                } else {
                    "statusDonePlural"
                };
                tf("progress", k, &[("ok", &op.ok)])
            } else {
                tf(
                    "progress",
                    "statusDoneFailed",
                    &[("ok", &op.ok), ("failed", &op.failed)],
                )
            }
        }
        ModOpPhase::Downloading => {
            if op.current_id != 0 && !op.current_name.is_empty() {
                tf(
                    "progress",
                    "statusDownloading",
                    &[("name", &op.current_name)],
                )
            } else {
                t("progress", "statusPreparing").into()
            }
        }
    }
}

/// Where a queued mod is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum QState {
    Queued,
    Downloading,
    Done,
    Failed,
}

struct QRow {
    id: u64,
    name: String,
    state: QState,
    ms: u64,
}

/// The dialog, drawn by the shell.
pub fn host(ctx: &egui::Context, cx: &mut Ctx) {
    let s = shared(ctx);
    let mut run: Vec<(&'static str, Value)> = Vec::new();
    {
        let mut st = s.lock();
        dialog(ctx, cx, &mut st, &mut run);
    }
    for (id, a) in run {
        if let Err(e) = cx.run(id, &a) {
            cx.stores.bus.err(e);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn dialog(ctx: &egui::Context, cx: &Ctx, st: &mut OpUi, run: &mut Vec<(&'static str, Value)>) {
    let tk = ctx.tokens();
    let op = &cx.stores.mods.op;
    let running = op.phase != ModOpPhase::Finished;

    // ── what changed since the last frame ──
    if st.phase != Some(op.phase) {
        if op.phase == ModOpPhase::SteamGuardMobile {
            st.guard_from = Some(std::time::Instant::now());
        }
        if op.phase != ModOpPhase::PasswordRequired {
            st.sending = false;
            st.distrust = false;
            st.password.clear();
        }
        st.phase = Some(op.phase);
    }
    if op.log_rev != st.seen_rev {
        if op.phase == ModOpPhase::SteamGuardMobile {
            // A new attempt: the countdown starts again.
            let fresh = op
                .log
                .iter()
                .rev()
                .take(8)
                .any(|(_, l)| model::find_ci(l, "retrying").is_some());
            if fresh {
                st.guard_from = Some(std::time::Instant::now());
            }
        }
        st.seen_rev = op.log_rev;
    }
    if op.phase == ModOpPhase::PasswordRequired
        && st.steamcmd_home.is_none()
        && st.home_task.is_none()
    {
        let hub = cx.hub.clone();
        st.home_task = Some(rt::spawn(async move {
            hub.steamcmd_dirs().await.map(|d| d.home)
        }));
    }
    if let Some(task) = &mut st.home_task
        && let Some(r) = task.poll()
    {
        st.home_task = None;
        st.steamcmd_home = Some(r.ok().flatten().unwrap_or_default()).filter(|h| !h.is_empty());
    }
    // The log as the pane reads it.
    if st.lines_rev != op.log_rev || st.lines_start != Some(op.started_at) {
        sync_lines(st, op);
    }
    // The current mod's bytes and speed.
    let progress: Option<Progress> = model::op_progress(op);
    match progress {
        Some(p) => {
            if st.rate_at != Some(p.at) {
                st.rate.add(p.at as f64, p.done as f64);
                st.rate_at = Some(p.at);
                st.speed = st.rate.per_second();
            }
        }
        None => {
            st.rate.reset();
            st.rate_at = None;
            st.speed = None;
        }
    }
    let speed = st.speed;
    let eta = match (progress, speed) {
        (Some(p), Some(s)) if s > 0.0 => Some(p.total.saturating_sub(p.done) as f64 / s * 1000.0),
        _ => None,
    };
    // The current mod's share counts only until it is among the finished.
    let current_done = op.completed.iter().any(|c| c.id == op.current_id);
    let overall = if op.total > 0 {
        let cur = match progress {
            Some(p) if !current_done => p.percent / 100.0,
            _ => 0.0,
        };
        ((op.completed.len() as f64 + cur) / op.total as f64).min(1.0)
    } else {
        0.0
    };
    let ok_count = op.completed.iter().filter(|c| c.ok).count();
    let fail_count = op.completed.len() - ok_count;
    let elapsed = if running {
        st.finished_ms = None;
        op.started_at.elapsed().as_secs_f64() * 1000.0
    } else {
        *st.finished_ms
            .get_or_insert_with(|| op.started_at.elapsed().as_secs_f64() * 1000.0)
    };
    let item_elapsed = op.item_started_at.elapsed().as_secs_f64() * 1000.0;
    let guard_left = st.guard_from.map_or(GUARD_S, |from| {
        (GUARD_S - from.elapsed().as_secs_f64()).max(0.0).ceil()
    });
    if running {
        // The clocks tick; nothing else asks for frames while SteamCMD is quiet.
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }

    // The queue: every mod asked for, then any that came through unannounced.
    let mut ids: Vec<u64> = op.ids.clone();
    for c in &op.completed {
        if !ids.contains(&c.id) {
            ids.push(c.id);
        }
    }
    let queue: Vec<QRow> = ids
        .into_iter()
        .map(|id| {
            let done = op.completed.iter().find(|c| c.id == id);
            let state = match done {
                Some(c) if c.ok => QState::Done,
                Some(_) => QState::Failed,
                None if running && id == op.current_id => QState::Downloading,
                None => QState::Queued,
            };
            let name = done
                .map(|c| c.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| cx.stores.mods.name_of(id));
            QRow {
                id,
                name,
                state,
                ms: done.map_or(0, |c| c.ms),
            }
        })
        .collect();

    // ── the frame ──
    let screen = ctx.content_rect();
    let show_log = !st.hide_log;
    let w = if show_log { 1088.0_f32 } else { 480.0 }
        .min(screen.width() - 32.0)
        .max(240.0);
    let h = 736.0_f32.min(screen.height() - 48.0).max(240.0);
    let id = Id::new("dzch.mods.opdialog");
    let a = ctx.animate_bool_with_time(id.with("in"), true, 0.14);
    let modal = egui::Modal::new(id)
        .area(
            egui::Modal::default_area(id)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, 2.0 * (1.0 - a))),
        )
        .frame(egui::Frame::NONE)
        .backdrop_color(alpha(tk.plot, 0.75 * a));
    let mut esc = false;
    let r = modal.show(ctx, |ui| {
        ui.multiply_opacity(a);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let p = ui.painter();
        p.add(paint::shadow_pop().as_shape(rect, CornerRadius::same(size::RADIUS_MD)));
        p.rect(
            rect,
            CornerRadius::same(size::RADIUS_MD),
            tk.panel,
            Stroke::new(1.0, tk.border_strong),
            StrokeKind::Inside,
        );
        let mut ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
        );
        ui.set_clip_rect(rect);
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        // ── header ──
        let head_h = if op.total > 0 {
            32.0 + 10.0 + 14.0
        } else {
            32.0
        } + 24.0;
        let (hr, _) = ui.allocate_exact_size(Vec2::new(w, head_h), Sense::hover());
        ui.painter()
            .hline(hr.x_range(), hr.bottom() - 0.5, Stroke::new(1.0, tk.border));
        let inner = hr.shrink2(Vec2::new(16.0, 12.0));
        let top = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 32.0));
        let ib = Rect::from_min_size(top.min, Vec2::splat(32.0));
        ui.painter().rect(
            ib,
            CornerRadius::same(size::RADIUS_MD),
            tk.bg,
            Stroke::new(1.0, tk.border),
            StrokeKind::Inside,
        );
        let ir = Rect::from_center_size(ib.center(), Vec2::splat(size::ICON));
        if running {
            Spinner::paint(&ui, ir, tk.accent);
        } else if op.failed > 0 || op.hint.is_some() {
            lucide::CIRCLE_X.paint(ui.painter(), ir, tk.err);
        } else {
            lucide::CIRCLE_CHECK.paint(ui.painter(), ir, tk.ok);
        }
        // Figures at the right: elapsed, succeeded, failed.
        let figs = [
            (t("progress", "elapsed"), clock(elapsed), tk.fg),
            (t("progress", "succeeded"), ok_count.to_string(), tk.ok),
            (
                t("progress", "failedLabel"),
                fail_count.to_string(),
                if fail_count > 0 { tk.err } else { tk.fg_muted },
            ),
        ];
        let mut x = top.right();
        for (label, value, ink) in figs.iter().rev() {
            let lg = crate::kit::panel::stencil(&ui, label, Color32::PLACEHOLDER, None);
            let vg = ph(&ui, value, font(Face::MonoSemibold, size::TEXT_SM));
            let fw = lg.size().x.max(vg.size().x);
            draw(
                ui.painter(),
                &lg,
                x - lg.size().x,
                top.top() + 8.0,
                tk.fg_faint,
            );
            draw(ui.painter(), &vg, x - vg.size().x, top.bottom() - 8.0, *ink);
            x -= fw + 20.0;
        }
        let tx = ib.right() + 12.0;
        let tw = (x - tx).max(40.0);
        let tg = ph_trunc(
            &ui,
            kind_label(op.kind),
            font(Face::Display, size::TEXT_LG),
            tw,
        );
        draw(ui.painter(), &tg, tx, top.top() + 9.0, tk.fg);
        let status = status_line(op);
        let sink = if op.hint.is_some() {
            tk.err
        } else if !running && op.failed == 0 {
            tk.ok
        } else {
            tk.fg_muted
        };
        let sg = ph_trunc(&ui, &status, font(Face::Sans, size::TEXT_XS), tw);
        draw(ui.painter(), &sg, tx, top.bottom() - 6.0, sink);
        if op.total > 0 {
            let row = Rect::from_min_size(
                Pos2::new(inner.left(), top.bottom() + 10.0),
                Vec2::new(inner.width(), 14.0),
            );
            let lg = crate::kit::panel::stencil(
                &ui,
                t("progress", "overall"),
                Color32::PLACEHOLDER,
                None,
            );
            draw(ui.painter(), &lg, row.left(), row.center().y, tk.fg_faint);
            let pct = format!("{}%", (overall * 100.0).round() as i64);
            let rest = format!(" · {}/{}", op.completed.len(), op.total);
            let rg = ph(&ui, &rest, font(Face::Mono, size::TEXT_2XS));
            let pg = ph(&ui, &pct, font(Face::Mono, size::TEXT_2XS));
            draw(
                ui.painter(),
                &rg,
                row.right() - rg.size().x,
                row.center().y,
                tk.fg_faint,
            );
            draw(
                ui.painter(),
                &pg,
                row.right() - rg.size().x - pg.size().x,
                row.center().y,
                tk.fg,
            );
            let mr = Rect::from_min_max(
                Pos2::new(row.left() + lg.size().x + 12.0, row.center().y - 3.0),
                Pos2::new(row.right() - 96.0 - 12.0, row.center().y + 3.0),
            );
            ui.put(
                mr,
                Meter::new(overall as f32)
                    .size(MeterSize::Md)
                    .tone(if fail_count > 0 {
                        Tone::Warn
                    } else {
                        Tone::Accent
                    })
                    .width(mr.width()),
            );
        }

        // ── footer (laid out before the body so the body knows its height) ──
        let note = if op.via == ModOpVia::Steamworks {
            t("progress", "steamworksFooter")
        } else {
            t("mods", "confirmWarning")
        };
        let note_w = (w - 32.0 - 12.0 - st.buttons_w).max(40.0);
        let note_g = paint::galley_wrap(
            &ui,
            note,
            font(Face::Sans, size::TEXT_2XS),
            tk.fg_faint,
            note_w,
        );
        let foot_h = (size::CONTROL + 16.0).max(note_g.size().y + 16.0);
        let fr = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - foot_h), rect.max);
        ui.painter()
            .hline(fr.x_range(), fr.top() + 0.5, Stroke::new(1.0, tk.border));

        // ── body: the left column, then the log ──
        let body = Rect::from_min_max(
            Pos2::new(rect.left(), hr.bottom()),
            Pos2::new(rect.right(), fr.top()),
        );
        let left_w = if show_log {
            352.0_f32.min(body.width())
        } else {
            body.width()
        };
        let lr = Rect::from_min_size(body.min, Vec2::new(left_w, body.height()));
        ui.painter()
            .rect_filled(lr, CornerRadius::ZERO, alpha(tk.bg, 0.4));
        if show_log {
            ui.painter()
                .vline(lr.right() - 0.5, lr.y_range(), Stroke::new(1.0, tk.border));
        }
        let mut left = ui.new_child(
            UiBuilder::new()
                .max_rect(lr.shrink(12.0))
                .layout(Layout::top_down(Align::Min))
                .id_salt("op-left"),
        );
        left.spacing_mut().item_spacing = Vec2::new(6.0, 12.0);
        aside(
            &mut left,
            cx,
            st,
            op,
            running,
            progress,
            speed,
            eta,
            item_elapsed,
            guard_left,
            current_done,
            &queue,
            run,
        );
        if show_log {
            let rr = Rect::from_min_max(Pos2::new(lr.right(), body.top()), body.max);
            let mut lui = ui.new_child(UiBuilder::new().max_rect(rr).id_salt("op-log"));
            lui.set_clip_rect(rr);
            let title = if op.via == ModOpVia::Steamworks {
                "$ steam (steamworks)"
            } else {
                "$ steamcmd"
            };
            LogPane::new(LOG_ID, &st.lines)
                .times(&st.times)
                .title(title)
                .live(running)
                .show(&mut lui);
        }

        // ── footer content ──
        let mut f = ui.new_child(
            UiBuilder::new()
                .max_rect(fr.shrink2(Vec2::new(16.0, 8.0)))
                .layout(Layout::right_to_left(Align::Center))
                .id_salt("op-foot"),
        );
        f.spacing_mut().item_spacing.x = 4.0;
        if running {
            if f.add(
                Button::new(t("progress", "cancel"))
                    .icon(lucide::X)
                    .variant(Variant::Danger),
            )
            .clicked()
            {
                run.push(("mods.op.cancel", Value::Null));
            }
            if f.add(
                Button::new(t("progress", "minimise"))
                    .icon(lucide::MINIMIZE_2)
                    .variant(Variant::Ghost),
            )
            .clicked()
            {
                run.push(("mods.op.minimise", Value::Null));
            }
        } else if f
            .add(Button::new(t("progress", "dismiss")).variant(Variant::Accent))
            .clicked()
        {
            run.push(("mods.op.dismiss", Value::Null));
        }
        let (icon, word) = if show_log {
            (lucide::PANEL_RIGHT_CLOSE, "hideLog")
        } else {
            (lucide::PANEL_RIGHT_OPEN, "showLog")
        };
        if f.add(
            Button::new(t("progress", word))
                .icon(icon)
                .variant(Variant::Ghost),
        )
        .clicked()
        {
            st.hide_log = !st.hide_log;
        }
        let used = f.min_rect().width();
        if (used - st.buttons_w).abs() > 0.5 {
            st.buttons_w = used;
            ctx.request_repaint();
        }
        let g = note_g;
        let nr = Rect::from_min_size(
            Pos2::new(fr.left() + 16.0, fr.center().y - g.size().y / 2.0),
            g.size(),
        );
        f.painter().galley(nr.min, g, tk.fg_faint);
        esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
    });
    // Escape (or the shell asking it to close): hidden while it runs, dismissed after.
    // A click outside is ignored.
    if esc || (r.should_close() && ctx.input(|i| i.key_pressed(egui::Key::Escape))) {
        let id = if running {
            "mods.op.minimise"
        } else {
            "mods.op.dismiss"
        };
        if !run.iter().any(|(r, _)| *r == id) {
            run.push((id, Value::Null));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn aside(
    ui: &mut Ui,
    cx: &Ctx,
    st: &mut OpUi,
    op: &ModOp,
    running: bool,
    progress: Option<Progress>,
    speed: Option<f64>,
    eta: Option<f64>,
    item_elapsed: f64,
    guard_left: f64,
    current_done: bool,
    queue: &[QRow],
    run: &mut Vec<(&'static str, Value)>,
) {
    let tk = ui.tokens();
    let w = ui.available_width();
    if op.phase == ModOpPhase::SteamGuardMobile {
        callout(ui, tk.warn, |ui| {
            heading(
                ui,
                lucide::SHIELD_ALERT,
                tk.warn,
                t("progress", "steamguardTitle"),
                &tf(
                    "progress",
                    "steamguardDesc",
                    &[("app", &t("progress", "steamguardApp"))],
                ),
            );
            ui.add_space(4.0);
            for (i, k) in ["steamguardStep1", "steamguardStep2", "steamguardStep3"]
                .iter()
                .enumerate()
            {
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                let c =
                    Rect::from_min_size(Pos2::new(r.left(), r.center().y - 8.0), Vec2::splat(16.0));
                let ink = if i == 2 { tk.ok } else { tk.warn };
                ui.painter().circle_filled(c.center(), 8.0, alpha(ink, 0.2));
                let n = ph(
                    ui,
                    &(i + 1).to_string(),
                    font(Face::MonoSemibold, size::TEXT_3XS),
                );
                paint::text_center(ui.painter(), &n, c, ink);
                let g = ph_trunc(
                    ui,
                    t("progress", k),
                    font(Face::Sans, size::TEXT_2XS),
                    r.width() - 24.0,
                );
                draw(ui.painter(), &g, r.left() + 24.0, r.center().y, tk.fg_muted);
            }
            ui.add_space(4.0);
            let (r, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
            let wg = ph(
                ui,
                t("progress", "steamguardWaiting"),
                font(Face::Sans, size::TEXT_2XS),
            );
            draw(ui.painter(), &wg, r.left(), r.center().y, tk.fg_faint);
            let ink = if guard_left <= 15.0 {
                tk.err
            } else if guard_left <= 30.0 {
                tk.warn
            } else {
                tk.fg_muted
            };
            let cg = ph(
                ui,
                &clock(guard_left * 1000.0),
                font(Face::MonoSemibold, size::TEXT_2XS),
            );
            draw(
                ui.painter(),
                &cg,
                r.right() - cg.size().x,
                r.center().y,
                ink,
            );
            ui.add(
                Meter::new((guard_left / GUARD_S) as f32).tone(if guard_left <= 15.0 {
                    Tone::Err
                } else {
                    Tone::Warn
                }),
            );
            if guard_left <= 0.0 {
                small(ui, t("progress", "steamguardTimeout"), tk.err);
            } else if guard_left <= 15.0 {
                small(ui, t("progress", "steamguardHurry"), tk.err);
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(1000));
        });
    }

    if op.phase == ModOpPhase::PasswordRequired {
        let manual = manual_cmd(cx, st.steamcmd_home.as_deref());
        callout(ui, tk.warn, |ui| {
            if st.distrust {
                let (r, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
                lucide::TERMINAL.paint(
                    ui.painter(),
                    Rect::from_center_size(
                        Pos2::new(r.left() + 8.0, r.center().y),
                        Vec2::splat(size::ICON),
                    ),
                    tk.info,
                );
                let g = ph(
                    ui,
                    t("progress", "manualTitle"),
                    font(Face::SansSemibold, size::TEXT_SM),
                );
                draw(ui.painter(), &g, r.left() + 24.0, r.center().y, tk.fg);
                small(ui, t("progress", "manualCopied"), tk.fg_muted);
                let cw = ui.available_width();
                let cg = paint::galley_wrap(
                    ui,
                    &manual,
                    font(Face::Mono, size::TEXT_2XS),
                    tk.fg,
                    cw - 16.0,
                );
                let (cr, _) =
                    ui.allocate_exact_size(Vec2::new(cw, cg.size().y + 12.0), Sense::hover());
                ui.painter().rect(
                    cr,
                    CornerRadius::same(size::RADIUS_SM),
                    tk.bg,
                    Stroke::new(1.0, tk.border),
                    StrokeKind::Inside,
                );
                ui.painter().galley(cr.min + Vec2::new(8.0, 6.0), cg, tk.fg);
                for (i, k) in ["manualStep1", "manualStep2", "manualStep3", "manualStep4"]
                    .iter()
                    .enumerate()
                {
                    small(ui, &format!("{}. {}", i + 1, t("progress", k)), tk.fg_muted);
                }
                let nw = ui.available_width();
                let ng = paint::galley_wrap(
                    ui,
                    t("progress", "manualNote"),
                    font(Face::Sans, size::TEXT_3XS),
                    tk.fg_faint,
                    nw,
                );
                let (nr, _) = ui.allocate_exact_size(Vec2::new(nw, ng.size().y), Sense::hover());
                ui.painter().galley(nr.min, ng, tk.fg_faint);
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), size::CONTROL),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        if ui
                            .add(
                                Button::new(t("progress", "manualConfirm"))
                                    .icon(lucide::CHECK)
                                    .variant(Variant::Accent),
                            )
                            .clicked()
                        {
                            run.push(("mods.op.cancel", Value::Null));
                        }
                    },
                );
            } else {
                heading(
                    ui,
                    lucide::KEY_ROUND,
                    tk.warn,
                    t("progress", "passwordTitle"),
                    t("progress", "passwordDesc"),
                );
                let mut send = false;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let bw = 28.0 + 80.0;
                    let r = ui.add(
                        Input::new(&mut st.password)
                            .kind(if st.reveal {
                                InputKind::Text
                            } else {
                                InputKind::Password
                            })
                            .placeholder(t("progress", "passwordPlaceholder"))
                            .enabled(!st.sending)
                            .width((ui.available_width() - bw - 12.0).max(60.0))
                            .id_salt("mods-op-password"),
                    );
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        send = true;
                    }
                    let eye = if st.reveal {
                        lucide::EYE_OFF
                    } else {
                        lucide::EYE
                    };
                    if ui
                        .add(
                            crate::kit::IconButton::new(eye, t("progress", "showPassword"))
                                .size(Size::Icon)
                                .variant(Variant::Ghost),
                        )
                        .clicked()
                    {
                        st.reveal = !st.reveal;
                    }
                    if ui
                        .add(
                            Button::new(t("progress", "passwordSend"))
                                .icon(lucide::SEND)
                                .spinner(st.sending)
                                .variant(Variant::Accent)
                                .enabled(!st.password.is_empty() && !st.sending),
                        )
                        .clicked()
                    {
                        send = true;
                    }
                });
                if send && !st.password.is_empty() && !st.sending {
                    st.sending = true;
                    let pw = std::mem::take(&mut st.password);
                    run.push(("mods.op.send_password", json!({ "input": pw })));
                }
                ui.allocate_ui_with_layout(
                    Vec2::new(ui.available_width(), size::CONTROL),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        if ui
                            .add(
                                Button::new(t("progress", "dontTrust"))
                                    .icon(lucide::SHIELD_OFF)
                                    .icon_size(12.0)
                                    .size(Size::Xs)
                                    .variant(Variant::Danger),
                            )
                            .clicked()
                        {
                            run.push(("mods.op.manual_login", Value::Null));
                        }
                    },
                );
            }
        });
    }

    if let Some(hint) = &op.hint {
        callout(ui, tk.err, |ui| {
            let hw = ui.available_width();
            let g = paint::galley_wrap(ui, hint, font(Face::Mono, size::TEXT_2XS), tk.err, hw);
            let (r, _) = ui.allocate_exact_size(Vec2::new(hw, g.size().y), Sense::hover());
            ui.painter().galley(r.min, g, tk.err);
        });
    }

    // The mod being worked on: its bytes, speed and time left.
    if running && op.phase == ModOpPhase::Downloading && op.current_id != 0 && !current_done {
        let start = ui.cursor().top();
        let slot = ui.painter().add(egui::Shape::Noop);
        ui.add(TransferProgress {
            label: t("progress", "itemProgress"),
            name: &op.current_name,
            percent: progress.map(|p| p.percent),
            done: progress.map(|p| p.done),
            total: progress.map(|p| p.total),
            speed,
            eta,
            elapsed: Some(item_elapsed),
            labels: TransferLabels {
                size: t("mods", "colSize"),
                speed: t("progress", "speed"),
                eta: "ETA",
                elapsed: t("progress", "elapsed"),
            },
        });
        let r = Rect::from_min_max(
            Pos2::new(ui.max_rect().left(), start),
            Pos2::new(ui.max_rect().right(), ui.cursor().top() - 12.0),
        );
        let _ = slot;
        let _ = r;
    }

    // The queue: every mod by name, with where it is. A bounded scroller.
    if !queue.is_empty() {
        let rest = ui.available_rect_before_wrap();
        let qh = rest.height().max(128.0);
        let (qr, _) = ui.allocate_exact_size(Vec2::new(w, qh), Sense::hover());
        ui.painter().rect(
            qr,
            CornerRadius::same(size::RADIUS_MD),
            tk.panel,
            Stroke::new(1.0, tk.border),
            StrokeKind::Inside,
        );
        let head = Rect::from_min_size(qr.min, Vec2::new(qr.width(), 28.0));
        ui.painter().hline(
            head.x_range(),
            head.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.7)),
        );
        let lg = crate::kit::panel::stencil(ui, t("progress", "queue"), Color32::PLACEHOLDER, None);
        draw(
            ui.painter(),
            &lg,
            head.left() + 12.0,
            head.center().y,
            tk.fg_faint,
        );
        let cnt = format!("{}/{}", op.completed.len(), queue.len());
        let cg = ph(ui, &cnt, font(Face::Mono, size::TEXT_2XS));
        draw(
            ui.painter(),
            &cg,
            head.right() - 12.0 - cg.size().x,
            head.center().y,
            tk.fg_faint,
        );
        let lr = Rect::from_min_max(
            Pos2::new(qr.left() + 1.0, head.bottom()),
            qr.max - Vec2::splat(1.0),
        );
        let mut lui = ui.new_child(UiBuilder::new().max_rect(lr).id_salt("op-queue"));
        lui.set_clip_rect(lr);
        let cur = queue.iter().position(|q| q.state == QState::Downloading);
        const QROW: f32 = 40.0;
        let mut area = egui::ScrollArea::vertical()
            .id_salt("op-queue-scroll")
            .auto_shrink([false, false]);
        // The row being downloaded stays in sight as the queue moves on.
        let key = Id::new("op-queue-cur");
        let last: Option<usize> = ui.data(|d| d.get_temp(key));
        if cur != last {
            ui.data_mut(|d| d.insert_temp(key, cur));
            if let Some(i) = cur {
                let top = i as f32 * QROW;
                area = area.vertical_scroll_offset((top - lr.height() / 2.0 + QROW / 2.0).max(0.0));
            }
        }
        area.show_rows(&mut lui, QROW, queue.len(), |ui, range| {
            for i in range {
                let Some(q) = queue.get(i) else {
                    continue;
                };
                queue_row(
                    ui,
                    cx,
                    q,
                    progress.filter(|_| !current_done),
                    i + 1 < queue.len(),
                );
            }
        });
    }
}

fn queue_row(ui: &mut Ui, cx: &Ctx, q: &QRow, progress: Option<Progress>, rule: bool) {
    let tk = ui.tokens();
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
    let p = ui.painter();
    if rule {
        p.hline(
            r.x_range(),
            r.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.5)),
        );
    }
    let fade = if q.state == QState::Queued { 0.6 } else { 1.0 };
    if q.state == QState::Downloading {
        p.rect_filled(r, CornerRadius::ZERO, alpha(tk.accent, 0.08));
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(r.left(), r.top() + 4.0),
                Pos2::new(r.left() + 2.0, r.bottom() - 4.0),
            ),
            CornerRadius::same(1),
            tk.accent,
        );
    }
    let ir = Rect::from_center_size(
        Pos2::new(r.left() + 12.0 + 7.0, r.center().y),
        Vec2::splat(size::ICON_SM),
    );
    let (icon, ink): (Option<Icon>, Color32) = match q.state {
        QState::Downloading => (None, tk.accent),
        QState::Done => (Some(lucide::CHECK), tk.ok),
        QState::Failed => (Some(lucide::X), tk.err),
        QState::Queued => (Some(lucide::CIRCLE), tk.fg_faint),
    };
    match icon {
        Some(i) => i.paint(p, ir, ink.gamma_multiply(fade)),
        None => Spinner::paint(ui, ir, ink),
    }
    // The figure at the right.
    let (fig, fink) = match q.state {
        QState::Downloading => (
            progress.map_or_else(
                || "…".to_string(),
                |p| format!("{}%", p.percent.floor() as i64),
            ),
            tk.accent,
        ),
        QState::Done => (clock(q.ms as f64), tk.fg_faint),
        QState::Failed => (t("progress", "itemFailed").to_string(), tk.err),
        QState::Queued => (t("progress", "itemQueued").to_string(), tk.fg_faint),
    };
    let fg = ph(ui, &fig, font(Face::Mono, size::TEXT_2XS));
    let fx = r.right() - 12.0 - fg.size().x;
    draw(
        ui.painter(),
        &fg,
        fx,
        r.center().y,
        fink.gamma_multiply(fade),
    );
    let tx = ir.right() + 8.0;
    let tw = (fx - 8.0 - tx).max(8.0);
    let face = if q.state == QState::Downloading {
        Face::SansSemibold
    } else {
        Face::Sans
    };
    let ng = ph_trunc(ui, &q.name, font(face, size::TEXT_XS), tw);
    draw(
        ui.painter(),
        &ng,
        tx,
        r.center().y - 7.0,
        tk.fg.gamma_multiply(fade),
    );
    let mut sub = q.id.to_string();
    if let Some(m) = cx.stores.mods.by_id(q.id) {
        sub.push_str(&format!(" · {}", format::bytes(m.size as f64)));
    }
    let sg = ph_trunc(ui, &sub, font(Face::Mono, size::TEXT_3XS), tw);
    draw(
        ui.painter(),
        &sg,
        tx,
        r.center().y + 8.0,
        tk.fg_faint.gamma_multiply(fade),
    );
    if crate::parts::is_cut(&ng, &q.name) {
        let _ = crate::kit::TipExt::tip(
            ui.interact(r, ui.id().with(("q", q.id)), Sense::hover()),
            &q.name,
        );
    }
}

/// A tinted box (`rounded-md border border-<tone>/35 bg-<tone>/8 p-3`).
fn callout(ui: &mut Ui, tone: Color32, body: impl FnOnce(&mut Ui)) {
    let slot = ui.painter().add(egui::Shape::Noop);
    let r = egui::Frame::new()
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            body(ui);
        });
    ui.painter().set(
        slot,
        egui::epaint::RectShape::new(
            r.response.rect,
            CornerRadius::same(size::RADIUS_MD),
            alpha(tone, 0.08),
            Stroke::new(1.0, alpha(tone, 0.35)),
            StrokeKind::Inside,
        ),
    );
}

/// An icon, a title and a sentence under it.
fn heading(ui: &mut Ui, icon: Icon, tone: Color32, title: &str, desc: &str) {
    let tk = ui.tokens();
    let w = ui.available_width();
    let tg = ph(ui, title, font(Face::SansSemibold, size::TEXT_SM));
    let dg = paint::galley_wrap(
        ui,
        desc,
        font(Face::Sans, size::TEXT_2XS),
        tk.fg_muted,
        w - 30.0,
    );
    let h = tg.size().y + 2.0 + dg.size().y;
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    icon.paint(
        ui.painter(),
        Rect::from_min_size(
            Pos2::new(r.left(), r.top() + 2.0),
            Vec2::splat(size::ICON_LG),
        ),
        tone,
    );
    draw(
        ui.painter(),
        &tg,
        r.left() + 30.0,
        r.top() + tg.size().y / 2.0,
        tk.fg,
    );
    ui.painter().galley(
        Pos2::new(r.left() + 30.0, r.top() + tg.size().y + 2.0),
        dg,
        tk.fg_muted,
    );
}

fn small(ui: &mut Ui, text: &str, color: Color32) {
    let w = ui.available_width();
    let g = paint::galley_wrap(ui, text, font(Face::Sans, size::TEXT_2XS), color, w);
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y), Sense::hover());
    ui.painter().galley(r.min, g, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(op: &mut ModOp, line: &str) {
        op.log.push_back((op.log.len() as u64, line.to_string()));
        op.log_rev += 1;
    }

    #[test]
    fn the_pane_follows_the_log_line_by_line() {
        let mut op = ModOp::idle();
        let mut st = OpUi::default();
        push(&mut op, "a");
        push(&mut op, "progress: 1");
        sync_lines(&mut st, &op);
        assert_eq!(st.lines, ["a", "progress: 1"]);
        // The live line is rewritten, a line comes, the front is dropped.
        if let Some(last) = op.log.back_mut() {
            last.1 = "progress: 2".into();
        }
        push(&mut op, "b");
        op.log.pop_front();
        op.log_dropped += 1;
        op.log_rev += 1;
        sync_lines(&mut st, &op);
        assert_eq!(st.lines, ["progress: 2", "b"]);
        assert_eq!(st.times.len(), st.lines.len());
        // Another operation: copied afresh.
        let mut next = ModOp::idle();
        next.started_at = op.started_at + std::time::Duration::from_secs(1);
        push(&mut next, "c");
        sync_lines(&mut st, &next);
        assert_eq!(st.lines, ["c"]);
    }
}
