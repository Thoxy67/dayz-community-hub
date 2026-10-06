//! The figures the stats view shows, worked out from the session log. Pure:
//! the log, the time now and the player's UTC offset in, the figures out.

use std::collections::{BTreeMap, HashMap, HashSet};

use dz_profile::{Session, SessionKind};
use serde::{Deserialize, Serialize};

const DAY: i64 = 86_400;
const HOUR: i64 = 3_600;

/// What a session was played on, for the window.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum PlayKind {
    Server,
    Offline,
    Unknown,
}

impl From<SessionKind> for PlayKind {
    fn from(k: SessionKind) -> Self {
        match k {
            SessionKind::Server => PlayKind::Server,
            SessionKind::Offline => PlayKind::Offline,
            SessionKind::Unknown => PlayKind::Unknown,
        }
    }
}

/// How far back the figures look.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum StatsRange {
    Week,
    Month,
    Year,
    All,
}

impl StatsRange {
    fn since(self, now: i64) -> Option<i64> {
        match self {
            StatsRange::Week => Some(now - 7 * DAY),
            StatsRange::Month => Some(now - 30 * DAY),
            StatsRange::Year => Some(now - 365 * DAY),
            StatsRange::All => None,
        }
    }
}

/// One session, for the window.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct SessionDto {
    pub kind: PlayKind,
    pub name: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub map: Option<String>,
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    /// How long it lasted (so far, when open); 0 when not measured.
    pub secs: i64,
    pub open: bool,
    pub measured: bool,
}

/// Time spent on one server or offline mission.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct PlaceStatDto {
    pub kind: PlayKind,
    pub name: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    /// The map played most there.
    pub map: Option<String>,
    pub secs: i64,
    pub sessions: u32,
    /// Sessions of unknown length among them (from the old history).
    pub unmeasured: u32,
    pub first: i64,
    pub last: i64,
}

/// Time spent on one map.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct MapStatDto {
    pub map: String,
    pub secs: i64,
    pub sessions: u32,
}

/// Time played on one day.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct DayStatDto {
    /// Days since 1970-01-01 in the player's time.
    pub day: i64,
    pub secs: i64,
    /// Sessions that started that day.
    pub sessions: u32,
}

/// Everything the stats view shows.
#[derive(Serialize, Clone, Debug, PartialEq, specta::Type)]
pub struct PlayStatsDto {
    pub total_secs: i64,
    pub sessions: u32,
    /// Distinct servers and missions.
    pub places: u32,
    /// Over measured sessions only.
    pub average_secs: i64,
    pub longest: Option<SessionDto>,
    pub days_played: u32,
    /// Days in a row with play, ending today (or yesterday), over all time.
    pub streak: u32,
    pub best_streak: u32,
    /// The first session ever (whatever the range), Unix seconds.
    pub first: Option<i64>,
    /// The session running now.
    pub current: Option<SessionDto>,
    /// Most played first.
    pub places_played: Vec<PlaceStatDto>,
    pub maps: Vec<MapStatDto>,
    /// Days with play, oldest first, over all time (whatever the range).
    pub days: Vec<DayStatDto>,
    /// Seconds by weekday (0 = Monday) and hour, in the player's time.
    pub week_hours: Vec<Vec<i64>>,
}

/// A session as it stands at `now`: an open one runs until now.
pub fn dto(s: &Session, now: i64) -> SessionDto {
    let end = if s.open { now.max(s.end) } else { s.end };
    SessionDto {
        kind: s.kind.into(),
        name: s.name.clone(),
        ip: s.ip.clone(),
        port: s.port,
        map: s.map.clone(),
        start: s.start,
        end,
        secs: if s.measured {
            (end - s.start).max(0)
        } else {
            0
        },
        open: s.open,
        measured: s.measured,
    }
}

