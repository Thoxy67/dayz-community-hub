//! One Steamworks session: connect to the running Steam client as DayZ,
//! subscribe to the items and have Steam download them, report progress,
//! disconnect.
//!
//! These run in the worker process ([`crate::worker`]): Steam shows DayZ
//! running until the process that connected exits, whatever
//! `SteamAPI_Shutdown` says, so the launcher never connects itself.
//!
//! The API is not thread-safe: a session lives on the thread that calls
//! [`download`] from start to end. Callbacks are dispatched by hand
//! (`SteamAPI_ManualDispatch_*`, the loop `SteamAPI_RunCallbacks` would
//! run), so the results of subscriptions and downloads can be read without
//! C++ callback objects.

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::DAYZ_APP_ID;
use crate::api::{self, Api, ApiCall, CallbackMsg, Iface, Pipe};
use crate::state::{self, ItemState};

/// How often Steam is polled.
const TICK: Duration = Duration::from_millis(100);
/// How long Steam may take to answer a subscription.
const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long nothing may move (no byte, no state change) before giving up.
const STALL_TIMEOUT: Duration = Duration::from_secs(180);
/// How long Steam may take to answer the unsubscriptions.
const UNSUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a fresh session may take to report the user logged on.
const LOGON_WAIT: Duration = Duration::from_secs(10);

/// What a download reports while it runs. `index` is the item's place in
/// the list given to [`download`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    /// A line for the log.
    Log(String),
    /// Steam is working on this item now.
    Starting { index: usize, id: u64 },
    /// Bytes of this item downloaded so far, of `total` (0 while unknown).
    Progress {
        index: usize,
        id: u64,
        done: u64,
        total: u64,
    },
    /// The item is installed and current, in `path`.
    Installed {
        index: usize,
        id: u64,
        path: PathBuf,
    },
    /// The item could not be downloaded.
    Failed {
        index: usize,
        id: u64,
        error: String,
    },
}

/// Each item's outcome: its folder, or why not.
pub type ItemResult = (u64, Result<PathBuf, String>);
/// Each item's unsubscription: whether it was subscribed, or why it failed.
pub type Unsubscribed = (u64, Result<bool, String>);

/// One session at a time in the process.
static ONE_SESSION: Mutex<()> = Mutex::new(());

/// Have the running Steam client download `ids` into the user's library,
/// subscribing the account to each. Blocks until every item is installed,
/// failed, or `cancel` is set, calling `on` with progress. Connects to Steam
/// as DayZ for that time only.
///
/// `Err` when no session could be opened (library, Steam not running, not
/// logged in): a sentence for the user.
pub(crate) fn download(
    ids: &[u64],
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Event),
) -> Result<Vec<ItemResult>, String> {
    let (_one, api) = begin()?;
    // SAFETY (all calls into `api` below): the functions are called with the
    // arguments their C declarations take, from this thread only, between
    // SteamAPI_InitFlat and SteamAPI_Shutdown (Session's drop).
    let session = Session::open(api)?;
    on(Event::Log(format!(
        "Connected to Steam as DayZ (app {DAYZ_APP_ID})"
    )));
    session.wait_logged_on()?;
    let ugc = unsafe { (api.ugc)() };
    if ugc.is_null() {
        return Err(NO_UGC.into());
    }
    let results = Downloads::new(&session, ugc, ids).run(cancel, on);
    drop(session);
    on(Event::Log("Disconnected from Steam".into()));
    Ok(results)
}

/// Connect to Steam as DayZ and disconnect at once: whether downloads
/// through Steam would work now. The error is the sentence [`download`]
/// would fail with.
pub(crate) fn check() -> Result<(), String> {
    let (_one, api) = begin()?;
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    // SAFETY: as in `download`.
    if unsafe { (api.ugc)() }.is_null() {
        return Err(NO_UGC.into());
    }
    Ok(())
}

