//! The control channel: the running window, driven from outside (agents, scripts, tests).
//! The protocol is `docs/control-protocol.md`.
//!
//! - `--control <port>`: [`start`] binds `127.0.0.1:<port>`, writes a random key to
//!   `<data dir>/control-key.local-<port>` (mode 0600 on Unix, removed when the window
//!   closes) and serves JSON lines on the tokio runtime, never on the UI thread.
//! - `hub.<method>` requests are answered there, straight from the [`Hub`] (read-only
//!   calls); every other request travels to the UI thread as a [`ControlRequest`],
//!   is handled at the start of the next frame ([`Control::frame`]) and answered on its
//!   oneshot channel. A request wakes the window; an idle window stays idle.
//! - `ui_shot` feeds the same [`ControlRequest`]s to a headless window, so a script and
//!   a live window behave alike.
//!
//! Requests are hostile: sizes are clamped, paths must be absolute, nothing panics.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use dz_hub::Hub;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use crate::HubApp;
use crate::app::Ctx;
use crate::commands;
use crate::i18n::{self, Locale};
use crate::views::ViewId;

/// Longer request lines are refused (and the connection closed).
const MAX_LINE: usize = 1 << 20;
/// A connection that sends no authorised request in this time is closed.
const FIRST_REQUEST: Duration = Duration::from_secs(30);
/// How long a request may wait for the UI thread (a minimised window may draw rarely).
const UI_TIMEOUT: Duration = Duration::from_secs(60);
/// Requests handled per frame at most; the rest wait for the next (which is asked for).
const PER_FRAME: usize = 64;
/// Synthetic input events waiting at most (a flood is dropped, not buffered).
const MAX_SYNTHETIC: usize = 4096;

/// One request for the UI thread, and where its reply goes.
pub struct ControlRequest {
    pub method: String,
    pub params: Value,
    pub reply: oneshot::Sender<Value>,
}

impl ControlRequest {
    pub fn new(method: impl Into<String>, params: Value) -> (Self, oneshot::Receiver<Value>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                method: method.into(),
                params,
                reply: tx,
            },
            rx,
        )
    }
}

/// `{"ok": true, "result": v}`.
pub fn ok(v: Value) -> Value {
    json!({"ok": true, "result": v})
}

/// `{"ok": false, "error": e}`.
pub fn err(e: impl std::fmt::Display) -> Value {
    json!({"ok": false, "error": e.to_string()})
}

/// The key file of a running control server, removed when dropped (the window closed).
pub struct KeyFile(PathBuf);

impl KeyFile {
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for KeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `<dir>/control-key.local-<port>`.
pub fn key_path(dir: &Path, port: u16) -> PathBuf {
    dir.join(format!("control-key.local-{port}"))
}

/// The UI thread's side of the channel: the requests to handle, screenshots waiting for
/// their image, input events waiting for the next frame's raw input.
pub struct Control {
    rx: mpsc::Receiver<ControlRequest>,
    shots: Vec<(u64, Option<PathBuf>, oneshot::Sender<Value>)>,
    next_shot: u64,
    synthetic: Vec<egui::Event>,
    _key: Option<KeyFile>,
}

impl Control {
    pub fn new(rx: mpsc::Receiver<ControlRequest>, key: Option<KeyFile>) -> Self {
        Self {
            rx,
            shots: Vec::new(),
            next_shot: 1,
            synthetic: Vec::new(),
            _key: key,
        }
    }

    /// Before a frame: the queued pointer and key events join the window's input.
    pub fn input_hook(&mut self, raw: &mut egui::RawInput) {
        raw.events.append(&mut self.synthetic);
    }

    /// At the start of a frame: screenshots that arrived, then the requests waiting.
    pub fn frame(&mut self, app: &mut HubApp, ctx: &egui::Context) {
        if !self.shots.is_empty() {
            self.take_screenshots(ctx);
        }
        for _ in 0..PER_FRAME {
            let Ok(req) = self.rx.try_recv() else {
                return;
            };
            if let Some(reply) = self.handle(app, ctx, &req.method, &req.params) {
                let _ = req.reply.send(reply);
            } else {
                // A screenshot: answered once the image is there.
                let id = self.next_shot;
                self.next_shot = self.next_shot.wrapping_add(1);
                let path = req
                    .params
                    .get("path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                self.shots.push((id, path, req.reply));
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(id)));
            }
        }
        ctx.request_repaint();
    }

