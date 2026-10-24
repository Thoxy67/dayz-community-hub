//! The offline view (= `features/offline/View.svelte` and `offline.svelte.ts`): DayZ
//! Community Offline Mode. Which missions are installed (one card per map), installing or
//! updating it (the hub downloads in the background and says when it is done with
//! `OfflineModeUpdated` / `OfflineModeError`), playing a mission, and removing or wiping
//! one, or all of them.
//!
//! The view's state lives in one [`OfflineState`] in egui's memory, so the commands of
//! `commands.rs` (buttons, palette, pad, control channel) and the status bar (its
//! "installing" segment, [`installing`]) read and change the same thing. Hub answers come
//! back through the bus (`bus.run`), so they land whether the view is on screen or not;
//! a newer list request supersedes an older one (generation counter).
//!
//! Clearing a save (one or all) is optimistic: the card loses its save line at once and
//! gets it back if the hub fails.
//!
//! The mods a mission loads (`-mod=`, as joining a server does) are chosen in the aside's
//! "Mods" panel: a checklist of the installed mods, kept in the profile by the hub
//! (`offline_mods`), so the choice holds between runs. A toggle is optimistic too.
//!
//! Files: `draw` (the header, the status strip, the cards and the aside).

mod draw;

use std::collections::HashMap;
use std::sync::Arc;

use dz_hub::HubEvent;
use dz_hub::features::offline::MissionSaveDto;
use egui::Id;
use egui::mutex::Mutex;
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::{plural, t, tf};
use crate::rt;
use crate::store::ConfirmRequest;
use crate::views::{View, ViewId};

/// The project's page ("Project page" in the aside).
pub const REPO_URL: &str = "https://github.com/Arkensor/DayZCommunityOfflineMode";

/// Known maps by the name their mission folder carries: key, name, terrain area.
const MAPS: &[(&str, &str, &str)] = &[
    ("chernarusplus", "Chernarus+", "225 km²"),
    ("enoch", "Livonia", "163 km²"),
    ("sakhal", "Sakhal", "236 km²"),
    ("namalsk", "Namalsk", "~100 km²"),
    ("deerisle", "Deer Isle", "~150 km²"),
    ("takistanplus", "Takistan+", "~150 km²"),
    ("esseker", "Esseker", "~130 km²"),
    ("banov", "Banov", "~150 km²"),
    ("pripyat", "Pripyat", "~100 km²"),
];

/// What kind of mission a folder says it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Offline,
    Coop,
    Pvp,
    Survival,
    Mission,
}

impl Kind {
    pub fn label(self) -> &'static str {
        t(
            "offline",
            match self {
                Kind::Offline => "kindOffline",
                Kind::Coop => "kindCoop",
                Kind::Pvp => "kindPvp",
                Kind::Survival => "kindSurvival",
                Kind::Mission => "kindMission",
            },
        )
    }
}

/// One installed mission, as its card shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mission {
    /// The folder name, which is what the hub wants back.
    pub id: String,
    /// The map's own key (`chernarusplus`), lower case.
    pub map_key: String,
    pub map: String,
    pub terrain: Option<&'static str>,
    pub kind: Kind,
}

fn is_mode(p: &str) -> bool {
    let p = p.to_ascii_lowercase();
    ["offline", "coop", "pvp", "surviv", "mission"]
        .iter()
        .any(|m| p.contains(m))
}

/// `DayZCommunityOfflineMode.ChernarusPlus` → Chernarus+, offline. Folders are
/// `<mode>.<map>` or `<map>.<mode>`; whichever half is not the mode is the map.
pub fn describe(id: &str) -> Mission {
    let map_part = id
        .split('.')
        .find(|p| !is_mode(p))
        .or_else(|| id.split('.').next())
        .unwrap_or(id);
    let mode = id
        .split('.')
        .find(|p| is_mode(p))
        .unwrap_or("")
        .to_ascii_lowercase();
    let key = map_part.to_lowercase();
    let known = MAPS.iter().find(|(k, _, _)| *k == key);
    let kind = if mode.contains("coop") {
        Kind::Coop
    } else if mode.contains("pvp") {
        Kind::Pvp
    } else if mode.contains("surv") {
        Kind::Survival
    } else if mode.contains("offline") {
        Kind::Offline
    } else {
        Kind::Mission
    };
    Mission {
        id: id.to_owned(),
        map: known.map_or_else(|| prettify(map_part), |(_, n, _)| (*n).to_owned()),
        terrain: known.map(|(_, _, a)| *a),
        map_key: key,
        kind,
    }
}

