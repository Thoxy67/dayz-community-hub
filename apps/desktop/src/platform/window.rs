//! The window as the system shows it: its icon, and where it was, how big and whether
//! maximised, from one start to the next.
//!
//! - **Geometry.** eframe's persistence saves the window (outer position in physical
//!   pixels, inner size, maximised) in `window.ron` beside the launcher's data (a
//!   temporary directory in mock mode), off the UI thread and only when it changed
//!   (`HubApp::auto_save_interval`, and on exit). At the next start eframe restores it
//!   before the window exists, with the size clamped to the largest monitor and, on
//!   Windows, the position moved onto a connected monitor (a window last seen on a
//!   monitor since unplugged would open where nobody can see it). [`sanitize`] first drops
//!   a file holding values that would hurt (not finite, absurd, or the `-32000` Windows
//!   gives a minimised window), after rewriting a file eframe 0.36 wrote in the format
//!   0.37 reads (eframe would ignore it), and [`restored`] corrects what eframe hands back: never
//!   full screen, at least the minimum size, and a maximised window gets back the size it
//!   had before it was maximised (kept in the preferences, `dzch.window`), so restoring
//!   it does not leave a screen-sized window. A minimised window is never saved
//!   (`HubApp::save` keeps the last value). Wayland compositors place windows
//!   themselves: there is no position to save.
//! - **Icon.** [`icon`] is the title-bar / taskbar / Alt+Tab icon (`WM_SETICON` on
//!   Windows, `_NET_WM_ICON` on X11); the Windows executable also carries `icon.ico`
//!   (`build.rs`), which Explorer and the registrations show.
//! - **Corners.** Rounded while the window stands free (`HubApp::draw`). On Linux the
//!   window is transparent and draws its own corners, once the surface it got can show
//!   the desktop through ([`note_alpha_modes`]; else it stays square, not black-cornered).
//!   On Windows 11 the system rounds them ([`round_corners`]), the window opaque.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{IconData, ViewportBuilder};

use crate::prefs::WindowState;

/// eframe's file, in the data directory.
const FILE_NAME: &str = "window.ron";
/// eframe's key for the window in that file.
const WINDOW_KEY: &str = "window";
/// Coordinates beyond this (pixels or points) are corrupt.
const MAX_COORD: f64 = 100_000.0;
/// Windows parks a minimised window at (-32000, -32000); anything this far out is that.
const MINIMISED_POS: f64 = 30_000.0;

/// The 256 px app icon (the Tauri app's `icons/128x128@2x.png`).
pub(crate) const PNG: &[u8] = include_bytes!("../../assets/app/icon-256.png");

/// The window's icon (an empty one if the bundled PNG cannot be read, which a test
/// rules out).
pub fn icon() -> IconData {
    eframe::icon_data::from_png_bytes(PNG).unwrap_or_default()
}

/// The window's surface blends with the desktop (`note_alpha_modes`).
static SEE_THROUGH: AtomicBool = AtomicBool::new(false);

/// Whether the window draws on a surface that shows the desktop through it: only then
/// may it round its own corners.
pub fn see_through() -> bool {
    SEE_THROUGH.load(Ordering::Relaxed)
}

/// Record whether the window's surface can blend with the desktop, from the alpha modes
/// it offers (eframe picks a blending one for a transparent window when there is one).
pub fn note_alpha_modes(modes: &[eframe::wgpu::CompositeAlphaMode]) {
    use eframe::wgpu::CompositeAlphaMode as M;
    let blends = modes
        .iter()
        .any(|m| matches!(m, M::PreMultiplied | M::PostMultiplied));
    SEE_THROUGH.store(blends, Ordering::Relaxed);
}