    fn take_screenshots(&mut self, ctx: &egui::Context) {
        let images: Vec<(u64, Arc<egui::ColorImage>)> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot {
                        image, user_data, ..
                    } => {
                        let id = user_data.data.as_ref()?.downcast_ref::<u64>()?;
                        Some((*id, Arc::clone(image)))
                    }
                    _ => None,
                })
                .collect()
        });
        for (id, image) in images {
            let Some(i) = self.shots.iter().position(|(s, _, _)| *s == id) else {
                continue;
            };
            let (_, path, reply) = self.shots.swap_remove(i);
            let r = match path {
                Some(p) => save_png(&image, &p),
                None => Err("missing `path`".to_string()),
            };
            let _ = reply.send(match r {
                Ok(v) => ok(v),
                Err(e) => err(e),
            });
        }
    }

    fn push(&mut self, events: impl IntoIterator<Item = egui::Event>) -> Value {
        for e in events {
            if self.synthetic.len() >= MAX_SYNTHETIC {
                return err("too much input queued");
            }
            self.synthetic.push(e);
        }
        ok(json!({"queued": true}))
    }

    /// One request; `None` for a screenshot (answered later).
    fn handle(
        &mut self,
        app: &mut HubApp,
        ctx: &egui::Context,
        method: &str,
        p: &Value,
    ) -> Option<Value> {
        let s = |k: &str| p.get(k).and_then(Value::as_str);
        let f = |k: &str| {
            p.get(k)
                .and_then(Value::as_f64)
                .map(|v| v as f32)
                .filter(|v| v.is_finite())
        };
        let reply = match method {
            "ui.screenshot" => {
                return match s("path") {
                    Some(path) if Path::new(path).is_absolute() => None,
                    Some(_) => Some(err("`path` must be absolute")),
                    None => Some(err("missing `path`")),
                };
            }
            "ui.state" => ok(state(app, ctx)),
            "ui.commands" => app.with_ctx(ctx, |cx| {
                let list: Vec<Value> = commands::all()
                    .iter()
                    .map(|c| {
                        json!({
                            "id": c.id,
                            "label": c.words(),
                            "group": c.group,
                            "enabled": (c.enabled)(cx),
                        })
                    })
                    .collect();
                ok(Value::Array(list))
            }),
            "ui.view" => match s("id").and_then(ViewId::from_id) {
                Some(v) => app.with_ctx(ctx, |cx| {
                    match s("focus") {
                        Some(focus) => cx.go_focus(v, focus),
                        None => cx.go(v),
                    }
                    ok(json!({"view": v.id()}))
                }),
                None => err("unknown view `id`"),
            },
            "ui.command" => match s("id") {
                Some(id) => {
                    let args = p.get("args").cloned().unwrap_or(Value::Null);
                    run(app, ctx, id, &args)
                }
                None => err("missing `id`"),
            },
            "ui.locale" => match s("code").and_then(Locale::from_code) {
                Some(l) => app.with_ctx(ctx, |cx| {
                    cx.set_locale(l);
                    ok(json!({"locale": l.code()}))
                }),
                None => err("unknown locale `code` (en fr de es ru)"),
            },
            "ui.theme" => match s("id") {
                Some(id) => app.with_ctx(ctx, |cx| set_theme(cx, id)),
                None => err("missing `id`"),
            },
            "ui.size" => match (f("w"), f("h")) {
                (Some(w), Some(h)) => {
                    let size = egui::vec2(w.clamp(320.0, 16_384.0), h.clamp(240.0, 16_384.0));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                    ok(json!({"w": size.x, "h": size.y}))
                }
                _ => err("missing `w`/`h`"),
            },
            "ui.click" => match (f("x"), f("y")) {
                (Some(x), Some(y)) => {
                    let pos = egui::pos2(x, y);
                    let button = match s("button") {
                        Some("right") => egui::PointerButton::Secondary,
                        Some("middle") => egui::PointerButton::Middle,
                        _ => egui::PointerButton::Primary,
                    };
                    let count = p.get("count").and_then(Value::as_u64).unwrap_or(1);
                    let m = modifiers(p);
                    let mut events = vec![egui::Event::PointerMoved(pos)];
                    for _ in 0..count.clamp(1, 3) {
                        for pressed in [true, false] {
                            events.push(egui::Event::PointerButton {
                                pos,
                                button,
                                pressed,
                                modifiers: m,
                            });
                        }
                    }
                    self.push(events)
                }
                _ => err("missing `x`/`y`"),
            },
            "ui.move" => match (f("x"), f("y")) {
                (Some(x), Some(y)) => self.push([egui::Event::PointerMoved(egui::pos2(x, y))]),
                _ => err("missing `x`/`y`"),
            },
            "ui.wheel" => match (f("x"), f("y")) {
                (Some(x), Some(y)) => {
                    let delta = egui::vec2(
                        f("dx").unwrap_or(0.0).clamp(-10_000.0, 10_000.0),
                        f("dy").unwrap_or(0.0).clamp(-10_000.0, 10_000.0),
                    );
                    self.push([
                        egui::Event::PointerMoved(egui::pos2(x, y)),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta,
                            phase: egui::TouchPhase::Move,
                            modifiers: modifiers(p),
                        },
                    ])
                }
                _ => err("missing `x`/`y`"),
            },
            "ui.key" => match s("key").and_then(key_from) {
                Some(key) => {
                    let m = modifiers(p);
                    self.push([true, false].map(|pressed| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: m,
                    }))
                }
                None => err("unknown `key`"),
            },
            "ui.text" => match s("text") {
                Some(t) if t.len() <= 64 * 1024 => self.push([egui::Event::Text(t.to_string())]),
                Some(_) => err("`text` is too long"),
                None => err("missing `text`"),
            },
            other if commands::registered(other) => run(app, ctx, other, p),
            other => err(format!("unknown method `{other}`")),
        };
        Some(reply)
    }
}

