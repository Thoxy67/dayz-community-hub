//! Pictures the window shows but does not ship: the Steam avatar (a `data:` URI from
//! the hub), friends' avatars (https URLs), news images (files in the hub's cache).
//!
//! [`texture`] answers at once: the texture when it is ready, `None` while it loads
//! (or when it failed: a picture that cannot be had is not asked for again). Fetching
//! and decoding run on the runtime, never on the frame; the upload goes straight to
//! egui's texture manager from there and the window repaints once it is in. Textures
//! are kept by source, a few hundred at most.

use std::collections::HashMap;
use std::sync::Arc;

use base64::Engine;
use egui::mutex::Mutex;
use egui::{Context, CornerRadius, Painter, Rect, TextureHandle, TextureOptions};

/// Sources kept before the cache starts over.
const KEEP: usize = 384;
/// Bytes a picture may have, and the longest side it is drawn at.
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 1024;

enum Entry {
    Loading,
    Ready(TextureHandle),
    Failed,
}

type Cache = Arc<Mutex<HashMap<String, Entry>>>;

fn cache(ctx: &Context) -> Cache {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Cache>(egui::Id::new("dzch.images"))
            .clone()
    })
}

/// The picture at `src` (`data:` URI, `http(s)://` URL or a file path), once it is in.
pub fn texture(ctx: &Context, src: &str) -> Option<TextureHandle> {
    if src.is_empty() {
        return None;
    }
    let cache = cache(ctx);
    let mut map = cache.lock();
    match map.get(src) {
        Some(Entry::Ready(t)) => return Some(t.clone()),
        Some(Entry::Loading | Entry::Failed) => return None,
        None => {}
    }
    if map.len() >= KEEP {
        map.retain(|_, e| matches!(e, Entry::Loading));
    }
    map.insert(src.to_owned(), Entry::Loading);
    drop(map);
    let (ctx, src, cache) = (ctx.clone(), src.to_owned(), Arc::clone(&cache));
    crate::rt::fire(async move {
        let entry = match load(&src).await {
            Ok(img) => Entry::Ready(ctx.load_texture(&src, img, TextureOptions::LINEAR)),
            Err(e) => {
                eprintln!("image {}: {e}", short(&src));
                Entry::Failed
            }
        };
        cache.lock().insert(src, entry);
    });
    None
}

/// Draw the picture at `src` into `rect`, cropped to fill it, rounded. False while
/// it is not there (the caller draws its placeholder).
pub fn paint(painter: &Painter, rect: Rect, src: &str, radius: impl Into<CornerRadius>) -> bool {
    let Some(tex) = texture(painter.ctx(), src) else {
        return false;
    };
    let [w, h] = tex.size().map(|v| v.max(1) as f32);
    // `object-cover`: the middle of the picture, at the box's proportions.
    let (bw, bh) = (rect.width().max(1.0), rect.height().max(1.0));
    let scale = (bw / w).max(bh / h);
    let (uw, uh) = ((bw / scale) / w, (bh / scale) / h);
    let uv = Rect::from_center_size(egui::pos2(0.5, 0.5), egui::vec2(uw, uh));
    painter.add(
        egui::epaint::RectShape::filled(rect, radius, egui::Color32::WHITE)
            .with_texture(tex.id(), uv),
    );
    true
}

async fn load(src: &str) -> Result<egui::ColorImage, String> {
    let bytes = if let Some(rest) = src.strip_prefix("data:") {
        let (_, data) = rest.split_once(";base64,").ok_or("not a base64 data URI")?;
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|e| e.to_string())?
    } else if src.starts_with("https://") || src.starts_with("http://") {
        let resp = reqwest::get(src).await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        if resp.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
            return Err("too large".into());
        }
        resp.bytes().await.map_err(|e| e.to_string())?.to_vec()
    } else {
        let meta = tokio::fs::metadata(src).await.map_err(|e| e.to_string())?;
        if meta.len() > MAX_BYTES as u64 {
            return Err("too large".into());
        }
        tokio::fs::read(src).await.map_err(|e| e.to_string())?
    };
    if bytes.len() > MAX_BYTES {
        return Err("too large".into());
    }
    tokio::task::spawn_blocking(move || decode(&bytes))
        .await
        .map_err(|e| e.to_string())?
}

fn decode(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let img = if img.width().max(img.height()) > MAX_SIDE {
        img.thumbnail(MAX_SIDE, MAX_SIDE)
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

    #[test]
    fn decodes_a_png_data_uri() {
        // 1×1 red pixel.
        let png = {
            let mut out = Vec::new();
            let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
                .unwrap();
            out
        };
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&png)
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let img = rt.block_on(load(&uri)).unwrap();
        assert_eq!(img.size, [1, 1]);
        assert_eq!(img.pixels[0], egui::Color32::RED);
    }

    #[test]
    fn junk_fails_without_panicking() {
        assert!(decode(b"not a picture").is_err());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(rt.block_on(load("data:image/png;base64,@@@")).is_err());
        assert!(rt.block_on(load("data:nope")).is_err());
        assert_eq!(short(&"é".repeat(100)).chars().count(), 64);
    }
}
