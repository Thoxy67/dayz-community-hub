//! The generic kit: the Svelte interface's `lib/components/ui` as egui widgets, dressed from
//! `ui.tokens()` only. App-aware parts live in `parts/`. Call [`install`] once at start
//! (tooltip delays). Widgets are builders implementing `egui::Widget` (`ui.add(…)`)
//! unless they hold content (`show(…)`); state lives in egui memory by id.
//!
//! Widget                    | Constructor
//! --------------------------|-------------------------------------------------------------
//! Button                    | `Button::new("Refresh").icon(lucide::REFRESH_CW).variant(Variant::Accent).size(Size::Sm)`; `.active(b) .enabled(b) .width(w) .start() .trailing(icon) .tip(t) .tip_kbd(t, kbd, Side) .danger_hover()`
//! Chip (latching)           | `Button::chip("1PP", on)`
//! IconButton                | `IconButton::new(lucide::X, "Close").kbd("Esc").side(Side::Bottom).size(Size::IconXs).variant(Variant::Ghost).active(b).danger_hover().tint(c)`
//! Tooltip                   | `response.tip("text")`, `response.tip_kbd("text", "Ctrl K", Side::Bottom)` (trait `TipExt`)
//! Kbd                       | `Kbd::new("Ctrl K")`; `Kbd::width(ui, keys)`, `Kbd::paint_at(ui, keys, x, cy)`
//! Input                     | `Input::new(&mut s).search().placeholder("…").clear_label("Clear").size(FieldSize::Xs).width(w).kind(InputKind::Mono|Password).id_salt(id)`
//! NumberInput               | `NumberInput::new(&mut f64).range(1.0, 65535.0).step(1.0).decimals(0)`
//! TextArea                  | `TextArea::new(&mut s).rows(4).mono().placeholder("…")`
//! Select                    | `Select::new("map", &mut v, &[SelectOption::new(v, "Label").group("G")]).placeholder("All").none(v0).width(w).label(aria)`
//! MultiSelect               | `MultiSelect::new("maps", &mut vec, &[SelectOption::new(v, "Label")]).placeholder("All maps").summary("2 maps").search("Find…", "Clear").empty("No match").width(w).list_width(w).label(aria)`
//! Switch                    | `Switch::new(&mut on, "Label").hint("…").tone(Tone::Warn)`
//! MiniSwitch                | `MiniSwitch::new(&mut on).label("aria")`
//! Checkbox                  | `Checkbox::new(&mut on).indeterminate(b).label("aria")`
//! Toggle (filter row)       | `Toggle::new(&mut on, "Official").count(412).swatch(c).tip("…")`
//! Segmented                 | `Segmented::new(&mut v, &[Segment::new(v, "Label").icon(i).title("…")]).size(FieldSize::Xs).icon_only().fill()`
//! TabStrip                  | `TabStrip::new("id", &mut v, &[Tab::new(v, "Label").icon(i).count(n)]).size(FieldSize::Sm).fill().height(h)`
//! Dialog                    | `Dialog::new("id", "Title").description("…").size(DialogSize::Md).close_label("Close").show(ctx, &mut open, \|ui, open\| body, \|ui, open\| footer)`
//! Confirm                   | `Confirm::new("id", "Delete?", "Delete", "Cancel").description("…").safe().show(ctx, &mut open) -> bool`
//! Toasts                    | `toast::ok(ctx, "Saved")`, `toast::warn/err/say(ctx, text, ToastTone)`, `toast::say_with_action(ctx, text, tone, "Undo", "cmd.id")`, `toast::dismiss(ctx, id)`; the window calls `toast::host(ctx, "Dismiss") -> Option<command id>`
//! Dropdown menu             | `menu::dropdown(&trigger_resp, Side::Bottom, Align::End, \|ui\| { ui.add(MenuItem::new("Copy").icon(i).kbd("Ctrl C").danger().checked(b)); menu::separator(ui); menu::label(ui, "Group"); menu::submenu(ui, "More", None, \|ui\| …); })`
//! Context menu              | `menu::context(&row_resp, \|ui\| …)`
//! Popover                   | `let r = menu::popover_trigger(ui, lucide::PALETTE, "Theme", 28.0); menu::popover(&r, Side::Bottom, Align::End, \|ui\| …)`; `menu::is_open(&r)`
//! Tag (badge)               | `Tag::new("1PP").tone(Tone::Warn).filled().icon(i).tip("…")`
//! Spinner                   | `Spinner::new().size(16.0).color(c)`; `Spinner::paint(ui, rect, color)`
//! Skeleton                  | `Skeleton::new(96.0, 8.0).radius(4).opacity(0.6)`; `SkeletonRows::new(8).media()`
//! Meter (progress)          | `Meter::new(0.4).tone(Tone::Ok).size(MeterSize::Xs).indeterminate().width(w)`
//! Slider                    | `Slider::new(&mut f64, 0.0..=1.0).step(0.05).label("aria")` (`drag_stopped()` = commit)
//! Separator                 | `Separator::horizontal()`, `Separator::vertical()`
//! Split / Splitter          | `Split::new("id", 340.0).pane(Pane::End).range(180.0, 640.0).keep(340.0).stacked().show(ui, \|ui\| main, \|ui\| aside)`; `Splitter::new(id, vertical).show(ui, rect) -> SplitterOutput`
//! VirtualList               | `VirtualList::new("id", n).row_height(44.0).header(28.0).stick_to_end().scroll_to(Some(i)).cursor(Some(i)).show(ui, \|ui\| header, \|ui, i\| row) -> VirtualOutput`
//! Panel                     | `Panel::new("id").title("Filters").no_scroll().padded().show(ui, \|ui\| …)` / `.show_with(ui, \|ui\| toolbar, \|ui\| body)`
//! Field (settings row)      | `Field::new("Label").hint("…").stacked().last().show(ui, \|ui\| control)`
//! SectionLabel              | `SectionLabel::new("Maps").icon(i).no_tick()`; `.show_with(ui, \|ui\| actions)`
//! StatStrip / Stat          | `StatStrip::show(ui, &[Stat::new("Players", "258 643").tone(t.ok).icon(i).grow().tip("…")])`
//! EmptyState                | `EmptyState::new().icon(i).title("…").text("…").detail(raw).compact().show(ui, \|ui\| action)`
//! Disclosure                | `Disclosure::new("id", "Details").default_open(false).show(ui, \|ui\| body)`
//! Alert                     | `Alert::new(Tone::Warn, lucide::TRIANGLE_ALERT, "Title").text("…").banner().dismissible("Dismiss").show(ui, \|ui\| actions) -> AlertResponse`
//! CommandLine               | `CommandLine::new("sudo …", "Copy").width(w)`
//! Copy                      | `Copy::new("1.2.3.4:2302").shown("…").labels("Copy", "Copied")`
//! Signal (ping bars)        | `Signal::new(Some(ms)).pending(b).bars_only().size(SignalSize::Xs)`; `signal_level(ms)`, `signal_color(&t, level)`
//! Players                   | `Players::new(41, 60).bots(3).queue(2).loading(b).compact().bots_label("Bots")`; `fill_tone(p, max)`
//! Flag                      | `Flag::new("fr").size(16.0).tip("France")`
//! Brand marks               | `Brand::Steam.show(ui, 16.0, color)`, `Brand::Linux.paint(&painter, rect, color)`
//! Topo                      | `topo::paint(&painter, rect, &tokens, 0.5)`
//! Drawing helpers           | `paint::{galley, galley_trunc, galley_wrap, tracked, text_at, text_center, ease, lerp, alpha, fade, plate, tick, focus_ring, float_frame, shadow_pop, glow_soft, glow}`, `panel::stencil(ui, text, color, max_w)`

