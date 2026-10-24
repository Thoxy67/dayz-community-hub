//! The theme being worn (= `lib/theme/theme.svelte.ts` + `stored.ts`, simplified as
//! `plan/architecture.md` says): a preset, one of the player's own themes (twelve
//! colours), or "follow the system" (a dark and a light preset the OS switch toggles
//! between). No window frame. The choice lives in [`Prefs`] (`dzch.theme`); this store
//! keeps what is not saved (the OS scheme, a hover preview) and applies the tokens when
//! they change, never every frame.

use egui::Context;

use crate::prefs::{CustomTheme, Prefs};
use crate::theme::presets::{self, DEFAULT_DARK};
use crate::theme::season::{self, Day};
use crate::theme::{self, Ambient, Oklch, Raw, Scheme, Tokens};

/// The names of the twelve colours, in the editor's order (= `TOKEN_NAMES`).
pub const TOKEN_NAMES: [&str; 12] = [
    "bg",
    "panel",
    "raised",
    "fg",
    "accent",
    "accent-fg",
    "ok",
    "warn",
    "err",
    "info",
    "map",
    "mods",
];

/// The colours worn, whatever chose them.
#[derive(Clone, Debug, PartialEq)]
pub struct Worn {
    /// A preset id or a custom theme's id.
    pub id: String,
    pub name: String,
    pub scheme: Scheme,
    pub raw: Raw,
    /// The preset a custom theme was started from.
    pub base: Option<String>,
}

pub fn is_custom(id: Option<&str>) -> bool {
    id.is_some_and(|i| i.starts_with("custom:"))
}

/// A preset id as it is now: a retired preset's survivor, or `None` if unknown (or not
/// of `scheme`).
pub fn resolve_preset(id: &str, scheme: Option<Scheme>) -> Option<&'static str> {
    let p = presets::by_id(id)?;
    (scheme.is_none_or(|s| s == p.scheme)).then_some(p.id)
}

fn new_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let ms = super::now_ms();
    let n = N.fetch_add(1, Ordering::Relaxed);
    format!("custom:{ms:x}{n:x}")
}

pub fn raw_get(raw: &Raw, name: &str) -> Option<Oklch> {
    Some(match name {
        "bg" => raw.bg,
        "panel" => raw.panel,
        "raised" => raw.raised,
        "fg" => raw.fg,
        "accent" => raw.accent,
        "accent-fg" | "accent_fg" => raw.accent_fg,
        "ok" => raw.ok,
        "warn" => raw.warn,
        "err" => raw.err,
        "info" => raw.info,
        "map" => raw.map,
        "mods" => raw.mods,
        _ => return None,
    })
}

pub fn raw_set(raw: &mut Raw, name: &str, v: Oklch) -> bool {
    let slot = match name {
        "bg" => &mut raw.bg,
        "panel" => &mut raw.panel,
        "raised" => &mut raw.raised,
        "fg" => &mut raw.fg,
        "accent" => &mut raw.accent,
        "accent-fg" | "accent_fg" => &mut raw.accent_fg,
        "ok" => &mut raw.ok,
        "warn" => &mut raw.warn,
        "err" => &mut raw.err,
        "info" => &mut raw.info,
        "map" => &mut raw.map,
        "mods" => &mut raw.mods,
        _ => return false,
    };
    *slot = v;
    true
}

/// `oklch(L% C H)` as the CSS (and the theme files) write it.
pub fn oklch_css(c: Oklch) -> String {
    format!("oklch({:.1}% {:.3} {:.0})", c[0] * 100.0, c[1], c[2])
}

