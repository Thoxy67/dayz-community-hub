//! The launcher's executable: the Steamworks worker hand-off, the arguments, the tokio
//! runtime, the hub, and the window (what the Tauri shell's `main.rs` and `setup()` did
//! before the egui rewrite).
// No console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

use clap::Parser;
use dz_hub::{CliArgs, Hub, HubConfig, HubMode};
use dzch_ui::app::{EventPipe, Kept, KeptSlot};
use dzch_ui::platform;
use dzch_ui::platform::life::Session;
use dzch_ui::prefs::Prefs;
use eframe::{egui_wgpu, wgpu};

/// The launcher's arguments: the old ones, unchanged for users (`--connect`,
/// `--reconnect`, a `.dzch` file or `dzch://` link), plus the window's own, hidden.
#[derive(Parser, Debug)]
#[command(name = "dayz-community-hub", about = "DayZ Community Hub", version = dz_hub::VERSION)]
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
    /// Take part in single instance under this name (also with `--mock`), apart from
    /// the real launcher.
    #[arg(long, hide = true, value_name = "NAME")]
    instance: Option<String>,
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
    // Started by a restart (after an update, an import, a reset): the previous window
    // goes first, so the single-instance check below does not hand off to it. Read
    // now, while this is the only thread, and taken out of the environment.
    let restart_of = dz_hub::features::updater::take_restart_of();

    let mut args = parse_args();
    // A restart runs again with the window's own flags only, never the link, the
    // file, `--connect` or `--reconnect` this one started with.
    dz_hub::features::updater::set_restart_args(restart_args(&args));
    // A file named relative to where this ran, or a desktop's `file://` URL.
    if let Ok(cwd) = std::env::current_dir() {
        args.cli = platform::normalize(args.cli, &cwd);
    }
    // One window per user: a later launch hands its arguments to the first and stops.
    let key = platform::instance::key(
        args.mock,
        args.instance.as_deref(),
        args.data_dir.as_deref(),
    );
    let instance = match (&key, platform::instance::default_dir()) {
        (Some(key), Some(dir)) => {
            // After a restart: the previous window's lock, the moment it lets go.
            let wait = restart_of.map_or(std::time::Duration::ZERO, |_| {
                dz_hub::features::updater::restart_wait()
            });
            match platform::instance::start_after(&dir, key, &args.cli, wait) {
                platform::instance::Start::HandedOff => std::process::exit(0),
                platform::instance::Start::Primary(p) => Some(p),
                platform::instance::Start::Alone => None,
            }
        }
        _ => {
            // No lock to wait on (`--mock` alone): the previous process itself.
            if let Some(pid) = restart_of {
                dz_hub::features::updater::wait_for_previous(pid);
            }
            None
        }
    };
    // A few workers: the heavy work (the list's order, the cache, the PTY, process
    // scans) goes to the blocking pool, and the async side mostly waits on sockets.
    let workers = std::thread::available_parallelism()
        .map_or(2, std::num::NonZeroUsize::get)
        .clamp(2, 4);
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(workers)
        .thread_name("dzch-rt")
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
        // The profile and the cached list load while the window is being made.
        hub.prewarm();
    }
    // `dzch://` links and `.dzch` files open this executable (the AppImage and the zip
    // have no installer, and a moved executable needs it pointed at its new place).
    platform::register::spawn(args.mock);
    // Who to wake: the window of the moment, or this thread between windows (on
    // Wayland closing into the tray really closes the window, and the launcher goes
    // on without one: `platform::life`).
    let session = Session::new(pipe.wake.clone());
    dzch_ui::rt::Rt::install_with(runtime.handle().clone(), pipe.wake.clone());
    // A later launch raises the window, or makes one, and hands its arguments on
    // (queued until the app is initialised, as the start-up ones are).
    if let Some(p) = &instance {
        let (hub, session) = (hub.clone(), Arc::clone(&session));
        let listening = p.listen(move |args| {
            // Also out of the tray: a hidden or closed window is shown again.
            session.show();
            if platform::asks_something(&args) {
                hub.hand_off(args);
            }
        });
        if let Err(e) = listening {
            eprintln!("single instance: {e}");
        }
    }

    // The interface's preferences, from the hub's data directory (a temporary one in
    // mock mode: the mock never touches the user's files).
    let data_dir = dz_hub::paths::data_dir();
    let EventPipe { rx, wake, .. } = pipe;
    let mut kept = Kept::new(rx, Prefs::load(Prefs::path_in(&data_dir)));
    kept.life.set_session(Arc::clone(&session));
    // The control channel outlives the windows too: one listener for the process.
    kept.control = args.control.and_then(|port| {
        dzch_ui::control::start(port, runtime.handle(), hub.clone(), wake.clone(), &data_dir)
    });
    // A Steam Deck and Game Mode, read once here (sysfs) and kept for the frames.
    let deck = platform::deck::warm();
    let slot: KeptSlot = Arc::new(Mutex::new(None));
    let result = loop {
        let window = Window {
            hub: &hub,
            wake: &wake,
            slot: &slot,
            data_dir: &data_dir,
            game_mode: deck.game_mode,
        };
        let result = window.run(kept);
        wake.clear();
        // What the window kept; none when it was never made.
        let back = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
        let into_tray = session.take_away();
        match back {
            Some(mut k) if into_tray && result.is_ok() => {
                if k.wait() {
                    kept = k;
                    continue;
                }
                // Quit from the tray: what a window's exit does.
                k.prefs.flush();
                hub.discord_quit(std::time::Duration::from_millis(300));
                break result;
            }
            _ => break result,
        }
    };
    // The window could not open (no graphics adapter, no display): say so, as for any
    // start-up failure.
    if let Err(e) = &result {
        report(&format!("cannot open the window: {e}"));
    }
    // Background work is dropped with the runtime; a write already on the blocking
    // pool (the profile, the play log, a favourite toggled just before the close) is
    // given a moment to land, bounded so a hung disk never keeps the process. The
    // window is gone by now, and a restarted instance waits for the lock below.
    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
    drop(instance);
    result
}

