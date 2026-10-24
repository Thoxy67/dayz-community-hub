//! `ui/tabs` (TabStrip): the strip of tabs above a set of panes, the underline kind.
//!
//! `border-b`, tabs `px-2.5 gap-1.5 text-xs text-fg-muted`, hover `bg-raised/60 text-fg`,
//! the one that is on `text-accent` with the accent mark under it: one 2px bar that
//! slides (`--duration-base`, ease-out) to the tab picked, inset 6px each side. A count
//! after a label sits in a small mono well. The strip scrolls sideways without a bar when
//! the tabs do not fit. Arrows, Home and End move the tab from the keyboard.

use egui::{
    Color32, CornerRadius, Id, Rect, Response, Sense, Ui, Vec2, Widget, WidgetInfo, WidgetType,
};

use super::input::FieldSize;
use super::paint::{self, alpha};
use super::tooltip::TipExt;
use crate::icons::Icon;
use crate::theme::{TokensExt, font, fonts::Face, motion, size};

/// One tab.
#[derive(Clone, Debug)]
pub struct Tab<'a, V> {
    pub id: V,
    pub label: &'a str,
    pub icon: Option<Icon>,
    /// A figure after the label: how many are in there.
    pub count: Option<u64>,
    pub title: &'a str,
    /// The icon's own colour (a warning on the tab), whatever the tab's state.
    pub tint: Option<Color32>,
    /// A small dot at the icon's top right, in this colour (`TabIcon.svelte`).
    pub dot: Option<Color32>,
}

impl<'a, V> Tab<'a, V> {
    pub fn new(id: V, label: &'a str) -> Self {
        Self {
            id,
            label,
            icon: None,
            count: None,
            title: "",
            tint: None,
            dot: None,
        }
    }

    /// The icon in its own colour, with an optional dot (missing or stale mods).
    pub fn tinted(mut self, tint: Color32, dot: Option<Color32>) -> Self {
        self.tint = Some(tint);
        self.dot = dot;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn count(mut self, n: u64) -> Self {
        self.count = Some(n);
        self
    }

    pub fn title(mut self, t: &'a str) -> Self {
        self.title = t;
        self
    }
}

/// The widget of a strip's tab `i` (the strip's id is `base`): the pad presses it.
pub fn tab_id(base: Id, i: usize) -> Id {
    base.with(("tab", i))
}

/// `TabStrip::new("mods-tabs", &mut tab, &tabs)`; `changed()` when the tab moved.
#[must_use = "add it with ui.add(…)"]
pub struct TabStrip<'a, V: PartialEq + Clone> {
    id: Id,
    value: &'a mut V,
    tabs: &'a [Tab<'a, V>],
    size: FieldSize,
    fill: bool,
    height: Option<f32>,
}

impl<'a, V: PartialEq + Clone> TabStrip<'a, V> {
    pub fn new(id_salt: impl egui::AsId, value: &'a mut V, tabs: &'a [Tab<'a, V>]) -> Self {
        Self {
            id: Id::new(id_salt),
            value,
            tabs,
            size: FieldSize::Sm,
            fill: false,
            height: None,
        }
    }

    pub fn size(mut self, s: FieldSize) -> Self {
        self.size = s;
        self
    }

    /// Share the width equally instead of sitting at the start.
    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    /// As tall as the header it is part of (`stretch`), so the mark lands on its edge.
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
}

struct Measured {
    w: f32,
    label: std::sync::Arc<egui::Galley>,
    count: Option<std::sync::Arc<egui::Galley>>,
}

impl<V: PartialEq + Clone> Widget for TabStrip<'_, V> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = ui.tokens();
        let (ctl, text) = match self.size {
            FieldSize::Sm => (size::control(), size::TEXT_XS),
            FieldSize::Xs => (size::control_sm(), size::TEXT_2XS),
        };
        let h = self.height.unwrap_or(ctl);
        let (pad, gap) = if self.fill { (4.0, 4.0) } else { (10.0, 6.0) };
        let f = font(Face::Sans, text);
        let mf = font(Face::Mono, size::TEXT_2XS);
        let measured: Vec<Measured> = self
            .tabs
            .iter()
            .map(|tab| {
                let label = paint::galley(ui, tab.label, f.clone(), t.fg);
                let count = tab
                    .count
                    .map(|n| paint::galley(ui, &n.to_string(), mf.clone(), t.fg));
                let mut w = 2.0 * pad + label.size().x;
                if tab.icon.is_some() {
                    w += size::ICON + gap;
                }
                if let Some(c) = &count {
                    w += gap + c.size().x + 8.0;
                }
                Measured { w, label, count }
            })
            .collect();
        let avail = ui.available_width();
        let natural: f32 = measured.iter().map(|m| m.w).sum();
        let n = self.tabs.len().max(1) as f32;
        let (outer, resp) = ui.allocate_exact_size(Vec2::new(avail, h), Sense::hover());
        let mut resp = resp;
        let p = ui.painter().clone();
        // The strip's own hairline, under everything.
        p.hline(
            outer.x_range(),
            outer.bottom() - 0.5,
            egui::Stroke::new(1.0, t.border),
        );