/// Ask Windows 11 to round the window's corners, or not (`DWMWA_WINDOW_CORNER_PREFERENCE`;
/// Windows 10 has no such thing and says no, which changes nothing). The system squares
/// a maximised window by itself.
#[cfg(windows)]
pub fn round_corners(frame: &eframe::Frame, on: bool) {
    use eframe::wgpu::rwh::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND, DwmSetWindowAttribute,
    };
    let Ok(h) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(w) = h.as_raw() else {
        return;
    };
    let pref = if on { DWMWCP_ROUND } else { DWMWCP_DONOTROUND };
    // SAFETY: `w.hwnd` is the live window's handle; the attribute is a 4-byte enum read
    // from `pref` during the call.
    unsafe {
        DwmSetWindowAttribute(
            w.hwnd.get() as _,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            std::ptr::from_ref(&pref).cast(),
            std::mem::size_of_val(&pref) as u32,
        );
    }
}

/// `window.ron` in `dir`.
pub fn path_in(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Rewrite the file at `path` if eframe 0.36 wrote it, then remove it if its window
/// would be wrong to restore. `saved` is the preferences' geometry (the size before
/// maximising). Blocking: call it before the window opens. Never fails (an error is
/// printed).
pub fn sanitize(path: &Path, saved: Option<WindowState>) {
    let Ok(mut text) = std::fs::read_to_string(path) else {
        return;
    };
    if let Some(new) = migrate(&text, saved, cfg!(windows)) {
        match std::fs::write(path, &new) {
            Ok(()) => text = new,
            Err(e) => eprintln!("window: cannot rewrite {}: {e}", path.display()),
        }
    }
    if let Err(why) = check(&text) {
        eprintln!("window: ignoring {}: {why}", path.display());
        if let Err(e) = std::fs::remove_file(path) {
            eprintln!("window: cannot remove {}: {e}", path.display());
        }
    }
}

/// Whether the file at `path` holds a position eframe will restore; then the window
/// must not also be centred (eframe centres after restoring).
pub fn has_saved_position(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| window(&t))
        .and_then(|w| field(&w, GEOMETRY).cloned())
        .and_then(|g| field(&g, "outer_position_pixels").cloned())
        .is_some_and(|p| matches!(p, ron::Value::Option(Some(_))))
}

/// The window eframe restores, corrected before it is created (eframe's
/// `NativeOptions::window_builder`). `saved` is the preferences' geometry;
/// `fullscreen` is set in Steam's Game Mode, where the window always takes the screen
/// (eframe's saved `fullscreen: false` must not undo it).
pub fn restored(
    builder: ViewportBuilder,
    saved: Option<WindowState>,
    fullscreen: bool,
) -> ViewportBuilder {
    let mut b = builder.with_fullscreen(fullscreen);
    // Saved maximised, eframe's size is the maximised one: restoring would leave a window
    // as big as the screen. The size before maximising, if smaller (eframe's is already
    // clamped to the monitor).
    if b.maximized == Some(true)
        && let (Some(size), Some(w)) = (b.inner_size, saved)
    {
        let before = egui::vec2(w.size[0], w.size[1]);
        if before.x.is_finite() && before.y.is_finite() {
            b.inner_size = Some(before.min(size));
        }
    }
    // Windows saves a minimised window's size (eframe clamps it to 64 pt); the minimum
    // size is not applied to a restored one. Grow it about its centre.
    if let (Some(size), Some(min)) = (b.inner_size, b.min_inner_size) {
        let grown = size.max(min);
        if grown != size {
            b.position = b.position.map(|p| p - (grown - size) / 2.0);
            b.inner_size = Some(grown);
        }
    }
    b
}

/// The window entry of eframe's file, if it has one that parses.
fn window(text: &str) -> Option<ron::Value> {
    let kv = ron::from_str::<HashMap<String, String>>(text).ok()?;
    ron::from_str::<ron::Value>(kv.get(WINDOW_KEY)?).ok()
}

/// eframe's (0.37) key for the window's position and size when neither maximised nor
/// full screen: `(outer_position_pixels: Option<Pos2>, scale_factor, inner_size_points:
/// Vec2)`.
const GEOMETRY: &str = "normal_geometry";

