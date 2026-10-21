//! Numbers, sizes, durations and times, formatted in the current language
//! (= `apps/gui/src/lib/format.ts` and `units.ts`).
//!
//! The Svelte side used the browser's `Intl`; these reproduce what it printed for the five
//! shipped languages (separators, compact suffixes, month names, date orders), checked
//! against ICU in the tests. Each formatter has a `_in(locale, …)` form for tests and for
//! text that must be in a given language.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};

use super::{Locale, fill, locale, t_in};

const NBSP: char = '\u{a0}';
const NNBSP: char = '\u{202f}';
/// What a missing number shows.
pub const NONE: &str = "—";

fn decimal_sep(l: Locale) -> char {
    match l {
        Locale::En => '.',
        _ => ',',
    }
}

fn group_sep(l: Locale) -> char {
    match l {
        Locale::En => ',',
        Locale::Fr => NNBSP,
        Locale::De | Locale::Es => '.',
        Locale::Ru => NBSP,
    }
}

/// `value` rounded half away from zero to `max_frac` decimals, trailing zeros dropped,
/// grouped by thousands once the integer part has at least `min_group + 3` digits.
fn decimal(l: Locale, value: f64, max_frac: u32, min_group: usize) -> String {
    if !value.is_finite() {
        return NONE.to_owned();
    }
    let scale = 10f64.powi(i32::try_from(max_frac).unwrap_or(0));
    let scaled = (value.abs() * scale).round();
    let (int_part, frac_part) = if scaled < 1e18 {
        // In range: exact integer arithmetic below.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let s = scaled as u64;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sc = scale as u64;
        let sc = sc.max(1);
        (
            (s / sc).to_string(),
            format!("{:0width$}", s % sc, width = max_frac as usize),
        )
    } else {
        (format!("{:.0}", value.abs()), String::new())
    };
    let frac = frac_part.trim_end_matches('0');
    let negative = value < 0.0 && (int_part != "0" || !frac.is_empty());

    let mut out = String::with_capacity(int_part.len() + frac.len() + 6);
    if negative {
        out.push('-');
    }
    let digits = int_part.len();
    let group = digits >= min_group + 3;
    for (i, c) in int_part.chars().enumerate() {
        if group && i > 0 && (digits - i) % 3 == 0 {
            out.push(group_sep(l));
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(decimal_sep(l));
        out.push_str(frac);
    }
    out
}

/// 18 432 (up to three decimals), in the current language.
pub fn num(n: f64) -> String {
    num_in(locale(), n)
}

/// [`num`] for an optional value: an em dash for nothing.
pub fn num_opt(n: Option<f64>) -> String {
    n.map_or_else(|| NONE.to_owned(), num)
}

pub fn num_in(l: Locale, n: f64) -> String {
    // Spanish groups only from five digits (1234, 12.345), like ICU.
    let min_group = if l == Locale::Es { 2 } else { 1 };
    decimal(l, n, 3, min_group)
}

/// The compact suffix for a power of ten (3, 6, 9, 12), or `None` to print the number in
/// full at that size (German thousands).
fn compact_suffix(l: Locale, pow: u32) -> Option<&'static str> {
    match (l, pow) {
        (Locale::En, 3) => Some("K"),
        (Locale::En, 6) => Some("M"),
        (Locale::En, 9) => Some("B"),
        (Locale::En, _) => Some("T"),
        (Locale::Fr, 3) => Some("\u{a0}k"),
        (Locale::Fr, 6) => Some("\u{a0}M"),
        (Locale::Fr, 9) => Some("\u{a0}Md"),
        (Locale::Fr, _) => Some("\u{a0}Bn"),
        (Locale::De, 3) => None,
        (Locale::De, 6) => Some("\u{a0}Mio."),
        (Locale::De, 9) => Some("\u{a0}Mrd."),
        (Locale::De, _) => Some("\u{a0}Bio."),
        (Locale::Es, 3) => Some("\u{a0}mil"),
        (Locale::Es, 6 | 9) => Some("\u{a0}M"),
        (Locale::Es, _) => Some("\u{a0}B"),
        (Locale::Ru, 3) => Some("\u{a0}тыс."),
        (Locale::Ru, 6) => Some("\u{a0}млн"),
        (Locale::Ru, 9) => Some("\u{a0}млрд"),
        (Locale::Ru, _) => Some("\u{a0}трлн"),
    }
}

