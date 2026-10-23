//! A server's in-game clock, estimated now from the last time it was seen.
//!
//! The list carries each server's clock ("14:09") as it stood when the list
//! was fetched; an A2S answer carries it fresh, with the server's time
//! acceleration (`etm`, how much faster than real time the day passes) and
//! its night acceleration (`entm`, DayZ's `serverNightTimeAcceleration`: a
//! multiplier applied on top of `etm` while it is night). The clock now is
//! the one seen, moved on by the real time since, at those rates. Without an
//! A2S answer the rates are unknown and taken as real time (1×), so a list's
//! clocks drift for accelerated servers until the ping scan reads them.
//!
//! Day and night, for the filter and for the rates alike, are bounded at
//! [`DAY_START`] (06:00) and [`NIGHT_START`] (20:00): DayZ's sun rises and
//! sets at different hours by map and date, roughly 05:00–07:00 and
//! 19:00–21:00 on the default maps; the bounds are the middle of those bands
//! (the rows' dawn and dusk icons cover the same hours).

use std::sync::atomic::{AtomicU64, Ordering};

/// Minutes in a day.
pub(crate) const DAY_MINUTES: u32 = 24 * 60;
/// Day starts at 06:00 (in minutes)...
pub const DAY_START: u32 = 6 * 60;
/// ...and night at 20:00.
pub const NIGHT_START: u32 = 20 * 60;

/// The fastest acceleration DayZ accepts (`serverTimeAcceleration` 0–24,
/// `serverNightTimeAcceleration` 0.1–64); anything above is clamped here.
const MAX_ACCEL: f64 = 64.0;

/// Whether `minute` (of the day) is daytime.
pub fn is_day(minute: u32) -> bool {
    (DAY_START..NIGHT_START).contains(&(minute % DAY_MINUTES))
}

/// `minute` (of the day) written "HH:MM" into `buf`, nothing allocated.
pub fn clock_text(minute: u32, buf: &mut [u8; 5]) -> &str {
    let m = minute % DAY_MINUTES;
    let (h, m) = (m / 60, m % 60);
    let digit = |v: u32| b'0' + (v % 10) as u8;
    *buf = [digit(h / 10), digit(h), b':', digit(m / 10), digit(m)];
    std::str::from_utf8(buf).unwrap_or_default()
}

/// A clock as seen: the minute of the day, the rates if known, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Clock {
    /// 0..1440.
    pub minute: u16,
    /// `etm`: the day's rate; `None` when unknown.
    pub day_accel: Option<f32>,
    /// `entm`: the night's multiplier on the day's rate; `None` when unknown.
    pub night_mult: Option<f32>,
    /// Unix seconds.
    pub seen: u64,
}

// The packed word: 0 is "no clock".
const W_PRESENT: u64 = 1 << 63;
const W_MINUTE_SHIFT: u32 = 32;
const W_DAY_SHIFT: u32 = 43;
const W_NIGHT_SHIFT: u32 = 53;
/// Ten bits each, in tenths: 1..=640 (0.1× to [`MAX_ACCEL`]), 0 for unknown and
/// [`W_RATE_STOPPED`] for a rate of zero.
const W_RATE_MASK: u64 = 0x3FF;
/// A rate of zero: a stopped clock (`etm0`), not an unknown one.
const W_RATE_STOPPED: u64 = W_RATE_MASK;

fn tenths(v: Option<f32>) -> u64 {
    match v.filter(|v| v.is_finite() && *v >= 0.0) {
        None => 0,
        Some(v) if v <= 0.0 => W_RATE_STOPPED,
        Some(v) => ((f64::from(v) * 10.0).round() as u64).clamp(1, (MAX_ACCEL * 10.0) as u64),
    }
}

fn from_tenths(t: u64) -> Option<f32> {
    match t {
        0 => None,
        W_RATE_STOPPED => Some(0.0),
        t => Some(t as f32 / 10.0),
    }
}

impl Clock {
    /// Pack for a snapshot's table: seconds in the low 32 bits (good to 2106).
    pub(crate) fn pack(&self) -> u64 {
        W_PRESENT
            | (self.seen.min(u64::from(u32::MAX)))
            | (u64::from(self.minute.min(1439)) << W_MINUTE_SHIFT)
            | (tenths(self.day_accel) << W_DAY_SHIFT)
            | (tenths(self.night_mult) << W_NIGHT_SHIFT)
    }

    pub(crate) fn unpack(w: u64) -> Option<Self> {
        if w & W_PRESENT == 0 {
            return None;
        }
        Some(Self {
            minute: ((w >> W_MINUTE_SHIFT) & 0x7FF).min(1439) as u16,
            day_accel: from_tenths((w >> W_DAY_SHIFT) & W_RATE_MASK),
            night_mult: from_tenths((w >> W_NIGHT_SHIFT) & W_RATE_MASK),
            seen: w & 0xFFFF_FFFF,
        })
    }

