//! Frames and headings: `ui/panel`, `ui/field`, `ui/section-label`, `ui/stat`
//! (StatStrip, Stat), `ui/empty-state` and `ui/disclosure`.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Color32, CornerRadius, Id, Layout, Margin, Pos2, Rect, Response, Sense, Stroke, Ui,
    UiBuilder, Vec2, Widget,
};

use super::paint::{self, alpha};
use crate::icons::{Icon, lucide};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// The stencilled label face: display, bold, 11px, `tracking 0.12em`, capitals.
pub fn stencil(ui: &Ui, text: &str, color: Color32, max_width: Option<f32>) -> Arc<egui::Galley> {
    paint::tracked(
        ui,
        text,
        font(Face::DisplayBold, size::TEXT_2XS + 1.0),
        color,
        0.12,
        true,
        max_width,
    )
}

/// `ui/panel`: a flat instrument panel (`bg-panel`, no border, no radius). With a title,
/// a header strip (`h-control border-b bg-raised/30`, the accent tick and the stencilled
/// name, controls at the end). Its body scrolls unless told not to.
/// `Panel::new("filters").title("Filters").show(ui, |ui| …)`.
#[must_use = "call show(…)"]
pub struct Panel<'a> {
    id: Id,
    title: &'a str,
    scroll: bool,
    padded: bool,
}

impl<'a> Panel<'a> {
    pub fn new(id_salt: impl egui::AsId) -> Self {
        Self {
            id: Id::new(id_salt),
            title: "",
            scroll: true,
            padded: false,
        }
    }

    pub fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    /// Keep the height of what it holds rather than scrolling.
    pub fn no_scroll(mut self) -> Self {
        self.scroll = false;
        self
    }

    /// `p-2` around the body.
    pub fn padded(mut self) -> Self {
        self.padded = true;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
        self.show_with(ui, |_| {}, body)
    }

    /// With controls at the end of the title strip (shown only when there is a title).
    pub fn show_with<R>(
        self,
        ui: &mut Ui,
        toolbar: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let t = ui.tokens();
        let full = if self.scroll {
            ui.available_rect_before_wrap()
        } else {
            Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), 0.0))
        };
        let bg = ui.painter().add(egui::Shape::Noop);
        let mut top = full.top();
        if !self.title.is_empty() {
            let hr = Rect::from_min_size(full.min, Vec2::new(full.width(), size::CONTROL));
            let p = ui.painter();
            p.rect_filled(hr, CornerRadius::ZERO, alpha(t.raised, 0.3));
            p.hline(hr.x_range(), hr.bottom() - 0.5, Stroke::new(1.0, t.border));
            paint::tick(p, hr.left() + 8.0, hr.center().y, &t);
            let mut tb = ui.new_child(
                UiBuilder::new()
                    .id_salt(self.id.with("toolbar"))
                    .max_rect(Rect::from_min_max(
                        hr.min,
                        Pos2::new(hr.right() - 6.0, hr.bottom()),
                    ))
                    .layout(Layout::right_to_left(Align::Center)),
            );
            tb.spacing_mut().item_spacing.x = 4.0;
            toolbar(&mut tb);
            let tools = tb.min_rect().width();
            let x = hr.left() + 8.0 + 2.0 + 6.0;
            let g = stencil(
                ui,
                self.title,
                t.fg_muted,
                Some((hr.right() - 6.0 - tools - 8.0 - x).max(0.0)),
            );
            paint::text_at(ui.painter(), &g, x, hr.center().y, t.fg_muted);
            top = hr.bottom();
        }
        let body_rect = Rect::from_min_max(
            Pos2::new(full.left(), top),
            Pos2::new(full.right(), full.bottom().max(top)),
        );
        let pad = if self.padded { 8 } else { 0 };
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(self.id.with("body"))
                .max_rect(body_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        let r = if self.scroll {
            child.set_clip_rect(body_rect.intersect(ui.clip_rect()));
            egui::ScrollArea::vertical()
                .id_salt(self.id.with("scroll"))
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    egui::Frame::new()
                        .inner_margin(Margin::same(pad))
                        .show(ui, body)
                        .inner
                })
                .inner
        } else {
            egui::Frame::new()
                .inner_margin(Margin::same(pad))
                .show(&mut child, body)
                .inner
        };
        let used = if self.scroll {
            full
        } else {
            Rect::from_min_max(full.min, Pos2::new(full.right(), child.min_rect().bottom()))
        };
        ui.painter().set(
            bg,
            egui::Shape::rect_filled(used, CornerRadius::ZERO, t.panel),
        );
        ui.allocate_rect(used, Sense::hover());
        r
    }
}

