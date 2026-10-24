//! Headless screenshots of the whole window on the MOCK hub, rendered offscreen with wgpu
//! (works with a locked screen, in CI, over SSH). It drives the same [`HubApp::ui`] as
//! the real window, on a real tokio runtime: the mock's timers run in real time.
//!
//! `cargo run -p dayz-community-hub --example ui_shot -- script.jsonl`
//!
//! The script is one JSON object per line, run in order; blank lines and lines starting
//! with `//` are skipped. The format is `xtask/src/shots.rs`'s, plus a few lines of its own:
//!
//! | line | effect |
//! |---|---|
//! | `{"size": [w, h]}` | the window's inner size in points (read before anything is drawn) |
//! | `{"ppp": 1.0}` | pixels per point (default 1: a shot is `w × h` pixels) |
//! | `{"locale": "fr"}` | the interface language (en fr de es ru) |
//! | `{"theme": "chernarus"}` | a theme: a preset id, a custom theme's id, or `system` |
//! | `{"view": "servers"}` | open a view by its id (`"focus"` optional) |
//! | `{"command": "id", "args": {…}}` | run a command of the registry (`commands.rs`) |
//! | `{"steps": n}` | settle (below), then run `n` frames |
//! | `{"wait": ms}` | run frames for `ms` of real time |
//! | `{"click": "label"}` | click the widget with that accessible label (AccessKit) |
//! | `{"type": "cher"}` | type text into what has the keys |
//! | `{"key": "ArrowDown"}` | press a key (egui's name: `Enter`, `Escape`, `Space`…) |
//! | `{"pad": "dpad_down"}` | a controller button, as if a pad sent it (`"repeat": true` for a held one): `dpad_up/down/left/right`, `a`, `b`, `x`, `y`, `lb`, `rb`, `lt`, `rt`, `menu`, `view`, `l3`, `r3`, `rs_up`, `rs_down` (or the action's name: `down`, `accept`, `prevTab`…) |
//! | `{"method": "ui.…", "params": {…}}` | a control-channel request (`docs/control-protocol.md`) |
//! | `{"shot": "/abs/path.png"}` | settle, then save the frame as a PNG |
//!
//! **Settling.** Before `steps` and `shot`, frames run (with real time passing) until the
//! window has started (the mock's list, profile and mods are in, or start-up failed) and
//! then until it is quiet: no frame asked for soon, no store loading. Bounded: 30 s for
//! start-up, 4 s for quiet (a spinner on screen repaints forever).
//!
//! Exit status: 0 when every line ran, 1 when a line failed (the rest still run), 2 for a
//! script that cannot be read.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::Path;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use dz_hub::{CliArgs, Hub, HubConfig, HubMode};
use dzch_ui::HubApp;
use dzch_ui::app::EventPipe;
use dzch_ui::control::{self, Control, ControlRequest};
use dzch_ui::i18n::Locale;
use dzch_ui::prefs::Prefs;
use dzch_ui::views::ViewId;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::Value;

type H = Harness<'static, Option<HubApp>>;

