//! `ui/alert` (Alert, CommandLine) and `ui/copy`.

use egui::{
    Align, Color32, CornerRadius, Layout, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2, Widget,
};

use super::Tone;
use super::paint::{self, alpha};
use super::tooltip::TipExt;
use crate::icons::{Icon, lucide};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What an [`Alert`] did this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct AlertResponse {
    pub dismissed: bool,
}

/// Something the reader has to know, and usually something to do about it: an icon in
/// a well, the title in the tone, a sentence under it, the way out at the far end. Wide
/// (60rem and over) it is one line; narrow, the actions go under the words.
/// `banner` runs edge to edge (a line under it) instead of a rounded card.
/// `Alert::new(Tone::Warn, lucide::TRIANGLE_ALERT, "Steam is not running").text("…").show(ui, |ui| …)`.
#[must_use = "call show(…)"]
pub struct Alert<'a> {
    tone: Tone,
    icon: Icon,
    title: &'a str,
    text: &'a str,
    banner: bool,
    dismiss: Option<&'a str>,
}

impl<'a> Alert<'a> {
    /// `tone`: Warn, Err, Ok or Info.
    pub fn new(tone: Tone, icon: Icon, title: &'a str) -> Self {
        Self {
            tone,
            icon,
            title,
            text: "",
            banner: false,
            dismiss: None,
        }
    }

    pub fn text(mut self, t: &'a str) -> Self {
        self.text = t;
        self
    }

    pub fn banner(mut self) -> Self {
        self.banner = true;
        self
    }

    /// The alert can be put away; `label` names the button.
    pub fn dismissible(mut self, label: &'a str) -> Self {
        self.dismiss = Some(label);
        self
    }

    /// `actions` gets a right-to-left row: add the far-end button first.
    pub fn show(self, ui: &mut Ui, actions: impl FnOnce(&mut Ui)) -> AlertResponse {
        let t = ui.tokens();
        let (edge, ground, ink, well) = match self.tone {
            Tone::Err => (
                alpha(t.err, 0.4),
                alpha(t.err, 0.08),
                t.err,
                alpha(t.err, 0.15),
            ),
            Tone::Ok => (
                alpha(t.ok, 0.35),
                alpha(t.ok, 0.08),
                t.ok,
                alpha(t.ok, 0.15),
            ),
            Tone::Info | Tone::Accent | Tone::Neutral | Tone::Muted => (
                t.border_strong,
                alpha(t.raised, 0.4),
                t.accent,
                alpha(t.accent, 0.12),
            ),
            Tone::Warn => (
                alpha(t.warn, 0.35),
                alpha(t.warn, 0.08),
                t.warn,
                alpha(t.warn, 0.15),
            ),
        };
        let w = ui.available_width();
        let wide = w >= 960.0;
        let top = ui.cursor().min;
        let bg = ui.painter().add(egui::Shape::Noop);
        let dismiss_w = if self.dismiss.is_some() {
            size::CONTROL_SM + 10.0
        } else {
            0.0
        };
        let text_left = top.x + 10.0 + size::CONTROL + 10.0;
        let right = top.x + w - 10.0 - dismiss_w;
        // Actions: laid out into a right-aligned row, measured, then the words take the rest.
        let actions_w_id = ui.id().with(("alert-actions", self.title));
        let last_aw: f32 = ui.data(|d| d.get_temp(actions_w_id)).unwrap_or(0.0);
        let words_w = if wide {
            (right - text_left - last_aw - 12.0).max(80.0)
        } else {
            (right - text_left).max(80.0)
        };
        let tg = paint::galley_wrap(
            ui,
            self.title,
            font(Face::SansSemibold, size::TEXT_XS),
            ink,
            words_w,
        );
        let bgal = (!self.text.is_empty()).then(|| {
            paint::galley_wrap(
                ui,
                self.text,
                font(Face::Sans, size::TEXT_2XS),
                t.fg_muted,
                words_w,
            )
        });
        let words_h = tg.size().y + bgal.as_ref().map_or(0.0, |g| g.size().y + 2.0);
        let words_top = top.y + 8.0;
        let actions_top = if wide {
            words_top
        } else {
            words_top + words_h + 6.0
        };
        let arow_h = if last_aw > 0.0 || !wide {
            size::CONTROL
        } else {
            0.0
        };
        let arect = Rect::from_min_max(
            Pos2::new(text_left, actions_top),
            Pos2::new(right, actions_top + size::CONTROL),
        );
        let mut aui = ui.new_child(
            UiBuilder::new()
                .max_rect(arect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        aui.spacing_mut().item_spacing.x = 6.0;
        actions(&mut aui);
        let aw = aui.min_rect().width();
        let has_actions = aw > 0.5;
        if (aw - last_aw).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(actions_w_id, aw));
            ui.ctx().request_repaint();
        }
        let inner_h = if wide {
            words_h
                .max(if has_actions { size::CONTROL } else { 0.0 })
                .max(size::CONTROL)
        } else {
            (words_h + if has_actions { 6.0 + arow_h } else { 0.0 }).max(size::CONTROL)
        };
        let h = inner_h + 16.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let p = ui.painter();
        if self.banner {
            p.set(
                bg,
                egui::Shape::rect_filled(rect, CornerRadius::ZERO, ground),
            );
            p.hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, edge));
        } else {
            p.set(
                bg,
                egui::Shape::Rect(egui::epaint::RectShape::new(
                    rect,
                    CornerRadius::same(size::RADIUS_MD),
                    ground,
                    Stroke::new(1.0, edge),
                    StrokeKind::Inside,
                )),
            );
        }
        let wr = Rect::from_min_size(
            Pos2::new(rect.left() + 10.0, rect.top() + 8.0),
            Vec2::splat(size::CONTROL),
        );
        p.rect_filled(wr, CornerRadius::same(size::RADIUS_SM), well);
        self.icon.paint(
            p,
            Rect::from_center_size(wr.center(), Vec2::splat(size::ICON_LG)),
            ink,
        );
        let ty = if wide {
            rect.top() + 8.0 + (inner_h - words_h) / 2.0
        } else {
            words_top
        };
        p.galley(Pos2::new(text_left, ty), tg.clone(), ink);
        if let Some(b) = bgal {
            p.galley(Pos2::new(text_left, ty + tg.size().y + 2.0), b, t.fg_muted);
        }
        let mut out = AlertResponse::default();
        if let Some(label) = self.dismiss {
            let xr = Rect::from_min_size(
                Pos2::new(rect.right() - 10.0 - size::CONTROL_SM, rect.top() + 8.0),
                Vec2::splat(size::CONTROL_SM),
            );
            let xb = ui.interact(xr, ui.id().with(("alert-x", self.title)), Sense::click());
            let hx = paint::ease(ui, xb.id, xb.hovered());
            let p = ui.painter();
            p.rect_filled(
                xr,
                CornerRadius::same(size::RADIUS_SM),
                paint::lerp(Color32::TRANSPARENT, t.raised, hx),
            );
            lucide::X.paint(
                p,
                Rect::from_center_size(xr.center(), Vec2::splat(size::ICON)),
                paint::lerp(t.fg_faint, t.fg, hx),
            );
            if xb.clicked() {
                out.dismissed = true;
            }
            let _ = xb.tip_kbd(label, "", super::Side::Bottom);
        }
        out
    }
}