/// `ui/field`: one row of a settings form: the name (and a hint under it) on the left in
/// a 176px column (wider in the pad density), the control on the right;
/// `border-b border/60 px-pad py-2`.
/// `Field::new("Steam folder").hint("…").show(ui, |ui| { ui.add(Input::new(&mut p)); })`.
#[must_use = "call show(…)"]
pub struct Field<'a> {
    label: &'a str,
    hint: &'a str,
    stacked: bool,
    last: bool,
}

impl<'a> Field<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            hint: "",
            stacked: false,
            last: false,
        }
    }

    pub fn hint(mut self, h: &'a str) -> Self {
        self.hint = h;
        self
    }

    /// The control under the name rather than beside it, for wide inputs.
    pub fn stacked(mut self) -> Self {
        self.stacked = true;
        self
    }

    /// The last row of its group: no line under it.
    pub fn last(mut self) -> Self {
        self.last = true;
        self
    }

    pub fn show<R>(self, ui: &mut Ui, control: impl FnOnce(&mut Ui) -> R) -> R {
        let t = ui.tokens();
        let w = ui.available_width();
        let left = ui.cursor().min;
        let label_w = if self.stacked {
            w - 2.0 * size::PAD
        } else {
            // Wider with the pad's larger words, so the hints keep their line count.
            (176.0 * size::text(size::TEXT_2XS) / size::TEXT_2XS).min((w * 0.4).max(176.0))
        };
        // `text-xs` (line height 1.45) and `text-2xs leading-snug` (1.375), `mt-0.5`.
        let (lg, l_off) = paint::galley_lh(
            ui,
            self.label,
            font(Face::SansMedium, size::TEXT_XS),
            t.fg,
            label_w,
            size::TEXT_XS * 1.45,
        );
        let hg = (!self.hint.is_empty()).then(|| {
            paint::galley_lh(
                ui,
                self.hint,
                font(Face::Sans, size::TEXT_2XS),
                t.fg_faint,
                label_w,
                size::TEXT_2XS * 1.375,
            )
        });
        let text_h = lg.size().y + hg.as_ref().map_or(0.0, |(g, _)| g.size().y + 2.0);
        // The control's height is last frame's (a control rarely changes height), so it
        // can be centred on the name before it is drawn.
        let hid = ui.id().with(("kit-field", self.label));
        let last_ctl: f32 = ui.data(|d| d.get_temp(hid)).unwrap_or(size::CONTROL);
        let ctl_left = if self.stacked {
            left.x + size::PAD
        } else {
            left.x + size::PAD + label_w + 12.0
        };
        let ctl_top = if self.stacked {
            left.y + 8.0 + text_h + 6.0
        } else {
            left.y + 8.0 + ((text_h - last_ctl) / 2.0).max(0.0)
        };
        let ctl_rect = Rect::from_min_max(
            Pos2::new(ctl_left, ctl_top),
            Pos2::new(
                (left.x + w - size::PAD).max(ctl_left),
                ctl_top + last_ctl.max(1.0),
            ),
        );
        let mut cui = ui.new_child(
            UiBuilder::new()
                .max_rect(ctl_rect)
                .layout(Layout::left_to_right(Align::Min)),
        );
        cui.spacing_mut().item_spacing.x = 6.0;
        let r = control(&mut cui);
        let ctl_h = cui.min_rect().height();
        if (ctl_h - last_ctl).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(hid, ctl_h));
            ui.ctx().request_repaint();
        }
        // `py-2`, and the `border-b` a pixel of its own under it unless it is the last.
        let border = if self.last { 0.0 } else { 1.0 };
        let h = if self.stacked {
            8.0 + text_h + 6.0 + ctl_h + 8.0 + border
        } else {
            8.0 + text_h.max(ctl_h) + 8.0 + border
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let p = ui.painter();
        let ty = if self.stacked {
            rect.top() + 8.0
        } else {
            (rect.top() + rect.bottom() - border) / 2.0 - text_h / 2.0
        };
        p.galley(
            Pos2::new(rect.left() + size::PAD, ty + l_off),
            lg.clone(),
            t.fg,
        );
        if let Some((hg, h_off)) = hg {
            p.galley(
                Pos2::new(rect.left() + size::PAD, ty + lg.size().y + 2.0 + h_off),
                hg,
                t.fg_faint,
            );
        }
        if !self.last {
            p.hline(
                rect.x_range(),
                rect.bottom() - 0.5,
                Stroke::new(1.0, alpha(t.border, 0.6)),
            );
        }
        r
    }
}

