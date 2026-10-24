//! `ui/dropdown-menu`, `ui/context-menu` and `ui/popover`: lists and panels that float
//! beside what opened them.
//!
//! One plate for all (`bg-overlay border rounded-md shadow-pop p-1`), `min-w-44` for a
//! dropdown and `min-w-52` for a context menu. Items `size::MENU_ROW` high (`rounded-xs
//! px-1.5 gap-2 text-xs text-fg-muted`), lit `bg-raised text-fg`; a danger item is red; the shortcut
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

/// The height a popup beside `trigger` can take without leaving the window: the larger
/// room above or below it (egui flips the popup to the side it fits), less the gap, the
/// plate's padding and an 8-point margin from the edge. Beside it, the window's height.
fn room(trigger: &Response, side: Side, gap: f32, pad: f32) -> f32 {
    let screen = bounds(&trigger.ctx);
    let r = trigger.rect;
    let room = match side {
        Side::Bottom | Side::Top => {
            (screen.bottom() - r.bottom()).max(r.top() - screen.top()) - gap
        }
        Side::Left | Side::Right => screen.height(),
    };
    (room - 2.0 * pad - 2.0 - 16.0).max(size::ROW * 2.0)
}

/// Where a popup may lie: the window less the room kept at its bottom (in pad mode the
/// status bar and the pad hints' bar, [`super::dialog::set_bottom_inset`]).
pub fn bounds(ctx: &egui::Context) -> Rect {
    let mut r = ctx.content_rect();
    r.max.y = (r.max.y - super::dialog::bottom_inset(ctx)).max(r.min.y);
    r
}

/// Keeps a popup above or below its anchor out of the bars [`bounds`] leaves out: while
/// the window's bottom is kept (pad mode), the side is chosen here (the asked one when the
/// popup fits there, else the one it fits, else the roomier) and egui does not flip it
/// back over the bars, whose height it counts as room. Its height must be bounded by the
/// room of the roomier side. Beside its anchor, or at the pointer, the popup as it was.
pub fn clear_of_bars<'a>(
    popup: Popup<'a>,
    ctx: &egui::Context,
    align: RectAlign,
    gap: f32,
) -> Popup<'a> {
    if super::dialog::bottom_inset(ctx) <= 0.0 {
        return popup.align(align);
    }
    let Some(a) = popup.get_anchor_rect() else {
        return popup.align(align);
    };
    let b = bounds(ctx);
    let below = b.bottom() - a.bottom() - gap;
    let above = a.top() - b.top() - gap;
    let (py, cy) = (align.parent().y(), align.child().y());
    let (pref, other) = if py == egui::Align::Max && cy == egui::Align::Min {
        (below, above)
    } else if py == egui::Align::Min && cy == egui::Align::Max {
        (above, below)
    } else {
        return popup.align(align);
    };
    let need = popup.get_expected_size().map_or(0.0, |s| s.y);
    let flip = need > pref && other > pref;
    let align = if flip { align.flip_y() } else { align };
    popup.align(align).align_alternatives(&[])
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
    let max_h = room(trigger, side, 4.0, 4.0);
    let popup = Popup::menu(trigger)
        .frame(paint::float_frame(&t, 4))
        .gap(4.0)
        .close_behavior(PopupCloseBehavior::CloseOnClick);
    let popup = clear_of_bars(popup, &trigger.ctx, rect_align(side, align), 4.0);
    sized(popup, trigger, max_h + 16.0, |popup| {
        popup.show(|ui| {
            dress(ui, &t, 176.0);
            bounded(ui, max_h, 176.0, add)
        })
    })
}

/// The menu a right-click on `target` opens, at the pointer (`min-w-52`).
pub fn context<R>(target: &Response, add: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>> {
    let t = target.ctx.tokens();
    // The pad's Menu opens it too (`crate::pad`): then at the widget, not the pointer,
    // below or above it as it fits (scrolling when neither side has the room).
    crate::pad::registry::menu(&target.ctx, target.id, target.interact_rect);
    let mut popup = Popup::context_menu(target);
    let at = crate::pad::registry::menu_anchor(&target.ctx, target.id, target.interact_rect);
    let side = if at.is_some() {
        Side::Bottom
    } else {
        Side::Right
    };
    let max_h = room(target, side, 0.0, 4.0);
    if let Some(at) = at {
        popup = popup.anchor(at);
    }
    let mut popup = popup
        .frame(paint::float_frame(&t, 4))
        .close_behavior(PopupCloseBehavior::CloseOnClick);
    if at.is_some() {
        popup = clear_of_bars(popup, &target.ctx, RectAlign::BOTTOM_START, 0.0);
    }
    sized(popup, target, max_h + 16.0, |popup| {
        popup.show(|ui| {
            dress(ui, &t, 208.0);
            bounded(ui, max_h, 208.0, add)
        })
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
    let max_h = room(trigger, side, gap, 0.0);
    let popup = Popup::from_toggle_button_response(trigger)
        .frame(paint::float_frame(&t, 0))
        .gap(gap)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside);
    let popup = clear_of_bars(popup, &trigger.ctx, rect_align(side, align), gap);
    sized(popup, trigger, max_h + 16.0, |popup| {
        popup.show(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("kit-popover-scroll")
                .max_height(max_h)
                .auto_shrink([true, true])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    add(ui)
                })
                .inner
        })
    })
}

/// Shows a popup with room for `h` points on its first frame. egui lays a popup out
/// once inside `default_area_size` (400 points tall) and keeps that height after, so
/// without this a menu stops at 400 however much room the window has.
fn sized<R>(popup: Popup<'_>, trigger: &Response, h: f32, show: impl FnOnce(Popup<'_>) -> R) -> R {
    let ctx = trigger.ctx.clone();
    let was = ctx.global_style().spacing.default_area_size;
    // Only while open or opening: the style is not touched for every closed trigger.
    let opening = popup.is_open() || trigger.clicked() || trigger.secondary_clicked();
    if !opening || was.y >= h {
        return show(popup);
    }
    ctx.global_style_mut(|s| s.spacing.default_area_size.y = h);
    let r = show(popup);
    ctx.global_style_mut(|s| s.spacing.default_area_size = was);
    r
}

/// A menu's items in a scroll area no taller than `max_h`: a long list (the themes, the
/// languages) scrolls instead of running off a short window.
fn bounded<R>(ui: &mut Ui, max_h: f32, min_w: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::ScrollArea::vertical()
        .id_salt("kit-menu-scroll")
        .max_height(max_h)
        .auto_shrink([true, true])
        .show(ui, |ui| {
            ui.set_min_width(min_w);
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            add(ui)
        })
        .inner
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
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::menu_row()), sense);
        let label = self.label;
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
        // A click without the pointer (the pad's A, AccessKit) closes the menu too: the
        // popup only sees the pointer's.
        if resp.clicked() {
            ui.close();
        }
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
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::menu_row()), Sense::click());
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
        let max_h = (ui.ctx().content_rect().height() - 26.0).max(size::ROW * 2.0);
        bounded(ui, max_h, 176.0, add)
    })
}
