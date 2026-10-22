//! Blocks a view is built of: `Section` (a titled block of a panel), `SectionCard` (a
//! titled card), `SettingsSection` (a part of the settings page), `SectionNav` (a long
//! page's own contents), `BulkBar` (the strip under a list while some of it is ticked),
//! `Empty` (nothing here, over the contour texture), `MasterDetail` (a list and the
//! details of its selection) and `Lightbox` (a picture over everything).

use egui::{
    Align, Color32, CornerRadius, Id, Layout, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2,
};

use super::text::draw;
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{EmptyState, Pane, SectionLabel, Split, panel, topo};
use crate::theme::{TokensExt, font, fonts::Face, motion, size};

/// Paints what a closure drew a ground under: a slot reserved before, filled after.
fn ground(ui: &Ui) -> egui::layers::ShapeIdx {
    ui.painter().add(egui::Shape::Noop)
}

// ---------------------------------------------------------------------------------------

/// `Section`: one titled block of a panel: the stencilled label with its icon and
/// controls, then the content (`px-pad py-2.5 gap-2`). Consecutive sections are divided
/// by a rule; the first has none (`.first()`).
pub struct Section<'a> {
    title: &'a str,
    icon: Option<Icon>,
    first: bool,
}

impl<'a> Section<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            icon: None,
            first: false,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The first section of its panel: no rule above it.
    pub fn first(mut self) -> Self {
        self.first = true;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        actions: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let tk = ui.tokens();
        let top = ui.cursor().top();
        let full = ui.available_rect_before_wrap();
        if !self.first {
            ui.painter()
                .hline(full.x_range(), top + 0.5, Stroke::new(1.0, tk.border));
        }
        let inner = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(size::PAD as i8, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 8.0;
                let mut l = SectionLabel::new(self.title);
                if let Some(i) = self.icon {
                    l = l.icon(i);
                }
                l.show_with(ui, actions);
                body(ui)
            });
        inner.inner
    }
}

// ---------------------------------------------------------------------------------------

/// `SectionCard`: a titled card of related settings or facts (`rounded-md border
/// bg-bg/40`): a header strip (icon in its tone, stencilled title, a short line of what it
/// is for, actions at the end), then the body, `padded` or not.
pub struct SectionCard<'a> {
    title: &'a str,
    description: &'a str,
    icon: Option<Icon>,
    tone: Option<Color32>,
    padded: bool,
}

impl<'a> SectionCard<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            description: "",
            icon: None,
            tone: None,
            padded: false,
        }
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = d;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The icon's colour (the accent by default).
    pub fn tone(mut self, c: Color32) -> Self {
        self.tone = Some(c);
        self
    }

    /// `p-3` around the body.
    pub fn padded(mut self) -> Self {
        self.padded = true;
        self
    }

    pub fn show<R>(
        self,
        ui: &mut Ui,
        actions: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let tk = ui.tokens();
        let slot = ground(ui);
        let w = ui.available_width();
        let top = ui.cursor().top();
        let left = ui.cursor().left();
        // Header: min-h-control, px-3 py-1, the raised wash, a rule under.
        let header = Rect::from_min_size(Pos2::new(left, top), Vec2::new(w, size::CONTROL));
        let (hrect, _) = ui.allocate_exact_size(header.size(), Sense::hover());
        {
            let p = ui.painter();
            p.rect_filled(
                hrect,
                CornerRadius {
                    nw: size::RADIUS_MD,
                    ne: size::RADIUS_MD,
                    sw: 0,
                    se: 0,
                },
                alpha(tk.raised, 0.3),
            );
            p.hline(
                hrect.x_range(),
                hrect.bottom() - 0.5,
                Stroke::new(1.0, tk.border),
            );
        }
        let mut hui = ui.new_child(
            UiBuilder::new()
                .max_rect(hrect.shrink2(Vec2::new(12.0, 0.0)))
                .layout(Layout::right_to_left(Align::Center)),
        );
        hui.spacing_mut().item_spacing.x = 4.0;
        actions(&mut hui);
        let reserved = hui.min_rect().width();
        let cy = hrect.center().y;
        let mut x = hrect.left() + 12.0;
        let room = hrect.right() - 12.0 - reserved - 8.0;
        if let Some(icon) = self.icon {
            icon.paint(
                ui.painter(),
                Rect::from_min_size(Pos2::new(x, cy - 7.0), Vec2::splat(size::ICON_SM)),
                self.tone.unwrap_or(tk.accent),
            );
            x += size::ICON_SM + 8.0;
        }
        let tg = panel::stencil(
            ui,
            self.title,
            Color32::PLACEHOLDER,
            Some((room - x).max(8.0)),
        );
        draw(ui.painter(), &tg, x, cy, tk.fg_muted);
        x += tg.size().x + 8.0;
        if !self.description.is_empty() && room - x > 16.0 {
            let dg = super::text::ph_trunc(
                ui,
                self.description,
                font(Face::Sans, size::TEXT_2XS),
                room - x,
            );
            draw(ui.painter(), &dg, x, cy, tk.fg_faint);
        }
        let margin = if self.padded { 12 } else { 0 };
        let r = egui::Frame::new()
            .inner_margin(egui::Margin::same(margin))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                body(ui)
            });
        let rect = Rect::from_min_max(
            Pos2::new(left, top),
            Pos2::new(left + w, r.response.rect.bottom()),
        );
        ui.painter().set(
            slot,
            egui::Shape::rect_filled(rect, CornerRadius::same(size::RADIUS_MD), alpha(tk.bg, 0.4)),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(size::RADIUS_MD),
            Stroke::new(1.0, tk.border),
            StrokeKind::Inside,
        );
        r.inner
    }
}

