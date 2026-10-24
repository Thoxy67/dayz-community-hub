//! `ui/dialog` (Dialog, Confirm): a question that has to be answered before anything
//! else happens.
//!
//! The veil is flat `bg-plot/75`; the plate `bg-panel border-border-strong rounded-md
//! shadow-pop`, 22 / 32 / 44 rem wide (never wider than the window less 1rem), popping in
//! (140ms, from 97% and 2px down) and fading out (100ms). Header: the accent tick, the
//! title's first line (`text-sm font-semibold`) and the close button on one centre line,
//! the description under the title. Body
//! scrolls (`px-3 py-2.5 text-xs`); footer `border-t px-3 py-2`, buttons at the end, the
//! safe one first. Escape and a click on the veil close it.

use egui::{
    Align, Context, CornerRadius, Id, Layout, Margin, Rect, Sense, Stroke, Ui, UiBuilder, Vec2,
};

use super::button::{Button, Variant};
use super::paint;
use crate::icons::lucide;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What the window keeps at its bottom over the veil this frame (in pad mode the status
/// bar and the pad hints' bar, drawn above dialogs): dialogs centre in the room above it
/// and never reach into it. Set by the window before the dialogs are drawn.
pub fn set_bottom_inset(ctx: &Context, px: f32) {
    if px.is_finite() {
        ctx.data_mut(|d| d.insert_temp(inset_id(), px.clamp(0.0, 400.0)));
    }
}

/// The room kept at the window's bottom ([`set_bottom_inset`]), 0 when none.
pub fn bottom_inset(ctx: &Context) -> f32 {
    ctx.data(|d| d.get_temp::<f32>(inset_id())).unwrap_or(0.0)
}

fn inset_id() -> Id {
    Id::unique("kit::dialog.bottom_inset")
}

/// How wide a dialog is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogSize {
    /// 22rem.
    #[default]
    Sm,
    /// 32rem.
    Md,
    /// 44rem.
    Lg,
}

impl DialogSize {
    fn width(self) -> f32 {
        match self {
            DialogSize::Sm => 352.0,
            DialogSize::Md => 512.0,
            DialogSize::Lg => 704.0,
        }
    }
}

/// `Dialog::new("wipe", "Wipe every setting?").description(…).show(ctx, &mut open, body, footer)`.
#[must_use = "call show(…)"]
pub struct Dialog<'a> {
    id: Id,
    title: &'a str,
    description: &'a str,
    size: DialogSize,
    close_label: &'a str,
    body: bool,
    footer: bool,
}

impl<'a> Dialog<'a> {
    pub fn new(id_salt: impl egui::AsIdSalt, title: &'a str) -> Self {
        Self {
            id: Id::unique(id_salt),
            title,
            description: "",
            size: DialogSize::Sm,
            close_label: "",
            body: true,
            footer: true,
        }
    }

    /// Header and footer only (the body closure is not called).
    pub fn without_body(mut self) -> Self {
        self.body = false;
        self
    }

    /// Header and body only (the footer closure is not called): a sheet that is read and
    /// closed from its header, such as the shortcuts.
    pub fn without_footer(mut self) -> Self {
        self.footer = false;
        self
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = d;
        self
    }

    pub fn size(mut self, s: DialogSize) -> Self {
        self.size = s;
        self
    }

    /// The close button's accessible name.
    pub fn close_label(mut self, l: &'a str) -> Self {
        self.close_label = l;
        self
    }

