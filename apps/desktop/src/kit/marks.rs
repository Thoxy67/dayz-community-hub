//! Small marks the lists wear: `ui/signal` (the ping as four bars), `ui/players` (count,
//! capacity and a bar), `ui/flag` and `ui/brand` (the Steam, Linux and Windows marks).

use std::collections::HashMap;
use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

use super::Tone;
use super::paint::{self, alpha};
use super::tooltip::TipExt;
use crate::icons::{flag, lucide};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, motion, size};

/// Past this, a reply is a timeout (the backend reports 9999 for one).
pub const PING_TIMEOUT_MS: u32 = 5000;

/// Bars lit for a round trip, 0–4.
pub fn signal_level(ms: Option<u32>) -> u8 {
    match ms {
        None => 0,
        Some(ms) if ms >= PING_TIMEOUT_MS => 0,
        Some(ms) if ms < 50 => 4,
        Some(ms) if ms < 100 => 3,
        Some(ms) if ms < 180 => 2,
        Some(_) => 1,
    }
}

/// The colour of a signal level: ok at 4, warn at 2–3, err at 1, faint at 0.
pub fn signal_color(t: &Tokens, level: u8) -> Color32 {
    match level {
        4.. => t.ok,
        2 | 3 => t.warn,
        1 => t.err,
        0 => t.fg_faint,
    }
}

/// How big a [`Signal`] is: bars 2/3/4px wide, 10/12/16 tall.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SignalSize {
    Xs,
    #[default]
    Sm,
    Md,
}

/// The ping, read the way a field radio reads a signal: four bars lit by how quickly
/// the server answered, the figure beside them. `pending` pulses the bars; a timeout is
/// four dark bars struck through and a red ×. `Signal::new(Some(92))`.
#[must_use = "add it with ui.add(…)"]
pub struct Signal {
    ms: Option<u32>,
    pending: bool,
    label: bool,
    size: SignalSize,
}

impl Signal {
    pub fn new(ms: Option<u32>) -> Self {
        Self {
            ms,
            pending: false,
            label: true,
            size: SignalSize::Sm,
        }
    }

    pub fn pending(mut self, p: bool) -> Self {
        self.pending = p;
        self
    }

    /// Without the figure.
    pub fn bars_only(mut self) -> Self {
        self.label = false;
        self
    }

    pub fn size(mut self, s: SignalSize) -> Self {
        self.size = s;
        self
    }
}

impl Widget for Signal {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let (h, bw, gap) = match self.size {
            SignalSize::Xs => (10.0, 2.0, 1.0),
            SignalSize::Sm => (12.0, 3.0, 2.0),
            SignalSize::Md => (16.0, 4.0, 2.0),
        };
        let level = signal_level(self.ms);
        let timed_out = !self.pending && self.ms.is_some_and(|m| m >= PING_TIMEOUT_MS);
        let tone = if self.pending {
            t.fg_faint
        } else {
            signal_color(&t, level)
        };
        let f = font(Face::Mono, size::TEXT_2XS);
        let (text, unit): (String, bool) = if !self.label {
            (String::new(), false)
        } else if self.pending {
            ("…".to_owned(), false)
        } else if timed_out {
            ("×".to_owned(), false)
        } else {
            match self.ms {
                None => ("—".to_owned(), false),
                Some(ms) => (ms.to_string(), true),
            }
        };
        let g = (!text.is_empty()).then(|| paint::galley(ui, &text, f.clone(), tone));
        let ug = unit.then(|| paint::galley(ui, "ms", f, t.fg_faint));
        let bars_w = 4.0 * bw + 3.0 * gap;
        let text_w =
            g.as_ref().map_or(0.0, |g| 6.0 + g.size().x) + ug.as_ref().map_or(0.0, |g| g.size().x);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(bars_w + text_w, f32::max(h, 14.0)),
            Sense::hover(),
        );
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let p = ui.painter();
        let bottom = rect.center().y + h / 2.0;
        let time = ui.input(|i| i.time);
        for b in 1..=4u8 {
            let bh = h * f32::from(b) * 0.25;
            let x = rect.left() + f32::from(b - 1) * (bw + gap);
            let r = Rect::from_min_max(Pos2::new(x, bottom - bh), Pos2::new(x + bw, bottom));
            let a = if self.pending {
                // animate-pulse (2s, opacity 1 → .5 → 1), staggered 120ms per bar, at 40%.
                let ph = ((time - f64::from(b) * 0.12) / 2.0).rem_euclid(1.0) as f32;
                let pulse = 0.75 + 0.25 * (ph * std::f32::consts::TAU).cos();
                0.4 * pulse
            } else if b <= level {
                1.0
            } else {
                0.2
            };
            p.rect_filled(r, CornerRadius::same(1), tone.gamma_multiply(a));
        }
        if self.pending {
            ui.ctx().request_repaint();
        }
        if timed_out {
            let c = Pos2::new(rect.left() + bars_w / 2.0, bottom - h / 2.0);
            let d = Vec2::angled((-25.0_f32).to_radians()) * (bars_w / 2.0 + 1.0);
            p.line_segment([c - d, c + d], Stroke::new(1.0, t.err));
        }
        let mut x = rect.left() + bars_w + 6.0;
        if let Some(g) = &g {
            let ink = if timed_out { t.err } else { tone };
            paint::text_at(p, g, x, rect.center().y, ink);
            x += g.size().x;
        }
        if let Some(u) = &ug {
            paint::text_at(p, u, x, rect.center().y, t.fg_faint);
        }
        resp
    }
}

