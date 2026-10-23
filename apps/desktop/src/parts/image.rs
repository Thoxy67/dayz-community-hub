//! Pictures the window shows but does not ship: the Steam avatar (a `data:` URI from
//! the hub), friends' avatars (https URLs), news images (files in the hub's cache).
//!
//! [`texture`] answers at once: the picture when it is ready, `None` while it loads
//! (or when it failed: a picture that cannot be had is not asked for again). Fetching
//! and decoding run on the runtime, never on the frame; the upload goes straight to
//! egui's texture manager from there and the window repaints once it is in.
//!
//! A picture is decoded at the size it is drawn: [`paint`] works the size out from its
//! box ([`side_for`]), so an 80 px thumbnail is not a 1024 px texture sampled down
//! (egui makes no mipmaps). [`texture`] gives the full size, for the reader and the
//! zoom. Textures are kept by source and size, a few hundred at most; past that the
//! ones not drawn for the longest go first, never one on screen.
//!
//! Web pictures share one client (pooled connections, timeouts), a few at a time.
//! Steam avatars are named by their hash and served to be kept for years, so they are
//! also kept on disk (`<app data>/cache/avatars`): a later start asks Steam for none
//! of the pictures it already has.
//!
//! A long source (the avatar's `data:` URI, kilobytes) is better kept by a key its
//! owner works out once ([`texture_keyed`], [`paint_keyed`]): the frame then hashes a
//! number, not the whole string.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use base64::Engine;
use egui::mutex::Mutex;
use egui::{Context, CornerRadius, Painter, Rect, TextureHandle, TextureId, TextureOptions, Vec2};
use rustc_hash::{FxHashMap, FxHashSet};

/// Pictures kept before the ones not drawn lately go.
const KEEP: usize = 384;
/// What an eviction brings the cache down to, so it does not run again at once.
const KEEP_AFTER: usize = KEEP * 3 / 4;
/// Passes a picture counts as on screen after it was last drawn: never evicted then.
const RECENT: u64 = 2;
/// Sources known to fail, kept so they are not asked for again (forgotten past this).
const FAILED_KEEP: usize = 1024;
/// Bytes a picture may have, and the longest side it is decoded at.
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 1024;
/// The smallest side a picture is decoded at: sizes are powers of two from here, so a
/// zoom step does not decode every picture again.
const MIN_SIDE: u32 = 64;
/// Web pictures downloaded at once (a friends list asks for a hundred together).
const WEB_AT_ONCE: usize = 6;
/// Avatars kept on disk, and what a prune brings them down to.
const DISK_KEEP: usize = 1000;
const DISK_KEEP_AFTER: usize = 800;

/// A picture that is in: its texture and size, both plain values (the cache owns the
/// texture for as long as it keeps the picture, and that outlives the frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pic {
    id: TextureId,
    size: [usize; 2],
}

impl Pic {
    pub fn id(&self) -> TextureId {
        self.id
    }

    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    /// The picture as egui's image widgets take it.
    pub fn sized(&self) -> egui::load::SizedTexture {
        egui::load::SizedTexture::new(self.id, self.size.map(|v| v as f32))
    }
}

/// A loaded picture: the handle keeps the texture alive, the [`Pic`] is what is handed out.
struct Loaded {
    _handle: TextureHandle,
    pic: Pic,
}

/// One kept picture: `None` while it loads.
struct Slot<T> {
    value: Option<T>,
    used: u64,
}

/// What the cache knows of a key.
#[derive(Debug, PartialEq, Eq)]
enum Look<'a, T> {
    Ready(&'a T),
    Busy,
    Missing,
}

/// Pictures by key, evicted least recently drawn first; failures apart, so they are not
/// asked for again while the cache turns over.
struct Slots<T> {
    map: FxHashMap<u64, Slot<T>>,
    failed: FxHashSet<u64>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self {
            map: FxHashMap::default(),
            failed: FxHashSet::default(),
        }
    }
}