    /// The clock in A2S_INFO's keywords (`...,etm4.000000,entm2.000000,mod,14:09`),
    /// seen at `seen`; `None` without a clock among them. Reads the text in place.
    pub(crate) fn from_keywords(keywords: &str, seen: u64) -> Option<Self> {
        let mut minute = None;
        let (mut day, mut night) = (None, None);
        for tag in keywords.split([',', '#']).map(str::trim) {
            if let Some(m) = minute_of(tag) {
                minute = Some(m);
            } else if let Some(v) = tag.strip_prefix("entm") {
                night = rate(v);
            } else if let Some(v) = tag.strip_prefix("etm") {
                day = rate(v);
            }
        }
        Some(Self {
            minute: minute?,
            day_accel: day,
            night_mult: night,
            seen,
        })
    }

    /// The minute of the day at `now` (Unix seconds): the clock seen, moved
    /// on at its rates (1× when unknown). A clock seen "later" than `now`
    /// (a skewed system clock) reads as seen.
    pub(crate) fn minute_at(&self, now: u64) -> u32 {
        let elapsed = now.saturating_sub(self.seen) as f64;
        let day = self.day_accel.map_or(1.0, f64::from).clamp(0.0, MAX_ACCEL);
        let night = day * self.night_mult.map_or(1.0, f64::from).clamp(0.1, MAX_ACCEL);
        advance(u32::from(self.minute), elapsed, day, night)
    }
}

