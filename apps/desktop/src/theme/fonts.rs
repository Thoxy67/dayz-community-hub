//! The faces: IBM Plex Sans for text, IBM Plex Mono for figures and addresses,
//! Sofia Sans Extra Condensed for titles, big figures and stencilled labels
//! (`--font-sans`, `--font-mono`, `--font-display`).
//!
//! egui picks no weight from a variable font, so every weight is its own family
//! of static instances (`assets/fonts`, made from the interface's woff2 files),
//! each a chain of subsets: latin, latin-ext, cyrillic, cyrillic-ext. egui's
//! emoji faces come last, for the odd symbol.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId, TextStyle};

/// A face and weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    Sans,
    SansMedium,
    SansSemibold,
    SansBold,
    Mono,
    MonoMedium,
    MonoSemibold,
    /// Display 600: titles.
    Display,
    /// Display 800: the big figures.
    DisplayBold,
}

impl Face {
    const ALL: [Face; 9] = [
        Face::Sans,
        Face::SansMedium,
        Face::SansSemibold,
        Face::SansBold,
        Face::Mono,
        Face::MonoMedium,
        Face::MonoSemibold,
        Face::Display,
        Face::DisplayBold,
    ];

    fn name(self) -> &'static str {
        match self {
            Face::Sans => "sans",
            Face::SansMedium => "sans-500",
            Face::SansSemibold => "sans-600",
            Face::SansBold => "sans-700",
            Face::Mono => "mono",
            Face::MonoMedium => "mono-500",
            Face::MonoSemibold => "mono-600",
            Face::Display => "display-600",
            Face::DisplayBold => "display-800",
        }
    }

    /// The face's family. Built once: a named family holds an `Arc<str>`, and `font` is
    /// called hundreds of times a frame, so each call clones it instead of allocating.
    pub fn family(self) -> FontFamily {
        static FAMILIES: std::sync::LazyLock<[FontFamily; 9]> = std::sync::LazyLock::new(|| {
            Face::ALL.map(|f| match f {
                Face::Sans => FontFamily::Proportional,
                Face::Mono => FontFamily::Monospace,
                f => FontFamily::Name(f.name().into()),
            })
        });
        FAMILIES[self as usize].clone()
    }
}

/// `FontId` for a face at a size (see [`super::size`] for the scale).
pub fn font(face: Face, size: f32) -> FontId {
    FontId::new(super::size::text(size), face.family())
}

macro_rules! subsets {
    ($stem:literal) => {
        [
            (
                $stem,
                "latin",
                include_bytes!(concat!("../../assets/fonts/", $stem, "-latin.ttf")).as_slice(),
            ),
            (
                $stem,
                "latin-ext",
                include_bytes!(concat!("../../assets/fonts/", $stem, "-latin-ext.ttf")).as_slice(),
            ),
            (
                $stem,
                "cyrillic",
                include_bytes!(concat!("../../assets/fonts/", $stem, "-cyrillic.ttf")).as_slice(),
            ),
            (
                $stem,
                "cyrillic-ext",
                include_bytes!(concat!("../../assets/fonts/", $stem, "-cyrillic-ext.ttf"))
                    .as_slice(),
            ),
        ]
    };
}

type Subset = (&'static str, &'static str, &'static [u8]);

fn files(face: Face) -> [Subset; 4] {
    match face {
        Face::Sans => subsets!("IBMPlexSans-400"),
        Face::SansMedium => subsets!("IBMPlexSans-500"),
        Face::SansSemibold => subsets!("IBMPlexSans-600"),
        Face::SansBold => subsets!("IBMPlexSans-700"),
        Face::Mono => subsets!("IBMPlexMono-400"),
        Face::MonoMedium => subsets!("IBMPlexMono-500"),
        Face::MonoSemibold => subsets!("IBMPlexMono-600"),
        Face::Display => subsets!("SofiaSansExtraCondensed-600"),
        Face::DisplayBold => subsets!("SofiaSansExtraCondensed-800"),
    }
}

/// The key of the symbols fallback face.
const SYMBOLS: &str = "DejaVuSans-symbols";

/// The font definitions: every face, each falling back on egui's emoji faces and its
/// sans (for signs the subsets lack).
pub fn definitions() -> FontDefinitions {
    let egui_defaults = FontDefinitions::default();
    let mut defs = FontDefinitions::empty();
    // Last: egui's own faces, for the few signs the subsets lack (`≤`, as a browser
    // falls back on a system font), then the emoji.
    let emoji: Vec<String> = ["NotoEmoji-Regular", "emoji-icon-font", "Ubuntu-Light"]
        .into_iter()
        .filter(|n| egui_defaults.font_data.contains_key(*n))
        .map(String::from)
        .collect();
    for n in &emoji {
        if let Some(d) = egui_defaults.font_data.get(n) {
            defs.font_data.insert(n.clone(), Arc::clone(d));
        }
    }
    // The signs no bundled face has (`→`, `✓`, the PlayStation shapes), from a DejaVu Sans
    // subset, before egui's faces: a browser finds them in a system font.
    defs.font_data.insert(
        SYMBOLS.into(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/DejaVuSans-symbols.ttf"
        ))),
    );
    for face in Face::ALL {
        let mut chain = Vec::new();
        for (stem, sub, bytes) in files(face) {
            let key = format!("{stem}-{sub}");
            defs.font_data
                .insert(key.clone(), Arc::new(FontData::from_static(bytes)));
            chain.push(key);
        }
        chain.push(SYMBOLS.to_owned());
        chain.extend(emoji.iter().cloned());
        defs.families.insert(face.family(), chain);
    }
    defs
}

/// egui's named text styles, mapped onto the scale (body = `text-sm`).
pub fn text_styles() -> std::collections::BTreeMap<TextStyle, FontId> {
    use super::size::*;
    [
        (TextStyle::Small, font(Face::Sans, TEXT_2XS)),
        (TextStyle::Body, font(Face::Sans, TEXT_SM)),
        (TextStyle::Button, font(Face::SansMedium, TEXT_SM)),
        (TextStyle::Monospace, font(Face::Mono, TEXT_XS)),
        (TextStyle::Heading, font(Face::DisplayBold, TEXT_XL)),
    ]
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sign the interface's words and glyphs use has a glyph in every face: the
    /// subsets, the symbols fallback or egui's own faces (a missing one draws as a box).
    #[test]
    fn every_face_has_the_signs_the_words_use() {
        let ctx = egui::Context::default();
        ctx.set_fonts(definitions());
        let mut out = ctx.run_ui(egui::RawInput::default(), |_| {});
        out.textures_delta.clear();
        let signs = "àéèêçœ «»–—‘’“”„•…‹›™−≤≥ ←↑→↓↵⋯ ✓✕○□△ ЁЖЩ";
        for face in Face::ALL {
            let missing: String = signs
                .chars()
                .filter(|c| !c.is_whitespace())
                .filter(|c| !ctx.fonts_mut(|f| f.has_glyphs(&font(face, 12.0), &c.to_string())))
                .collect();
            assert!(missing.is_empty(), "{face:?} lacks {missing}");
        }
    }

    /// `family` indexes the families by the face's number: `ALL` is in that order.
    #[test]
    fn each_face_has_its_own_family() {
        for (i, face) in Face::ALL.into_iter().enumerate() {
            assert_eq!(face as usize, i);
        }
        assert_eq!(Face::Sans.family(), FontFamily::Proportional);
        assert_eq!(Face::Mono.family(), FontFamily::Monospace);
        assert_eq!(
            Face::DisplayBold.family(),
            FontFamily::Name("display-800".into())
        );
    }
}