/// `ui/section-label`: the name of a cluster of rows: the accent tick, an optional icon,
/// mono tracked capitals in `fg-muted`. `SectionLabel::new("Maps").icon(lucide::MAP)`.
#[must_use = "add it with ui.add(…) or show_with(…)"]
pub struct SectionLabel<'a> {
    text: &'a str,
    icon: Option<Icon>,
    tick: bool,
}

impl<'a> SectionLabel<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            icon: None,
            tick: true,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Without the accent tick, inside something that already carries one.
    pub fn no_tick(mut self) -> Self {
        self.tick = false;
        self
    }

    /// With controls at the far end of the same line ("all / none", a count).
    pub fn show_with(self, ui: &mut Ui, actions: impl FnOnce(&mut Ui)) -> Response {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
        let mut aui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        aui.spacing_mut().item_spacing.x = 4.0;
        actions(&mut aui);
        let used = aui.min_rect().width();
        self.paint(ui, rect, used);
        resp
    }

    fn paint(&self, ui: &Ui, rect: Rect, reserved: f32) {
        let t = ui.tokens();
        let p = ui.painter();
        let cy = rect.center().y;
        let mut x = rect.left();
        if self.tick {
            paint::tick(p, x, cy, &t);
            x += 2.0 + 6.0;
        }
        if let Some(icon) = self.icon {
            icon.paint(
                p,
                Rect::from_min_size(Pos2::new(x, cy - 7.0), Vec2::splat(size::ICON_SM)),
                t.fg_faint,
            );
            x += size::ICON_SM + 6.0;
        }
        let room = (rect.right() - reserved - if reserved > 0.0 { 6.0 } else { 0.0 } - x).max(0.0);
        let g = paint::tracked(
            ui,
            self.text,
            font(Face::Mono, size::TEXT_2XS),
            t.fg_muted,
            0.08,
            true,
            Some(room),
        );
        paint::text_at(p, &g, x, cy, t.fg_muted);
    }
}

impl Widget for SectionLabel<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui, rect, 0.0);
        }
        resp
    }
}

/// One tile of a [`StatStrip`]: the number on top (mono, in its tone), what it is under
/// it (10px, faint). `Stat::new("Players", "258 643").tone(t.ok)`.
#[must_use = "add it to a StatStrip"]
pub struct Stat<'a> {
    label: &'a str,
    value: &'a str,
    tone: Option<Color32>,
    icon: Option<Icon>,
    grow: bool,
    tip: &'a str,
}

impl<'a> Stat<'a> {
    pub fn new(label: &'a str, value: &'a str) -> Self {
        Self {
            label,
            value,
            tone: None,
            icon: None,
            grow: false,
            tip: "",
        }
    }

    pub fn tone(mut self, c: Color32) -> Self {
        self.tone = Some(c);
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// A tile of words rather than figures: takes the slack.
    pub fn grow(mut self) -> Self {
        self.grow = true;
        self
    }

    pub fn tip(mut self, t: &'a str) -> Self {
        self.tip = t;
        self
    }
}

/// `ui/stat`'s strip: tiles side by side, hairlines between, a line under.
/// `StatStrip::show(ui, &[Stat::new(…), …])`.
pub struct StatStrip;

impl StatStrip {
    pub fn show(ui: &mut Ui, stats: &[Stat<'_>]) -> Response {
        let t = ui.tokens();
        let w = ui.available_width();
        let vf = font(Face::Mono, size::TEXT_2XS);
        let lf = font(Face::Sans, size::TEXT_3XS);
        let tiles: Vec<(Arc<egui::Galley>, Arc<egui::Galley>, f32)> = stats
            .iter()
            .map(|s| {
                let v = paint::galley(ui, s.value, vf.clone(), s.tone.unwrap_or(t.fg));
                let l = paint::galley(ui, s.label, lf.clone(), t.fg_faint);
                let inner = v
                    .size()
                    .x
                    .max(l.size().x + s.icon.map_or(0.0, |_| size::ICON_SM + 4.0));
                let min = if s.grow { 80.0 } else { 56.0 };
                (v, l, (inner + 16.0).max(min))
            })
            .collect();
        let natural: f32 = tiles.iter().map(|x| x.2).sum();
        let grows = stats.iter().filter(|s| s.grow).count().max(1) as f32;
        let slack = ((w - natural) / grows).max(0.0);
        let h = 4.0 + 15.0 + 1.0 + 12.0 + 4.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let p = ui.painter();
        p.hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, t.border),
        );
        let mut x = rect.left();
        for (i, (s, (v, l, tw))) in stats.iter().zip(&tiles).enumerate() {
            let tw = tw + if s.grow { slack } else { 0.0 };
            let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(tw, h - 1.0));
            x += tw;
            let ink = s.tone.unwrap_or(t.fg);
            p.galley_with_override_text_color(
                Pos2::new(r.left() + 8.0, r.top() + 4.0),
                v.clone(),
                ink,
            );
            let mut lx = r.left() + 8.0;
            let ly = r.top() + 4.0 + 16.0 + 6.0;
            if let Some(icon) = s.icon {
                icon.paint(
                    p,
                    Rect::from_center_size(Pos2::new(lx + 7.0, ly), Vec2::splat(size::ICON_SM)),
                    t.fg_faint,
                );
                lx += size::ICON_SM + 4.0;
            }
            paint::text_at(p, l, lx, ly, t.fg_faint);
            if i + 1 < stats.len() {
                p.vline(
                    r.right() - 0.5,
                    r.y_range(),
                    Stroke::new(1.0, alpha(t.border, 0.5)),
                );
            }
            if !s.tip.is_empty() {
                let tr = ui.interact(r, resp.id.with(i), Sense::hover());
                let _ = super::TipExt::tip(tr, s.tip);
            }
        }
        resp
    }
}