// ---------------------------------------------------------------------------------------

/// `SettingsSection`: one part of the settings page: the icon, a title in the display face,
/// a sentence of what it governs, something on the right (`aside`), then its controls in a
/// panel (`rounded-md border bg-bg/60`). `.danger()` turns it red. The response's rect is
/// what the page's navigation scrolls to.
pub struct SettingsSection<'a> {
    title: &'a str,
    description: &'a str,
    icon: Option<Icon>,
    danger: bool,
}

impl<'a> SettingsSection<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            description: "",
            icon: None,
            danger: false,
        }
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = d;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn show(
        self,
        ui: &mut Ui,
        aside: impl FnOnce(&mut Ui),
        body: impl FnOnce(&mut Ui),
    ) -> Response {
        let tk = ui.tokens();
        let tone = if self.danger { tk.err } else { tk.accent };
        let ink = if self.danger { tk.err } else { tk.fg };
        let top = ui.cursor().top();
        let w = ui.available_width();
        let left = ui.cursor().left();
        // The aside first, at the right, so the words know their room.
        let mut aui = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(
                    Pos2::new(left, top),
                    Vec2::new(w, 28.0),
                ))
                // `items-start`: the aside's top on the title's.
                .layout(Layout::right_to_left(Align::Min)),
        );
        aui.spacing_mut().item_spacing.x = 6.0;
        aside(&mut aui);
        let reserved = aui.min_rect().width();
        let mut x = left;
        if let Some(icon) = self.icon {
            icon.paint(
                ui.painter(),
                Rect::from_min_size(Pos2::new(x, top + 3.0), Vec2::splat(size::ICON_LG)),
                tone,
            );
            x += size::ICON_LG + 8.0;
        }
        let room = (left + w - reserved - 8.0 - x).max(40.0);
        let tg = paint::tracked(
            ui,
            self.title,
            font(Face::DisplayBold, size::TEXT_XL),
            Color32::PLACEHOLDER,
            0.02,
            true,
            Some(room),
        );
        // text-xl at leading 1.05
        let line_h = size::TEXT_XL * 1.05;
        ui.painter()
            .galley(Pos2::new(x, top + (line_h - tg.size().y) / 2.0), tg, ink);
        let mut y = top + line_h;
        if !self.description.is_empty() {
            // `mt-0.5 text-xs` (line height 1.45).
            y += 2.0;
            let (dg, off) = paint::galley_lh(
                ui,
                self.description,
                font(Face::Sans, size::TEXT_XS),
                Color32::PLACEHOLDER,
                room,
                size::TEXT_XS * 1.45,
            );
            ui.painter()
                .galley(Pos2::new(x, y + off), dg.clone(), tk.fg_muted);
            y += dg.size().y;
        }
        let head_bottom = y.max(top + 26.0) + 8.0;
        ui.allocate_rect(
            Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + w, head_bottom)),
            Sense::hover(),
        );
        let slot = ground(ui);
        // The panel's 1px border is outside its rows, as the CSS box has it.
        let r = egui::Frame::new()
            .inner_margin(egui::Margin::same(1))
            .show(ui, |ui| {
                ui.set_width(w - 2.0);
                body(ui);
            });
        let panel = r.response.rect;
        ui.painter().set(
            slot,
            egui::Shape::rect_filled(
                panel,
                CornerRadius::same(size::RADIUS_MD),
                alpha(tk.bg, 0.6),
            ),
        );
        let border = if self.danger {
            alpha(tk.err, 0.4)
        } else {
            tk.border
        };
        ui.painter().rect_stroke(
            panel,
            CornerRadius::same(size::RADIUS_MD),
            Stroke::new(1.0, border),
            StrokeKind::Inside,
        );
        ui.interact(
            Rect::from_min_max(Pos2::new(left, top), panel.max),
            ui.next_auto_id(),
            Sense::hover(),
        )
    }
}

