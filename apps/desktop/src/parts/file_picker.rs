//! Picking a file or a folder: the system's dialog with a mouse, the window's own
//! browser while a controller drives it or inside Steam's Game Mode (the system's
//! dialog has no pad navigation there, and gamescope may not show it at all).
//!
//! Call sites build a [`Pick`] and `await` [`pick`] from their task, exactly as they
//! awaited `rfd`'s dialog: `pick(Pick::folder(title)).await -> Option<PathBuf>`. The
//! window says each frame which one applies ([`set_in_app`]) and draws the browser
//! ([`show`]) after the views.
//!
//! The browser: the places to start from (home, every Steam library, the drives or
//! the removable media), the folder's path with its parent, then its folders first
//! and its files (those the filters keep), read off the UI thread. A folder opens on
//! A or a click. With the pad, the file the focus is on is the one chosen: A or X (the
//! dialog's confirming button) takes it; with a mouse a click selects a file and a
//! second click (or a double-click) takes it. A save names the file in a field (the
//! on-screen keyboard types it), and asks before replacing a file.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Rect, Sense, Ui, Vec2};
use tokio::sync::oneshot;

use crate::app::Ctx;
use crate::i18n::{t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::{
    Button, Confirm, Dialog, DialogSize, FieldSize, IconButton, Input, PadKey, Size, Variant, paint,
};
use crate::rt::{self, Task};
use crate::theme::{TokensExt, font, fonts::Face, size};

/// What is picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Folder,
    /// A file to write, named by the player.
    Save,
}

/// A question for [`pick`].
#[derive(Clone, Debug)]
pub struct Pick {
    kind: Kind,
    title: String,
    /// (label, extensions without the dot).
    filters: Vec<(String, Vec<String>)>,
    /// A save's suggested name.
    name: String,
}

impl Pick {
    pub fn file(title: impl Into<String>) -> Self {
        Self::new(Kind::File, title)
    }

    pub fn folder(title: impl Into<String>) -> Self {
        Self::new(Kind::Folder, title)
    }

    /// A file to write, `name` suggested.
    pub fn save(title: impl Into<String>, name: impl Into<String>) -> Self {
        let mut p = Self::new(Kind::Save, title);
        p.name = name.into();
        p
    }

    fn new(kind: Kind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            filters: Vec::new(),
            name: String::new(),
        }
    }

    /// Only files ending in one of `exts` ("json", no dot).
    pub fn filter(mut self, label: impl Into<String>, exts: &[&str]) -> Self {
        self.filters.push((
            label.into(),
            exts.iter().map(|e| e.to_ascii_lowercase()).collect(),
        ));
        self
    }

    fn exts(&self) -> Vec<&str> {
        self.filters
            .iter()
            .flat_map(|(_, e)| e.iter().map(String::as_str))
            .collect()
    }

    async fn native(self) -> Option<PathBuf> {
        let mut d = rfd::AsyncFileDialog::new().set_title(&self.title);
        for (label, exts) in &self.filters {
            d = d.add_filter(label, exts);
        }
        let h = match self.kind {
            Kind::File => d.pick_file().await,
            Kind::Folder => d.pick_folder().await,
            Kind::Save => d.set_file_name(&self.name).save_file().await,
        };
        h.map(|f| f.path().to_path_buf())
    }
}

static IN_APP: AtomicBool = AtomicBool::new(false);

/// Whether the window's own browser answers: a controller drives the window, or Steam's
/// Game Mode holds it. Set once a frame by the window.
pub fn set_in_app(on: bool) {
    IN_APP.store(on, Ordering::Relaxed);
}

struct Request {
    pick: Pick,
    reply: oneshot::Sender<Option<PathBuf>>,
}

/// The question waiting for the window (one at a time: a newer one replaces it).
static PENDING: Mutex<Option<Request>> = Mutex::new(None);
/// The browser on screen (the UI thread's only).
static OPEN: Mutex<Option<Browser>> = Mutex::new(None);
/// Where the browser was last, for the next question.
static LAST: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Ask for a file or a folder: `None` when the player cancels.
pub async fn pick(p: Pick) -> Option<PathBuf> {
    if !IN_APP.load(Ordering::Relaxed) {
        return p.native().await;
    }
    let (tx, rx) = oneshot::channel();
    if let Ok(mut slot) = PENDING.lock() {
        // An older question still waiting is answered "cancelled".
        if let Some(old) = slot.replace(Request { pick: p, reply: tx }) {
            let _ = old.reply.send(None);
        }
    } else {
        return None;
    }
    if let Some(rt) = rt::Rt::get() {
        rt.waker().repaint();
    }
    rx.await.ok().flatten()
}

