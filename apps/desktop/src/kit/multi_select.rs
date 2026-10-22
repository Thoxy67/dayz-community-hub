//! `ui/select`, several values: the same control as [`super::Select`], a list that ticks
//! and unticks and stays open, and a search field at its top that narrows it as one types.
//!
//! The first row, worded as the placeholder, clears the choice (none = all). The field
//! takes the keys as the list opens; the search is case-insensitive, anywhere in the
//! label. Arrows move, Enter ticks (the first match while typing), Space ticks once the
//! arrows have moved (before that it types), Escape closes and gives the keys back to the
//! control. The pad walks the rows (each is focusable) and A ticks.

use egui::{
    Color32, CornerRadius, Id, Popup, PopupCloseBehavior, Rect, RectAlign, Response, Sense, Ui,
    Vec2, Widget, WidgetInfo, WidgetType,
};

use super::paint;
use super::select::{SelectOption, trigger};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// `MultiSelect::new("maps", &mut maps, &options).placeholder("All maps")
/// .summary("2 maps").search("Find a map…", "Clear search").empty("No map matches")`.
/// `changed()` on the response when the choice changed.
#[must_use = "add it with ui.add(…)"]
pub struct MultiSelect<'a, V: PartialEq + Clone> {
    id: Id,
    values: &'a mut Vec<V>,
    options: &'a [SelectOption<'a, V>],
    placeholder: &'a str,
    summary: &'a str,
    search: Option<(&'a str, &'a str)>,
    empty: &'a str,
    width: f32,
    list_width: Option<f32>,
    enabled: bool,
    label: &'a str,
}

