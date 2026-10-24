//! System notifications (the launcher's own, while its window is not in front; when to
//! send one is decided in [`super::life`]).
//!
//! - Linux: `org.freedesktop.Notifications` over D-Bus through notify-rust's zbus
//!   backend (pure Rust, no libdbus), with the launcher's desktop entry and icon name.
//! - Windows: a WinRT toast through notify-rust. An unpackaged program has no
//!   AppUserModelID of its own registered, so the toast is shown under notify-rust's
//!   default one (PowerShell's), the only one Windows always accepts.
//!
//! Each notification is shown from its own short-lived thread: the D-Bus round trip
//! never touches the frame, and a failure is printed, never more.

/// What to tell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub body: String,
}

/// Show `note`, off the calling thread. Never fails: an error is printed.
pub fn send(note: Note) {
    let spawned = std::thread::Builder::new()
        .name("notify".into())
        .spawn(move || {
            if let Err(e) = show(&note) {
                eprintln!("notify: {e}");
            }
        });
    if let Err(e) = spawned {
        eprintln!("notify: {e}");
    }
}

#[cfg(target_os = "linux")]
fn show(note: &Note) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("DayZ Community Hub")
        .summary(&note.title)
        .body(&note.body)
        .icon("dayz-community-hub")
        .hint(notify_rust::Hint::DesktopEntry(desktop_entry_id()))
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The desktop entry the notification belongs to: a package's
/// `dayz-community-hub.desktop`, or from an AppImage the handler entry
/// `register.rs` writes (no other is there).
#[cfg(target_os = "linux")]
fn desktop_entry_id() -> String {
    if std::env::var_os("APPIMAGE").is_some_and(|a| !a.is_empty())
        && let Some(bin) = std::env::current_exe()
            .ok()
            .and_then(|b| b.file_name().map(|n| n.to_string_lossy().into_owned()))
    {
        let file = super::register::handler_file_name(&bin);
        return file.trim_end_matches(".desktop").to_string();
    }
    "dayz-community-hub".to_string()
}

#[cfg(windows)]
fn show(note: &Note) -> Result<(), String> {
    notify_rust::Notification::new()
        .appname("DayZ Community Hub")
        .summary(&note.title)
        .body(&note.body)
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn show(_: &Note) -> Result<(), String> {
    Err("no notifications on this system".into())
}