// ---------------------------------------------------------------------------------------

/// One entry of a [`SectionNav`].
#[derive(Clone, Copy, Debug)]
pub struct NavEntry<'a> {
    pub id: &'a str,
    pub label: &'a str,
    /// A small warning dot: unsaved edits there.
    pub dot: bool,
}

/// `SectionNav`: a page's own table of contents (`w-52 border-r bg-bg/40 px-2 py-4`), the
/// section being read lit. Answers the entry clicked; the page scrolls to it. Which one is
/// being read is [`SectionNav::spy`]'s answer.
pub struct SectionNav<'a> {
    title: &'a str,
    sections: &'a [NavEntry<'a>],
    active: &'a str,
}

impl<'a> SectionNav<'a> {
    pub fn new(title: &'a str, sections: &'a [NavEntry<'a>], active: &'a str) -> Self {
        Self {
            title,
            sections,
            active,
        }
    }

    /// The section being read: the last whose top has passed `view_top + 24` (the first
    /// when none has). `tops` are the sections' tops on screen, in order.
    pub fn spy<'s>(view_top: f32, tops: &[(&'s str, f32)]) -> Option<&'s str> {
        let mut best = tops.first().map(|(id, _)| *id);
        for (id, y) in tops {
            if *y <= view_top + 24.0 {
                best = Some(id);
            }
        }
        best
    }

    /// Fills the height it is given, 208px wide.
    pub fn show(self, ui: &mut Ui) -> Option<&'a str> {
        let tk = ui.tokens();
        let full = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(full.min, Vec2::new(208.0, full.height()));
        ui.allocate_rect(rect, Sense::hover());
        // An index: a controller's moves stay in it until it has nothing more that way.
        crate::pad::region(ui.ctx(), rect);
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, alpha(tk.bg, 0.4));
        p.vline(
            rect.right() - 0.5,
            rect.y_range(),
            Stroke::new(1.0, tk.border),
        );
        let mut y = rect.top() + 16.0;
        let x = rect.left() + 8.0;
        let w = rect.width() - 16.0;
        let tg = panel::stencil(ui, self.title, Color32::PLACEHOLDER, Some(w - 16.0));
        draw(ui.painter(), &tg, x + 8.0, y + 5.5, tk.fg_faint);
        y += 11.0 + 8.0;
        let mut clicked = None;
        for (i, s) in self.sections.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, size::CONTROL));
            let resp = ui.interact(r, ui.id().with(("section-nav", i)), Sense::click());
            let on = s.id == self.active;
            let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered());
            let p = ui.painter();
            if on {
                p.rect_filled(r, CornerRadius::same(size::RADIUS_SM), tk.raised);
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(r.left(), r.top() + 6.0),
                        Pos2::new(r.left() + 2.0, r.bottom() - 6.0),
                    ),
                    CornerRadius::same(1),
                    tk.accent,
                );
            } else if hover > 0.0 {
                p.rect_filled(
                    r,
                    CornerRadius::same(size::RADIUS_SM),
                    alpha(tk.raised, 0.6 * hover),
                );
            }
            let ink = if on {
                tk.fg
            } else {
                paint::lerp(tk.fg_muted, tk.fg, hover)
            };
            let room = w - 16.0 - if s.dot { 14.0 } else { 0.0 };
            let g = super::text::ph_trunc(ui, s.label, font(Face::Sans, size::TEXT_XS), room);
            draw(p, &g, r.left() + 8.0, r.center().y, ink);
            if s.dot {
                p.circle_filled(Pos2::new(r.right() - 11.0, r.center().y), 3.0, tk.warn);
            }
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                clicked = Some(s.id);
            }
            y += size::CONTROL + 2.0;
        }
        clicked
    }
}

// ---------------------------------------------------------------------------------------

/// `BulkBar`: the strip at the foot of a list while some of it is ticked: an accent mark,
/// how many ("3 mods selected"), a quieter figure, buttons right after (`lead`), and the
/// bulk actions at the far end. It slides up when it appears.
pub struct BulkBar<'a> {
    label: &'a str,
    info: &'a str,
}

impl<'a> BulkBar<'a> {
    pub fn new(label: &'a str) -> Self {
        Self { label, info: "" }
    }

