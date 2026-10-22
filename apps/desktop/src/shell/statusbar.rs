//! The status bar (= `StatusBar.svelte` and `status/*.svelte`): the window's instrument
//! panel in two groups. On the left, what is happening: the server list, the ping scan,
//! the mods' downloads and their state, the offline mode's install, the launcher's own
//! download; on the right, the state of things: the game, BattlEye, Steam, an update, the
//! version. Every segment has the same anatomy ([`Segment`]); a narrow bar drops the words,
//! then the least needed segments, by its own width.
//!
//! Nothing here ticks for nothing: ages and the game's time are re-read every 30 s (one
//! repaint asked for then), the Steam client and BattlEye are asked with that clock while
//! the window has focus (every 5 s for BattlEye while DayZ runs); a sweep or a pulse
//! repaints at 30 fps only while it shows, and not at all with motion off.

use std::time::Duration;

use egui::{Color32, CornerRadius, Id, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::app::{Ctx, reduced_motion};
use crate::i18n::{format, t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{Align, Brand, MenuItem, Side, TipExt, menu, paint};
use crate::rt::{self, Task};
use crate::store::mods::{ModOpPhase, ModOpVia};
use crate::store::pad::InputMode;
use crate::store::servers::RefreshPhase;
use crate::store::updater::UpdateState;
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};
use crate::views::ViewId;
use dz_game::battleye::BattlEyeState;
use dz_hub::features::steamcmd::ModOpType;
use dz_hub::features::steamworks::ModDownloaderDto;

/// The bar's clock: ages, the game's time, Steam's and BattlEye's state.
const CLOCK: f64 = 30.0;
/// BattlEye is asked this often while DayZ runs.
const BE_EVERY: f64 = 5.0;
/// How long a running game may go without BattlEye's service before it is a fault.
const BE_GRACE: f64 = 20.0;
/// No flash while the window opens.
const ARM_AFTER: f64 = 1.5;
/// A flash's length (`animate-flash`).
const FLASH: f64 = 0.9;
/// Frames per second of a sweep or a pulse.
const ANIM_FPS: f32 = 30.0;

/// Words go below this bar width; then segments, as `class` says.
const WORDS_MIN: f32 = 1100.0;

/// The phase of Steam's listing (mirror of the hub's private `Phase`).
#[derive(serde::Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum SourcePhase {
    Idle,
    Running,
    Done,
    Failed,
    Off,
}

/// What the bar keeps between frames.
#[derive(Default)]
pub(crate) struct StatusState {
    opened_at: Option<f64>,
    next_clock: f64,
    steam: Option<Task<Option<bool>>>,
    steam_running: Option<bool>,
    be_next: f64,
    /// When the game was first seen running (egui time).
    game_seen: Option<f64>,
    flash_total: Option<(u32, f64)>,
    flash_behind: Option<(usize, f64)>,
}

/// How a segment reads at a glance (= `Segment.svelte`'s `Tone`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Idle,
    Accent,
    Info,
    Ok,
    Warn,
    Err,
}

impl Tone {
    fn icon(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Idle => t.fg_faint,
            Tone::Accent => t.accent,
            Tone::Info => t.info,
            Tone::Ok => t.ok,
            Tone::Warn => t.warn,
            Tone::Err => t.err,
        }
    }

    fn text(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Warn => t.warn,
            Tone::Err => t.err,
            Tone::Ok => t.ok,
            _ => t.fg_muted,
        }
    }
}

#[derive(Clone, Copy)]
enum SegIcon {
    Icon(Icon),
    Steam,
}

/// One span of a segment's text.
struct Piece {
    text: String,
    mono: bool,
    /// Its own colour (else the segment's).
    color: Option<Color32>,
    /// A word: hidden on a narrow bar.
    word: bool,
    /// Cut past this width.
    max_w: Option<f32>,
    /// A dot rather than text.
    dot: Option<Color32>,
    /// Flashes since (egui time).
    flash: Option<f64>,
}

impl Piece {
    fn text(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            mono: false,
            color: None,
            word: false,
            max_w: None,
            dot: None,
            flash: None,
        }
    }
    fn mono(s: impl Into<String>) -> Self {
        Self {
            mono: true,
            ..Self::text(s)
        }
    }
    fn word(mut self) -> Self {
        self.word = true;
        self
    }
    fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
    fn max(mut self, w: f32) -> Self {
        self.max_w = Some(w);
        self
    }
    fn flash(mut self, at: Option<f64>) -> Self {
        self.flash = at;
        self
    }
    fn dot(c: Color32) -> Self {
        Self {
            dot: Some(c),
            ..Self::text("")
        }
    }
}

