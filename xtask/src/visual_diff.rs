//! `cargo xtask visual-diff [views...]`: how close each egui view is to the Svelte interface.
//!
//! Compares `var/shots/<view>.png` (from `cargo xtask shots`) with `var/reference/<view>.png`
//! (the Svelte interface on its mock backend). With no view named, every view that has a
//! reference. Pure Rust, no external tools:
//!
//! - the shot is resized to the reference's size when they differ, both are turned to greyscale;
//! - **SSIM**: mean structural similarity over non-overlapping 8×8 windows (1.0 is identical);
//! - **diff px**: share of pixels whose grey levels differ by more than 32;
//! - `var/shots/<view>-diff.png`: the reference's ink in red, the shot's in blue, shared in black.
//!
//! The table is printed and written into `docs/ui-parity.md` between `<!-- visual-diff:start -->`
//! and `<!-- visual-diff:end -->` (markers appended when absent); nothing else in the file is
//! touched.
//!
//! `visual-diff --pair <shot.png> <reference.png> [diff.png]` compares two files and prints the
//! numbers, without touching the docs.

use std::path::{Path, PathBuf};

use image::{GrayImage, RgbImage, imageops::FilterType};

pub const START: &str = "<!-- visual-diff:start -->";
pub const END: &str = "<!-- visual-diff:end -->";
const DOC: &str = "docs/ui-parity.md";
/// Grey levels two pixels may differ by and still count as the same.
const TOLERANCE: u8 = 32;

/// The numbers for one comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub ssim: f64,
    /// Share of differing pixels, 0..=1.
    pub diff: f64,
    /// The shot's size before any resizing.
    pub shot_size: (u32, u32),
    pub ref_size: (u32, u32),
}