    pub fn info(mut self, info: &'a str) -> Self {
        self.info = info;
        self
    }

    pub fn show(
        self,
        ui: &mut Ui,
        lead: impl FnOnce(&mut Ui),
        actions: impl FnOnce(&mut Ui),
    ) -> Response {
        let tk = ui.tokens();
        let w = ui.available_width();
        let h = 6.0 + size::CONTROL_SM + 6.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
        let rise = paint::ease_for(ui, resp.id.with("in"), true, motion::BASE);
        let rect_shown = rect.translate(Vec2::new(0.0, 6.0 * (1.0 - rise)));
        let p = ui.painter();
        p.rect_filled(rect_shown, CornerRadius::ZERO, tk.bg);
        p.hline(
            rect_shown.x_range(),
            rect_shown.top() + 0.5,
            Stroke::new(1.0, alpha(tk.accent, 0.3)),
        );
        let mut c = ui.new_child(
            UiBuilder::new()
                .max_rect(rect_shown.shrink2(Vec2::new(size::PAD, 6.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        c.multiply_opacity(rise);
        c.spacing_mut().item_spacing.x = 6.0;
        let (bar, _) = c.allocate_exact_size(Vec2::new(2.0, 16.0), Sense::hover());
        c.painter()
            .rect_filled(bar, CornerRadius::same(1), tk.accent);
        let lg = super::text::ph(&c, self.label, font(Face::SansMedium, size::TEXT_XS));
        let (lr, _) = c.allocate_exact_size(lg.size(), Sense::hover());
        draw(c.painter(), &lg, lr.left(), lr.center().y, tk.fg);
        if !self.info.is_empty() {
            let ig = super::text::ph(&c, self.info, font(Face::Mono, size::TEXT_2XS));
            let (ir, _) = c.allocate_exact_size(ig.size(), Sense::hover());
            draw(c.painter(), &ig, ir.left(), ir.center().y, tk.fg_faint);
        }
        lead(&mut c);
        c.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            actions(ui);
        });
        resp
    }
}

// ---------------------------------------------------------------------------------------

/// `Empty`: nothing here: the kit's empty state over the contour texture, filling what it
/// is given (at least 192px tall).
pub struct Empty<'a> {
    icon: Option<Icon>,
    title: &'a str,
    text: &'a str,
    error: &'a str,
    detail: &'a str,
    compact: bool,
}

impl<'a> Default for Empty<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Empty<'a> {
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

    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// `action` adds its buttons under the words.
    pub fn show(self, ui: &mut Ui, action: impl FnOnce(&mut Ui)) {
        let tk = ui.tokens();
        let avail = ui.available_rect_before_wrap();
        let rect = Rect::from_min_size(
            avail.min,
            Vec2::new(avail.width(), avail.height().max(192.0)),
        );
        topo::paint(&ui.painter().with_clip_rect(rect), rect, &tk, 0.5);
        let mut c = ui.new_child(UiBuilder::new().max_rect(rect));
        let mut e = EmptyState::new()
            .title(self.title)
            .text(self.text)
            .error(self.error)
            .detail(self.detail);
        if let Some(i) = self.icon {
            e = e.icon(i);
        }
        if self.compact {
            e = e.compact();
        }
        e.show(&mut c, action);
        ui.allocate_rect(rect, Sense::hover());
    }
}

// ---------------------------------------------------------------------------------------

/// `MasterDetail`: a list and the details of what is selected in it. Wide: side by side,
/// the details pane resizable and remembered under `id`. Narrow (below `breakpoint` of its
/// own width): the list takes the whole width and the details slide over its right edge
/// while something is selected.
pub struct MasterDetail {
    id: Id,
    show: bool,
    selected: bool,
    initial: f32,
    min: f32,
    max: f32,
    breakpoint: f32,
}

impl MasterDetail {
    pub fn new(id_salt: impl egui::AsId) -> Self {
        Self {
            id: Id::new(id_salt),
            show: true,
            selected: true,
            initial: 400.0,
            min: 320.0,
            max: 620.0,
            breakpoint: 980.0,
        }
    }

    /// The details pane is wanted at all (the view's toggle).
    pub fn detail_shown(mut self, on: bool) -> Self {
        self.show = on;
        self
    }

    /// Something is selected: the details pane only shows then.
    pub fn selected(mut self, on: bool) -> Self {
        self.selected = on;
        self
    }