/// The figures over `range`. `utc_offset` is the player's offset from UTC in
/// seconds (east positive), for days and hours in their own time.
pub fn stats(log: &[Session], now: i64, range: StatsRange, utc_offset: i64) -> PlayStatsDto {
    let since = range.since(now);
    let in_range: Vec<SessionDto> = log
        .iter()
        .filter(|s| since.is_none_or(|t| s.start >= t || s.open))
        .map(|s| dto(s, now))
        .collect();

    let total_secs: i64 = in_range.iter().map(|s| s.secs).sum();
    let measured: Vec<&SessionDto> = in_range.iter().filter(|s| s.measured).collect();
    let average_secs = if measured.is_empty() {
        0
    } else {
        total_secs / measured.len() as i64
    };
    let longest = measured.iter().max_by_key(|s| s.secs).map(|s| (*s).clone());

    // Per server or mission.
    let mut places: HashMap<String, (PlaceStatDto, HashMap<String, i64>)> = HashMap::new();
    for s in &in_range {
        let key = match (&s.ip, s.port) {
            (Some(ip), Some(port)) => format!("{ip}:{port}"),
            _ => format!("{:?}:{}", s.kind, s.name),
        };
        let (p, maps) = places.entry(key).or_insert_with(|| {
            (
                PlaceStatDto {
                    kind: s.kind,
                    name: s.name.clone(),
                    ip: s.ip.clone(),
                    port: s.port,
                    map: None,
                    secs: 0,
                    sessions: 0,
                    unmeasured: 0,
                    first: s.start,
                    last: s.start,
                },
                HashMap::new(),
            )
        });
        p.secs += s.secs;
        p.sessions += 1;
        p.unmeasured += u32::from(!s.measured);
        p.first = p.first.min(s.start);
        if s.start >= p.last {
            p.last = s.start;
            // The name it had most recently.
            p.name = s.name.clone();
        }
        if let Some(m) = &s.map {
            *maps.entry(m.clone()).or_default() += s.secs.max(1);
        }
    }
    let mut places_played: Vec<PlaceStatDto> = places
        .into_values()
        .map(|(mut p, maps)| {
            p.map = maps.into_iter().max_by_key(|(_, t)| *t).map(|(m, _)| m);
            p
        })
        .collect();
    places_played.sort_by(|a, b| {
        b.secs
            .cmp(&a.secs)
            .then(b.sessions.cmp(&a.sessions))
            .then(b.last.cmp(&a.last))
    });

    // Per map.
    let mut maps: HashMap<String, MapStatDto> = HashMap::new();
    for s in &in_range {
        if let Some(m) = &s.map {
            let e = maps.entry(m.to_lowercase()).or_insert_with(|| MapStatDto {
                map: m.clone(),
                secs: 0,
                sessions: 0,
            });
            e.secs += s.secs;
            e.sessions += 1;
        }
    }
    let mut maps: Vec<MapStatDto> = maps.into_values().collect();
    maps.sort_by(|a, b| b.secs.cmp(&a.secs).then(b.sessions.cmp(&a.sessions)));

    // Per hour of the week and days played, over the range.
    let mut week_hours = vec![vec![0i64; 24]; 7];
    let mut played_days: HashSet<i64> = HashSet::new();
    for s in &in_range {
        played_days.insert((s.start + utc_offset).div_euclid(DAY));
        split(s, utc_offset, |day, weekday, hour, piece| {
            played_days.insert(day);
            week_hours[weekday][hour] += piece;
        });
    }

    // Per day over all time: the calendar moves from year to year, and a
    // streak is a streak whatever the range.
    let mut days: BTreeMap<i64, (i64, u32)> = BTreeMap::new();
    let mut all_days: HashSet<i64> = HashSet::new();
    for s in log.iter().map(|s| dto(s, now)) {
        let first_day = (s.start + utc_offset).div_euclid(DAY);
        all_days.insert(first_day);
        days.entry(first_day).or_default().1 += 1;
        split(&s, utc_offset, |day, _, _, piece| {
            all_days.insert(day);
            days.entry(day).or_default().0 += piece;
        });
    }
    let (streak, best_streak) = streaks(&all_days, (now + utc_offset).div_euclid(DAY));

    PlayStatsDto {
        total_secs,
        sessions: in_range.len() as u32,
        places: places_played.len() as u32,
        average_secs,
        longest,
        days_played: played_days.len() as u32,
        streak,
        best_streak,
        first: log.iter().map(|s| s.start).min(),
        current: log.iter().rev().find(|s| s.open).map(|s| dto(s, now)),
        places_played,
        maps,
        days: days
            .into_iter()
            .map(|(day, (secs, sessions))| DayStatDto {
                day,
                secs,
                sessions,
            })
            .collect(),
        week_hours,
    }
}

/// Cut a measured session at each hour of the player's time: `f` gets the
/// day (since 1970-01-01), weekday (0 = Monday), hour and seconds of each
/// piece.
fn split(s: &SessionDto, utc_offset: i64, mut f: impl FnMut(i64, usize, usize, i64)) {
    let start = s.start + utc_offset;
    let end = start + s.secs;
    let mut t = start;
    while t < end {
        let piece = ((t.div_euclid(HOUR) + 1) * HOUR).min(end) - t;
        let day = t.div_euclid(DAY);
        // 1970-01-01 was a Thursday: Monday is 0.
        let weekday = (day + 3).rem_euclid(7) as usize;
        let hour = (t.rem_euclid(DAY) / HOUR) as usize;
        f(day, weekday, hour, piece);
        t += piece;
    }
}