/// Unsubscribe the account from those of `ids` it is subscribed to. Steam
/// then removes their files from its library by itself, once DayZ (this
/// session) is no longer running. Each id's outcome: `Ok(true)` when it was
/// unsubscribed, `Ok(false)` when it was not subscribed, or why not.
///
/// `Err` when no session could be opened, as for [`download`].
pub(crate) fn unsubscribe(ids: &[u64]) -> Result<Vec<Unsubscribed>, String> {
    let (_one, api) = begin()?;
    // SAFETY (all calls into `api` below): as in `download`.
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    let ugc = unsafe { (api.ugc)() };
    if ugc.is_null() {
        return Err(NO_UGC.into());
    }
    let mut out: Vec<Unsubscribed> = Vec::with_capacity(ids.len());
    // Calls sent, by index into `out`.
    let mut waiting: HashMap<ApiCall, usize> = HashMap::new();
    for &id in ids {
        let st = ItemState(unsafe { (api.item_state)(ugc, id) });
        if !st.subscribed() {
            out.push((id, Ok(false)));
            continue;
        }
        let call = unsafe { (api.unsubscribe)(ugc, id) };
        if call == 0 {
            out.push((id, Err("Steam would not unsubscribe from it".into())));
        } else {
            waiting.insert(call, out.len());
            out.push((id, Err("Steam did not answer the unsubscription".into())));
        }
    }
    let deadline = Instant::now() + UNSUBSCRIBE_TIMEOUT;
    while !waiting.is_empty() && Instant::now() < deadline {
        let mut answered = Vec::new();
        session.pump(|callback, bytes| {
            if callback == state::CALL_COMPLETED
                && let Some(c) = state::call_completed(bytes)
                && c.callback == state::UNSUBSCRIBE_RESULT
            {
                let result = session
                    .call_result(c.call, c.callback, c.size)
                    .and_then(|b| state::subscribe_result(&b));
                answered.push((c.call, result));
            }
        });
        for (call, result) in answered {
            let Some(i) = waiting.remove(&call) else {
                continue;
            };
            out[i].1 = match result {
                Some((state::RESULT_OK, _)) => Ok(true),
                Some((code, _)) => Err(format!(
                    "Unsubscribing failed: {}",
                    state::result_text(code)
                )),
                None => Err("Steam did not say whether the unsubscription worked".into()),
            };
        }
        std::thread::sleep(TICK);
    }
    Ok(out)
}

/// A Workshop item the account is subscribed to, as Steam has it now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subscribed {
    pub id: u64,
    pub state: ItemState,
    /// Bytes downloaded and to download while Steam fetches it; (0, 0) else.
    pub bytes: (u64, u64),
}

/// What the Workshop says about an item: its title and the items it
/// requires ("Required items" on its page; Steam calls them children).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Details {
    pub id: u64,
    pub title: String,
    pub requires: Vec<u64>,
}

/// How long the Workshop may take to answer a details query.
const DETAILS_TIMEOUT: Duration = Duration::from_secs(20);
/// Items per details query (`kNumUGCResultsPerPage`).
const DETAILS_PAGE: usize = 50;
/// More than `sizeof(SteamUGCDetails_t)` (about 9.7 KB) under any packing.
const DETAILS_BUF: usize = 16 * 1024;
/// How deep requirements of requirements are followed.
const DETAILS_DEPTH: usize = 3;

/// Every DayZ Workshop item the account is subscribed to (locally disabled
/// ones too), with what Steam is doing with each, and the Workshop's details
/// (title, required items) of the subscribed items, of `also` (the mods on
/// disk), and of what they require, followed a few levels down, except the
/// ids in `known`. A short session: Steam shows DayZ running for a moment.
///
/// `Err` when no session could be opened, as for [`download`].
pub(crate) fn subscriptions(
    also: &[u64],
    known: &[u64],
) -> Result<(Vec<Subscribed>, Vec<Details>), String> {
    let (_one, api) = begin()?;
    // SAFETY (all calls into `api` below): as in `download`.
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    let ugc = unsafe { (api.ugc)() };
    if ugc.is_null() {
        return Err(NO_UGC.into());
    }
    let n = unsafe { (api.num_subscribed)(ugc, true) };
    let mut ids = vec![0u64; n as usize];
    // SAFETY: `ids` holds `n` entries, the most Steam writes.
    let got = unsafe { (api.subscribed_items)(ugc, ids.as_mut_ptr(), n, true) };
    ids.truncate(got.min(n) as usize);
    let subscribed: Vec<Subscribed> = ids
        .iter()
        .map(|&id| {
            let state = ItemState(unsafe { (api.item_state)(ugc, id) });
            let mut bytes = (0u64, 0u64);
            if state.downloading() || state.pending() {
                let (mut done, mut total) = (0u64, 0u64);
                if unsafe { (api.download_info)(ugc, id, &mut done, &mut total) } {
                    bytes = (done, total);
                }
            }
            Subscribed { id, state, bytes }
        })
        .collect();

    let mut details: Vec<Details> = Vec::new();
    let mut asked: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut next: Vec<u64> = ids.iter().chain(also).copied().collect();
    for _ in 0..DETAILS_DEPTH {
        let want: Vec<u64> = next
            .into_iter()
            .filter(|&id| !known.contains(&id) && asked.insert(id))
            .collect();
        if want.is_empty() {
            break;
        }
        let found = session.details(ugc, &want);
        next = found
            .iter()
            .flat_map(|d| d.requires.iter().copied())
            .collect();
        details.extend(found);
    }
    Ok((subscribed, details))
}

