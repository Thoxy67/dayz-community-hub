//! The SVG icons (lucide, game-icons, boxicons, circle-flags), the ones the Svelte interface used.
//!
//! The SVG files live in `assets/icons/<set>/<name>.svg` and the constants that embed them
//! in `icons/list.rs`. The folder is the source: drop an SVG there and run
//! `python3 scripts/icons.py`, which rewrites `list.rs` (a test fails when they disagree).
//! An icon is rasterised with resvg once per (icon, pixel size), uploaded as a texture and
//! cached in the context's data; monochrome icons are white and take their colour from the
//! mesh tint, flags (`colored`) are drawn as they are. The common icons are rasterised
//! ahead on a worker ([`prewarm`]), so the first frames only upload them; sizes left unused
//! for a while (an old zoom) are let go.

mod list;

pub use list::{bx, flag, game, lucide};

use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, Rect, TextureId, Vec2};
use resvg::{tiny_skia, usvg};
use rustc_hash::{FxHashMap, FxHashSet};

/// One icon: its SVG and how it is coloured.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Icon {
    /// `<set>/<name>`, as in the interface's `~icons/<set>/<name>` imports.
    pub name: &'static str,
    svg: &'static [u8],
    /// Drawn with its own colours, never tinted (flags).
    pub colored: bool,
}

impl std::fmt::Debug for Icon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Icon")
            .field("name", &self.name)
            .field("colored", &self.colored)
            .finish()
    }
}

/// The largest side, in pixels, an icon is rasterised at.
const MAX_PX: u32 = 1024;
/// Past this many textures, those not drawn for [`UNUSED_PASSES`] are let go.
const KEEP: usize = 384;
const UNUSED_PASSES: u64 = 600;

impl Icon {
    const fn new(name: &'static str, svg: &'static [u8], colored: bool) -> Self {
        Self { name, svg, colored }
    }

    /// The SVG source.
    pub fn svg(&self) -> &'static [u8] {
        self.svg
    }

    /// Draws the icon fitted (aspect kept, centred) in `rect`. `tint` colours monochrome
    /// icons; colored icons ignore it. A broken SVG draws nothing.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, tint: Color32) {
        if !rect.is_positive() || !rect.is_finite() {
            return;
        }
        let ctx = painter.ctx();
        let ppp = ctx.pixels_per_point();
        let side = (rect.width().max(rect.height()) * ppp).round();
        let px = if side.is_finite() {
            (side as u32).clamp(1, MAX_PX)
        } else {
            1
        };
        let Some((id, aspect)) = texture(ctx, *self, px) else {
            return;
        };
        let fitted = fit(aspect, rect);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let tint = if self.colored { Color32::WHITE } else { tint };
        painter.image(id, fitted, uv, tint);
    }

    /// Allocates a `size`×`size` square in `ui` and draws the icon in it.
    pub fn show(&self, ui: &mut egui::Ui, size: f32, tint: Color32) -> egui::Response {
        let size = if size.is_finite() { size.max(0.0) } else { 0.0 };
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, tint);
        }
        response
    }
}

/// `aspect` (width / height) fitted and centred in `rect`.
fn fit(aspect: f32, rect: Rect) -> Rect {
    let (w, h) = (rect.width(), rect.height());
    let size = if aspect >= w / h {
        Vec2::new(w, w / aspect)
    } else {
        Vec2::new(h * aspect, h)
    };
    Rect::from_center_size(rect.center(), size)
}

/// An icon's texture at one pixel size. The handle keeps it alive; the frame draws by its
/// id (cloning and dropping a handle each take the texture manager's write lock).
struct Slot {
    _texture: egui::TextureHandle,
    id: TextureId,
    aspect: f32,
    /// The pass it was last drawn in.
    used: u64,
}