/// "HH:MM" as minutes of the day, if it is a clock.
pub(crate) fn minute_of(t: &str) -> Option<u16> {
    let b = t.as_bytes();
    if b.len() != 5 || b.get(2) != Some(&b':') {
        return None;
    }
    let d = |i: usize| {
        b.get(i)
            .filter(|c| c.is_ascii_digit())
            .map(|c| u16::from(c - b'0'))
    };
    let (h, m) = (d(0)? * 10 + d(1)?, d(3)? * 10 + d(4)?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// A rate as DayZ writes it: `4.000000`, or `4-000000` through the master server.
fn rate(s: &str) -> Option<f32> {
    let mut buf = [0u8; 16];
    let b = s.as_bytes();
    let slot = buf.get_mut(..b.len())?;
    for (o, c) in slot.iter_mut().zip(b) {
        *o = if *c == b'-' { b'.' } else { *c };
    }
    std::str::from_utf8(slot)
        .ok()?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

/// Move `minute` on by `real_secs` of real time, at `day` × real time between
/// [`DAY_START`] and [`NIGHT_START`] and `night` × real time otherwise. A
/// day rate of zero stops the clock (DayZ's `serverTimeAcceleration = 0`).
fn advance(minute: u32, real_secs: f64, day: f64, night: f64) -> u32 {
    let minute = minute % DAY_MINUTES;
    if day <= 0.0 || night <= 0.0 || !real_secs.is_finite() || real_secs <= 0.0 {
        return minute;
    }
    // In-game minutes per real second.
    let (day_r, night_r) = (day / 60.0, night / 60.0);
    let day_len = f64::from(NIGHT_START - DAY_START);
    let night_len = f64::from(DAY_MINUTES) - day_len;
    // Whole in-game days bring the clock back where it was.
    let cycle = day_len / day_r + night_len / night_r;
    let mut left = real_secs % cycle;
    let mut m = f64::from(minute);
    // At most a night, a day and a night again.
    for _ in 0..4 {
        let at = m as u32;
        let (end, r) = if is_day(at) {
            (f64::from(NIGHT_START), day_r)
        } else if at < DAY_START {
            (f64::from(DAY_START), night_r)
        } else {
            (f64::from(DAY_MINUTES + DAY_START), night_r)
        };
        let to_end = (end - m) / r;
        if left < to_end {
            m += left * r;
            break;
        }
        left -= to_end;
        m = end % f64::from(DAY_MINUTES);
    }
    // A hair up, so a whole number of cycles worked out in floating point lands on
    // the minute it started from, not one short.
    ((m + 1e-6).floor() as u32) % DAY_MINUTES
}

/// A table of clocks, one atomic word per server.
pub(crate) fn table(n: usize) -> Box<[AtomicU64]> {
    (0..n).map(|_| AtomicU64::new(0)).collect()
}

pub(crate) fn load(t: &[AtomicU64], i: usize) -> Option<Clock> {
    Clock::unpack(t.get(i)?.load(Ordering::Relaxed))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn day_and_night_are_bounded_at_six_and_eight() {
        assert!(!is_day(5 * 60 + 59));
        assert!(is_day(6 * 60));
        assert!(is_day(19 * 60 + 59));
        assert!(!is_day(20 * 60));
        assert!(!is_day(0));
    }

    #[test]
    fn keywords_give_the_clock_and_its_rates() {
        let c = Clock::from_keywords(
            "battleye,no3rd,external,privHive,shard000,lqs0,etm4.000000,entm2.000000,mod,14:09",
            100,
        )
        .unwrap();
        assert_eq!(c.minute, 14 * 60 + 9);
        assert_eq!(c.day_accel, Some(4.0));
        assert_eq!(c.night_mult, Some(2.0));
        let c = Clock::from_keywords("lqs0#etm6-000000#entm8-000000#mod#03:00", 0).unwrap();
        assert_eq!(
            (c.minute, c.day_accel, c.night_mult),
            (180, Some(6.0), Some(8.0))
        );
        let c = Clock::from_keywords("battleye,22:30", 0).unwrap();
        assert_eq!((c.day_accel, c.night_mult), (None, None));
        assert!(Clock::from_keywords("battleye,etm4.0", 0).is_none());
        assert!(Clock::from_keywords("", 0).is_none());
        assert!(Clock::from_keywords("25:00,etmx", 0).is_none());
    }

    #[test]
    fn a_clock_packs_and_unpacks() {
        let c = Clock {
            minute: 1439,
            day_accel: Some(4.0),
            night_mult: Some(0.5),
            seen: 1_790_000_000,
        };
        assert_eq!(Clock::unpack(c.pack()), Some(c));
        let unknown = Clock {
            minute: 0,
            day_accel: None,
            night_mult: None,
            seen: 0,
        };
        assert_eq!(Clock::unpack(unknown.pack()), Some(unknown));
        assert_eq!(Clock::unpack(0), None);
        // A stopped clock stays stopped once packed, not taken for an unknown rate.
        let stopped = Clock::from_keywords("etm0.000000,entm1.000000,10:00", 1000).unwrap();
        assert_eq!(stopped.day_accel, Some(0.0));
        let back = Clock::unpack(stopped.pack()).unwrap();
        assert_eq!(back, stopped);
        assert_eq!(back.minute_at(1000 + 5 * 3600), 600);
        // Too fast is clamped to the fastest DayZ accepts.
        let fast = Clock {
            day_accel: Some(500.0),
            ..stopped
        };
        assert_eq!(Clock::unpack(fast.pack()).unwrap().day_accel, Some(64.0));
    }

    #[test]
    fn the_clock_moves_on_at_its_rates() {
        let at = |minute: u16, day, night| Clock {
            minute,
            day_accel: day,
            night_mult: night,
            seen: 1000,
        };
        // Unknown rates: real time.
        assert_eq!(at(600, None, None).minute_at(1000 + 30 * 60), 630);
        // 4× by day: an hour of real time is four in-game.
        assert_eq!(at(8 * 60, Some(4.0), None).minute_at(1000 + 3600), 12 * 60);
        // Across dusk: 19:00 at 4×, 15 real minutes to 20:00, then 4×2 = 8× at night.
        assert_eq!(
            at(19 * 60, Some(4.0), Some(2.0)).minute_at(1000 + 30 * 60),
            22 * 60
        );
        // Across midnight and dawn.
        assert_eq!(at(23 * 60, None, None).minute_at(1000 + 8 * 3600), 7 * 60);
        // A whole cycle comes back: 4× by day (210 real minutes), 8× at night (75).
        assert_eq!(
            at(13 * 60, Some(4.0), Some(2.0)).minute_at(1000 + 285 * 60 * 1000),
            13 * 60
        );
        // A stopped clock, and one seen in the future.
        assert_eq!(at(100, Some(0.0), None).minute_at(1_000_000), 100);
        assert_eq!(at(100, None, None).minute_at(10), 100);
    }

    #[test]
    fn a_minute_is_written_as_a_clock() {
        let mut b = [0u8; 5];
        assert_eq!(clock_text(0, &mut b), "00:00");
        assert_eq!(clock_text(14 * 60 + 9, &mut b), "14:09");
        assert_eq!(clock_text(1439 + 1440, &mut b), "23:59");
    }

    #[test]
    fn clocks_as_the_list_writes_them() {
        assert_eq!(minute_of("06:30"), Some(390));
        assert_eq!(minute_of("6:30"), None);
        assert_eq!(minute_of("24:00"), None);
        assert_eq!(minute_of(""), None);
        assert_eq!(minute_of("ab:cd"), None);
    }
}