/// Real time between frames: the hub works meanwhile.
const FRAME_SLEEP: Duration = Duration::from_millis(8);
const BOOT_LIMIT: Duration = Duration::from_secs(30);
const QUIET_LIMIT: Duration = Duration::from_secs(4);

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: ui_shot script.jsonl");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::from(2);
        }
    };
    let mut lines = Vec::new();
    for (n, l) in text.lines().enumerate() {
        let l = l.trim();
        if l.is_empty() || l.starts_with("//") {
            continue;
        }
        match serde_json::from_str::<Value>(l) {
            Ok(v) if v.is_object() => lines.push(v),
            Ok(_) => {
                eprintln!("{path}:{}: not a JSON object", n + 1);
                return ExitCode::from(2);
            }
            Err(e) => {
                eprintln!("{path}:{}: {e}", n + 1);
                return ExitCode::from(2);
            }
        }
    }
    let size = lines
        .iter()
        .find_map(|l| {
            let a = l.get("size")?.as_array()?;
            Some([a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32])
        })
        .unwrap_or([1400.0, 860.0])
        .map(|v| {
            if v.is_finite() {
                v.clamp(200.0, 8192.0)
            } else {
                800.0
            }
        });
    let ppp = lines
        .iter()
        .find_map(|l| l.get("ppp")?.as_f64())
        .map_or(1.0, |v| v as f32)
        .clamp(0.25, 4.0);

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("cannot start the async runtime: {e}");
            return ExitCode::from(2);
        }
    };
    let (control_tx, mut h) = match window(&runtime, size, ppp) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // The first frame makes the app; its fonts and icons take effect on the next ones.
    step_n(&mut h, 4);

    let mut failed = false;
    for l in &lines {
        if let Err(e) = run_line(&mut h, &control_tx, l) {
            eprintln!("ui_shot: {l}: {e}");
            failed = true;
        }
    }
    runtime.shutdown_background();
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// The window on a fresh mock hub, with a control channel fed by the script.
fn window(
    runtime: &tokio::runtime::Runtime,
    size: [f32; 2],
    ppp: f32,
) -> Result<(mpsc::Sender<ControlRequest>, H), String> {
    let pipe = EventPipe::new();
    let config = HubConfig {
        mode: HubMode::Mock,
        data_dir: None,
        args: CliArgs::none(),
    };
    let hub = Hub::new(config, pipe.sink.clone())?;
    {
        let _g = runtime.enter();
        hub.start();
    }
    let (tx, rx) = mpsc::channel::<ControlRequest>();
    let handle = runtime.handle().clone();
    let EventPipe {
        rx: events, wake, ..
    } = pipe;
    let mut parts = Some((hub, events, handle, wake, rx));
    let h = Harness::builder()
        .with_size(egui::vec2(size[0], size[1]))
        .with_pixels_per_point(ppp)
        .with_max_steps(1_000_000)
        .wgpu()
        .build_ui_state(
            move |ui, app: &mut Option<HubApp>| match app {
                Some(a) => a.ui(ui),
                None => {
                    if let Some((hub, events, handle, wake, rx)) = parts.take() {
                        dzch_ui::rt::Rt::install(handle, ui.ctx().clone());
                        let _ = wake.set(ui.ctx().clone());
                        // The user's preferences stay untouched: defaults, kept in memory.
                        let mut a = HubApp::new(ui.ctx(), hub, events, Prefs::default());
                        a.set_control(Control::new(rx, None));
                        *app = Some(a);
                    }
                }
            },
            None,
        );
    Ok((tx, h))
}

fn run_line(h: &mut H, tx: &mpsc::Sender<ControlRequest>, l: &Value) -> Result<(), String> {
    let s = |k: &str| l.get(k).and_then(Value::as_str);
    if l.get("size").is_some() || l.get("ppp").is_some() {
        // Read before the window was made.
        return Ok(());
    }
    if let Some(code) = s("locale") {
        let loc = Locale::from_code(code).ok_or_else(|| format!("unknown locale `{code}`"))?;
        with_ctx(h, |cx| cx.set_locale(loc))?;
        step_n(h, 2);
        return Ok(());
    }
    if let Some(id) = s("theme") {
        let r = with_ctx(h, |cx| control::set_theme(cx, id))?;
        step_n(h, 2);
        return reply_ok(&r);
    }
    if let Some(id) = s("view") {
        let v = ViewId::from_id(id).ok_or_else(|| format!("unknown view `{id}`"))?;
        let focus = s("focus").map(str::to_string);
        with_ctx(h, |cx| match focus {
            Some(f) => cx.go_focus(v, f),
            None => cx.go(v),
        })?;
        step_n(h, 2);
        return Ok(());
    }
    if let Some(id) = s("command") {
        let args = l.get("args").cloned().unwrap_or(Value::Null);
        with_ctx(h, |cx| cx.run(id, &args))??;
        step_n(h, 2);
        return Ok(());
    }
    if let Some(n) = l.get("steps").and_then(Value::as_u64) {
        settle(h);
        step_n(h, usize::try_from(n.min(100_000)).unwrap_or(0));
        return Ok(());
    }
    if let Some(ms) = l.get("wait").and_then(Value::as_u64) {
        let end = Instant::now() + Duration::from_millis(ms.min(600_000));
        while Instant::now() < end {
            step(h);
        }
        return Ok(());
    }
    if let Some(b) = s("pad") {
        let a = dzch_ui::pad::action_named(b).ok_or_else(|| format!("unknown pad button `{b}`"))?;
        let repeat = l.get("repeat").and_then(Value::as_bool).unwrap_or(false);
        with_ctx(h, |cx| {
            cx.stores.pad.inject(a, repeat);
            cx.egui.request_repaint();
        })?;
        step_n(h, 4);
        return Ok(());
    }
    if let Some(text) = s("type") {
        h.event(egui::Event::Text(text.to_string()));
        step_n(h, 3);
        return Ok(());
    }
    if let Some(name) = s("key") {
        let key = egui::Key::from_name(name).ok_or_else(|| format!("unknown key `{name}`"))?;
        // Into the next frame's input, where the window's raw input hook sees it.
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        step_n(h, 3);
        return Ok(());
    }
    if let Some(label) = s("click") {
        let node = h
            .query_all_by_label(label)
            .next()
            .ok_or_else(|| format!("no widget labelled `{label}`"))?;
        node.click();
        step_n(h, 3);
        return Ok(());
    }
    if let Some(m) = s("method") {
        let params = l
            .get("params")
            .cloned()
            .unwrap_or_else(|| Value::Object(Default::default()));
        let (req, mut reply) = ControlRequest::new(m, params);
        tx.send(req).map_err(|_| "the window is gone".to_string())?;
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            step(h);
            if let Ok(r) = reply.try_recv() {
                let text = r.to_string();
                let cut = if std::env::var_os("UI_SHOT_FULL").is_some() {
                    text.len()
                } else {
                    text.len().min(400)
                };
                println!("{m}: {}", text.get(..cut).unwrap_or(&text));
                step_n(h, 2);
                return reply_ok(&r);
            }
        }
        return Err("no reply in 10 s".into());
    }
    if let Some(p) = s("shot") {
        let p = Path::new(p);
        if !p.is_absolute() {
            return Err("`shot` must be an absolute path".into());
        }
        settle(h);
        step_n(h, 2);
        let img = h.render()?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        img.save(p).map_err(|e| format!("{}: {e}", p.display()))?;
        println!("{}", p.display());
        return Ok(());
    }
    Err("unknown line".into())
}