/// What a click on a segment (or one of its small buttons) does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    Refresh,
    GoServers,
    Scan,
    ScanPause,
    ReopenOp,
    Go(ViewId, Option<&'static str>),
    RepairBe,
    SteamMenu,
    Command(&'static str),
}

/// Bar progress: none, a sweep (nothing to measure), or a fraction 0–1.
#[derive(Clone, Copy, PartialEq)]
enum Progress {
    None,
    Sweep,
    At(f32),
}

struct Segment {
    icon: SegIcon,
    tone: Tone,
    tip: String,
    act: Option<Act>,
    progress: Progress,
    pulse: bool,
    pieces: Vec<Piece>,
    /// Hidden below this bar width.
    hide_below: f32,
    /// Small icon buttons beside it: icon, label, action.
    actions: Vec<(Icon, &'static str, Act)>,
}

impl Segment {
    fn new(icon: SegIcon, tone: Tone, tip: String) -> Self {
        Self {
            icon,
            tone,
            tip,
            act: None,
            progress: Progress::None,
            pulse: false,
            pieces: Vec::new(),
            hide_below: 0.0,
            actions: Vec::new(),
        }
    }
    fn act(mut self, a: Act) -> Self {
        self.act = Some(a);
        self
    }
    fn act_if(mut self, a: Option<Act>) -> Self {
        self.act = a;
        self
    }
    fn progress(mut self, p: Progress) -> Self {
        self.progress = p;
        self
    }
    fn pulse(mut self, on: bool) -> Self {
        self.pulse = on;
        self
    }
    fn piece(mut self, p: Piece) -> Self {
        self.pieces.push(p);
        self
    }
    fn hide_below(mut self, w: f32) -> Self {
        self.hide_below = w;
        self
    }
}

pub(super) fn show(ui: &mut Ui, cx: &mut Ctx, st: &mut StatusState) {
    let tk = ui.tokens();
    let rect = ui.max_rect();
    let now = ui.input(|i| i.time);
    // (The top border is the panel's separator line.)
    clock(ui, cx, st, now);

    let bar_w = rect.width();
    let words = bar_w >= WORDS_MIN;
    let cy = rect.center().y - 0.5;

    let left = left_segments(cx, st, now, &tk);
    let right = right_segments(cx, st, now, &tk);

    // Right group, from the right edge inwards; the version last.
    let mut x = rect.right() - 4.0;
    let mut acts: Vec<(Act, Rect)> = Vec::new();
    if bar_w >= 1000.0 {
        let v = tf("nav", "version", &[("version", &dz_hub::VERSION)]);
        let g = paint::galley(ui, &v, font(Face::Mono, size::TEXT_2XS), tk.fg_faint);
        x -= g.size().x + 16.0;
        paint::text_at(ui.painter(), &g, x + 8.0, cy, tk.fg_faint);
        x -= 2.0;
    }
    let shown_right: Vec<&Segment> = right.iter().filter(|s| bar_w >= s.hide_below).collect();
    let mut widths = Vec::with_capacity(shown_right.len());
    for s in &shown_right {
        widths.push(width(ui, s, words));
    }
    let mut placed = Vec::with_capacity(shown_right.len());
    for (s, w) in shown_right.iter().zip(&widths).rev() {
        x -= w;
        placed.push((*s, x));
        x -= 2.0;
    }
    let right_start = x;
    for (s, sx) in placed.into_iter().rev() {
        let (a, resp) = draw(ui, s, sx, cy, words, now, &tk);
        acts.extend(a);
        if s.act == Some(Act::SteamMenu) {
            ui.ctx().data_mut(|d| {
                d.insert_temp(
                    Id::new(("dzch.shell.menu", "steam")),
                    egui::Popup::default_response_id(&resp),
                )
            });
            steam_menu(ui, cx, &resp);
        }
    }
    // The divider.
    let div_x = right_start - 9.0;
    ui.painter().vline(
        div_x + 0.5,
        (cy - 7.0)..=(cy + 7.0),
        Stroke::new(1.0, tk.border),
    );
    // Left group, clipped to what is left.
    let clip = Rect::from_min_max(
        pos2(rect.left(), rect.top()),
        pos2(div_x - 4.0, rect.bottom()),
    );
    let mut lui = ui.new_child(egui::UiBuilder::new().max_rect(clip).id_salt("sb-left"));
    lui.set_clip_rect(clip.intersect(ui.clip_rect()));
    let mut x = rect.left() + 4.0;
    for s in left.iter().filter(|s| bar_w >= s.hide_below) {
        let w = width(&lui, s, words);
        let (a, _) = draw(&mut lui, s, x, cy, words, now, &tk);
        acts.extend(a);
        x += w + 2.0;
    }

    // Animations: a sweep or a pulse asks for its next frame; a flash until it fades.
    let moving = !reduced_motion(ui.ctx())
        && left
            .iter()
            .chain(right.iter())
            .any(|s| s.progress == Progress::Sweep || s.pulse);
    if moving {
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f32(1.0 / ANIM_FPS));
    }
    for at in [st.flash_total.map(|f| f.1), st.flash_behind.map(|f| f.1)]
        .into_iter()
        .flatten()
    {
        if now - at < FLASH {
            ui.ctx().request_repaint();
        }
    }

    for (a, _) in acts {
        run(cx, a);
    }
}

/// The bar's clock, and what it asks for.
fn clock(ui: &Ui, cx: &mut Ctx, st: &mut StatusState, now: f64) {
    let ctx = ui.ctx();
    let opened = *st.opened_at.get_or_insert(now);
    if now - opened < ARM_AFTER {
        ctx.request_repaint_after(Duration::from_secs_f64(ARM_AFTER - (now - opened) + 0.01));
    }
    if let Some(task) = &mut st.steam
        && let Some(r) = task.poll()
    {
        st.steam = None;
        st.steam_running = r;
    }
    let focused = cx.stores.app.focused;
    let running = cx.stores.game.running;
    if now >= st.next_clock {
        st.next_clock = now + CLOCK;
        if focused && cx.stores.app.initialized && st.steam.is_none() {
            let hub = cx.hub.clone();
            st.steam = Some(rt::spawn(async move {
                hub.steamworks_status().await.ok().map(|s| s.steam_running)
            }));
        }
    }
    ctx.request_repaint_after(Duration::from_secs_f64((st.next_clock - now).max(0.05)));
    // BattlEye: with the clock while focused, every few seconds while DayZ runs.
    if running {
        if st.game_seen.is_none() {
            st.game_seen = Some(now);
            st.be_next = now;
        }
    } else {
        st.game_seen = None;
    }
    let every = if running { BE_EVERY } else { CLOCK };
    if cx.stores.app.initialized && (focused || running) && now >= st.be_next {
        st.be_next = now + every;
        cx.stores.battleye.refresh();
    }
    if running || focused {
        ctx.request_repaint_after(Duration::from_secs_f64((st.be_next - now).max(0.05)));
    }
    // Flashes: a figure that changes after the window opened.
    let armed = now - opened >= ARM_AFTER;
    let total = cx.stores.servers.total;
    match st.flash_total {
        Some((v, _)) if v == total => {}
        _ => st.flash_total = Some((total, if armed { now } else { f64::NEG_INFINITY })),
    }
    let behind = cx.stores.mods.stale_count();
    match st.flash_behind {
        Some((v, _)) if v == behind => {}
        _ => st.flash_behind = Some((behind, if armed { now } else { f64::NEG_INFINITY })),
    }
}

fn left_segments(cx: &Ctx, st: &StatusState, now: f64, tk: &Tokens) -> Vec<Segment> {
    let mut out = Vec::with_capacity(4);
    out.push(list_segment(cx, st, tk));
    if let Some(s) = scan_segment(cx) {
        out.push(s);
    }
    out.extend(mod_segments(cx, st, now));
    if crate::views::offline::installing(cx.egui) {
        let tip = t("shell", "sbOfflineTip").to_string();
        out.push(
            Segment::new(SegIcon::Icon(lucide::MAP), Tone::Info, tip)
                .act(Act::Go(ViewId::Offline, None))
                .progress(Progress::Sweep)
                .piece(Piece::text(t("shell", "sbOffline")).word())
                .piece(Piece::text(t("offline", "installing"))),
        );
    }
    let u = &cx.stores.updater;
    if u.state == UpdateState::Downloading {
        let v = u.info.as_ref().map_or("", |i| i.version.as_str());
        let tip = tf(
            "shell",
            "sbUpdatingTip",
            &[("version", &v), ("percent", &u.percent())],
        );
        let mut s = Segment::new(SegIcon::Icon(lucide::DOWNLOAD), Tone::Info, tip)
            .act(Act::Command("about.open_update"))
            .progress(if u.total > 0 {
                Progress::At(u.percent() as f32 / 100.0)
            } else {
                Progress::Sweep
            })
            .piece(Piece::text(t("shell", "sbUpdating")).word());
        if u.total > 0 {
            s = s.piece(Piece::mono(format!("{}%", u.percent())));
        }
        out.push(s);
    }
    out
}

/// The server list, always there (= `ListSegment.svelte`).
fn list_segment(cx: &Ctx, st: &StatusState, tk: &Tokens) -> Segment {
    let s = &cx.stores.servers;
    let down = s.list_error.is_some() && s.total == 0;
    let age = if s.last_refreshed > 0 {
        format::relative(s.last_refreshed / 1000)
    } else {
        String::new()
    };
    let failed = s
        .last_refresh
        .as_ref()
        .filter(|_| !s.refreshing)
        .filter(|r| r.phase == RefreshPhase::Failed)
        .map(|r| r.error.clone().unwrap_or_default());
    let steam = cx
        .stores
        .sources
        .now
        .as_ref()
        .filter(|n| {
            crate::store::mirror::<_, SourcePhase>(&n.steam.phase) == Some(SourcePhase::Running)
        })
        .map(|n| (n.steam.requests_done, n.steam.requests_total, n.steam.added));
    let (tone, tip, act, progress, note) = if s.refreshing {
        (
            Tone::Accent,
            t("shell", "sbFetchingTip").to_string(),
            None,
            Progress::Sweep,
            t("shell", "sbFetching").to_string(),
        )
    } else if down {
        let e = s.list_error.clone().unwrap_or_default();
        (
            Tone::Err,
            tf("shell", "sbListFailedTip", &[("error", &e)]),
            Some(Act::Refresh),
            Progress::None,
            t("nav", "sbListDown").to_string(),
        )
    } else if let Some(e) = failed {
        (
            Tone::Err,
            tf("shell", "sbListFailedTip", &[("error", &e)]),
            Some(Act::Refresh),
            Progress::None,
            t("shell", "sbListFailed").to_string(),
        )
    } else if let Some((done, total, added)) = steam {
        let tip = tf(
            "shell",
            "sbSteamListingTip",
            &[
                ("done", &format::num(f64::from(done))),
                ("total", &format::num(f64::from(total))),
                ("added", &format::num(f64::from(added))),
            ],
        );
        let (progress, note) = if total > 0 {
            let f = done as f32 / total as f32;
            (
                Progress::At(f),
                format!("Steam {}%", (f * 100.0).floor() as u32),
            )
        } else {
            (Progress::Sweep, "Steam".to_string())
        };
        (Tone::Accent, tip, Some(Act::GoServers), progress, note)
    } else {
        let stale = s.stale();
        let key = if stale { "sbListStaleTip" } else { "sbListTip" };
        let tip = tf(
            "shell",
            key,
            &[("count", &format::num(f64::from(s.total))), ("age", &age)],
        );
        (
            if stale { Tone::Warn } else { Tone::Idle },
            tip,
            Some(Act::Refresh),
            Progress::None,
            age,
        )
    };
    let icon = if down {
        lucide::CLOUD_OFF
    } else {
        lucide::SERVER
    };
    let mut seg = Segment::new(SegIcon::Icon(icon), tone, tip)
        .act_if(act)
        .progress(progress);
    if !down {
        seg = seg
            .piece(Piece::mono(format::num(f64::from(s.total))).flash(st.flash_total.map(|f| f.1)))
            .piece(Piece::text(t("shell", "sbServersWord")).word());
    }
    if !note.is_empty() {
        let mut p = if down {
            Piece::text(note)
        } else {
            Piece::text(format!("· {note}")).word()
        };
        if tone == Tone::Idle {
            p = p.color(tk.fg_faint);
        }
        seg = seg.piece(p);
    }
    seg
}

/// The ping scan (= `ScanSegment.svelte`).
fn scan_segment(cx: &Ctx) -> Option<Segment> {
    let s = &cx.stores.servers;
    if let Some((done, total)) = s.scan {
        let paused = s.scan_paused;
        let args: [(&str, &dyn std::fmt::Display); 2] = [
            ("done", &format::num(f64::from(done))),
            ("total", &format::num(f64::from(total))),
        ];
        let tip = tf(
            "shell",
            if paused {
                "sbPingPausedTip"
            } else {
                "sbPingTip"
            },
            &args,
        );
        let mut seg = Segment::new(
            SegIcon::Icon(lucide::RADAR),
            if paused { Tone::Warn } else { Tone::Accent },
            tip,
        )
        .pulse(!paused)
        .progress(if total > 0 {
            Progress::At(done as f32 / total as f32)
        } else {
            Progress::None
        })
        .piece(Piece::text(t("nav", if paused { "scanPaused" } else { "sbPing" })).word())
        .piece(Piece::mono(format!(
            "{}/{}",
            format::num(f64::from(done)),
            format::num(f64::from(total))
        )));
        seg.actions = vec![
            if paused {
                (lucide::PLAY, t("nav", "resumeScan"), Act::ScanPause)
            } else {
                (lucide::PAUSE, t("nav", "pauseScan"), Act::ScanPause)
            },
            (lucide::ROTATE_CCW, t("nav", "sbRescan"), Act::Scan),
        ];
        Some(seg)
    } else if s.total > 0 && !s.refreshing {
        Some(
            Segment::new(
                SegIcon::Icon(lucide::RADAR),
                Tone::Idle,
                t("nav", "sbRescan").to_string(),
            )
            .act(Act::Scan)
            .piece(Piece::text(t("nav", "sbPing")).word()),
        )
    } else {
        None
    }
}

/// The mods' side of the work (= `ModSegments.svelte`).
fn mod_segments(cx: &Ctx, st: &StatusState, _now: f64) -> Vec<Segment> {
    let mut out = Vec::new();
    let mods = &cx.stores.mods;
    if mods.busy() {
        let op = &mods.op;
        let waiting = matches!(
            op.phase,
            ModOpPhase::SteamGuardMobile | ModOpPhase::PasswordRequired
        );
        let kind = match op.kind {
            ModOpType::Login => t("progress", "kindLogin"),
            ModOpType::Repair => t("progress", "kindRepair"),
            ModOpType::InstallServer | ModOpType::InstallManual => t("progress", "kindInstall"),
            _ => t("progress", "kindUpdate"),
        };
        let via = if op.via == ModOpVia::Steamworks {
            "Steam"
        } else {
            "SteamCMD"
        };
        let tip = if waiting {
            t("shell", "sbModWaitingTip").to_string()
        } else {
            tf(
                "shell",
                "sbModOpTip",
                &[
                    ("kind", &kind),
                    ("via", &via),
                    ("name", &op.current_name),
                    ("current", &op.current),
                    ("total", &op.total),
                ],
            )
        };
        let mut seg = Segment::new(
            SegIcon::Icon(if waiting {
                lucide::KEY_ROUND
            } else {
                lucide::HARD_DRIVE_DOWNLOAD
            }),
            if waiting { Tone::Warn } else { Tone::Info },
            tip,
        )
        .act(Act::ReopenOp)
        .pulse(!waiting && op.total == 0)
        .progress(if waiting {
            Progress::None
        } else if op.total > 0 {
            Progress::At((op.ok + op.failed) as f32 / op.total as f32)
        } else {
            Progress::Sweep
        });
        if waiting {
            seg = seg.piece(Piece::text(t("shell", "sbModWaiting")));
        } else {
            seg = seg.piece(Piece::text(kind).word());
            if op.total > 0 {
                seg = seg.piece(Piece::mono(format!("{}/{}", op.current, op.total)));
            }
            seg = seg.piece(
                Piece::text(op.current_name.clone())
                    .max(160.0)
                    .color(Color32::PLACEHOLDER),
            );
        }
        out.push(seg);
    } else {
        let missing = !cx.stores.profile.via_steam()
            && cx
                .stores
                .servers
                .stats
                .as_ref()
                .is_some_and(|s| !s.has_steamcmd);
        if missing {
            out.push(
                Segment::new(
                    SegIcon::Icon(lucide::TRIANGLE_ALERT),
                    Tone::Warn,
                    t("shell", "sbSteamcmdMissingTip").to_string(),
                )
                .act(Act::Command("settings.open_steam"))
                .piece(Piece::text(t("nav", "sbSteamcmdMissing")).word()),
            );
        }
    }
    let behind = mods.stale_count();
    if mods.checking {
        out.push(
            Segment::new(
                SegIcon::Icon(lucide::PUZZLE),
                Tone::Info,
                t("shell", "sbModsCheckingTip").to_string(),
            )
            .progress(Progress::Sweep)
            .hide_below(800.0)
            .piece(Piece::text(t("nav", "sbModsChecking")).word()),
        );
    } else if behind > 0 {
        let count = i64::try_from(behind).unwrap_or(i64::MAX);
        out.push(
            Segment::new(
                SegIcon::Icon(lucide::PUZZLE),
                Tone::Warn,
                tf("shell", "sbModsBehindTip", &[("count", &count)]),
            )
            .act(Act::Go(ViewId::Mods, None))
            .hide_below(800.0)
            .piece(Piece::mono(behind.to_string()).flash(st.flash_behind.map(|f| f.1)))
            .piece(Piece::text(t("shell", "sbModsBehind")).word()),
        );
    }
    out
}

fn right_segments(cx: &Ctx, st: &StatusState, now: f64, tk: &Tokens) -> Vec<Segment> {
    let mut out = Vec::with_capacity(4);
    let pad = cx.stores.pad.mode(cx.prefs.get().ui.pad_mode) == InputMode::Gamepad;
    // The game: an indicator only.
    if cx.stores.game.running {
        let played = cx
            .stores
            .game
            .since
            .map(|s| format::duration(i64::try_from(s.elapsed().as_secs()).unwrap_or(0)));
        let first = match &played {
            Some(p) => tf("shell", "sbGameFor", &[("time", p)]),
            None => t("shell", "sbGameOn").to_string(),
        };
        let quit = t("shell", if pad { "sbGameQuitPad" } else { "sbGameQuit" });
        let mut s = Segment::new(
            SegIcon::Icon(lucide::GAMEPAD_2),
            Tone::Ok,
            format!("{first} {quit}"),
        )
        .piece(Piece::dot(tk.ok))
        .piece(Piece::text("DayZ").word());
        if let Some(p) = played {
            s = s.piece(Piece::mono(p).color(tk.fg_muted));
        }
        out.push(s);
    }
    if let Some(s) = battleye_segment(cx, st, now, tk) {
        out.push(s);
    }
    out.push(steam_segment(cx, st, tk));
    let u = &cx.stores.updater;
    match (u.state, &u.info) {
        (UpdateState::Available, Some(info)) => {
            let tip = tf("shell", "sbUpdateTip", &[("version", &info.version)]);
            out.push(
                Segment::new(SegIcon::Icon(lucide::DOWNLOAD), Tone::Ok, tip)
                    .act(Act::Command("about.open_update"))
                    .piece(Piece::mono(tf(
                        "nav",
                        "sbUpdate",
                        &[("version", &info.version)],
                    ))),
            );
        }
        (UpdateState::Done, _) => out.push(
            Segment::new(
                SegIcon::Icon(lucide::REFRESH_CW),
                Tone::Ok,
                t("shell", "sbUpdateReadyTip").to_string(),
            )
            .act(Act::Command("about.open_update"))
            .piece(Piece::text(t("shell", "sbUpdateReady")).word()),
        ),
        _ => {}
    }
    out
}

/// BattlEye (= `BattlEyeSegment.svelte`).
fn battleye_segment(cx: &Ctx, st: &StatusState, now: f64, tk: &Tokens) -> Option<Segment> {
    let b = &cx.stores.battleye;
    let status = b.status.as_ref()?;
    let be = status.state;
    if be == BattlEyeState::NoGame {
        return None;
    }
    let service = status.service;
    let repairing = b.repairing;
    let not_started = service
        && be != BattlEyeState::Running
        && st.game_seen.is_some_and(|seen| now - seen > BE_GRACE);
    let broken = matches!(be, BattlEyeState::Missing | BattlEyeState::Disabled);
    let tone = if repairing {
        Tone::Info
    } else if not_started {
        Tone::Err
    } else if broken {
        Tone::Warn
    } else {
        Tone::Idle
    };
    let icon = if not_started {
        lucide::SHIELD_X
    } else if broken {
        lucide::SHIELD_ALERT
    } else {
        lucide::SHIELD_CHECK
    };
    let tip = if repairing {
        t("shell", "sbBeRepairing").to_string()
    } else {
        let first = if not_started {
            t("shell", "sbBeNotStarted")
        } else {
            match be {
                BattlEyeState::Running => t("shell", "sbBeRunning"),
                BattlEyeState::Disabled => t("shell", "sbBeDisabled"),
                BattlEyeState::Missing if service => t("shell", "sbBeMissing"),
                BattlEyeState::Missing => t("shell", "sbBeMissingProton"),
                _ if service => t("shell", "sbBeReady"),
                _ => t("shell", "sbBeReadyProton"),
            }
        };
        let action = if be == BattlEyeState::Running && !not_started {
            ""
        } else if service {
            t("shell", "sbBeRepairAction")
        } else if broken {
            t("shell", "sbBeInstallAction")
        } else {
            ""
        };
        if action.is_empty() {
            first.to_string()
        } else {
            format!("{first} {action}")
        }
    };
    let acts = !repairing && be != BattlEyeState::Running && (service || broken);
    let mut seg = Segment::new(SegIcon::Icon(icon), tone, tip)
        .act_if(acts.then_some(Act::RepairBe))
        .progress(if repairing {
            Progress::Sweep
        } else {
            Progress::None
        })
        .hide_below(if not_started || broken { 0.0 } else { 900.0 })
        .piece(Piece::text("BattlEye").word());
    seg = if not_started || broken {
        seg.piece(Piece::text(t("shell", "sbBeOff")))
    } else {
        seg.piece(Piece::dot(if be == BattlEyeState::Running {
            tk.ok
        } else {
            tk.fg_faint
        }))
    };
    Some(seg)
}

/// Steam (= `SteamSegment.svelte`): what downloads the mods, whether the client runs.
fn steam_segment(cx: &Ctx, st: &StatusState, tk: &Tokens) -> Segment {
    let via = cx.stores.profile.via_steam();
    let login = cx
        .stores
        .profile
        .data
        .as_ref()
        .and_then(|p| p.steamcmd_logged_in.clone());
    let running = st.steam_running;
    let mut tip = if via {
        t("shell", "sbSteamClient").to_string()
    } else if let Some(l) = &login {
        tf("shell", "sbSteamCmdAs", &[("login", l)])
    } else {
        t("shell", "sbSteamCmdNoLogin").to_string()
    };
    match running {
        Some(true) => {
            tip.push(' ');
            tip.push_str(t("shell", "sbSteamUp"));
        }
        Some(false) => {
            tip.push(' ');
            tip.push_str(t("shell", "sbSteamDown"));
        }
        None => {}
    }
    tip.push(' ');
    tip.push_str(t("shell", "sbSteamAction"));
    Segment::new(
        SegIcon::Steam,
        if running == Some(false) {
            Tone::Warn
        } else {
            Tone::Idle
        },
        tip,
    )
    .act(Act::SteamMenu)
    .hide_below(900.0)
    .piece(Piece::text(if via { "Steam" } else { "SteamCMD" }).word())
    .piece(Piece::dot(match running {
        Some(true) => tk.ok,
        Some(false) => tk.warn,
        None => tk.fg_faint,
    }))
}

fn piece_galley(ui: &Ui, p: &Piece, tone: Tone, ink: Color32) -> std::sync::Arc<egui::Galley> {
    let f = font(piece_face(p, tone), size::TEXT_2XS);
    match p.max_w {
        Some(w) => paint::galley_trunc(ui, &p.text, f, ink, w),
        None => paint::galley(ui, &p.text, f, ink),
    }
}

/// A piece's face: mono for figures, medium on good news (`text-ok font-medium`).
fn piece_face(p: &Piece, tone: Tone) -> Face {
    match (p.mono, tone == Tone::Ok) {
        (true, true) => Face::MonoMedium,
        (true, false) => Face::Mono,
        (false, true) => Face::SansMedium,
        (false, false) => Face::Sans,
    }
}

/// The segment's own width (its small buttons included).
fn width(ui: &Ui, s: &Segment, words: bool) -> f32 {
    let mut w = 8.0 + 12.0;
    for p in s.pieces.iter().filter(|p| words || !p.word) {
        w += 6.0;
        w += match p.dot {
            Some(_) => 6.0,
            None => piece_galley(ui, p, s.tone, Color32::WHITE).size().x,
        };
    }
    w += 8.0;
    if !s.actions.is_empty() {
        w += 20.0 * s.actions.len() as f32 + 4.0;
    }
    w
}

/// Draws a segment with its left edge at `x`; returns the action clicked, with the
/// segment's rect (for a menu to open from).
fn draw(
    ui: &mut Ui,
    s: &Segment,
    x: f32,
    cy: f32,
    words: bool,
    now: f64,
    tk: &Tokens,
) -> (Option<(Act, Rect)>, egui::Response) {
    let total_w = width(ui, s, words);
    let actions_w = if s.actions.is_empty() {
        0.0
    } else {
        20.0 * s.actions.len() as f32 + 4.0
    };
    let body = Rect::from_min_size(pos2(x, cy - 11.0), vec2(total_w - actions_w, 22.0));
    let id = Id::new((
        "sb-seg",
        match s.icon {
            SegIcon::Icon(i) => i.name,
            SegIcon::Steam => "steam",
        },
    ));
    let sense = if s.act.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(body, id, sense);
    let lit = s.act.is_some() && (resp.hovered() || menu::is_open(&resp));
    let h = paint::ease(ui, id.with("h"), lit);
    let p = ui.painter();
    p.rect_filled(
        body,
        CornerRadius::same(size::RADIUS_XS),
        paint::lerp(Color32::TRANSPARENT, tk.raised, h),
    );
    let still = reduced_motion(ui.ctx());
    let pulse = if s.pulse && !still {
        // `animate-pulse`: opacity 1 → .5 → 1 over 2 s.
        let ph = (now % 2.0) / 2.0;
        1.0 - 0.5 * (std::f64::consts::PI * ph).sin().powi(2) as f32
    } else {
        1.0
    };
    let icon_ink = s.tone.icon(tk).gamma_multiply(pulse);
    let text_ink = if s.act.is_some() {
        paint::lerp(s.tone.text(tk), tk.fg, h)
    } else {
        s.tone.text(tk)
    };
    let mut px = body.left() + 8.0;
    let ir = Rect::from_min_size(pos2(px, cy - 6.0), Vec2::splat(12.0));
    match s.icon {
        SegIcon::Icon(i) => i.paint(p, ir, icon_ink),
        SegIcon::Steam => Brand::Steam.paint(p, ir, icon_ink),
    }
    px += 12.0;
    for piece in s.pieces.iter().filter(|p| words || !p.word) {
        px += 6.0;
        if let Some(c) = piece.dot {
            p.circle_filled(pos2(px + 3.0, cy), 3.0, c);
            px += 6.0;
            continue;
        }
        let ink = match piece.color {
            Some(c) if c == Color32::PLACEHOLDER => tk.fg_faint,
            Some(c) => c,
            None => text_ink,
        };
        let g = piece_galley(ui, piece, s.tone, ink);
        if let Some(at) = piece.flash {
            let k = ((now - at) / FLASH) as f32;
            if (0.0..1.0).contains(&k) && !still {
                let fr = Rect::from_min_size(
                    pos2(px - 2.0, cy - g.size().y / 2.0),
                    g.size() + vec2(4.0, 0.0),
                );
                p.rect_filled(
                    fr,
                    CornerRadius::same(size::RADIUS_XS),
                    paint::alpha(tk.accent, 0.35 * (1.0 - k)),
                );
            }
        }
        paint::text_at(p, &g, px, cy, ink);
        px += g.size().x;
    }
    // The bar along the foot.
    let bar = Rect::from_min_max(
        pos2(x + 8.0, cy + 11.0 - 2.0),
        pos2(x + total_w - 8.0, cy + 11.0),
    );
    let fill = match s.tone {
        Tone::Idle => tk.fg_faint,
        other => other.icon(tk),
    };
    match s.progress {
        Progress::None => {}
        Progress::At(f) => {
            p.rect_filled(bar, CornerRadius::same(1), tk.raised);
            let f = ui.ctx().animate_value_with_time(
                id.with("bar"),
                f.clamp(0.0, 1.0),
                crate::theme::motion::BASE,
            );
            let r = Rect::from_min_size(bar.min, vec2(bar.width() * f, bar.height()));
            p.rect_filled(r, CornerRadius::same(1), fill);
        }
        Progress::Sweep => {
            p.rect_filled(bar, CornerRadius::same(1), tk.raised);
            let band = bar.width() / 3.0;
            // `animate-sweep`: from -100% to 300% of the band in 1.6 s.
            let k = if still {
                0.0
            } else {
                ((now % 1.6) / 1.6) as f32
            };
            let left = bar.left() - band + k * (bar.width() + band);
            let r = Rect::from_min_max(
                pos2(left.max(bar.left()), bar.top()),
                pos2((left + band).min(bar.right()), bar.bottom()),
            );
            if r.width() > 0.0 {
                p.rect_filled(r, CornerRadius::same(1), fill);
            }
        }
    }
    let mut clicked = None;
    // The small buttons beside it (`size-5`, icons 12).
    let mut bx = body.right();
    for (i, (icon, label, act)) in s.actions.iter().enumerate() {
        let r = Rect::from_min_size(pos2(bx, cy - 10.0), Vec2::splat(20.0));
        let b = ui.interact(r, id.with(("act", i)), Sense::click());
        let bh = paint::ease(ui, b.id, b.hovered());
        let p = ui.painter();
        p.rect_filled(
            r,
            CornerRadius::same(size::RADIUS_XS),
            paint::lerp(Color32::TRANSPARENT, tk.raised, bh),
        );
        icon.paint(
            p,
            Rect::from_center_size(r.center(), Vec2::splat(12.0)),
            paint::lerp(tk.fg_faint, tk.fg, bh),
        );
        if b.tip_kbd(label, "", Side::Top).clicked() {
            clicked = Some((*act, r));
        }
        bx += 20.0;
    }
    let resp = if menu::is_open(&resp) {
        resp
    } else {
        resp.tip_kbd(&s.tip, "", Side::Top)
    };
    if let (Some(a), true) = (s.act, resp.clicked())
        && a != Act::SteamMenu
    {
        clicked = Some((a, body));
    }
    (clicked, resp)
}

fn run(cx: &mut Ctx, a: Act) {
    match a {
        Act::Refresh => {
            let _ = cx.run("servers.refresh", &serde_json::Value::Null);
        }
        Act::GoServers => cx.go(ViewId::Servers),
        Act::Scan => {
            let _ = cx.run("servers.scan", &serde_json::Value::Null);
        }
        Act::ScanPause => {
            let _ = cx.run("servers.scan_pause", &serde_json::Value::Null);
        }
        Act::ReopenOp => {
            let _ = cx.run("mods.reopen_op", &serde_json::Value::Null);
        }
        Act::Go(v, Some(f)) => cx.go_focus(v, f),
        Act::Go(v, None) => cx.go(v),
        Act::RepairBe => {
            let _ = cx.run("battleye.repair", &serde_json::Value::Null);
        }
        Act::Command(id) => {
            let _ = cx.run(id, &serde_json::Value::Null);
        }
        Act::SteamMenu => {}
    }
}

/// The Steam segment's menu: what downloads the mods, and the Steam settings.
fn steam_menu(ui: &Ui, cx: &mut Ctx, resp: &egui::Response) {
    let tk = ui.tokens();
    let current = cx
        .stores
        .profile
        .data
        .as_ref()
        .map_or(ModDownloaderDto::Steamcmd, |p| p.mod_downloader);
    let busy = cx.stores.mods.busy();
    let mut pick: Option<ModDownloaderDto> = None;
    let mut settings = false;
    menu::dropdown(resp, Side::Top, Align::End, |ui| {
        ui.set_min_width(240.0 - 8.0);
        menu::label(ui, t("nav", "sbDownloader"));
        for (value, label, hint) in [
            (
                ModDownloaderDto::Steamcmd,
                "SteamCMD",
                t("nav", "sbSteamcmdHint"),
            ),
            (
                ModDownloaderDto::Steamworks,
                t("settings", "downloaderSteam"),
                t("nav", "sbSteamHint"),
            ),
        ] {
            let on = current == value;
            let enabled = !(busy && !on);
            let w = ui.available_width().max(232.0);
            let (r, item) = ui.allocate_exact_size(
                vec2(w, 36.0),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let lit = enabled && item.hovered();
            let p = ui.painter();
            if lit {
                p.rect_filled(r, CornerRadius::same(size::RADIUS_XS), tk.raised);
            }
            let a = if enabled { 1.0 } else { paint::DISABLED };
            if on {
                lucide::CHECK.paint(
                    p,
                    Rect::from_center_size(
                        pos2(r.left() + 6.0 + 7.0, r.center().y),
                        Vec2::splat(12.0),
                    ),
                    tk.accent.gamma_multiply(a),
                );
            }
            let tx = r.left() + 6.0 + 14.0 + 8.0;
            let ink = if on || lit { tk.fg } else { tk.fg_muted };
            let g = paint::galley(
                ui,
                label,
                font(Face::Sans, size::TEXT_XS),
                ink.gamma_multiply(a),
            );
            paint::text_at(p, &g, tx, r.top() + 4.0 + 8.0, ink.gamma_multiply(a));
            let hint = if busy && !on {
                t("nav", "sbDownloaderBusy")
            } else {
                hint
            };
            let hg = paint::galley_trunc(
                ui,
                hint,
                font(Face::Sans, size::TEXT_3XS),
                tk.fg_faint,
                (r.right() - 6.0 - tx).max(0.0),
            );
            paint::text_at(
                p,
                &hg,
                tx,
                r.bottom() - 4.0 - 7.0,
                tk.fg_faint.gamma_multiply(a),
            );
            if item.clicked() && !on {
                pick = Some(value);
            }
        }
        menu::separator(ui);
        if ui
            .add(MenuItem::new(t("nav", "sbSteamSettings")).icon(lucide::SETTINGS))
            .clicked()
        {
            settings = true;
        }
    });
    if let Some(v) = pick {
        cx.stores.profile.set_mod_downloader(v);
    }
    if settings {
        cx.go_focus(ViewId::Settings, "steam");
    }
}
