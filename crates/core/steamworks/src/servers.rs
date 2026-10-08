//! The addresses of DayZ's servers, from Steam's master server through the
//! running client (`ISteamMatchmakingServers`): the list DayZ's own browser
//! starts from. Needs no Web API key, only Steam running and logged in.
//!
//! The master answers in about a second; what is slow is the client then
//! querying every server itself, about ten a second. So only the addresses
//! are taken, as soon as the master gives them, and the request is
//! cancelled: the caller queries the servers (A2S) much faster.
//!
//! Steam truncates one request at 10 000 servers and DayZ lists far more
//! (most of them farms: fake servers by the thousand on one address), so a
//! listing is a sequence of *partitions*, filter sets whose answers the
//! caller merges: the servers Steam knows have players first, then the
//! empty ones map by map. (`collapse_addr_hash`, "one server per address",
//! was tried for the maps farms fill: the master answers it with 3 servers.)
//!
//! `RequestInternetServerList` reports through a C++ object implementing
//! `ISteamMatchmakingServerListResponse`: one is laid out by hand (a vtable
//! pointer, then our own fields), and its callbacks only set flags.
//!
//! Lifetime rule, from A Beautiful Potato Launcher's experience: releasing a
//! request still refreshing crashes Steam's client library, so a request is
//! cancelled, then released, and only then are its filters and response
//! object freed (Steam reads them for the life of the request).

use std::ffi::{c_char, c_void};
use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::DAYZ_APP_ID;
use crate::api::{Iface, ListRequest};
use crate::session::{Session, begin};

/// Filters of one request, as `(key, value)` pairs (`MatchMakingKeyValuePair_t`):
/// `hasplayers`, `noplayers`, `map`, `collapse_addr_hash`, `gametagsand`…
pub type Filters = Vec<(String, String)>;

/// Steam's ceiling for one request: an answer this long was truncated.
pub const STEAM_LIST_CAP: usize = 10_000;

/// How often Steam is polled while a request runs.
const TICK: Duration = Duration::from_millis(100);
/// The master's answer is complete once it has not grown for this long.
const SETTLED: Duration = Duration::from_millis(800);
/// How long the master may take to answer one request.
const PARTITION_TIMEOUT: Duration = Duration::from_secs(20);
/// Pause between requests, so the master does not take them for a flood.
const PARTITION_GAP: Duration = Duration::from_millis(500);
/// Stop after this many empty answers in a row: Steam is throttling.
const MAX_EMPTY_IN_A_ROW: usize = 2;
/// How long the session stays open after the last request, for the client
/// to tear its queries down.
const SETTLE: Duration = Duration::from_millis(800);
/// Bytes of `gameserveritem_t` read: its `servernetadr_t`.
const ADDR_LEN: usize = 8;

/// One server as the master lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MasterEntry {
    pub ip: Ipv4Addr,
    pub query_port: u16,
    pub game_port: u16,
}

/// How Steam ended a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Answer {
    Listed,
    /// Nothing matched, or Steam throttles.
    NoServers,
    /// The master never answered.
    TimedOut,
    /// Stopped on request.
    Cancelled,
}

/// One partition's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub filters: Filters,
    pub answer: Answer,
    /// Servers the master listed.
    pub listed: usize,
    /// Steam stopped at its cap: there were more.
    pub capped: bool,
}

/// What a listing reports while it runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ListEvent {
    /// Servers just listed by a partition whose filters say `empty`
    /// (see [`empty_of`]).
    Found {
        empty: Option<bool>,
        servers: Vec<MasterEntry>,
    },
    /// A partition is over.
    Done(Listing),
}

/// What Steam's master counts for the players of servers these filters
/// list, which a server cannot inflate: `Some(true)` for `noplayers`,
/// `Some(false)` for `hasplayers`, else unknown.
pub fn empty_of(filters: &Filters) -> Option<bool> {
    let has = |k: &str| filters.iter().any(|(key, _)| key == k);
    if has("noplayers") {
        Some(true)
    } else if has("hasplayers") {
        Some(false)
    } else {
        None
    }
}

fn flag(key: &str) -> (String, String) {
    (key.to_owned(), "1".to_owned())
}

/// The partitions of a full listing, the most useful first: the master
/// answers a short burst, then about one request a minute, so the order
/// is what a cut-short listing still has. Servers with players, then empty
/// servers on each of `maps` (the busiest first, as the caller orders them).
pub fn partitions(maps: &[String]) -> Vec<Filters> {
    let mut parts = vec![vec![flag("hasplayers")]];
    parts.extend(
        maps.iter()
            .map(|m| vec![flag("noplayers"), ("map".to_owned(), m.clone())]),
    );
    parts
}