impl<T> Slots<T> {
    /// The picture at `key`, marked as drawn on `pass`.
    fn look(&mut self, key: u64, pass: u64) -> Look<'_, T> {
        if self.failed.contains(&key) {
            return Look::Busy;
        }
        match self.map.get_mut(&key) {
            Some(slot) => {
                slot.used = pass;
                slot.value.as_ref().map_or(Look::Busy, Look::Ready)
            }
            None => Look::Missing,
        }
    }

    /// `key` starts loading; makes room first when the cache is full.
    fn start(&mut self, key: u64, pass: u64) {
        if self.map.len() >= KEEP {
            self.evict(pass);
        }
        self.map.insert(
            key,
            Slot {
                value: None,
                used: pass,
            },
        );
    }

    /// `key` is in (`Some`) or will not come (`None`).
    fn finish(&mut self, key: u64, value: Option<T>) {
        match value {
            Some(v) => {
                if let Some(slot) = self.map.get_mut(&key) {
                    slot.value = Some(v);
                }
            }
            None => {
                self.map.remove(&key);
                if self.failed.len() >= FAILED_KEEP {
                    self.failed.clear();
                }
                self.failed.insert(key);
            }
        }
    }

    /// Drops the pictures drawn longest ago, down to [`KEEP_AFTER`]; never one still
    /// loading or drawn in the last [`RECENT`] passes (the cache grows past [`KEEP`]
    /// rather than drop a picture on screen).
    fn evict(&mut self, pass: u64) {
        let mut old: Vec<(u64, u64)> = self
            .map
            .iter()
            .filter(|(_, s)| s.value.is_some() && s.used.saturating_add(RECENT) < pass)
            .map(|(k, s)| (s.used, *k))
            .collect();
        old.sort_unstable();
        let extra = self.map.len().saturating_sub(KEEP_AFTER);
        for (_, k) in old.into_iter().take(extra) {
            self.map.remove(&k);
        }
    }
}

type Cache = Arc<Mutex<Slots<Loaded>>>;

fn cache(ctx: &Context) -> Cache {
    let id = egui::Id::new("dzch.images");
    // A read lock on the context in the common case; the write lock only the first time.
    if let Some(c) = ctx.data(|d| d.get_temp::<Cache>(id)) {
        return c;
    }
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Cache>(id).clone())
}

/// The side to decode a picture at to fill a box of `size` points: twice its longer
/// side in pixels (an `object-cover` crop of a wide picture needs the room), rounded up
/// to a power of two, within [`MIN_SIDE`]..=[`MAX_SIDE`].
pub fn side_for(ctx: &Context, size: Vec2) -> u32 {
    side_px(size.max_elem() * ctx.pixels_per_point())
}

fn side_px(px: f32) -> u32 {
    if !px.is_finite() || px <= 0.0 {
        return MIN_SIDE;
    }
    let want = (px * 2.0).ceil().min(MAX_SIDE as f32) as u32;
    want.clamp(MIN_SIDE, MAX_SIDE)
        .next_power_of_two()
        .min(MAX_SIDE)
}

/// The cache key of a picture at a side.
fn sized(key: u64, side: u32) -> u64 {
    key_of(&(key, side))
}

/// The picture at `src` (`data:` URI, `http(s)://` URL or a file path) at full size,
/// once it is in.
pub fn texture(ctx: &Context, src: &str) -> Option<Pic> {
    texture_keyed(ctx, key_of(src), src, MAX_SIDE)
}

/// The picture at `src` decoded at `side` ([`side_for`]), kept by `key` (a hash of
/// `src` its owner worked out once, [`key_of`]): the frame then does not hash `src`.
/// A new key is a new picture.
pub fn texture_keyed(ctx: &Context, key: u64, src: &str, side: u32) -> Option<Pic> {
    if src.is_empty() {
        return None;
    }
    let key = sized(key, side);
    let pass = ctx.cumulative_pass_nr();
    let cache = cache(ctx);
    let mut store = cache.lock();
    match store.look(key, pass) {
        Look::Ready(l) => return Some(l.pic),
        Look::Busy => return None,
        Look::Missing => {}
    }
    store.start(key, pass);
    drop(store);
    let (ctx, src, cache) = (ctx.clone(), src.to_owned(), Arc::clone(&cache));
    crate::rt::fire(async move {
        let loaded = fetch(&ctx, &src, side).await;
        cache.lock().finish(key, loaded);
    });
    None
}