/// `deer_isle` → Deer Isle, `TakistanPlus` → Takistan +: camel case split, `_`/`-` runs
/// as spaces, the first word "plus" as `+`, every word capitalised.
fn prettify(s: &str) -> String {
    // camel case: a space between a lower-case and an upper-case ASCII letter
    let mut a = String::with_capacity(s.len() + 4);
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if prev.is_some_and(|p| p.is_ascii_lowercase()) && c.is_ascii_uppercase() {
            a.push(' ');
        }
        a.push(c);
        prev = Some(c);
    }
    // runs of `_` / `-` → one space
    let mut b = String::with_capacity(a.len());
    let mut in_run = false;
    for c in a.chars() {
        if c == '_' || c == '-' {
            if !in_run {
                b.push(' ');
            }
            in_run = true;
        } else {
            b.push(c);
            in_run = false;
        }
    }
    // words (ASCII alphanumerics, as `\w` without `_`, gone by now) and what is between
    let word = |c: char| c.is_ascii_alphanumeric();
    let mut tokens: Vec<(bool, String)> = Vec::new();
    for c in b.chars() {
        match tokens.last_mut() {
            Some((w, t)) if *w == word(c) => t.push(c),
            _ => tokens.push((word(c), c.to_string())),
        }
    }
    if let Some(tok) = tokens
        .iter_mut()
        .find(|(w, t)| *w && t.eq_ignore_ascii_case("plus"))
    {
        tok.1 = "+".to_owned();
    }
    let mut out = String::with_capacity(b.len());
    for (w, tok) in tokens {
        if w {
            let mut cs = tok.chars();
            if let Some(f) = cs.next() {
                out.push(f.to_ascii_uppercase());
                out.extend(cs);
            }
        } else {
            out.push_str(&tok);
        }
    }
    out
}

/// The status strip's tone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Ok,
    Warn,
    Err,
}

/// The view's state (= the `offline` store).
#[derive(Default)]
pub struct OfflineState {
    /// Sorted by map name.
    pub missions: Vec<Mission>,
    /// Each mission's save, by folder name: absent when it has none.
    pub saves: HashMap<String, MissionSaveDto>,
    pub loading: bool,
    /// An install or update is running in the background.
    pub installing: bool,
    /// The list was read once (or failed once).
    pub loaded: bool,
    /// Why the last read of the missions failed: the empty list then says so and offers
    /// to read again, rather than invite an install.
    pub load_error: Option<String>,
    pub status: String,
    pub tone: Tone,
    /// The card under the pointer: what `offline.launch` without a mission plays (pad X).
    pub focused: Option<String>,
    load_gen: u64,
    saves_gen: u64,
    /// The installed mods chosen to load with a mission, in the order chosen.
    pub mods: Vec<u64>,
    /// The choice was read from the profile.
    pub mods_loaded: bool,
    /// The checklist's search.
    pub mods_query: String,
    mods_gen: u64,
    /// The checklist's rows (installed mod ids: the chosen first, in their order,
    /// then the others by name), made again only when `mods_rows_key` moves.
    pub mods_rows: Vec<u64>,
    /// What `mods_rows` was made from: the mods' revision, the search, the choice.
    pub mods_rows_key: Option<(u64, String, Vec<u64>)>,
    /// A save of the choice is on its way to the hub.
    mods_saving: bool,
    /// The choice made while a save was on its way: sent when it lands, so the
    /// profile keeps the last one made.
    mods_pending: Option<Vec<u64>>,
}

impl OfflineState {
    pub fn busy(&self) -> bool {
        self.loading || self.installing
    }

