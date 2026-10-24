//! A tool's output, live: `LogPane` (each line with the time it arrived, coloured by what it
//! says, filterable, copyable, following the end while the reader is at the end), and what
//! reads a log (`log.ts`): [`classify_line`], [`Rate`].
//!
//! The pane keeps each line's kind and the filtered index between frames, extended as lines
//! arrive: a frame classifies only the new lines and lays out only the rows on screen.

use std::collections::VecDeque;
use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{
    Align, Color32, CornerRadius, Id, Layout, Pos2, Rect, Response, Sense, Stroke, Ui, UiBuilder,
    Vec2,
};

use super::text::{clock, draw, ph, ph_trunc};
use crate::i18n::t;
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, FieldSize, Input, Side, Size, Tag, TipExt, Variant};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// What a log line says, by the words tools use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    Err,
    Warn,
    Ok,
    Progress,
    Login,
    Dim,
    Normal,
}

impl LineKind {
    /// Its colour (`LINE_CLASS`).
    pub fn color(self, t: &Tokens) -> Color32 {
        match self {
            LineKind::Err => t.err,
            LineKind::Warn => t.warn,
            LineKind::Ok => t.ok,
            LineKind::Progress => t.info,
            LineKind::Login => t.accent,
            LineKind::Dim => t.fg_faint,
            LineKind::Normal => t.fg_muted,
        }
    }
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `\bword\b` somewhere in `s`.
fn has_word(s: &str, word: &str) -> bool {
    let b = s.as_bytes();
    let mut from = 0;
    while let Some(i) = s.get(from..).and_then(|r| r.find(word)) {
        let at = from + i;
        let end = at + word.len();
        let before = at == 0 || !b.get(at - 1).copied().is_some_and(is_word_byte);
        let after = !b.get(end).copied().is_some_and(is_word_byte);
        if before && after {
            return true;
        }
        from = at + 1;
    }
    false
}

/// `\d+\s*%`.
fn has_percent(s: &str) -> bool {
    let b = s.as_bytes();
    for (i, c) in b.iter().enumerate() {
        if *c == b'%' {
            let mut j = i;
            while j > 0 && b.get(j - 1).is_some_and(u8::is_ascii_whitespace) {
                j -= 1;
            }
            if j > 0 && b.get(j - 1).is_some_and(u8::is_ascii_digit) {
                return true;
            }
        }
    }
    false
}

/// What kind of line a tool wrote (`classifyLine`).
pub fn classify_line(line: &str) -> LineKind {
    let l = line.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if has_word(&l, "error")
        || any(&[
            "failed",
            "failure",
            "abort",
            "not found",
            "invalid password",
            "access denied",
            "no connection",
            "timeout expired",
        ])
    {
        return LineKind::Err;
    }
    if any(&[
        "warning",
        "timed out",
        "retry",
        "steam guard",
        "two-factor",
        "rate limit",
    ]) {
        return LineKind::Warn;
    }
    let ends_ok = {
        let s = l.strip_suffix('.').unwrap_or(&l);
        s.ends_with("ok") && {
            let b = s.as_bytes();
            b.len() == 2 || !b.get(b.len() - 3).copied().is_some_and(is_word_byte)
        }
    };
    if ends_ok
        || any(&[
            "success",
            "already up to date",
            "fully installed",
            "logged in ok",
            "downloaded item",
        ])
    {
        return LineKind::Ok;
    }
    if has_percent(&l)
        || any(&[
            "downloading",
            "update state",
            "reconfiguring",
            "validating",
            "progress:",
        ])
    {
        return LineKind::Progress;
    }
    if l.starts_with("steam>")
        || any(&[
            "logging in",
            "+login",
            "connecting",
            "loading steam",
            "steamcmd",
        ])
    {
        return LineKind::Login;
    }
    let trimmed = l.trim();
    if trimmed.is_empty()
        || l.starts_with('[')
        || any(&["appinfo", "waiting on", "idle"])
        || trimmed.bytes().all(|b| b.is_ascii_digit())
    {
        return LineKind::Dim;
    }
    LineKind::Normal
}

/// Bytes per second over the last few seconds of samples, so one burst does not swing the
/// figure. A sample that goes backwards (a new file) resets it.
#[derive(Clone, Debug)]
pub struct Rate {
    samples: VecDeque<(f64, f64)>,
    window_ms: f64,
}

impl Default for Rate {
    fn default() -> Self {
        Self::new(5000.0)
    }
}

impl Rate {
    pub fn new(window_ms: f64) -> Self {
        Self {
            samples: VecDeque::new(),
            window_ms,
        }
    }