/// A key for [`texture_keyed`]: worked out once per source, by its owner.
pub fn key_of<T: std::hash::Hash + ?Sized>(src: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    src.hash(&mut h);
    h.finish()
}

async fn fetch(ctx: &Context, src: &str, side: u32) -> Option<Loaded> {
    match load(src, side).await {
        Ok(img) => {
            let size = img.size;
            let handle = ctx.load_texture(short(src), img, TextureOptions::LINEAR);
            Some(Loaded {
                pic: Pic {
                    id: handle.id(),
                    size,
                },
                _handle: handle,
            })
        }
        Err(e) => {
            eprintln!("image {}: {e}", short(src));
            None
        }
    }
}

/// Draw the picture at `src` into `rect`, cropped to fill it, rounded. False while
/// it is not there (the caller draws its placeholder).
pub fn paint(painter: &Painter, rect: Rect, src: &str, radius: impl Into<CornerRadius>) -> bool {
    paint_keyed(painter, rect, key_of(src), src, radius)
}

/// [`paint`], for a picture kept by key ([`texture_keyed`]).
pub fn paint_keyed(
    painter: &Painter,
    rect: Rect,
    key: u64,
    src: &str,
    radius: impl Into<CornerRadius>,
) -> bool {
    let side = side_for(painter.ctx(), rect.size());
    match texture_keyed(painter.ctx(), key, src, side) {
        Some(pic) => paint_texture(painter, rect, pic, radius),
        None => false,
    }
}

fn paint_texture(painter: &Painter, rect: Rect, pic: Pic, radius: impl Into<CornerRadius>) -> bool {
    let [w, h] = pic.size.map(|v| v.max(1) as f32);
    // `object-cover`: the middle of the picture, at the box's proportions.
    let (bw, bh) = (rect.width().max(1.0), rect.height().max(1.0));
    let scale = (bw / w).max(bh / h);
    let (uw, uh) = ((bw / scale) / w, (bh / scale) / h);
    let uv = Rect::from_center_size(egui::pos2(0.5, 0.5), egui::vec2(uw, uh));
    painter.add(
        egui::epaint::RectShape::filled(rect, radius, egui::Color32::WHITE)
            .with_texture(pic.id, uv),
    );
    true
}

async fn load(src: &str, side: u32) -> Result<egui::ColorImage, String> {
    let (bytes, keep) = if let Some(rest) = src.strip_prefix("data:") {
        let (_, data) = rest.split_once(";base64,").ok_or("not a base64 data URI")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|e| e.to_string())?;
        (bytes, None)
    } else if src.starts_with("https://") || src.starts_with("http://") {
        web(src).await?
    } else {
        (read_file(Path::new(src)).await?, None)
    };
    if bytes.len() > MAX_BYTES {
        return Err("too large".into());
    }
    let (img, bytes) = tokio::task::spawn_blocking(move || (decode(&bytes, side), bytes))
        .await
        .map_err(|e| e.to_string())?;
    let img = img?;
    // Kept only once it decodes: the disk never holds a page of HTML named like an avatar.
    if let Some(path) = keep {
        keep_on_disk(path, bytes).await;
    }
    Ok(img)
}

async fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let meta = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
    if meta.len() > MAX_BYTES as u64 {
        return Err("too large".into());
    }
    tokio::fs::read(path).await.map_err(|e| e.to_string())
}

/// A web picture's bytes: from the disk for an avatar already kept, else downloaded.
/// The path is where to keep it once it decodes (avatars only).
async fn web(url: &str) -> Result<(Vec<u8>, Option<PathBuf>), String> {
    let path = avatar_file(url).and_then(|name| Some(avatars_dir()?.join(name)));
    if let Some(p) = &path
        && let Ok(bytes) = read_file(p).await
    {
        return Ok((bytes, None));
    }
    static GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(WEB_AT_ONCE);
    // The gate is never closed; without a permit the download simply goes ahead.
    let _permit = GATE.acquire().await.ok();
    // The hub's client for pictures: one pool of connections, and a host that stops
    // answering fails the picture instead of leaving it loading for the session.
    let mut resp = dz_hub::net::pictures()
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let len = resp.content_length().unwrap_or(0);
    if len > MAX_BYTES as u64 {
        return Err("too large".into());
    }
    // Read in chunks, so a body without a length stops at the cap instead of after it.
    let mut out = Vec::with_capacity(len as usize);
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        push_capped(&mut out, &chunk)?;
    }
    Ok((out, path))
}