    pub fn installed(&self) -> bool {
        !self.missions.is_empty()
    }

    /// How many distinct maps.
    pub fn map_count(&self) -> usize {
        let mut keys: Vec<&str> = self.missions.iter().map(|m| m.map_key.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        keys.len()
    }

    fn say(&mut self, text: impl Into<String>, tone: Tone) {
        self.status = text.into();
        self.tone = tone;
    }

    fn mission(&self, id: &str) -> Option<&Mission> {
        self.missions.iter().find(|m| m.id == id)
    }

    fn set_missions(&mut self, ids: Vec<String>) {
        let mut list: Vec<Mission> = ids.iter().take(10_000).map(|id| describe(id)).collect();
        list.sort_by_cached_key(|m| m.map.to_lowercase());
        self.missions = list;
    }
}

type Shared = Arc<Mutex<OfflineState>>;

/// A hub call answering a folder, for [`open_folder`].
type PathFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<std::path::PathBuf, String>> + Send>>;

/// The view's state, shared with its commands and the status bar.
pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::unique("dzch.offline.view"), || {
            Arc::new(Mutex::new(OfflineState::default()))
        })
        .clone()
    })
}

fn with<R>(ctx: &egui::Context, f: impl FnOnce(&mut OfflineState) -> R) -> R {
    let s = shared(ctx);
    let mut st = s.lock();
    f(&mut st)
}

/// Whether the offline mode is being installed (the status bar's segment).
pub fn installing(ctx: &egui::Context) -> bool {
    with(ctx, |st| st.installing)
}

// ── actions ─────────────────────────────────────────────────────────────────

/// What the status strip says once the list is read.
enum After {
    /// "{n} missions available" / "No missions found…".
    Count,
    /// Leave it.
    Keep,
    /// This.
    Say(String, Tone),
}

/// Read the missions (then their saves); a newer read supersedes this one.
fn load(cx: &Ctx, after: After) {
    let wanted = with(cx.egui, |st| {
        st.loading = true;
        st.load_gen = st.load_gen.wrapping_add(1);
        st.load_gen
    });
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        let r = hub.get_offline_missions().await;
        bus.run(move |cx| {
            let fresh = with(cx.egui, |st| {
                if st.load_gen != wanted {
                    return false;
                }
                st.loading = false;
                st.loaded = true;
                let ok = r.is_ok();
                match r {
                    Ok(ids) => {
                        st.load_error = None;
                        st.set_missions(ids);
                        if let After::Count = after {
                            let n = st.missions.len();
                            if n == 0 {
                                st.say(t("offline", "statusNoMissions"), Tone::Warn);
                            } else {
                                let key = plural(n as i64, "statusAvailableOne", "statusAvailable");
                                st.say(tf("offline", key, &[("count", &n)]), Tone::Ok);
                            }
                        }
                    }
                    Err(e) => {
                        st.missions.clear();
                        st.load_error = Some(e.clone());
                        st.say(e, Tone::Err);
                    }
                }
                if let After::Say(text, tone) = after {
                    st.say(text, tone);
                }
                ok
            });
            if fresh {
                load_saves(cx);
            }
        });
    });
}

/// Read the saves: only an extra, the cards stand without it.
fn load_saves(cx: &Ctx) {
    let wanted = with(cx.egui, |st| {
        st.saves_gen = st.saves_gen.wrapping_add(1);
        st.saves_gen
    });
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        let r = hub.offline_saves().await;
        bus.run(move |cx| {
            with(cx.egui, |st| {
                if st.saves_gen == wanted {
                    st.saves = r
                        .unwrap_or_default()
                        .into_iter()
                        .map(|s| (s.mission.clone(), s))
                        .collect();
                }
            });
        });
    });
}

/// Download or update the offline mode in the background.
fn install(cx: &Ctx) {
    let go = with(cx.egui, |st| {
        if st.busy() {
            return false;
        }
        st.installing = true;
        st.say(t("offline", "statusDownloading"), Tone::Neutral);
        true
    });
    if !go {
        return;
    }
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        if let Err(e) = hub.update_offline_mode().await {
            bus.run(move |cx| {
                with(cx.egui, |st| {
                    st.installing = false;
                    st.say(e, Tone::Err);
                });
            });
        }
    });
}