    /// A sample: `t` in ms, `bytes` so far.
    pub fn add(&mut self, t: f64, bytes: f64) {
        if self.samples.back().is_some_and(|l| bytes < l.1) {
            self.samples.clear();
        }
        self.samples.push_back((t, bytes));
        while self.samples.len() > 2
            && self
                .samples
                .front()
                .is_some_and(|f| t - f.0 > self.window_ms)
        {
            self.samples.pop_front();
        }
    }

    /// Bytes per second, or `None` before there are two samples a moment apart.
    pub fn per_second(&self) -> Option<f64> {
        let (a, b) = (self.samples.front()?, self.samples.back()?);
        if b.0 - a.0 < 400.0 {
            return None;
        }
        Some((b.1 - a.1) * 1000.0 / (b.0 - a.0))
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }
}

// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct LogState {
    filter: String,
    /// The kind of every line seen, in order.
    kinds: Vec<LineKind>,
    /// What the kinds were read from: line count and the first line's hash.
    seen_first: u64,
    /// Indices of the lines the filter keeps, and what it was built for.
    shown: Vec<u32>,
    shown_for: (String, usize),
    copied_at: Option<f64>,
    follow: bool,
    started: bool,
    /// A command asked to go back to the end (`log.follow`).
    jump: bool,
    /// The offset the next frame's scroller starts at (going back to the end).
    scroll_to: Option<f32>,
}

type Shared = Arc<Mutex<LogState>>;

fn hash_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = rustc_hash::FxHasher::default();
    s.hash(&mut h);
    h.finish()
}

impl LogState {
    /// Brings the kinds and the filtered index up to `lines`.
    fn sync(&mut self, lines: &[String], classify: fn(&str) -> LineKind) {
        let first = lines.first().map_or(0, |l| hash_str(l));
        if lines.len() < self.kinds.len() || first != self.seen_first {
            self.kinds.clear();
            self.shown.clear();
            self.shown_for = (String::new(), 0);
            self.seen_first = first;
        }
        for l in lines.iter().skip(self.kinds.len()) {
            self.kinds.push(classify(l));
        }
        let q = self.filter.trim().to_lowercase();
        if self.shown_for.0 != q {
            self.shown.clear();
            self.shown_for = (q.clone(), 0);
        }
        for (i, l) in lines.iter().enumerate().skip(self.shown_for.1) {
            if q.is_empty() || l.to_lowercase().contains(&q) {
                self.shown.push(u32::try_from(i).unwrap_or(u32::MAX));
            }
        }
        self.shown_for.1 = lines.len();
    }
}

/// The lines as the Copy button writes them: `[+m:ss] line` when times are known.
pub fn copy_text(lines: &[String], times: Option<&[f64]>) -> String {
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if let Some(ts) = times {
            out.push_str(&format!("[+{}] ", clock(ts.get(i).copied().unwrap_or(0.0))));
        }
        out.push_str(l);
    }
    out
}

fn state_of(ctx: &egui::Context, id: Id) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(id, || {
            Arc::new(Mutex::new(LogState {
                follow: true,
                ..Default::default()
            }))
        })
        .clone()
    })
}

/// `LogPane`: a tool's output, live. Each line with the time it arrived (`times`, ms since
/// the start, parallel to `lines`), coloured by what it says, filterable, copyable. It
/// follows the end while the reader is at the end; scrolling up stops it, and a button
/// brings it back. Lines are one row each (a long one is cut; its hint has it whole).
pub struct LogPane<'a> {
    id: Id,
    lines: &'a [String],
    times: Option<&'a [f64]>,
    title: &'a str,
    live: bool,
    classify: fn(&str) -> LineKind,
}

