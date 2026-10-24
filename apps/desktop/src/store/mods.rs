//! Installed Workshop mods, their update state, and the one download operation that may
//! run at a time (install, update, repair, login), with its live log (=
//! `stores/mods.svelte.ts`). SteamCMD or the running Steam client downloads, as the
//! profile says: both report the same steps, as `HubEvent::ModProgress`.
//!
//! The hub keeps the operation too: a window started while one runs, or before its
//! summary was closed, gets it back with its queue, results and log ([`Mods::resume`]).

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dz_hub::events::ModProgressEvent;
use dz_hub::features::mods::usage::{self, SavedModsDto, ServerRemoval, Unused};
use dz_hub::features::mods::{InstalledModDto, ModUsageDto};
use dz_hub::features::steamcmd::op::{self as hub_op, ModOpSnapshot};
use dz_hub::features::steamcmd::{ModOpType, ModProgressKind};
use dz_hub::features::steamworks::{SteamSubscriptionDto, SteamSubscriptionsDto, WorkshopItemDto};
use dz_hub::{Hub, HubEvent};
use rustc_hash::FxHashMap;

use super::connect::Launch;
use super::{Bus, ConfirmRequest, Signal};
use crate::i18n::{plural, t, tf};
use crate::rt::{self, Task};

const UPDATES_TTL_MS: u128 = 5 * 60 * 1000;
/// A removed mod Steam calls subscribed again after this long was subscribed again
/// (an earlier answer can predate the removal).
const RESUBSCRIBED_AFTER: Duration = Duration::from_secs(30);
/// Lines of SteamCMD output kept for the dialog; past this the oldest go.
pub const LOG_MAX: usize = 5000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModOpPhase {
    #[default]
    Downloading,
    SteamGuardMobile,
    PasswordRequired,
    Finished,
}

impl From<hub_op::ModOpPhase> for ModOpPhase {
    fn from(p: hub_op::ModOpPhase) -> Self {
        match p {
            hub_op::ModOpPhase::Downloading => Self::Downloading,
            hub_op::ModOpPhase::SteamGuardMobile => Self::SteamGuardMobile,
            hub_op::ModOpPhase::PasswordRequired => Self::PasswordRequired,
            hub_op::ModOpPhase::Finished => Self::Finished,
        }
    }
}

/// What downloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModOpVia {
    #[default]
    Steamcmd,
    Steamworks,
}

impl From<hub_op::ModOpVia> for ModOpVia {
    fn from(v: hub_op::ModOpVia) -> Self {
        match v {
            hub_op::ModOpVia::Steamcmd => Self::Steamcmd,
            hub_op::ModOpVia::Steamworks => Self::Steamworks,
        }
    }
}

/// A mod that came through or failed, and how long it took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completed {
    pub id: u64,
    pub name: String,
    pub ok: bool,
    pub ms: u64,
}

/// The running (or finished, not yet dismissed) operation.
#[derive(Clone, Debug)]
pub struct ModOp {
    pub active: bool,
    pub kind: ModOpType,
    pub via: ModOpVia,
    pub phase: ModOpPhase,
    pub current: usize,
    pub total: usize,
    pub current_name: String,
    /// The mod being worked on (0 before the first starts).
    pub current_id: u64,
    /// Every mod this operation works on: guessed when it starts, then the hub's list.
    pub ids: Vec<u64>,
    pub names: FxHashMap<u64, String>,
    pub completed: Vec<Completed>,
    pub ok: usize,
    pub failed: usize,
    pub hint: Option<String>,
    /// Raw SteamCMD output, with when each line came (ms after the start). The live
    /// download line is rewritten in place.
    pub log: VecDeque<(u64, String)>,
    /// Bumped whenever the log changes: a view caches its galleys on it.
    pub log_rev: u64,
    /// Lines dropped from the front of the log so far ([`LOG_MAX`]): a view that keeps
    /// a copy of it drops as many.
    pub log_dropped: u64,
    pub started_at: Instant,
    /// When the mod being worked on started.
    pub item_started_at: Instant,
    /// The hub's numbers for the mod being downloaded (`at_ms` after the start, on
    /// this window's clock): the bar, the speed and the time left come from them.
    pub bytes: Option<hub_op::LiveBytesDto>,
    /// The dialog is closed while the operation carries on; the status bar shows it.
    pub minimised: bool,
}

