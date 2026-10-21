//! The launcher's executable: the Steamworks worker hand-off, the arguments, the tokio
//! runtime, the hub, and the window (as `apps/gui/src-tauri/src/main.rs` and Tauri's
//! `setup()` did).
// No console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use clap::Parser;
use dz_hub::{CliArgs, Hub, HubConfig, HubMode};
use dzch_ui::app::EventPipe;
use dzch_ui::prefs::Prefs;

/// The launcher's arguments: the old ones, unchanged for users (`--connect`,
/// `--reconnect`, a `.dzch` file or `dzch://` link), plus the window's own, hidden.
#[derive(Parser, Debug)]
#[command(name = "dayz-community-hub", about = "DayZ Community Hub")]
struct Args {
    #[command(flatten)]
    cli: CliArgs,
    /// An invented world (9 500 servers, mods, a profile) in a temporary directory.
    #[arg(long, hide = true)]
    mock: bool,
    /// Listen for the control channel on 127.0.0.1:<port>.
    #[arg(long, hide = true, value_name = "PORT")]
    control: Option<u16>,
    /// Keep the launcher's data here instead of the usual directory.
    #[arg(long, hide = true, value_name = "DIR")]
    data_dir: Option<PathBuf>,
}

/// The window as Tauri opened it (`tauri.conf.json`): 1280×800, at least 900×600,
/// no system decorations (the title bar is ours), opaque.
const SIZE: [f32; 2] = [1280.0, 800.0];
const MIN_SIZE: [f32; 2] = [900.0, 600.0];
const TITLE: &str = "DayZ Community Hub";

fn main() -> eframe::Result {
    // Started by the launcher to talk to Steam: does that, exits.
    dz_steamworks::serve_if_worker();
    #[cfg(all(windows, not(debug_assertions)))]
    fatal_error_box::install();

    let args = Args::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => fail(&format!("cannot start the async runtime: {e}")),
    };

    let pipe = EventPipe::new();
    let config = HubConfig {
        mode: if args.mock {
            HubMode::Mock
        } else {
            HubMode::Live
        },
        data_dir: args.data_dir.clone(),
        args: args.cli.clone(),
    };
    let hub = match Hub::new(config, pipe.sink.clone()) {
        Ok(h) => h,
        Err(e) => fail(&e),
    };
    {
        let _guard = runtime.enter();
        hub.start();
    }
    if !args.mock {
        // `dzch://` links open this executable (the AppImage and the zip have no
        // installer, and a moved executable needs it pointed at its new place).
        let _ = dz_hub::features::dzch_cli::register_url_scheme();
    }

    // The interface's preferences, from the hub's data directory (a temporary one in
    // mock mode: the mock never touches the user's files).
    let prefs = Prefs::load(Prefs::path_in(&dz_hub::paths::data_dir()));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(TITLE)
        .with_app_id("dayz-community-hub")
        .with_inner_size(SIZE)
        .with_min_inner_size(MIN_SIZE)
        .with_decorations(false)
        .with_transparent(false);
    if let Some(w) = prefs.get().window {
        viewport = viewport
            .with_inner_size([w.size[0].max(MIN_SIZE[0]), w.size[1].max(MIN_SIZE[1])])
            .with_maximized(w.maximized);
        if let Some(p) = w.pos {
            viewport = viewport.with_position(p);
        }
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: prefs.get().window.is_none_or(|w| w.pos.is_none()),
        persist_window: false,
        ..Default::default()
    };
    let handle = runtime.handle().clone();
    let EventPipe { rx, wake, .. } = pipe;
    let result = eframe::run_native(
        TITLE,
        options,
        Box::new(move |cc| {
            dzch_ui::rt::Rt::install(handle.clone(), cc.egui_ctx.clone());
            let _ = wake.set(cc.egui_ctx.clone());
            let control = args.control.and_then(|port| {
                let dir = dz_hub::paths::data_dir();
                dzch_ui::control::start(port, &handle, hub.clone(), cc.egui_ctx.clone(), &dir)
            });
            let mut app = dzch_ui::HubApp::new(&cc.egui_ctx, hub, rx, prefs);
            if let Some(c) = control {
                app.set_control(c);
            }
            Ok(Box::new(app))
        }),
    );
    // Background work is dropped with the runtime, without waiting on it.
    runtime.shutdown_background();
    result
}

/// Say why the launcher cannot start, and stop.
fn fail(msg: &str) -> ! {
    eprintln!("DayZ Community Hub: {msg}");
    #[cfg(all(windows, not(debug_assertions)))]
    fatal_error_box::show(&format!("DayZ Community Hub cannot start: {msg}"));
    std::process::exit(1);
}

/// A release build on Windows has no console: a panic or a start-up failure would
/// close the launcher without a word. Say what happened in a message box first. (A
/// release build aborts on panic, so every panic there is the end.)
#[cfg(all(windows, not(debug_assertions)))]
mod fatal_error_box {
    use std::ffi::c_void;

    const MB_OK: u32 = 0x0;
    const MB_ICONERROR: u32 = 0x10;
    const MB_TOPMOST: u32 = 0x0004_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn show(text: &str) {
        let (text, caption) = (wide(text), wide("DayZ Community Hub"));
        // SAFETY: both strings are NUL-terminated UTF-16; no owner window.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | MB_ICONERROR | MB_TOPMOST,
            );
        }
    }

    pub fn install() {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            default(info);
            let what = info
                .payload()
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown error".to_owned());
            let mut text = format!("DayZ Community Hub stopped: {what}");
            if let Some(at) = info.location() {
                text.push_str(&format!("\n\n({}:{})", at.file(), at.line()));
            }
            let lower = what.to_ascii_lowercase();
            if lower.contains("wgpu") || lower.contains("adapter") || lower.contains("surface") {
                text.push_str(
                    "\n\nThe launcher draws its window with the graphics card (Direct3D 12 or \
                     Vulkan). Update the graphics driver, then start the launcher again.",
                );
            }
            show(&text);
        }));
    }
}
