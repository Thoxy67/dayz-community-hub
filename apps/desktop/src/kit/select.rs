//! `ui/select`: one value out of a list, in the theme's clothes.
//!
//! Trigger: `h-control rounded-sm border bg-bg px-1.5 text-xs`, the chevron at the end,
//! accent-edged while open. List: the floating plate (`bg-overlay rounded-md shadow-pop
//! p-1`), at least the trigger's width and 8rem, at most 20rem tall and never past the
//! window's edge; items `size::MENU_ROW` high (`rounded-xs px-1.5 text-xs
//! text-fg-muted`, lit on hover), the chosen one accent with a tick. Options with a
//! `group` sit under a mono, tracked heading, groups in first-seen order. Arrows
//! move, Enter picks, Escape closes; typing a letter jumps to a match.

use egui::{
    Color32, CornerRadius, Id, Popup, PopupCloseBehavior, Rect, RectAlign, Response, Role, Sense,
    Ui, Vec2, Widget, WidgetInfo,
};

use super::paint::{self, alpha, fade};
use crate::icons::lucide;
use crate::theme::{TokensExt, font, fonts::Face, size};

/// One choice.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectOption<'a, V> {
    pub value: V,
    pub label: &'a str,
    /// The heading it sits under; empty for none.
    pub group: &'a str,
}

impl<'a, V> SelectOption<'a, V> {
    pub fn new(value: V, label: &'a str) -> Self {
        Self {
            value,
            label,
            group: "",
        }
    }

    pub fn group(mut self, g: &'a str) -> Self {
        self.group = g;
        self
    }
}

/// `Select::new("map", &mut map, &options).placeholder("All maps").width(160.0)`.
/// `changed()` on the response when a choice was made.
#[must_use = "add it with ui.add(…)"]
pub struct Select<'a, V: PartialEq + Clone> {
    id: Id,
    value: &'a mut V,
    options: &'a [SelectOption<'a, V>],
    placeholder: &'a str,
    none: Option<V>,
    width: Option<f32>,
    enabled: bool,
    label: &'a str,
}

impl<'a, V: PartialEq + Clone> Select<'a, V> {
    pub fn new(
        id_salt: impl egui::AsIdSalt,
        value: &'a mut V,
        options: &'a [SelectOption<'a, V>],
    ) -> Self {
        Self {
            id: Id::unique(id_salt),
            value,
            options,
            placeholder: "",
            none: None,
            width: None,
            enabled: true,
            label: "",
        }
    }

    /// Shown (faint) when the value is none of the options.
    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    /// A first item, worded as the placeholder, that sets the value to `none`.
    pub fn none(mut self, none: V) -> Self {
        self.none = Some(none);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    /// The accessible name (`aria-label`).
    pub fn label(mut self, l: &'a str) -> Self {
        self.label = l;
        self
    }
}

const PAD: f32 = 6.0;