/// A folder's entry.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    dir: bool,
}

/// A place to start from.
#[derive(Clone, PartialEq, Eq)]
struct Root {
    label: String,
    path: PathBuf,
    icon: Icon,
}

/// The most a folder lists (a system folder holds thousands).
const MAX_ENTRIES: usize = 4000;

/// A folder's entries the question keeps, folders first, by name (tested).
fn list_dir(dir: &Path, kind: Kind, exts: &[String], hidden: bool) -> Result<Vec<Entry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for e in rd.flatten() {
        if out.len() >= MAX_ENTRIES {
            break;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if !hidden && name.starts_with('.') {
            continue;
        }
        // Through links: a linked folder opens like a folder.
        let is_dir = std::fs::metadata(e.path()).is_ok_and(|m| m.is_dir());
        if !is_dir {
            if kind == Kind::Folder {
                continue;
            }
            let ext = Path::new(&name)
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase());
            if !exts.is_empty() && !ext.is_some_and(|x| exts.contains(&x)) {
                continue;
            }
        }
        out.push(Entry { name, dir: is_dir });
    }
    out.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

/// The places to start from: home, the Steam libraries, then the drives (Windows) or
/// the file system and the removable media (Linux: a Deck's SD card).
fn roots(steam: Vec<String>) -> Vec<Root> {
    let user = std::env::var("USER").unwrap_or_default();
    let media: Vec<PathBuf> = ["/run/media", "/media"]
        .iter()
        .map(|b| Path::new(b).join(&user))
        .collect();
    roots_in(dirs::home_dir(), steam, &media)
}

/// [`roots`] from where home is and the folders the removable media mount in (tested).
fn roots_in(home: Option<PathBuf>, steam: Vec<String>, media: &[PathBuf]) -> Vec<Root> {
    let mut out = Vec::new();
    if let Some(home) = home {
        out.push(Root {
            label: t("picker", "home").to_string(),
            path: home,
            icon: lucide::HOUSE,
        });
    }
    for s in steam {
        let path = PathBuf::from(&s);
        let last = path
            .file_name()
            .map_or_else(|| s.clone(), |n| n.to_string_lossy().into_owned());
        out.push(Root {
            label: tf("picker", "steamLibrary", &[("name", &last)]),
            path,
            icon: lucide::GAMEPAD_2,
        });
    }
    #[cfg(windows)]
    for c in b'A'..=b'Z' {
        let p = PathBuf::from(format!("{}:\\", c as char));
        if p.is_dir() {
            out.push(Root {
                label: format!("{}:", c as char),
                path: p,
                icon: lucide::HARD_DRIVE,
            });
        }
    }
    #[cfg(not(windows))]
    {
        out.push(Root {
            label: "/".to_string(),
            path: PathBuf::from("/"),
            icon: lucide::HARD_DRIVE,
        });
        for dir in media {
            let Ok(rd) = std::fs::read_dir(dir) else {
                continue;
            };
            for e in rd.flatten().take(16) {
                if e.path().is_dir() {
                    out.push(Root {
                        label: e.file_name().to_string_lossy().into_owned(),
                        path: e.path(),
                        icon: lucide::MEMORY_STICK,
                    });
                }
            }
        }
    }
    #[cfg(windows)]
    let _ = media;
    out.dedup_by(|a, b| a.path == b.path);
    out
}

/// A save's file name with an extension the filters ask for (tested).
fn with_ext(name: &str, exts: &[&str]) -> String {
    let name = name.trim();
    let has = Path::new(name)
        .extension()
        .is_some_and(|x| exts.iter().any(|e| x.eq_ignore_ascii_case(e)));
    match exts.first() {
        Some(e) if !has => format!("{name}.{e}"),
        _ => name.to_string(),
    }
}

/// Whether `name` can be a file's name here (no folder in it, not empty).
fn valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty() && n != "." && n != ".." && !n.contains(['/', '\\']) && !n.contains('\0')
}

/// The entry of the folder listed as `name` (Windows' names ignore case) (tested).
fn entry<'e>(entries: &'e [Entry], name: &str) -> Option<&'e Entry> {
    entries.iter().find(|e| {
        if cfg!(windows) {
            e.name.eq_ignore_ascii_case(name)
        } else {
            e.name == name
        }
    })
}