    /// Shows the dialog while `open` (and for its fade-out after). `body` and `footer`
    /// get the dialog's open flag through the `&mut bool` to close it themselves; pass
    /// `|_, _| {}` for a part it does not have. Returns `true` on the frame it closed.
    pub fn show(
        self,
        ctx: &Context,
        open: &mut bool,
        body: impl FnOnce(&mut Ui, &mut bool),
        footer: impl FnOnce(&mut Ui, &mut bool),
    ) -> bool {
        let was = *open;
        let a = ctx.animate_bool_with_time(
            self.id.with("shown"),
            *open,
            if *open { 0.14 } else { 0.10 },
        );
        if a <= 0.0 && !*open {
            return false;
        }
        let t = ctx.tokens();
        let screen = ctx.content_rect();
        let w = self
            .size
            .width()
            .min(screen.width() - 16.0)
            .min(screen.width() - 32.0)
            .max(120.0);
        // Centred in the room above what the window keeps at its bottom (the pad's bar).
        let inset = bottom_inset(ctx);
        let max_h = (screen.height() - 32.0 - inset).max(120.0);
        let lift = 2.0 * (1.0 - a) - inset / 2.0;
        let area = egui::Modal::default_area(self.id)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, lift));
        let modal = egui::Modal::new(self.id)
            .area(area)
            .frame(egui::Frame::NONE)
            .backdrop_color(egui::Color32::TRANSPARENT);
        let mut close = false;
        let footer_w_id = self.id.with("footer-w");
        let r = modal.show(ctx, |ui| {
            paint::veil(ui.painter(), paint::alpha(t.plot, 0.75 * a));
            ui.multiply_opacity(a);
            let scale = 0.97 + 0.03 * a;
            let w = w * scale;
            egui::Frame::new()
                .fill(t.panel)
                .stroke(Stroke::new(1.0, t.border_strong))
                .corner_radius(CornerRadius::same(size::RADIUS_MD))
                .shadow(paint::shadow_pop())
                .show(ui, |ui| {
                    ui.set_width(w);
                    ui.set_max_height(max_h);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    // Header.
                    let text_w = w - 24.0 - 2.0 - 8.0 - 8.0 - size::CONTROL_SM;
                    let tg = paint::galley_wrap(
                        ui,
                        self.title,
                        font(Face::SansSemibold, size::TEXT_SM),
                        t.fg,
                        text_w,
                    );
                    let dg = (!self.description.is_empty()).then(|| {
                        paint::galley_wrap(
                            ui,
                            self.description,
                            font(Face::Sans, size::TEXT_XS),
                            t.fg_muted,
                            text_w,
                        )
                    });
                    // The tick, the title's first line and the close button share one
                    // centre line, `cy` below the header's top.
                    let cy = 8.0 + size::CONTROL_SM / 2.0;
                    let line_h = tg.rows.first().map_or(tg.size().y, |r| r.rect().height());
                    let title_top = cy - line_h / 2.0;
                    let text_h =
                        title_top + tg.size().y + dg.as_ref().map_or(0.0, |g| g.size().y + 2.0);
                    let hh = text_h.max(8.0 + size::CONTROL_SM) + 8.0;
                    let (hr, _) = ui.allocate_exact_size(Vec2::new(w, hh), Sense::hover());
                    let p = ui.painter();
                    let tick = Rect::from_center_size(
                        egui::pos2(hr.left() + 12.0 + 1.0, hr.top() + cy),
                        Vec2::new(2.0, 12.0),
                    );
                    p.rect_filled(tick, CornerRadius::same(1), t.accent);
                    let tx = tick.right() + 8.0;
                    p.galley(egui::pos2(tx, hr.top() + title_top), tg.clone(), t.fg);
                    if let Some(dg) = dg {
                        p.galley(
                            egui::pos2(tx, hr.top() + title_top + tg.size().y + 2.0),
                            dg,
                            t.fg_muted,
                        );
                    }
                    p.hline(hr.x_range(), hr.bottom() - 0.5, Stroke::new(1.0, t.border));
                    let cr = Rect::from_center_size(
                        egui::pos2(hr.right() - 12.0 - size::CONTROL_SM / 2.0, hr.top() + cy),
                        Vec2::splat(size::CONTROL_SM),
                    );
                    let cb = ui.interact(cr, self.id.with("close"), Sense::click());
                    // A controller's press never lands on it.
                    crate::pad::registry::cross(ui.ctx(), cb.id);
                    let label = self.close_label;
                    cb.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
                    let hov = paint::ease(ui, cb.id, cb.hovered());
                    let p = ui.painter();
                    p.rect_filled(
                        cr,
                        CornerRadius::same(size::RADIUS_SM),
                        paint::lerp(egui::Color32::TRANSPARENT, t.raised, hov),
                    );
                    lucide::X.paint(
                        p,
                        Rect::from_center_size(cr.center(), Vec2::splat(size::ICON)),
                        paint::lerp(t.fg_faint, t.fg, hov),
                    );
                    if cb.clicked() {
                        close = true;
                    }
                    // Body.
                    let mut still = true;
                    let foot_room = if self.footer { 60.0 } else { 20.0 };
                    if self.body {
                        egui::Frame::new()
                            .inner_margin(Margin::symmetric(12, 10))
                            .show(ui, |ui| {
                                ui.set_width(w - 24.0);
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                egui::ScrollArea::vertical()
                                    .id_salt(self.id.with("body"))
                                    .max_height((max_h - hh - foot_room).max(40.0))
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.style_mut().override_font_id =
                                            Some(font(Face::Sans, size::TEXT_XS));
                                        body(ui, &mut still);
                                    });
                            });
                    }
                    // Footer: laid out at the end, its width remembered from the last frame.
                    if !self.footer {
                        if !still {
                            close = true;
                        }
                        return;
                    }
                    let fh = size::control() + 16.0;
                    let (fr, _) = ui.allocate_exact_size(Vec2::new(w, fh), Sense::hover());
                    ui.painter()
                        .hline(fr.x_range(), fr.top() + 0.5, Stroke::new(1.0, t.border));
                    let fw: f32 = ui.data(|d| d.get_temp(footer_w_id)).unwrap_or(0.0);
                    let inner = Rect::from_min_max(
                        egui::pos2(
                            (fr.right() - 12.0 - fw).max(fr.left() + 12.0),
                            fr.top() + 8.0,
                        ),
                        egui::pos2(fr.right() - 12.0, fr.bottom() - 8.0),
                    );
                    let mut fui = ui.new_child(
                        UiBuilder::new()
                            .max_rect(inner)
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    fui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);
                    // Its buttons show the pad keys their places answer to (B, X).
                    let count_id = self.id.with("footer-n");
                    let last_n: usize = ui.data(|d| d.get_temp(count_id)).unwrap_or(0);
                    super::button::footer_begin(last_n);
                    footer(&mut fui, &mut still);
                    let n = super::button::footer_end();
                    if n != last_n {
                        ui.data_mut(|d| d.insert_temp(count_id, n));
                        ui.ctx().request_repaint();
                    }
                    let used = fui.min_rect().width();
                    if (used - fw).abs() > 0.5 {
                        ui.data_mut(|d| d.insert_temp(footer_w_id, used));
                        ui.ctx().request_repaint();
                    }
                    if !still {
                        close = true;
                    }
                });
        });
        if close || (*open && r.should_close()) {
            *open = false;
        }
        if a < 1.0 {
            ctx.request_repaint();
        }
        was && !*open
    }
}

