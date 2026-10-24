//! Text helpers the parts share: galleys laid out once in [`Color32::PLACEHOLDER`] (so a
//! cached galley takes its colour when it is drawn, and survives a theme switch), a
//! map's name as players say it (`map-name.ts`), and the clock of a log (`log.ts`).

use std::borrow::Cow;
use std::sync::Arc;

use egui::{Color32, FontId, Galley, Painter, Pos2, Ui};

use crate::kit::paint;

/// One line, never wrapped, coloured when drawn.
pub fn ph(ui: &Ui, text: &str, font: FontId) -> Arc<Galley> {
    paint::galley(ui, text, font, Color32::PLACEHOLDER)
}

/// One line cut with `…` past `max_width`, coloured when drawn.
pub fn ph_trunc(ui: &Ui, text: &str, font: FontId, max_width: f32) -> Arc<Galley> {
    paint::galley_trunc(ui, text, font, Color32::PLACEHOLDER, max_width.max(1.0))
}

/// Draws `g` with its left edge at `x`, centred on `cy`, in `color`.
pub fn draw(p: &Painter, g: &Arc<Galley>, x: f32, cy: f32, color: Color32) {
    p.galley(Pos2::new(x, cy - g.size().y / 2.0), Arc::clone(g), color);
}

/// A galley's first baseline, from its top.
pub fn baseline(g: &Galley) -> f32 {
    g.rows.first().map_or(g.size().y * 0.8, |r| {
        r.pos.y
            + r.row
                .glyphs
                .first()
                .map_or(r.row.size.y * 0.8, |gl| gl.pos.y)
    })
}

/// Draws `g` with its left edge at `x` and its first baseline on `base`: texts of
/// different faces on one line (stencil, mono figures, words) read as one line.
pub fn draw_on(p: &Painter, g: &Arc<Galley>, x: f32, base: f32, color: Color32) {
    p.galley(Pos2::new(x, base - baseline(g)), Arc::clone(g), color);
}

/// Whether a galley was cut (its text is not all shown).
pub fn is_cut(g: &Galley, text: &str) -> bool {
    g.text().len() != text.len()
}

/// A map's name as players say it, from the id servers report: `enoch` is Livonia,
/// `chernarusplus` is Chernarus+. Unknown ids are split and capitalised.
pub fn map_name(id: &str) -> Cow<'_, str> {
    const KNOWN: &[(&str, &str)] = &[
        ("chernarusplus", "Chernarus+"),
        ("chernarus", "Chernarus"),
        ("enoch", "Livonia"),
        ("sakhal", "Sakhal"),
        ("namalsk", "Namalsk"),
        ("deerisle", "Deer Isle"),
        ("banov", "Banov"),
        ("esseker", "Esseker"),
        ("takistanplus", "Takistan+"),
        ("pripyat", "Pripyat"),
        ("chiemsee", "Chiemsee"),
        ("rostow", "Rostow"),
        ("valning", "Valning"),
        ("iztek", "Iztek"),
        ("melkart", "Melkart"),
        ("anastara", "Anastara"),
        ("alteria", "Alteria"),
        ("swansisland", "Swans Island"),
        ("pnw", "Pacific Northwest"),
    ];
    if id.is_empty() {
        return Cow::Borrowed("");
    }
    if let Some((_, name)) = KNOWN.iter().find(|(k, _)| k.eq_ignore_ascii_case(id)) {
        return Cow::Borrowed(name);
    }
    // `_`/`-` runs become one space, camelCase splits, every word starts upper-case.
    let mut out = String::with_capacity(id.len() + 4);
    let mut prev: Option<char> = None;
    let mut sep = false;
    for c in id.chars() {
        if c == '_' || c == '-' {
            sep = true;
            continue;
        }
        if sep {
            if !out.is_empty() {
                out.push(' ');
            }
            sep = false;
        } else if prev.is_some_and(|p| p.is_ascii_lowercase()) && c.is_ascii_uppercase() {
            out.push(' ');
        }
        let word_start = out.is_empty() || out.ends_with(' ') || !prev.is_some_and(is_word);
        if word_start {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
        prev = Some(c);
    }
    Cow::Owned(out)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `m:ss`, or `h:mm:ss` past the hour (`clock(ms)` in `log.ts`).
pub fn clock(ms: f64) -> String {
    let s = if ms.is_finite() && ms > 0.0 {
        (ms / 1000.0).floor() as u64
    } else {
        0
    };
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{sec:02}")
    } else {
        format!("{m}:{sec:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_names() {
        assert_eq!(map_name("enoch"), "Livonia");
        assert_eq!(map_name("ChernarusPlus"), "Chernarus+");
        assert_eq!(map_name("deer_isle-x"), "Deer Isle X");
        assert_eq!(map_name("myMap"), "My Map");
        assert_eq!(map_name(""), "");
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(65_000.0), "1:05");
        assert_eq!(clock(3_725_000.0), "1:02:05");
        assert_eq!(clock(-5.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }
}