/// A shell command to run by hand, with a button that copies it: `h-control-sm
/// rounded-sm border bg-bg`, the command in faint mono (cut, whole in the hint), the copy
/// button behind a hairline turning into a green tick for 1.5s.
/// `CommandLine::new("sudo …", "Copy")`.
#[must_use = "add it with ui.add(…)"]
pub struct CommandLine<'a> {
    command: &'a str,
    copy_label: &'a str,
    width: Option<f32>,
}

impl<'a> CommandLine<'a> {
    pub fn new(command: &'a str, copy_label: &'a str) -> Self {
        Self {
            command,
            copy_label,
            width: None,
        }
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

impl Widget for CommandLine<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let f = font(Face::Mono, size::TEXT_2XS);
        let natural = paint::galley(ui, self.command, f.clone(), t.fg_muted)
            .size()
            .x
            + 12.0
            + size::CONTROL_SM
            + 2.0;
        let w = self.width.unwrap_or_else(|| {
            natural
                .min(ui.available_width())
                .max(192.0_f32.min(ui.available_width()))
        });
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL_SM), Sense::hover());
        let p = ui.painter();
        paint::plate(p, rect, size::RADIUS_SM, t.bg, t.border);
        let br = Rect::from_min_max(
            Pos2::new(rect.right() - size::CONTROL_SM, rect.top()),
            rect.max,
        );
        let g = paint::galley_trunc(
            ui,
            self.command,
            f,
            t.fg_muted,
            (br.left() - rect.left() - 12.0).max(0.0),
        );
        paint::text_at(p, &g, rect.left() + 6.0, rect.center().y, t.fg_muted);
        p.vline(br.left() + 0.5, br.y_range(), Stroke::new(1.0, t.border));
        let b = ui.interact(br, resp.id.with("copy"), Sense::click());
        let copied_id = resp.id.with("copied");
        let now = ui.input(|i| i.time);
        if b.clicked() {
            ui.ctx().copy_text(self.command.to_owned());
            ui.data_mut(|d| d.insert_temp(copied_id, now));
        }
        let copied = ui
            .data(|d| d.get_temp::<f64>(copied_id))
            .is_some_and(|at| now - at < 1.5);
        if copied {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let hb = paint::ease(ui, b.id, b.hovered());
        let p = ui.painter();
        let inner = br.shrink(1.0);
        p.rect_filled(
            inner,
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: 3,
                se: 3,
            },
            paint::lerp(Color32::TRANSPARENT, t.raised, hb),
        );
        let (icon, ink) = if copied {
            (lucide::CHECK, t.ok)
        } else {
            (lucide::COPY, paint::lerp(t.fg_faint, t.fg, hb))
        };
        icon.paint(
            p,
            Rect::from_center_size(br.center(), Vec2::splat(size::ICON_SM)),
            ink,
        );
        let _ = b.tip_kbd(self.copy_label, "", super::Side::Bottom);
        resp.tip_kbd(self.command, "", super::Side::Top)
    }
}