/// `ui/empty-state`: what a list says when it has nothing in it: an icon in a well, a
/// title, a sentence, and the button that would put something there.
/// `EmptyState::new().icon(lucide::STAR).title("No favourites").text("…").show(ui, |ui| …)`.
#[must_use = "call show(…)"]
pub struct EmptyState<'a> {
    icon: Option<Icon>,
    title: &'a str,
    text: &'a str,
    error: &'a str,
    detail: &'a str,
    compact: bool,
}

impl Default for EmptyState<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> EmptyState<'a> {
    pub fn new() -> Self {
        Self {
            icon: None,
            title: "",
            text: "",
            error: "",
            detail: "",
            compact: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }

    pub fn text(mut self, t: &'a str) -> Self {
        self.text = t;
        self
    }

    /// What went wrong, in red and selectable, before the text.
    pub fn error(mut self, e: &'a str) -> Self {
        self.error = e;
        self
    }

    /// A raw detail (an error's own words) after the text: mono, faint, selectable.
    pub fn detail(mut self, d: &'a str) -> Self {
        self.detail = d;
        self
    }

    /// One line for a narrow column.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// The full form centres itself in the space on offer; `action` adds its buttons.
    pub fn show(self, ui: &mut Ui, action: impl FnOnce(&mut Ui)) {
        let t = ui.tokens();
        if self.compact {
            let w = ui.available_width();
            let mut x0 = 8.0;
            if self.icon.is_some() {
                x0 += size::ICON_SM + 6.0;
            }
            let mut job = LayoutJob::default();
            let f = font(Face::Sans, size::TEXT_2XS);
            if !self.title.is_empty() {
                job.append(self.title, 0.0, TextFormat::simple(f.clone(), t.fg));
                job.append(" ", 0.0, TextFormat::simple(f.clone(), t.fg_muted));
            }
            job.append(self.text, 0.0, TextFormat::simple(f, t.fg_muted));
            job.wrap.max_width = (w - x0 - 8.0).max(20.0);
            let g = ui.painter().layout_job(job);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(w, g.size().y + 16.0), Sense::hover());
            if let Some(icon) = self.icon {
                icon.paint(
                    ui.painter(),
                    Rect::from_min_size(
                        Pos2::new(rect.left() + 8.0, rect.top() + 10.0),
                        Vec2::splat(size::ICON_SM),
                    ),
                    t.fg_faint,
                );
            }
            ui.painter()
                .galley(Pos2::new(rect.left() + x0, rect.top() + 8.0), g, t.fg_muted);
            return;
        }
        let full = ui.available_rect_before_wrap();
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(full.shrink(24.0))
                .layout(Layout::top_down(Align::Center)),
        );
        child.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
        // Centred vertically: the height is last frame's, the first frame settles at once.
        let hid = ui.id().with(("empty-h", self.title));
        let last_h: f32 = ui.data(|d| d.get_temp(hid)).unwrap_or(0.0);
        child.add_space(((full.height() - 48.0 - last_h) / 2.0).max(0.0));
        let start = child.cursor().top();
        if let Some(icon) = self.icon {
            let (r, _) = child.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            paint::plate(child.painter(), r, size::RADIUS_MD, t.bg, t.border);
            icon.paint(
                child.painter(),
                Rect::from_center_size(r.center(), Vec2::splat(size::ICON_LG)),
                t.fg_faint,
            );
        }
        if !self.title.is_empty() {
            let g = paint::galley_wrap(
                &child,
                self.title,
                font(Face::SansMedium, size::TEXT_SM),
                t.fg,
                300.0,
            );
            let (r, _) = child.allocate_exact_size(g.size(), Sense::hover());
            child.painter().galley(r.min, g, t.fg);
        }
        if !self.error.is_empty() {
            let w = 300.0_f32.min(full.width());
            child.allocate_ui(Vec2::new(w, 0.0), |ui| {
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(self.error)
                                .font(font(Face::Sans, size::TEXT_XS))
                                .color(t.err),
                        )
                        .wrap()
                        .selectable(true),
                    );
                });
            });
        }
        if !self.text.is_empty() {
            let mut job = LayoutJob::simple(
                self.text.to_owned(),
                font(Face::Sans, size::TEXT_XS),
                t.fg_muted,
                300.0,
            );
            job.halign = Align::Center;
            let g = child.painter().layout_job(job);
            let (r, _) = child.allocate_exact_size(
                Vec2::new(300.0_f32.min(full.width()), g.size().y),
                Sense::hover(),
            );
            child
                .painter()
                .galley(Pos2::new(r.center().x, r.top()), g, t.fg_muted);
        }
        if !self.detail.is_empty() {
            let w = 300.0_f32.min(full.width());
            child.allocate_ui(Vec2::new(w, 0.0), |ui| {
                ui.with_layout(Layout::top_down(Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(self.detail)
                                .font(font(Face::Mono, size::TEXT_3XS))
                                .color(t.fg_faint),
                        )
                        .wrap()
                        .selectable(true),
                    );
                });
            });
        }
        child.add_space(4.0);
        // The buttons centred under the words: a row as wide as they were last frame.
        let aw_id = hid.with("actions-w");
        let aw: f32 = ui.data(|d| d.get_temp(aw_id)).unwrap_or(0.0);
        let row = child.allocate_ui_with_layout(
            Vec2::new(aw.max(1.0), 0.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                action(ui);
            },
        );
        let used = row.response.rect.width();
        if (used - aw).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(aw_id, used));
            ui.ctx().request_repaint();
        }
        let h = child.cursor().top() - start;
        if (h - last_h).abs() > 0.5 {
            ui.data_mut(|d| d.insert_temp(hid, h));
            ui.ctx().request_repaint();
        }
        ui.allocate_rect(full, Sense::hover());
    }
}