/// Run a command of the registry, refusing a disabled one.
fn run(app: &mut HubApp, ctx: &egui::Context, id: &str, args: &Value) -> Value {
    app.with_ctx(ctx, |cx| {
        let Some(c) = commands::find(id) else {
            return err(format!("unknown command `{id}`"));
        };
        if !(c.enabled)(cx) {
            return err(format!("`{id}` is disabled now"));
        }
        match cx.run(id, args) {
            Ok(()) => ok(json!({})),
            Err(e) => err(e),
        }
    })
}

/// Wear a theme: a preset id, a custom theme's id, `custom`, or `system`.
pub fn set_theme(cx: &mut Ctx, id: &str) -> Value {
    let want = (id != "system").then_some(id);
    cx.stores.theme.use_theme(cx.prefs, want);
    let selected = cx.stores.theme.selected(cx.prefs).map(str::to_string);
    if want.is_some() && selected.as_deref() != want && want != Some("custom") {
        // A retired preset resolves to its survivor; anything else is unknown.
        if selected.is_none() {
            return err(format!("unknown theme `{id}`"));
        }
    }
    cx.egui.request_repaint();
    ok(json!({"theme": selected}))
}

/// What the window shows and holds, for `ui.state`.
pub fn state(app: &mut HubApp, ctx: &egui::Context) -> Value {
    let r = ctx.content_rect();
    app.with_ctx(ctx, |cx| {
        let s = &*cx.stores;
        let p = cx.prefs.get();
        json!({
            "view": cx.nav.view.id(),
            "focus": cx.nav.focus,
            "locale": i18n::locale().code(),
            "theme": s.theme.current(cx.prefs).id,
            "themeSelected": s.theme.selected(cx.prefs),
            "railCollapsed": p.ui.rail_collapsed,
            "scale": p.scale,
            "window": [r.width(), r.height()],
            "initialized": s.app.initialized,
            "initError": s.app.init_error,
            "wizard": s.app.show_wizard,
            "dialogs": {
                "palette": s.dialogs.palette,
                "shortcuts": s.dialogs.shortcuts,
                "confirm": s.dialogs.pending().is_some(),
            },
            "counts": {
                "servers": s.servers.total,
                "maps": s.servers.maps.len(),
                "favorites": s.profile.favorites.len(),
                "history": s.profile.history.len(),
                "mods": s.mods.installed.len(),
                "pinged": s.servers.pinged(),
            },
            "busy": {
                "servers": s.servers.loading || s.servers.refreshing,
                "mods": s.mods.loading || s.mods.busy(),
            },
        })
    })
}