struct Browser {
    req: Request,
    open: bool,
    dir: PathBuf,
    entries: Vec<Entry>,
    listing: Option<Task<Result<Vec<Entry>, String>>>,
    error: Option<String>,
    roots: Vec<Root>,
    roots_task: Option<Task<Vec<Root>>>,
    hidden: bool,
    /// The file the first press chose (a second one confirms it).
    selected: Option<String>,
    name: String,
    /// Scroll the list back to its top (a new folder).
    top: bool,
    /// A save that would replace this file, asked about first.
    replace: Option<PathBuf>,
}

impl Browser {
    fn new(req: Request, cx: &Ctx) -> Self {
        let start = LAST
            .lock()
            .ok()
            .and_then(|l| l.clone())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));
        let hub = cx.hub.clone();
        let roots_task = rt::spawn(async move {
            let steam = hub.steam_library_dirs().await;
            tokio::task::spawn_blocking(move || roots(steam))
                .await
                .unwrap_or_default()
        });
        let name = req.pick.name.clone();
        let mut b = Self {
            req,
            open: true,
            dir: start.clone(),
            entries: Vec::new(),
            listing: None,
            error: None,
            roots: Vec::new(),
            roots_task: Some(roots_task),
            hidden: false,
            selected: None,
            name,
            top: true,
            replace: None,
        };
        b.go(start);
        b
    }

    /// Read `dir` (off the UI thread); the list shows it once read.
    fn go(&mut self, dir: PathBuf) {
        self.dir = dir.clone();
        // The old folder's rows would be joined to the new path while it is read.
        self.entries.clear();
        self.selected = None;
        self.error = None;
        self.top = true;
        let kind = self.req.pick.kind;
        let exts: Vec<String> = self
            .req
            .pick
            .exts()
            .into_iter()
            .map(str::to_string)
            .collect();
        let hidden = self.hidden;
        self.listing = Some(rt::spawn(async move {
            tokio::task::spawn_blocking(move || list_dir(&dir, kind, &exts, hidden))
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        }));
    }

    fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(r) = self.roots_task.as_mut().and_then(Task::poll) {
            self.roots = r;
            self.roots_task = None;
            changed = true;
        }
        if let Some(r) = self.listing.as_mut().and_then(Task::poll) {
            self.listing = None;
            match r {
                Ok(e) => self.entries = e,
                Err(e) => {
                    self.entries.clear();
                    self.error = Some(e);
                }
            }
            changed = true;
        }
        changed
    }
}

/// After the views: the browser, while a question waits for it.
pub fn show(cx: &mut Ctx) {
    let Ok(mut open) = OPEN.lock() else {
        return;
    };
    if let Some(req) = PENDING.lock().ok().and_then(|mut p| p.take()) {
        if let Some(old) = open.take() {
            let _ = old.req.reply.send(None);
        }
        *open = Some(Browser::new(req, cx));
    }
    let Some(b) = open.as_mut() else {
        return;
    };
    if b.poll() {
        cx.egui.request_repaint();
    }
    if b.listing.is_some() || b.roots_task.is_some() {
        cx.egui
            .request_repaint_after(std::time::Duration::from_millis(50));
    }
    let title = b.req.pick.title.clone();
    let mut is_open = b.open;
    // The body and the footer each borrow the browser in turn.
    let cell = std::cell::RefCell::new((&mut *b, None::<PathBuf>, None::<Option<PathBuf>>));
    let closed = Dialog::new("dzch.picker", &title)
        .size(DialogSize::Lg)
        .close_label(t("common", "close"))
        .show(
            cx.egui,
            &mut is_open,
            |ui, _| {
                let (b, go, answer) = &mut *cell.borrow_mut();
                body(ui, b, go, answer);
            },
            |ui, open| {
                let (b, _, answer) = &mut *cell.borrow_mut();
                footer(ui, b, open, answer);
            },
        );
    let (_, go, answer) = cell.into_inner();
    b.open = is_open;
    if let Some(dir) = go {
        b.go(dir);
    }
    // Replacing a file: asked first, as the system's dialog does.
    let mut answer = answer;
    if let Some(target) = b.replace.clone() {
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let body = tf("picker", "overwriteBody", &[("name", &name)]);
        let mut asking = true;
        let yes = Confirm::new(
            "dzch.picker.replace",
            t("picker", "overwriteTitle"),
            t("picker", "overwrite"),
            t("common", "cancel"),
        )
        .description(&body)
        .show(cx.egui, &mut asking);
        if yes {
            answer = Some(Some(target));
        }
        if !asking {
            b.replace = None;
        }
    }
    if let Some(a) = answer {
        b.open = false;
        if let Ok(mut l) = LAST.lock() {
            *l = Some(b.dir.clone());
        }
        if let Some(b) = open.take() {
            let _ = b.req.reply.send(a);
        }
        cx.egui.request_repaint();
    } else if (closed || !b.open)
        && let Some(b) = open.take()
    {
        let _ = b.req.reply.send(None);
    }
}