/// The divisor's power of ten for a magnitude (Spanish keeps millions up to 10^12).
fn compact_pow(l: Locale, abs: f64) -> u32 {
    let pow = if abs >= 1e12 {
        12
    } else if abs >= 1e9 {
        9
    } else if abs >= 1e6 {
        6
    } else if abs >= 1e3 {
        3
    } else {
        0
    };
    if l == Locale::Es && pow == 9 { 6 } else { pow }
}

/// 18.4K / 18,4 k / 18.432 / 18,4 mil / 18,4 тыс. — one decimal at most.
pub fn compact(n: f64) -> String {
    compact_in(locale(), n)
}

/// [`compact`] for an optional value: an em dash for nothing.
pub fn compact_opt(n: Option<f64>) -> String {
    n.map_or_else(|| NONE.to_owned(), compact)
}

pub fn compact_in(l: Locale, n: f64) -> String {
    if !n.is_finite() {
        return NONE.to_owned();
    }
    let round1 = |v: f64| (v * 10.0).round() / 10.0;
    let mut pow = compact_pow(l, n.abs());
    // Rounding can carry into the next unit (999 999 → 1M): choose again once rounded.
    for _ in 0..2 {
        // A size printed in full (German thousands) rounds the full number.
        let div = if compact_suffix(l, pow).is_some() {
            pow
        } else {
            0
        };
        let unit = 10f64.powi(div as i32);
        let rounded = round1(n.abs() / unit) * unit;
        let again = compact_pow(l, rounded);
        if again == pow {
            break;
        }
        pow = again;
    }
    let suffix = if pow == 0 {
        None
    } else {
        compact_suffix(l, pow)
    };
    match suffix {
        Some(s) => decimal(l, n / 10f64.powi(pow as i32), 1, 2) + s,
        None => decimal(l, n, 1, 2),
    }
}

/// "3 minutes ago", from a Unix timestamp in seconds, in the current language.
pub fn relative(ts: i64) -> String {
    relative_at(Local::now().timestamp(), ts)
}

/// [`relative`] as seen at `now` (Unix seconds).
pub fn relative_at(now: i64, ts: i64) -> String {
    relative_in(locale(), now, ts)
}

/// [`relative_at`] in a given language.
pub fn relative_in(l: Locale, now: i64, ts: i64) -> String {
    let diff = now.saturating_sub(ts);
    let word = |k: &str| t_in(l, "shell", k).to_owned();
    let count = |k: &str, unit: i64| fill(t_in(l, "shell", k), &[("count", &(diff / unit))]);
    match diff {
        d if d < 60 => word("timeJustNow"),
        d if d < 120 => word("timeMinuteAgo"),
        d if d < 3_600 => count("timeMinutesAgo", 60),
        d if d < 7_200 => word("timeHourAgo"),
        d if d < 86_400 => count("timeHoursAgo", 3_600),
        d if d < 172_800 => word("timeDayAgo"),
        d if d < 604_800 => count("timeDaysAgo", 86_400),
        d if d < 1_209_600 => word("timeWeekAgo"),
        d if d < 2_592_000 => count("timeWeeksAgo", 604_800),
        _ => word("timeLongAgo"),
    }
}

fn local(ts: i64) -> Option<DateTime<Local>> {
    Local.timestamp_opt(ts, 0).earliest()
}

/// A full local date and time with seconds, for tooltips (`toLocaleString`).
pub fn date_time(ts: i64) -> String {
    local(ts).map_or_else(
        || NONE.to_owned(),
        |d| date_time_in(locale(), &d.naive_local()),
    )
}