impl<V: PartialEq + Clone> Widget for Select<'_, V> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let f = font(Face::Sans, size::TEXT_XS);
        let chosen = self.options.iter().find(|o| o.value == *self.value);
        let shown = chosen.map_or(self.placeholder, |o| o.label);
        let w = self.width.unwrap_or_else(|| {
            let widest = self
                .options
                .iter()
                .map(|o| o.label)
                .chain(std::iter::once(self.placeholder))
                .map(|l| paint::galley(ui, l, f.clone(), t.fg).size().x)
                .fold(0.0_f32, f32::max);
            widest + 2.0 * PAD + 4.0 + size::ICON_SM + 2.0
        });
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, size::control()), sense);
        let popup_id = self.id.with("popup");
        let open = Popup::is_id_open(ui.ctx(), popup_id);
        let label = if self.label.is_empty() {
            shown
        } else {
            self.label
        };
        resp.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, label));
        trigger(ui, rect, &resp, shown, chosen.is_some(), open, self.enabled);
        if !self.enabled {
            return resp;
        }
        let resp_c = resp.clone();
        let mut picked: Option<V> = None;
        let toggle = resp.clicked()
            || (resp.has_focus()
                && ui
                    .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)));
        let popup = Popup::from_response(&resp_c)
            .id(popup_id)
            .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
            .kind(egui::PopupKind::Popup)
            .layout(egui::Layout::top_down_justified(egui::Align::Min))
            .close_behavior(PopupCloseBehavior::CloseOnClick)
            .frame(paint::float_frame(&t, 4))
            .gap(4.0)
            .width(w.max(128.0) - 8.0);
        let popup = super::menu::clear_of_bars(popup, ui.ctx(), RectAlign::BOTTOM_START, 4.0);
        // The list flips above the trigger when there is more room there; either way it
        // stops 8 points short of the window's edge.
        let screen = super::menu::bounds(ui.ctx());
        let room = (screen.bottom() - rect.bottom()).max(rect.top() - screen.top()) - 4.0 - 26.0;
        let max_h = (320.0 - 10.0_f32).min(room).max(size::menu_row() * 3.0);
        popup.show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_min_width(w.max(128.0) - 10.0);
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .show(ui, |ui| {
                    // Keyboard: a highlighted row, moved by the arrows, picked by Enter.
                    let hl_id = self.id.with("hl");
                    let count = self.options.len() + usize::from(self.none.is_some());
                    let mut hl: Option<usize> = ui.data(|d| d.get_temp(hl_id));
                    let (down, up, enter) = ui.input(|i| {
                        (
                            i.key_pressed(egui::Key::ArrowDown),
                            i.key_pressed(egui::Key::ArrowUp),
                            i.key_pressed(egui::Key::Enter),
                        )
                    });
                    if count > 0 {
                        if down {
                            hl = Some(hl.map_or(0, |h| (h + 1).min(count - 1)));
                        }
                        if up {
                            hl = Some(hl.map_or(0, |h| h.saturating_sub(1)));
                        }
                    }
                    let typed = ui.input(|i| {
                        i.events.iter().find_map(|e| match e {
                            egui::Event::Text(s) => s.chars().next(),
                            _ => None,
                        })
                    });
                    if let Some(c) = typed {
                        let c = c.to_lowercase().next().unwrap_or(c);
                        let base = usize::from(self.none.is_some());
                        if let Some(i) = self.options.iter().position(|o| {
                            o.label.chars().next().and_then(|f| f.to_lowercase().next()) == Some(c)
                        }) {
                            hl = Some(i + base);
                        }
                    }
                    let mut index = 0usize;
                    if let Some(none) = &self.none {
                        let r = item(ui, self.placeholder, false, true, hl == Some(index));
                        if r.clicked() || (enter && hl == Some(index)) {
                            picked = Some(none.clone());
                        }
                        if r.hovered() {
                            hl = Some(index);
                        }
                        index += 1;
                    }
                    let mut last_group = "";
                    let mut seen: Vec<&str> = Vec::new();
                    // Grouped in first-seen order; ungrouped options flat, where they fall.
                    let mut order: Vec<usize> = Vec::with_capacity(self.options.len());
                    for (i, o) in self.options.iter().enumerate() {
                        if o.group.is_empty() {
                            order.push(i);
                        } else if !seen.contains(&o.group) {
                            seen.push(o.group);
                            order.extend(
                                self.options
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, p)| p.group == o.group)
                                    .map(|(j, _)| j),
                            );
                        }
                    }
                    for i in order {
                        let Some(o) = self.options.get(i) else {
                            continue;
                        };
                        if !o.group.is_empty() && o.group != last_group {
                            heading(ui, o.group, ui.available_width());
                        }
                        last_group = o.group;
                        let sel = o.value == *self.value;
                        let r = item(ui, o.label, sel, false, hl == Some(index));
                        if r.clicked() || (enter && hl == Some(index)) {
                            picked = Some(o.value.clone());
                        }
                        if r.hovered() {
                            hl = Some(index);
                        }
                        if (down || up || typed.is_some()) && hl == Some(index) {
                            r.scroll_to_me(None);
                        }
                        index += 1;
                    }
                    ui.data_mut(|d| d.insert_temp(hl_id, hl));
                });
        });
        if let Some(v) = picked {
            if v != *self.value {
                *self.value = v;
                resp.mark_changed();
            }
            Popup::close_id(ui.ctx(), popup_id);
            ui.data_mut(|d| d.remove::<Option<usize>>(self.id.with("hl")));
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

/// The closed control: the value (faint while it is the placeholder) and the chevron,
/// accent-edged while open or focused from the keys.
pub(super) fn trigger(
    ui: &Ui,
    rect: Rect,
    resp: &Response,
    shown: &str,
    set: bool,
    open: bool,
    en: bool,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let t = ui.tokens();
    let hover = paint::ease(ui, resp.id.with("hover"), en && resp.hovered());
    let border = if open || paint::keyboard_focus(ui, resp) {
        alpha(t.accent, 0.6)
    } else {
        paint::lerp(t.border, t.border_strong, hover)
    };
    let p = ui.painter();
    paint::plate(p, rect, size::RADIUS_SM, fade(t.bg, en), fade(border, en));
    if !open && paint::keyboard_focus(ui, resp) {
        paint::focus_ring(p, rect, size::RADIUS_SM, &t);
    }
    let room = rect.width() - 2.0 * PAD - 4.0 - size::ICON_SM;
    let ink = if set { t.fg } else { t.fg_faint };
    let g = paint::galley_trunc(ui, shown, font(Face::Sans, size::TEXT_XS), ink, room);
    paint::text_at(p, &g, rect.left() + PAD, rect.center().y, fade(ink, en));
    let ir = Rect::from_center_size(
        egui::pos2(rect.right() - PAD - size::ICON_SM / 2.0, rect.center().y),
        Vec2::splat(size::ICON_SM),
    );
    lucide::CHEVRON_DOWN.paint(p, ir, fade(t.fg_faint, en));
}

/// A row of the list.
fn item(ui: &mut Ui, label: &str, selected: bool, faint: bool, highlighted: bool) -> Response {
    let t = ui.tokens();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::menu_row()), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(Role::Button, true, selected, label));
    let lit = highlighted || resp.hovered();
    let p = ui.painter();
    p.rect_filled(
        rect,
        CornerRadius::same(size::RADIUS_XS),
        if lit { t.raised } else { Color32::TRANSPARENT },
    );
    let ink = if selected {
        t.accent
    } else if lit {
        t.fg
    } else if faint {
        t.fg_faint
    } else {
        t.fg_muted
    };
    let room = rect.width() - 12.0 - if selected { size::ICON_SM + 6.0 } else { 0.0 };
    let g = paint::galley_trunc(ui, label, font(Face::Sans, size::TEXT_XS), ink, room);
    paint::text_at(p, &g, rect.left() + 6.0, rect.center().y, ink);
    if selected {
        let ir = Rect::from_center_size(
            egui::pos2(rect.right() - 6.0 - size::ICON_SM / 2.0, rect.center().y),
            Vec2::splat(size::ICON_SM),
        );
        lucide::CHECK.paint(p, ir, ink);
    }
    resp
}

/// A group's heading: `px-1.5 pt-1.5 pb-0.5 font-mono text-2xs tracking-[0.08em] uppercase`.
pub(crate) fn heading(ui: &mut Ui, text: &str, width: f32) {
    let t = ui.tokens();
    let g = paint::tracked(
        ui,
        text,
        font(Face::Mono, size::TEXT_2XS),
        t.fg_faint,
        0.08,
        true,
        Some(width - 12.0),
    );
    let h = g.size().y + 8.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    ui.painter().galley(
        egui::pos2(rect.left() + 6.0, rect.top() + 6.0),
        g,
        t.fg_faint,
    );
}