fn modifiers(p: &Value) -> egui::Modifiers {
    let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
    let cmd = b("ctrl") || b("cmd");
    egui::Modifiers {
        alt: b("alt"),
        ctrl: cmd && !cfg!(target_os = "macos"),
        shift: b("shift"),
        mac_cmd: cmd && cfg!(target_os = "macos"),
        command: cmd,
    }
}

fn key_from(name: &str) -> Option<egui::Key> {
    egui::Key::from_name(name).or(match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => Some(egui::Key::Enter),
        "esc" | "escape" => Some(egui::Key::Escape),
        "left" => Some(egui::Key::ArrowLeft),
        "right" => Some(egui::Key::ArrowRight),
        "up" => Some(egui::Key::ArrowUp),
        "down" => Some(egui::Key::ArrowDown),
        "space" => Some(egui::Key::Space),
        "tab" => Some(egui::Key::Tab),
        "backspace" => Some(egui::Key::Backspace),
        "delete" => Some(egui::Key::Delete),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        _ => None,
    })
}

/// Write the window's image as a PNG at `path` (absolute).
pub fn save_png(image: &egui::ColorImage, path: &Path) -> Result<Value, String> {
    if !path.is_absolute() {
        return Err("`path` must be absolute".into());
    }
    let [w, h] = image.size;
    let (Ok(w32), Ok(h32)) = (u32::try_from(w), u32::try_from(h)) else {
        return Err("image too large".into());
    };
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    let img = image::RgbaImage::from_raw(w32, h32, bytes).ok_or("bad image size")?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    img.save_with_format(path, image::ImageFormat::Png)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json!({"path": path.to_string_lossy(), "width": w, "height": h}))
}

// ── the hub's read-only calls ───────────────────────────────────────────────

/// The `hub.*` methods (answered off the UI thread).
pub const HUB_METHODS: &[&str] = &[
    "hub.servers_query",
    "hub.servers_lookup",
    "hub.server_maps",
    "hub.get_profile",
    "hub.get_installed_mods",
    "hub.mods_usage",
    "hub.get_app_stats",
    "hub.refresh_state",
    "hub.sources_status",
    "hub.ping_scan_state",
    "hub.game_running",
    "hub.gamepad_status",
    "hub.get_system_specs",
];