/// A path as wide as `w` at most, its start cut ("…/Steam/steamapps"): the end says
/// where the browser is.
fn tail(ui: &Ui, path: &str, w: f32) -> std::sync::Arc<egui::Galley> {
    let tk = ui.tokens();
    let f = font(Face::Mono, size::TEXT_XS);
    let full = paint::galley(ui, path, f.clone(), tk.fg_muted);
    if full.size().x <= w {
        return full;
    }
    let starts: Vec<usize> = path.char_indices().map(|(i, _)| i).collect();
    // The fewest characters cut that fit, found by halving.
    let (mut lo, mut hi) = (0usize, starts.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi) / 2;
        let cut = starts.get(mid).and_then(|i| path.get(*i..)).unwrap_or("");
        let g = paint::galley(ui, &format!("\u{2026}{cut}"), f.clone(), tk.fg_muted);
        if g.size().x <= w {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let cut = starts.get(lo).and_then(|i| path.get(*i..)).unwrap_or("");
    paint::galley(ui, &format!("\u{2026}{cut}"), f, tk.fg_muted)
}

fn body(
    ui: &mut Ui,
    b: &mut Browser,
    go: &mut Option<PathBuf>,
    answer: &mut Option<Option<PathBuf>>,
) {
    let tk = ui.tokens();
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
    // The places.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        let here = b
            .roots
            .iter()
            .filter(|r| b.dir.starts_with(&r.path))
            .max_by_key(|r| r.path.components().count())
            .map(|r| r.path.clone());
        for r in &b.roots {
            let on = here.as_ref() == Some(&r.path);
            if ui
                .add(Button::chip(&r.label, on).icon(r.icon).size(Size::Sm))
                .clicked()
            {
                *go = Some(r.path.clone());
            }
        }
    });
    // Where, its parent, and the hidden entries.
    ui.horizontal(|ui| {
        let parent = b.dir.parent().map(Path::to_path_buf);
        let up = ui.add(
            IconButton::new(lucide::CORNER_LEFT_UP, t("picker", "parent"))
                .size(Size::Icon)
                .enabled(parent.is_some()),
        );
        // In an empty folder the pad lands on the way back up.
        if b.entries.is_empty() && b.listing.is_none() {
            crate::pad::primary(ui.ctx(), up.id);
        }
        if up.clicked()
            && let Some(p) = parent
        {
            *go = Some(p);
        }
        let label = if b.hidden {
            t("picker", "hideHidden")
        } else {
            t("picker", "showHidden")
        };
        let eye = if b.hidden {
            lucide::EYE_OFF
        } else {
            lucide::EYE
        };
        let path = b.dir.to_string_lossy().into_owned();
        let hidden_w = size::control();
        let w = (ui.available_width() - hidden_w - 6.0).max(40.0);
        let g = tail(ui, &path, w);
        let (r, _) = ui.allocate_exact_size(Vec2::new(w, size::control()), Sense::hover());
        paint::text_at(ui.painter(), &g, r.left(), r.center().y, tk.fg_muted);
        if ui
            .add(
                IconButton::new(eye, label)
                    .size(Size::Icon)
                    .active(b.hidden),
            )
            .clicked()
        {
            b.hidden = !b.hidden;
            *go = Some(b.dir.clone());
        }
    });
    if b.req.pick.kind == Kind::Save {
        ui.horizontal(|ui| {
            let l = paint::galley(
                ui,
                t("picker", "fileName"),
                font(Face::Sans, size::TEXT_XS),
                tk.fg_muted,
            );
            let (r, _) =
                ui.allocate_exact_size(Vec2::new(l.size().x, size::control()), Sense::hover());
            paint::text_at(ui.painter(), &l, r.left(), r.center().y, tk.fg_muted);
            ui.add(
                Input::new(&mut b.name)
                    .size(FieldSize::Sm)
                    .width(ui.available_width())
                    .id_salt("dzch.picker.name"),
            );
        });
    }
    list(ui, b, go, answer);
}