impl Session {
    /// The Workshop's details of `ids`, those it answered for. A query that
    /// fails or times out gives nothing for its page.
    fn details(&self, ugc: Iface, ids: &[u64]) -> Vec<Details> {
        let api = self.api;
        let mut out = Vec::new();
        for page in ids.chunks(DETAILS_PAGE) {
            let mut page = page.to_vec();
            let handle = unsafe { (api.query_details)(ugc, page.as_mut_ptr(), page.len() as u32) };
            if handle == u64::MAX {
                continue;
            }
            unsafe { (api.set_return_children)(ugc, handle, true) };
            let call = unsafe { (api.send_query)(ugc, handle) };
            let mut returned = None;
            let deadline = Instant::now() + DETAILS_TIMEOUT;
            while call != 0 && returned.is_none() && Instant::now() < deadline {
                self.pump(|callback, bytes| {
                    if callback == state::CALL_COMPLETED
                        && let Some(c) = state::call_completed(bytes)
                        && c.call == call
                        && c.callback == state::UGC_QUERY_COMPLETED
                    {
                        returned = Some(
                            self.call_result(c.call, c.callback, c.size)
                                .and_then(|b| state::query_completed(&b)),
                        );
                    }
                });
                if returned.is_none() {
                    std::thread::sleep(TICK);
                }
            }
            if let Some(Some((_, state::RESULT_OK, count))) = returned {
                let mut buf = vec![0u8; DETAILS_BUF];
                for i in 0..count {
                    buf.fill(0);
                    if !unsafe { (api.query_result)(ugc, handle, i, buf.as_mut_ptr()) } {
                        continue;
                    }
                    let Some((id, state::RESULT_OK, title)) = state::details_head(&buf) else {
                        continue;
                    };
                    let count = state::details_children(&buf).unwrap_or(0).min(1024);
                    let mut children = vec![0u64; count as usize];
                    let requires = if count > 0
                        && unsafe {
                            (api.query_children)(
                                ugc,
                                handle,
                                i,
                                children.as_mut_ptr(),
                                children.len() as u32,
                            )
                        } {
                        children.retain(|&c| c != 0);
                        children
                    } else {
                        Vec::new()
                    };
                    out.push(Details {
                        id,
                        title,
                        requires,
                    });
                }
            }
            unsafe { (api.release_query)(ugc, handle) };
        }
        out
    }
}

const NO_UGC: &str = "This Steam client does not offer the Workshop interface the launcher needs. Update Steam and try again.";

/// Load the library, take the one session, and make sure Steam runs.
fn begin() -> Result<(std::sync::MutexGuard<'static, ()>, &'static Api), String> {
    let api =
        api::get().map_err(|e| format!("The Steamworks library could not be loaded: {e}."))?;
    let one = match ONE_SESSION.try_lock() {
        Ok(guard) => guard,
        // A session that panicked still closed Steam (Session's drop).
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err("Another download through Steam is already running.".into());
        }
    };
    // SAFETY: takes nothing; callable without a session.
    if !unsafe { (api.is_steam_running)() } {
        return Err(format!(
            "Steam is not running. Start Steam, log in, and try again.{}",
            flatpak_note()
        ));
    }
    Ok((one, api))
}

