//! Where the server list comes from, and how far each source has got: the
//! launcher list (dayzsalauncher.com), Steam's master (Web API),
//! the servers kept from earlier lists. Shown above the server list; sent
//! as `sources-changed` at most a few times a second.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use crate::{Hub, HubEvent};
use serde::{Deserialize, Serialize};

use dz_api::ServerList;

/// How often `sources-changed` may fire.
const EMIT_EVERY: Duration = Duration::from_millis(250);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Running,
    Done,
    Failed,
    /// Turned off, or nothing to reach it with.
    Off,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct DzsaStatus {
    pub phase: Phase,
    /// Servers in its last list.
    pub count: u32,
    pub error: Option<String>,
    /// When its list was fetched, Unix seconds.
    pub at: Option<i64>,
}

/// Steam's listing as it stands. Also kept on disk with what the last
/// listing learned (`memory`): unknown fields are ignored, missing ones
/// default.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct SteamStatus {
    pub phase: Phase,
    /// Not listed again: the last listing (its figures here, `finished`
    /// when it ended) is less than ten minutes old, maybe from before a
    /// restart. `phase` is `Done`.
    pub recent: bool,
    /// While `recent`: when a listing may run again (Unix seconds).
    pub next_at: Option<i64>,
    /// Off because the profile has no Steam Web API key: the only way the
    /// master is asked.
    pub needs_key: bool,
    /// Why the listing failed: a sentence.
    pub note: Option<String>,
    /// Requests to the master answered, and planned so far (splits add some).
    pub requests_done: u32,
    pub requests_total: u32,
    /// What is being asked now.
    pub current: Option<String>,
    /// The master refused; asked again at this time (Unix seconds).
    pub waiting_until: Option<i64>,
    /// Addresses the master listed.
    pub listed: u32,
    /// Of those, on farm addresses: left out.
    pub farms: u32,
    /// Of those, already in the list.
    pub known: u32,
    /// Of those, not queried: they answered as fakes, without readable
    /// mods, as duplicates or not at all, recently (the verdict is kept a
    /// few hours).
    pub remembered: u32,
    /// Queried (A2S), and of those answered.
    pub queried: u32,
    pub answered: u32,
    /// Modded servers whose mods could not be read: left out.
    pub no_mods: u32,
    /// Answered reporting bots, a farm's fake count: left out.
    pub fakes: u32,
    /// Added to the list.
    pub added: u32,
    pub started: Option<i64>,
    pub finished: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct KeptStatus {
    pub phase: Phase,
    /// Servers earlier lists had that the current one lacks.
    pub total: u32,
    pub checked: u32,
    /// Still answering, so kept.
    pub alive: u32,
    /// Did not answer: removed.
    pub removed: u32,
    /// Answered reporting bots, a farm's fake count: removed.
    pub fakes: u32,
}

/// Everything above, and what the list holds now.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct SourcesChanged {
    pub dzsa: DzsaStatus,
    pub steam: SteamStatus,
    pub kept: KeptStatus,
    /// Entries combed out as duplicates of another (same address to join).
    pub duplicates: u32,
    /// Servers in the list, and of those official ones.
    pub total: u32,
    pub official: u32,
}

static STATUS: LazyLock<Mutex<SourcesChanged>> = LazyLock::new(Mutex::default);
static DIRTY: AtomicBool = AtomicBool::new(false);
/// Poked by every [`update`]: wakes the notifier.
static CHANGED: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Change the status; the window hears of it shortly.
pub(crate) fn update(f: impl FnOnce(&mut SourcesChanged)) {
    let mut s = STATUS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut s);
    DIRTY.store(true, Ordering::Release);
    drop(s);
    CHANGED.notify_one();
}

/// Steam's listing as it stands.
pub(crate) fn steam() -> SteamStatus {
    STATUS.lock().map(|s| s.steam.clone()).unwrap_or_default()
}

/// Count what `list` holds.
pub(crate) fn counted(list: &ServerList) {
    let official = list.result.iter().filter(|s| s.official).count() as u32;
    update(|s| {
        s.total = list.result.len() as u32;
        s.official = official;
    });
}

pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// The sources as they stand.
impl Hub {
    /// The sources as they stand.
    pub async fn sources_status(&self) -> SourcesChanged {
        STATUS.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// Emit `sources-changed` when the status moved, at most every
/// [`EMIT_EVERY`]. Runs for the life of the app, asleep while nothing moves.
pub(crate) fn spawn_notifier(hub: Hub) {
    tokio::spawn(async move {
        crate::wake::on_change(&CHANGED, EMIT_EVERY, || {
            if DIRTY.swap(false, Ordering::AcqRel) {
                let s = STATUS.lock().map(|s| s.clone()).unwrap_or_default();
                hub.emit(HubEvent::SourcesChanged(Box::new(s)));
            }
            true
        })
        .await;
    });
}