/// How full a server is, as a tone: empty is quiet, past half warns, full is an error.
pub fn fill_tone(players: u32, max: u32) -> Tone {
    if players == 0 || max == 0 {
        Tone::Muted
    } else if players >= max {
        Tone::Err
    } else if players * 2 > max {
        Tone::Warn
    } else {
        Tone::Ok
    }
}

/// Players on a server: the count in its tone, `/max`, `+queue`, the bots at the end,
/// and a 4px bar under them (the bots' part hatched). `Players::new(41, 60)`.
#[must_use = "add it with ui.add(…)"]
pub struct Players<'a> {
    players: u32,
    max: u32,
    bots: u32,
    queue: u32,
    loading: bool,
    compact: bool,
    bots_label: &'a str,
}

impl<'a> Players<'a> {
    pub fn new(players: u32, max: u32) -> Self {
        Self {
            players,
            max,
            bots: 0,
            queue: 0,
            loading: false,
            compact: false,
            bots_label: "",
        }
    }

    pub fn bots(mut self, n: u32) -> Self {
        self.bots = n;
        self
    }

    pub fn queue(mut self, n: u32) -> Self {
        self.queue = n;
        self
    }

    pub fn loading(mut self, l: bool) -> Self {
        self.loading = l;
        self
    }

    /// 64px wide instead of 96.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    pub fn bots_label(mut self, l: &'a str) -> Self {
        self.bots_label = l;
        self
    }
}

