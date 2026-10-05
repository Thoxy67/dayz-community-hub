// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;

fn main() {
    // Started by the launcher to talk to Steam: does that, exits.
    dz_steamworks::serve_if_worker();
    #[cfg(all(windows, not(debug_assertions)))]
    fatal_error_box::install();
    let args = dayz_community_hub_lib::CliArgs::parse();
    dayz_community_hub_lib::run(args);
}

/// A release build on Windows has no console: a panic (the WebView2 runtime
/// missing or broken, most often) would close the launcher without a word.
/// Say what happened in a message box first. (A release build aborts on
/// panic, so every panic there is the end.)
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
            if what.to_ascii_lowercase().contains("webview") {
                text.push_str(
                    "\n\nThe launcher draws its window with Microsoft Edge WebView2. \
                     Install or repair the WebView2 Runtime (the \"Evergreen Standalone \
                     Installer\" from https://developer.microsoft.com/microsoft-edge/webview2/), \
                     then start the launcher again.",
                );
            }
            let (text, caption) = (wide(&text), wide("DayZ Community Hub"));
            // SAFETY: both strings are NUL-terminated UTF-16; no owner window.
            unsafe {
                MessageBoxW(
                    std::ptr::null_mut(),
                    text.as_ptr(),
                    caption.as_ptr(),
                    MB_OK | MB_ICONERROR | MB_TOPMOST,
                );
            }
        }));
    }
}
