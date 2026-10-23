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
        .hint(notify_rust::Hint::DesktopEntry("dayz-community-hub".into()))
        .show()
        .map(|_| ())
        .map_err(|e| e.to_string())
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