/// The folder's entries, in a box of fixed height (a folder may hold thousands: only
/// the rows in sight are drawn, with one more each side for the pad to step onto).
fn list(
    ui: &mut Ui,
    b: &mut Browser,
    go: &mut Option<PathBuf>,
    answer: &mut Option<Option<PathBuf>>,
) {
    let tk = ui.tokens();
    let row_h = size::control() + 4.0;
    let screen_h = ui.ctx().content_rect().height();
    let h = (screen_h * 0.36).clamp(row_h * 3.0, row_h * 12.0);
    let w = ui.available_width();
    let (frame, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    paint::plate(ui.painter(), frame, size::RADIUS_SM, tk.sunken, tk.border);
    let note = |ui: &mut Ui, text: &str| {
        let g = paint::galley_wrap(
            ui,
            text,
            font(Face::Sans, size::TEXT_XS),
            tk.fg_muted,
            w - 24.0,
        );
        paint::text_center(ui.painter(), &g, frame, tk.fg_muted);
    };
    if b.listing.is_some() && b.entries.is_empty() {
        note(ui, t("picker", "reading"));
        return;
    }
    if let Some(e) = &b.error {
        note(ui, &tf("picker", "unreadable", &[("error", e)]));
        return;
    }
    if b.entries.is_empty() {
        note(ui, t("picker", "empty"));
        return;
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(frame.shrink(1.0)));
    let mut area = egui::ScrollArea::vertical()
        .id_salt("dzch.picker.list")
        .auto_shrink([false, false])
        .max_height(h - 2.0);
    if std::mem::take(&mut b.top) {
        area = area.vertical_scroll_offset(0.0);
    }
    let n = b.entries.len();
    area.show_viewport(&mut child, |ui, vp| {
        let total = row_h * n as f32;
        ui.set_height(total);
        let first = ((vp.min.y / row_h).floor() as isize - 1).max(0) as usize;
        let last = (((vp.max.y / row_h).ceil() as usize) + 1).min(n);
        let top = ui.min_rect().top();
        let rw = ui.available_width();
        for i in first..last {
            let Some(e) = b.entries.get(i) else {
                break;
            };
            let r = Rect::from_min_size(
                egui::pos2(ui.min_rect().left(), top + row_h * i as f32),
                Vec2::new(rw, row_h),
            );
            let id = egui::Id::unique(("dzch.picker.row", &e.name, e.dir));
            let resp = ui.interact(r, id, Sense::click());
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &e.name));
            if i == 0 {
                crate::pad::primary(ui.ctx(), id);
            }
            // With the pad, the focus is the choice: X (the dialog's confirm) takes it.
            if resp.gained_focus() && crate::kit::tooltip::pad_mode(ui.ctx()) {
                // (A save keeps the name typed: A copies a file's name into it.)
                b.selected = (!e.dir && b.req.pick.kind == Kind::File).then(|| e.name.clone());
            }
            let chosen = b.selected.as_deref() == Some(e.name.as_str()) && !e.dir;
            let hover = paint::ease(ui, id, resp.hovered() || resp.has_focus());
            let fill = if chosen {
                paint::alpha(tk.accent, 0.18)
            } else {
                paint::alpha(tk.raised, hover)
            };
            let p = ui.painter();
            p.rect_filled(
                r.shrink(1.0),
                egui::CornerRadius::same(size::RADIUS_SM),
                fill,
            );
            let icon = if e.dir { lucide::FOLDER } else { lucide::FILE };
            let ink = if e.dir { tk.accent } else { tk.fg_muted };
            let ir = Rect::from_center_size(
                egui::pos2(r.left() + 8.0 + size::ICON / 2.0, r.center().y),
                Vec2::splat(size::ICON),
            );
            icon.paint(p, ir, ink);
            let tx = ir.right() + 8.0;
            let g = paint::galley_trunc(
                ui,
                &e.name,
                font(Face::Sans, size::TEXT_SM),
                tk.fg,
                (r.right() - tx - 8.0).max(10.0),
            );
            paint::text_at(ui.painter(), &g, tx, r.center().y, tk.fg);
            if resp.clicked() || resp.double_clicked() {
                let path = b.dir.join(&e.name);
                if e.dir {
                    *go = Some(path);
                } else if b.req.pick.kind == Kind::Save {
                    b.name = e.name.clone();
                } else if chosen || resp.double_clicked() {
                    *answer = Some(Some(path));
                } else {
                    b.selected = Some(e.name.clone());
                }
            }
        }
    });
}

