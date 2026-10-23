//! The site ids of servers already found, kept on disk between runs: a search
//! by name takes about a second and one by IP eight or more, plus up to dozens
//! of candidates' pages, while a known id costs two requests. [`lookup`](crate::lookup)
//! checks a known id against the server's address before trusting it, so an
//! entry that went stale only costs the search it would have cost anyway.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Resolved;

/// Entries kept at most; the oldest go first.
const MAX_ENTRIES: usize = 500;

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct Entry {
    id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    avg: Option<f64>,
    /// When it was found, Unix seconds.
    at: i64,
}

/// Server key (`ip:game_port:query_port`) → its site id, as found.
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(transparent)]
pub struct KnownIds(HashMap<String, Entry>);

impl KnownIds {
    /// The file at `path`; empty when it is missing or broken. Blocking.
    pub fn load(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Write the file (atomically, flushed). Blocking.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        dz_common::fs::write_atomic(path, &bytes)
    }

    /// The id found for `key`, unless older than `max_age_secs` at `now` (Unix
    /// seconds).
    pub fn get(&self, key: &str, max_age_secs: i64, now: i64) -> Option<Resolved> {
        self.0
            .get(key)
            .filter(|e| now.saturating_sub(e.at) < max_age_secs)
            .map(|e| Resolved {
                id: e.id,
                avg_players_7d: e.avg,
            })
    }

    /// Remember `r` for `key`, found at `now`; true when that changed anything.
    pub fn put(&mut self, key: &str, r: Resolved, now: i64) -> bool {
        if let Some(e) = self.0.get(key)
            && e.id == r.id
            && e.avg == r.avg_players_7d
            && now.saturating_sub(e.at) < 3600
        {
            return false;
        }
        self.0.insert(
            key.to_owned(),
            Entry {
                id: r.id,
                avg: r.avg_players_7d,
                at: now,
            },
        );
        if self.0.len() > MAX_ENTRIES {
            let mut ages: Vec<(i64, String)> =
                self.0.iter().map(|(k, e)| (e.at, k.clone())).collect();
            ages.sort_unstable();
            for (_, k) in ages.into_iter().take(self.0.len() - MAX_ENTRIES) {
                self.0.remove(&k);
            }
        }
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_age_and_stay_bounded() {
        let mut k = KnownIds::default();
        let r = Resolved {
            id: 42,
            avg_players_7d: Some(12.5),
        };
        assert!(k.put("1.2.3.4:2302:27016", r, 1000));
        assert!(!k.put("1.2.3.4:2302:27016", r, 1001));
        let dir = std::env::temp_dir().join(format!("dz-dm-ids-{}", std::process::id()));
        let path = dir.join("ids.json");
        k.save(&path).unwrap();
        let back = KnownIds::load(&path);
        let got = back.get("1.2.3.4:2302:27016", 100, 1050).unwrap();
        assert_eq!((got.id, got.avg_players_7d), (42, Some(12.5)));
        assert!(back.get("1.2.3.4:2302:27016", 100, 1200).is_none());
        assert!(KnownIds::load(&dir.join("missing.json")).0.is_empty());
        for i in 0..(MAX_ENTRIES + 10) {
            k.put(&format!("k{i}"), r, 2000 + i as i64);
        }
        assert_eq!(k.0.len(), MAX_ENTRIES);
        assert!(k.get("k0", i64::MAX, 0).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