/// `ui/copy`: text a player may paste elsewhere (an address): the text itself is the
/// button, a copy glyph shows on hover and turns into a tick for 1.4s once copied.
/// `Copy::new("45.0.0.0:2302").labels("Copy", "Copied")`.
#[must_use = "add it with ui.add(…)"]
pub struct Copy<'a> {
    text: &'a str,
    shown: Option<&'a str>,
    copy: &'a str,
    copied: &'a str,
    max_width: Option<f32>,
    wrap: bool,
    color: Option<Color32>,
    px: f32,
}

impl<'a> Copy<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            shown: None,
            copy: "",
            copied: "",
            max_width: None,
            wrap: false,
            color: None,
            px: size::TEXT_2XS,
        }
    }

    /// The resting colour when the use site sets one (`class="text-fg-muted"`, `text-fg`);
    /// `fg_faint` otherwise. Under the pointer it still turns `fg-muted`.
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }

    /// The text size when the use site sets one (`text-xs`, `text-3xs`); `text-2xs` otherwise.
    pub fn text_size(mut self, px: f32) -> Self {
        self.px = px;
        self
    }

    /// Long text wraps onto more lines (anywhere: a path) instead of being cut.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    /// What is shown, when it is not the text copied.
    pub fn shown(mut self, s: &'a str) -> Self {
        self.shown = Some(s);
        self
    }

    /// The hint words (`common.copy`, `common.copied`).
    pub fn labels(mut self, copy: &'a str, copied: &'a str) -> Self {
        self.copy = copy;
        self.copied = copied;
        self
    }

    pub fn max_width(mut self, w: f32) -> Self {
        self.max_width = Some(w);
        self
    }
}

impl Widget for Copy<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let f = font(Face::Mono, self.px);
        let rest = self.color.unwrap_or(t.fg_faint);
        let room = self.max_width.unwrap_or_else(|| ui.available_width()) - 16.0;
        let shown = self.shown.unwrap_or(self.text);
        let g = if self.wrap {
            let mut job =
                egui::text::LayoutJob::simple(shown.to_owned(), f, t.fg_faint, room.max(8.0));
            job.wrap.break_anywhere = true;
            ui.fonts_mut(|fo| fo.layout_job(job))
        } else {
            paint::galley_trunc(ui, shown, f, t.fg_faint, room.max(0.0))
        };
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(g.size().x + 4.0 + 12.0, g.size().y.max(14.0)),
            Sense::click(),
        );
        let copied_id = resp.id.with("copied");
        let now = ui.input(|i| i.time);
        if resp.clicked() {
            ui.ctx().copy_text(self.text.to_owned());
            ui.data_mut(|d| d.insert_temp(copied_id, now));
        }
        let copied = ui
            .data(|d| d.get_temp::<f64>(copied_id))
            .is_some_and(|at| now - at < 1.4);
        if copied {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let ink = if copied {
            t.ok
        } else {
            paint::lerp(rest, t.fg_muted, hover)
        };
        let p = ui.painter();
        paint::text_at(p, &g, rect.left(), rect.center().y, ink);
        // After the text: its last line when it wraps.
        let (end_x, end_cy) = match g.rows.last().filter(|_| g.rows.len() > 1) {
            Some(row) => {
                let top = rect.center().y - g.size().y / 2.0;
                (row.rect().right(), top + row.rect().center().y)
            }
            None => (g.size().x, rect.center().y),
        };
        let ir = Rect::from_min_size(
            Pos2::new(rect.left() + end_x + 4.0, end_cy - 6.0),
            Vec2::splat(12.0),
        );
        if copied {
            lucide::CHECK.paint(p, ir, ink);
        } else if hover > 0.0 {
            lucide::COPY.paint(p, ir, ink.gamma_multiply(hover));
        }
        let tip = if copied { self.copied } else { self.copy };
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
            .tip_kbd(tip, "", super::Side::Top)
    }
}
