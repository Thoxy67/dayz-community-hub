//! Installed Workshop mods, their update state, and the one download operation that may
//! run at a time (install, update, repair, login), with its live log (=
//! `stores/mods.svelte.ts`). SteamCMD or the running Steam client downloads, as the
//! profile says: both report the same steps, as `HubEvent::ModProgress`.
//!
//! The hub keeps the operation too: a window started while one runs, or before its
//! summary was closed, gets it back with its queue, results and log ([`Mods::resume`]).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use dz_hub::events::ModProgressEvent;
use dz_hub::features::mods::{InstalledModDto, ModUsageDto};
use dz_hub::features::steamcmd::{ModOpType, ModProgressKind};
use dz_hub::features::steamworks::{SteamSubscriptionDto, SteamSubscriptionsDto, WorkshopItemDto};
use dz_hub::{Hub, HubEvent};
use rustc_hash::FxHashMap;
use serde::Deserialize;

use super::connect::Launch;
use super::{Bus, ConfirmRequest, Signal, mirror};
use crate::i18n::{plural, t, tf};
use crate::rt::{self, Task};

const UPDATES_TTL_MS: u128 = 5 * 60 * 1000;
/// After a removal, how often the disk is read again while Steam's copy of a removed
/// mod is still there, and for how long.
const RECHECK: Duration = Duration::from_secs(3);
const RECHECK_FOR: Duration = Duration::from_secs(120);
/// A removed mod Steam calls subscribed again after this long was subscribed again
/// (an earlier answer can predate the removal).
const RESUBSCRIBED_AFTER: Duration = Duration::from_secs(30);
/// Lines of SteamCMD output kept for the dialog; past this the oldest go.
pub const LOG_MAX: usize = 5000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModOpPhase {
    #[default]
    Downloading,
    SteamGuardMobile,
    PasswordRequired,
    Finished,
}

/// What downloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModOpVia {
    #[default]
    Steamcmd,
    Steamworks,
}

/// A mod that came through or failed, and how long it took.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
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
    pub started_at: Instant,
    /// When the mod being worked on started.
    pub item_started_at: Instant,
    /// The dialog is closed while the operation carries on; the status bar shows it.
    pub minimised: bool,
}