/// One window, made from what the previous one kept (or what `main` gathered).
struct Window<'a> {
    hub: &'a Hub,
    wake: &'a dzch_ui::rt::Waker,
    slot: &'a KeptSlot,
    data_dir: &'a std::path::Path,
    game_mode: bool,
}

impl Window<'_> {
    /// Open the window and run it until it closes (eframe reuses its event loop from
    /// one window to the next). Returns when the window is gone; what outlives it is
    /// then in the slot.
    fn run(&self, kept: Kept) -> eframe::Result {
        // The window where it was (eframe's `window.ron` beside the preferences; see
        // `platform::window`), else centred at Tauri's default size.
        let window_file = platform::window::path_in(self.data_dir);
        platform::window::sanitize(&window_file);
        let saved = kept.prefs.get().window;
        let centered = !platform::window::has_saved_position(&window_file);
        let game_mode = self.game_mode;
        let options = |gpu: Gpu| {
            let mut viewport = egui::ViewportBuilder::default()
                .with_title(TITLE)
                .with_app_id("dayz-community-hub")
                .with_icon(Arc::new(platform::window::icon()))
                .with_inner_size(SIZE)
                .with_min_inner_size(MIN_SIZE)
                .with_decorations(false)
                .with_transparent(false);
            // Before the first `window.ron`: the size the preferences kept (eframe's
            // file, once written, takes over).
            if let Some(w) = saved {
                viewport = viewport
                    .with_inner_size([w.size[0].max(MIN_SIZE[0]), w.size[1].max(MIN_SIZE[1])])
                    .with_maximized(w.maximized);
            }
            // Game Mode shows one window over the whole screen and scales one of
            // another size to fit (blurred): the window takes the screen's size (here
            // for the first start, and again in `restored`, which runs after eframe's
            // saved window).
            if game_mode {
                viewport = viewport.with_fullscreen(true);
            }
            eframe::NativeOptions {
                viewport,
                centered,
                persist_window: true,
                persistence_path: Some(window_file.clone()),
                window_builder: Some(Box::new(move |b| {
                    platform::window::restored(b, saved, game_mode)
                })),
                wgpu_options: wgpu_options(gpu),
                ..Default::default()
            }
        };
        let (hub, wake, slot) = (self.hub.clone(), self.wake.clone(), Arc::clone(self.slot));
        let creator: eframe::AppCreator<'static> = Box::new(move |cc| {
            wake.set(&cc.egui_ctx);
            let mut app = dzch_ui::HubApp::resume(&cc.egui_ctx, hub, kept, slot);
            app.set_saved_window(cc.storage.and_then(|s| s.get_string("window")));
            // The tray icon (Linux: its own thread; Windows: this thread's message
            // loop), with the mock too, to try it. Started with the first window, then
            // kept from one to the next.
            if !app.life_mut().has_tray() {
                let view = platform::life::tray_view(true, None);
                if let Some(tray) = platform::tray::Tray::start(&wake, view) {
                    app.life_mut().set_tray(tray);
                }
            }
            Ok(Box::new(app))
        });
        // The app is made once, by whichever attempt gets a graphics device: a first
        // attempt that fails before it (no adapter, no surface on the primary backend)
        // leaves it for the fallback.
        let creator = Rc::new(RefCell::new(Some(creator)));
        let plan = gpu_plan();
        let mut result = Ok(());
        for (i, gpu) in plan.iter().enumerate() {
            let slot = Rc::clone(&creator);
            result = eframe::run_native(
                TITLE,
                options(*gpu),
                Box::new(move |cc| match slot.borrow_mut().take() {
                    Some(make) => make(cc),
                    None => Err("the window was already made".into()),
                }),
            );
            let retry = matches!(result, Err(eframe::Error::Wgpu(_)))
                && creator.borrow().is_some()
                && i + 1 < plan.len();
            if !retry {
                break;
            }
            if let Err(e) = &result {
                eprintln!("DayZ Community Hub: {e}; trying the fallback graphics backend");
            }
        }
        result
    }
}

