//! Play sessions: when DayZ ran, on which server or offline map, and for how
//! long. Kept in `sessions.json` beside the profile, not in it: the profile
//! is rewritten on every change and this log only grows. Being a `.json` in
//! the data folder, it travels with a profile export and comes back with an
//! import.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{History, Snapshot, set_aside};

/// The log's file name, in the data folder.
pub const SESSIONS_FILE: &str = "sessions.json";

/// What a session was played on.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SessionKind {
    /// A server joined through the launcher.
    #[default]
    Server,
    /// An offline mission (Community Offline Mode).
    Offline,
    /// DayZ started some other way (Steam, its own launcher): played, but
    /// where is not known.
    Unknown,
}

/// One stretch of DayZ running.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Session {
    #[serde(default)]
    pub kind: SessionKind,
    /// The server's name, or the offline mission's folder.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    /// The game port, for joining again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<String>,
    /// Unix seconds.
    pub start: i64,
    /// Unix seconds. While the game runs, the last time it was seen running.
    pub end: i64,
    /// The game is running in this session now.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open: bool,
    /// Seen running from start to end. False for a join carried over from
    /// the history the launcher kept before it measured anything: it
    /// happened, how long it lasted is not known.
    #[serde(default = "yes", skip_serializing_if = "Clone::clone")]
    pub measured: bool,
}

fn yes() -> bool {
    true
}

impl Session {
    /// How long it lasted, in seconds: 0 when not measured.
    pub fn secs(&self) -> i64 {
        if self.measured {
            (self.end - self.start).max(0)
        } else {
            0
        }
    }

    /// The same server or mission: what per-server figures group by.
    pub fn place_key(&self) -> String {
        match (&self.ip, self.port) {
            (Some(ip), Some(port)) => format!("{ip}:{port}"),
            _ => format!("{:?}:{}", self.kind, self.name),
        }
    }
}

/// Every session, oldest first.
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PlayLog {
    #[serde(default)]
    pub sessions: Vec<Session>,
    #[serde(skip)]
    pub path: PathBuf,
}

impl PlayLog {
    /// Read the log at `path`. Missing, it starts from `history` (each join
    /// a session of unknown length) so the stats have the past to show; a
    /// file that does not parse is set aside and the log starts empty.
    pub fn load(path: impl AsRef<Path>, history: &[History]) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut log = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<PlayLog>(&text) {
                Ok(log) => log,
                Err(e) => {
                    set_aside(&path, &e);
                    PlayLog::default()
                }
            },
            Err(_) => PlayLog::from_history(history),
        };
        log.path = path;
        log
    }

    /// The joins the old history remembers, as sessions of unknown length.
    pub fn from_history(history: &[History]) -> Self {
        let mut sessions: Vec<Session> = history
            .iter()
            .map(|h| Session {
                kind: SessionKind::Server,
                name: h.name.clone(),
                ip: Some(h.ip.clone()),
                port: Some(h.port),
                map: None,
                start: h.ts,
                end: h.ts,
                open: false,
                measured: false,
            })
            .collect();
        sessions.sort_by_key(|s| s.start);
        PlayLog {
            sessions,
            path: PathBuf::new(),
        }
    }

    /// The session running now, if any.
    pub fn open_mut(&mut self) -> Option<&mut Session> {
        self.sessions.iter_mut().rev().find(|s| s.open)
    }

    /// The log as it should be written, for [`crate::Writer`].
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            path: self.path.clone(),
            data: serde_json::to_string(self).unwrap_or_else(|_| "{}".into()),
            seq: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(name: &str, ts: i64) -> History {
        History {
            name: name.into(),
            ip: "1.2.3.4".into(),
            port: 2302,
            ts,
            extra: None,
        }
    }

    #[test]
    fn a_missing_log_starts_from_the_history_unmeasured() {
        let dir = std::env::temp_dir().join(format!("dzch-sessions-{}", std::process::id()));
        let path = dir.join(SESSIONS_FILE);
        let log = PlayLog::load(&path, &[history("B", 200), history("A", 100)]);
        assert_eq!(log.sessions.len(), 2);
        assert_eq!(log.sessions[0].name, "A", "oldest first");
        assert!(log.sessions.iter().all(|s| !s.measured && s.secs() == 0));
        assert_eq!(log.path, path);
    }

    #[test]
    fn a_session_round_trips_and_old_fields_default() {
        let s = Session {
            kind: SessionKind::Offline,
            name: "DayZCommunityOfflineMode.Enoch".into(),
            ip: None,
            port: None,
            map: Some("enoch".into()),
            start: 1_000,
            end: 4_600,
            open: false,
            measured: true,
        };
        let text = serde_json::to_string(&s).unwrap();
        assert!(
            !text.contains("measured") && !text.contains("open"),
            "{text}"
        );
        assert_eq!(serde_json::from_str::<Session>(&text).unwrap(), s);
        assert_eq!(s.secs(), 3_600);

        let bare: Session = serde_json::from_str(r#"{"name":"x","start":5,"end":9}"#).unwrap();
        assert_eq!(bare.kind, SessionKind::Server);
        assert!(bare.measured && !bare.open);
    }
}
