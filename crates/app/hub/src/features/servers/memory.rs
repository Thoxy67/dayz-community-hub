//! What Steam listings learned, kept beside the server list cache
//! (`steam_listing_cache.json`), so a restart (one after an update
//! included) neither lists Steam's master again minutes after a listing nor
//! queries again the addresses that answered as fakes:
//!
//! - when the last complete listing started and finished, and its figures
//!   (shown when the next one is skipped as too recent);
//! - the requests it asked that stayed under the cap
//!   (`dz_api::PlannedPart`), so the next listing asks them straight away
//!   rather than hitting the cap again to find the same splits;
//! - each address it queried: when Steam last listed it and how it last
//!   answered, so fakes, unreadable mods, silence and duplicates are not
//!   asked again for a while, and the servers it added are its own again
//!   after a restart (not "kept" servers to check).
//!
//! The file is only a shortcut: missing, unreadable or from another
//! version, it is ignored field by field, and the listing runs as it would
//! without it.

use std::path::{Path, PathBuf};

use dz_api::PlannedPart;
use serde::{Deserialize, Serialize};

use super::sources::SteamStatus;

/// What this build writes.
const VERSION: u32 = 1;
/// A complete listing is not run again sooner than this after it finished.
pub(crate) const MIN_INTERVAL: i64 = 10 * 60;
/// The servers the last listing added are its own after a restart while it
/// finished less than this ago; older, they are checked like any server a
/// list no longer has.
const RESTORE_FOR: i64 = 24 * 3600;
/// Addresses Steam has not listed for this long are forgotten (unless
/// their verdict still holds).
const FORGET_AFTER: i64 = 3 * 24 * 3600;
/// A clock this far ahead of a saved time is taken as wrong, and the time
/// as unknown.
const CLOCK_SKEW: i64 = 60;

/// How an address answered when it was last queried.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    /// Answered and was added to the list.
    Answered,
    /// Reported bots: a farm's emulator.
    Fake,
    /// Modded, its mods unreadable.
    NoMods,
    /// Did not answer, twice.
    Silent,
    /// Combed out: another entry answers for its address to join.
    Duplicate,
    /// Written by another version.
    #[serde(other)]
    Unknown,
}

impl Verdict {
    /// How long (seconds) the address is not queried again after this
    /// answer. A server that answered is in the list and joins the ping
    /// scan; one that went silent may be back within the hour.
    fn holds(self) -> i64 {
        match self {
            Verdict::Fake | Verdict::Duplicate => 12 * 3600,
            Verdict::NoMods => 6 * 3600,
            Verdict::Silent => 3600,
            Verdict::Answered | Verdict::Unknown => 0,
        }
    }
}

/// One address as remembered.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Memo {
    /// When a listing last listed it (Unix seconds).
    pub seen: i64,
    pub verdict: Verdict,
    /// When it last answered so.
    pub at: i64,
}

impl Memo {
    /// Whether its verdict still holds at `now`: not queried again.
    pub(crate) fn holds(&self, now: i64) -> bool {
        self.at <= now + CLOCK_SKEW && now - self.at < self.verdict.holds()
    }
}

/// [`Memo`] with its address, as the file holds it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
struct AddrMemo {
    ip: String,
    port: i64,
    #[serde(flatten)]
    memo: Memo,
}

/// The file.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub(crate) struct ListingMemory {
    pub version: u32,
    /// The last complete listing: when it started and finished (Unix
    /// seconds), and its figures as the sources bar showed them.
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub status: SteamStatus,
    /// The requests it asked that stayed under the cap.
    pub plan: Vec<PlannedPart>,
    addresses: Vec<AddrMemo>,
}

/// An (ip, query port) pair.
pub(crate) type Addr = (String, i64);

impl ListingMemory {
    /// Whether the last complete listing finished less than
    /// [`MIN_INTERVAL`] before `now`: then when the next may run.
    pub(crate) fn next_listing(&self, now: i64) -> Option<i64> {
        let done = self.completed_at.filter(|&t| t <= now + CLOCK_SKEW)?;
        (now - done < MIN_INTERVAL).then_some(done + MIN_INTERVAL)
    }

    /// When the listing whose servers are still its own started: the last
    /// complete one, if it finished less than [`RESTORE_FOR`] ago.
    pub(crate) fn restorable_since(&self, now: i64) -> Option<i64> {
        let done = self.completed_at.filter(|&t| t <= now + CLOCK_SKEW)?;
        if now - done >= RESTORE_FOR {
            return None;
        }
        self.started_at.filter(|&s| s <= done)
    }

    /// The addresses, taken out of the file's form.
    pub(crate) fn take_addresses(&mut self) -> impl Iterator<Item = (Addr, Memo)> + use<> {
        std::mem::take(&mut self.addresses)
            .into_iter()
            .map(|a| ((a.ip, a.port), a.memo))
    }

    /// Put `addresses` in for saving, without those forgotten by `now`.
    pub(crate) fn set_addresses<'a>(
        &mut self,
        addresses: impl Iterator<Item = (&'a Addr, &'a Memo)>,
        now: i64,
    ) {
        self.addresses = addresses
            .filter(|(_, m)| now - m.seen < FORGET_AFTER || m.holds(now))
            .map(|((ip, port), memo)| AddrMemo {
                ip: ip.clone(),
                port: *port,
                memo: *memo,
            })
            .collect();
        // One order whatever the map's: the file changes only when it does.
        self.addresses
            .sort_unstable_by(|a, b| (&a.ip, a.port).cmp(&(&b.ip, b.port)));
    }
}