/// The arguments, never fatal: `--help` and `--version` are shown (a message box on
/// Windows, whose release build has no console) and end the process; an unknown or
/// broken argument (a typo in a shortcut, a flag a third-party launcher adds) is
/// reported on stderr and left out, and the window opens with the rest.
fn parse_args() -> Args {
    use clap::error::ErrorKind;
    match Args::try_parse() {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            let _ = e.print();
            #[cfg(all(windows, not(debug_assertions)))]
            fatal_error_box::inform(&e.to_string());
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("DayZ Community Hub: {e}");
            lenient(std::env::args_os())
        }
    }
}

/// What can be read of `argv` when it does not parse cleanly: each argument clap
/// names as the trouble (an unknown flag, a value it cannot read, and that value) is
/// left out in turn until the rest parses; else none at all.
fn lenient(argv: impl IntoIterator<Item = OsString>) -> Args {
    use clap::error::{ContextKind, ContextValue};
    let mut argv: Vec<OsString> = argv.into_iter().collect();
    for _ in 0..16 {
        let e = match Args::try_parse_from(&argv) {
            Ok(a) => return a,
            Err(e) => e,
        };
        let said = |k| match e.get(k) {
            Some(ContextValue::String(s)) => Some(s.clone()),
            _ => None,
        };
        // "--control <PORT>" names the flag "--control".
        let flag = said(ContextKind::InvalidArg)
            .and_then(|s| s.split([' ', '=']).next().map(str::to_owned));
        let value = said(ContextKind::InvalidValue);
        let before = argv.len();
        if let Some(v) = value
            && let Some(i) = argv.iter().skip(1).position(|a| a.to_str() == Some(&v))
        {
            argv.remove(i + 1);
        }
        if let Some(f) = flag {
            let with_value = format!("{f}=");
            if let Some(i) = argv.iter().skip(1).position(|a| {
                a.to_str()
                    .is_some_and(|a| a == f || a.starts_with(&with_value))
            }) {
                argv.remove(i + 1);
            }
        }
        if argv.len() == before {
            break;
        }
    }
    Args::try_parse_from(argv.first().cloned())
        .unwrap_or_else(|_| fail("cannot read the command line"))
}

