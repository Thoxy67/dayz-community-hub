//! The SVG icons of the Svelte interface (lucide, game-icons, boxicons, circle-flags).
//!
//! The SVG files live in `assets/icons/<set>/<name>.svg` and the constants that embed them
//! in `icons/list.rs`; both are written by `scripts/icons.py`, which rescans `apps/gui/src`.
//! An icon is rasterised with resvg once per (icon, pixel size), uploaded as a texture and
//! cached in the context's data; monochrome icons are white and take their colour from the
//! mesh tint, flags (`colored`) are drawn as they are.

mod list;

pub use list::{bx, flag, game, lucide};

use std::collections::HashMap;
use std::sync::Arc;

use egui::mutex::Mutex;
use egui::{Color32, Rect, Vec2};
use resvg::{tiny_skia, usvg};

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
        let Some(entry) = texture(ctx, *self, px) else {
            return;
        };
        let fitted = fit(entry.aspect, rect);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        let tint = if self.colored { Color32::WHITE } else { tint };
        painter.image(entry.texture.id(), fitted, uv, tint);
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

#[derive(Clone)]
struct Entry {
    texture: egui::TextureHandle,
    aspect: f32,
}

#[derive(Default)]
struct Cache {
    textures: HashMap<(&'static str, u32), Entry>,
    /// Icons whose SVG failed: logged once, never retried.
    broken: HashMap<&'static str, ()>,
}

type SharedCache = Arc<Mutex<Cache>>;

fn cache(ctx: &egui::Context) -> SharedCache {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<SharedCache>(egui::Id::new("dzch-icon-cache"))
            .clone()
    })
}

fn texture(ctx: &egui::Context, icon: Icon, px: u32) -> Option<Entry> {
    let cache = cache(ctx);
    let mut cache = cache.lock();
    if let Some(entry) = cache.textures.get(&(icon.name, px)) {
        return Some(entry.clone());
    }
    if cache.broken.contains_key(icon.name) {
        return None;
    }
    match rasterise(icon.svg, px) {
        Ok((image, aspect)) => {
            let name = format!("icon:{}@{px}", icon.name);
            let texture = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
            let entry = Entry { texture, aspect };
            cache.textures.insert((icon.name, px), entry.clone());
            Some(entry)
        }
        Err(e) => {
            eprintln!("icon {}: {e}", icon.name);
            cache.broken.insert(icon.name, ());
            None
        }
    }
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
        assert!(cache.broken.contains_key("test/broken"));
        assert_eq!(cache.textures.len(), 1);
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