fn launch(cx: &Ctx, id: String) {
    let bus = cx.stores.bus.clone();
    bus.info(tf(
        "offline",
        "statusLaunching",
        &[("mission", &describe(&id).map)],
    ));
    let hub = cx.hub.clone();
    rt::fire(async move {
        match hub.launch_offline_mission(id).await {
            Ok(()) => bus.run(|cx| cx.stores.game.launched()),
            Err(e) => bus.err(tf("offline", "statusLaunchFailed", &[("error", &e)])),
        }
    });
}

/// Read the mods chosen for offline missions from the profile.
fn load_mods(cx: &Ctx) {
    let wanted = with(cx.egui, |st| {
        st.mods_gen = st.mods_gen.wrapping_add(1);
        st.mods_gen
    });
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        let r = hub.offline_mods().await;
        bus.run(move |cx| match r {
            Ok(ids) => with(cx.egui, |st| {
                if st.mods_gen == wanted {
                    st.mods = ids;
                    st.mods_loaded = true;
                }
            }),
            Err(e) => cx.stores.bus.err(e),
        });
    });
}

/// Choose `ids` (shown at once), then have the hub keep them; a failure reads the
/// profile's choice back. One save at a time: a choice made while one is on its
/// way waits for it, and only the latest waiting one is sent.
fn save_mods(cx: &mut Ctx, ids: Vec<u64>) {
    let send = with(cx.egui, |st| {
        st.mods = ids.clone();
        st.mods_gen = st.mods_gen.wrapping_add(1);
        if st.mods_saving {
            st.mods_pending = Some(ids.clone());
            false
        } else {
            st.mods_saving = true;
            true
        }
    });
    if send {
        send_mods(cx, ids);
    }
}

/// Send one save of the choice; when it lands, the one that waited (if any) goes.
fn send_mods(cx: &Ctx, ids: Vec<u64>) {
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        let r = hub.set_offline_mods(ids).await;
        bus.run(move |cx| {
            let next = with(cx.egui, |st| {
                let next = st.mods_pending.take();
                st.mods_saving = next.is_some();
                next
            });
            match r {
                Ok(()) => {
                    if let Some(ids) = next {
                        send_mods(cx, ids);
                    } else {
                        // The unused-mods filter keeps what offline missions load.
                        cx.stores.mods.load_saved();
                    }
                }
                Err(e) => {
                    // What waited built on the choice that failed: read it back.
                    with(cx.egui, |st| st.mods_saving = false);
                    cx.stores
                        .bus
                        .err(tf("offline", "modsSaveFailed", &[("error", &e)]));
                    load_mods(cx);
                }
            }
        });
    });
}

/// `ids` with `id` added (at the end) or taken out.
fn toggled(ids: &[u64], id: u64, on: bool) -> Vec<u64> {
    if on && ids.contains(&id) {
        return ids.to_vec();
    }
    let mut out: Vec<u64> = ids.iter().copied().filter(|x| *x != id).collect();
    if on {
        out.push(id);
    }
    out
}

/// Open a folder the hub names (created first where it must be); errors ignored.
fn open_folder(cx: &Ctx, f: impl FnOnce(dz_hub::Hub) -> PathFuture + Send + 'static) {
    let hub = cx.hub.clone();
    rt::fire(async move {
        if let Ok(p) = f(hub).await {
            let _ = crate::platform::deck::open(p);
        }
    });
}

