//! The seasonal switch: in October the window wears `halloween`, in December
//! `christmas` (or `powder` when the player's own theme is a light one), as an override
//! over the player's choice, which is never touched. Picking a theme by hand while the
//! override is on cancels it for the rest of that month (`UiPrefs::season_opt_out`
//! holds the month, `"2026-10"`); it comes back the next period.
//!
//! Pure: the date is passed in ([`Day`]), so the rules are tested on any day; the
//! theme store reads the local date ([`Day::today`]) once a minute at most.

use std::time::Duration;

use super::Scheme;

/// The Halloween preset, worn through October.
pub const HALLOWEEN: &str = "halloween";
/// The winter preset, worn through December over a dark theme.
pub const CHRISTMAS: &str = "christmas";
/// The light winter preset, worn through December over a light theme.
pub const POWDER: &str = "powder";

/// A calendar day (local time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    /// 1 to 31.
    pub day: u32,
}

impl Day {
    pub const fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Today, in local time.
    pub fn today() -> Self {
        use chrono::Datelike;
        let n = chrono::Local::now();
        Self::new(n.year(), n.month(), n.day())
    }

    /// `2026-10-15`; `None` for anything else.
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.trim().splitn(3, '-');
        let year = it.next()?.parse::<i32>().ok()?;
        let month = it.next()?.parse::<u32>().ok()?;
        let day = it.next()?.parse::<u32>().ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day))
            .then_some(Self::new(year, month, day))
    }

    /// The month's key, as the opt-out stores it: `2026-10`.
    pub fn period_key(&self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }
}

/// A season with a theme of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Halloween,
    Winter,
}

/// The season `day` falls in, if any: every day of October and of December.
pub fn season(day: Day) -> Option<Season> {
    match day.month {
        10 => Some(Season::Halloween),
        12 => Some(Season::Winter),
        _ => None,
    }
}

/// The preset a season wears over a player's theme of `scheme`.
pub fn preset_for(season: Season, scheme: Scheme) -> &'static str {
    match (season, scheme) {
        (Season::Halloween, _) => HALLOWEEN,
        (Season::Winter, Scheme::Dark) => CHRISTMAS,
        (Season::Winter, Scheme::Light) => POWDER,
    }
}

/// The preset worn instead of the player's on `day` (`None`: theirs): the switch is
/// on, `day` is in a season, and the player did not opt out of this month.
pub fn override_for(
    day: Option<Day>,
    enabled: bool,
    opt_out: Option<&str>,
    scheme: Scheme,
) -> Option<&'static str> {
    let day = day.filter(|_| enabled)?;
    let s = season(day)?;
    if opt_out == Some(day.period_key().as_str()) {
        return None;
    }
    Some(preset_for(s, scheme))
}

/// How long until the next month begins (local time), plus a second: the one wake
/// the switch needs while the window sits idle. A day when the local time cannot be
/// read.
pub fn until_next_month() -> Duration {
    use chrono::{Datelike, Local, NaiveDate, TimeZone};
    let now = Local::now();
    let (y, m) = if now.month() == 12 {
        (now.year().saturating_add(1), 1)
    } else {
        (now.year(), now.month() + 1)
    };
    let next = NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|d| d.and_hms_opt(0, 0, 1))
        .and_then(|d| Local.from_local_datetime(&d).earliest());
    next.and_then(|n| (n - now).to_std().ok())
        .unwrap_or(Duration::from_secs(24 * 3600))
        .max(Duration::from_secs(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(y: i32, m: u32, d: u32) -> Option<&'static str> {
        override_for(Some(Day::new(y, m, d)), true, None, Scheme::Dark)
    }

    #[test]
    fn the_periods_are_october_and_december() {
        assert_eq!(on(2026, 9, 30), None);
        assert_eq!(on(2026, 10, 1), Some(HALLOWEEN));
        assert_eq!(on(2026, 10, 31), Some(HALLOWEEN));
        assert_eq!(on(2026, 11, 1), None);
        assert_eq!(on(2026, 12, 1), Some(CHRISTMAS));
        assert_eq!(on(2026, 12, 31), Some(CHRISTMAS));
        assert_eq!(on(2027, 1, 1), None);
    }

    #[test]
    fn a_light_theme_gets_powder_in_december() {
        let d = Some(Day::new(2026, 12, 10));
        assert_eq!(override_for(d, true, None, Scheme::Light), Some(POWDER));
        let d = Some(Day::new(2026, 10, 10));
        assert_eq!(override_for(d, true, None, Scheme::Light), Some(HALLOWEEN));
    }

    #[test]
    fn the_opt_out_holds_for_its_month_only() {
        let out = Some("2026-10");
        let oct = |d| override_for(Some(Day::new(2026, 10, d)), true, out, Scheme::Dark);
        assert_eq!(oct(1), None);
        assert_eq!(oct(31), None);
        let dec = override_for(Some(Day::new(2026, 12, 1)), true, out, Scheme::Dark);
        assert_eq!(dec, Some(CHRISTMAS));
        let next = override_for(Some(Day::new(2027, 10, 1)), true, out, Scheme::Dark);
        assert_eq!(next, Some(HALLOWEEN));
    }

    #[test]
    fn off_or_no_date_is_never_an_override() {
        let d = Some(Day::new(2026, 10, 5));
        assert_eq!(override_for(d, false, None, Scheme::Dark), None);
        assert_eq!(override_for(None, true, None, Scheme::Dark), None);
        // Something unexpected in the file is no opt-out.
        assert_eq!(
            override_for(d, true, Some("??"), Scheme::Dark),
            Some(HALLOWEEN)
        );
    }

    #[test]
    fn days_parse_and_key() {
        assert_eq!(Day::parse("2026-10-15"), Some(Day::new(2026, 10, 15)));
        assert_eq!(Day::parse("2026-13-01"), None);
        assert_eq!(Day::parse("hello"), None);
        assert_eq!(Day::new(2026, 3, 1).period_key(), "2026-03");
        assert!(until_next_month() <= Duration::from_secs(32 * 24 * 3600));
    }

    #[test]
    fn the_seasonal_presets_exist() {
        for id in [HALLOWEEN, CHRISTMAS, POWDER] {
            assert!(super::super::presets::by_id(id).is_some(), "{id}");
        }
        let scheme = |id| super::super::presets::by_id(id).map(|p| p.scheme);
        assert_eq!(scheme(POWDER), Some(Scheme::Light));
        assert_eq!(scheme(CHRISTMAS), Some(Scheme::Dark));
    }
}