/// Read `oklch(L% C H)` or `oklch(L C H)` (L in 0–1).
pub fn parse_oklch(s: &str) -> Option<Oklch> {
    let inner = s.trim().strip_prefix("oklch(")?.strip_suffix(')')?;
    let mut it = inner.split_whitespace();
    let l = it.next()?;
    let l = match l.strip_suffix('%') {
        Some(p) => p.parse::<f32>().ok()? / 100.0,
        None => l.parse::<f32>().ok()?,
    };
    let c = it.next()?.parse::<f32>().ok()?;
    let h = it.next()?.trim_end_matches("deg").parse::<f32>().ok()?;
    let ok = [l, c, h].iter().all(|v| v.is_finite());
    ok.then_some([l.clamp(0.0, 1.0), c.clamp(0.0, 0.5), h.rem_euclid(360.0)])
}

/// Where the seasonal switch reads the date from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Clock {
    /// No date: no seasonal theme (tests, screenshots unless they ask for one).
    #[default]
    Off,
    /// The local date (the window `main` makes).
    Local,
    /// This day (`ui_shot`'s `{"date": …}`, tests).
    Fixed(Day),
}

/// How often the local date is read again.
const DATE_CHECK: std::time::Duration = std::time::Duration::from_secs(60);

#[derive(Default)]
pub struct Theme {
    /// The OS's scheme, as egui reports it.
    pub os: Option<Scheme>,
    /// A theme shown while the pointer rests on it in a menu; never stored.
    preview: Option<String>,
    applied: Option<(Scheme, Raw, Ambient)>,
    clock: Clock,
    /// The date the seasonal switch goes by (`None`: no seasonal theme).
    today: Option<Day>,
    /// When the local date was last read, and when the next month begins.
    checked: Option<std::time::Instant>,
    month_ends: Option<std::time::Instant>,
}

impl Theme {
    /// Where the seasonal switch reads the date from; read again at once.
    pub fn set_clock(&mut self, clock: Clock) {
        self.clock = clock;
        self.checked = None;
        self.month_ends = None;
        self.today = match clock {
            Clock::Off => None,
            Clock::Local => Some(Day::today()),
            Clock::Fixed(d) => Some(d),
        };
    }

    /// The local date, read once a minute at most (a frame's input or a hub event
    /// brings the check; no frame of its own but one when the month turns).
    fn tick(&mut self, ctx: &Context, prefs: &Prefs) {
        if self.clock != Clock::Local {
            return;
        }
        let now = std::time::Instant::now();
        if self
            .checked
            .is_none_or(|c| now.duration_since(c) >= DATE_CHECK)
        {
            self.checked = Some(now);
            self.today = Some(Day::today());
            self.month_ends = Some(now + season::until_next_month());
        }
        // One wake when the month turns, while the switch is on: the override starts
        // or ends without waiting for input.
        if prefs.get().ui.seasonal_themes
            && let Some(end) = self.month_ends
        {
            ctx.request_repaint_after(end.saturating_duration_since(now));
        }
    }