/// `ui/disclosure`: a quiet line that opens onto more (`h-row px-2 text-2xs
/// text-fg-faint`, a chevron that turns). `Disclosure::new("details", "Details").show(ui, |ui| …)`.
#[must_use = "call show(…)"]
pub struct Disclosure<'a> {
    id: Id,
    label: &'a str,
    default_open: bool,
}

impl<'a> Disclosure<'a> {
    pub fn new(id_salt: impl egui::AsId, label: &'a str) -> Self {
        Self {
            id: Id::new(id_salt),
            label,
            default_open: false,
        }
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Returns the body's result when open.
    pub fn show<R>(self, ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let t = ui.tokens();
        let mut open: bool = ui.data_mut(|d| *d.get_persisted_mut_or(self.id, self.default_open));
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::ROW), Sense::click());
        if resp.clicked() {
            open = !open;
            ui.data_mut(|d| d.insert_persisted(self.id, open));
        }
        let label = self.label;
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::CollapsingHeader, true, open, label)
        });
        let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
        let p = ui.painter();
        p.rect_filled(
            rect,
            CornerRadius::ZERO,
            paint::lerp(Color32::TRANSPARENT, alpha(t.raised, 0.5), hover),
        );
        let ink = if open {
            t.fg_muted
        } else {
            paint::lerp(t.fg_faint, t.fg_muted, hover)
        };
        let chev = if open {
            lucide::CHEVRON_DOWN
        } else {
            lucide::CHEVRON_RIGHT
        };
        chev.paint(
            p,
            Rect::from_min_size(
                Pos2::new(rect.left() + 8.0, rect.center().y - 7.0),
                Vec2::splat(size::ICON_SM),
            ),
            ink,
        );
        let x = rect.left() + 8.0 + size::ICON_SM + 4.0;
        let g = paint::galley_trunc(
            ui,
            self.label,
            font(Face::Sans, size::TEXT_2XS),
            ink,
            (rect.right() - 8.0 - x).max(0.0),
        );
        paint::text_at(p, &g, x, rect.center().y, ink);
        let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        open.then(|| body(ui))
    }
}
