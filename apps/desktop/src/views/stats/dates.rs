//! Days as the stats count them (days since 1970-01-01 in the player's own time, as the
//! hub's `days` and the Svelte calendar do) and the ways the view writes them
//! (`Intl.DateTimeFormat` with `timeZone: "UTC"` over `day × 86 400 000`).

use chrono::{Datelike, NaiveDate};

use crate::i18n::{self, Locale};

pub const DAY: i64 = 86_400;

/// The player's offset from UTC now, in seconds (east positive).
pub fn utc_offset() -> i64 {
    i64::from(chrono::Local::now().offset().local_minus_utc())
}

/// The day `ts` (Unix seconds) falls on in the player's time.
pub fn day_of(ts: i64, offset: i64) -> i64 {
    ts.saturating_add(offset).div_euclid(DAY)
}

/// The day's calendar date.
pub fn date_of(day: i64) -> Option<NaiveDate> {
    chrono::DateTime::from_timestamp(day.checked_mul(DAY)?, 0).map(|d| d.date_naive())
}

/// The Monday that starts the first of `weeks` columns ending with the week of `end`
/// (1970-01-01 was a Thursday).
pub fn first_monday(end: i64, weeks: i64) -> i64 {
    end - (end + 3).rem_euclid(7) - (weeks - 1) * 7
}

/// The history's filter for a day: from its local midnight to the next, Unix seconds.
pub fn day_bounds(day: i64, offset: i64) -> (i64, i64) {
    let from = day.saturating_mul(DAY).saturating_sub(offset);
    (from, from.saturating_add(DAY))
}

const MONTHS: [[&str; 12]; 5] = [
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ],
    [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ],
    [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
    // Genitive: they follow a day number.
    [
        "января",
        "февраля",
        "марта",
        "апреля",
        "мая",
        "июня",
        "июля",
        "августа",
        "сентября",
        "октября",
        "ноября",
        "декабря",
    ],
];

/// Short month names standing alone (`{month: "short"}`): the calendar's column heads.
const MONTHS_SHORT: [[&str; 12]; 5] = [
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ],
    [
        "Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez",
    ],
    [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    [
        "янв.",
        "февр.",
        "март",
        "апр.",
        "май",
        "июнь",
        "июль",
        "авг.",
        "сент.",
        "окт.",
        "нояб.",
        "дек.",
    ],
];

const WEEKDAYS: [[&str; 7]; 5] = [
    [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ],
    [
        "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
    ],
    [
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
        "Sonntag",
    ],
    [
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
        "domingo",
    ],
    [
        "понедельник",
        "вторник",
        "среда",
        "четверг",
        "пятница",
        "суббота",
        "воскресенье",
    ],
];

fn li(l: Locale) -> usize {
    match l {
        Locale::En => 0,
        Locale::Fr => 1,
        Locale::De => 2,
        Locale::Es => 3,
        Locale::Ru => 4,
    }
}

fn pick<const N: usize>(table: &[[&'static str; N]; 5], l: Locale, i: u32) -> &'static str {
    table
        .get(li(l))
        .and_then(|r| r.get(i as usize))
        .copied()
        .unwrap_or("")
}

/// `{dateStyle: "long"}`: "October 21, 2026", "21 octobre 2026".
pub fn long_date_in(l: Locale, d: NaiveDate) -> String {
    let (y, m, day) = (d.year(), pick(&MONTHS, l, d.month0()), d.day());
    match l {
        Locale::En => format!("{m} {day}, {y}"),
        Locale::Fr => format!("{day} {m} {y}"),
        Locale::De => format!("{day}. {m} {y}"),
        Locale::Es => format!("{day} de {m} de {y}"),
        Locale::Ru => format!("{day} {m} {y} г."),
    }
}

/// The weekday too, first letter capitalised (the calendar's tooltip):
/// "Wednesday, October 21, 2026", "Mercredi 21 octobre 2026".
pub fn full_date_in(l: Locale, d: NaiveDate) -> String {
    let wd = pick(&WEEKDAYS, l, d.weekday().num_days_from_monday());
    let s = match l {
        Locale::Fr => format!("{wd} {}", long_date_in(l, d)),
        _ => format!("{wd}, {}", long_date_in(l, d)),
    };
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => s,
    }
}

/// A column head: the month's short name.
pub fn month_short_in(l: Locale, d: NaiveDate) -> &'static str {
    pick(&MONTHS_SHORT, l, d.month0())
}

/// `{month: "short", year: "numeric"}`: "Oct 2025", "oct. 2025".
pub fn month_year_in(l: Locale, d: NaiveDate) -> String {
    let y = d.year();
    match l {
        Locale::Ru => format!("{} {y} г.", month_short_in(l, d)),
        Locale::De => format!("{} {y}", i18n::format::month(l, d.month())),
        _ => format!("{} {y}", month_short_in(l, d)),
    }
}

/// [`long_date_in`] of a day number, in the current language.
pub fn long_day(day: i64) -> String {
    date_of(day).map_or_else(String::new, |d| long_date_in(i18n::locale(), d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_and_weeks() {
        // 2026-10-21 (a Wednesday) at noon UTC, in Paris (UTC+2).
        let ts = 1_792_584_000;
        let day = day_of(ts, 7200);
        assert_eq!(date_of(day), NaiveDate::from_ymd_opt(2026, 10, 21));
        let mon = first_monday(day, 1);
        assert_eq!(date_of(mon), NaiveDate::from_ymd_opt(2026, 10, 19));
        assert_eq!(first_monday(day, 53), mon - 52 * 7);
        let (from, to) = day_bounds(day, 7200);
        assert!(from <= ts && ts < to);
        assert_eq!(to - from, DAY);
        assert_eq!(day_of(from, 7200), day);
    }

    #[test]
    fn written_dates() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap_or_default();
        assert_eq!(long_date_in(Locale::En, d), "October 21, 2026");
        assert_eq!(full_date_in(Locale::Fr, d), "Mercredi 21 octobre 2026");
        assert_eq!(full_date_in(Locale::De, d), "Mittwoch, 21. Oktober 2026");
        assert_eq!(month_year_in(Locale::Fr, d), "oct. 2026");
        assert_eq!(month_year_in(Locale::De, d), "Okt. 2026");
        assert_eq!(month_short_in(Locale::Fr, d), "oct.");
    }
}