/// On Linux, a Steam installed only as a Flatpak lives in a sandbox the
/// launcher cannot talk to.
fn flatpak_note() -> &'static str {
    #[cfg(target_os = "linux")]
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let flatpak = home.join(".var/app/com.valvesoftware.Steam").is_dir();
        let native = home.join(".steam/sdk64/steamclient.so").exists();
        if flatpak && !native {
            return " Steam installed as a Flatpak cannot be reached from outside its sandbox: \
                     use SteamCMD downloads with it, or install Steam from your distribution.";
        }
    }
    ""
}

/// A connection to Steam as DayZ, shut down when dropped.
struct Session {
    api: &'static Api,
    pipe: Pipe,
}

impl Session {
    fn open(api: &'static Api) -> Result<Self, String> {
        set_app_id(true);
        let mut msg = [0 as c_char; api::ERR_MSG_LEN];
        let result = unsafe { (api.init_flat)(msg.as_mut_ptr()) };
        if result != api::INIT_OK {
            set_app_id(false);
            // SAFETY: Valve writes a NUL-terminated message into the buffer,
            // which starts zeroed; the last byte is forced to NUL anyway.
            msg[api::ERR_MSG_LEN - 1] = 0;
            let detail = unsafe { CStr::from_ptr(msg.as_ptr()) }
                .to_string_lossy()
                .trim()
                .to_string();
            return Err(match result {
                api::INIT_NO_STEAM_CLIENT => format!(
                    "Could not connect to Steam. Make sure the Steam client is running and logged in.{}",
                    flatpak_note()
                ),
                api::INIT_VERSION_MISMATCH => {
                    "Steam is out of date: let it update itself, restart it, and try again.".into()
                }
                _ if detail.is_empty() => {
                    "Steam refused the connection. The account logged in to Steam must own DayZ."
                        .into()
                }
                _ => format!(
                    "Steam refused the connection ({detail}). The account logged in to Steam must own DayZ."
                ),
            });
        }
        // Callbacks are read by hand: declared after init, before any is read.
        let pipe = unsafe {
            (api.dispatch_init)();
            (api.get_pipe)()
        };
        Ok(Self { api, pipe })
    }

    /// Steam's own login is what downloads: wait a moment for it to report
    /// in, then give up.
    fn wait_logged_on(&self) -> Result<(), String> {
        let user = unsafe { (self.api.user)() };
        let deadline = Instant::now() + LOGON_WAIT;
        loop {
            if !user.is_null() && unsafe { (self.api.logged_on)(user) } {
                return Ok(());
            }
            if Instant::now() > deadline {
                return Err(
                    "Steam is running but not logged in, or is in offline mode. Log in to Steam and try again."
                        .into(),
                );
            }
            self.pump(|_, _| {});
            std::thread::sleep(TICK);
        }
    }

    /// Run a frame and hand each pending callback (id, bytes) to `f`.
    fn pump(&self, mut f: impl FnMut(i32, &[u8])) {
        let api = self.api;
        unsafe { (api.dispatch_run_frame)(self.pipe) };
        let mut msg = CallbackMsg::default();
        while unsafe { (api.dispatch_next)(self.pipe, &mut msg) } {
            let bytes: &[u8] = match usize::try_from(msg.size) {
                // SAFETY: Steam hands `size` bytes at `param`, valid until
                // FreeLastCallback below.
                Ok(n) if n > 0 && !msg.param.is_null() => unsafe {
                    std::slice::from_raw_parts(msg.param, n)
                },
                _ => &[],
            };
            f(msg.callback, bytes);
            unsafe { (api.dispatch_free_last)(self.pipe) };
        }
    }

    /// The result of a finished call, if it is a `callback`, `size` bytes.
    fn call_result(&self, call: ApiCall, callback: i32, size: u32) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; size as usize];
        let mut failed = false;
        let ok = unsafe {
            (self.api.dispatch_call_result)(
                self.pipe,
                call,
                buf.as_mut_ptr().cast(),
                i32::try_from(size).ok()?,
                callback,
                &mut failed,
            )
        };
        (ok && !failed).then_some(buf)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // Steam keeps showing DayZ running until this process exits.
        unsafe { (self.api.shutdown)() };
        set_app_id(false);
    }
}