/// The arguments a restart starts the next instance with: the window's own, hidden
/// flags (the same world, the same control port), never what asked something of
/// this one (a link, a `.dzch` file, `--connect`, `--reconnect`).
fn restart_args(a: &Args) -> Vec<OsString> {
    let mut v = Vec::new();
    if let Some(d) = &a.data_dir {
        v.push("--data-dir".into());
        v.push(std::path::absolute(d).unwrap_or_else(|_| d.clone()).into());
    }
    if let Some(n) = &a.instance {
        v.push("--instance".into());
        v.push(n.into());
    }
    if a.mock {
        v.push("--mock".into());
    }
    if let Some(p) = a.control {
        v.push("--control".into());
        v.push(p.to_string().into());
    }
    v
}

/// One way to get a graphics device: the backends asked for, and whether a software
/// (CPU) adapter will do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Gpu {
    backends: wgpu::Backends,
    software: bool,
}

/// The backends tried, in order: the platform's own first (Vulkan on Linux, Direct3D
/// 12 on Windows), enumerating only it, then once the rest when it gives no device
/// (an old or broken driver). `WGPU_BACKEND` names the only one tried.
fn gpu_plan() -> Vec<Gpu> {
    if let Some(backends) = wgpu::Backends::from_env() {
        return vec![Gpu {
            backends,
            software: true,
        }];
    }
    let (first, then) = if cfg!(windows) {
        (
            wgpu::Backends::DX12,
            wgpu::Backends::VULKAN | wgpu::Backends::GL,
        )
    } else if cfg!(target_os = "linux") {
        (wgpu::Backends::VULKAN, wgpu::Backends::GL)
    } else {
        (wgpu::Backends::PRIMARY, wgpu::Backends::GL)
    };
    vec![
        Gpu {
            backends: first,
            software: false,
        },
        Gpu {
            backends: then,
            software: true,
        },
    ]
}