/// List DayZ's servers, one request per partition, handing the addresses to `on` as the master gives them
/// and each partition's outcome when it ends. Returns how many requests
/// were answered. Connects to Steam as DayZ for that time (seconds).
///
/// `Err` when no session could be opened: a sentence for the user.
pub(crate) fn list(
    partitions: &[Filters],
    cancel: &AtomicBool,
    on: &mut dyn FnMut(ListEvent),
) -> Result<usize, String> {
    let (_one, api) = begin()?;
    // SAFETY (all calls into `api` below): as in session::download.
    let session = Session::open(api)?;
    session.wait_logged_on()?;
    let mms = unsafe { (api.servers)() };
    if mms.is_null() {
        return Err("This Steam client does not offer the server list interface. Update Steam and try again.".into());
    }
    let (mut answered, mut empty_in_a_row) = (0, 0);
    for (i, filters) in partitions.iter().enumerate() {
        if i > 0 && !pause(PARTITION_GAP, cancel) {
            break;
        }
        let listing = request(&session, mms, filters.clone(), cancel, on);
        match listing.answer {
            Answer::Cancelled => break,
            Answer::Listed => {
                answered += 1;
                empty_in_a_row = 0;
            }
            Answer::NoServers | Answer::TimedOut => empty_in_a_row += 1,
        }
        on(ListEvent::Done(listing));
        if empty_in_a_row >= MAX_EMPTY_IN_A_ROW {
            break;
        }
    }
    // A released request's HTTP query to the master is torn down by the
    // client's own thread a moment later; shutting down before that makes
    // it assert ("IClientHTTP::ReleaseHTTPRequest failed") and send Valve a
    // crash dump. Let it finish.
    let settle = Instant::now() + SETTLE;
    while Instant::now() < settle {
        session.pump(|_, _| {});
        std::thread::sleep(TICK);
    }
    Ok(answered)
}

/// Sleep `d` unless cancelled; false when cancelled.
fn pause(d: Duration, cancel: &AtomicBool) -> bool {
    let end = Instant::now() + d;
    while Instant::now() < end {
        if cancel.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::sleep(TICK);
    }
    !cancel.load(Ordering::SeqCst)
}

/// `MatchMakingKeyValuePair_t`.
#[repr(C)]
struct KeyValue {
    key: [c_char; 256],
    value: [c_char; 256],
}

impl KeyValue {
    fn new(key: &str, value: &str) -> Self {
        let mut kv = Self {
            key: [0; 256],
            value: [0; 256],
        };
        for (dst, src) in [(&mut kv.key, key), (&mut kv.value, value)] {
            // The last byte stays NUL.
            for (d, s) in dst.iter_mut().zip(src.bytes().take(255)) {
                *d = s as c_char;
            }
        }
        kv
    }
}

/// `ISteamMatchmakingServerListResponse`: a vtable pointer first, as a C++
/// object with virtual methods (and no virtual destructor) is laid out,
/// then the state its methods set.
#[repr(C)]
struct Response {
    vtable: *const VTable,
    done: AtomicBool,
    answer: AtomicI32,
}

/// Its three pure virtual methods, in declaration order. On x86_64, Linux
/// and Windows alike, a method takes `this` as its first argument in the C
/// convention.
#[repr(C)]
struct VTable {
    responded: unsafe extern "C" fn(*mut Response, ListRequest, i32),
    failed: unsafe extern "C" fn(*mut Response, ListRequest, i32),
    complete: unsafe extern "C" fn(*mut Response, ListRequest, i32),
}

static VTABLE: VTable = VTable {
    responded: on_server,
    failed: on_server,
    complete: on_complete,
};

/// `ServerResponded` / `ServerFailedToRespond`: the client's own queries,
/// which are cancelled.
unsafe extern "C" fn on_server(_: *mut Response, _: ListRequest, _: i32) {}

/// `RefreshComplete`.
unsafe extern "C" fn on_complete(this: *mut Response, _: ListRequest, answer: i32) {
    // SAFETY: `this` is the Response handed to the request, alive until
    // after the request is released.
    if let Some(r) = unsafe { this.as_ref() } {
        r.answer.store(answer, Ordering::SeqCst);
        r.done.store(true, Ordering::SeqCst);
    }
}

/// `EMatchMakingServerResponse::eNoServersListedOnMasterServer`.
const NO_SERVERS_LISTED: i32 = 2;