fn remove_mission(cx: &Ctx, id: String) {
    let req = ConfirmRequest::new(
        t("offline", "dialogRemoveMissionTitle"),
        tf("offline", "dialogRemoveMissionMessage", &[("mission", &id)]),
    )
    .confirm_label(t("offline", "dialogRemoveMissionConfirm"))
    .danger();
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        if !bus.confirm(req).await {
            return;
        }
        let shown = id.clone();
        bus.run(move |cx| {
            with(cx.egui, |st| {
                st.loading = true;
                st.say(
                    tf("offline", "statusRemovingMission", &[("mission", &shown)]),
                    Tone::Neutral,
                );
            });
        });
        let r = hub.remove_mission(id.clone()).await;
        bus.run(move |cx| match r {
            Ok(()) => {
                let text = tf("offline", "statusRemovedMission", &[("mission", &id)]);
                load(cx, After::Say(text, Tone::Ok));
            }
            Err(e) => with(cx.egui, |st| {
                st.loading = false;
                st.say(
                    tf("offline", "statusRemoveFailed", &[("error", &e)]),
                    Tone::Err,
                );
            }),
        });
    });
}

fn remove_all(cx: &Ctx) {
    let req = ConfirmRequest::new(
        t("offline", "dialogRemoveTitle"),
        t("offline", "dialogRemoveMessage"),
    )
    .confirm_label(t("offline", "dialogRemoveConfirm"))
    .danger();
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        if !bus.confirm(req).await {
            return;
        }
        bus.run(|cx| {
            with(cx.egui, |st| {
                st.loading = true;
                st.say(t("offline", "statusRemovingAll"), Tone::Neutral);
            });
        });
        let r = hub.remove_offline_mode().await;
        bus.run(move |cx| match r {
            Ok(n) => {
                let text = if n > 0 {
                    let key = plural(n as i64, "statusRemovedFoldersOne", "statusRemovedFolders");
                    tf("offline", key, &[("count", &n)])
                } else {
                    t("offline", "statusNothingToRemove").to_owned()
                };
                load(cx, After::Say(text, Tone::Ok));
            }
            Err(e) => with(cx.egui, |st| {
                st.loading = false;
                st.say(
                    tf("offline", "statusRemoveFailed", &[("error", &e)]),
                    Tone::Err,
                );
            }),
        });
    });
}

/// Clear one map's save: gone from its card at once, back if the hub fails.
fn clear_save(cx: &Ctx, id: String) {
    let map = describe(&id).map;
    let req = ConfirmRequest::new(
        tf("offline", "dialogClearOneTitle", &[("map", &map)]),
        tf("offline", "dialogClearOneMessage", &[("map", &map)]),
    )
    .confirm_label(t("offline", "clearSave"))
    .danger();
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        if !bus.confirm(req).await {
            return;
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        let key = id.clone();
        bus.run(move |cx| {
            let _ = tx.send(with(cx.egui, |st| st.saves.remove(&key)));
        });
        let before = rx.await.ok().flatten();
        let r = hub.clear_mission_save(id.clone()).await;
        bus.run(move |cx| {
            with(cx.egui, |st| match r {
                Ok(_) => st.say(
                    tf("offline", "statusClearedOne", &[("map", &map)]),
                    Tone::Ok,
                ),
                Err(e) => {
                    if let Some(s) = before {
                        st.saves.entry(id).or_insert(s);
                    }
                    st.say(
                        tf("offline", "statusClearFailed", &[("error", &e)]),
                        Tone::Err,
                    );
                }
            });
        });
    });
}