impl ModOp {
    pub(crate) fn idle() -> Self {
        let now = Instant::now();
        Self {
            active: false,
            kind: ModOpType::UpdateAll,
            via: ModOpVia::Steamcmd,
            phase: ModOpPhase::Downloading,
            current: 0,
            total: 0,
            current_name: String::new(),
            current_id: 0,
            ids: Vec::new(),
            names: FxHashMap::default(),
            completed: Vec::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log: VecDeque::new(),
            log_rev: 0,
            log_dropped: 0,
            started_at: now,
            item_started_at: now,
            bytes: None,
            minimised: false,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Where a mod stands in the running operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpState {
    Downloading,
    Queued,
}

/// The arguments of an operation (`ModOpArgs`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpArgs {
    pub ip: Option<String>,
    pub port: Option<i64>,
    pub mod_id: Option<u64>,
    pub mod_name: Option<String>,
    pub mod_ids: Option<Vec<u64>>,
    pub mod_names: Option<Vec<String>>,
}

pub struct Mods {
    hub: Hub,
    bus: Bus,
    pub installed: Vec<InstalledModDto>,
    /// Moves whenever `installed` changed (a read, a check, a removal, a link flipped):
    /// views key on it instead of hashing every mod.
    pub revision: u64,
    pub loading: bool,
    /// The disk has been read at least once (an empty list is then an answer).
    pub loaded: bool,
    pub checking: bool,
    last_checked: Option<Instant>,
    pub op: ModOp,
    /// What the running Steam client says about the account's Workshop items.
    pub steam: Option<SteamSubscriptionsDto>,
    pub steam_error: Option<String>,
    /// Which favourites and played servers run each mod, by id.
    pub usage: FxHashMap<u64, ModUsageDto>,
    /// The saved servers' mods (`None` until the hub answered), for [`Mods::unused`].
    pub saved: Option<SavedModsDto>,
    /// The installed mods nothing needs (see `dz_hub::features::mods::usage`), worked
    /// out again whenever the mods, Steam's details or the saved servers move.
    unused: Option<Unused>,
    by_id: FxHashMap<u64, usize>,
    steam_by_id: FxHashMap<u64, usize>,
    workshop: FxHashMap<u64, usize>,
    required_by: FxHashMap<u64, Vec<u64>>,
    stale_count: usize,
    total_size: u64,

    load_task: Option<Task<Result<Vec<InstalledModDto>, String>>>,
    usage_task: Option<Task<Result<Vec<ModUsageDto>, String>>>,
    saved_task: Option<Task<Result<SavedModsDto, String>>>,
    /// The saved servers changed while they were being read: read them again.
    saved_again: bool,
    /// What the saved servers were last read against: the list's generation, its
    /// profile flags' and the profile's writes ([`Mods::follow_saved`]).
    saved_key: (u64, u64, u64),
    check_task: Option<Task<Result<Vec<InstalledModDto>, String>>>,
    steam_task: Option<Task<Result<SteamSubscriptionsDto, String>>>,
    steam_before: Vec<u64>,
    start_task: Option<Task<Result<(), String>>>,
    attach_task: Option<Task<Option<ModOpSnapshot>>>,
    /// Steps that came while the snapshot was asked for.
    early: Vec<ModProgressEvent>,
    /// Steps at or below this were in the snapshot.
    snap_seq: u32,
    /// The operation's steps are being followed.
    listening: bool,
    /// The last log line is SteamCMD's live progress line.
    last_was_progress: bool,
    /// What goes on once every mod came through.
    on_success: Option<Launch>,
    mutate: Vec<Task<Result<MutateDone, String>>>,
    /// Mods the player removed this run, and when. Steam deletes its copy of an
    /// unsubscribed item in its own time: until then the folder is still on disk and
    /// Steam's answers may still name it, so these are kept out of the list.
    removed: FxHashMap<u64, Instant>,
    /// A read was asked for while one was under way: read again once it is in.
    load_again: bool,
    /// A forced check was asked for while one ran: run it after.
    check_again: bool,
    /// Set from the hub's watcher thread when the mod folders changed: read them again.
    disk_changed: Arc<AtomicBool>,
    /// The hub is (re)arming the watch on the folders, which follows each read.
    watch_task: Option<Task<Result<(), String>>>,
}

/// What a write to the mods did, for the toast and the follow-up.
enum MutateDone {
    Reload(Vec<String>),
    /// Mods removed (`removed`), those whose Steam copy stays (`kept`), the toasts.
    Removed {
        removed: Vec<u64>,
        kept: Vec<u64>,
        says: Vec<String>,
    },
    Managed(u64, bool, String),
    /// The link could not be changed: the optimistic flip is undone by a re-read.
    ManagedFailed(String),
}

impl Mods {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            installed: Vec::new(),
            revision: 0,
            loading: false,
            loaded: false,
            checking: false,
            last_checked: None,
            op: ModOp::idle(),
            steam: None,
            steam_error: None,
            usage: FxHashMap::default(),
            saved: None,
            unused: None,
            by_id: FxHashMap::default(),
            steam_by_id: FxHashMap::default(),
            workshop: FxHashMap::default(),
            required_by: FxHashMap::default(),
            stale_count: 0,
            total_size: 0,
            load_task: None,
            usage_task: None,
            saved_task: None,
            saved_again: false,
            saved_key: (0, 0, 0),
            check_task: None,
            steam_task: None,
            steam_before: Vec::new(),
            start_task: None,
            attach_task: None,
            early: Vec::new(),
            snap_seq: 0,
            listening: false,
            last_was_progress: false,
            on_success: None,
            mutate: Vec::new(),
            removed: FxHashMap::default(),
            load_again: false,
            check_again: false,
            disk_changed: Arc::new(AtomicBool::new(false)),
            watch_task: None,
        }
    }

    // ── derived ─────────────────────────────────────────────────────────────