impl Widget for Players<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let w = if self.compact { 64.0 } else { 96.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 15.0 + 2.0 + 4.0), Sense::hover());
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let tone = fill_tone(self.players, self.max);
        let (ink, fill) = match tone {
            Tone::Ok => (t.ok, t.ok),
            Tone::Warn => (t.warn, t.warn),
            Tone::Err => (t.err, t.err),
            _ => (t.fg_faint, alpha(t.fg_faint, 0.4)),
        };
        let p = ui.painter();
        let line_y = rect.top() + 7.5;
        let mut x = rect.left();
        let n = paint::galley(
            ui,
            &self.players.to_string(),
            font(Face::MonoSemibold, size::TEXT_2XS),
            ink,
        );
        let ink_n = if self.loading {
            ink.gamma_multiply(0.5)
        } else {
            ink
        };
        paint::text_at(p, &n, x, line_y, ink_n);
        x += n.size().x + 4.0;
        let m = paint::galley(
            ui,
            &format!("/{}", self.max),
            font(Face::Mono, size::TEXT_2XS),
            t.fg_faint,
        );
        paint::text_at(p, &m, x, line_y, t.fg_faint);
        x += m.size().x + 4.0;
        if self.queue > 0 {
            let q = paint::galley(
                ui,
                &format!("+{}", self.queue),
                font(Face::Mono, size::TEXT_2XS),
                t.warn,
            );
            paint::text_at(p, &q, x, line_y, t.warn);
        }
        if self.bots > 0 {
            let b = paint::galley(
                ui,
                &self.bots.to_string(),
                font(Face::Mono, size::TEXT_2XS),
                t.fg_faint,
            );
            let bx = rect.right() - b.size().x;
            paint::text_at(p, &b, bx, line_y, t.fg_faint);
            let ir =
                Rect::from_min_size(Pos2::new(bx - 2.0 - 12.0, line_y - 6.0), Vec2::splat(12.0));
            lucide::BOT.paint(p, ir, t.fg_faint);
            if !self.bots_label.is_empty() {
                let hr = ui.interact(
                    ir.union(Rect::from_min_size(Pos2::new(bx, line_y - 7.0), b.size())),
                    resp.id.with("bots"),
                    Sense::hover(),
                );
                let _ = hr.tip_kbd(self.bots_label, "", super::Side::Top);
            }
        }
        let bar = Rect::from_min_size(
            Pos2::new(rect.left(), rect.bottom() - 4.0),
            Vec2::new(w, 4.0),
        );
        let p = ui.painter();
        p.rect_filled(bar, CornerRadius::same(2), t.raised);
        let pct = if self.max > 0 {
            (self.players as f32 / self.max as f32).min(1.0)
        } else {
            0.0
        };
        let shown = ui
            .ctx()
            .animate_value_with_time(resp.id.with("pct"), pct, motion::SLOW);
        if shown > 0.0 {
            p.rect_filled(
                Rect::from_min_size(bar.min, Vec2::new(w * shown, 4.0)),
                CornerRadius::same(2),
                fill,
            );
        }
        let bot_pct = if self.max > 0 {
            (self.bots as f32 / self.max as f32).min(pct)
        } else {
            0.0
        };
        if bot_pct > 0.0 {
            // repeating-linear-gradient(135deg, fg-faint 0 2px, transparent 2px 4px)
            let hatch = Rect::from_min_max(
                Pos2::new(bar.left() + w * (pct - bot_pct), bar.top()),
                Pos2::new(bar.left() + w * pct, bar.bottom()),
            );
            let hp = p.with_clip_rect(hatch.intersect(p.clip_rect()));
            let mut x = hatch.left() - 4.0;
            while x < hatch.right() + 4.0 {
                hp.line_segment(
                    [
                        Pos2::new(x, hatch.bottom()),
                        Pos2::new(x + 4.0, hatch.top()),
                    ],
                    Stroke::new(1.4, t.fg_faint),
                );
                x += 4.0 * std::f32::consts::SQRT_2;
            }
        }
        resp
    }
}

/// `ui/flag`: a country's flag in a circle from its ISO code; an unknown code is the
/// code in a faint round chip. `Flag::new("fr").size(16.0).tip("France")`.
#[must_use = "add it with ui.add(…)"]
pub struct Flag<'a> {
    code: &'a str,
    size: f32,
    tip: &'a str,
}

impl<'a> Flag<'a> {
    pub fn new(code: &'a str) -> Self {
        Self {
            code,
            size: size::ICON,
            tip: "",
        }
    }

    pub fn size(mut self, px: f32) -> Self {
        self.size = px;
        self
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }
}

impl Widget for Flag<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        if ui.is_rect_visible(rect) {
            match flag::by_code(self.code) {
                Some(icon) => icon.paint(ui.painter(), rect, Color32::WHITE),
                None => {
                    let t = ui.tokens();
                    ui.painter()
                        .circle_filled(rect.center(), self.size / 2.0, t.raised);
                    let code: String = self
                        .code
                        .trim()
                        .chars()
                        .take(3)
                        .collect::<String>()
                        .to_uppercase();
                    let g = paint::galley(
                        ui,
                        &code,
                        font(Face::Mono, size::TEXT_3XS * self.size / size::ICON),
                        t.fg_faint,
                    );
                    paint::text_center(ui.painter(), &g, rect, t.fg_faint);
                }
            }
        }
        let tip = if self.tip.is_empty() {
            self.code
        } else {
            self.tip
        };
        resp.tip_kbd(tip, "", super::Side::Top)
    }
}

/// `ui/brand`: marks the icon sets do not carry (simple-icons, CC0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Brand {
    Steam,
    Linux,
    Windows,
}

