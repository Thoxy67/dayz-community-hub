//! `ui/dropdown-menu`, `ui/context-menu` and `ui/popover`: lists and panels that float
//! beside what opened them.
//!
//! One plate for all (`bg-overlay border rounded-md shadow-pop p-1`), `min-w-44` for a
//! dropdown and `min-w-52` for a context menu. Items `h-row rounded-xs px-1.5 gap-2
//! text-xs text-fg-muted`, lit `bg-raised text-fg`; a danger item is red; the shortcut
//! sits at the end. Labels are mono, tracked capitals; separators a hairline.
//! A click anywhere closes a menu, Escape too.

use egui::{
    Color32, CornerRadius, InnerResponse, Popup, PopupCloseBehavior, Rect, RectAlign, Response,
    Sense, Ui, Vec2, Widget, WidgetInfo, WidgetType,
};

use super::kbd::Kbd;
use super::paint::{self, fade};
use super::tooltip::Side;
use crate::icons::{Icon, lucide};
use crate::theme::{Tokens, TokensExt, font, fonts::Face, size};

/// Where a dropdown lines up with its trigger.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    #[default]
    End,
}

fn rect_align(side: Side, align: Align) -> RectAlign {
    match (side, align) {
        (Side::Bottom, Align::Start) => RectAlign::BOTTOM_START,
        (Side::Bottom, Align::Center) => RectAlign::BOTTOM,
        (Side::Bottom, Align::End) => RectAlign::BOTTOM_END,
        (Side::Top, Align::Start) => RectAlign::TOP_START,
        (Side::Top, Align::Center) => RectAlign::TOP,
        (Side::Top, Align::End) => RectAlign::TOP_END,
        (Side::Right, Align::Start) => RectAlign::RIGHT_START,
        (Side::Right, Align::Center) => RectAlign::RIGHT,
        (Side::Right, Align::End) => RectAlign::RIGHT_END,
        (Side::Left, Align::Start) => RectAlign::LEFT_START,
        (Side::Left, Align::Center) => RectAlign::LEFT,
        (Side::Left, Align::End) => RectAlign::LEFT_END,
    }
}

/// Dresses a menu's `Ui`: no spacing between items, the overlay plate for sub-menus.
fn dress(ui: &mut Ui, t: &Tokens, min_w: f32) {
    ui.set_min_width(min_w);
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    let v = &mut ui.style_mut().visuals;
    v.window_fill = t.overlay;
    v.window_stroke = egui::Stroke::new(1.0, t.border);
    v.menu_corner_radius = CornerRadius::same(size::RADIUS_MD);
    v.popup_shadow = paint::shadow_pop();
    ui.spacing_mut().menu_margin = egui::Margin::same(4);
}

/// A dropdown menu opened by clicking `trigger`, below it and lined up with its end
/// (`side="bottom" align="end"`). `menu::dropdown(&resp, Side::Bottom, Align::End, |ui| {
/// ui.add(MenuItem::new("Copy").icon(lucide::COPY)); })`.
pub fn dropdown<R>(
    trigger: &Response,
    side: Side,
    align: Align,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    let t = trigger.ctx.tokens();
    Popup::menu(trigger)
        .frame(paint::float_frame(&t, 4))
        .align(rect_align(side, align))
        .gap(4.0)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            dress(ui, &t, 176.0);
            add(ui)
        })
}

/// The menu a right-click on `target` opens, at the pointer (`min-w-52`).
pub fn context<R>(target: &Response, add: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>> {
    let t = target.ctx.tokens();
    Popup::context_menu(target)
        .frame(paint::float_frame(&t, 4))
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            dress(ui, &t, 208.0);
            add(ui)
        })
}

/// A small panel beside its trigger that stays open while used inside
/// (`ui/popover`): closes on a click outside or Escape. `p-0`: the content pads itself.
pub fn popover<R>(
    trigger: &Response,
    side: Side,
    align: Align,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    let t = trigger.ctx.tokens();
    let gap = if side == Side::Right { 8.0 } else { 4.0 };
    Popup::from_toggle_button_response(trigger)
        .frame(paint::float_frame(&t, 0))
        .align(rect_align(side, align))
        .gap(gap)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            add(ui)
        })
}

/// Whether the popup (dropdown or popover) `trigger` opens is open now, for a trigger
/// that shows it (`data-[state=open]:bg-raised`).
pub fn is_open(trigger: &Response) -> bool {
    Popup::is_id_open(&trigger.ctx, Popup::default_response_id(trigger))
}

/// A popover's trigger (`grid place-items-center rounded-md text-fg-faint`, lit while
/// open): an icon in a `size` square. Returns its response for [`popover`].
pub fn popover_trigger(ui: &mut Ui, icon: Icon, label: &str, side: f32) -> Response {
    let t = ui.tokens();
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let open = is_open(&resp);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let hover = paint::ease(ui, resp.id.with("hover"), resp.hovered() || open);
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::same(size::RADIUS_MD),
        paint::lerp(Color32::TRANSPARENT, t.raised, hover),
    );
    let px = if side >= size::CONTROL {
        size::ICON
    } else {
        size::ICON_SM
    };
    icon.paint(
        p,
        Rect::from_center_size(rect.center(), Vec2::splat(px)),
        paint::lerp(t.fg_faint, t.fg, hover),
    );
    super::TipExt::tip_kbd(resp, label, "", Side::Bottom)
}