        let mut moved: Option<V> = None;
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(outer)
                .layout(egui::Layout::left_to_right(egui::Align::Min)),
        );
        let current = self.tabs.iter().position(|tab| tab.id == *self.value);
        let mut ink_target: Option<(f32, f32)> = None;
        let mut focused: Option<usize> = None;
        let scroll = egui::ScrollArea::horizontal()
            .id_salt(self.id.with("scroll"))
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .scroll([!self.fill && natural > avail, false]);
        scroll.show(&mut child, |ui| {
            let total = if self.fill { avail } else { natural };
            let (strip, _) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());
            let p = ui.painter();
            let mut x = strip.left();
            for (i, (tab, m)) in self.tabs.iter().zip(&measured).enumerate() {
                let w = if self.fill { avail / n } else { m.w };
                let r = Rect::from_min_size(egui::pos2(x, strip.top()), Vec2::new(w, h));
                x += w;
                let id = tab_id(self.id, i);
                let tr = ui.interact(r, id, Sense::click());
                let on = Some(i) == current;
                let label = tab.label;
                tr.widget_info(|| {
                    WidgetInfo::selected(WidgetType::SelectableLabel, true, on, label)
                });
                if tr.clicked() && !on {
                    moved = Some(tab.id.clone());
                }
                if tr.has_focus() {
                    focused = Some(i);
                }
                if on {
                    ink_target = Some((r.left() + 6.0, (r.width() - 12.0).max(0.0)));
                }
                let hover = paint::ease(ui, id.with("hover"), tr.hovered());
                let lit = paint::ease(ui, id.with("on"), on);
                p.rect_filled(
                    r,
                    CornerRadius::ZERO,
                    paint::lerp(Color32::TRANSPARENT, alpha(t.raised, 0.6), hover),
                );
                let ink = paint::lerp(paint::lerp(t.fg_muted, t.fg, hover), t.accent, lit);
                // Content centred in the tab, cut when the share is narrow.
                let mut content = m.w - 2.0 * pad;
                let room = r.width() - 2.0 * pad;
                let label_g = if content > room + 0.5 {
                    let cut = (m.label.size().x - (content - room)).max(0.0);
                    content = room;
                    paint::galley_trunc(ui, tab.label, f.clone(), t.fg, cut)
                } else {
                    m.label.clone()
                };
                let mut cx = r.center().x - content / 2.0;
                let cy = r.center().y;
                if let Some(icon) = tab.icon {
                    let ir = Rect::from_min_size(
                        egui::pos2(cx, cy - size::ICON / 2.0),
                        Vec2::splat(size::ICON),
                    );
                    icon.paint(p, ir, tab.tint.unwrap_or(ink));
                    if let Some(d) = tab.dot {
                        p.circle_filled(ir.right_top() + Vec2::new(-1.0, 1.0), 3.0, d);
                    }
                    cx += size::ICON + gap;
                }
                paint::text_at(p, &label_g, cx, cy, ink);
                cx += label_g.size().x + gap;
                if let Some(cg) = &m.count {
                    let well = Rect::from_min_size(
                        egui::pos2(cx, cy - 8.0),
                        Vec2::new(cg.size().x + 8.0, 16.0),
                    );
                    p.rect_filled(
                        well,
                        CornerRadius::same(size::RADIUS_XS),
                        paint::lerp(t.raised, alpha(t.accent, 0.15), lit),
                    );
                    paint::text_center(p, cg, well, paint::lerp(t.fg_faint, t.accent, lit));
                }
                if paint::keyboard_focus(ui, &tr) {
                    paint::focus_ring(p, r.shrink(2.0), size::RADIUS_XS, &t);
                }
                let _ = tr.on_hover_cursor(egui::CursorIcon::PointingHand).tip_kbd(
                    tab.title,
                    "",
                    super::Side::Bottom,
                );
            }
            if let Some((ix, iw)) = ink_target {
                // Placed without sliding the first time it is seen.
                let ctx = ui.ctx();
                let seen_id = self.id.with("ink-seen");
                let seen = ctx.data(|d| d.get_temp::<bool>(seen_id)).unwrap_or(false);
                let (x, w) = if seen {
                    (
                        ctx.animate_value_with_time(
                            self.id.with("ink-x"),
                            ix - strip.left(),
                            motion::BASE,
                        ),
                        ctx.animate_value_with_time(self.id.with("ink-w"), iw, motion::BASE),
                    )
                } else {
                    ctx.data_mut(|d| d.insert_temp(seen_id, true));
                    (
                        ctx.animate_value_with_time(self.id.with("ink-x"), ix - strip.left(), 0.0),
                        ctx.animate_value_with_time(self.id.with("ink-w"), iw, 0.0),
                    )
                };
                let bar = Rect::from_min_size(
                    egui::pos2(strip.left() + x, strip.bottom() - 2.0),
                    Vec2::new(w, 2.0),
                );
                p.rect_filled(bar, CornerRadius::same(1), t.accent);
            }
        });
        // LT and RT switch the tabs while the pad is in the pane they head.
        crate::pad::registry::tabs(
            ui.ctx(),
            ui.layer_id(),
            outer,
            self.id,
            self.tabs.len(),
            current,
        );
        if let (Some(_), Some(cur)) = (focused, current) {
            let (l, r, home, end) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::Home),
                    i.key_pressed(egui::Key::End),
                )
            });
            let last = self.tabs.len().saturating_sub(1);
            let k = if l {
                Some(cur.saturating_sub(1))
            } else if r {
                Some((cur + 1).min(last))
            } else if home {
                Some(0)
            } else if end {
                Some(last)
            } else {
                None
            };
            if let Some(k) = k.filter(|&k| k != cur)
                && let Some(tab) = self.tabs.get(k)
            {
                moved = Some(tab.id.clone());
                ui.memory_mut(|m| m.request_focus(tab_id(self.id, k)));
            }
        }
        if let Some(v) = moved {
            *self.value = v;
            resp.mark_changed();
        }
        resp
    }
}