/// One row of the log (`text-[11px] leading-[1.55]`).
const LINE_H: f32 = 17.0;

impl<'a> LogPane<'a> {
    pub fn new(id_salt: impl egui::AsIdSalt, lines: &'a [String]) -> Self {
        Self {
            id: Id::unique(id_salt),
            lines,
            times: None,
            title: "log",
            live: false,
            classify: classify_line,
        }
    }

    pub fn times(mut self, times: &'a [f64]) -> Self {
        self.times = Some(times);
        self
    }

    pub fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    /// Still being written: a LIVE mark and a cursor.
    pub fn live(mut self, on: bool) -> Self {
        self.live = on;
        self
    }

    pub fn classify(mut self, f: fn(&str) -> LineKind) -> Self {
        self.classify = f;
        self
    }

    /// `log.filter`: the filter of the pane shown under `id_salt` (applied as it draws).
    pub fn set_filter(ctx: &egui::Context, id_salt: impl egui::AsIdSalt, text: &str) {
        let st = state_of(ctx, Id::unique(id_salt));
        st.lock().filter = text.chars().take(256).collect();
        ctx.request_repaint();
    }

    /// `log.follow`: back to the end, following it.
    pub fn follow(ctx: &egui::Context, id_salt: impl egui::AsIdSalt) {
        let st = state_of(ctx, Id::unique(id_salt));
        st.lock().jump = true;
        ctx.request_repaint();
    }

    /// `log.copy`: copy the lines (the pane's button says "Copied" a moment).
    pub fn copy(
        ctx: &egui::Context,
        id_salt: impl egui::AsIdSalt,
        lines: &[String],
        times: Option<&[f64]>,
    ) {
        ctx.copy_text(copy_text(lines, times));
        let st = state_of(ctx, Id::unique(id_salt));
        st.lock().copied_at = Some(ctx.input(|i| i.time));
        ctx.request_repaint();
    }

    /// Fills the rect it is given.
    #[allow(clippy::too_many_lines)]
    pub fn show(self, ui: &mut Ui) -> Response {
        let tk = ui.tokens();
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::hover());
        let state: Shared = state_of(ui.ctx(), self.id);
        let mut st = state.lock();
        st.sync(self.lines, self.classify);
        ui.painter().rect_filled(rect, CornerRadius::ZERO, tk.plot);