const STEAM: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#fff"><path d="M11.979 0C5.678 0 .511 4.86.022 11.037l6.432 2.658c.545-.371 1.203-.59 1.912-.59.063 0 .125.004.188.006l2.861-4.142V8.91c0-2.495 2.028-4.524 4.524-4.524 2.494 0 4.524 2.031 4.524 4.527s-2.03 4.525-4.524 4.525h-.105l-4.076 2.911c0 .052.004.105.004.159 0 1.875-1.515 3.396-3.39 3.396-1.635 0-3.016-1.173-3.331-2.727L.436 15.27C1.862 20.307 6.486 24 11.979 24c6.627 0 11.999-5.373 11.999-12S18.605 0 11.979 0zM7.54 18.21l-1.473-.61c.262.543.714.999 1.314 1.25 1.297.539 2.793-.076 3.332-1.375.263-.63.264-1.319.005-1.949s-.75-1.121-1.377-1.383c-.624-.26-1.29-.249-1.878-.03l1.523.63c.956.4 1.409 1.5 1.009 2.455-.397.957-1.497 1.41-2.454 1.012H7.54zm11.415-9.303c0-1.662-1.353-3.015-3.015-3.015-1.665 0-3.015 1.353-3.015 3.015 0 1.665 1.35 3.015 3.015 3.015 1.663 0 3.015-1.35 3.015-3.015zm-5.273-.005c0-1.252 1.013-2.266 2.265-2.266 1.249 0 2.266 1.014 2.266 2.266 0 1.251-1.017 2.265-2.266 2.265-1.253 0-2.265-1.014-2.265-2.265z"/></svg>"##;