fn load(path: &Path) -> Result<GrayImage, String> {
    image::open(path)
        .map(|i| i.to_luma8())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Compares `shot` with `reference`; writes the diff map to `diff` when given.
pub fn compare(shot: &Path, reference: &Path, diff: Option<&Path>) -> Result<Score, String> {
    let r = load(reference)?;
    let s = load(shot)?;
    let shot_size = s.dimensions();
    let s = if s.dimensions() == r.dimensions() {
        s
    } else {
        image::imageops::resize(&s, r.width(), r.height(), FilterType::Triangle)
    };
    if let Some(path) = diff {
        diff_map(&r, &s)
            .save(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(Score {
        ssim: ssim(&r, &s),
        diff: diff_share(&r, &s),
        shot_size,
        ref_size: r.dimensions(),
    })
}

/// Mean SSIM over non-overlapping 8×8 windows of the common area.
pub fn ssim(a: &GrayImage, b: &GrayImage) -> f64 {
    const WIN: u32 = 8;
    let w = (a.width().min(b.width()) / WIN) * WIN;
    let h = (a.height().min(b.height()) / WIN) * WIN;
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let k = f64::from(WIN * WIN);
    let (mut sum, mut n) = (0.0, 0usize);
    for by in (0..h).step_by(WIN as usize) {
        for bx in (0..w).step_by(WIN as usize) {
            let (mut sa, mut sb, mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0, 0.0, 0.0);
            for y in by..by + WIN {
                for x in bx..bx + WIN {
                    let p = f64::from(a.get_pixel(x, y).0[0]);
                    let q = f64::from(b.get_pixel(x, y).0[0]);
                    sa += p;
                    sb += q;
                    saa += p * p;
                    sbb += q * q;
                    sab += p * q;
                }
            }
            let (ma, mb) = (sa / k, sb / k);
            let (va, vb, cov) = (saa / k - ma * ma, sbb / k - mb * mb, sab / k - ma * mb);
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f64 }
}

/// Share of the common area whose grey levels differ by more than [`TOLERANCE`].
pub fn diff_share(a: &GrayImage, b: &GrayImage) -> f64 {
    let (w, h) = (a.width().min(b.width()), a.height().min(b.height()));
    let mut n = 0u64;
    for y in 0..h {
        for x in 0..w {
            if a.get_pixel(x, y).0[0].abs_diff(b.get_pixel(x, y).0[0]) > TOLERANCE {
                n += 1;
            }
        }
    }
    let total = u64::from(w) * u64::from(h);
    if total == 0 {
        0.0
    } else {
        n as f64 / total as f64
    }
}

/// Reference ink red, shot ink blue, ink in both black, paper white. On a dark reference (mean
/// grey below the middle) the light text is the ink: both images are inverted first.
pub fn diff_map(reference: &GrayImage, shot: &GrayImage) -> RgbImage {
    let pixels = reference.as_raw();
    let sum: u64 = pixels.iter().map(|p| u64::from(*p)).sum();
    let dark = !pixels.is_empty() && sum / (pixels.len() as u64) < 128;
    diff_map_inked(reference, shot, dark)
}

fn diff_map_inked(reference: &GrayImage, shot: &GrayImage, invert: bool) -> RgbImage {
    let (w, h) = (
        reference.width().min(shot.width()),
        reference.height().min(shot.height()),
    );
    let ink = |v: u8| if invert { 255 - v } else { v };
    RgbImage::from_fn(w, h, |x, y| {
        let p = ink(reference.get_pixel(x, y).0[0]);
        let q = ink(shot.get_pixel(x, y).0[0]);
        image::Rgb([q, p.min(q), p])
    })
}

/// The Markdown table for the scores (views without a shot are listed as missing).
pub fn table(rows: &[(String, Result<Score, String>)]) -> String {
    let mut out =
        String::from("| view | SSIM | diff px | size (shot / reference) |\n|---|---|---|---|\n");
    for (view, row) in rows {
        match row {
            Ok(s) => out.push_str(&format!(
                "| {view} | {:.4} | {:.2} % | {}×{} / {}×{} |\n",
                s.ssim,
                s.diff * 100.0,
                s.shot_size.0,
                s.shot_size.1,
                s.ref_size.0,
                s.ref_size.1
            )),
            Err(e) => out.push_str(&format!("| {view} | – | – | {} |\n", e.replace('|', "/"))),
        }
    }
    let scored: Vec<f64> = rows
        .iter()
        .filter_map(|(_, r)| r.as_ref().ok().map(|s| s.ssim))
        .collect();
    if !scored.is_empty() {
        out.push_str(&format!(
            "\nMean SSIM {:.4} over {} view(s).\n",
            scored.iter().sum::<f64>() / scored.len() as f64,
            scored.len()
        ));
    }
    out
}

/// `doc` with the marked block replaced by `block` (markers appended at the end when absent).
pub fn splice(doc: &str, block: &str) -> String {
    let body = format!("{START}\n{}\n{END}", block.trim_end());
    if let Some(start) = doc.find(START)
        && let Some(end_rel) = doc.get(start..).and_then(|rest| rest.find(END))
    {
        let end = start + end_rel + END.len();
        let mut out = String::with_capacity(doc.len() + body.len());
        out.push_str(doc.get(..start).unwrap_or_default());
        out.push_str(&body);
        out.push_str(doc.get(end..).unwrap_or_default());
        return out;
    }
    let mut out = doc.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&body);
    out.push('\n');
    out
}

fn write_doc(root: &Path, block: &str) -> Result<(), String> {
    let path = root.join(DOC);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    // Read as late as possible: another tool may be editing the rest of the file.
    let doc = match std::fs::read_to_string(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            "# UI parity: the egui window against the Svelte interface\n".to_string()
        }
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let generated = format!(
        "Generated by `cargo xtask visual-diff`: `var/shots/<view>.png` against \
         `var/reference/<view>.png`, greyscale SSIM over 8×8 windows, share of pixels differing \
         by more than {TOLERANCE} levels.\n\n{block}"
    );
    std::fs::write(&path, splice(&doc, &generated)).map_err(|e| format!("{}: {e}", path.display()))
}

/// The views with a reference image, in the rail's order, then any others alphabetically.
fn reference_views(root: &Path) -> Result<Vec<String>, String> {
    let dir = root.join("var/reference");
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut found: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".png").map(str::to_string)
        })
        .collect();
    found.sort_by_key(|v| {
        (
            crate::shots::VIEWS
                .iter()
                .position(|k| k == v)
                .unwrap_or(usize::MAX),
            v.clone(),
        )
    });
    Ok(found)
}