        // Header.
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0));
        ui.painter().hline(
            head.x_range(),
            head.bottom() - 0.5,
            Stroke::new(1.0, tk.border),
        );
        let mut hui = ui.new_child(
            UiBuilder::new()
                .max_rect(head.shrink2(Vec2::new(12.0, 6.0)))
                .layout(Layout::left_to_right(Align::Center))
                .id_salt(self.id.with("head")),
        );
        hui.spacing_mut().item_spacing.x = 8.0;
        lucide::TERMINAL.show(&mut hui, size::ICON_SM, tk.fg_faint);
        let tg = paint::tracked(
            &hui,
            self.title,
            font(Face::Mono, size::TEXT_2XS),
            Color32::PLACEHOLDER,
            0.1,
            true,
            None,
        );
        let (tr, _) = hui.allocate_exact_size(tg.size(), Sense::hover());
        draw(hui.painter(), &tg, tr.left(), tr.center().y, tk.fg_faint);
        hui.add(Tag::new(&self.lines.len().to_string()));
        if self.live {
            let time = hui.input(|i| i.time);
            let pulse = 0.75 + 0.25 * ((time / 2.0).fract() as f32 * std::f32::consts::TAU).cos();
            let lg = ph(&hui, t("common", "live"), font(Face::Mono, size::TEXT_3XS));
            let (lr, _) =
                hui.allocate_exact_size(Vec2::new(10.0 + lg.size().x, 12.0), Sense::hover());
            hui.painter().circle_filled(
                Pos2::new(lr.left() + 3.0, lr.center().y),
                3.0,
                tk.ok.gamma_multiply(pulse),
            );
            draw(hui.painter(), &lg, lr.left() + 10.0, lr.center().y, tk.ok);
            hui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        hui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let now = ui.input(|i| i.time);
            let copied = st.copied_at.is_some_and(|at| now - at < 1.5);
            if copied {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
            }
            let b = if copied {
                Button::new(t("common", "copied")).icon(lucide::CHECK)
            } else {
                Button::new(t("common", "copy")).icon(lucide::COPY)
            };
            if ui
                .add(b.variant(Variant::Ghost).size(Size::Xs).icon_size(12.0))
                .clicked()
            {
                ui.ctx().copy_text(copy_text(self.lines, self.times));
                st.copied_at = Some(now);
            }
            ui.add(
                Input::new(&mut st.filter)
                    .search()
                    .placeholder(t("common", "filterLines"))
                    .size(FieldSize::Xs)
                    .width(192.0)
                    .id_salt(self.id.with("filter")),
            );
        });
        st.sync(self.lines, self.classify);

        // Body.
        let body = Rect::from_min_max(Pos2::new(rect.left(), head.bottom()), rect.max);
        let mut bui = ui.new_child(
            UiBuilder::new()
                .max_rect(body)
                .layout(Layout::top_down(Align::Min))
                .id_salt(self.id.with("body")),
        );
        bui.spacing_mut().item_spacing.y = 0.0;
        let n = st.shown.len();
        let cursor = self.live && st.filter.trim().is_empty();
        let rows = n + usize::from(cursor);
        let mut area = egui::ScrollArea::vertical()
            .id_salt(self.id.with("scroll"))
            .auto_shrink([false, false])
            .stick_to_bottom(true);
        if let Some(y) = st.scroll_to.take() {
            area = area.vertical_scroll_offset(y);
        }
        let mono = font(Face::Mono, size::TEXT_2XS);
        let time_w = if self.times.is_some() {
            56.0 + 12.0
        } else {
            0.0
        };
        let out = area.show_rows(&mut bui, LINE_H, rows + 1, |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let w = ui.available_width();
            for r in range {
                let (row, rresp) = ui.allocate_exact_size(Vec2::new(w, LINE_H), Sense::hover());
                // The first and last rows are the body's `py-2`.
                if r == 0 {
                    continue;
                }
                let k = r - 1;
                if k == n {
                    if cursor {
                        let time = ui.input(|i| i.time);
                        let a =
                            0.5 + 0.5 * ((time / 2.0).fract() as f32 * std::f32::consts::TAU).cos();
                        let cr = Rect::from_min_size(
                            Pos2::new(row.left() + 12.0 + time_w, row.center().y - 6.0),
                            Vec2::new(6.0, 12.0),
                        );
                        ui.painter().rect_filled(
                            cr,
                            CornerRadius::ZERO,
                            tk.accent.gamma_multiply(a),
                        );
                        ui.ctx()
                            .request_repaint_after(std::time::Duration::from_millis(100));
                    }
                    continue;
                }
                let Some(i) = st.shown.get(k).map(|i| *i as usize) else {
                    continue;
                };
                let Some(line) = self.lines.get(i) else {
                    continue;
                };
                if rresp.hovered() {
                    ui.painter()
                        .rect_filled(row, CornerRadius::ZERO, alpha(tk.raised, 0.3));
                }
                let mut x = row.left() + 12.0;
                if let Some(ts) = self.times {
                    let at = ts.get(i).copied().unwrap_or(0.0);
                    let g = ph(ui, &format!("+{}", clock(at)), mono.clone());
                    draw(
                        ui.painter(),
                        &g,
                        x + 56.0 - g.size().x,
                        row.center().y,
                        alpha(tk.fg_faint, 0.7),
                    );
                    x += time_w;
                }
                let kind = st.kinds.get(i).copied().unwrap_or(LineKind::Normal);
                let room = row.right() - 12.0 - x;
                let g = ph_trunc(ui, line, mono.clone(), room);
                draw(ui.painter(), &g, x, row.center().y, kind.color(&tk));
                if super::text::is_cut(&g, line) {
                    let _ = rresp.tip_kbd(line, "", Side::Top);
                }
            }
        });
        let at_end =
            out.state.clamped_offset().y + out.inner_rect.height() >= out.content_size.y - 24.0;
        st.follow = at_end;
        if std::mem::take(&mut st.jump) && !at_end {
            st.scroll_to = Some((out.content_size.y - out.inner_rect.height()).max(0.0));
            st.follow = true;
            ui.ctx().request_repaint();
        }
        st.started = true;
        if !st.follow {
            let label = t("common", "live");
            let g = ph(ui, label, font(Face::SansSemibold, size::TEXT_2XS));
            let w = 12.0 + 12.0 + 6.0 + g.size().x + 12.0;
            let br = Rect::from_min_size(
                Pos2::new(body.right() - 16.0 - w, body.bottom() - 16.0 - 22.0),
                Vec2::new(w, 22.0),
            );
            let r = ui.interact(br, self.id.with("jump"), Sense::click());
            let p = ui.painter();
            p.add(paint::shadow_pop().as_shape(br, CornerRadius::same(11)));
            paint::plate(p, br, 11, tk.accent, alpha(tk.accent, 0.5));
            lucide::ARROW_DOWN_TO_LINE.paint(
                p,
                Rect::from_min_size(
                    Pos2::new(br.left() + 12.0, br.center().y - 6.0),
                    Vec2::splat(12.0),
                ),
                tk.accent_fg,
            );
            draw(p, &g, br.left() + 30.0, br.center().y, tk.accent_fg);
            if r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                st.scroll_to = Some((out.content_size.y - out.inner_rect.height()).max(0.0));
                st.follow = true;
                ui.ctx().request_repaint();
            }
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_like_the_svelte_log() {
        assert_eq!(
            classify_line("ERROR! Download item 123 failed"),
            LineKind::Err
        );
        assert_eq!(classify_line("errors here"), LineKind::Normal);
        assert_eq!(classify_line("Warning: low disk"), LineKind::Warn);
        assert_eq!(classify_line("Success. Downloaded item 1"), LineKind::Ok);
        assert_eq!(classify_line("Waiting for user info...OK"), LineKind::Ok);
        assert_eq!(
            classify_line(" Update state (0x61) downloading, progress: 42.1"),
            LineKind::Progress
        );
        assert_eq!(classify_line("12 %"), LineKind::Progress);
        assert_eq!(
            classify_line("Logging in user 'x' to Steam Public..."),
            LineKind::Login
        );
        assert_eq!(classify_line("Steam>"), LineKind::Login);
        assert_eq!(classify_line("[  0%] Checking"), LineKind::Progress);
        assert_eq!(classify_line("[----] Verifying"), LineKind::Dim);
        assert_eq!(classify_line("   "), LineKind::Dim);
        assert_eq!(classify_line("hello"), LineKind::Normal);
        assert_eq!(classify_line("bokeh"), LineKind::Normal);
    }

    #[test]
    fn rate_smooths_and_resets() {
        let mut r = Rate::new(5000.0);
        r.add(0.0, 0.0);
        assert_eq!(r.per_second(), None);
        r.add(1000.0, 1000.0);
        assert_eq!(r.per_second(), Some(1000.0));
        r.add(2000.0, 500.0);
        assert_eq!(r.per_second(), None);
    }

    #[test]
    fn log_state_follows_new_lines() {
        let mut s = LogState::default();
        let mut lines = vec!["a".to_owned(), "error x".to_owned()];
        s.sync(&lines, classify_line);
        assert_eq!(s.kinds, [LineKind::Normal, LineKind::Err]);
        lines.push("b".to_owned());
        s.filter = "B".into();
        s.sync(&lines, classify_line);
        assert_eq!(s.shown, [2]);
        s.sync(&["z".to_owned()], classify_line);
        assert_eq!(s.kinds.len(), 1);
    }
}