const LINUX: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#fff"><path d="M12.504 0c-.155 0-.315.008-.48.021-4.226.333-3.105 4.807-3.17 6.298-.076 1.092-.3 1.953-1.05 3.02-.885 1.051-2.127 2.75-2.716 4.521-.278.832-.41 1.684-.287 2.489a.424.424 0 00-.11.135c-.26.268-.45.6-.663.839-.199.199-.485.267-.797.4-.313.136-.658.269-.864.68-.09.189-.136.394-.132.602 0 .199.027.4.055.536.058.399.116.728.04.97-.249.68-.28 1.145-.106 1.484.174.334.535.47.94.601.81.2 1.91.135 2.774.6.926.466 1.866.67 2.616.47.526-.116.97-.464 1.208-.946.587-.003 1.23-.269 2.26-.334.699-.058 1.574.267 2.577.2.025.134.063.198.114.333l.003.003c.391.778 1.113 1.132 1.884 1.071.771-.06 1.592-.536 2.257-1.306.631-.765 1.683-1.084 2.378-1.503.348-.199.629-.469.649-.853.023-.4-.2-.811-.714-1.376v-.097l-.003-.003c-.17-.2-.25-.535-.338-.926-.085-.401-.182-.786-.492-1.046h-.003c-.059-.054-.123-.067-.188-.135a.357.357 0 00-.19-.064c.431-1.278.264-2.55-.173-3.694-.533-1.41-1.465-2.638-2.175-3.483-.796-1.005-1.576-1.957-1.56-3.368.026-2.152.236-6.133-3.544-6.139zm.529 3.405h.013c.213 0 .396.062.584.198.19.135.33.332.438.533.105.259.158.459.166.724 0-.02.006-.04.006-.06v.105a.086.086 0 01-.004-.021l-.004-.024a1.807 1.807 0 01-.15.706.953.953 0 01-.213.335.71.71 0 00-.088-.042c-.104-.045-.198-.064-.284-.133a1.312 1.312 0 00-.22-.066c.05-.06.146-.133.183-.198.053-.128.082-.264.088-.402v-.02a1.21 1.21 0 00-.061-.4c-.045-.134-.101-.2-.183-.333-.084-.066-.167-.132-.267-.132h-.016c-.093 0-.176.03-.262.132a.8.8 0 00-.205.334 1.18 1.18 0 00-.09.4v.019c.002.089.008.179.02.267-.193-.067-.438-.135-.607-.202a1.635 1.635 0 01-.018-.2v-.02a1.772 1.772 0 01.15-.768c.082-.22.232-.406.43-.533a.985.985 0 01.594-.2zm-2.962.059h.036c.142 0 .27.048.399.135.146.129.264.288.344.465.09.199.14.4.153.667v.004c.007.134.006.2-.002.266v.08c-.03.007-.056.018-.083.024-.152.055-.274.135-.393.2.012-.09.013-.18.003-.267v-.015c-.012-.133-.04-.2-.082-.333a.613.613 0 00-.166-.267.248.248 0 00-.183-.064h-.021c-.071.006-.13.04-.186.132a.552.552 0 00-.12.27.944.944 0 00-.023.33v.015c.012.135.037.2.08.334.046.134.098.2.166.268.01.009.02.018.034.024-.07.057-.117.07-.176.136a.304.304 0 01-.131.068 2.62 2.62 0 01-.275-.402 1.772 1.772 0 01-.155-.667 1.759 1.759 0 01.08-.668 1.43 1.43 0 01.283-.535c.128-.133.26-.2.418-.2zm1.37 1.706c.332 0 .733.065 1.216.399.293.2.523.269 1.052.468h.003c.255.136.405.266.478.399v-.131a.571.571 0 01.016.47c-.123.31-.516.643-1.063.842v.002c-.268.135-.501.333-.775.465-.276.135-.588.292-1.012.267a1.139 1.139 0 01-.448-.067 3.566 3.566 0 01-.322-.198c-.195-.135-.363-.332-.612-.465v-.005h-.005c-.4-.246-.616-.512-.686-.71-.07-.268-.005-.47.193-.6.224-.135.38-.271.483-.336.104-.074.143-.102.176-.131h.002v-.003c.169-.202.436-.47.839-.601.139-.036.294-.065.466-.065zm2.8 2.142c.358 1.417 1.196 3.475 1.735 4.473.286.534.855 1.659 1.102 3.024.156-.005.33.018.513.064.646-1.671-.546-3.467-1.089-3.966-.22-.2-.232-.335-.123-.335.59.534 1.365 1.572 1.646 2.757.13.535.16 1.104.021 1.67.067.028.135.06.205.067 1.032.534 1.413.938 1.23 1.537v-.043c-.06-.003-.12 0-.18 0h-.016c.151-.467-.182-.825-1.065-1.224-.915-.4-1.646-.336-1.77.465-.008.043-.013.066-.018.135-.068.023-.139.053-.209.064-.43.268-.662.669-.793 1.187-.13.533-.17 1.156-.205 1.869v.003c-.02.334-.17.838-.319 1.35-1.5 1.072-3.58 1.538-5.348.334a2.645 2.645 0 00-.402-.533 1.45 1.45 0 00-.275-.333c.182 0 .338-.03.465-.067a.615.615 0 00.314-.334c.108-.267 0-.697-.345-1.163-.345-.467-.931-.995-1.788-1.521-.63-.4-.986-.87-1.15-1.396-.165-.534-.143-1.085-.015-1.645.245-1.07.873-2.11 1.274-2.763.107-.065.037.135-.408.974-.396.751-1.14 2.497-.122 3.854a8.123 8.123 0 01.647-2.876c.564-1.278 1.743-3.504 1.836-5.268.048.036.217.135.289.202.218.133.38.333.59.465.21.201.477.335.876.335.039.003.075.006.11.006.412 0 .73-.134.997-.268.29-.134.52-.334.74-.4h.005c.467-.135.835-.402 1.044-.7zm2.185 8.958c.037.6.343 1.245.882 1.377.588.134 1.434-.333 1.791-.765l.211-.01c.315-.007.577.01.847.268l.003.003c.208.199.305.53.391.876.085.4.154.78.409 1.066.486.527.645.906.636 1.14l.003-.007v.018l-.003-.012c-.015.262-.185.396-.498.595-.63.401-1.746.712-2.457 1.57-.618.737-1.37 1.14-2.036 1.191-.664.053-1.237-.2-1.574-.898l-.005-.003c-.21-.4-.12-1.025.056-1.69.176-.668.428-1.344.463-1.897.037-.714.076-1.335.195-1.814.12-.465.308-.797.641-.984l.045-.022zm-10.814.049h.01c.053 0 .105.005.157.014.376.055.706.333 1.023.752l.91 1.664.003.003c.243.533.754 1.064 1.189 1.637.434.598.77 1.131.729 1.57v.006c-.057.744-.48 1.148-1.125 1.294-.645.135-1.52.002-2.395-.464-.968-.536-2.118-.469-2.857-.602-.369-.066-.61-.2-.723-.4-.11-.2-.113-.602.123-1.23v-.004l.002-.003c.117-.334.03-.752-.027-1.118-.055-.401-.083-.71.043-.94.16-.334.396-.4.69-.533.294-.135.64-.202.915-.47h.002v-.002c.256-.268.445-.601.668-.838.19-.201.38-.336.663-.336zm7.159-9.074c-.435.201-.945.535-1.488.535-.542 0-.97-.267-1.28-.466-.154-.134-.28-.268-.373-.335-.164-.134-.144-.333-.074-.333.109.016.129.134.199.2.096.066.215.2.36.333.292.2.68.467 1.167.467.485 0 1.053-.267 1.398-.466.195-.135.445-.334.648-.467.156-.136.149-.267.279-.267.128.016.034.134-.147.332a8.097 8.097 0 01-.69.468zm-1.082-1.583V5.64c-.006-.02.013-.042.029-.05.074-.043.18-.027.26.004.063 0 .16.067.15.135-.006.049-.085.066-.135.066-.055 0-.092-.043-.141-.068-.052-.018-.146-.008-.163-.065zm-.551 0c-.02.058-.113.049-.166.066-.047.025-.086.068-.14.068-.05 0-.13-.02-.136-.068-.01-.066.088-.133.15-.133.08-.031.184-.047.259-.005.019.009.036.03.03.05v.02h.003z"/></svg>"##;