/// eframe 0.36's window entry, `(inner_position_pixels, outer_position_pixels,
/// fullscreen, maximized, inner_size_points)` (the last three optional), in 0.37's
/// format: the whole file again, or `None` when there is nothing to rewrite (already
/// 0.37's, unreadable, no window). 0.37 restores a position as pixels over the saved
/// scale factor, which 0.36 did not save: the position is kept only on Windows, where
/// eframe then places the window in pixels whatever the factor; elsewhere the window
/// opens centred once. A maximised window's size was the screen's: the preferences'
/// size before maximising is the one to come back to.
fn migrate(text: &str, saved: Option<WindowState>, keep_position: bool) -> Option<String> {
    let mut kv = ron::from_str::<HashMap<String, String>>(text).ok()?;
    let w = ron::from_str::<ron::Value>(kv.get(WINDOW_KEY)?).ok()?;
    if field(&w, GEOMETRY).is_some() {
        return None;
    }
    let pair = |key: &str| match field(&w, key) {
        Some(ron::Value::Option(Some(v))) => Some((
            field(v, "x").and_then(as_f64)?,
            field(v, "y").and_then(as_f64)?,
        )),
        _ => None,
    };
    let flag = |key: &str| matches!(field(&w, key), Some(ron::Value::Bool(true)));
    let maximized = flag("maximized");
    let mut size = pair("inner_size_points");
    if maximized && let (Some((sw, sh)), Some(before)) = (size, saved) {
        let (bw, bh) = (f64::from(before.size[0]), f64::from(before.size[1]));
        if bw.is_finite() && bh.is_finite() && bw >= 1.0 && bh >= 1.0 {
            size = Some((bw.min(sw), bh.min(sh)));
        }
    }
    let entry = match size {
        // Nothing eframe 0.37 can restore: no entry, the window opens at the default.
        None => None,
        Some((sw, sh)) => {
            let pos = match pair("outer_position_pixels").filter(|_| keep_position) {
                Some((x, y)) => format!("Some((x:{x:?},y:{y:?}))"),
                None => "None".to_owned(),
            };
            Some(format!(
                "({GEOMETRY}:(outer_position_pixels:{pos},scale_factor:1.0,inner_size_points:(x:{sw:?},y:{sh:?})),fullscreen:{},maximized:{maximized})",
                flag("fullscreen")
            ))
        }
    };
    match entry {
        Some(e) => kv.insert(WINDOW_KEY.to_owned(), e),
        None => kv.remove(WINDOW_KEY),
    };
    ron::to_string(&kv).ok()
}