    /// The seasonal preset worn over the player's theme now, if any.
    pub fn seasonal(&self, prefs: &Prefs) -> Option<&'static str> {
        let p = prefs.get();
        let own = self.resolve(prefs, p.theme.preset.as_deref()).scheme;
        season::override_for(
            self.today,
            p.ui.seasonal_themes,
            p.ui.season_opt_out.as_deref(),
            own,
        )
    }

    /// A theme picked by hand: while a seasonal theme is worn, the player's choice wins
    /// for the rest of the month.
    fn picked(&self, prefs: &mut Prefs) {
        if self.seasonal(prefs).is_some()
            && let Some(d) = self.today
        {
            prefs.edit().ui.season_opt_out = Some(d.period_key());
        }
    }

    fn os_scheme(&self) -> Scheme {
        self.os.unwrap_or(Scheme::Dark)
    }

    fn resolve(&self, prefs: &Prefs, id: Option<&str>) -> Worn {
        let s = &prefs.get().theme;
        if is_custom(id)
            && let Some(c) = s.customs.iter().find(|c| Some(c.id.as_str()) == id)
        {
            return Worn {
                id: c.id.clone(),
                name: c.name.clone(),
                scheme: c.scheme,
                raw: c.tokens,
                base: c.base.clone(),
            };
        }
        let pair = match self.os_scheme() {
            Scheme::Light => &s.pair.light,
            Scheme::Dark => &s.pair.dark,
        };
        let preset = id
            .and_then(presets::by_id)
            .or_else(|| presets::by_id(pair))
            .or_else(|| presets::by_id(DEFAULT_DARK));
        match preset {
            Some(p) => Worn {
                id: p.id.to_string(),
                name: p.name.to_string(),
                scheme: p.scheme,
                raw: p.raw,
                base: None,
            },
            None => Worn {
                id: DEFAULT_DARK.to_string(),
                name: "Chernarus".to_string(),
                scheme: Scheme::Dark,
                raw: CHERNARUS,
                base: None,
            },
        }
    }

    /// The colours actually worn (a hover preview included).
    pub fn current(&self, prefs: &Prefs) -> Worn {
        let id = self
            .preview
            .clone()
            .or_else(|| self.seasonal(prefs).map(str::to_string))
            .or_else(|| prefs.get().theme.preset.clone());
        self.resolve(prefs, id.as_deref())
    }

    /// The colours of any selectable id (`None` = follow the system), for a preview card.
    pub fn look(&self, prefs: &Prefs, id: Option<&str>) -> Worn {
        self.resolve(prefs, id)
    }

    /// What is selected: a preset id, "custom" for any of the player's own, or `None`
    /// for "follow the system".
    pub fn selected<'p>(&self, prefs: &'p Prefs) -> Option<&'p str> {
        let p = prefs.get().theme.preset.as_deref();
        if is_custom(p) { Some("custom") } else { p }
    }

    /// The custom theme being worn.
    pub fn custom<'p>(&self, prefs: &'p Prefs) -> Option<&'p CustomTheme> {
        let s = &prefs.get().theme;
        let id = s.preset.as_deref();
        if !is_custom(id) {
            return None;
        }
        s.customs.iter().find(|c| Some(c.id.as_str()) == id)
    }

    /// Wear `id`: a preset, a custom theme, "custom" (the first of the player's own),
    /// or `None` to follow the system.
    pub fn use_theme(&mut self, prefs: &mut Prefs, id: Option<&str>) {
        self.preview = None;
        self.picked(prefs);
        let id = match id {
            Some("custom") => prefs.get().theme.customs.first().map(|c| c.id.clone()),
            Some(i) if is_custom(Some(i)) => prefs
                .get()
                .theme
                .customs
                .iter()
                .any(|c| c.id == i)
                .then(|| i.to_string()),
            Some(i) => resolve_preset(i, None).map(str::to_string),
            None => None,
        };
        if prefs.get().theme.preset != id {
            prefs.edit().theme.preset = id;
        }
    }

    /// Wear `id` for a moment without choosing it; `None` ends the preview.
    pub fn preview(&mut self, id: Option<&str>) {
        self.preview = id.map(str::to_string);
    }

    /// Which preset "follow the system" wears for `scheme`.
    pub fn set_pair(&mut self, prefs: &mut Prefs, scheme: Scheme, id: &str) {
        let Some(p) = resolve_preset(id, Some(scheme)) else {
            return;
        };
        let pair = &mut prefs.edit().theme.pair;
        match scheme {
            Scheme::Dark => pair.dark = p.to_string(),
            Scheme::Light => pair.light = p.to_string(),
        }
    }

    /// A new custom theme copied from `from` (what is worn when `None`), worn at once.
    pub fn duplicate(
        &mut self,
        prefs: &mut Prefs,
        from: Option<&str>,
        name: Option<&str>,
    ) -> String {
        let src = match from {
            Some(id) => self.resolve(prefs, Some(id)),
            None => self.current(prefs),
        };
        self.picked(prefs);
        let base = if is_custom(Some(&src.id)) {
            src.base.clone()
        } else {
            Some(src.id.clone())
        };
        let c = CustomTheme {
            id: new_id(),
            name: name.map_or_else(|| format!("{} *", src.name), str::to_string),
            scheme: src.scheme,
            tokens: src.raw,
            base,
        };
        let id = c.id.clone();
        let s = prefs.edit();
        s.theme.customs.push(c);
        s.theme.preset = Some(id.clone());
        self.preview = None;
        id
    }

    /// The worn custom theme, a copy of what is worn made first if none is.
    fn edit<'p>(&mut self, prefs: &'p mut Prefs) -> Option<&'p mut CustomTheme> {
        self.picked(prefs);
        if self.custom(prefs).is_none() {
            self.duplicate(prefs, None, None);
        }
        let s = &mut prefs.edit().theme;
        let id = s.preset.clone();
        s.customs.iter_mut().find(|c| Some(&c.id) == id.as_ref())
    }

    pub fn set_token(&mut self, prefs: &mut Prefs, name: &str, value: Oklch) {
        if let Some(c) = self.edit(prefs) {
            raw_set(&mut c.tokens, name, value);
        }
    }

    /// Replace the worn custom theme's colours (a new one if none is worn).
    pub fn set_custom(&mut self, prefs: &mut Prefs, scheme: Scheme, raw: Raw, name: Option<&str>) {
        if let Some(c) = self.edit(prefs) {
            c.scheme = scheme;
            c.tokens = raw;
            if let Some(n) = name.map(str::trim).filter(|n| !n.is_empty()) {
                c.name = n.to_string();
            }
        }
    }

    pub fn set_scheme(&mut self, prefs: &mut Prefs, scheme: Scheme) {
        if let Some(c) = self.edit(prefs) {
            c.scheme = scheme;
        }
    }

    pub fn rename(&mut self, prefs: &mut Prefs, id: &str, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(c) = prefs.edit().theme.customs.iter_mut().find(|c| c.id == id) {
            c.name = name.chars().take(48).collect();
        }
    }

    /// Back to the preset it was started from.
    pub fn reset(&mut self, prefs: &mut Prefs, id: &str) {
        let base = prefs
            .get()
            .theme
            .customs
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.base.as_deref())
            .and_then(presets::by_id);
        let Some(p) = base else {
            return;
        };
        if let Some(c) = prefs.edit().theme.customs.iter_mut().find(|c| c.id == id) {
            c.tokens = p.raw;
            c.scheme = p.scheme;
        }
    }

    pub fn remove(&mut self, prefs: &mut Prefs, id: &str) {
        if !prefs.get().theme.customs.iter().any(|c| c.id == id) {
            return;
        }
        let s = &mut prefs.edit().theme;
        s.customs.retain(|c| c.id != id);
        if s.preset.as_deref() == Some(id) {
            s.preset = None;
        }
    }

    /// A theme as a file others can import: `{version: 2, app, name, scheme, tokens}`.
    pub fn export_json(&self, prefs: &Prefs, id: Option<&str>) -> String {
        let w = match id {
            Some(id) => self.resolve(prefs, Some(id)),
            None => self.current(prefs),
        };
        let tokens: serde_json::Map<String, serde_json::Value> = TOKEN_NAMES
            .iter()
            .filter_map(|n| raw_get(&w.raw, n).map(|c| ((*n).to_string(), oklch_css(c).into())))
            .collect();
        let v = serde_json::json!({
            "version": 2,
            "app": "dayz-community-hub",
            "name": w.name,
            "scheme": w.scheme,
            "tokens": tokens,
        });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// Add a theme file as a new custom theme and wear it. Missing or unreadable
    /// colours come from what is worn.
    pub fn import_json(&mut self, prefs: &mut Prefs, text: &str, fallback_name: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
            return false;
        };
        let Some(tokens) = v.get("tokens").and_then(|t| t.as_object()) else {
            return false;
        };
        let mut raw = self.current(prefs).raw;
        for n in TOKEN_NAMES {
            if let Some(c) = tokens.get(n).and_then(|s| s.as_str()).and_then(parse_oklch) {
                raw_set(&mut raw, n, c);
            }
        }
        let name = v
            .get("name")
            .and_then(|n| n.as_str())
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map_or_else(
                || fallback_name.to_string(),
                |n| n.chars().take(48).collect(),
            );
        let scheme = if v.get("scheme").and_then(|s| s.as_str()) == Some("light") {
            Scheme::Light
        } else {
            Scheme::Dark
        };
        self.picked(prefs);
        let c = CustomTheme {
            id: new_id(),
            name,
            scheme,
            tokens: raw,
            base: None,
        };
        let id = c.id.clone();
        let s = prefs.edit();
        s.theme.customs.push(c);
        s.theme.preset = Some(id);
        true
    }

    /// Each frame: follow the OS scheme and wear the tokens when they changed.
    pub fn sync(&mut self, ctx: &Context, prefs: &Prefs) -> bool {
        let os = ctx.system_theme().map(|t| match t {
            egui::Theme::Light => Scheme::Light,
            egui::Theme::Dark => Scheme::Dark,
        });
        if os.is_some() {
            self.os = os;
        }
        self.tick(ctx, prefs);
        let w = self.current(prefs);
        let ambient = presets::by_id(&w.id)
            .filter(|_| !is_custom(Some(&w.id)))
            .map_or(Ambient::None, |p| p.ambient);
        let key = (w.scheme, w.raw, ambient);
        if self.applied.as_ref() == Some(&key) {
            return false;
        }
        theme::apply(ctx, Tokens::derive(&w.raw, w.scheme));
        crate::kit::season::set_ambient(ctx, ambient);
        self.applied = Some(key);
        true
    }
}