/// Clear every save: the cards lose their save lines at once, back if the hub fails.
fn clear_saves(cx: &Ctx) {
    let req = ConfirmRequest::new(
        t("offline", "dialogClearTitle"),
        t("offline", "dialogClearMessage"),
    )
    .confirm_label(t("offline", "dialogClearConfirm"))
    .danger();
    let (hub, bus) = (cx.hub.clone(), cx.stores.bus.clone());
    rt::fire(async move {
        if !bus.confirm(req).await {
            return;
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        bus.run(move |cx| {
            let _ = tx.send(with(cx.egui, |st| std::mem::take(&mut st.saves)));
        });
        let before = rx.await.unwrap_or_default();
        let r = hub.clear_offline_saves().await;
        bus.run(move |cx| {
            with(cx.egui, |st| match r {
                Ok(n) => {
                    let text = if n > 0 {
                        let key = plural(n as i64, "statusClearedSavesOne", "statusClearedSaves");
                        tf("offline", key, &[("count", &n)])
                    } else {
                        t("offline", "statusNoSaves").to_owned()
                    };
                    st.say(text, Tone::Ok);
                }
                Err(e) => {
                    for (k, v) in before {
                        st.saves.entry(k).or_insert(v);
                    }
                    st.say(
                        tf("offline", "statusClearFailed", &[("error", &e)]),
                        Tone::Err,
                    );
                }
            });
        });
    });
}

// ── the view ────────────────────────────────────────────────────────────────

pub struct OfflineView;

impl OfflineView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OfflineView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for OfflineView {
    fn on_show(&mut self, cx: &mut Ctx) {
        // Once: revisits keep the list unless Refresh is pressed.
        let (first, mods) = with(cx.egui, |st| (!st.loaded && !st.loading, !st.mods_loaded));
        if first {
            load(cx, After::Count);
        }
        if mods {
            load_mods(cx);
        }
        // The checklist lists the installed mods.
        let m = &mut cx.stores.mods;
        if !m.loaded && !m.loading {
            m.load();
        }
    }

    fn on_event(&mut self, ev: &HubEvent, cx: &mut Ctx) {
        match ev {
            HubEvent::OfflineModeUpdated => {
                let text = t("shell", "statusOfflineUpdated");
                with(cx.egui, |st| {
                    st.installing = false;
                    st.say(text, Tone::Ok);
                });
                cx.stores.bus.ok(text);
                load(cx, After::Keep);
            }
            HubEvent::OfflineModeError(e) => with(cx.egui, |st| {
                st.installing = false;
                st.say(
                    tf("shell", "statusUpdateFailed", &[("error", e)]),
                    Tone::Err,
                );
            }),
            _ => {}
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run: Vec<(&'static str, Value)> = Vec::new();
        {
            let mut st = shared.lock();
            draw::show(ui, &mut st, &cx.stores.mods, &mut run);
        }
        for (id, args) in run {
            if let Err(e) = cx.run(id, &args) {
                cx.stores.bus.err(e);
            }
        }
    }
}

// ── commands ────────────────────────────────────────────────────────────────

/// The mission a command names (`{"mission": "<folder>"}`), else the card under the
/// pointer; only one the list has.
fn arg_mission(cx: &Ctx, a: &Value) -> Option<String> {
    with(cx.egui, |st| {
        let id = a
            .get("mission")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| st.focused.clone())?;
        st.mission(&id).map(|m| m.id.clone())
    })
}

pub fn can_act(cx: &Ctx) -> bool {
    with(cx.egui, |st| !st.busy())
}

pub fn can_maintain(cx: &Ctx) -> bool {
    with(cx.egui, |st| !st.busy() && st.installed())
}

/// `offline.refresh`: read the missions again.
pub fn cmd_refresh(cx: &mut Ctx, _: &Value) {
    load(cx, After::Count);
}

/// `offline.open_missions_dir`: DayZ's Missions folder in the file manager.
pub fn cmd_open_missions_dir(cx: &mut Ctx, _: &Value) {
    open_folder(cx, |hub| {
        Box::pin(async move { hub.open_missions_dir().await })
    });
}

/// `offline.install`: download or update the offline mode (its view shows the progress).
pub fn cmd_install(cx: &mut Ctx, _: &Value) {
    if cx.nav.view != ViewId::Offline {
        cx.go(ViewId::Offline);
    }
    install(cx);
}

/// `offline.launch {"mission": "<folder>"}` (without one: the card under the pointer).
pub fn cmd_launch(cx: &mut Ctx, a: &Value) {
    if let Some(id) = arg_mission(cx, a) {
        launch(cx, id);
    }
}

/// `offline.open_dir {"mission": "<folder>"}`.
pub fn cmd_open_dir(cx: &mut Ctx, a: &Value) {
    if let Some(id) = arg_mission(cx, a) {
        open_folder(cx, move |hub| {
            Box::pin(async move { hub.open_mission_dir(id).await })
        });
    }
}

/// `offline.clear_save {"mission": "<folder>"}`: asks first.
pub fn cmd_clear_save(cx: &mut Ctx, a: &Value) {
    if let Some(id) = arg_mission(cx, a) {
        let has = with(cx.egui, |st| st.saves.contains_key(&id));
        if has {
            clear_save(cx, id);
        }
    }
}

/// `offline.remove_mission {"mission": "<folder>"}`: asks first.
pub fn cmd_remove_mission(cx: &mut Ctx, a: &Value) {
    if let Some(id) = arg_mission(cx, a) {
        remove_mission(cx, id);
    }
}

/// `offline.toggle_mod {"id", "on"?}`: load an installed mod with offline missions, or
/// not (toggles without `on`); kept in the profile.
pub fn cmd_toggle_mod(cx: &mut Ctx, a: &Value) {
    let Some(id) = a
        .get("id")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.trim().parse().ok()))
        .filter(|id| *id != 0)
    else {
        return;
    };
    let ids = with(cx.egui, |st| st.mods.clone());
    let on = a
        .get("on")
        .and_then(Value::as_bool)
        .unwrap_or(!ids.contains(&id));
    // Only an installed mod can be chosen; one already chosen can always go.
    if on && cx.stores.mods.by_id(id).is_none() {
        return;
    }
    save_mods(cx, toggled(&ids, id, on));
}

