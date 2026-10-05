//! Controllers: a Steam Deck in Game Mode, an Xbox or PlayStation pad on
//! Windows or Linux. One thread reads them through gilrs (the webview's own
//! Gamepad API is unreliable in WebKitGTK) and turns them into a few actions
//! (`gamepad-input`) that the window's spatial navigation acts on.
//!
//! The thread sleeps in the platform's own wait (epoll on Linux) until a pad
//! does something, or until a held direction is due to repeat: with no pad,
//! it costs nothing. Nothing is sent while the window is not focused. If the
//! pads cannot be opened (no udev access, say), it says so once and the app
//! runs without them.

pub(crate) mod map;

use gilrs::{Axis, Button, EventType, GamepadId, Gilrs};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_specta::Event;

use map::{Btn, PadState};
pub use map::{PadAction, PadKind};

/// A pad asked the window to do something.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct GamepadInput {
    pub action: PadAction,
    /// Fired again because the button or stick is still held.
    pub repeat: bool,
}

/// A connected pad.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type)]
pub struct PadInfo {
    pub name: String,
    pub kind: PadKind,
}

/// The pads connected now; sent whenever one comes or goes.
#[derive(Serialize, Deserialize, Clone, Debug, specta::Type, Event)]
pub struct GamepadPads(pub Vec<PadInfo>);

/// What the window needs to know at start-up.
#[derive(Serialize, Clone, Debug, specta::Type)]
pub struct GamepadStatus {
    /// Pads could be opened at all.
    pub available: bool,
    pub pads: Vec<PadInfo>,
    /// Running inside Steam's big-screen interface, a Steam Deck's Game Mode
    /// or Big Picture, where Steam's on-screen keyboard is there to type with.
    pub steam_ui: bool,
}

/// The main window has the focus. Starts true: a compositor that never says
/// (gamescope has been known not to) must not leave the pad dead.
static FOCUSED: AtomicBool = AtomicBool::new(true);

fn shared() -> &'static Mutex<(bool, Vec<PadInfo>)> {
    static S: OnceLock<Mutex<(bool, Vec<PadInfo>)>> = OnceLock::new();
    S.get_or_init(|| Mutex::new((false, Vec::new())))
}

/// The main window gained or lost the focus.
pub(crate) fn set_focused(focused: bool) {
    FOCUSED.store(focused, Ordering::Relaxed);
}

/// Steam's big-screen interface sets these in the environment of what it runs.
fn in_steam_ui() -> bool {
    ["SteamDeck", "SteamGamepadUI", "SteamTenfoot", "SteamOS"]
        .iter()
        .any(|k| std::env::var(k).is_ok_and(|v| !v.is_empty() && v != "0"))
}

/// The pads, and whether a controller UI is to be expected.
#[tauri::command]
#[specta::specta]
pub(crate) fn gamepad_status() -> GamepadStatus {
    let s = shared().lock().unwrap_or_else(|e| e.into_inner());
    GamepadStatus {
        available: s.0,
        pads: s.1.clone(),
        steam_ui: in_steam_ui(),
    }
}

fn btn(b: Button) -> Option<Btn> {
    Some(match b {
        Button::South => Btn::South,
        Button::East => Btn::East,
        Button::West => Btn::West,
        Button::North => Btn::North,
        Button::Start => Btn::Start,
        Button::Select => Btn::Select,
        Button::LeftTrigger => Btn::Lb,
        Button::RightTrigger => Btn::Rb,
        Button::LeftTrigger2 => Btn::Lt,
        Button::RightTrigger2 => Btn::Rt,
        Button::DPadUp => Btn::DUp,
        Button::DPadDown => Btn::DDown,
        Button::DPadLeft => Btn::DLeft,
        Button::DPadRight => Btn::DRight,
        Button::LeftThumb => Btn::L3,
        Button::RightThumb => Btn::R3,
        _ => return None,
    })
}

/// A second pad acting within this long of another is taken for the same
/// hands on a mirrored device (Steam Input's virtual pad beside the real one)
/// and ignored.
const OTHER_PAD_QUIET: Duration = Duration::from_millis(150);