pub fn run(root: &Path, args: &[&str]) -> Result<(), String> {
    if args.first() == Some(&"--pair") {
        let [shot, reference, rest @ ..] = args.get(1..).unwrap_or_default() else {
            return Err(
                "usage: cargo xtask visual-diff --pair <shot.png> <reference.png> [diff.png]"
                    .into(),
            );
        };
        let diff = rest.first().map(PathBuf::from);
        let s = compare(Path::new(shot), Path::new(reference), diff.as_deref())?;
        println!(
            "SSIM {:.4}  diff px {:.2} %  shot {}×{}  reference {}×{}",
            s.ssim,
            s.diff * 100.0,
            s.shot_size.0,
            s.shot_size.1,
            s.ref_size.0,
            s.ref_size.1
        );
        return Ok(());
    }
    let views: Vec<String> = if args.is_empty() {
        reference_views(root)?
    } else {
        args.iter().map(|a| (*a).to_string()).collect()
    };
    if views.is_empty() {
        return Err("no view to compare: var/reference/ holds no PNG".into());
    }
    let shots = root.join("var/shots");
    std::fs::create_dir_all(&shots).map_err(|e| format!("{}: {e}", shots.display()))?;
    let mut rows = Vec::new();
    for view in &views {
        if view.is_empty() || view.contains(['/', '\\', '.']) {
            return Err(format!("`{view}` is not a view id"));
        }
        let shot = shots.join(format!("{view}.png"));
        let reference = root.join("var/reference").join(format!("{view}.png"));
        let row = if !reference.is_file() {
            Err("no reference".to_string())
        } else if !shot.is_file() {
            Err("no shot (cargo xtask shots)".to_string())
        } else {
            compare(
                &shot,
                &reference,
                Some(&shots.join(format!("{view}-diff.png"))),
            )
        };
        rows.push((view.clone(), row));
    }
    let block = table(&rows);
    print!("{block}");
    write_doc(root, &block)?;
    println!("visual-diff: updated {DOC}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(shift: u32) -> GrayImage {
        GrayImage::from_fn(64, 64, |x, y| {
            image::Luma([if ((x + shift) / 4 + y / 4).is_multiple_of(2) {
                0
            } else {
                255
            }])
        })
    }

    #[test]
    fn ssim_is_one_for_the_same_image_and_lower_for_a_shifted_one() {
        let (a, b) = (checker(0), checker(2));
        assert!((ssim(&a, &a) - 1.0).abs() < 1e-9);
        assert!(ssim(&a, &b) < 0.9);
        assert_eq!(diff_share(&a, &a), 0.0);
        assert!(diff_share(&a, &b) > 0.4);
    }

    #[test]
    fn the_diff_map_colours_each_side() {
        let white = GrayImage::from_pixel(1, 1, image::Luma([255]));
        let black = GrayImage::from_pixel(1, 1, image::Luma([0]));
        let light = |r, s| diff_map_inked(r, s, false).get_pixel(0, 0).0;
        assert_eq!(light(&black, &white), [255, 0, 0]);
        assert_eq!(light(&white, &black), [0, 0, 255]);
        assert_eq!(light(&black, &black), [0, 0, 0]);
        // A dark reference: its light text is the ink, still red.
        assert_eq!(diff_map(&black, &black).get_pixel(0, 0).0, [255, 255, 255]);
        let mut dark = GrayImage::from_pixel(2, 1, image::Luma([0]));
        dark.put_pixel(0, 0, image::Luma([200]));
        let shot = GrayImage::from_pixel(2, 1, image::Luma([0]));
        assert_eq!(diff_map(&dark, &shot).get_pixel(0, 0).0, [255, 55, 55]);
    }

    #[test]
    fn only_the_marked_block_changes() {
        let doc =
            "# T\n\nbefore\n\n<!-- visual-diff:start -->\nold\n<!-- visual-diff:end -->\n\nafter\n";
        assert_eq!(
            splice(doc, "new\n"),
            "# T\n\nbefore\n\n<!-- visual-diff:start -->\nnew\n<!-- visual-diff:end -->\n\nafter\n"
        );
        assert_eq!(
            splice("# T\n", "x"),
            "# T\n\n<!-- visual-diff:start -->\nx\n<!-- visual-diff:end -->\n"
        );
    }

    #[test]
    fn identical_files_score_one() {
        let dir = std::env::temp_dir().join(format!("dzch-xtask-vd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.png"), dir.join("b.png"));
        checker(0).save(&a).unwrap();
        checker(1).save(&b).unwrap();
        let same = compare(&a, &a, Some(&dir.join("d.png"))).unwrap();
        let other = compare(&b, &a, None).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!((same.ssim - 1.0).abs() < 1e-9 && same.diff == 0.0);
        assert!(other.ssim < 1.0);
    }
}