fn footer(ui: &mut Ui, b: &mut Browser, open: &mut bool, answer: &mut Option<Option<PathBuf>>) {
    if ui
        .add(
            Button::new(t("common", "cancel"))
                .size(Size::Sm)
                .pad_key(PadKey::Back),
        )
        .clicked()
    {
        *open = false;
    }
    let (label, target) = match b.req.pick.kind {
        Kind::Folder => (t("picker", "chooseFolder"), Some(b.dir.clone())),
        Kind::File => (
            t("picker", "open"),
            b.selected.as_ref().map(|n| b.dir.join(n)),
        ),
        Kind::Save => {
            let exts = b.req.pick.exts();
            let name = with_ext(&b.name, &exts);
            // Not over a folder of that name (nothing could be written there).
            let free = !entry(&b.entries, &name).is_some_and(|e| e.dir);
            (
                t("picker", "save"),
                (valid_name(&b.name) && free).then(|| b.dir.join(name)),
            )
        }
    };
    let enabled = target.is_some() && b.listing.is_none() && b.replace.is_none();
    if ui
        .add(
            Button::new(label)
                .variant(Variant::Accent)
                .size(Size::Sm)
                .enabled(enabled)
                .pad_key(PadKey::Confirm),
        )
        .clicked()
        && let Some(p) = target
    {
        let exists = b.req.pick.kind == Kind::Save
            && p.file_name()
                .and_then(|n| entry(&b.entries, &n.to_string_lossy()).map(|e| !e.dir))
                .unwrap_or(false);
        if exists {
            b.replace = Some(p);
        } else {
            *answer = Some(Some(p));
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "dzch-picker-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn folders_first_and_files_by_filter() {
        let d = temp();
        std::fs::create_dir(d.join("b-folder")).unwrap();
        std::fs::create_dir(d.join("A-folder")).unwrap();
        std::fs::create_dir(d.join(".hidden")).unwrap();
        std::fs::write(d.join("theme.JSON"), "{}").unwrap();
        std::fs::write(d.join("notes.txt"), "").unwrap();
        let exts = vec!["json".to_string()];
        let names = |v: Vec<Entry>| v.into_iter().map(|e| e.name).collect::<Vec<_>>();
        let got = names(list_dir(&d, Kind::File, &exts, false).unwrap());
        assert_eq!(got, ["A-folder", "b-folder", "theme.JSON"]);
        let got = names(list_dir(&d, Kind::Folder, &exts, true).unwrap());
        assert_eq!(got, [".hidden", "A-folder", "b-folder"]);
        let got = names(list_dir(&d, Kind::File, &[], false).unwrap());
        assert_eq!(got.len(), 4);
        assert!(list_dir(&d.join("missing"), Kind::File, &[], false).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_save_gets_its_extension() {
        assert_eq!(with_ext("profile", &["dchub"]), "profile.dchub");
        assert_eq!(with_ext("p.DCHUB ", &["dchub"]), "p.DCHUB");
        assert_eq!(
            with_ext("theme.dzch-theme", &["json"]),
            "theme.dzch-theme.json"
        );
        assert_eq!(with_ext("x", &[]), "x");
        assert!(valid_name("a.json"));
        assert!(!valid_name(" "));
        assert!(!valid_name("../a"));
        assert!(!valid_name(".."));
    }

    #[test]
    fn home_comes_first() {
        let d = temp();
        let home = d.join("home");
        let media = d.join("media");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(media.join("SDCARD")).unwrap();
        let steam = d.join("SteamLibrary").to_string_lossy().into_owned();
        let r = roots_in(
            Some(home.clone()),
            vec![steam.clone()],
            std::slice::from_ref(&media),
        );
        assert_eq!(r.first().map(|r| r.path.clone()), Some(home));
        assert!(r.first().map(|r| r.icon) == Some(lucide::HOUSE));
        assert!(r.iter().any(|r| r.path == Path::new(&steam)));
        #[cfg(not(windows))]
        assert!(r.iter().any(|r| r.path == media.join("SDCARD")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_save_finds_the_file_it_would_replace() {
        let e = vec![
            Entry {
                name: "maps".into(),
                dir: true,
            },
            Entry {
                name: "theme.json".into(),
                dir: false,
            },
        ];
        assert!(entry(&e, "theme.json").is_some_and(|e| !e.dir));
        assert!(entry(&e, "maps").is_some_and(|e| e.dir));
        assert!(entry(&e, "other.json").is_none());
    }
}
