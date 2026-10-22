//! Words, in five languages (en fr de es ru).
//!
//! The words live in `apps/desktop/i18n/<lang>.json`, `{ dictionary: { word: text } }`,
//! sorted by key: these files are the source, edited by hand (first exported from the
//! Svelte interface's intlayer dictionaries, with the same dictionary and word keys).
//! The tests want every English word in every language, and no key or insert English
//! does not have; at run time a missing word would fall back to English. Inserts are written
//! `{name}` and filled by [`tf`].
//!
//! Each language is embedded, parsed the first time it is read and kept for the life of
//! the process (its strings leaked once, so [`t`] hands out `&'static str`): switching
//! back and forth is instant and memory is bounded by five languages.
//!
//! ```ignore
//! use dzch_ui::i18n::{t, tf, plural};
//! ui.label(t("common", "close"));
//! ui.label(tf("servers", "loaded", &[("count", &9499)]));
//! let key = plural(n, "deleteSelectedMessage", "deleteSelectedMessagePlural");
//! let msg = tf("mods", key, &[("count", &n), ("size", &size)]);
//! ```

pub mod format;

use std::collections::BTreeMap;
use std::fmt::{Display, Write as _};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

use rustc_hash::{FxHashMap, FxHashSet};

/// The languages the window ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Locale {
    #[default]
    En,
    Fr,
    De,
    Es,
    Ru,
}

impl Locale {
    /// Every shipped language, in the order a picker shows them.
    pub const ALL: [Locale; 5] = [Locale::En, Locale::Fr, Locale::De, Locale::Es, Locale::Ru];

    /// The ISO 639-1 code, as saved in the preferences (`"en"`, `"fr"`…).
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Fr => "fr",
            Locale::De => "de",
            Locale::Es => "es",
            Locale::Ru => "ru",
        }
    }

    /// The language named in itself, which is what a picker should show.
    pub fn label(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Fr => "Français",
            Locale::De => "Deutsch",
            Locale::Es => "Español",
            Locale::Ru => "Русский",
        }
    }

    /// The language for an exact code (`"fr"`), if shipped.
    pub fn from_code(code: &str) -> Option<Locale> {
        Locale::ALL.into_iter().find(|l| l.code() == code)
    }

    fn index(self) -> usize {
        self as usize
    }

    fn from_index(i: u8) -> Locale {
        Locale::ALL.get(usize::from(i)).copied().unwrap_or_default()
    }

    fn source(self) -> &'static str {
        match self {
            Locale::En => include_str!("../../i18n/en.json"),
            Locale::Fr => include_str!("../../i18n/fr.json"),
            Locale::De => include_str!("../../i18n/de.json"),
            Locale::Es => include_str!("../../i18n/es.json"),
            Locale::Ru => include_str!("../../i18n/ru.json"),
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Switch the language. Instant once that language has been read once; the caller
/// repaints and saves the choice.
pub fn set_locale(l: Locale) {
    // Parse now rather than in the middle of the next frame's first label.
    let _ = words(l);
    CURRENT.store(l as u8, Ordering::Relaxed);
}

/// The current language.
pub fn locale() -> Locale {
    Locale::from_index(CURRENT.load(Ordering::Relaxed))
}

/// Which language to start in: the saved choice, else the OS language when it is one
/// we ship, else English (= `pickLocale`).
pub fn initial_locale(saved: Option<&str>) -> Locale {
    let os = sys_locale::get_locale();
    pick_locale(saved, os.as_deref())
}

/// The pure rule behind [`initial_locale`]: a shipped saved choice wins; otherwise the
/// OS language's first two letters ("fr-CA" and "fr_FR" count as French), else English.
pub fn pick_locale(saved: Option<&str>, os: Option<&str>) -> Locale {
    if let Some(l) = saved.and_then(Locale::from_code) {
        return l;
    }
    let os = os.filter(|s| !s.is_empty()).unwrap_or("en");
    let prefix: String = os.chars().take(2).flat_map(char::to_lowercase).collect();
    Locale::from_code(&prefix).unwrap_or_default()
}

type Dict = FxHashMap<&'static str, &'static str>;
type Words = FxHashMap<&'static str, Dict>;

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// One language's words, parsed once. A broken file (it is embedded and tested, so
/// only a bad export) yields no words: every lookup then falls back to English.
fn words(l: Locale) -> &'static Words {
    static CELLS: [OnceLock<Words>; 5] = [const { OnceLock::new() }; 5];
    let Some(cell) = CELLS.get(l.index()) else {
        static EMPTY: OnceLock<Words> = OnceLock::new();
        return EMPTY.get_or_init(Words::default);
    };
    cell.get_or_init(|| parse(l.source()).unwrap_or_default())
}

