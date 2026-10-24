//! The tray icon and its menu: show or hide the launcher, rejoin the last server,
//! quit. What a pick does is decided in [`super::life`]; this module only puts the icon
//! up and hands the picks over a channel, waking the window (`request_repaint`, which
//! eframe honours while the window is hidden too) or, between windows (Wayland's close
//! to the tray), `main`, which waits for them ([`crate::rt::Waker`]). The tray outlives
//! the windows: it goes from one to the next with [`super::life::Life`].
//!
//! - Linux: a StatusNotifierItem through ksni (D-Bus over zbus, pure Rust: no GTK, no
//!   libappindicator), on its own thread. It is up only once a StatusNotifierWatcher
//!   took it and a host shows it: GNOME without the AppIndicator extension has none,
//!   and [`Tray::available`] then stays false (so closing quits). A host missing at
//!   start (the panel still loading, the extension turned on later) or that goes away
//!   later (a shell restart, the plugin turned off) keeps it false until it is there.
//! - Windows: `Shell_NotifyIcon` through tray-icon (windows-sys, no GTK). Win32 ties
//!   the icon's hidden window to the thread that made it, which must pump messages:
//!   it is made on the window's thread, whose event loop (winit's) pumps them.
//!   tray-icon's `build()` succeeds even when no notification area took the icon (it
//!   waits for `TaskbarCreated`), so the icon is up only while Explorer's
//!   notification area window (`Shell_TrayWnd`, the one `Shell_NotifyIcon` talks to)
//!   exists, looked at again every few seconds ([`Tray::poll`]). `FindWindowW` by
//!   class sends no message, so a hung Explorer cannot make the frame wait (which
//!   `Shell_NotifyIconGetRect`, a message to Explorer, could).
//! - Elsewhere: no tray.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use crate::rt::Waker;

/// What was picked in the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayMsg {
    /// The icon itself, or the menu's "show / hide" item.
    Toggle,
    Rejoin,
    Quit,
}

/// The tray's words.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayView {
    pub tooltip: String,
    /// "Show the launcher" or "Hide the launcher".
    pub toggle: String,
    /// "Rejoin <server>", when there is a last server.
    pub rejoin: Option<String>,
    pub quit: String,
}

/// The running tray. Dropping it takes the icon down.
pub struct Tray {
    up: Arc<AtomicBool>,
    picks: mpsc::Receiver<TrayMsg>,
    shown: Option<TrayView>,
    inner: imp::Inner,
}

impl Tray {
    /// Put the icon up with these words; `None` where there is no tray at all. The
    /// icon may come up a moment later (Linux), or never: see [`Tray::available`].
    pub fn start(waker: &Waker, view: TrayView) -> Option<Tray> {
        let up = Arc::new(AtomicBool::new(false));
        let (tx, picks) = mpsc::channel();
        let send = Sender {
            tx,
            waker: waker.clone(),
        };
        let inner = imp::start(send, Arc::clone(&up), view.clone())?;
        Some(Tray {
            up,
            picks,
            shown: Some(view),
            inner,
        })
    }

    /// Whether a tray host shows the icon now.
    pub fn available(&self) -> bool {
        self.up.load(Ordering::SeqCst)
    }

    /// Look again whether the icon is shown, where that cannot be told by an event
    /// (Windows); while the window is out of sight, ask for a pass now and then so a
    /// tray that went away is noticed.
    pub fn poll(&mut self, ctx: &egui::Context, out_of_sight: bool) {
        self.inner.poll(ctx, out_of_sight);
    }

    /// The next pick, if any.
    pub fn try_recv(&self) -> Option<TrayMsg> {
        self.picks.try_recv().ok()
    }

    /// New words (sent on only when they changed).
    pub fn set_view(&mut self, view: TrayView) {
        if self.shown.as_ref() != Some(&view) {
            self.inner.update(&view);
            self.shown = Some(view);
        }
    }
}

/// Hands a pick to the window (or to `main` between windows) and wakes it.
#[derive(Clone)]
struct Sender {
    tx: mpsc::Sender<TrayMsg>,
    waker: Waker,
}

impl Sender {
    fn send(&self, m: TrayMsg) {
        let _ = self.tx.send(m);
        self.waker.repaint();
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn wake(&self) {
        self.waker.repaint();
    }
}

/// The app icon, `px` square, as RGBA (empty when it cannot be read).
fn icon_rgba(px: u32) -> (Vec<u8>, u32, u32) {
    let icon = super::window::icon();
    let Some(img) = image::RgbaImage::from_raw(icon.width, icon.height, icon.rgba) else {
        return (Vec::new(), 0, 0);
    };
    if img.width() == 0 || img.height() == 0 {
        return (Vec::new(), 0, 0);
    }
    let small = image::imageops::resize(&img, px, px, image::imageops::FilterType::Lanczos3);
    let (w, h) = small.dimensions();
    (small.into_raw(), w, h)
}

/// A menu label as a StatusNotifierItem host reads it: `_` marks an access key there,
/// so a server's own underscores are doubled.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn sni_label(s: &str) -> String {
    s.replace('_', "__")
}