/// By the icon's name and the pixel size. Not by the SVG's address: a constant's bytes
/// may sit at a different address in each place it is used.
type Key = (&'static str, u32);

fn key(icon: Icon, px: u32) -> Key {
    (icon.name, px)
}

#[derive(Default)]
struct Cache {
    textures: FxHashMap<Key, Slot>,
    /// Rasterised ahead by [`prewarm`], uploaded when first drawn.
    ready: FxHashMap<Key, (egui::ColorImage, f32)>,
    /// Icons whose SVG failed: logged once, never retried.
    broken: FxHashSet<&'static str>,
    /// The pass of the last sweep of unused sizes.
    swept: u64,
}

type SharedCache = Arc<Mutex<Cache>>;

fn cache(ctx: &egui::Context) -> SharedCache {
    let id = egui::Id::unique("dzch-icon-cache");
    // A read lock on the context once the cache exists; the write lock only to make it.
    ctx.data(|d| d.get_temp::<SharedCache>(id))
        .unwrap_or_else(|| ctx.data_mut(|d| d.get_temp_mut_or_default::<SharedCache>(id).clone()))
}

fn texture(ctx: &egui::Context, icon: Icon, px: u32) -> Option<(TextureId, f32)> {
    let k = key(icon, px);
    let pass = ctx.cumulative_pass_nr();
    let cache = cache(ctx);
    let mut cache = cache.lock();
    if let Some(slot) = cache.textures.get_mut(&k) {
        slot.used = pass;
        return Some((slot.id, slot.aspect));
    }
    if cache.broken.contains(&k.0) {
        return None;
    }
    let made = match cache.ready.remove(&k) {
        Some(ready) => Ok(ready),
        None => rasterise(icon.svg, px),
    };
    match made {
        Ok((image, aspect)) => {
            let name = format!("icon:{}@{px}", icon.name);
            let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
            let id = texture.id();
            if cache.textures.len() >= KEEP && cache.swept != pass {
                cache.swept = pass;
                cache.textures.retain(|_, s| s.used + UNUSED_PASSES >= pass);
            }
            cache.textures.insert(
                k,
                Slot {
                    _texture: texture,
                    id,
                    aspect,
                    used: pass,
                },
            );
            Some((id, aspect))
        }
        Err(e) => {
            eprintln!("icon {}: {e}", icon.name);
            cache.broken.insert(k.0);
            None
        }
    }
}

/// Rasterise `icons` at each of `sizes` (points, at the context's current scale) on a
/// worker thread, so drawing them first only uploads them. Never waits: the frame that
/// draws one before the worker reached it rasterises it itself.
pub fn prewarm(
    ctx: &egui::Context,
    icons: &'static [Icon],
    sizes: &[f32],
) -> Option<std::thread::JoinHandle<()>> {
    let ppp = ctx.pixels_per_point();
    let pxs: Vec<u32> = sizes
        .iter()
        .map(|s| (s * ppp).round())
        .filter(|s| s.is_finite())
        .map(|s| (s as u32).clamp(1, MAX_PX))
        .collect();
    let cache = cache(ctx);
    std::thread::Builder::new()
        .name("icons".into())
        .spawn(move || {
            for &icon in icons {
                for &px in &pxs {
                    let k = key(icon, px);
                    {
                        let c = cache.lock();
                        if c.textures.contains_key(&k) || c.broken.contains(&k.0) {
                            continue;
                        }
                    }
                    // Rasterised outside the lock: the frame never waits on it.
                    if let Ok(ready) = rasterise(icon.svg, px) {
                        let mut c = cache.lock();
                        if !c.textures.contains_key(&k) {
                            c.ready.insert(k, ready);
                        }
                    }
                }
            }
        })
        .ok()
}

/// The SVG rendered so that its longest side is `px` pixels, and its aspect ratio.
fn rasterise(svg: &[u8], px: u32) -> Result<(egui::ColorImage, f32), String> {
    let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (sw, sh) = (size.width(), size.height());
    if !(sw > 0.0 && sh > 0.0) {
        return Err("empty size".into());
    }
    let scale = px as f32 / sw.max(sh);
    let w = ((sw * scale).round() as u32).clamp(1, MAX_PX);
    let h = ((sh * scale).round() as u32).clamp(1, MAX_PX);
    let mut pixmap = tiny_skia::Pixmap::new(w, h).ok_or("cannot allocate the pixmap")?;
    let transform = tiny_skia::Transform::from_scale(w as f32 / sw, h as f32 / sh);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let image = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pixmap.data());
    Ok((image, sw / sh))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> impl Iterator<Item = &'static Icon> {
        lucide::ALL
            .iter()
            .chain(game::ALL)
            .chain(bx::ALL)
            .chain(flag::ALL)
    }

    #[test]
    fn every_icon_parses() {
        let mut count = 0;
        for icon in all() {
            let tree = usvg::Tree::from_data(icon.svg(), &usvg::Options::default());
            assert!(
                tree.is_ok(),
                "{} does not parse: {:?}",
                icon.name,
                tree.err()
            );
            count += 1;
        }
        assert!(count > 400, "only {count} icons");
    }

    #[test]
    fn monochrome_icons_render_white() -> Result<(), String> {
        let (image, aspect) = rasterise(lucide::SERVER.svg(), 48)?;
        assert_eq!(image.size, [48, 48]);
        assert!((aspect - 1.0).abs() < f32::EPSILON);
        let opaque: Vec<_> = image.pixels.iter().filter(|p| p.a() == 255).collect();
        assert!(!opaque.is_empty());
        assert!(
            opaque
                .iter()
                .all(|p| p.r() == 255 && p.g() == 255 && p.b() == 255)
        );
        Ok(())
    }

    #[test]
    fn flags_by_code() {
        assert_eq!(flag::by_code("FR"), Some(flag::FR));
        assert_eq!(flag::by_code(" gb "), Some(flag::GB));
        assert_eq!(flag::by_code("xx-nope"), None);
        const { assert!(flag::FR.colored && !lucide::SERVER.colored) };
        assert!(flag::ALL.len() >= 250);
    }

    #[test]
    fn broken_svg_draws_nothing() {
        let broken = Icon::new("test/broken", b"<svg", false);
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            broken.show(ui, 16.0, Color32::RED);
            broken.show(ui, 16.0, Color32::RED);
            lucide::SERVER.show(ui, 16.0, Color32::RED);
        });
        out.textures_delta.clear();
        let cache = cache(&ctx);
        let cache = cache.lock();
        assert!(cache.broken.contains(&key(broken, 16).0));
        assert_eq!(cache.textures.len(), 1);
    }

    #[test]
    fn prewarmed_icons_are_uploaded_from_the_workers_image() -> Result<(), String> {
        let ctx = egui::Context::default();
        let icons: &'static [Icon] = &[lucide::SERVER, lucide::MAP];
        prewarm(&ctx, icons, &[16.0])
            .ok_or("the worker did not start")?
            .join()
            .map_err(|_| "the worker panicked")?;
        {
            let cache = cache(&ctx);
            let cache = cache.lock();
            assert_eq!(cache.ready.len(), 2);
            assert!(cache.textures.is_empty());
        }
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            lucide::SERVER.show(ui, 16.0, Color32::RED);
        });
        out.textures_delta.clear();
        let cache = cache(&ctx);
        let cache = cache.lock();
        assert_eq!(cache.ready.len(), 1, "the drawn one was taken");
        assert!(cache.textures.contains_key(&key(lucide::SERVER, 16)));
        Ok(())
    }

    /// `list.rs` names every SVG file of the icon folder, and nothing else (rerun
    /// `scripts/icons.py` after adding or removing one).
    #[test]
    fn list_matches_the_svg_folder() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let mut on_disk = Vec::new();
        for set in ["lucide", "game-icons", "boxicons", "circle-flags"] {
            let Ok(entries) = std::fs::read_dir(root.join(set)) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "svg")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                {
                    on_disk.push(format!("{set}/{stem}"));
                }
            }
        }
        on_disk.sort();
        let mut listed: Vec<String> = all().map(|i| i.name.to_string()).collect();
        listed.sort();
        assert_eq!(listed, on_disk, "run python3 scripts/icons.py");
    }

    #[test]
    fn fit_keeps_aspect() {
        let r = fit(
            2.0,
            Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::splat(10.0)),
        );
        assert_eq!(r.size(), Vec2::new(10.0, 5.0));
        assert_eq!(r.center(), egui::pos2(5.0, 5.0));
    }
}
