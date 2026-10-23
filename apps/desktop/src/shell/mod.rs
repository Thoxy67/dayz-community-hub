//! The shell (the Svelte interface's `src/shell`, ported): title bar, rail, status bar, and what floats over
//! everything (the dialog host, the command palette, the shortcuts sheet, the mod
//! operation's dialog, the setup wizard, the pad hints, the window's resize edges).
//!
//! [`crate::app::HubApp`] lays the window out and calls these hooks into their regions:
//!
//! | hook                     | region                                               |
//! |--------------------------|------------------------------------------------------|
//! | [`titlebar`]`(ui, cx)`   | the top strip, `theme::size::TITLEBAR` high          |
//! | [`rail`]`(ui, cx)`       | the left column, `RAIL` or `RAIL_COLLAPSED` wide (`cx.prefs.get().ui.rail_collapsed`) |
//! | [`statusbar`]`(ui, cx)`  | the bottom strip, [`STATUSBAR_HEIGHT`] high          |
//! | [`overlays`]`(ctx, cx)`  | the whole window, after the views, before the toasts |
//!
//! Module by module (each a port of its Svelte namesake):
//!
//! | module        | Svelte                                                       |
//! |---------------|--------------------------------------------------------------|
//! | `titlebar`    | `TitleBar.svelte`, `LanguageMenu.svelte`, `ThemeMenu.svelte` (in `menus`) |
//! | `rail`        | `Rail.svelte`, `RejoinCard.svelte`                           |
//! | `statusbar`   | `StatusBar.svelte`, `status/*.svelte`                        |
//! | `dialogs`     | `ConfirmHost`, `PasswordHost`, `ConnectDialog`, `ShortcutsDialog` |
//! | `palette`     | `CommandPalette.svelte` (+ `lib/shortcuts/fuzzy.ts` in `fuzzy`) |
//! | `pad_hints`   | `PadHints.svelte`                                            |
//! | `window`      | the resize edges of the undecorated window                   |
//!
//! The mod operation's dialog and the setup wizard belong to the mods and setup views:
//! they install themselves with [`set_mod_op_host`] / [`set_wizard_host`]; until then a
//! plain progress dialog stands in for the first, nothing for the second.
//!
//! What the shell remembers between frames (the palette's query, a status segment's
//! clock, the theme menu's hover preview) lives in one [`State`] kept in egui's memory;
//! hooks lock it for their own drawing only.

mod dialogs;
pub mod fuzzy;
mod menus;
mod pad_hints;
mod palette;
mod rail;
mod statusbar;
mod titlebar;
mod window;
pub use window::release_after_hand_over;

pub(crate) use dialogs::shortcut_list;

use std::sync::{Arc, OnceLock};

use egui::mutex::Mutex;

use crate::app::Ctx;

/// The status bar's height (`h-7`).
pub const STATUSBAR_HEIGHT: f32 = 28.0;

/// The pad hints' bar height (`h-7`), drawn just above the status bar in pad mode.
pub const PAD_HINTS_HEIGHT: f32 = 28.0;

/// A dialog a view owns, drawn by the shell over everything: the mod operation's
/// (`ModOpDialog.svelte`) or the setup wizard (`SetupWizard.svelte`).
pub type Host = for<'a, 'b> fn(&'a egui::Context, &'a mut Ctx<'b>);

static MOD_OP_HOST: OnceLock<Host> = OnceLock::new();
static WIZARD_HOST: OnceLock<Host> = OnceLock::new();

/// The mods view's operation dialog, drawn while `cx.stores.mods.op.active` and not
/// minimised. The first call wins.
pub fn set_mod_op_host(host: Host) {
    let _ = MOD_OP_HOST.set(host);
}

/// The setup wizard, drawn while `cx.stores.app.show_wizard`. The first call wins.
pub fn set_wizard_host(host: Host) {
    let _ = WIZARD_HOST.set(host);
}

/// What the shell keeps between frames.
#[derive(Default)]
pub(crate) struct State {
    pub palette: palette::Palette,
    pub status: statusbar::StatusState,
    pub dialogs: dialogs::DialogState,
    pub menus: menus::MenuState,
}

/// The stencilled label (`label-stencil`: display face, 11 px, tracked 0.12 em, capitals).
pub(crate) fn stencil(
    ui: &egui::Ui,
    text: &str,
    color: egui::Color32,
    max_width: Option<f32>,
) -> Arc<egui::Galley> {
    crate::kit::paint::tracked(
        ui,
        text,
        crate::theme::font(
            crate::theme::fonts::Face::DisplayBold,
            crate::theme::size::TEXT_2XS,
        ),
        color,
        0.12,
        true,
        max_width,
    )
}

/// The keyboard's focus ring (`:focus-visible`) on a shell control drawn by hand: `rect`
/// is where it goes (the control's shape, or inside it where the edge would clip it).
pub(crate) fn ring(ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect, radius: u8) {
    if crate::kit::paint::keyboard_focus(ui, resp) {
        let tk = crate::theme::TokensExt::tokens(ui);
        crate::kit::paint::focus_ring(ui.painter(), rect, radius, &tk);
    }
}

fn state(ctx: &egui::Context) -> Arc<Mutex<State>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<Mutex<State>>>(egui::Id::new("dzch.shell"))
            .clone()
    })
}

// ── the shell's commands (registered in `commands.rs`) ──────────────────────

fn arg<'a>(a: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(serde_json::Value::as_str)
}

/// `window.drag`: move the window with the pointer (as a press on the title bar does).
pub fn cmd_window_drag(cx: &mut Ctx, _: &serde_json::Value) {
    window::hand_over(cx.egui, egui::ViewportCommand::StartDrag);
}