fn to_reply<T: serde::Serialize>(r: Result<T, String>) -> Value {
    match r.and_then(|v| serde_json::to_value(v).map_err(|e| e.to_string())) {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

/// The browser's query with every field defaulted, `params` over it.
fn server_query(p: &Value) -> Result<dz_hub::features::browser::ServerQuery, String> {
    let mut q = json!({
        "search": "", "map": null,
        "firstPerson": "all", "password": "all", "battleye": "all", "modded": "all",
        "official": "all", "hideEmpty": false, "hideFull": false, "maxPing": 0,
        "showExcluded": false, "sort": "none", "asc": true, "offset": 0, "limit": 50,
    });
    if let (Some(q), Some(p)) = (q.as_object_mut(), p.as_object()) {
        for (k, v) in p {
            q.insert(k.clone(), v.clone());
        }
    }
    let mut q: dz_hub::features::browser::ServerQuery =
        serde_json::from_value(q).map_err(|e| format!("bad query: {e}"))?;
    q.limit = q.limit.min(1000);
    Ok(q)
}

/// A `hub.*` call.
pub async fn hub_call(hub: &Hub, method: &str, p: &Value) -> Value {
    match method {
        "hub.servers_query" => match server_query(p) {
            Ok(q) => to_reply(hub.servers_query(q).await),
            Err(e) => err(e),
        },
        "hub.servers_lookup" => {
            let keys: Vec<String> = p
                .get("keys")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .take(1000)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            to_reply(hub.servers_lookup(keys).await)
        }
        "hub.server_maps" => to_reply(hub.server_maps().await),
        "hub.get_profile" => to_reply(hub.get_profile().await),
        "hub.get_installed_mods" => to_reply(hub.get_installed_mods().await),
        "hub.mods_usage" => to_reply(hub.mods_usage().await),
        "hub.get_app_stats" => to_reply(hub.get_app_stats().await),
        "hub.refresh_state" => to_reply(Ok(hub.refresh_state().await)),
        "hub.sources_status" => to_reply(Ok(hub.sources_status().await)),
        "hub.ping_scan_state" => to_reply(hub.ping_scan_state().await),
        "hub.game_running" => to_reply(hub.game_running().await),
        "hub.gamepad_status" => to_reply(Ok(hub.gamepad_status().await)),
        "hub.get_system_specs" => to_reply(hub.get_system_specs().await),
        other => err(format!(
            "`{other}` is not a hub method of the control channel ({})",
            HUB_METHODS.join(", ")
        )),
    }
}

// ── the server ──────────────────────────────────────────────────────────────

/// Bind `127.0.0.1:port`, write the key file into `dir` and serve on `rt`. `None` when
/// the port cannot be bound or no key can be made or written (the channel is then off:
/// a server whose key nobody can read would only refuse).
pub fn start(
    port: u16,
    rt: &tokio::runtime::Handle,
    hub: Hub,
    ctx: egui::Context,
    dir: &Path,
) -> Option<Control> {
    let listener = match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("control: cannot listen on 127.0.0.1:{port}: {e}");
            return None;
        }
    };
    if let Err(e) = listener.set_nonblocking(true) {
        eprintln!("control: {e}");
        return None;
    }
    let key = match random_key() {
        Ok(k) => k,
        Err(e) => {
            eprintln!("control: no random key: {e}");
            return None;
        }
    };
    let path = key_path(dir, port);
    if let Err(e) = write_key_file(&path, &key) {
        eprintln!("control: {e}");
        return None;
    }
    eprintln!(
        "control: listening on 127.0.0.1:{port}, key in {}",
        path.display()
    );
    let rx = serve_on(listener, rt, Some(hub), ctx, key, FIRST_REQUEST);
    Some(Control::new(rx, Some(KeyFile(path))))
}

/// A new key: 32 random bytes from the operating system, as hex.
fn random_key() -> Result<String, String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Write the key file: a stale one is replaced; on Unix the new file is created 0600.
fn write_key_file(p: &Path, key: &str) -> Result<(), String> {
    use std::io::Write;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    let _ = std::fs::remove_file(p);
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open, 0o600);
    let mut f = open
        .open(p)
        .map_err(|e| format!("cannot create {}: {e}", p.display()))?;
    f.write_all(key.as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", p.display()))
}

/// Compare keys in time that does not depend on where they differ; an empty key never matches.
fn keys_match(given: &str, key: &str) -> bool {
    let (a, b) = (given.as_bytes(), key.as_bytes());
    !b.is_empty()
        && a.len() == b.len()
        && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn serve_on(
    listener: std::net::TcpListener,
    rt: &tokio::runtime::Handle,
    hub: Option<Hub>,
    ctx: egui::Context,
    key: String,
    first: Duration,
) -> mpsc::Receiver<ControlRequest> {
    let (tx, rx) = mpsc::channel::<ControlRequest>();
    let key: Arc<str> = key.into();
    let rt2 = rt.clone();
    rt.spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(listener) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("control: {e}");
                return;
            }
        };
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            let (tx, ctx, key, hub) = (tx.clone(), ctx.clone(), Arc::clone(&key), hub.clone());
            rt2.spawn(async move { serve(stream, tx, ctx, hub, &key, first).await });
        }
    });
    rx
}

/// The next line, trimmed; `None` at the end of the stream, after an error or at `deadline`.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(
    r: &mut R,
    deadline: Option<tokio::time::Instant>,
) -> Option<Result<String, &'static str>> {
    let mut buf = Vec::new();
    let mut limited = (&mut *r).take(MAX_LINE as u64 + 1);
    let read = limited.read_until(b'\n', &mut buf);
    let n = match deadline {
        Some(d) => tokio::time::timeout_at(d, read).await.ok()?.ok()?,
        None => read.await.ok()?,
    };
    if n == 0 {
        return None;
    }
    if buf.len() > MAX_LINE {
        return Some(Err("line too long"));
    }
    Some(
        String::from_utf8(buf)
            .map(|s| s.trim().to_string())
            .map_err(|_| "not UTF-8"),
    )
}