impl<'a, V: PartialEq + Clone> MultiSelect<'a, V> {
    pub fn new(
        id_salt: impl egui::AsId,
        values: &'a mut Vec<V>,
        options: &'a [SelectOption<'a, V>],
    ) -> Self {
        Self {
            id: Id::new(id_salt),
            values,
            options,
            placeholder: "",
            summary: "",
            search: None,
            empty: "",
            width: 160.0,
            list_width: None,
            enabled: true,
            label: "",
        }
    }

    /// Shown (faint) while nothing is chosen, and the words of the row that clears.
    pub fn placeholder(mut self, p: &'a str) -> Self {
        self.placeholder = p;
        self
    }

    /// What the closed control says while something is chosen ("enoch", "3 maps"); by
    /// default the labels chosen, joined.
    pub fn summary(mut self, s: &'a str) -> Self {
        self.summary = s;
        self
    }

    /// A search field at the top of the list: its placeholder and its clear button's
    /// accessible name.
    pub fn search(mut self, placeholder: &'a str, clear_label: &'a str) -> Self {
        self.search = Some((placeholder, clear_label));
        self
    }

    /// Said in the list when the search leaves no row.
    pub fn empty(mut self, e: &'a str) -> Self {
        self.empty = e;
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// The list's width, when it should be wider than the control.
    pub fn list_width(mut self, w: f32) -> Self {
        self.list_width = Some(w);
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

/// What the open list remembers between frames.
#[derive(Clone, Debug, Default)]
struct Find {
    query: String,
    /// The row lit by the keys, among the rows shown.
    hl: Option<usize>,
    /// The arrows moved since the last letter typed (Space then ticks, not types).
    arrowed: bool,
    /// The field should take the keys (the list just opened).
    focus: bool,
    /// The field had the keys (closing then gives them back to the control).
    typing: bool,
}

/// A row of the open list: the one that clears, or an option by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    All,
    Opt(usize),
}

/// The rows shown for `query`: every option under the row that clears, or only the
/// options whose label holds it (any case).
fn rows<V>(options: &[SelectOption<'_, V>], query: &str) -> Vec<Row> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return std::iter::once(Row::All)
            .chain((0..options.len()).map(Row::Opt))
            .collect();
    }
    options
        .iter()
        .enumerate()
        .filter(|(_, o)| o.label.to_lowercase().contains(&needle))
        .map(|(i, _)| Row::Opt(i))
        .collect()
}

impl<V: PartialEq + Clone> Widget for MultiSelect<'_, V> {
    fn ui(self, ui: &mut Ui) -> Response {
        let joined;
        let shown = if self.values.is_empty() {
            self.placeholder
        } else if !self.summary.is_empty() {
            self.summary
        } else {
            joined = self
                .options
                .iter()
                .filter(|o| self.values.contains(&o.value))
                .map(|o| o.label)
                .collect::<Vec<_>>()
                .join(", ");
            joined.as_str()
        };
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let w = self.width;
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL), sense);
        let popup_id = self.id.with("popup");
        let find_id = self.id.with("find");
        let open = Popup::is_id_open(ui.ctx(), popup_id);
        let label = if self.label.is_empty() {
            shown
        } else {
            self.label
        };
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, label));
        let set = !self.values.is_empty();
        trigger(ui, rect, &resp, shown, set, open, self.enabled);
        if !self.enabled {
            return resp;
        }
        let toggle = resp.clicked()
            || (resp.has_focus()
                && ui
                    .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space)));
        if toggle && !open {
            // A fresh list: no search left over, the field takes the keys.
            let fresh = Find {
                focus: self.search.is_some(),
                ..Find::default()
            };
            ui.data_mut(|d| d.insert_temp(find_id, fresh));
        }
        let list_w = self.list_width.unwrap_or(w).max(w).max(128.0);
        let popup = Popup::from_response(&resp)
            .id(popup_id)
            .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
            .kind(egui::PopupKind::Popup)
            .layout(egui::Layout::top_down_justified(egui::Align::Min))
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .frame(paint::float_frame(&ui.tokens(), 4))
            .align(RectAlign::BOTTOM_START)
            .gap(4.0)
            .width(list_w - 8.0);
        let mut act: Option<Row> = None;
        let mut st: Find = ui.data(|d| d.get_temp(find_id)).unwrap_or_default();
        let values: &[V] = self.values;
        let shown_popup = popup.show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.set_min_width(list_w - 10.0);
            act = list(ui, &self, values, &mut st);
        });
        if shown_popup.is_some() {
            if !Popup::is_id_open(ui.ctx(), popup_id) && st.typing {
                // Closed from the keys: they go back to the control.
                if !ui.input(|i| i.pointer.any_click()) {
                    resp.request_focus();
                }
                st.typing = false;
            }
            ui.data_mut(|d| d.insert_temp(find_id, st));
        }
        match act {
            Some(Row::All) if !self.values.is_empty() => {
                self.values.clear();
                resp.mark_changed();
            }
            Some(Row::Opt(i)) => {
                if let Some(o) = self.options.get(i) {
                    if let Some(at) = self.values.iter().position(|v| *v == o.value) {
                        self.values.remove(at);
                    } else {
                        self.values.push(o.value.clone());
                    }
                    resp.mark_changed();
                }
            }
            _ => {}
        }
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