/// Start reading the pads, on a thread of its own.
pub(crate) fn spawn(app: AppHandle) {
    let spawned = std::thread::Builder::new()
        .name("gamepad".into())
        .spawn(move || run(app));
    if let Err(e) = spawned {
        eprintln!("[gamepad] thread not started: {e}");
    }
}

fn run(app: AppHandle) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        // A platform gilrs does not know: nothing to read.
        Err(gilrs::Error::NotImplemented(_)) => {
            eprintln!("[gamepad] controllers are not supported on this platform");
            return;
        }
        Err(e) => {
            eprintln!("[gamepad] controllers unavailable: {e}");
            return;
        }
    };
    let pads_of = |g: &Gilrs| -> Vec<PadInfo> {
        g.gamepads()
            .map(|(_, p)| PadInfo {
                name: p.name().to_string(),
                kind: map::kind_of(p.vendor_id(), p.name()),
            })
            .collect()
    };
    let pads = pads_of(&gilrs);
    *shared().lock().unwrap_or_else(|e| e.into_inner()) = (true, pads.clone());
    // For a window that asked `gamepad_status` before the pads were opened.
    let _ = GamepadPads(pads).emit_to(&app, "main");

    let mut states: HashMap<GamepadId, PadState> = HashMap::new();
    let mut last: Option<(GamepadId, Instant)> = None;
    let mut was_focused = true;

    let send = |a: PadAction, repeat: bool| {
        let _ = GamepadInput { action: a, repeat }.emit_to(&app, "main");
    };

    loop {
        let wait = states
            .values()
            .filter_map(PadState::deadline)
            .min()
            .map(|at| at.saturating_duration_since(Instant::now()));
        let ev = gilrs.next_event_blocking(wait);
        let now = Instant::now();

        let focused = FOCUSED.load(Ordering::Relaxed);
        if focused != was_focused {
            was_focused = focused;
            // Whatever was held when the focus left is not held on return.
            states.values_mut().for_each(PadState::release_all);
        }

        if let Some(ev) = ev {
            // Another pad spoke a moment ago: this one is its echo.
            let echo =
                |id: GamepadId| last.is_some_and(|(l, at)| l != id && now - at < OTHER_PAD_QUIET);
            let fired = match ev.event {
                EventType::Connected | EventType::Disconnected => {
                    states.remove(&ev.id);
                    let pads = pads_of(&gilrs);
                    shared().lock().unwrap_or_else(|e| e.into_inner()).1 = pads.clone();
                    let _ = GamepadPads(pads).emit_to(&app, "main");
                    None
                }
                _ if !focused => None,
                EventType::ButtonPressed(..) if echo(ev.id) => None,
                EventType::ButtonPressed(b, _) => {
                    btn(b).and_then(|b| states.entry(ev.id).or_default().button(b, true, now))
                }
                // A release always counts, so nothing stays held.
                EventType::ButtonReleased(b, _) => {
                    btn(b).and_then(|b| states.entry(ev.id).or_default().button(b, false, now))
                }
                EventType::AxisChanged(Axis::LeftStickX | Axis::LeftStickY, _, _) => {
                    let (x, y) = gilrs
                        .connected_gamepad(ev.id)
                        .map(|p| (p.value(Axis::LeftStickX), p.value(Axis::LeftStickY)))
                        .unwrap_or_default();
                    let st = states.entry(ev.id).or_default();
                    let fired = st.stick(x, y, now);
                    if fired.is_some() && echo(ev.id) {
                        st.release_all();
                        None
                    } else {
                        fired
                    }
                }
                EventType::AxisChanged(Axis::RightStickY, y, _) => {
                    let st = states.entry(ev.id).or_default();
                    let fired = st.right_stick(y, now);
                    if fired.is_some() && echo(ev.id) {
                        st.release_all();
                        None
                    } else {
                        fired
                    }
                }
                _ => None,
            };
            if let Some((a, repeat)) = fired {
                last = Some((ev.id, now));
                send(a, repeat);
            }
        }

        if focused {
            for (id, st) in &mut states {
                if let Some((a, repeat)) = st.tick(now) {
                    last = Some((*id, now));
                    send(a, repeat);
                }
            }
        }
    }
}