/// A menu label as Win32 reads it: `&` marks an access key there, so a server's own
/// ampersands are doubled; a tab would start the shortcut's text, so none is kept.
#[cfg_attr(not(windows), allow(dead_code))]
fn win_label(s: &str) -> String {
    s.replace('\t', " ").replace('&', "&&")
}

/// RGBA to the ARGB32 (network byte order) a StatusNotifierItem carries.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn argb(rgba: &[u8]) -> Vec<u8> {
    let (px, _) = rgba.as_chunks::<4>();
    px.iter().flat_map(|&[r, g, b, a]| [a, r, g, b]).collect()
}

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;

    use ksni::blocking::TrayMethods;
    use ksni::menu::StandardItem;
    use ksni::{MenuItem, OfflineReason, ToolTip};

    use super::{Sender, TrayMsg, TrayView, argb, icon_rgba, sni_label};

    /// The words go to the tray's thread, which hands them to ksni (a round trip
    /// through its service, never made on the window's thread).
    pub struct Inner {
        views: mpsc::Sender<TrayView>,
    }

    impl Inner {
        pub fn update(&self, view: &TrayView) {
            let _ = self.views.send(view.clone());
        }

        /// The host's coming and going are events here: nothing to look at.
        pub fn poll(&mut self, _: &egui::Context, _: bool) {}
    }

    struct Item {
        view: TrayView,
        icon: Vec<ksni::Icon>,
        send: Sender,
        up: Arc<AtomicBool>,
        /// The host was found missing at least once (at registration too).
        missed: Arc<AtomicBool>,
    }

    impl ksni::Tray for Item {
        fn id(&self) -> String {
            "dayz-community-hub".into()
        }

        fn title(&self) -> String {
            "DayZ Community Hub".into()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            self.icon.clone()
        }

        fn tool_tip(&self) -> ToolTip {
            ToolTip {
                title: self.view.tooltip.clone(),
                ..ToolTip::default()
            }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.send.send(TrayMsg::Toggle);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let item = |label: &str, m: TrayMsg| -> MenuItem<Self> {
                StandardItem {
                    label: sni_label(label),
                    activate: Box::new(move |t: &mut Self| t.send.send(m)),
                    ..StandardItem::default()
                }
                .into()
            };
            let mut items = vec![item(&self.view.toggle, TrayMsg::Toggle)];
            if let Some(r) = &self.view.rejoin {
                items.push(item(r, TrayMsg::Rejoin));
            }
            items.push(MenuItem::Separator);
            items.push(item(&self.view.quit, TrayMsg::Quit));
            items
        }

        fn watcher_online(&self) {
            self.up.store(true, Ordering::SeqCst);
            self.send.wake();
        }

        fn watcher_offline(&self, reason: OfflineReason) -> bool {
            eprintln!("tray: no tray host for now ({reason:?})");
            self.missed.store(true, Ordering::SeqCst);
            self.up.store(false, Ordering::SeqCst);
            self.send.wake();
            // Keep waiting for it to come back.
            true
        }
    }

    pub fn start(send: Sender, up: Arc<AtomicBool>, view: TrayView) -> Option<Inner> {
        let (views, rx) = mpsc::channel::<TrayView>();
        let spawned = std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || run(send, up, view, &rx));
        match spawned {
            Ok(_) => Some(Inner { views }),
            Err(e) => {
                eprintln!("tray: {e}");
                None
            }
        }
    }

    /// The tray's thread: register (no host, no tray: `up` stays false until one
    /// appears, ksni then registers again and says so), then pass the words on until
    /// the window drops its end.
    fn run(send: Sender, up: Arc<AtomicBool>, view: TrayView, rx: &mpsc::Receiver<TrayView>) {
        let (rgba, w, h) = icon_rgba(64);
        let icon = match (i32::try_from(w), i32::try_from(h)) {
            (Ok(width), Ok(height)) if !rgba.is_empty() => vec![ksni::Icon {
                width,
                height,
                data: argb(&rgba),
            }],
            _ => Vec::new(),
        };
        let missed = Arc::new(AtomicBool::new(false));
        let item = Item {
            view,
            icon,
            send: send.clone(),
            up: Arc::clone(&up),
            missed: Arc::clone(&missed),
        };
        // A watcher missing at start (the panel still loading, the AppIndicator
        // extension turned on later) is waited for, not taken as no tray for good.
        let handle = match item.assume_sni_available(true).spawn() {
            Ok(h) => h,
            Err(e) => {
                eprintln!("tray: no system tray ({e}); closing the window quits");
                return;
            }
        };
        // Registered now unless the host was missing (`watcher_online` says when
        // it comes).
        if !missed.load(Ordering::SeqCst) {
            up.store(true, Ordering::SeqCst);
        }
        send.wake();
        while let Ok(mut v) = rx.recv() {
            // Only the newest of a burst matters.
            while let Ok(next) = rx.try_recv() {
                v = next;
            }
            if handle.update(|t| t.view = v).is_none() {
                break;
            }
        }
        up.store(false, Ordering::SeqCst);
        handle.shutdown().wait();
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::{Sender, TrayMsg, TrayView, icon_rgba, win_label};

    /// How often the notification area is looked for.
    const EVERY: Duration = Duration::from_secs(2);

    /// Whether Explorer's notification area (the window `Shell_NotifyIcon` sends to)
    /// exists. By class only: no message is sent, nothing can block.
    fn notification_area() -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW;
        use windows_sys::w;
        // SAFETY: two valid (or null) wide strings; the handle is only compared.
        let hwnd = unsafe { FindWindowW(w!("Shell_TrayWnd"), std::ptr::null()) };
        !hwnd.is_null()
    }

    /// The icon and its items, on the window's thread.
    pub struct Inner {
        _icon: TrayIcon,
        toggle: MenuItem,
        rejoin: MenuItem,
        quit: MenuItem,
        up: Arc<AtomicBool>,
        looked: Instant,
    }

    impl Inner {
        pub fn poll(&mut self, ctx: &egui::Context, out_of_sight: bool) {
            if self.looked.elapsed() >= EVERY {
                self.looked = Instant::now();
                self.up.store(notification_area(), Ordering::SeqCst);
            }
            if out_of_sight {
                ctx.request_repaint_after(EVERY);
            }
        }

        pub fn update(&self, view: &TrayView) {
            self.toggle.set_text(win_label(&view.toggle));
            match &view.rejoin {
                Some(r) => {
                    self.rejoin.set_text(win_label(r));
                    self.rejoin.set_enabled(true);
                }
                None => {
                    self.rejoin.set_text("—");
                    self.rejoin.set_enabled(false);
                }
            }
            self.quit.set_text(win_label(&view.quit));
        }
    }

    impl Drop for Inner {
        fn drop(&mut self) {
            self.up.store(false, Ordering::SeqCst);
        }
    }

    pub fn start(send: Sender, up: Arc<AtomicBool>, view: TrayView) -> Option<Inner> {
        let toggle = MenuItem::new(win_label(&view.toggle), true, None);
        let rejoin = MenuItem::new(
            view.rejoin
                .as_deref()
                .map_or_else(|| "—".to_string(), win_label),
            view.rejoin.is_some(),
            None,
        );
        let quit = MenuItem::new(win_label(&view.quit), true, None);
        let menu = Menu::new();
        let built = menu
            .append(&toggle)
            .and_then(|()| menu.append(&rejoin))
            .and_then(|()| menu.append(&PredefinedMenuItem::separator()))
            .and_then(|()| menu.append(&quit));
        if let Err(e) = built {
            eprintln!("tray: {e}");
            return None;
        }
        let ids = [
            (toggle.id().clone(), TrayMsg::Toggle),
            (rejoin.id().clone(), TrayMsg::Rejoin),
            (quit.id().clone(), TrayMsg::Quit),
        ];
        let on_menu = send.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Some((_, m)) = ids.iter().find(|(id, _)| *id == e.id) {
                on_menu.send(*m);
            }
        }));
        let on_icon = send;
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = e
            {
                on_icon.send(TrayMsg::Toggle);
            }
        }));
        let (rgba, w, h) = icon_rgba(32);
        let mut builder = TrayIconBuilder::new()
            .with_tooltip(&view.tooltip)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false);
        match Icon::from_rgba(rgba, w, h) {
            Ok(icon) => builder = builder.with_icon(icon),
            Err(e) => eprintln!("tray: icon: {e}"),
        }
        match builder.build() {
            Ok(icon) => {
                // Built is not shown: only while a notification area took it.
                up.store(notification_area(), Ordering::SeqCst);
                Some(Inner {
                    _icon: icon,
                    toggle,
                    rejoin,
                    quit,
                    up,
                    looked: Instant::now(),
                })
            }
            Err(e) => {
                eprintln!("tray: no system tray ({e}); closing the window quits");
                None
            }
        }
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::{Sender, TrayView};

    pub struct Inner;

    impl Inner {
        pub fn update(&self, _: &TrayView) {}

        pub fn poll(&mut self, _: &egui::Context, _: bool) {}
    }

    pub fn start(_: Sender, _: Arc<AtomicBool>, _: TrayView) -> Option<Inner> {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_is_made_small_and_reordered() {
        let (rgba, w, h) = icon_rgba(32);
        assert_eq!((w, h), (32, 32));
        assert_eq!(rgba.len(), 32 * 32 * 4);
        assert_eq!(argb(&[1, 2, 3, 4, 5, 6, 7, 8]), [4, 1, 2, 3, 8, 5, 6, 7]);
        assert_eq!(sni_label("DE_1 | x"), "DE__1 | x");
        assert_eq!(win_label("Bob & Alice\tx"), "Bob && Alice x");
    }
}