/// Appends `chunk`, or fails once the picture would pass [`MAX_BYTES`].
fn push_capped(out: &mut Vec<u8>, chunk: &[u8]) -> Result<(), String> {
    if out.len() + chunk.len() > MAX_BYTES {
        return Err("too large".into());
    }
    out.extend_from_slice(chunk);
    Ok(())
}

/// The file name a Steam avatar is kept under: its own (`<sha1>_full.jpg`), when `url`
/// is one. Steam names an avatar by the hash of the picture, so a kept file is never
/// stale; anything else on the web is not kept.
fn avatar_file(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let host = host.to_ascii_lowercase();
    if !(host.starts_with("avatars") && host.ends_with(".steamstatic.com")) {
        return None;
    }
    let name = path.split(['?', '#']).next()?.rsplit('/').next()?;
    let hash = name.split(['_', '.']).next()?;
    let plain = name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    (hash.len() == 40
        && hash.bytes().all(|b| b.is_ascii_hexdigit())
        && plain
        && name.len() <= 64
        && matches!(ext.as_deref(), Some("jpg" | "jpeg" | "png" | "gif")))
    .then(|| name.to_ascii_lowercase())
}

/// `<app data>/cache/avatars`, beside the news pictures' cache.
fn avatars_dir() -> Option<PathBuf> {
    let images = dz_hub::paths::images_cache_dir().ok()?;
    Some(images.parent()?.join("avatars"))
}

/// Writes a downloaded avatar to `path` (through a `.part` file, so a half-written one
/// is never read), and prunes the folder once per run. Failing only costs a download.
async fn keep_on_disk(path: PathBuf, bytes: Vec<u8>) {
    let _ = tokio::task::spawn_blocking(move || {
        let dir = path.parent()?;
        std::fs::create_dir_all(dir).ok()?;
        let part = path.with_extension("part");
        std::fs::write(&part, &bytes).ok()?;
        if std::fs::rename(&part, &path).is_err() {
            let _ = std::fs::remove_file(&part);
        }
        static PRUNED: OnceLock<()> = OnceLock::new();
        if PRUNED.set(()).is_ok() {
            prune(dir);
        }
        Some(())
    })
    .await;
}

/// Brings the avatars folder down to [`DISK_KEEP_AFTER`] files once it holds more than
/// [`DISK_KEEP`], the oldest first.
fn prune(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            meta.is_file()
                .then(|| (meta.modified().unwrap_or(std::time::UNIX_EPOCH), e.path()))
        })
        .collect();
    if files.len() <= DISK_KEEP {
        return;
    }
    files.sort_unstable();
    let extra = files.len() - DISK_KEEP_AFTER;
    for (_, p) in files.into_iter().take(extra) {
        let _ = std::fs::remove_file(p);
    }
}

