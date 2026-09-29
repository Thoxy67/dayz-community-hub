//! One Steamworks session: connect to the running Steam client as DayZ,
//! subscribe to the items and have Steam download them, report progress,
//! disconnect.
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

use crate::DAYZ_APP_ID;
use crate::api::{self, Api, ApiCall, CallbackMsg, Iface, Pipe};
use crate::state::{self, ItemState};

/// How often Steam is polled.
const TICK: Duration = Duration::from_millis(100);
/// How long Steam may take to answer a subscription.
const SUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long nothing may move (no byte, no state change) before giving up.
const STALL_TIMEOUT: Duration = Duration::from_secs(180);
/// How long a fresh session may take to report the user logged on.
const LOGON_WAIT: Duration = Duration::from_secs(10);

/// What a download reports while it runs. `index` is the item's place in
/// the list given to [`download`].
#[derive(Debug, Clone, PartialEq)]
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

/// One session at a time in the process.
static ONE_SESSION: Mutex<()> = Mutex::new(());
/// Set while a session is connected to Steam (Steam shows DayZ running).
static OPEN: AtomicBool = AtomicBool::new(false);

/// A session is connected to Steam right now: DayZ must not be launched.
pub fn session_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// Have the running Steam client download `ids` into the user's library,
/// subscribing the account to each. Blocks until every item is installed,
/// failed, or `cancel` is set, calling `on` with progress. Connects to Steam
/// as DayZ for that time only.
///
/// `Err` when no session could be opened (library, Steam not running, not
/// logged in): a sentence for the user.
pub fn download(
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
pub fn check() -> Result<(), String> {
    let (_one, api) = begin()?;
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    // SAFETY: as in `download`.
    if unsafe { (api.ugc)() }.is_null() {
        return Err(NO_UGC.into());
    }
    Ok(())
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
        OPEN.store(true, Ordering::SeqCst);
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
        // Steam stops showing DayZ as running; DayZ can be launched again.
        unsafe { (self.api.shutdown)() };
        OPEN.store(false, Ordering::SeqCst);
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