/// Tell the library which game it is (it reads the environment during
/// init), and take it back afterwards so nothing started later inherits it.
fn set_app_id(on: bool) {
    let id = DAYZ_APP_ID.to_string();
    for key in ["SteamAppId", "SteamGameId"] {
        // SAFETY: set and removed only here, under ONE_SESSION; nothing else
        // in the app reads these variables.
        unsafe {
            if on {
                std::env::set_var(key, &id);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

/// Where an item is.
enum Phase {
    /// SubscribeItem was sent at this time; its answer has not come.
    Subscribing(ApiCall, Instant),
    /// Subscribed and DownloadItem called: Steam is on it.
    Downloading,
    Done(PathBuf),
    Failed(String),
}

struct Item {
    id: u64,
    phase: Phase,
    state: ItemState,
    bytes: (u64, u64),
}

struct Downloads<'a> {
    session: &'a Session,
    ugc: Iface,
    items: Vec<Item>,
    /// Item index by id, for callbacks.
    by_id: HashMap<u64, usize>,
    /// The item reported as the one Steam works on.
    current: Option<usize>,
    /// When anything last moved.
    last_activity: Instant,
}

impl<'a> Downloads<'a> {
    fn new(session: &'a Session, ugc: Iface, ids: &[u64]) -> Self {
        let now = Instant::now();
        Self {
            session,
            ugc,
            items: ids
                .iter()
                .map(|&id| Item {
                    id,
                    phase: Phase::Subscribing(0, now),
                    state: ItemState(u32::MAX),
                    bytes: (0, 0),
                })
                .collect(),
            by_id: ids.iter().enumerate().map(|(i, &id)| (id, i)).collect(),
            current: None,
            last_activity: now,
        }
    }

    fn run(mut self, cancel: &AtomicBool, on: &mut dyn FnMut(Event)) -> Vec<ItemResult> {
        let api = self.session.api;
        for i in 0..self.items.len() {
            let id = self.items[i].id;
            let call = unsafe { (api.subscribe)(self.ugc, id) };
            if call == 0 {
                self.fail(i, "Steam would not subscribe to it".into(), on);
            } else {
                self.items[i].phase = Phase::Subscribing(call, Instant::now());
                on(Event::Log(format!("Subscribing to {id}")));
            }
        }

        while self.items.iter().any(|it| it.unresolved()) {
            if cancel.load(Ordering::SeqCst) {
                on(Event::Log("Cancelled".into()));
                for i in 0..self.items.len() {
                    if self.items[i].unresolved() {
                        self.fail(i, "Cancelled".into(), on);
                    }
                }
                break;
            }
            self.dispatch(on);
            self.poll(on);
            if self.last_activity.elapsed() > STALL_TIMEOUT {
                for i in 0..self.items.len() {
                    if self.items[i].unresolved() {
                        self.fail(
                            i,
                            format!(
                                "Steam made no progress for {} minutes; check Steam's Downloads page",
                                STALL_TIMEOUT.as_secs() / 60
                            ),
                            on,
                        );
                    }
                }
                break;
            }
            std::thread::sleep(TICK);
        }

        self.items
            .into_iter()
            .map(|it| {
                let r = match it.phase {
                    Phase::Done(p) => Ok(p),
                    Phase::Failed(e) => Err(e),
                    _ => Err("Not finished".into()),
                };
                (it.id, r)
            })
            .collect()
    }

    /// Read what Steam sent: subscription answers and download results.
    fn dispatch(&mut self, on: &mut dyn FnMut(Event)) {
        let mut subscribed: Vec<(ApiCall, Option<(i32, u64)>)> = Vec::new();
        let mut downloaded: Vec<(u64, i32)> = Vec::new();
        let session = self.session;
        session.pump(|callback, bytes| match callback {
            state::CALL_COMPLETED => {
                if let Some(c) = state::call_completed(bytes)
                    && c.callback == state::SUBSCRIBE_RESULT
                {
                    let result = session
                        .call_result(c.call, c.callback, c.size)
                        .and_then(|b| state::subscribe_result(&b));
                    subscribed.push((c.call, result));
                }
            }
            state::DOWNLOAD_RESULT => {
                if let Some((app, id, result)) = state::download_result(bytes)
                    && app == DAYZ_APP_ID
                {
                    downloaded.push((id, result));
                }
            }
            _ => {}
        });

        for (call, result) in subscribed {
            let Some(i) = self
                .items
                .iter()
                .position(|it| matches!(it.phase, Phase::Subscribing(c, _) if c == call))
            else {
                continue;
            };
            let id = self.items[i].id;
            self.last_activity = Instant::now();
            match result {
                Some((state::RESULT_OK, _)) => {
                    on(Event::Log(format!("Subscribed {id}")));
                    if unsafe { (self.session.api.download)(self.ugc, id, true) } {
                        self.items[i].phase = Phase::Downloading;
                    } else {
                        self.fail(i, "Steam would not download it".into(), on);
                    }
                }
                Some((code, _)) => self.fail(
                    i,
                    format!("Subscribing failed: {}", state::result_text(code)),
                    on,
                ),
                None => self.fail(
                    i,
                    "Steam did not say whether the subscription worked".into(),
                    on,
                ),
            }
        }
        for (id, result) in downloaded {
            if let Some(&i) = self.by_id.get(&id)
                && result != state::RESULT_OK
                && self.items[i].unresolved()
            {
                self.fail(
                    i,
                    format!("Download failed: {}", state::result_text(result)),
                    on,
                );
            }
        }
    }

    /// Ask Steam where each item is.
    fn poll(&mut self, on: &mut dyn FnMut(Event)) {
        let api = self.session.api;
        let mut working = None;
        let mut moved = Vec::new();
        for i in 0..self.items.len() {
            let id = self.items[i].id;
            match self.items[i].phase {
                Phase::Subscribing(_, since) => {
                    if since.elapsed() > SUBSCRIBE_TIMEOUT {
                        self.fail(i, "Steam did not answer the subscription".into(), on);
                    }
                    continue;
                }
                Phase::Downloading => {}
                Phase::Done(_) | Phase::Failed(_) => continue,
            }
            let st = ItemState(unsafe { (api.item_state)(self.ugc, id) });
            if st != self.items[i].state {
                self.items[i].state = st;
                self.last_activity = Instant::now();
                on(Event::Log(format!("{id}: {st}")));
            }
            if st.ready() {
                self.install(i, on);
                continue;
            }
            if st.downloading() {
                working.get_or_insert(i);
            }
            if st.downloading() || st.pending() {
                let (mut done, mut total) = (0u64, 0u64);
                if unsafe { (api.download_info)(self.ugc, id, &mut done, &mut total) }
                    && (done, total) != self.items[i].bytes
                {
                    self.items[i].bytes = (done, total);
                    self.last_activity = Instant::now();
                    moved.push((i, id, done, total));
                }
            }
        }
        // Steam fetches one item at a time: the one downloading, or else the
        // first still waiting.
        if let Some(i) = working.or_else(|| self.items.iter().position(Item::unresolved)) {
            self.start(i, on);
        }
        for (index, id, done, total) in moved {
            if self.current == Some(index) {
                on(Event::Progress {
                    index,
                    id,
                    done,
                    total,
                });
            }
        }
    }

    /// Report `i` as the item being worked on, once.
    fn start(&mut self, i: usize, on: &mut dyn FnMut(Event)) {
        if self.current != Some(i) {
            self.current = Some(i);
            on(Event::Starting {
                index: i,
                id: self.items[i].id,
            });
        }
    }

    fn install(&mut self, i: usize, on: &mut dyn FnMut(Event)) {
        let id = self.items[i].id;
        let mut size = 0u64;
        let mut stamp = 0u32;
        let mut folder = [0 as c_char; 4096];
        let ok = unsafe {
            (self.session.api.install_info)(
                self.ugc,
                id,
                &mut size,
                folder.as_mut_ptr(),
                folder.len() as u32,
                &mut stamp,
            )
        };
        folder[folder.len() - 1] = 0;
        // SAFETY: NUL-terminated just above.
        let path = unsafe { CStr::from_ptr(folder.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        if !ok || path.is_empty() {
            self.fail(i, "Steam says it is installed but not where".into(), on);
            return;
        }
        let path = PathBuf::from(path);
        self.start(i, on);
        self.items[i].phase = Phase::Done(path.clone());
        self.last_activity = Instant::now();
        on(Event::Installed { index: i, id, path });
    }

    fn fail(&mut self, i: usize, error: String, on: &mut dyn FnMut(Event)) {
        let id = self.items[i].id;
        self.items[i].phase = Phase::Failed(error.clone());
        on(Event::Failed {
            index: i,
            id,
            error,
        });
    }
}

impl Item {
    fn unresolved(&self) -> bool {
        matches!(self.phase, Phase::Subscribing(..) | Phase::Downloading)
    }
}