impl ModOp {
    fn idle() -> Self {
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
            started_at: now,
            item_started_at: now,
            minimised: false,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// The hub's `ModOpSnapshot`, read through serde (the hub does not export the type).
#[derive(Deserialize, Default)]
#[serde(default)]
struct Snapshot {
    seq: u32,
    kind: Option<ModOpType>,
    via: ModOpVia,
    phase: ModOpPhase,
    current: usize,
    total: usize,
    current_id: u64,
    current_name: String,
    queue: Vec<Queued>,
    completed: Vec<Completed>,
    ok: usize,
    failed: usize,
    hint: Option<String>,
    log: Vec<LogLine>,
    log_live: bool,
    started_at: i64,
    item_started_ms: u64,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Queued {
    id: u64,
    name: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct LogLine {
    at_ms: u64,
    line: String,
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
    by_id: FxHashMap<u64, usize>,
    steam_by_id: FxHashMap<u64, usize>,
    workshop: FxHashMap<u64, usize>,
    required_by: FxHashMap<u64, Vec<u64>>,
    stale_count: usize,
    total_size: u64,

    load_task: Option<Task<Result<Vec<InstalledModDto>, String>>>,
    usage_task: Option<Task<Result<Vec<ModUsageDto>, String>>>,
    check_task: Option<Task<Result<Vec<InstalledModDto>, String>>>,
    steam_task: Option<Task<Result<SteamSubscriptionsDto, String>>>,
    steam_before: Vec<u64>,
    start_task: Option<Task<Result<(), String>>>,
    attach_task: Option<Task<Option<Snapshot>>>,
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
    /// When the disk and Steam are next read for the removed mods still on disk.
    recheck_at: Option<Instant>,
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
            loading: false,
            loaded: false,
            checking: false,
            last_checked: None,
            op: ModOp::idle(),
            steam: None,
            steam_error: None,
            usage: FxHashMap::default(),
            by_id: FxHashMap::default(),
            steam_by_id: FxHashMap::default(),
            workshop: FxHashMap::default(),
            required_by: FxHashMap::default(),
            stale_count: 0,
            total_size: 0,
            load_task: None,
            usage_task: None,
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
            recheck_at: None,
        }
    }

    // ── derived ─────────────────────────────────────────────────────────────

    fn set_installed(&mut self, mut list: Vec<InstalledModDto>) {
        // A removed mod whose folder is gone is gone for good; one still on disk is
        // Steam's copy waiting to be deleted: kept out of the list, read again soon.
        let on_disk: Vec<u64> = list.iter().map(|m| m.id).collect();
        let before = self.removed.len();
        self.removed.retain(|id, _| on_disk.contains(id));
        if self.removed.len() < before {
            // Steam deleted a copy: its view of the subscriptions has moved too.
            self.load_steam();
        }
        list.retain(|m| !self.removed.contains_key(&m.id));
        self.recheck_at = self
            .removed
            .values()
            .any(|at| at.elapsed() < RECHECK_FOR)
            .then(|| Instant::now() + RECHECK);
        self.installed = list;
        self.by_id = self
            .installed
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id, i))
            .collect();
        self.stale_count = self.installed.iter().filter(|m| m.update_available).count();
        self.total_size = self.installed.iter().map(|m| m.size).sum();
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

    /// Ask the Workshop which mods are behind. Needs a Steam API key (`has_key`).
    pub fn check_updates(&mut self, force: bool, has_key: bool) {
        if !has_key {
            return;
        }
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
    pub fn refresh(&mut self, has_key: bool) {
        self.load();
        self.load_steam();
        self.check_updates(true, has_key);
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
                &[("name", &m.name), ("id", &m.id), ("size", &m.size_human)],
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
        let many = n > 1;
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
                let _ = open::that_detached(p);
            }
        });
    }

    pub fn open_mod_dir(&self, id: u64) {
        let hub = self.hub.clone();
        rt::fire(async move {
            if let Ok(p) = hub.open_mod_dir(id).await {
                let _ = open::that_detached(p);
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
        self.attach_task = Some(rt::spawn(async move {
            let snap = hub.mod_op_attach().await?;
            mirror::<_, Snapshot>(&snap)
        }));
    }

    fn apply_snapshot(&mut self, s: Snapshot) {
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
        op.kind = s.kind.unwrap_or(ModOpType::UpdateAll);
        op.via = s.via;
        op.phase = s.phase;
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
        op.completed = s.completed;
        op.ok = s.ok;
        op.failed = s.failed;
        op.hint = s.hint;
        op.log = s.log.into_iter().map(|l| (l.at_ms, l.line)).collect();
        op.log_rev = 1;
        op.started_at = started;
        op.item_started_at = started + std::time::Duration::from_millis(s.item_started_ms);
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
                    self.set_installed(list);
                    signals.push(Signal::ModsLoaded);
                    // Only an extra: the list stands without it.
                    let hub = self.hub.clone();
                    self.usage_task = Some(rt::spawn(async move { hub.mods_usage().await }));
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
        if let Some(task) = &mut self.check_task
            && let Some(r) = task.poll()
        {
            self.check_task = None;
            self.checking = false;
            changed = true;
            if std::mem::take(&mut self.check_again) {
                // This answer predates what changed: the next one replaces it.
                self.check_updates(true, true);
            } else {
                match r {
                    Ok(list) => {
                        self.set_installed(list);
                        self.last_checked = Some(Instant::now());
                        signals.push(Signal::ModsChanged);
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
                    // Out of the list now; the disk and Steam are read again until
                    // Steam has deleted its copies.
                    let now = Instant::now();
                    for id in removed.into_iter().filter(|id| !kept.contains(id)) {
                        self.removed.insert(id, now);
                    }
                    let list = std::mem::take(&mut self.installed);
                    self.set_installed(list);
                    if let Some(s) = self.steam.take() {
                        self.set_steam(s);
                    }
                    self.load();
                    self.load_steam();
                    signals.push(Signal::ModsChanged);
                    self.toasts(says);
                }
                Ok(MutateDone::Managed(id, managed, name)) => {
                    if let Some(m) = self.by_id.get(&id).and_then(|i| self.installed.get_mut(*i)) {
                        m.managed = managed;
                    }
                    let word = if managed { "linked" } else { "unlinked" };
                    self.bus.ok(tf("mods", word, &[("name", &name)]));
                    signals.push(Signal::ModsChanged);
                }
                Ok(MutateDone::ManagedFailed(e)) => {
                    self.load();
                    self.bus.err(e);
                }
                Err(e) => self.bus.err(e),
            }
        }
        // The removed mods Steam has not deleted yet: read again in a moment.
        if let Some(at) = self.recheck_at {
            let now = Instant::now();
            if now >= at {
                // The disk only: asking Steam starts a session (a worker process),
                // which is asked once more when the copy is gone.
                self.recheck_at = None;
                self.load();
            } else if let Some(r) = rt::Rt::get() {
                r.ctx().request_repaint_after(at - now);
            }
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