pub fn date_time_in(l: Locale, d: &NaiveDateTime) -> String {
    let (y, m, day) = (d.year(), d.month(), d.day());
    let (h, mi, s) = (d.hour(), d.minute(), d.second());
    match l {
        Locale::En => {
            let h12 = match h % 12 {
                0 => 12,
                x => x,
            };
            let ampm = if h < 12 { "AM" } else { "PM" };
            format!("{m}/{day}/{y}, {h12}:{mi:02}:{s:02} {ampm}")
        }
        Locale::Fr => format!("{day:02}/{m:02}/{y} {h}:{mi:02}:{s:02}"),
        Locale::De => format!("{day}.{m}.{y}, {h}:{mi:02}:{s:02}"),
        Locale::Es => format!("{day}/{m}/{y}, {h}:{mi:02}:{s:02}"),
        Locale::Ru => format!("{day:02}.{m:02}.{y}, {h}:{mi:02}:{s:02}"),
    }
}

/// A calendar date with a short month ("Oct 21, 2026", "21 oct. 2026"), from Unix seconds.
pub fn date(ts: i64) -> String {
    local(ts).map_or_else(|| NONE.to_owned(), |d| date_in(locale(), d.date_naive()))
}

/// [`date`] from a text date: `2026-10-21` (taken as written) or RFC 3339 (shown in
/// local time). Anything else is returned as given.
pub fn date_str(s: &str) -> String {
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return date_in(locale(), d.with_timezone(&Local).date_naive());
    }
    match s
        .get(..10)
        .and_then(|p| NaiveDate::parse_from_str(p, "%Y-%m-%d").ok())
    {
        Some(d) => date_in(locale(), d),
        None => s.to_owned(),
    }
}

/// Short month names as they read inside a date (genitive in Russian).
fn month(l: Locale, m: u32) -> &'static str {
    const EN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const FR: [&str; 12] = [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ];
    const DE: [&str; 12] = [
        "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.",
        "Dez.",
    ];
    const ES: [&str; 12] = [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ];
    const RU: [&str; 12] = [
        "янв.",
        "февр.",
        "мар.",
        "апр.",
        "мая",
        "июн.",
        "июл.",
        "авг.",
        "сент.",
        "окт.",
        "нояб.",
        "дек.",
    ];
    let names = match l {
        Locale::En => &EN,
        Locale::Fr => &FR,
        Locale::De => &DE,
        Locale::Es => &ES,
        Locale::Ru => &RU,
    };
    let i = usize::try_from(m.saturating_sub(1)).unwrap_or(0);
    names.get(i).copied().unwrap_or("")
}

pub fn date_in(l: Locale, d: NaiveDate) -> String {
    let (y, m, day) = (d.year(), month(l, d.month()), d.day());
    match l {
        Locale::En => format!("{m} {day}, {y}"),
        Locale::Fr | Locale::Es => format!("{day} {m} {y}"),
        Locale::De => format!("{day}. {m} {y}"),
        Locale::Ru => format!("{day} {m} {y} г."),
    }
}

/// `v` with one decimal (half away from zero) when it is under 100 and not in bytes,
/// as the Svelte `toFixed` printed it.
fn fixed(v: f64, decimals: bool) -> String {
    if decimals {
        format!("{:.1}", (v * 10.0).round() / 10.0)
    } else {
        format!("{:.0}", v.round())
    }
}

/// 1.4 GB, from bytes (always a dot: this one does not depend on the language).
pub fn bytes(n: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut i = 0usize;
    let mut v = if n.is_finite() { n } else { 0.0 };
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let unit = UNITS.get(i).copied().unwrap_or("B");
    format!("{} {unit}", fixed(v, !(v >= 100.0 || i == 0)))
}

/// 2h 15m, from seconds. Under a minute is "<1m".
pub fn duration(secs: i64) -> String {
    let s = secs.max(0);
    let (h, m) = (s / 3_600, (s % 3_600) / 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        "<1m".to_owned()
    }
}

/// Great-circle distance in km between two `[lon, lat]` points.
pub fn distance_km(a: [f64; 2], b: [f64; 2]) -> f64 {
    let rad = f64::to_radians;
    let d_lat = rad(b[1] - a[1]);
    let d_lon = rad(b[0] - a[0]);
    let h = (d_lat / 2.0).sin().powi(2)
        + rad(a[1]).cos() * rad(b[1]).cos() * (d_lon / 2.0).sin().powi(2);
    6371.0 * 2.0 * h.sqrt().atan2((1.0 - h).sqrt())
}