fn reply_ok(r: &Value) -> Result<(), String> {
    if r.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(())
    } else {
        Err(r
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("failed")
            .to_string())
    }
}

/// The app's context, between frames.
fn with_ctx<R>(h: &mut H, f: impl FnOnce(&mut dzch_ui::app::Ctx) -> R) -> Result<R, String> {
    let ctx = h.ctx.clone();
    let app = h
        .state_mut()
        .as_mut()
        .ok_or_else(|| "the window is not made yet".to_string())?;
    Ok(app.with_ctx(&ctx, f))
}

/// One frame, with the control channel's input, then a little real time.
fn step(h: &mut H) {
    let mut raw = std::mem::take(h.input_mut());
    let ctx = h.ctx.clone();
    if let Some(app) = h.state_mut().as_mut() {
        app.control_input(&mut raw);
        app.with_ctx(&ctx, |c| dzch_ui::pad::take_keys(c, &mut raw));
    }
    *h.input_mut() = raw;
    h.step();
    std::thread::sleep(FRAME_SLEEP);
}

fn step_n(h: &mut H, n: usize) {
    for _ in 0..n {
        step(h);
    }
}

/// Started: initialised (or failed), and the list, the profile and the mods are in.
fn booted(a: &HubApp) -> bool {
    let s = a.stores();
    if s.app.init_error.is_some() {
        return true;
    }
    s.app.initialized
        && (s.servers.total > 0 || s.servers.list_error.is_some())
        && s.profile.data.is_some()
        && s.mods.loaded
}

/// Nothing loading, and the last frame asked for no frame soon.
fn quiet(h: &H) -> bool {
    let idle = h
        .output()
        .viewport_output
        .values()
        .all(|v| v.repaint_delay >= Duration::from_millis(500));
    let loading = h.state().as_ref().is_none_or(|a| {
        let s = a.stores();
        s.servers.loading || s.mods.loading
    });
    idle && !loading
}

/// Run frames until the window has started, then until it is quiet (bounded).
fn settle(h: &mut H) {
    let end = Instant::now() + BOOT_LIMIT;
    while Instant::now() < end && !h.state().as_ref().is_some_and(booted) {
        step(h);
    }
    if !h.state().as_ref().is_some_and(booted) {
        eprintln!("ui_shot: the window did not finish starting in {BOOT_LIMIT:?}");
    }
    let end = Instant::now() + QUIET_LIMIT;
    let mut calm = 0;
    while Instant::now() < end && calm < 3 {
        step(h);
        calm = if quiet(h) { calm + 1 } else { 0 };
    }
}