pub mod alert;
pub mod button;
pub mod dialog;
pub mod input;
pub mod kbd;
pub mod marks;
pub mod menu;
pub mod multi_select;
pub mod paint;
pub mod panel;
pub mod progress;
pub mod segmented;
pub mod select;
pub mod slider;
pub mod split;
pub mod tabs;
pub mod tag;
pub mod toast;
pub mod toggle;
pub mod tooltip;
pub mod topo;
pub mod virtual_list;

#[cfg(test)]
mod tests;

pub use alert::{Alert, AlertResponse, CommandLine, Copy};
pub use button::{Button, IconButton, Size, Variant};
pub use dialog::{Confirm, Dialog, DialogSize};
pub use input::{FieldSize, Input, InputKind, NumberInput, TextArea};
pub use kbd::Kbd;
pub use marks::{
    Brand, Flag, PING_TIMEOUT_MS, Players, Signal, SignalSize, fill_tone, signal_color,
    signal_level,
};
pub use menu::{Align, MenuItem};
pub use multi_select::MultiSelect;
pub use panel::{Disclosure, EmptyState, Field, Panel, SectionLabel, Stat, StatStrip};
pub use progress::{Meter, MeterSize, Skeleton, SkeletonRows, Spinner};
pub use segmented::{Segment, Segmented};
pub use select::{Select, SelectOption};
pub use slider::Slider;
pub use split::{Pane, Separator, Split, Splitter, SplitterOutput};
pub use tabs::{Tab, TabStrip};
pub use tag::Tag;
pub use toast::ToastTone;
pub use toggle::{Checkbox, MiniSwitch, Switch, Toggle};
pub use tooltip::{Side, TipExt};
pub use virtual_list::{VirtualList, VirtualOutput};

use egui::Color32;

use crate::theme::Tokens;

/// A meaning colour, as the Svelte props name them (`tone="warn"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Ok,
    Warn,
    Err,
    Info,
    /// Faint (`fg-faint`): nothing to report.
    Muted,
}

impl Tone {
    /// The tone's ink.
    pub fn color(self, t: &Tokens) -> Color32 {
        match self {
            Tone::Neutral => t.fg_muted,
            Tone::Accent => t.accent,
            Tone::Ok => t.ok,
            Tone::Warn => t.warn,
            Tone::Err => t.err,
            Tone::Info => t.info,
            Tone::Muted => t.fg_faint,
        }
    }
}

/// Once at start: the tooltip provider's delays (350ms, and 400ms during which the next
/// hint comes at once).
pub fn install(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        s.interaction.tooltip_delay = 0.35;
        s.interaction.tooltip_grace_time = 0.4;
        s.interaction.show_tooltips_only_when_still = false;
        s.spacing.menu_margin = egui::Margin::same(4);
    });
}