/// The run of days with play that ends today (or yesterday, today not
/// being over), and the longest run.
fn streaks(days: &HashSet<i64>, today: i64) -> (u32, u32) {
    let mut sorted: Vec<i64> = days.iter().copied().collect();
    sorted.sort_unstable();
    let mut best = 0u32;
    let mut run = 0u32;
    let mut prev: Option<i64> = None;
    for &d in &sorted {
        run = if prev == Some(d - 1) { run + 1 } else { 1 };
        best = best.max(run);
        prev = Some(d);
    }
    let mut current = 0u32;
    let mut d = if days.contains(&today) {
        today
    } else {
        today - 1
    };
    while days.contains(&d) {
        current += 1;
        d -= 1;
    }
    (current, best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(name: &str, ip: &str, start: i64, secs: i64, map: &str) -> Session {
        Session {
            kind: SessionKind::Server,
            name: name.into(),
            ip: Some(ip.into()),
            port: Some(2302),
            map: Some(map.into()),
            start,
            end: start + secs,
            open: false,
            measured: true,
        }
    }

    // 2026-10-05 00:00 UTC, a Monday.
    const MONDAY: i64 = 1_791_158_400;

    #[test]
    fn totals_places_and_maps() {
        let log = vec![
            session("A", "1.1.1.1", MONDAY + 10 * HOUR, 2 * HOUR, "enoch"),
            session(
                "B",
                "2.2.2.2",
                MONDAY + DAY + 20 * HOUR,
                HOUR,
                "chernarusplus",
            ),
            session(
                "A",
                "1.1.1.1",
                MONDAY + 2 * DAY + 9 * HOUR,
                3 * HOUR,
                "enoch",
            ),
        ];
        let st = stats(&log, MONDAY + 3 * DAY, StatsRange::All, 0);
        assert_eq!(st.total_secs, 6 * HOUR);
        assert_eq!(st.sessions, 3);
        assert_eq!(st.places, 2);
        assert_eq!(st.average_secs, 2 * HOUR);
        assert_eq!(st.longest.as_ref().unwrap().secs, 3 * HOUR);
        assert_eq!(st.places_played[0].name, "A");
        assert_eq!(st.places_played[0].secs, 5 * HOUR);
        assert_eq!(st.places_played[0].sessions, 2);
        assert_eq!(st.places_played[0].map.as_deref(), Some("enoch"));
        assert_eq!(st.maps[0].map, "enoch");
        assert_eq!(st.days.len(), 3);
        assert_eq!((st.days_played, st.streak, st.best_streak), (3, 3, 3));
    }

    #[test]
    fn a_session_across_midnight_splits_into_days_and_hours() {
        // Sunday 23:30 to Monday 01:00 UTC; the player is at UTC+1.
        let log = vec![session("A", "1.1.1.1", MONDAY - 30 * 60, 90 * 60, "enoch")];
        let st = stats(&log, MONDAY + DAY, StatsRange::All, HOUR);
        // Local: Monday 00:30 to 02:00.
        assert_eq!(st.days.len(), 1);
        assert_eq!(st.week_hours[0][0], 30 * 60);
        assert_eq!(st.week_hours[0][1], HOUR);
        let st = stats(&log, MONDAY + DAY, StatsRange::All, 0);
        assert_eq!(st.days.len(), 2, "UTC: Sunday then Monday");
        assert_eq!(st.week_hours[6][23], 30 * 60);
    }

    #[test]
    fn the_range_the_open_session_and_unmeasured_joins() {
        let mut open = session("Now", "3.3.3.3", MONDAY + 3 * DAY, 60, "sakhal");
        open.open = true;
        let mut old = session("Old", "4.4.4.4", MONDAY - 60 * DAY, 0, "enoch");
        old.measured = false;
        let log = vec![old, open];
        let now = MONDAY + 3 * DAY + HOUR;
        let st = stats(&log, now, StatsRange::Week, 0);
        assert_eq!(st.sessions, 1, "the old join is out of the week");
        assert_eq!(st.days.len(), 2, "the calendar keeps every day");
        assert_eq!(
            st.current.as_ref().unwrap().secs,
            HOUR,
            "open runs until now"
        );
        let all = stats(&log, now, StatsRange::All, 0);
        assert_eq!(all.sessions, 2);
        assert_eq!(
            all.average_secs, HOUR,
            "the unmeasured join is not averaged"
        );
        assert_eq!(
            all.places_played
                .iter()
                .find(|p| p.name == "Old")
                .unwrap()
                .unmeasured,
            1
        );
        assert_eq!(all.first, Some(MONDAY - 60 * DAY));
    }

    #[test]
    fn streaks_end_today_or_yesterday() {
        let days: HashSet<i64> = [1, 2, 3, 7, 8].into_iter().collect();
        assert_eq!(streaks(&days, 9), (2, 3));
        assert_eq!(streaks(&days, 8), (2, 3));
        assert_eq!(streaks(&days, 10), (0, 3));
    }
}