/// Send one request, hand on the addresses as the master lists them, stop
/// once its answer has settled, and release it.
fn request(
    session: &Session,
    mms: Iface,
    filters: Filters,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(ListEvent),
) -> Listing {
    let api = session.api;
    let mut pairs: Vec<KeyValue> = filters.iter().map(|(k, v)| KeyValue::new(k, v)).collect();
    // Steam reads `pairs` through the first pointer, as one array (the SDK's
    // own usage, steamworks-rs too); a pointer per pair as well makes the
    // layout right if it indexed the pointers instead.
    let mut pointers: Vec<*mut c_void> = pairs
        .iter_mut()
        .map(|p| std::ptr::from_mut(p).cast())
        .collect();
    let mut response = Box::new(Response {
        vtable: &VTABLE,
        done: AtomicBool::new(false),
        answer: AtomicI32::new(0),
    });
    // SAFETY: `pointers`, `pairs` and `response` outlive the request: it is
    // cancelled and released below before they drop.
    let req = unsafe {
        (api.request_internet)(
            mms,
            DAYZ_APP_ID,
            pointers.as_mut_ptr(),
            pointers.len() as u32,
            std::ptr::from_mut(response.as_mut()).cast(),
        )
    };
    let empty = empty_of(&filters);
    let mut listing = Listing {
        filters,
        answer: Answer::TimedOut,
        listed: 0,
        capped: false,
    };
    if req.is_null() {
        return listing;
    }

    let started = Instant::now();
    let mut grew = Instant::now();
    loop {
        session.pump(|_, _| {});
        let count = usize::try_from(unsafe { (api.server_count)(mms, req) }).unwrap_or(0);
        if count > listing.listed {
            let servers: Vec<MasterEntry> = (listing.listed..count)
                .filter_map(|i| {
                    let item = unsafe { (api.server_details)(mms, req, i as i32) };
                    // SAFETY: a gameserveritem_t, longer than ADDR_LEN, valid
                    // until the next call into the interface; read at once.
                    (!item.is_null())
                        .then(|| parse_addr(unsafe { std::slice::from_raw_parts(item, ADDR_LEN) }))
                        .flatten()
                })
                .collect();
            listing.listed = count;
            grew = Instant::now();
            if !servers.is_empty() {
                on(ListEvent::Found { empty, servers });
            }
        }
        if cancel.load(Ordering::SeqCst) {
            listing.answer = Answer::Cancelled;
            break;
        }
        if response.done.load(Ordering::SeqCst) {
            listing.answer = match response.answer.load(Ordering::SeqCst) {
                NO_SERVERS_LISTED if listing.listed == 0 => Answer::NoServers,
                _ => Answer::Listed,
            };
            break;
        }
        if listing.listed > 0 && grew.elapsed() >= SETTLED {
            listing.answer = Answer::Listed;
            break;
        }
        if started.elapsed() > PARTITION_TIMEOUT {
            break;
        }
        std::thread::sleep(TICK);
    }
    listing.capped = listing.listed >= STEAM_LIST_CAP;
    unsafe {
        if (api.is_refreshing)(mms, req) {
            (api.cancel_query)(mms, req);
        }
        (api.release_request)(mms, req);
    }
    drop(pointers);
    drop(pairs);
    drop(response);
    listing
}

/// A `servernetadr_t`: connection port, query port, IP in host order.
fn parse_addr(b: &[u8]) -> Option<MasterEntry> {
    let u16_at = |at: usize| Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?));
    let ip = Ipv4Addr::from(u32::from_le_bytes(b.get(4..8)?.try_into().ok()?));
    let entry = MasterEntry {
        ip,
        query_port: u16_at(2)?,
        game_port: u16_at(0)?,
    };
    (entry.query_port != 0 && !ip.is_unspecified()).then_some(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_reads_as_steam_lays_it_out() {
        // Captured from the client: 2302, 27017, 203.109.246.79.
        let b = [254, 8, 137, 105, 79, 246, 109, 203];
        assert_eq!(
            parse_addr(&b),
            Some(MasterEntry {
                ip: Ipv4Addr::new(203, 109, 246, 79),
                query_port: 27017,
                game_port: 2302,
            })
        );
        assert!(parse_addr(&[0; 8]).is_none());
        assert!(parse_addr(&[1; 4]).is_none());
    }

    #[test]
    fn a_full_listing_asks_for_populated_servers_first() {
        let p = partitions(&["enoch".into()]);
        assert_eq!(p[0], vec![flag("hasplayers")]);
        assert_eq!(
            p[1],
            vec![flag("noplayers"), ("map".into(), "enoch".into())]
        );
        assert_eq!(empty_of(&p[0]), Some(false));
        assert_eq!(empty_of(&p[1]), Some(true));
    }

    #[test]
    fn filters_are_nul_terminated_and_truncated() {
        let kv = KeyValue::new("map", &"x".repeat(300));
        assert_eq!(
            kv.key[..4],
            [b'm' as c_char, b'a' as c_char, b'p' as c_char, 0]
        );
        assert_eq!(kv.value[255], 0);
        assert_eq!(kv.value[254], b'x' as c_char);
    }
}