/// `Err` says why the saved window must not be restored. A file eframe cannot read
/// (not RON, no window) is fine: eframe ignores it.
fn check(text: &str) -> Result<(), String> {
    let Some(w) = window(text) else {
        return Ok(());
    };
    let Some(g) = field(&w, GEOMETRY) else {
        return Ok(());
    };
    if let Some(f) = field(g, "scale_factor").and_then(as_f64)
        && !(f.is_finite() && f > 0.0 && f <= 16.0)
    {
        return Err(format!("scale_factor = {f}"));
    }
    for key in ["outer_position_pixels", "inner_size_points"] {
        let v = match field(g, key) {
            Some(ron::Value::Option(Some(v))) => &**v,
            Some(v @ ron::Value::Map(_)) => v,
            _ => continue,
        };
        for axis in ["x", "y"] {
            let f = field(v, axis).and_then(as_f64);
            match f {
                Some(f) if !f.is_finite() || f.abs() > MAX_COORD => {
                    return Err(format!("{key}.{axis} = {f}"));
                }
                Some(f) if key != "inner_size_points" && f <= -MINIMISED_POS => {
                    return Err(format!("{key}: saved minimised"));
                }
                Some(f) if key == "inner_size_points" && f < 1.0 => {
                    return Err(format!("{key}.{axis} = {f}"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn as_f64(v: &ron::Value) -> Option<f64> {
    match v {
        ron::Value::Number(n) => Some(n.into_f64()),
        _ => None,
    }
}

fn field<'a>(v: &'a ron::Value, name: &str) -> Option<&'a ron::Value> {
    match v {
        ron::Value::Map(m) => m.get(&ron::Value::String(name.into())),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// eframe's file as it writes it: a map of key → RON string.
    fn file(window: &str) -> String {
        format!("{{\"window\":{window:?}}}")
    }

    /// eframe 0.37's window entry.
    const WINDOW: &str = "(normal_geometry:(outer_position_pixels:Some((x:100.0,y:100.0)),scale_factor:1.0,inner_size_points:(x:1200.0,y:780.0)),fullscreen:false,maximized:false)";
    /// eframe 0.36's.
    const OLD_WINDOW: &str = "(inner_position_pixels:Some((x:101.0,y:132.0)),outer_position_pixels:Some((x:100.0,y:100.0)),fullscreen:false,maximized:false,inner_size_points:Some((x:1200.0,y:780.0)))";

    #[test]
    fn the_icon_decodes() {
        let i = icon();
        assert_eq!((i.width, i.height), (256, 256));
        assert_eq!(i.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn a_saved_window_is_kept() {
        assert_eq!(check(&file(WINDOW)), Ok(()));
        assert_eq!(check("not ron at all"), Ok(()));
        assert_eq!(check("{}"), Ok(()));
        // A second monitor left of the main one: negative but real.
        assert_eq!(
            check(&file(&WINDOW.replace("x:100.0", "x:-1820.0"))),
            Ok(())
        );
    }

    #[test]
    fn minimised_or_corrupt_windows_are_dropped() {
        let minimised = WINDOW.replace("(x:100.0,y:100.0)", "(x:-32000.0,y:-32000.0)");
        assert!(check(&file(&minimised)).is_err());
        let zero_scale = WINDOW.replace("scale_factor:1.0", "scale_factor:0.0");
        assert!(check(&file(&zero_scale)).is_err());
        for size in ["(x:NaN,y:780.0)", "(x:1200.0,y:1e30)", "(x:0.0,y:0.0)"] {
            let w = WINDOW.replace("(x:1200.0,y:780.0)", size);
            assert!(check(&file(&w)).is_err(), "{size}");
        }
    }

    #[test]
    fn sanitize_removes_only_a_bad_file() {
        let dir = std::env::temp_dir().join(format!("dzch-window-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = path_in(&dir);
        std::fs::write(&path, file(WINDOW)).unwrap();
        sanitize(&path, None);
        assert!(path.exists());
        assert!(has_saved_position(&path));
        std::fs::write(&path, file(&WINDOW.replace("x:1200.0", "x:inf"))).unwrap();
        sanitize(&path, None);
        assert!(!path.exists());
        assert!(!has_saved_position(&path));
        // Wayland: no position saved, so the window is centred.
        std::fs::write(
            &path,
            file("(normal_geometry:(outer_position_pixels:None,scale_factor:1.0,inner_size_points:(x:1200.0,y:780.0)),fullscreen:false,maximized:false)"),
        )
        .unwrap();
        assert!(!has_saved_position(&path));
        // eframe 0.36's file: rewritten in place, then kept.
        std::fs::write(&path, file(OLD_WINDOW)).unwrap();
        sanitize(&path, None);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            window(&text)
                .and_then(|w| field(&w, GEOMETRY).cloned())
                .is_some()
        );
        assert_eq!(has_saved_position(&path), cfg!(windows));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The entry rewritten, as eframe 0.37 deserialises it.
    fn migrated(old: &str, saved: Option<WindowState>, keep_position: bool) -> ron::Value {
        let text = migrate(&file(old), saved, keep_position).unwrap();
        window(&text).unwrap()
    }

    fn geometry(w: &ron::Value, key: &str) -> Option<(f64, f64)> {
        let g = field(w, GEOMETRY)?;
        let v = match field(g, key)? {
            ron::Value::Option(Some(v)) => &**v,
            v => v,
        };
        Some((
            field(v, "x").and_then(as_f64)?,
            field(v, "y").and_then(as_f64)?,
        ))
    }

    #[test]
    fn an_eframe_0_36_file_is_rewritten_for_0_37() {
        // The new entry deserialises as egui-winit's own type.
        let text = migrate(&file(OLD_WINDOW), None, true).unwrap();
        let kv = ron::from_str::<HashMap<String, String>>(&text).unwrap();
        let typed: egui_winit_window::WindowSettings = ron::from_str(&kv[WINDOW_KEY]).unwrap();
        assert!(!typed.maximized);
        let w = migrated(OLD_WINDOW, None, true);
        assert_eq!(geometry(&w, "outer_position_pixels"), Some((100.0, 100.0)));
        assert_eq!(geometry(&w, "inner_size_points"), Some((1200.0, 780.0)));
        assert_eq!(check(&file(&ron::to_string(&w).unwrap())), Ok(()));
        // Off Windows the position goes (no scale factor to read it with).
        let w = migrated(OLD_WINDOW, None, false);
        assert_eq!(geometry(&w, "outer_position_pixels"), None);
        // Maximised: the size before maximising, within the saved one.
        let max = OLD_WINDOW.replace("maximized:false", "maximized:true");
        let before = Some(WindowState {
            pos: None,
            size: [1100.0, 700.0],
            maximized: true,
        });
        let w = migrated(&max, before, true);
        assert_eq!(geometry(&w, "inner_size_points"), Some((1100.0, 700.0)));
        assert_eq!(field(&w, "maximized"), Some(&ron::Value::Bool(true)));
        // Already 0.37's, or nothing readable: left alone.
        assert_eq!(migrate(&file(WINDOW), None, true), None);
        assert_eq!(migrate("not ron", None, true), None);
        // No size: the entry goes.
        let text = migrate(&file("(fullscreen:false,maximized:false)"), None, true).unwrap();
        assert!(window(&text).is_none());
    }

    /// egui-winit 0.37's `WindowSettings`, mirrored with its serde names, to prove the
    /// rewritten entry is what eframe reads.
    mod egui_winit_window {
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        pub struct Geometry {
            pub outer_position_pixels: Option<egui::Pos2>,
            pub scale_factor: f32,
            pub inner_size_points: egui::Vec2,
        }
        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        pub struct WindowSettings {
            pub normal_geometry: Geometry,
            pub fullscreen: bool,
            pub maximized: bool,
        }
    }

    #[test]
    fn restored_corrects_what_eframe_hands_back() {
        let saved = Some(WindowState {
            pos: None,
            size: [1100.0, 700.0],
            maximized: true,
        });
        let base = ViewportBuilder::default().with_min_inner_size([900.0, 600.0]);
        // Maximised: the size before maximising, within eframe's.
        let b = restored(
            base.clone()
                .with_inner_size([1920.0, 1040.0])
                .with_maximized(true)
                .with_fullscreen(true),
            saved,
            false,
        );
        assert_eq!(b.inner_size, Some(egui::vec2(1100.0, 700.0)));
        assert_eq!(b.fullscreen, Some(false));
        // A strip saved while minimised grows to the minimum about its centre.
        let b = restored(
            base.clone()
                .with_inner_size([160.0, 64.0])
                .with_position([500.0, 400.0]),
            None,
            false,
        );
        assert_eq!(b.inner_size, Some(egui::vec2(900.0, 600.0)));
        assert_eq!(b.position, Some(egui::pos2(130.0, 132.0)));
        // A normal window is left as it was.
        let b = restored(base.clone().with_inner_size([1200.0, 780.0]), saved, false);
        assert_eq!(b.inner_size, Some(egui::vec2(1200.0, 780.0)));
        // Game Mode: full screen survives eframe's saved `fullscreen: false`.
        let b = restored(
            base.with_inner_size([1200.0, 780.0]).with_fullscreen(false),
            saved,
            true,
        );
        assert_eq!(b.fullscreen, Some(true));
    }
}