/// "Are you sure", for something that cannot be taken back. The button that acts says
/// what it does; the one that does nothing is first.
/// `Confirm::new("del-route", "Delete the route?", "Delete route", "Cancel").show(ctx, &mut open)`
/// returns `true` on the frame the acting button was pressed.
#[must_use = "call show(…)"]
pub struct Confirm<'a> {
    id: Id,
    title: &'a str,
    description: &'a str,
    confirm: &'a str,
    cancel: &'a str,
    danger: bool,
}

impl<'a> Confirm<'a> {
    pub fn new(
        id_salt: impl egui::AsIdSalt,
        title: &'a str,
        confirm: &'a str,
        cancel: &'a str,
    ) -> Self {
        Self {
            id: Id::unique(id_salt),
            title,
            description: "",
            confirm,
            cancel,
            danger: true,
        }
    }

    pub fn description(mut self, d: &'a str) -> Self {
        self.description = d;
        self
    }

    /// The acting button in accent rather than red.
    pub fn safe(mut self) -> Self {
        self.danger = false;
        self
    }

    pub fn show(self, ctx: &Context, open: &mut bool) -> bool {
        let mut confirmed = false;
        let (confirm, cancel, danger) = (self.confirm, self.cancel, self.danger);
        Dialog::new(self.id, self.title)
            .description(self.description)
            .close_label(cancel)
            .without_body()
            .show(
                ctx,
                open,
                |_, _| {},
                |ui, open| {
                    if ui
                        .add(Button::new(cancel).variant(Variant::Ghost))
                        .clicked()
                    {
                        *open = false;
                    }
                    let v = if danger {
                        Variant::Danger
                    } else {
                        Variant::Accent
                    };
                    if ui.add(Button::new(confirm).variant(v)).clicked() {
                        confirmed = true;
                        *open = false;
                    }
                },
            );
        confirmed
    }
}