async fn serve(
    stream: tokio::net::TcpStream,
    tx: mpsc::Sender<ControlRequest>,
    ctx: egui::Context,
    hub: Option<Hub>,
    key: &str,
    first: Duration,
) {
    let (read, mut out) = stream.into_split();
    let mut reader = BufReader::new(read);
    // One deadline from accept to the first authorised request; after it, none.
    let mut deadline = Some(tokio::time::Instant::now() + first);
    let mut conn_key: Option<String> = None;
    while let Some(line) = read_line(&mut reader, deadline).await {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                let _ = write_line(&mut out, &json!({"ok": false, "error": e})).await;
                break;
            }
        };
        if line.is_empty() {
            continue;
        }
        // A line that is not JSON closes the connection: an HTTP request (a web page's
        // `fetch` to this port) starts with a request line that never parses.
        let msg = match serde_json::from_str::<Value>(&line) {
            Ok(m) if m.is_object() => m,
            Ok(_) => {
                let _ = write_line(&mut out, &err("a request is a JSON object; closing")).await;
                break;
            }
            Err(e) => {
                let _ = write_line(&mut out, &err(format!("bad JSON: {e}; closing"))).await;
                break;
            }
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
        let given = msg.get("key").and_then(Value::as_str).or_else(|| {
            (method == "auth")
                .then(|| params.get("key").and_then(Value::as_str))
                .flatten()
        });
        let k = given.or(conn_key.as_deref()).unwrap_or("");
        if !keys_match(k, key) {
            let _ = write_line(&mut out, &with_id(err("unauthorized"), &id)).await;
            break;
        }
        deadline = None;
        let reply = if method == "auth" {
            conn_key = Some(k.to_string());
            ok(json!({}))
        } else if method.starts_with("hub.") {
            match &hub {
                Some(hub) => hub_call(hub, &method, &params).await,
                None => err("no hub"),
            }
        } else {
            let (req, rrx) = ControlRequest::new(method, params);
            if tx.send(req).is_err() {
                break;
            }
            ctx.request_repaint();
            match tokio::time::timeout(UI_TIMEOUT, rrx).await {
                Ok(Ok(v)) => v,
                Ok(Err(_)) => err("the window dropped the request"),
                Err(_) => err("timeout"),
            }
        };
        if write_line(&mut out, &with_id(reply, &id)).await.is_err() {
            break;
        }
    }
}

fn with_id(mut v: Value, id: &Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("id".into(), id.clone());
    }
    v
}

