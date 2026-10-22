//! The mock's news: the world's articles, and two switches for states its world does not
//! show by itself (`DZCH_MOCK=news-blocked`, `DZCH_MOCK=news-images`).
//!
//! - `news-blocked`: `fetch_news` fails as when Cloudflare turns the request away.
//! - `news-images`: the articles carry pictures (a hero each, one inline picture in the
//!   body), and `fetch_image` paints them into the mock's data directory: invented
//!   gradients written as uncompressed PNGs, so nothing goes over the network.
//!
//! Every body is the same short text; the first article (an update) also has patch
//! notes for two platforms.

use std::path::PathBuf;

use crate::features::news::{ArticleBodyDto, ArticleDto, BLOCKED_PREFIX, PatchNotesDto};

use super::{World, switch};

/// Where the mock's pictures pretend to come from.
const IMAGE_BASE: &str = "https://mock.invalid/news/";
const IMAGE_W: u32 = 480;
const IMAGE_H: u32 = 270;

impl World {
    /// `fetch_news` in the mock.
    pub(crate) fn news(&self) -> Result<Vec<ArticleDto>, String> {
        if switch("news-blocked") {
            return Err(format!(
                "{BLOCKED_PREFIX} dayz.com asked for a browser check (Cloudflare)"
            ));
        }
        if !switch("news-images") {
            return Ok(self.articles.clone());
        }
        Ok(self
            .articles
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let mut a = a.clone();
                a.image_url = Some(format!("{IMAGE_BASE}{i}.png"));
                a
            })
            .collect())
    }

    /// `fetch_article` in the mock.
    pub(crate) fn article(&self, slug: &str) -> Result<ArticleBodyDto, String> {
        let Some(i) = self.articles.iter().position(|a| a.slug == slug) else {
            return Err(format!("dayz.com has no /article/{slug}"));
        };
        let mut html = "<p>Survivors, a new update brings changes to vehicles, base building and the economy.</p><h2>Vehicles</h2><p>Handling was reworked on every terrain type.</p>".to_owned();
        if switch("news-images") {
            html.push_str(&format!(
                r#"<p><img src="{IMAGE_BASE}{i}-inline.png" alt=""></p><p>Read the <a href="/article/updates/notes">full notes</a>.</p>"#
            ));
        }
        let notes = if i == 0 {
            ["Steam", "Xbox/Microsoft Store"]
                .into_iter()
                .map(|platform| PatchNotesDto {
                    platform: platform.to_owned(),
                    html: format!("<p><strong>{platform} Stable 1.28 - Version 1.28.160000</strong></p><p><strong>FIXED</strong></p><ul><li><p>Vehicles could clip through fences</p></li><li><p>A rare crash when opening the map</p></li></ul><p><strong>CHANGED</strong></p><ul><li><p>Wolves hear footsteps from further away</p></li></ul>"),
                })
                .collect()
        } else {
            Vec::new()
        };
        Ok(ArticleBodyDto { html, notes })
    }

    /// `fetch_image` in the mock: a picture painted into the data directory.
    pub(crate) async fn news_image(&self, url: &str) -> Result<PathBuf, String> {
        let Some(name) = url
            .strip_prefix(IMAGE_BASE)
            .filter(|_| switch("news-images"))
        else {
            return Err(format!("The mock world has no images: {url}"));
        };
        let seed = name
            .bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        let path = self.data_dir().join(format!("news-{seed:08x}.png"));
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }
        let png = tokio::task::spawn_blocking(move || picture(seed))
            .await
            .map_err(|e| e.to_string())?;
        tokio::fs::write(&path, png)
            .await
            .map_err(|e| format!("Mock image {}: {e}", path.display()))?;
        Ok(path)
    }
}

/// An invented picture: a dusk gradient with a darker ridge, tinted by `seed`.
fn picture(seed: u32) -> Vec<u8> {
    let tint = |shift: u32| (seed >> shift) as u8 / 4;
    let (tr, tg, tb) = (tint(0), tint(8), tint(16));
    let mut raw = Vec::with_capacity(((IMAGE_W * 3 + 1) * IMAGE_H) as usize);
    for y in 0..IMAGE_H {
        raw.push(0); // filter: none
        let ridge = IMAGE_H as f32 * 0.62;
        for x in 0..IMAGE_W {
            let fx = x as f32 / IMAGE_W as f32;
            let fy = y as f32 / IMAGE_H as f32;
            let hill = ridge + (fx * 9.0 + seed as f32 * 0.001).sin() * 18.0;
            let ground = y as f32 > hill;
            let (r, g, b) = if ground {
                (30.0 + fx * 20.0, 34.0 + fx * 10.0, 28.0)
            } else {
                (200.0 - fy * 140.0, 140.0 - fy * 90.0, 90.0 - fy * 40.0)
            };
            raw.push((r as u8).saturating_add(tr));
            raw.push((g as u8).saturating_add(tg));
            raw.push((b as u8).saturating_add(tb));
        }
    }
    png_rgb(IMAGE_W, IMAGE_H, &raw)
}

/// A PNG of filtered RGB scanlines, deflated with stored blocks (no compression).
fn png_rgb(w: u32, h: u32, raw: &[u8]) -> Vec<u8> {
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65_535).peekable();
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        let len = b.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(b);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = 0xffff_ffffu32;
    for &byte in kind.iter().chain(data) {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picture_is_a_png() {
        let png = picture(42);
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(png.ends_with(&[0xae, 0x42, 0x60, 0x82]), "IEND's CRC");
    }
}