/// The open list: the keys, the search field, the rows. Returns the row ticked.
fn list<V: PartialEq + Clone>(
    ui: &mut Ui,
    m: &MultiSelect<'_, V>,
    values: &[V],
    st: &mut Find,
) -> Option<Row> {
    let mut act = None;
    let mut shown = rows(m.options, &st.query);
    // The keys, before the field reads them: Enter and a ticking Space are the list's.
    let (down, up) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::ArrowUp),
        )
    });
    if !shown.is_empty() && (down || up) {
        let last = shown.len() - 1;
        st.hl = Some(match (st.hl, down) {
            (None, _) => 0,
            (Some(h), true) => (h + 1).min(last),
            (Some(h), false) => h.saturating_sub(1),
        });
        st.arrowed = true;
    }
    let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let space = st.arrowed
        && st.hl.is_some()
        && ui.input_mut(|i| {
            let hit = i.consume_key(egui::Modifiers::NONE, egui::Key::Space);
            if hit {
                i.events
                    .retain(|e| !matches!(e, egui::Event::Text(s) if s == " "));
            }
            hit
        });
    if enter || space {
        let typed = !st.query.trim().is_empty();
        act = st
            .hl
            .or(typed.then_some(0))
            .and_then(|h| shown.get(h).copied());
    }
    if let Some((hint, clear)) = m.search {
        let frame = egui::Frame::new().inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 4,
        });
        frame.show(ui, |ui| {
            let before = st.query.clone();
            let r = ui.add(
                super::Input::new(&mut st.query)
                    .search()
                    .size(super::FieldSize::Xs)
                    .placeholder(hint)
                    .clear_label(clear)
                    .char_limit(64)
                    .id_salt(m.id.with("search")),
            );
            if st.focus {
                if r.has_focus() {
                    st.focus = false;
                } else {
                    r.request_focus();
                }
            }
            st.typing = r.has_focus();
            if st.typing {
                // Up and Down walk the list: they must not move egui's focus away.
                let keep = egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    tab: false,
                    escape: false,
                };
                ui.memory_mut(|mem| mem.set_focus_lock_filter(r.id, keep));
            }
            if st.query != before {
                shown = rows(m.options, &st.query);
                st.arrowed = false;
                st.hl = (!st.query.trim().is_empty() && !shown.is_empty()).then_some(0);
            }
        });
    }
    let moved = down || up;
    let room = 320.0 - 10.0 - if m.search.is_some() { 30.0 } else { 0.0 };
    egui::ScrollArea::vertical()
        .id_salt(m.id.with("scroll"))
        .max_height(room)
        .show(ui, |ui| {
            if shown.is_empty() && !m.empty.is_empty() {
                empty_row(ui, m.empty);
            }
            for (n, row) in shown.iter().enumerate() {
                let (text, on, faint) = match *row {
                    Row::All => (m.placeholder, values.is_empty(), true),
                    Row::Opt(i) => {
                        let Some(o) = m.options.get(i) else {
                            continue;
                        };
                        (o.label, values.contains(&o.value), false)
                    }
                };
                let r = check_item(ui, text, on, faint, st.hl == Some(n));
                if r.clicked() {
                    act = Some(*row);
                    st.hl = Some(n);
                    // Ticked with the mouse: the field keeps the keys, to type on. (The
                    // pad's A is no pointer click: its focus stays on the row.)
                    if m.search.is_some() && ui.input(|i| i.pointer.any_click()) {
                        st.focus = true;
                    }
                }
                if r.hovered() && ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                    st.hl = Some(n);
                }
                if moved && st.hl == Some(n) {
                    r.scroll_to_me(None);
                }
            }
        });
    act
}

/// "Nothing matches", faint, the height of a row.
fn empty_row(ui: &mut Ui, text: &str) {
    let t = ui.tokens();
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), size::ROW), Sense::hover());
    let f = font(Face::Sans, size::TEXT_XS);
    let g = paint::galley_trunc(ui, text, f, t.fg_faint, r.width() - 12.0);
    paint::text_at(ui.painter(), &g, r.left() + 6.0, r.center().y, t.fg_faint);
}

/// A row of the list: a box ticked or not, then the label.
fn check_item(ui: &mut Ui, label: &str, on: bool, faint: bool, highlighted: bool) -> Response {
    let t = ui.tokens();
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size::ROW), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, on, label));
    let lit = highlighted || resp.hovered() || resp.has_focus();
    let hover = paint::ease(ui, resp.id.with("hover"), lit);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(size::RADIUS_XS),
        if lit { t.raised } else { Color32::TRANSPARENT },
    );
    let bx = Rect::from_center_size(
        egui::pos2(rect.left() + 6.0 + 7.0, rect.center().y),
        Vec2::splat(14.0),
    );
    let tick = if on { 1.0 } else { 0.0 };
    super::toggle::paint_box(ui, bx, on, false, tick, hover, true);
    let ink = if on || lit {
        t.fg
    } else if faint {
        t.fg_faint
    } else {
        t.fg_muted
    };
    let x = bx.right() + 8.0;
    let f = font(Face::Sans, size::TEXT_XS);
    let g = paint::galley_trunc(ui, label, f, ink, rect.right() - 6.0 - x);
    paint::text_at(ui.painter(), &g, x, rect.center().y, ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