fn decode(bytes: &[u8], side: u32) -> Result<egui::ColorImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let side = side.clamp(1, MAX_SIDE);
    let img = if img.width().max(img.height()) > side {
        img.thumbnail(side, side)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

/// A source short enough for a log line (a `data:` URI is long).
fn short(src: &str) -> &str {
    let end = src.char_indices().nth(64).map_or(src.len(), |(i, _)| i);
    &src[..end]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn decodes_a_png_data_uri() {
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png(1, 1))
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let img = rt.block_on(load(&uri, MAX_SIDE)).unwrap();
        assert_eq!(img.size, [1, 1]);
        assert_eq!(img.pixels[0], egui::Color32::RED);
    }

    #[test]
    fn decodes_at_the_side_asked_for() {
        let bytes = png(400, 200);
        assert_eq!(decode(&bytes, 128).unwrap().size, [128, 64]);
        // Never larger than the picture is.
        assert_eq!(decode(&bytes, MAX_SIDE).unwrap().size, [400, 200]);
    }

    #[test]
    fn sides_are_powers_of_two_within_bounds() {
        // An 80 × 56 thumbnail at 1× and 2×.
        assert_eq!(side_px(80.0), 256);
        assert_eq!(side_px(160.0), 512);
        assert_eq!(side_px(1.0), MIN_SIDE);
        assert_eq!(side_px(5000.0), MAX_SIDE);
        assert_eq!(side_px(f32::NAN), MIN_SIDE);
        assert_ne!(sized(1, 256), sized(1, 512));
    }

    #[test]
    fn junk_fails_without_panicking() {
        assert!(decode(b"not a picture", MAX_SIDE).is_err());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(load("data:image/png;base64,@@@", 64)).is_err());
        assert!(rt.block_on(load("data:nope", 64)).is_err());
        assert_eq!(short(&"é".repeat(100)).chars().count(), 64);
    }

    #[test]
    fn a_body_stops_at_the_cap() {
        let mut out = Vec::new();
        push_capped(&mut out, &vec![0; MAX_BYTES - 1]).unwrap();
        push_capped(&mut out, &[0]).unwrap();
        assert!(push_capped(&mut out, &[0]).is_err());
        assert_eq!(out.len(), MAX_BYTES);
    }

    #[test]
    fn only_steam_avatars_are_kept_on_disk() {
        let h = "fef49e7fa7e1997310d705b2a6158ff8dc1cdfeb";
        assert_eq!(
            avatar_file(&format!("https://avatars.steamstatic.com/{h}_full.jpg")).as_deref(),
            Some(format!("{h}_full.jpg").as_str())
        );
        assert!(avatar_file(&format!("https://avatars.akamai.steamstatic.com/{h}.jpg")).is_some());
        assert!(
            avatar_file(&format!(
                "https://AVATARS.steamstatic.com/{h}_medium.png?x=1"
            ))
            .is_some()
        );
        // Another host, a name that is not a hash, a path that climbs.
        assert!(avatar_file(&format!("https://example.com/{h}_full.jpg")).is_none());
        assert!(avatar_file("https://avatars.steamstatic.com/avatar_full.jpg").is_none());
        assert!(avatar_file(&format!("https://avatars.steamstatic.com/{h}..%2F.jpg")).is_none());
        assert!(avatar_file(&format!("https://avatars.steamstatic.com/{h}_full.html")).is_none());
        assert!(avatar_file("data:image/png;base64,AAAA").is_none());
    }

    #[test]
    fn the_cache_drops_what_was_drawn_longest_ago() {
        let mut s = Slots::<u64>::default();
        for k in 0..KEEP as u64 {
            s.start(k, k);
            s.finish(k, Some(k));
        }
        // Key 0 is drawn again late: it stays; 1 and 2 are the oldest and go.
        assert_eq!(s.look(0, 1000), Look::Ready(&0));
        s.start(9999, 1000);
        assert_eq!(s.map.len(), KEEP_AFTER + 1);
        assert_eq!(s.look(0, 1000), Look::Ready(&0));
        assert_eq!(s.look(1, 1000), Look::Missing);
        assert_eq!(s.look(2, 1000), Look::Missing);
        assert_eq!(
            s.look(KEEP as u64 - 1, 1000),
            Look::Ready(&(KEEP as u64 - 1))
        );
        assert_eq!(s.look(9999, 1000), Look::Busy);
    }

    #[test]
    fn the_cache_keeps_what_is_on_screen_and_loading() {
        let mut s = Slots::<u64>::default();
        for k in 0..KEEP as u64 {
            s.start(k, 10);
            if k % 2 == 0 {
                s.finish(k, Some(k));
            }
        }
        // Everything was drawn this pass (or is loading): nothing is dropped.
        s.start(9999, 11);
        assert_eq!(s.map.len(), KEEP + 1);
    }

    #[test]
    fn a_failure_is_not_asked_for_again() {
        let mut s = Slots::<u64>::default();
        s.start(7, 1);
        s.finish(7, None);
        assert_eq!(s.look(7, 2), Look::Busy);
        assert!(s.map.is_empty());
    }
}