    /// Take a new list in. True when what the server rows show of it moved (which mods
    /// are on disk, their versions, their update flags), so the rows look again only then.
    fn set_installed(&mut self, mut list: Vec<InstalledModDto>) -> bool {
        // A removed mod whose folder is gone is gone for good; one still on disk is
        // Steam's copy waiting to be deleted: kept out of the list until the watch on
        // the folders sees it go.
        let on_disk: Vec<u64> = list.iter().map(|m| m.id).collect();
        let before = self.removed.len();
        self.removed.retain(|id, _| on_disk.contains(id));
        if self.removed.len() < before {
            // Steam deleted a copy: its view of the subscriptions has moved too.
            self.load_steam();
        }
        list.retain(|m| !self.removed.contains_key(&m.id));
        if list == self.installed {
            return false;
        }
        let shown = |m: &InstalledModDto| (m.id, m.local_updated, m.update_available);
        let rows_moved = list.len() != self.installed.len()
            || list
                .iter()
                .zip(&self.installed)
                .any(|(a, b)| shown(a) != shown(b));
        self.revision += 1;
        self.installed = list;
        self.by_id = self
            .installed
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id, i))
            .collect();
        self.stale_count = self.installed.iter().filter(|m| m.update_available).count();
        self.total_size = self.installed.iter().map(|m| m.size).sum();
        rows_moved
    }

    fn set_steam(&mut self, mut s: SteamSubscriptionsDto) {
        // A removed mod Steam still lists: an answer from before the removal, unless
        // it is subscribed again well after it.
        let removed = &mut self.removed;
        s.items.retain(|i| match removed.get(&i.id) {
            Some(at) if i.subscribed && at.elapsed() > RESUBSCRIBED_AFTER => {
                removed.remove(&i.id);
                true
            }
            Some(_) => false,
            None => true,
        });
        self.steam_by_id = s.items.iter().enumerate().map(|(i, x)| (x.id, i)).collect();
        self.workshop = s
            .details
            .iter()
            .enumerate()
            .map(|(i, d)| (d.id, i))
            .collect();
        let mut req: FxHashMap<u64, Vec<u64>> = FxHashMap::default();
        for d in &s.details {
            for c in &d.requires {
                req.entry(*c).or_default().push(d.id);
            }
        }
        self.required_by = req;
        self.steam = Some(s);
    }

    pub fn by_id(&self, id: u64) -> Option<&InstalledModDto> {
        self.by_id.get(&id).and_then(|i| self.installed.get(*i))
    }

    pub fn steam_item(&self, id: u64) -> Option<&SteamSubscriptionDto> {
        let s = self.steam.as_ref()?;
        self.steam_by_id.get(&id).and_then(|i| s.items.get(*i))
    }

    /// The Workshop's title and required items.
    pub fn workshop(&self, id: u64) -> Option<&WorkshopItemDto> {
        let s = self.steam.as_ref()?;
        self.workshop.get(&id).and_then(|i| s.details.get(*i))
    }

    /// The Workshop items `id` requires, when Steam told.
    pub fn requires(&self, id: u64) -> Vec<u64> {
        self.workshop(id)
            .map(|w| w.requires.clone())
            .unwrap_or_default()
    }

    /// The installed mods no favourite, history server or offline choice needs:
    /// `None` until the disk and the saved servers were read.
    pub fn unused(&self) -> Option<&Unused> {
        self.unused.as_ref()
    }

    fn recompute_unused(&mut self) {
        self.unused = match (&self.saved, self.loaded) {
            (Some(saved), true) => {
                // A mod being downloaded (here or by Steam) is wanted: never offered.
                let ids: Vec<u64> = self
                    .installed
                    .iter()
                    .map(|m| m.id)
                    .filter(|id| {
                        self.op_state(*id).is_none()
                            && !self
                                .steam_item(*id)
                                .is_some_and(|i| i.downloading || i.pending)
                    })
                    .collect();
                Some(usage::unused_mods(&ids, saved, &|id| self.requires(id)))
            }
            _ => None,
        };
    }

    /// What removing the mods of the server `this` (running `server_mods`)
    /// would remove and keep, by `saved` (a fresh read of the saved servers).
    pub fn server_removal(
        &self,
        server_mods: &[u64],
        this: &usage::ServerRef,
        saved: &SavedModsDto,
    ) -> ServerRemoval {
        let installed: rustc_hash::FxHashSet<u64> = self.installed.iter().map(|m| m.id).collect();
        usage::server_removal(server_mods, this, &installed, saved, &|id| {
            self.requires(id)
        })
    }

    /// Take a fresh read of the saved servers (a command asked for one).
    pub fn set_saved(&mut self, saved: SavedModsDto) {
        self.saved = Some(saved);
        self.recompute_unused();
    }

    /// Read the saved servers' mods again (a favourite, the history or the offline
    /// choice changed).
    pub fn load_saved(&mut self) {
        if self.saved_task.is_some() {
            self.saved_again = true;
            return;
        }
        let hub = self.hub.clone();
        self.saved_task = Some(rt::spawn(async move { hub.saved_server_mods().await }));
    }

    /// Read the saved servers' mods again when the server list was replaced, the
    /// favourites moved or a profile write landed (`written`), so the unused count
    /// follows them while the view stays open. Every frame: only compares numbers.
    pub fn follow_saved(&mut self, list: &dz_hub::features::browser::ServerSnapshot, written: u64) {
        let key = (list.generation(), list.profile_generation(), written);
        if key == self.saved_key {
            return;
        }
        self.saved_key = key;
        // Before the disk was read the load reads them when it ends.
        if self.loaded {
            self.load_saved();
        }
    }

    /// The mods that require `id`.
    pub fn required_by(&self, id: u64) -> &[u64] {
        self.required_by.get(&id).map_or(&[], Vec::as_slice)
    }

    /// A mod's name: as the running operation named it, else from its files, else the
    /// Workshop, else its id.
    pub fn name_of(&self, id: u64) -> String {
        if let Some(n) = self.op.names.get(&id) {
            return n.clone();
        }
        if let Some(m) = self.by_id(id) {
            return m.name.clone();
        }
        match self.workshop(id) {
            Some(w) if !w.title.is_empty() => w.title.clone(),
            _ => format!("Workshop {id}"),
        }
    }

    /// Items Steam is downloading or has queued.
    pub fn steam_active(&self) -> impl Iterator<Item = &SteamSubscriptionDto> {
        self.steam
            .iter()
            .flat_map(|s| s.items.iter())
            .filter(|i| i.downloading || i.pending)
    }

    /// Steam has downloads under way: its view is worth asking for again.
    pub fn steam_busy(&self) -> bool {
        self.steam_active().next().is_some()
    }

    /// Mods with a newer version on the Workshop.
    pub fn stale(&self) -> impl Iterator<Item = &InstalledModDto> {
        self.installed.iter().filter(|m| m.update_available)
    }

    pub fn stale_count(&self) -> usize {
        self.stale_count
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// True while an operation is working (not merely showing a finished summary).
    pub fn busy(&self) -> bool {
        self.op.active && self.op.phase != ModOpPhase::Finished
    }

    /// Where a mod stands in the running operation.
    pub fn op_state(&self, id: u64) -> Option<OpState> {
        if !self.busy() || !self.op.ids.contains(&id) {
            return None;
        }
        let through = self.op.completed.iter().any(|c| c.id == id);
        if through {
            None
        } else if self.op.current_id == id {
            Some(OpState::Downloading)
        } else {
            Some(OpState::Queued)
        }
    }

    // ── reading ─────────────────────────────────────────────────────────────

    /// Read what is on disk. Callers at the same moment share one read.
    pub fn load(&mut self) {
        if self.load_task.is_some() {
            // The read under way may predate what changed: read again after it.
            self.load_again = true;
            return;
        }
        self.loading = true;
        let hub = self.hub.clone();
        self.load_task = Some(rt::spawn(async move { hub.get_installed_mods().await }));
    }

    /// Ask the Workshop which mods are behind. The hub answers with or without a Steam
    /// API key.
    pub fn check_updates(&mut self, force: bool) {
        if self.check_task.is_some() {
            // The check under way may have read the disk before what changed
            // (an update that just ended): its answer would undo it.
            self.check_again |= force;
            return;
        }
        if !force
            && self
                .last_checked
                .is_some_and(|t| t.elapsed().as_millis() < UPDATES_TTL_MS)
        {
            return;
        }
        self.checking = true;
        let hub = self.hub.clone();
        self.check_task = Some(rt::spawn(async move { hub.check_mod_updates().await }));
    }

    /// Ask Steam which items the account is subscribed to and which it is downloading.
    /// When a download of Steam's ends, the disk is read again.
    pub fn load_steam(&mut self) {
        if self.steam_task.is_some() {
            return;
        }
        self.steam_before = self.steam_active().map(|i| i.id).collect();
        let ids: Vec<u64> = self.installed.iter().map(|m| m.id).collect();
        let hub = self.hub.clone();
        self.steam_task = Some(rt::spawn(async move { hub.steam_subscriptions(ids).await }));
    }

    /// Read the folders again, ask Steam again, check for updates.
    pub fn refresh(&mut self) {
        self.load();
        self.load_steam();
        self.check_updates(true);
    }

    /// Have the hub watch the mod folders: a change sets `disk_changed` and wakes the
    /// frame, which reads them again. Asked after each read, since the folders may have
    /// moved (a library added); the hub keeps a watch on the same folders as it is.
    fn watch(&mut self) {
        if self.watch_task.is_some() {
            return;
        }
        let Some(rt) = rt::Rt::get() else {
            return;
        };
        let flag = Arc::clone(&self.disk_changed);
        let waker = rt.waker().clone();
        let on_change: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            flag.store(true, Ordering::Release);
            waker.repaint();
        });
        let hub = self.hub.clone();
        self.watch_task = Some(rt::spawn(async move { hub.watch_mods(on_change).await }));
    }

    // ── changes ─────────────────────────────────────────────────────────────

    /// Delete a mod, after asking.
    pub fn remove(&self, id: u64) {
        let Some(m) = self.by_id(id) else {
            return;
        };
        let req = ConfirmRequest::new(
            t("mods", "deleteSingleTitle"),
            tf(
                "mods",
                "deleteSingleMessage",
                &[
                    ("name", &m.name),
                    ("id", &m.id),
                    ("size", &crate::i18n::format::bytes(m.size as f64)),
                ],
            ),
        )
        .danger();
        let name = m.name.clone();
        self.bus.confirm_then(req, move |cx| {
            let hub = cx.hub.clone();
            let task = rt::spawn(async move {
                let kept = hub
                    .delete_mod(id)
                    .await
                    .map_err(|e| format!("{}: {e}", t("shell", "errorFailed")))?;
                Ok(MutateDone::Removed {
                    removed: vec![id],
                    kept: if kept { vec![id] } else { Vec::new() },
                    says: vec![if kept {
                        tf("mods", "keptInSteam", &[("name", &name)])
                    } else {
                        tf("mods", "deletedSingle", &[("name", &name)])
                    }],
                })
            });
            cx.stores.mods.mutate.push(task);
        });
    }

    /// Delete several mods, after asking.
    pub fn remove_many(&self, ids: Vec<u64>) {
        if ids.is_empty() {
            return;
        }
        let n = ids.len();
        let size: u64 = self
            .installed
            .iter()
            .filter(|m| ids.contains(&m.id))
            .map(|m| m.size)
            .sum();
        let size = crate::i18n::format::bytes(size as f64);
        let msg = plural(
            i64::try_from(n).unwrap_or(i64::MAX),
            "deleteSelectedMessage",
            "deleteSelectedMessagePlural",
        );
        let req = ConfirmRequest::new(
            t("mods", "deleteSelectedTitle"),
            tf("mods", msg, &[("count", &n), ("size", &size)]),
        )
        .confirm_label(tf("mods", "deleteButton", &[("count", &n)]))
        .danger();
        self.remove_asking(ids, req);
    }

    /// Delete `ids` once `req` is answered yes, by the same path as [`Mods::remove_many`]
    /// (one Steam session for the unsubscriptions, the toasts, the list updated).
    pub fn remove_asking(&self, ids: Vec<u64>, req: ConfirmRequest) {
        if ids.is_empty() {
            return;
        }
        let n = ids.len();
        let many = n > 1;
        self.bus.confirm_then(req, move |cx| {
            let hub = cx.hub.clone();
            let task = rt::spawn(async move {
                let removed = ids.clone();
                let kept = hub
                    .delete_mods_bulk(ids)
                    .await
                    .map_err(|e| format!("{}: {e}", t("mods", "deleteFailed")))?;
                let word = if many {
                    "deletedSelectedPlural"
                } else {
                    "deletedSelected"
                };
                let mut says = vec![tf("mods", word, &[("count", &n)])];
                if !kept.is_empty() {
                    says.push(tf("mods", "keptInSteamMany", &[("count", &kept.len())]));
                }
                Ok(MutateDone::Removed {
                    removed,
                    kept,
                    says,
                })
            });
            cx.stores.mods.mutate.push(task);
        });
    }

    /// Link or unlink a mod for the game: the row flips at once (optimistic), the
    /// hub's answer settles it, a failure re-reads the disk.
    pub fn toggle_managed(&mut self, id: u64) {
        let Some(m) = self.by_id.get(&id).and_then(|i| self.installed.get_mut(*i)) else {
            return;
        };
        m.managed = !m.managed;
        let name = m.name.clone();
        self.revision += 1;
        let hub = self.hub.clone();
        self.mutate.push(rt::spawn(async move {
            Ok(match hub.toggle_mod_managed(id).await {
                Ok(managed) => MutateDone::Managed(id, managed, name),
                Err(e) => MutateDone::ManagedFailed(tf("mods", "toggleFailed", &[("error", &e)])),
            })
        }));
    }

    /// When the Workshop was last asked which mods are behind.
    pub fn last_checked(&self) -> Option<Instant> {
        self.last_checked
    }

    /// Delete the launcher's downloads and game links, after asking.
    pub fn cleanup(&self) {
        let req = ConfirmRequest::new(t("mods", "cleanupTitle"), t("mods", "cleanupMessage"));
        self.bus.confirm_then(req, move |cx| {
            let hub = cx.hub.clone();
            let task = rt::spawn(async move {
                let r = hub
                    .cleanup_mods()
                    .await
                    .map_err(|e| format!("{}: {e}", t("mods", "cleanupFailed")))?;
                Ok(MutateDone::Reload(vec![r]))
            });
            cx.stores.mods.mutate.push(task);
        });
    }

    pub fn open_workshop_dir(&self) {
        let hub = self.hub.clone();
        rt::fire(async move {
            if let Ok(p) = hub.open_workshop_dir().await {
                let _ = crate::platform::deck::open(p);
            }
        });
    }

    pub fn open_mod_dir(&self, id: u64) {
        let hub = self.hub.clone();
        rt::fire(async move {
            if let Ok(p) = hub.open_mod_dir(id).await {
                let _ = crate::platform::deck::open(p);
            }
        });
    }

    // ── operations ──────────────────────────────────────────────────────────

    pub fn update_one(&mut self, id: u64, via_steam: bool) {
        let name = self.by_id(id).map(|m| m.name.clone());
        let args = OpArgs {
            mod_id: Some(id),
            mod_name: name,
            ..OpArgs::default()
        };
        self.start(ModOpType::UpdateOne, args, None, via_steam);
    }

    pub fn update_all(&mut self, via_steam: bool) {
        self.start(ModOpType::UpdateAll, OpArgs::default(), None, via_steam);
    }

    pub fn update_stale(&mut self, via_steam: bool) {
        self.start(ModOpType::UpdateStale, OpArgs::default(), None, via_steam);
    }

    pub fn update_many(&mut self, ids: Vec<u64>, via_steam: bool) {
        if ids.is_empty() {
            return;
        }
        let args = OpArgs {
            mod_ids: Some(ids),
            ..OpArgs::default()
        };
        self.start(ModOpType::UpdateSelected, args, None, via_steam);
    }

    /// Download again and check every file: slow, for a mod that does not load.
    pub fn repair(&mut self, id: u64, via_steam: bool) {
        let name = self.name_of(id);
        let args = OpArgs {
            mod_ids: Some(vec![id]),
            mod_names: Some(vec![name]),
            ..OpArgs::default()
        };
        self.start(ModOpType::Repair, args, None, via_steam);
    }

    /// Log SteamCMD in once, so downloads use its cached login.
    pub fn login(&mut self) {
        self.start(ModOpType::Login, OpArgs::default(), None, false);
    }

    pub fn install(&mut self, ids: Vec<u64>, via_steam: bool) {
        if ids.is_empty() {
            return;
        }
        let names = ids.iter().map(u64::to_string).collect();
        let args = OpArgs {
            mod_ids: Some(ids),
            mod_names: Some(names),
            ..OpArgs::default()
        };
        self.start(ModOpType::InstallManual, args, None, via_steam);
    }

    /// Run an operation; `on_success` goes on when every mod came through. One at a
    /// time: a running one is brought back to the front instead.
    pub fn start(
        &mut self,
        kind: ModOpType,
        args: OpArgs,
        on_success: Option<Launch>,
        via_steam: bool,
    ) {
        if self.busy() {
            self.op.minimised = false;
            return;
        }
        let ids = match (&args.mod_ids, args.mod_id, kind) {
            (Some(ids), _, _) => ids.clone(),
            (None, Some(id), _) => vec![id],
            (None, None, ModOpType::UpdateStale) => self.stale().map(|m| m.id).collect(),
            (None, None, ModOpType::UpdateAll) => self.installed.iter().map(|m| m.id).collect(),
            _ => Vec::new(),
        };
        // Installing or updating a removed mod brings it back.
        for id in &ids {
            self.removed.remove(id);
        }
        let mut op = ModOp::idle();
        op.active = true;
        op.kind = kind;
        // A login is always SteamCMD's.
        op.via = if via_steam && kind != ModOpType::Login {
            ModOpVia::Steamworks
        } else {
            ModOpVia::Steamcmd
        };
        op.current_name = t("mods", "preparing").to_string();
        op.ids = ids;
        self.op = op;
        self.on_success = on_success;
        self.listening = true;
        self.last_was_progress = false;
        self.snap_seq = 0;
        let hub = self.hub.clone();
        self.start_task = Some(rt::spawn(async move {
            hub.start_mod_operation(
                kind,
                args.ip,
                args.port,
                args.mod_id,
                args.mod_name,
                args.mod_ids,
                args.mod_names,
            )
            .await
        }));
    }

    /// At start: follow the operation the hub kept, if any.
    pub fn resume(&mut self) {
        self.listening = true;
        self.early.clear();
        let hub = self.hub.clone();
        self.attach_task = Some(rt::spawn(async move { hub.mod_op_attach().await }));
    }

    fn apply_snapshot(&mut self, s: ModOpSnapshot) {
        let now = Instant::now();
        let age = |at_ms: i64| {
            let ago = super::now_ms().saturating_sub(at_ms).max(0);
            now.checked_sub(std::time::Duration::from_millis(
                u64::try_from(ago).unwrap_or(0),
            ))
            .unwrap_or(now)
        };
        let started = age(s.started_at);
        let mut op = ModOp::idle();
        op.active = true;
        op.kind = s.kind;
        op.via = s.via.into();
        op.phase = s.phase.into();
        op.current = s.current;
        op.total = s.total;
        op.current_name = if s.current_name.is_empty() {
            t("mods", "preparing").to_string()
        } else {
            s.current_name
        };
        op.current_id = s.current_id;
        op.ids = s.queue.iter().map(|q| q.id).collect();
        op.names = s.queue.into_iter().map(|q| (q.id, q.name)).collect();
        op.completed = s
            .completed
            .into_iter()
            .map(|c| Completed {
                id: c.id,
                name: c.name,
                ok: c.ok,
                ms: c.ms,
            })
            .collect();
        op.ok = s.ok;
        op.failed = s.failed;
        op.hint = s.hint;
        op.log = s.log.into_iter().map(|l| (l.at_ms, l.line)).collect();
        op.log_rev = 1;
        op.started_at = started;
        op.item_started_at = started + std::time::Duration::from_millis(s.item_started_ms);
        op.bytes = s.bytes;
        self.op = op;
        self.snap_seq = s.seq;
        self.last_was_progress = s.log_live;
    }

    /// What each step does to the operation (the hub folds the same steps into its
    /// snapshot).
    fn apply(&mut self, ev: &ModProgressEvent, signals: &mut Vec<Signal>) {
        let op = &mut self.op;
        match ev.kind {
            ModProgressKind::Queued => {
                op.ids = ev.queue.iter().map(|q| q.id).collect();
                op.names = ev.queue.iter().map(|q| (q.id, q.name.clone())).collect();
            }
            ModProgressKind::LoggedIn => signals.push(Signal::SteamcmdLoggedIn),
            ModProgressKind::SteamGuardMobileRequired => op.phase = ModOpPhase::SteamGuardMobile,
            ModProgressKind::PasswordRequired => op.phase = ModOpPhase::PasswordRequired,
            ModProgressKind::Starting => {
                op.phase = ModOpPhase::Downloading;
                op.current = ev.current;
                op.total = ev.total;
                op.current_name.clone_from(&ev.name);
                op.current_id = ev.mod_id;
                op.item_started_at = Instant::now();
                op.bytes = None;
            }
            ModProgressKind::Done | ModProgressKind::Failed => {
                op.current = ev.current;
                op.total = ev.total;
                let ms = u64::try_from(op.item_started_at.elapsed().as_millis()).unwrap_or(0);
                op.completed.push(Completed {
                    id: ev.mod_id,
                    name: ev.name.clone(),
                    ok: ev.kind == ModProgressKind::Done,
                    ms,
                });
                op.item_started_at = Instant::now();
            }
            ModProgressKind::LogLine => {
                if let Some(line) = &ev.log_line {
                    let at = op.elapsed_ms();
                    op.log.push_back((at, line.clone()));
                    while op.log.len() > LOG_MAX {
                        op.log.pop_front();
                        op.log_dropped += 1;
                    }
                    op.log_rev += 1;
                    self.last_was_progress = false;
                }
            }
            ModProgressKind::LogProgress => {
                if let Some(line) = &ev.log_line {
                    let at = op.elapsed_ms();
                    match op.log.back_mut() {
                        Some(last) if self.last_was_progress => *last = (at, line.clone()),
                        _ => op.log.push_back((at, line.clone())),
                    }
                    op.log_rev += 1;
                    self.last_was_progress = true;
                }
            }
            ModProgressKind::Bytes => {
                if let Some(bytes) = ev.bytes {
                    op.bytes = Some(hub_op::LiveBytesDto {
                        mod_id: ev.mod_id,
                        bytes,
                        at_ms: op.elapsed_ms(),
                    });
                }
            }
            ModProgressKind::Finished => {
                op.phase = ModOpPhase::Finished;
                op.ok = ev.ok;
                op.failed = ev.failed;
                op.hint.clone_from(&ev.hint);
                if op.kind == ModOpType::Login {
                    if ev.ok > 0 {
                        self.bus.ok(t("mods", "loginOk"));
                    } else {
                        self.bus.err(t("mods", "loginFailed"));
                    }
                } else if ev.hint.is_none() && ev.failed == 0 {
                    self.bus
                        .ok(tf("mods", "updatedSuccessfully", &[("count", &ev.ok)]));
                    if let Some(l) = self.on_success.take() {
                        signals.push(Signal::ModOpSucceeded(l));
                    }
                } else if ev.failed > 0 {
                    self.bus.warn(tf(
                        "mods",
                        "updateResults",
                        &[("ok", &ev.ok), ("failed", &ev.failed)],
                    ));
                }
                // A finished operation behind a minimised dialog goes by itself.
                if self.op.minimised {
                    self.dismiss(signals);
                }
            }
        }
    }

    /// Close the progress window; the stores re-read what is on disk.
    pub fn dismiss(&mut self, signals: &mut Vec<Signal>) {
        self.op.active = false;
        self.on_success = None;
        let hub = self.hub.clone();
        rt::fire(async move { hub.mod_op_dismiss().await });
        signals.push(Signal::ModOpDismissed);
    }

    pub fn send_input(&self, input: String) {
        let hub = self.hub.clone();
        let bus = self.bus.clone();
        rt::fire(async move {
            if let Err(e) = hub.send_steamcmd_input(input).await {
                bus.err(tf("mods", "passwordSendFailed", &[("error", &e)]));
            }
        });
    }

    pub fn cancel(&mut self, signals: &mut Vec<Signal>) {
        self.listening = false;
        let hub = self.hub.clone();
        rt::fire(async move {
            let _ = hub.cancel_mod_operation().await;
        });
        self.dismiss(signals);
    }

    // ── the frame ───────────────────────────────────────────────────────────

    pub fn on_event(&mut self, ev: &HubEvent, signals: &mut Vec<Signal>) {
        let HubEvent::ModProgress(step) = ev else {
            return;
        };
        if !self.listening {
            return;
        }
        if self.attach_task.is_some() {
            self.early.push((**step).clone());
            return;
        }
        if self.snap_seq > 0 && step.seq <= self.snap_seq {
            return;
        }
        if self.op.active {
            self.apply(step, signals);
        }
    }

    pub fn poll(&mut self, signals: &mut Vec<Signal>) -> bool {
        let mut changed = false;
        if let Some(task) = &mut self.load_task
            && let Some(r) = task.poll()
        {
            self.load_task = None;
            self.loading = false;
            self.loaded = true;
            changed = true;
            if std::mem::take(&mut self.load_again) {
                self.load();
            }
            match r {
                Ok(list) => {
                    // The rows look again only when what they show moved.
                    if self.set_installed(list) {
                        signals.push(Signal::ModsChanged);
                    }
                    self.watch();
                    signals.push(Signal::ModsLoaded);
                    // Only an extra: the list stands without it.
                    let hub = self.hub.clone();
                    self.usage_task = Some(rt::spawn(async move { hub.mods_usage().await }));
                    self.load_saved();
                }
                Err(e) => self.bus.err(tf("mods", "loadFailed", &[("error", &e)])),
            }
        }
        if let Some(task) = &mut self.usage_task
            && let Some(r) = task.poll()
        {
            self.usage_task = None;
            if let Ok(u) = r {
                self.usage = u.into_iter().map(|x| (x.id, x)).collect();
                changed = true;
            }
        }
        if let Some(task) = &mut self.saved_task
            && let Some(r) = task.poll()
        {
            self.saved_task = None;
            changed = true;
            match r {
                Ok(saved) => self.saved = Some(saved),
                Err(e) => eprintln!("[mods] saved servers not read: {e}"),
            }
            if std::mem::take(&mut self.saved_again) {
                self.load_saved();
            }
        }
        if let Some(task) = &mut self.check_task
            && let Some(r) = task.poll()
        {
            self.check_task = None;
            self.checking = false;
            changed = true;
            if std::mem::take(&mut self.check_again) {
                // This answer predates what changed: the next one replaces it.
                self.check_updates(true);
            } else {
                match r {
                    Ok(list) => {
                        if self.set_installed(list) {
                            signals.push(Signal::ModsChanged);
                        }
                        self.last_checked = Some(Instant::now());
                    }
                    Err(e) => self
                        .bus
                        .err(tf("mods", "updateCheckFailed", &[("error", &e)])),
                }
            }
        }
        if let Some(task) = &mut self.steam_task
            && let Some(r) = task.poll()
        {
            self.steam_task = None;
            changed = true;
            match r {
                Ok(answer) => {
                    self.steam_error = if answer.available {
                        None
                    } else {
                        answer.reason.clone()
                    };
                    // A failed question keeps the last good answer on screen.
                    if answer.available || !self.steam.as_ref().is_some_and(|s| s.available) {
                        self.set_steam(answer);
                    }
                    let still: Vec<u64> = self.steam_active().map(|i| i.id).collect();
                    if self.steam_before.iter().any(|id| !still.contains(id)) {
                        self.load();
                    }
                }
                Err(e) => self.steam_error = Some(e),
            }
        }
        if let Some(task) = &mut self.start_task
            && let Some(r) = task.poll()
        {
            self.start_task = None;
            if let Err(e) = r {
                self.bus
                    .err(tf("mods", "operationFailed", &[("error", &e)]));
                self.op.active = false;
                self.listening = false;
                changed = true;
            }
        }
        if let Some(task) = &mut self.attach_task
            && let Some(snap) = task.poll()
        {
            self.attach_task = None;
            changed = true;
            let early = std::mem::take(&mut self.early);
            match snap {
                Some(s) => {
                    self.apply_snapshot(s);
                    let seq = self.snap_seq;
                    for ev in early.iter().filter(|e| e.seq > seq) {
                        self.apply(ev, signals);
                    }
                }
                None => self.listening = false,
            }
        }
        let mut done = Vec::new();
        self.mutate.retain_mut(|task| match task.poll() {
            Some(r) => {
                done.push(r);
                false
            }
            None => task.is_pending(),
        });
        for r in done {
            changed = true;
            match r {
                Ok(MutateDone::Reload(says)) => {
                    self.load();
                    self.toasts(says);
                }
                Ok(MutateDone::Removed {
                    removed,
                    kept,
                    says,
                }) => {
                    // Out of the list now; the watch on the folders reads them again
                    // when Steam deletes its copies.
                    let now = Instant::now();
                    for id in removed.into_iter().filter(|id| !kept.contains(id)) {
                        self.removed.insert(id, now);
                    }
                    let list = self.installed.clone();
                    let rows_moved = self.set_installed(list);
                    if let Some(s) = self.steam.take() {
                        self.set_steam(s);
                    }
                    self.load();
                    self.load_steam();
                    if rows_moved {
                        signals.push(Signal::ModsChanged);
                    }
                    self.toasts(says);
                }
                Ok(MutateDone::Managed(id, managed, name)) => {
                    if let Some(m) = self.by_id.get(&id).and_then(|i| self.installed.get_mut(*i)) {
                        m.managed = managed;
                        self.revision += 1;
                    }
                    let word = if managed { "linked" } else { "unlinked" };
                    self.bus.ok(tf("mods", word, &[("name", &name)]));
                }
                Ok(MutateDone::ManagedFailed(e)) => {
                    self.load();
                    self.bus.err(e);
                }
                Err(e) => self.bus.err(e),
            }
        }
        if let Some(task) = &mut self.watch_task
            && let Some(r) = task.poll()
        {
            self.watch_task = None;
            if let Err(e) = r {
                // The view's slow read keeps the list current without it.
                eprintln!("[mods] folders not watched: {e}");
            }
        }
        // The folders changed (Steam, the player, an `@` link): read them once. Not
        // while an operation downloads: its end reads them anyway, and each item would
        // read them again meanwhile.
        if !self.busy() && self.disk_changed.swap(false, Ordering::AcqRel) {
            self.load();
            if self.steam.is_some() {
                // Steam's manifest is a file read; its view moves with the folders.
                self.load_steam();
            }
            changed = true;
        }
        if changed {
            self.recompute_unused();
        }
        changed
    }

    /// The first toast says it worked, the others add to it.
    fn toasts(&self, says: Vec<String>) {
        for (i, s) in says.into_iter().enumerate() {
            if i == 0 {
                self.bus.ok(s);
            } else {
                self.bus.info(s);
            }
        }
    }
}