/// Chernarus, should the preset table ever lose it.
const CHERNARUS: Raw = Raw {
    bg: [0.183, 0.007, 135.0],
    panel: [0.217, 0.012, 129.0],
    raised: [0.266, 0.017, 127.0],
    fg: [0.914, 0.022, 98.0],
    accent: [0.788, 0.141, 85.0],
    accent_fg: [0.195, 0.019, 98.0],
    ok: [0.748, 0.141, 130.0],
    warn: [0.734, 0.144, 62.0],
    err: [0.619, 0.170, 32.0],
    info: [0.701, 0.063, 246.0],
    map: [0.770, 0.078, 88.0],
    mods: [0.747, 0.086, 173.0],
};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_system_pair_and_wears_presets() {
        let mut prefs = Prefs::default();
        let mut th = Theme {
            os: Some(Scheme::Light),
            ..Theme::default()
        };
        assert_eq!(th.current(&prefs).id, "topo");
        th.os = Some(Scheme::Dark);
        assert_eq!(th.current(&prefs).id, "chernarus");
        th.use_theme(&mut prefs, Some("nord"));
        assert_eq!(th.current(&prefs).id, "nord");
        th.use_theme(&mut prefs, None);
        assert_eq!(th.selected(&prefs), None);
    }

    #[test]
    fn custom_themes_edit_export_and_import() {
        let mut prefs = Prefs::default();
        let mut th = Theme::default();
        th.use_theme(&mut prefs, Some("nord"));
        th.set_token(&mut prefs, "accent", [0.5, 0.1, 30.0]);
        let c = th.custom(&prefs).unwrap().clone();
        assert_eq!(c.base.as_deref(), Some("nord"));
        assert_eq!(c.tokens.accent, [0.5, 0.1, 30.0]);
        let json = th.export_json(&prefs, None);
        assert!(th.import_json(&mut prefs, &json, "Imported"));
        assert_eq!(prefs.get().theme.customs.len(), 2);
        let back = th.custom(&prefs).unwrap().clone();
        assert!((back.tokens.accent[0] - 0.5).abs() < 0.01);
        th.reset(&mut prefs, &c.id);
        th.remove(&mut prefs, &back.id);
        assert_eq!(prefs.get().theme.customs.len(), 1);
        assert_eq!(prefs.get().theme.preset, None);
    }

    #[test]
    fn the_season_overrides_without_touching_the_choice() {
        let mut prefs = Prefs::default();
        let mut th = Theme {
            os: Some(Scheme::Dark),
            ..Theme::default()
        };
        th.use_theme(&mut prefs, Some("nord"));
        th.set_clock(Clock::Fixed(Day::new(2026, 9, 30)));
        assert_eq!(th.current(&prefs).id, "nord");
        th.set_clock(Clock::Fixed(Day::new(2026, 10, 1)));
        assert_eq!(th.current(&prefs).id, "halloween");
        assert_eq!(prefs.get().theme.preset.as_deref(), Some("nord"));
        th.set_clock(Clock::Fixed(Day::new(2026, 12, 31)));
        assert_eq!(th.current(&prefs).id, "christmas");
        th.set_clock(Clock::Fixed(Day::new(2027, 1, 1)));
        assert_eq!(th.current(&prefs).id, "nord");
        // Switched off: the player's own.
        th.set_clock(Clock::Fixed(Day::new(2026, 10, 3)));
        prefs.edit().ui.seasonal_themes = false;
        assert_eq!(th.current(&prefs).id, "nord");
        prefs.edit().ui.seasonal_themes = true;
        // A preview still shows what it previews.
        th.preview(Some("topo"));
        assert_eq!(th.current(&prefs).id, "topo");
        th.preview(None);
        assert_eq!(th.current(&prefs).id, "halloween");
    }

    #[test]
    fn a_light_theme_wears_powder_in_december() {
        let mut prefs = Prefs::default();
        let mut th = Theme {
            os: Some(Scheme::Light),
            ..Theme::default()
        };
        th.set_clock(Clock::Fixed(Day::new(2026, 12, 1)));
        // Following the system, which is light: topo.
        assert_eq!(th.current(&prefs).id, "powder");
        th.use_theme(&mut prefs, Some("gruvbox_dark"));
        prefs.edit().ui.season_opt_out = None;
        assert_eq!(th.current(&prefs).id, "christmas");
    }

    #[test]
    fn a_choice_during_the_season_holds_until_the_next() {
        let mut prefs = Prefs::default();
        let mut th = Theme {
            os: Some(Scheme::Dark),
            ..Theme::default()
        };
        // Outside a season a choice records nothing.
        th.set_clock(Clock::Fixed(Day::new(2026, 9, 12)));
        th.use_theme(&mut prefs, Some("nord"));
        assert_eq!(prefs.get().ui.season_opt_out, None);
        th.set_clock(Clock::Fixed(Day::new(2026, 10, 12)));
        assert_eq!(th.current(&prefs).id, "halloween");
        th.use_theme(&mut prefs, Some("oled"));
        assert_eq!(prefs.get().ui.season_opt_out.as_deref(), Some("2026-10"));
        assert_eq!(th.current(&prefs).id, "oled");
        th.set_clock(Clock::Fixed(Day::new(2026, 10, 31)));
        assert_eq!(th.current(&prefs).id, "oled");
        th.set_clock(Clock::Fixed(Day::new(2026, 12, 2)));
        assert_eq!(th.current(&prefs).id, "christmas");
        th.set_clock(Clock::Fixed(Day::new(2027, 10, 1)));
        assert_eq!(th.current(&prefs).id, "halloween");
        // Picking the seasonal theme itself is a choice like any other: it stays.
        th.use_theme(&mut prefs, Some("halloween"));
        th.set_clock(Clock::Fixed(Day::new(2027, 11, 1)));
        assert_eq!(th.current(&prefs).id, "halloween");
    }

    #[test]
    fn parses_css_oklch() {
        assert_eq!(parse_oklch("oklch(50% 0.1 30)"), Some([0.5, 0.1, 30.0]));
        assert_eq!(parse_oklch("oklch(0.5 0.1 30deg)"), Some([0.5, 0.1, 30.0]));
        assert_eq!(parse_oklch("rgb(1,2,3)"), None);
    }
}