/// `offline.clear_mods`: offline missions load no mod.
pub fn cmd_clear_mods(cx: &mut Ctx, _: &Value) {
    save_mods(cx, Vec::new());
}

/// `offline.open_project`: the project's GitHub page in the browser.
pub fn cmd_open_project(cx: &mut Ctx, _: &Value) {
    crate::views::news::open_url(&cx.stores.bus, REPO_URL);
}

/// `offline.clear_saves`: every mission's save; asks first.
pub fn cmd_clear_saves(cx: &mut Ctx, _: &Value) {
    clear_saves(cx);
}

/// `offline.remove_all`: every mission folder; asks first.
pub fn cmd_remove_all(cx: &mut Ctx, _: &Value) {
    remove_all(cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_mission_folders() {
        let m = describe("DayZCommunityOfflineMode.ChernarusPlus");
        assert_eq!(m.map, "Chernarus+");
        assert_eq!(m.map_key, "chernarusplus");
        assert_eq!(m.terrain, Some("225 km²"));
        assert_eq!(m.kind, Kind::Offline);
        let m = describe("Enoch.Coop");
        assert_eq!((m.map.as_str(), m.kind), ("Livonia", Kind::Coop));
        let m = describe("pvp.deer_isle");
        assert_eq!((m.map.as_str(), m.kind), ("Deer Isle", Kind::Pvp));
        let m = describe("SomeMap");
        assert_eq!(
            (m.map.as_str(), m.kind, m.terrain),
            ("Some Map", Kind::Mission, None)
        );
        assert_eq!(describe("Survival.TakistanPlusX").kind, Kind::Survival);
        assert_eq!(prettify("takistan_plus"), "Takistan +");
        assert_eq!(prettify("big-old--map"), "Big Old Map");
        assert_eq!(describe("").map, "");
        assert_eq!(describe("é.Offline").map, "é");
    }

    #[test]
    fn a_toggle_adds_at_the_end_or_takes_out() {
        assert_eq!(toggled(&[1, 2], 3, true), vec![1, 2, 3]);
        assert_eq!(toggled(&[1, 2, 3], 2, false), vec![1, 3]);
        assert_eq!(
            toggled(&[1, 2], 1, true),
            vec![1, 2],
            "already chosen: kept in place"
        );
        assert!(toggled(&[], 5, false).is_empty());
    }

    #[test]
    fn counts_maps_and_sorts() {
        let mut st = OfflineState::default();
        st.set_missions(vec![
            "DayZCommunityOfflineMode.Sakhal".into(),
            "DayZCommunityOfflineMode.ChernarusPlus".into(),
            "Coop.ChernarusPlus".into(),
        ]);
        assert_eq!(st.missions[0].map, "Chernarus+");
        assert_eq!(st.missions[2].map, "Sakhal");
        assert_eq!(st.map_count(), 2);
        assert!(st.installed() && !st.busy());
    }
}