/// `<data dir>/steam_listing_cache.json`.
pub(crate) fn path() -> PathBuf {
    crate::paths::data_dir().join("steam_listing_cache.json")
}

/// Read the file at `path`; `None` when missing or unreadable. Parsed on
/// the blocking pool.
pub(crate) async fn load(path: &Path) -> Option<ListingMemory> {
    let data = tokio::fs::read(path).await.ok()?;
    tokio::task::spawn_blocking(move || serde_json::from_slice::<ListingMemory>(&data).ok())
        .await
        .ok()
        .flatten()
}

/// Write `memory` to `path` atomically, on the blocking pool.
pub(crate) async fn save(path: PathBuf, mut memory: ListingMemory) -> Result<(), String> {
    memory.version = VERSION;
    tokio::task::spawn_blocking(move || {
        let data = serde_json::to_vec(&memory).map_err(|e| e.to_string())?;
        dz_api::write_atomic(&path, &data).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("listing memory write task failed: {e}"))?
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
mod tests {
    use super::*;
    use dz_api::WebPartition;

    const NOW: i64 = 1_800_000_000;

    fn memo(verdict: Verdict, ago: i64) -> Memo {
        Memo {
            seen: NOW - ago,
            verdict,
            at: NOW - ago,
        }
    }

    #[test]
    fn a_verdict_holds_for_a_while_then_the_address_is_asked_again() {
        assert!(memo(Verdict::Fake, 3600).holds(NOW));
        assert!(!memo(Verdict::Fake, 13 * 3600).holds(NOW));
        assert!(memo(Verdict::Silent, 600).holds(NOW));
        assert!(!memo(Verdict::Silent, 3700).holds(NOW));
        assert!(
            !memo(Verdict::Answered, 10).holds(NOW),
            "answered: in the list"
        );
        // A time in the future is a clock gone wrong: not trusted.
        assert!(!memo(Verdict::Fake, -3600).holds(NOW));
    }

    #[test]
    fn a_listing_finished_minutes_ago_is_not_run_again() {
        let mut m = ListingMemory {
            started_at: Some(NOW - 400),
            completed_at: Some(NOW - 300),
            ..Default::default()
        };
        assert_eq!(m.next_listing(NOW), Some(NOW - 300 + MIN_INTERVAL));
        assert_eq!(m.restorable_since(NOW), Some(NOW - 400));
        m.completed_at = Some(NOW - MIN_INTERVAL);
        assert_eq!(m.next_listing(NOW), None);
        m.completed_at = Some(NOW + 3600);
        assert_eq!(m.next_listing(NOW), None, "finished in the future: no");
        assert_eq!(m.restorable_since(NOW), None);
        m.completed_at = Some(NOW - RESTORE_FOR);
        assert_eq!(m.restorable_since(NOW), None);
        assert_eq!(ListingMemory::default().next_listing(NOW), None);
    }

    #[tokio::test]
    async fn the_file_round_trips_and_forgets_old_addresses() {
        let dir = std::env::temp_dir().join(format!("dz-hub-listing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("steam_listing_cache.json");
        let addrs: Vec<(Addr, Memo)> = vec![
            (("1.1.1.1".into(), 27016), memo(Verdict::Answered, 60)),
            (("2.2.2.2".into(), 27016), memo(Verdict::Fake, 3600)),
            // Not listed for a week and its verdict over: forgotten.
            (("3.3.3.3".into(), 27016), memo(Verdict::Silent, 7 * 86_400)),
        ];
        let mut m = ListingMemory {
            started_at: Some(NOW - 200),
            completed_at: Some(NOW - 100),
            plan: vec![PlannedPart {
                part: WebPartition::new(Some(true), "\\map\\enoch"),
                rows: 42,
            }],
            ..Default::default()
        };
        m.status.added = 7;
        m.set_addresses(addrs.iter().map(|(a, m)| (a, m)), NOW);
        save(file.clone(), m).await.unwrap();
        let mut back = load(&file).await.unwrap();
        assert_eq!(back.version, VERSION);
        assert_eq!(back.completed_at, Some(NOW - 100));
        assert_eq!(back.status.added, 7);
        assert_eq!(back.plan[0].rows, 42);
        let got: Vec<(Addr, Memo)> = back.take_addresses().collect();
        assert_eq!(got, addrs[..2].to_vec());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_missing_corrupt_or_foreign_file_is_ignored() {
        let dir = std::env::temp_dir().join(format!("dz-hub-listing-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("steam_listing_cache.json");
        assert!(load(&file).await.is_none());
        std::fs::write(&file, b"{\"completed_at\": 12, \"plan\": [tr").unwrap();
        assert!(load(&file).await.is_none());
        // Another version's fields and verdicts: what is known is read.
        std::fs::write(
            &file,
            br#"{"version":9,"completed_at":12,"later":true,"status":{"added":3,"new":1},
                "addresses":[{"ip":"1.1.1.1","port":2,"seen":3,"verdict":"maybe","at":4}]}"#,
        )
        .unwrap();
        let mut m = load(&file).await.unwrap();
        assert_eq!(m.completed_at, Some(12));
        assert_eq!(m.status.added, 3);
        let a: Vec<(Addr, Memo)> = m.take_addresses().collect();
        assert_eq!(a[0].1.verdict, Verdict::Unknown);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