/// The width a menu's rows share: the widest row's natural width (remembered from the
/// last frame), never under the menu's minimum.
fn menu_width(ui: &Ui, natural: f32) -> f32 {
    let id = ui.id().with("kit-menu-w");
    let last: f32 = ui.data(|d| d.get_temp(id)).unwrap_or(0.0);
    let w = natural.max(last).max(ui.min_rect().width()).min(480.0);
    if w > last + 0.5 {
        ui.data_mut(|d| d.insert_temp(id, w));
        ui.ctx().request_repaint();
    }
    w
}

fn natural(ui: &Ui, icon: bool, label: &str, kbd: &str, check: bool, sub: bool) -> f32 {
    let f = font(Face::Sans, size::TEXT_XS);
    let mut w = 12.0 + paint::galley(ui, label, f, Color32::WHITE).size().x;
    if icon {
        w += size::ICON + 8.0;
    }
    if !kbd.is_empty() {
        w += Kbd::width(ui, kbd) + 8.0;
    }
    if check {
        w += size::ICON_SM + 8.0;
    }
    if sub {
        w += size::ICON_SM + 8.0;
    }
    w + 8.0
}

/// One item of a menu: `MenuItem::new("Delete").icon(lucide::TRASH_2).danger().kbd("Del")`.
#[must_use = "add it with ui.add(…)"]
pub struct MenuItem<'a> {
    label: &'a str,
    icon: Option<Icon>,
    kbd: &'a str,
    danger: bool,
    enabled: bool,
    checked: Option<bool>,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            kbd: "",
            danger: false,
            enabled: true,
            checked: None,
        }
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn kbd(mut self, kbd: &'a str) -> Self {
        self.kbd = kbd;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// A tick at the end when `on` (a theme or language picked from a menu).
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }
}

impl Widget for MenuItem<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let nat = natural(
            ui,
            self.icon.is_some(),
            self.label,
            self.kbd,
            self.checked.is_some(),
            false,
        );
        let w = menu_width(ui, nat);
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::ROW), sense);
        let label = self.label;
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
        if ui.is_rect_visible(rect) {
            let lit = self.enabled && (resp.hovered() || resp.has_focus());
            row(
                ui,
                rect,
                &t,
                lit,
                self.enabled,
                self.danger,
                self.icon,
                self.label,
                self.kbd,
                self.checked,
                false,
            );
        }
        resp
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &Ui,
    rect: Rect,
    t: &Tokens,
    lit: bool,
    en: bool,
    danger: bool,
    icon: Option<Icon>,
    label: &str,
    kbd: &str,
    checked: Option<bool>,
    sub: bool,
) {
    let p = ui.painter();
    if lit {
        p.rect_filled(rect, CornerRadius::same(size::RADIUS_XS), t.raised);
    }
    let ink = fade(
        if danger {
            t.err
        } else if lit {
            t.fg
        } else {
            t.fg_muted
        },
        en,
    );
    let cy = rect.center().y;
    let mut x = rect.left() + 6.0;
    if let Some(icon) = icon {
        icon.paint(
            p,
            Rect::from_min_size(
                egui::pos2(x, cy - size::ICON / 2.0),
                Vec2::splat(size::ICON),
            ),
            ink,
        );
        x += size::ICON + 8.0;
    }
    let mut right = rect.right() - 6.0;
    if sub {
        right -= size::ICON_SM;
        lucide::CHEVRON_RIGHT.paint(
            p,
            Rect::from_min_size(egui::pos2(right, cy - 7.0), Vec2::splat(size::ICON_SM)),
            ink,
        );
        right -= 8.0;
    }
    if checked == Some(true) {
        right -= size::ICON_SM;
        lucide::CHECK.paint(
            p,
            Rect::from_min_size(egui::pos2(right, cy - 7.0), Vec2::splat(size::ICON_SM)),
            fade(t.accent, en),
        );
        right -= 8.0;
    }
    if !kbd.is_empty() {
        right -= Kbd::width(ui, kbd);
        Kbd::paint_at(ui, kbd, right, cy);
        right -= 8.0;
    }
    let g = paint::galley_trunc(
        ui,
        label,
        font(Face::Sans, size::TEXT_XS),
        ink,
        (right - x).max(0.0),
    );
    paint::text_at(p, &g, x, cy, ink);
}

/// A menu's group heading: `px-1.5 pt-1.5 pb-0.5 font-mono text-2xs tracking-[0.08em]
/// uppercase text-fg-faint`.
pub fn label(ui: &mut Ui, text: &str) {
    let w = menu_width(ui, 0.0);
    super::select::heading(ui, text, w);
}

/// A hairline between groups: `my-1 h-px bg-border`, to the plate's edges.
pub fn separator(ui: &mut Ui) {
    let t = ui.tokens();
    let w = menu_width(ui, 0.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 9.0), Sense::hover());
    ui.painter().hline(
        (rect.left() - 4.0)..=(rect.right() + 4.0),
        rect.center().y,
        egui::Stroke::new(1.0, t.border),
    );
}

/// An item that opens a sub-menu beside it (`side="left"` in the Svelte title-bar menus;
/// egui places it where there is room).
pub fn submenu<R>(
    ui: &mut Ui,
    label: &str,
    icon: Option<Icon>,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>> {
    let t = ui.tokens();
    let nat = natural(ui, icon.is_some(), label, "", false, true);
    let w = menu_width(ui, nat);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::ROW), Sense::click());
    let open = Popup::is_id_open(
        ui.ctx(),
        egui::containers::menu::SubMenu::id_from_widget_id(resp.id),
    );
    row(
        ui,
        rect,
        &t,
        resp.hovered() || open,
        true,
        false,
        icon,
        label,
        "",
        None,
        true,
    );
    egui::containers::menu::SubMenu::new().show(ui, &resp, |ui| {
        dress(ui, &t, 176.0);
        add(ui)
    })
}