const WINDOWS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#fff"><path d="M0,0H11.377V11.372H0ZM12.623,0H24V11.372H12.623ZM0,12.623H11.377V24H0Zm12.623,0H24V24H12.623"/></svg>"##;

type BrandCache = HashMap<(Brand, u32), Option<egui::TextureHandle>>;

impl Brand {
    fn svg(self) -> &'static str {
        match self {
            Brand::Steam => STEAM,
            Brand::Linux => LINUX,
            Brand::Windows => WINDOWS,
        }
    }

    /// Paints the mark in `rect` (square, centred) in `tint`.
    pub fn paint(self, painter: &egui::Painter, rect: Rect, tint: Color32) {
        let side = rect.width().min(rect.height());
        if side <= 0.0 || !side.is_finite() {
            return;
        }
        let ctx = painter.ctx();
        let px = ((side * ctx.pixels_per_point()).round() as u32).clamp(1, 512);
        let cache_id = egui::Id::new("kit::brand-cache");
        let cache: Arc<Mutex<BrandCache>> = ctx.data_mut(|d| {
            d.get_temp_mut_or_default::<Arc<Mutex<BrandCache>>>(cache_id)
                .clone()
        });
        let tex = {
            let mut c = cache.lock();
            c.entry((self, px))
                .or_insert_with(|| {
                    raster(self.svg(), px).map(|img| {
                        ctx.load_texture(
                            format!("brand-{self:?}-{px}"),
                            img,
                            egui::TextureOptions::LINEAR,
                        )
                    })
                })
                .clone()
        };
        if let Some(tex) = tex {
            let r = Rect::from_center_size(rect.center(), Vec2::splat(side));
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                tint,
            );
        }
    }

    /// A `size` square holding the mark.
    pub fn show(self, ui: &mut Ui, size: f32, tint: Color32) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size.max(0.0)), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, tint);
        }
        resp
    }
}

fn raster(svg: &str, px: u32) -> Option<egui::ColorImage> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pix = tiny_skia::Pixmap::new(px, px)?;
    let s = px as f32 / tree.size().width().max(tree.size().height()).max(1.0);
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(s, s),
        &mut pix.as_mut(),
    );
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        pix.data(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(signal_level(None), 0);
        assert_eq!(signal_level(Some(9999)), 0);
        assert_eq!(signal_level(Some(30)), 4);
        assert_eq!(signal_level(Some(92)), 3);
        assert_eq!(signal_level(Some(166)), 2);
        assert_eq!(signal_level(Some(240)), 1);
        assert_eq!(fill_tone(0, 60), Tone::Muted);
        assert_eq!(fill_tone(41, 60), Tone::Warn);
        assert_eq!(fill_tone(10, 60), Tone::Ok);
        assert_eq!(fill_tone(60, 60), Tone::Err);
    }

    #[test]
    fn brands_rasterise() {
        for svg in [STEAM, LINUX, WINDOWS] {
            assert!(raster(svg, 24).is_some());
        }
    }
}