/// `window.minimize_maximize_close {"action": "minimize" | "maximize" | "close"}`; the
/// title bar's three buttons. `maximize` toggles.
pub fn cmd_window(cx: &mut Ctx, a: &serde_json::Value) {
    let max = cx.stores.app.maximized;
    let cmd = match arg(a, "action") {
        // Through the window's life: on Wayland it is the only one to know.
        Some("minimize") => {
            crate::platform::life::apply(cx.egui, crate::platform::life::Act::Minimize);
            return;
        }
        Some("maximize") => egui::ViewportCommand::Maximized(!max),
        Some("close") => egui::ViewportCommand::Close,
        _ => return,
    };
    cx.egui.send_viewport_cmd(cmd);
}

fn open_menu(cx: &Ctx, which: &str) {
    let id = cx
        .egui
        .data(|d| d.get_temp::<egui::Id>(egui::Id::new(("dzch.shell.menu", which))));
    if let Some(id) = id {
        egui::Popup::open_id(cx.egui, id);
    }
}

/// `theme.menu`: open the title bar's theme menu.
pub fn cmd_theme_menu(cx: &mut Ctx, _: &serde_json::Value) {
    open_menu(cx, "theme");
}

/// `steam.downloader_menu`: open the status bar's Steam menu (what downloads the mods).
pub fn cmd_steam_menu(cx: &mut Ctx, _: &serde_json::Value) {
    open_menu(cx, "steam");
}

/// `palette.actions`: the palette, empty (recent picks, actions, views).
pub fn cmd_palette_actions(cx: &mut Ctx, _: &serde_json::Value) {
    cx.stores.dialogs.shortcuts = false;
    cx.stores.dialogs.palette = true;
}

/// `palette.connect_address {"address": "1.2.3.4:2302"}`: the palette typed into with an
/// address, its Address section first.
pub fn cmd_palette_address(cx: &mut Ctx, a: &serde_json::Value) {
    let text: String = arg(a, "address").unwrap_or("").chars().take(256).collect();
    cx.egui
        .data_mut(|d| d.insert_temp(egui::Id::new(palette::SEED), text));
    cx.stores.dialogs.shortcuts = false;
    cx.stores.dialogs.palette = false;
    // Reopened on the next frame so the seed is read as it opens.
    cx.stores.bus.run(|cx| cx.stores.dialogs.palette = true);
}

/// `palette.navigate {"move": "down" | "up" | "pagedown" | "pageup" | "next_section" |
/// "prev_section" | "run" | "close"}`: the palette's keys, for a pad or the control channel.
pub fn cmd_palette_navigate(cx: &mut Ctx, a: &serde_json::Value) {
    let Some(m) = arg(a, "move") else {
        return;
    };
    let m = m.to_string();
    cx.egui.data_mut(|d| {
        let q = d.get_temp_mut_or_default::<Vec<String>>(egui::Id::new(palette::MOVES));
        if q.len() < 64 {
            q.push(m);
        }
    });
}

/// `field.escape`: leave the focused text field (egui's own Esc empties nothing; a
/// view's search field empties itself on Esc before this).
pub fn cmd_field_escape(cx: &mut Ctx, _: &serde_json::Value) {
    let focused = cx.egui.memory(|m| m.focused());
    if let Some(id) = focused {
        cx.egui.memory_mut(|m| m.surrender_focus(id));
    }
}

/// The title bar: drag area, search pill, counters, profile, language, theme, window
/// buttons.
pub fn titlebar(ui: &mut egui::Ui, cx: &mut Ctx) {
    let st = state(ui.ctx());
    let mut st = st.lock();
    titlebar::show(ui, cx, &mut st.menus);
}

/// The launcher's mark (`/icon.svg`, its own colours) fitted in `rect`: the title bar's,
/// and the setup's.
pub fn logo(ui: &egui::Ui, rect: egui::Rect) {
    titlebar::logo(ui, rect);
}

/// The navigation rail.
pub fn rail(ui: &mut egui::Ui, cx: &mut Ctx) {
    rail::show(ui, cx);
}

/// The status bar.
pub fn statusbar(ui: &mut egui::Ui, cx: &mut Ctx) {
    let st = state(ui.ctx());
    let mut st = st.lock();
    statusbar::show(ui, cx, &mut st.status);
}

/// Whether the setup wizard covers the window below the title bar this frame: the main
/// area's view is then not drawn (nobody would see it, and it would cost a frame).
pub fn wizard_covers(cx: &Ctx) -> bool {
    cx.stores.app.show_wizard && WIZARD_HOST.get().is_some()
}

/// Everything that floats: dialogs, palette, sheet, mod operation, wizard, pad hints,
/// and the window's resize edges.
pub fn overlays(ctx: &egui::Context, cx: &mut Ctx) {
    let st = state(ctx);
    let mut st = st.lock();
    pad_hints::show(ctx, cx);
    match MOD_OP_HOST.get() {
        Some(host) => {
            if cx.stores.mods.op.active && !cx.stores.mods.op.minimised {
                host(ctx, cx);
            }
        }
        None => dialogs::mod_op_fallback(ctx, cx, &mut st.dialogs.mod_op_until),
    }
    if cx.stores.app.show_wizard
        && let Some(host) = WIZARD_HOST.get()
    {
        host(ctx, cx);
    }
    dialogs::connect(ctx, cx, &mut st.dialogs);
    dialogs::password(ctx, cx, &mut st.dialogs);
    dialogs::shortcuts(ctx, cx, &mut st.dialogs);
    crate::views::favorites::edit::show(ctx, cx);
    palette::show(ctx, cx, &mut st.palette);
    // The question goes over everything else: a flow waits on it.
    dialogs::confirm(ctx, cx, &mut st.dialogs);
    window::resize_edges(ctx, cx);
}