async fn write_line(out: &mut tokio::net::tcp::OwnedWriteHalf, v: &Value) -> std::io::Result<()> {
    let mut s = v.to_string();
    s.push('\n');
    out.write_all(s.as_bytes()).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader as StdBufReader, Write};
    use std::net::TcpStream;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn runtime() -> &'static tokio::runtime::Runtime {
        static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
        RT.get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap()
        })
    }

    fn boot_with(first: Duration) -> (u16, mpsc::Receiver<ControlRequest>) {
        let l = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        l.set_nonblocking(true).unwrap();
        let port = l.local_addr().unwrap().port();
        let rx = serve_on(
            l,
            runtime().handle(),
            // No hub: a second mock hub would move the app test's data directory.
            None,
            egui::Context::default(),
            KEY.into(),
            first,
        );
        (port, rx)
    }

    struct Conn(StdBufReader<TcpStream>, TcpStream);

    impl Conn {
        fn open(port: u16) -> Self {
            let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            Self(StdBufReader::new(s.try_clone().unwrap()), s)
        }
        fn send(&mut self, line: &str) {
            writeln!(self.1, "{line}").unwrap();
        }
        fn recv(&mut self) -> Value {
            let mut r = String::new();
            let _ = self.0.read_line(&mut r);
            serde_json::from_str(&r).unwrap_or_default()
        }
        fn call(&mut self, line: &str) -> Value {
            self.send(line);
            self.recv()
        }
        fn closed(&mut self) -> bool {
            let mut r = String::new();
            matches!(self.0.read_line(&mut r), Ok(0) | Err(_))
        }
    }

    fn answer(rx: &mpsc::Receiver<ControlRequest>) -> (String, Value) {
        let req = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let _ = req.reply.send(ok(json!({})));
        (req.method, req.params)
    }

    #[test]
    fn requests_need_the_key() {
        let (port, rx) = boot_with(FIRST_REQUEST);
        let mut c = Conn::open(port);
        let r = c.call(r#"{"id":1,"method":"ui.state"}"#);
        assert_eq!(
            (r["error"].clone(), r["id"].clone()),
            (json!("unauthorized"), json!(1))
        );
        assert!(c.closed());
        for key in ["nope", &KEY[..63], &KEY.to_uppercase()] {
            let mut c = Conn::open(port);
            let r = c.call(&format!(r#"{{"key":"{key}","method":"ui.state"}}"#));
            assert_eq!(r["error"], "unauthorized");
        }
        // `params.key` authenticates only `auth`.
        let mut c = Conn::open(port);
        let r = c.call(&format!(
            r#"{{"method":"ui.state","params":{{"key":"{KEY}"}}}}"#
        ));
        assert_eq!(r["error"], "unauthorized");
        assert!(rx.try_recv().is_err(), "nothing reached the window");
    }

    #[test]
    fn requests_with_the_key_reach_the_window() {
        let (port, rx) = boot_with(FIRST_REQUEST);
        let mut c = Conn::open(port);
        c.send(&format!(
            r#"{{"id":7,"key":"{KEY}","method":"ui.view","params":{{"id":"mods"}}}}"#
        ));
        let (m, p) = answer(&rx);
        assert_eq!((m.as_str(), p), ("ui.view", json!({"id": "mods"})));
        let r = c.recv();
        assert_eq!((r["ok"].clone(), r["id"].clone()), (json!(true), json!(7)));
        // `auth` once, then no key.
        let mut c = Conn::open(port);
        assert_eq!(
            c.call(&format!(r#"{{"method":"auth","key":"{KEY}"}}"#))["ok"],
            true
        );
        c.send(r#"{"id":2,"method":"ui.state"}"#);
        answer(&rx);
        assert_eq!(c.recv()["ok"], true);
    }

    #[test]
    fn bad_lines_close_the_connection() {
        let (port, rx) = boot_with(FIRST_REQUEST);
        let mut c = Conn::open(port);
        assert_eq!(c.call("GET / HTTP/1.1")["ok"], false);
        assert!(c.closed());
        let mut c = Conn::open(port);
        let big = format!(r#"{{"key":"{KEY}","pad":"{}"}}"#, "x".repeat(MAX_LINE));
        assert_eq!(c.call(&big)["error"], "line too long");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn a_silent_connection_is_closed() {
        let (port, _rx) = boot_with(Duration::from_millis(300));
        let mut c = Conn::open(port);
        let t0 = std::time::Instant::now();
        assert!(c.closed());
        assert!(t0.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn hub_calls_answer_off_the_window() {
        let (port, rx) = boot_with(FIRST_REQUEST);
        let mut c = Conn::open(port);
        let r = c.call(&format!(
            r#"{{"key":"{KEY}","method":"hub.servers_query","params":{{"limit":3}}}}"#
        ));
        assert_eq!(r["error"], "no hub", "{r}");
        assert!(rx.try_recv().is_err(), "hub calls never reach the window");
    }

    #[test]
    fn the_query_is_defaulted_and_clamped() {
        let q = server_query(&json!({"search": "dayz", "limit": 1_000_000})).unwrap();
        assert_eq!((q.search.as_str(), q.limit), ("dayz", 1000));
        assert!(server_query(&json!({"sort": "nonsense"})).is_err());
    }

    #[test]
    fn the_key_file_is_private_and_removed() {
        let dir = std::env::temp_dir().join(format!("dzch-control-{}", std::process::id()));
        let p = key_path(&dir, 7981);
        write_key_file(&p, KEY).unwrap();
        write_key_file(&p, KEY).unwrap(); // a stale one is replaced
        assert_eq!(std::fs::read_to_string(&p).unwrap(), KEY);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        drop(KeyFile(p.clone()));
        assert!(!p.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_compare_whole_and_are_random() {
        assert!(keys_match(KEY, KEY));
        assert!(!keys_match(&KEY[..63], KEY));
        assert!(!keys_match("", ""));
        let (a, b) = (random_key().unwrap(), random_key().unwrap());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