/// The device the window draws with: the power-saving GPU first (a 2D launcher has
/// no use for the discrete one, which then stays asleep, or free for DayZ, on hybrid
/// laptops); `WGPU_POWER_PREF=high` asks for the other.
fn wgpu_options(gpu: Gpu) -> egui_wgpu::WgpuConfiguration {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = gpu.backends;
    let power = wgpu::PowerPreference::from_env().unwrap_or(wgpu::PowerPreference::LowPower);
    setup.power_preference = power;
    let software = gpu.software;
    setup.native_adapter_selector = Some(Arc::new(move |adapters, surface| {
        adapters
            .iter()
            .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
            .filter_map(|a| adapter_rank(a.get_info().device_type, power, software).map(|r| (r, a)))
            .min_by_key(|(r, _)| *r)
            .map(|(_, a)| a.clone())
            .ok_or_else(|| "no graphics adapter can draw this window".to_owned())
    }));
    egui_wgpu::WgpuConfiguration {
        wgpu_setup: egui_wgpu::WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}

/// How much an adapter of this kind is wanted (lower first); `None` leaves it out.
/// A software adapter (lavapipe, WARP) only when nothing else is left: on the
/// primary backend it is refused, so the fallback's hardware OpenGL wins over it.
fn adapter_rank(
    kind: wgpu::DeviceType,
    power: wgpu::PowerPreference,
    software: bool,
) -> Option<u8> {
    use wgpu::DeviceType as D;
    let high = power == wgpu::PowerPreference::HighPerformance;
    Some(match kind {
        D::IntegratedGpu if high => 1,
        D::IntegratedGpu => 0,
        D::DiscreteGpu if high => 0,
        D::DiscreteGpu => 1,
        D::VirtualGpu => 2,
        D::Other => 3,
        D::Cpu if software => 4,
        D::Cpu => return None,
    })
}

/// Say why the launcher cannot start, and stop.
fn fail(msg: &str) -> ! {
    report(msg);
    std::process::exit(1);
}

/// Say why the launcher cannot start: on stderr, and in a message box on Windows,
/// whose release build has no console.
fn report(msg: &str) {
    eprintln!("DayZ Community Hub: {msg}");
    #[cfg(all(windows, not(debug_assertions)))]
    fatal_error_box::show(&format!("DayZ Community Hub cannot start: {msg}"));
}

/// A release build on Windows has no console: a panic or a start-up failure would
/// close the launcher without a word. Say what happened in a message box first. (A
/// release build aborts on panic, so every panic there is the end.)
#[cfg(all(windows, not(debug_assertions)))]
mod fatal_error_box {
    use std::ffi::c_void;

    const MB_OK: u32 = 0x0;
    const MB_ICONERROR: u32 = 0x10;
    const MB_ICONINFORMATION: u32 = 0x40;
    const MB_TOPMOST: u32 = 0x0004_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(hwnd: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `text`, and what to do when it is the graphics card that failed.
    pub fn show(text: &str) {
        let mut text = text.to_owned();
        let lower = text.to_ascii_lowercase();
        if lower.contains("wgpu") || lower.contains("adapter") || lower.contains("surface") {
            text.push_str(
                "\n\nThe launcher draws its window with the graphics card (Direct3D 12, \
                 Vulkan or OpenGL). Update the graphics driver, then start the launcher \
                 again.",
            );
        }
        message(&text, MB_ICONERROR);
    }

    /// `text` as information (`--help`, `--version`).
    pub fn inform(text: &str) {
        message(text, MB_ICONINFORMATION);
    }

    fn message(text: &str, icon: u32) {
        let (text, caption) = (wide(text), wide("DayZ Community Hub"));
        // SAFETY: both strings are NUL-terminated UTF-16; no owner window.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | icon | MB_TOPMOST,
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
            show(&text);
        }));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<OsString> {
        std::iter::once("dayz-community-hub")
            .chain(a.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn a_restart_keeps_the_world_and_drops_what_was_asked() {
        let a = Args::try_parse_from(argv(&[
            "--reconnect",
            "--data-dir",
            "/tmp/w",
            "--instance",
            "t",
            "--mock",
            "--control",
            "4000",
            "dzch://1.2.3.4:2302",
        ]))
        .unwrap();
        let got: Vec<String> = restart_args(&a)
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            got,
            [
                "--data-dir",
                "/tmp/w",
                "--instance",
                "t",
                "--mock",
                "--control",
                "4000"
            ]
        );
        let plain = Args::try_parse_from(argv(&["--connect", "1.2.3.4:2302"])).unwrap();
        assert!(restart_args(&plain).is_empty());
    }

    #[test]
    fn an_unknown_argument_does_not_stop_the_window() {
        assert!(Args::try_parse_from(argv(&["--bogus", "--mock"])).is_err());
        let a = lenient(argv(&[
            "--bogus",
            "--mock",
            "--control",
            "x",
            "--data-dir=/w",
        ]));
        assert!(a.mock && a.control.is_none());
        assert_eq!(a.data_dir.as_deref(), Some(std::path::Path::new("/w")));
        let b = lenient(argv(&["--connect", "1.2.3.4:2302", "a.dzch", "b.dzch"]));
        assert_eq!(b.cli.open.as_deref(), Some("a.dzch"));
    }

    #[test]
    fn the_power_saving_gpu_comes_first_and_software_last() {
        use wgpu::{DeviceType as D, PowerPreference as P};
        let low = |k| adapter_rank(k, P::LowPower, false);
        assert!(low(D::IntegratedGpu) < low(D::DiscreteGpu));
        assert_eq!(low(D::Cpu), None);
        let high = |k| adapter_rank(k, P::HighPerformance, true);
        assert!(high(D::DiscreteGpu) < high(D::IntegratedGpu));
        assert!(high(D::Cpu) > high(D::Other));
        assert!(gpu_plan().last().unwrap().software);
    }
}