    pub fn initial(mut self, w: f32) -> Self {
        self.initial = w;
        self
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    pub fn breakpoint(mut self, w: f32) -> Self {
        self.breakpoint = w;
        self
    }

    pub fn show(self, ui: &mut Ui, main: impl FnOnce(&mut Ui), detail: impl FnOnce(&mut Ui)) {
        let rect = ui.available_rect_before_wrap();
        let narrow = rect.width() > 0.0 && rect.width() < self.breakpoint;
        let open = self.show && self.selected;
        if open && !narrow {
            Split::new(self.id, self.initial)
                .pane(Pane::End)
                .range(self.min, self.max)
                .keep((self.breakpoint - self.initial).min(520.0))
                .show(ui, main, detail);
            return;
        }
        let mut mui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(Layout::top_down(Align::Min)),
        );
        main(&mut mui);
        if open {
            let tk = ui.tokens();
            let a = paint::ease_for(ui, self.id.with("drawer"), true, motion::BASE);
            let w = (rect.width() * 0.88).min(384.0);
            let dr = Rect::from_min_max(Pos2::new(rect.right() - w, rect.top()), rect.max);
            let p = ui.painter();
            p.add(paint::shadow_pop().as_shape(dr, CornerRadius::ZERO));
            p.rect_filled(dr, CornerRadius::ZERO, tk.bg);
            p.vline(
                dr.left() + 0.5,
                dr.y_range(),
                Stroke::new(1.0, tk.border_strong),
            );
            // The drawer takes the pointer over the list, and the controller's moves.
            let bg = ui.interact(dr, self.id.with("drawer-bg"), Sense::click());
            crate::pad::cover(ui.ctx(), bg.id, ui.layer_id(), dr);
            crate::pad::region(ui.ctx(), dr);
            let mut dui = ui.new_child(
                UiBuilder::new()
                    .max_rect(dr)
                    .layout(Layout::top_down(Align::Min)),
            );
            dui.multiply_opacity(a);
            detail(&mut dui);
        }
        ui.allocate_rect(rect, Sense::hover());
    }
}

// ---------------------------------------------------------------------------------------

/// `Lightbox`: a picture over everything (`bg-plot/90 p-8`), closed by a click anywhere,
/// the round button or Escape. `open` false is closed.
pub struct Lightbox<'a> {
    image: egui::ImageSource<'a>,
    close_label: &'a str,
}

impl<'a> Lightbox<'a> {
    pub fn new(image: impl Into<egui::ImageSource<'a>>, close_label: &'a str) -> Self {
        Self {
            image: image.into(),
            close_label,
        }
    }

    pub fn show(self, ctx: &egui::Context, open: &mut bool) {
        if !*open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            *open = false;
            return;
        }
        let tk = ctx.tokens();
        let screen = ctx.content_rect();
        let id = Id::new("parts-lightbox");
        egui::Area::new(id)
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let a = paint::ease_for(ui, id.with("in"), true, motion::FAST);
                let (rect, bg) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, alpha(tk.plot, 0.9 * a));
                let inner = rect.shrink(32.0);
                let img = egui::Image::new(self.image)
                    .max_size(inner.size())
                    .maintain_aspect_ratio(true)
                    .corner_radius(CornerRadius::same(size::RADIUS_MD));
                let mut iui = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inner)
                        .layout(Layout::centered_and_justified(egui::Direction::TopDown)),
                );
                iui.multiply_opacity(a);
                let shown = iui.add(img.sense(Sense::click()));
                let close = Rect::from_min_size(
                    Pos2::new(rect.right() - 16.0 - size::CONTROL_LG, rect.top() + 16.0),
                    Vec2::splat(size::CONTROL_LG),
                );
                let cr = ui.interact(close, id.with("close"), Sense::click());
                let h = paint::ease(ui, cr.id.with("hover"), cr.hovered());
                ui.painter()
                    .circle_filled(close.center(), size::CONTROL_LG / 2.0, tk.raised);
                lucide::X.paint(
                    ui.painter(),
                    Rect::from_center_size(close.center(), Vec2::splat(size::ICON)),
                    paint::lerp(tk.fg_muted, tk.fg, h),
                );
                let cr = crate::kit::TipExt::tip(cr, self.close_label);
                if bg.clicked() || shown.clicked() || cr.clicked() {
                    *open = false;
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spy_picks_the_last_section_passed() {
        let tops = [("a", 0.0), ("b", 100.0), ("c", 400.0)];
        assert_eq!(SectionNav::spy(90.0, &tops), Some("b"));
        assert_eq!(SectionNav::spy(-100.0, &tops), Some("a"));
        assert_eq!(SectionNav::spy(1000.0, &tops), Some("c"));
        assert_eq!(SectionNav::spy(0.0, &[]), None);
    }
}