fn parse(src: &str) -> Result<Words, String> {
    let raw: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(src).map_err(|e| e.to_string())?;
    Ok(raw
        .into_iter()
        .map(|(d, ws)| {
            let dict: Dict = ws.into_iter().map(|(k, v)| (leak(k), leak(v))).collect();
            (leak(d), dict)
        })
        .collect())
}

fn lookup(l: Locale, dict: &str, key: &str) -> Option<&'static str> {
    words(l).get(dict)?.get(key).copied()
}

/// Unknown keys come back as themselves; interned so a typo shown every frame costs
/// one allocation, not one per frame (bounded by the literals in the code).
fn intern(key: &str) -> &'static str {
    static SEEN: OnceLock<Mutex<FxHashSet<&'static str>>> = OnceLock::new();
    let set = SEEN.get_or_init(Default::default);
    let Ok(mut set) = set.lock() else {
        return "";
    };
    if let Some(s) = set.get(key) {
        return s;
    }
    let s = leak(key.to_owned());
    set.insert(s);
    s
}

/// A word in the current language; falls back to English, then to the key itself.
pub fn t(dict: &str, key: &str) -> &'static str {
    t_in(locale(), dict, key)
}

/// A word in a given language, with the same fallbacks as [`t`].
pub fn t_in(l: Locale, dict: &str, key: &str) -> &'static str {
    lookup(l, dict, key)
        .or_else(|| lookup(Locale::En, dict, key))
        .unwrap_or_else(|| intern(key))
}

/// Whether a word exists in English (the reference language).
pub fn has(dict: &str, key: &str) -> bool {
    lookup(Locale::En, dict, key).is_some()
}

/// A word with its inserts filled: `{count}` in the text takes the value of the arg
/// named `count`. Unknown placeholders are left as written.
pub fn tf(dict: &str, key: &str, args: &[(&str, &dyn Display)]) -> String {
    fill(t(dict, key), args)
}

/// Fill `{name}` placeholders of a template. Values are written once, never re-read as
/// a template.
pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, from) = rest.split_at(open);
        out.push_str(before);
        let after = from.get(1..).unwrap_or("");
        let name_end = after.find('}');
        let arg = name_end.and_then(|end| {
            let name = after.get(..end)?;
            args.iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| (end, *v))
        });
        match arg {
            Some((end, v)) => {
                let _ = write!(out, "{v}");
                rest = after.get(end + 1..).unwrap_or("");
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `one` when `count` is exactly 1, `other` otherwise (zero included) — the Svelte
/// `plural(count, one, other)`. The dictionaries carry both forms as two keys.
pub fn plural<T>(count: i64, one: T, other: T) -> T {
    if count == 1 { one } else { other }
}

/// The names of the `{placeholders}` in a text, in order (duplicates kept).
pub fn placeholders(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = rest.get(open + 1..).unwrap_or("");
        match after.find('}') {
            Some(end) => {
                let name = after.get(..end).unwrap_or("");
                if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    out.push(name);
                }
                rest = after.get(end + 1..).unwrap_or("");
            }
            None => break,
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
mod tests;
