//! Pinging servers over A2S: the scan of the whole list (driven by Rust,
//! reporting only its progress), small explicit lists, and one server.
//!
//! Every result lands in the live store (`browser::live`), which the browser
//! reads; nothing here touches the app state's lock.
//!
//! A whole-list scan belongs to the backend, not to the window that asked
//! for it: its progress goes out as `ping-scan-progress` events to whatever
//! window listens, and `Hub::ping_scan_state` tells a window that has just
//! (re)loaded how far it has got. A `Channel` dies with the page that made
//! it, so a reload used to leave the scan running unseen.

use crate::{Hub, HubEvent};
use futures_util::{StreamExt, stream};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

use crate::features::browser::live::{self, Live};

/// Default timeout for an explicit list.
const LIST_TIMEOUT_MS: u64 = 5_000;
/// Default timeout of a manual ping.
const MANUAL_TIMEOUT_MS: u64 = 10_000;
/// Default concurrency for an explicit list.
const LIST_CONCURRENT: usize = 10;
/// Results per `PingResults` event for an explicit list.
const BATCH_SIZE: usize = 50;
/// How often partial batches of an explicit list are sent (at most).
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);
/// How often a whole-list scan's progress is sent (at most).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

/// One ping, as the window receives it.
#[derive(Serialize, Clone, Debug)]
pub struct PingResultDto {
    pub ip: String,
    pub port: i64,
    pub ms: u32,
    pub players: Option<u8>,
    pub max_players: Option<u8>,
    /// Bots reported by A2S_INFO (DayZ servers often pad this to fake a full server).
    pub bots: Option<u8>,
    /// True when the query failed (timeout or error).
    pub failed: bool,
}

fn to_dto(l: &Live, ip: String, port: i64) -> PingResultDto {
    PingResultDto {
        ip,
        port,
        ms: l.ms.unwrap_or(dz_a2s::PING_TIMEOUT_SENTINEL),
        players: l.players,
        max_players: l.max_players,
        bots: l.bots,
        failed: l.failed,
    }
}

/// How far the scan of the whole list has got.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanProgress {
    /// Which scan: each whole-list scan has a new one, so a window can tell
    /// a late word from a scan that was replaced. 0 before any scan.
    pub id: u32,
    pub done: u32,
    pub total: u32,
    pub paused: bool,
    pub running: bool,
}

/// How far the whole-list scan has got, sent at most twice a second while
/// it runs and once when it ends. Every window hears it, whoever started it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct PingScanProgress(pub ScanProgress);

/// What started a whole-list scan.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanKind {
    /// The whole list (`start_scan`, or the end of a refresh).
    Whole,
    /// Only servers added to the list after a scan had ended (`enqueue`).
    Added,
}

/// The scan as a window that has just loaded finds it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanState {
    /// The last whole-list scan's progress; `id` 0 when none has run in
    /// this process.
    pub progress: ScanProgress,
    /// What started it; null when none has run.
    pub kind: Option<ScanKind>,
    /// When it started, Unix ms; null when none has run.
    pub started_at: Option<i64>,
    /// Small explicit lists (`ping_servers`) being pinged beside it. Their
    /// results land in the live store, so a window re-queries for them.
    pub lists_running: u32,
}

/// One whole-list scan's counters, shared by its loop, its queue and
/// [`ping_scan_state`].
struct Run {
    id: u32,
    kind: ScanKind,
    started_at: i64,
    /// Targets sent so far: the scan is over when it has done as many.
    total: AtomicU32,
    done: AtomicU32,
    running: AtomicBool,
}

impl Run {
    fn new(id: u32, kind: ScanKind) -> Arc<Self> {
        Arc::new(Self {
            id,
            kind,
            started_at: now_ms(),
            total: AtomicU32::new(0),
            done: AtomicU32::new(0),
            running: AtomicBool::new(true),
        })
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The scan's controls, managed by the app on its own.
#[derive(Default)]
pub struct PingState {
    /// Read by every scan loop on every result: an atomic, not a lock.
    paused: AtomicBool,
    /// The running scan, so a new one (or a refresh) can abort it.
    background: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// The running whole-list scan's queue, which servers added to the list
    /// meanwhile join ([`enqueue`]).
    queue: Mutex<Option<Queue>>,
    /// The last whole-list scan, running or not, for [`ping_scan_state`].
    last: Mutex<Option<Arc<Run>>>,
    /// The last scan id handed out.
    ids: AtomicU32,
    /// Explicit lists being pinged.
    lists: Arc<AtomicU32>,
}

struct Queue {
    /// Targets, with the tries each has had.
    tx: mpsc::UnboundedSender<(String, u32)>,
    run: Arc<Run>,
}

impl PingState {
    pub(crate) fn abort_background(&self) {
        if let Some(handle) = self.background.lock().ok().and_then(|mut h| h.take()) {
            handle.abort();
        }
        if let Ok(mut q) = self.queue.lock() {
            *q = None;
        }
        // An aborted loop never reaches its own end: say it is over here.
        if let Some(run) = self.last.lock().ok().and_then(|l| l.clone()) {
            run.running.store(false, Ordering::Relaxed);
        }
    }

    /// Stop the whole-list scan and clear the pause: what `cancel_ping` and
    /// a refresh do.
    pub(crate) fn stop(&self) {
        self.abort_background();
        self.paused.store(false, Ordering::Relaxed);
    }

    /// A new whole-list scan's counters, remembered as the last one.
    fn begin(&self, kind: ScanKind) -> Arc<Run> {
        let run = Run::new(self.ids.fetch_add(1, Ordering::Relaxed) + 1, kind);
        if let Ok(mut l) = self.last.lock() {
            *l = Some(Arc::clone(&run));
        }
        run
    }

    /// The scan as it stands.
    pub(crate) fn state(&self) -> ScanState {
        let last = self.last.lock().ok().and_then(|l| l.clone());
        let paused = self.is_paused();
        let lists_running = self.lists.load(Ordering::Relaxed);
        match last {
            Some(run) => ScanState {
                progress: progress_of(&run, paused, run.running.load(Ordering::Relaxed)),
                kind: Some(run.kind),
                started_at: Some(run.started_at),
                lists_running,
            },
            None => ScanState {
                progress: ScanProgress {
                    id: 0,
                    done: 0,
                    total: 0,
                    paused,
                    running: false,
                },
                kind: None,
                started_at: None,
                lists_running,
            },
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}

/// Parse "ip:port".
pub(crate) fn parse_target(target: &str) -> Option<(&str, i64)> {
    let (ip, port) = target.rsplit_once(':')?;
    Some((ip, port.parse().ok()?))
}

/// What asking one target came to.
enum Asked {
    Done(String, i64, Live),
    /// No answer, tries left: to the back of the queue.
    Again(String, u32),
    /// Not "ip:port": counted done, nothing recorded.
    Invalid,
}

fn progress_of(run: &Run, paused: bool, running: bool) -> ScanProgress {
    ScanProgress {
        id: run.id,
        done: run.done.load(Ordering::Relaxed),
        total: run.total.load(Ordering::Relaxed),
        paused,
        running,
    }
}

/// Where a scan's results go besides the live store.
enum Sink {
    /// Batches of results (`PingResults`), for explicit lists; they are in
    /// the live store all the same.
    Batches(Hub),
    /// `PingScanProgress` events, for the whole-list scan.
    Event(Hub),
}

/// Ping `targets` with `concurrency` queries in flight, recording each result
/// in the live store as it lands. A server that does not answer is asked
/// again up to `retries` times, after the targets queued before it: one lost
/// UDP packet should not mark it dead, nor should it hold a slot meanwhile.
/// A `whole` scan takes more targets while it runs ([`enqueue`]).
async fn scan(
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
    sink: Sink,
    ping: Arc<PingState>,
    run: Arc<Run>,
) {
    #[cfg(feature = "mock")]
    if let Sink::Batches(hub) | Sink::Event(hub) = &sink
        && hub.is_mock()
    {
        return mock_scan(targets, sink, ping, run).await;
    }
    let whole = matches!(sink, Sink::Event(_));
    // Each target with the number of tries it has had: one that timed out
    // goes to the back of the queue rather than being asked again at once,
    // so the silent ones are retried in a short pass at the end instead of
    // each holding a slot for `retries + 1` timeouts.
    let (tx, rx) = mpsc::unbounded_channel::<(String, u32)>();
    for t in targets {
        if tx.send((t, 0)).is_ok() {
            run.total.fetch_add(1, Ordering::Relaxed);
        }
    }
    let again = tx.clone();
    if whole && let Ok(mut q) = ping.queue.lock() {
        *q = Some(Queue {
            tx,
            run: Arc::clone(&run),
        });
    }
    let done = &run.done;
    let total = &run.total;
    let progress = |running: bool| progress_of(&run, ping.is_paused(), running);
    let end = || {
        run.running.store(false, Ordering::Relaxed);
        if let Sink::Event(hub) = &sink {
            hub.emit(HubEvent::PingScanProgress(PingScanProgress(progress(
                false,
            ))));
        }
    };
    // A scan is over when it has done every target it was sent (retries
    // keep the queue open, so it never runs dry by itself). A whole scan's
    // queue then closes, under its lock so nothing is sent into it after.
    let finished = || {
        if done.load(Ordering::Relaxed) < total.load(Ordering::Relaxed) {
            return false;
        }
        if !whole {
            return true;
        }
        let Ok(mut q) = ping.queue.lock() else {
            return true;
        };
        if done.load(Ordering::Relaxed) < total.load(Ordering::Relaxed) {
            return false;
        }
        *q = None;
        true
    };

    // One client for the whole scan. async-a2s binds a socket per query, so
    // the sockets open at once are the queries in flight, bounded by
    // `concurrency`: an unbounded burst runs Winsock out of buffer space
    // (WSAENOBUFS, 10055, learn.microsoft.com "Windows Sockets Error Codes").
    // Its own retries are off: a timeout here is retried by the queue.
    let Ok(mut client) = dz_a2s::client_with_timeout(timeout).await else {
        end();
        return;
    };
    client.retries(0);
    let client = Arc::new(client);

    let targets = Box::pin(stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|t| (t, rx))
    }));
    let mut results = targets
        .map(|(target, tries)| {
            let client = Arc::clone(&client);
            async move {
                let Some((ip, port)) = parse_target(&target) else {
                    return Asked::Invalid;
                };
                // The challenge and its answer, both within `timeout`.
                let r = tokio::time::timeout(timeout, dz_a2s::ping_using(&client, &target))
                    .await
                    .unwrap_or_else(|_| Err(dz_common::Error::A2sQuery("timeout".into())));
                if r.is_err() && tries < retries {
                    return Asked::Again(target, tries + 1);
                }
                let l = Live::from_result(&r);
                live::store().record(ip, port, l);
                Asked::Done(ip.to_owned(), port, l)
            }
        })
        .buffer_unordered(concurrency)
        .fuse();

    let mut batch: Vec<PingResultDto> = Vec::new();
    let mut flush = tokio::time::interval(if whole {
        PROGRESS_INTERVAL
    } else {
        FLUSH_INTERVAL
    });
    flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // Progress is sent only when it moved: a paused scan, or one waiting on
    // slow servers, would otherwise repeat itself on every tick.
    let mut last_sent: Option<ScanProgress> = None;
    let mut send = |batch: &mut Vec<PingResultDto>| {
        let fresh = |p: ScanProgress, last: &mut Option<ScanProgress>| {
            let moved = *last != Some(p);
            *last = Some(p);
            moved
        };
        match &sink {
            Sink::Batches(hub) if !batch.is_empty() => {
                hub.emit(HubEvent::PingResults(std::mem::take(batch)));
            }
            Sink::Event(hub) => {
                let p = progress(true);
                if fresh(p, &mut last_sent) {
                    hub.emit(HubEvent::PingScanProgress(PingScanProgress(p)));
                }
            }
            _ => {}
        }
    };

    loop {
        if finished() {
            break;
        }
        if ping.is_paused() {
            flush.tick().await;
            send(&mut batch);
            continue;
        }
        tokio::select! {
            biased;
            next = results.next() => match next {
                None => break,
                Some(Asked::Again(target, tries)) => {
                    if again.send((target, tries)).is_err() {
                        done.fetch_add(1, Ordering::Relaxed);
                    }
                }
                Some(Asked::Invalid) => {
                    done.fetch_add(1, Ordering::Relaxed);
                }
                Some(Asked::Done(ip, port, l)) => {
                    done.fetch_add(1, Ordering::Relaxed);
                    if let Sink::Batches(_) = &sink {
                        batch.push(to_dto(&l, ip, port));
                        if batch.len() >= BATCH_SIZE {
                            send(&mut batch);
                        }
                    }
                }
            },
            _ = flush.tick() => send(&mut batch),
        }
    }
    if let Sink::Batches(hub) = &sink
        && !batch.is_empty()
    {
        hub.emit(HubEvent::PingResults(batch));
    }
    end();
}

/// A scan in the mock world: every target answers as mock.ts' `pingOf`
/// says, the whole list at mock.ts' pace (700 servers per 250 ms), an
/// explicit list in batches of 200 every 60 ms. Nothing goes on the network.
#[cfg(feature = "mock")]
async fn mock_scan(targets: Vec<String>, sink: Sink, ping: Arc<PingState>, run: Arc<Run>) {
    let whole = matches!(sink, Sink::Event(_));
    let (every, per_tick) = if whole {
        (PROGRESS_INTERVAL, 1400)
    } else {
        (Duration::from_millis(60), 200)
    };
    run.total.fetch_add(
        u32::try_from(targets.len()).unwrap_or(u32::MAX),
        Ordering::Relaxed,
    );
    let mut tick = tokio::time::interval(every);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut rest = targets.into_iter().peekable();
    while rest.peek().is_some() {
        tick.tick().await;
        if !ping.is_paused() {
            let mut batch = Vec::new();
            for target in rest.by_ref().take(per_tick) {
                run.done.fetch_add(1, Ordering::Relaxed);
                let Some((ip, port)) = parse_target(&target) else {
                    continue;
                };
                let l = crate::mock::live_of(ip, port);
                live::store().record(ip, port, l);
                batch.push(to_dto(&l, ip.to_owned(), port));
            }
            if let Sink::Batches(hub) = &sink
                && !batch.is_empty()
            {
                hub.emit(HubEvent::PingResults(batch));
            }
        }
        if let Sink::Event(hub) = &sink
            && rest.peek().is_some()
        {
            let p = progress_of(&run, ping.is_paused(), true);
            hub.emit(HubEvent::PingScanProgress(PingScanProgress(p)));
        }
    }
    run.running.store(false, Ordering::Relaxed);
    if let Sink::Event(hub) = &sink {
        let p = progress_of(&run, ping.is_paused(), false);
        hub.emit(HubEvent::PingScanProgress(PingScanProgress(p)));
    }
}

/// Start the scan of the whole list, replacing any running one. The order
/// and settings come from the profile; see `browser::scan_targets`. Returns
/// its first progress (its `id` is how the window knows its words).
pub(crate) fn start_whole_scan(
    hub: &Hub,
    ping: &Arc<PingState>,
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
) -> ScanProgress {
    ping.abort_background();
    let run = ping.begin(ScanKind::Whole);
    let first = progress_of(&run, ping.is_paused(), true);
    let state = Arc::clone(ping);
    let hub = hub.clone();
    let handle = tokio::spawn(async move {
        scan(
            targets,
            concurrency,
            timeout,
            retries,
            Sink::Event(hub),
            state,
            run,
        )
        .await;
    });
    if let Ok(mut h) = ping.background.lock() {
        *h = Some(handle);
    }
    first
}

/// Have `targets` pinged with the whole-list scan: into its queue when one
/// runs (its progress grows), else in a scan of their own, whose progress
/// the status bar hears as `ping-scan-progress`.
pub(crate) fn enqueue(
    hub: &Hub,
    ping: &Arc<PingState>,
    targets: Vec<String>,
    concurrency: usize,
    timeout: Duration,
    retries: u32,
) {
    if targets.is_empty() {
        return;
    }
    if let Ok(mut q) = ping.queue.lock()
        && let Some(queue) = q.as_ref()
    {
        let mut sent = true;
        for t in &targets {
            if queue.tx.send((t.clone(), 0)).is_err() {
                sent = false;
                break;
            }
            queue.run.total.fetch_add(1, Ordering::Relaxed);
        }
        if sent {
            return;
        }
        // The scan was stopped without closing its queue.
        *q = None;
    }
    let run = ping.begin(ScanKind::Added);
    let state = Arc::clone(ping);
    let hub = hub.clone();
    let handle = tokio::spawn(async move {
        scan(
            targets,
            concurrency,
            timeout,
            retries,
            Sink::Event(hub),
            state,
            run,
        )
        .await;
    });
    if let Ok(mut h) = ping.background.lock() {
        *h = Some(handle);
    }
}

impl Hub {
    /// Ping a small explicit list (favorites, history); the results come as
    /// `PingResults` batches and land in the live store. Runs beside the
    /// whole-list scan, not instead of it.
    pub async fn ping_servers(
        &self,
        targets: Vec<String>,
        concurrency: Option<usize>,
        timeout_ms: Option<u64>,
    ) -> Result<(), String> {
        let ping = self.ping();
        if targets.is_empty() || ping.is_paused() {
            return Ok(());
        }
        let concurrency = concurrency.unwrap_or(LIST_CONCURRENT).clamp(5, 100);
        let timeout =
            Duration::from_millis(timeout_ms.unwrap_or(LIST_TIMEOUT_MS).clamp(1000, 5000));
        let state = Arc::clone(ping);
        let lists = Arc::clone(&ping.lists);
        lists.fetch_add(1, Ordering::Relaxed);
        let hub = self.clone();
        tokio::spawn(async move {
            scan(
                targets,
                concurrency,
                timeout,
                0,
                Sink::Batches(hub),
                state,
                // Counted for itself only: an explicit list is not "the scan".
                Run::new(0, ScanKind::Whole),
            )
            .await;
            lists.fetch_sub(1, Ordering::Relaxed);
        });
        Ok(())
    }

    /// The whole-list scan as it stands: what a window shows after it
    /// loads, before the next `PingScanProgress` arrives.
    pub async fn ping_scan_state(&self) -> Result<ScanState, String> {
        Ok(self.ping().state())
    }

    /// Ping one server, with the long manual timeout. Returns the RTT in ms.
    pub async fn ping_single(
        &self,
        ip: String,
        port: i64,
        timeout_ms: Option<u64>,
    ) -> Result<u32, String> {
        mocked!(self => {
            let ms = crate::mock::ping_of(&format!("{ip}:{port}"));
            live::store().record(&ip, port, crate::mock::live_of(&ip, port));
            Ok(ms)
        });
        let addr = format!("{ip}:{port}");
        let timeout = Duration::from_millis(timeout_ms.unwrap_or(MANUAL_TIMEOUT_MS));
        let r = match tokio::time::timeout(timeout, dz_a2s::ping(&addr)).await {
            Ok(r) => r,
            Err(_) => Err(dz_common::Error::A2sQuery("Timeout".into())),
        };
        live::store().record(&ip, port, Live::from_result(&r));
        r.map(|p| p.ms).map_err(|e| e.to_string())
    }

    /// Stop the whole-list scan and clear the pause.
    pub async fn cancel_ping(&self) -> Result<(), String> {
        self.ping().stop();
        Ok(())
    }

    /// Pause or resume scanning. Returns whether it is now paused.
    pub async fn toggle_ping_pause(&self) -> Result<bool, String> {
        Ok(!self.ping().paused.fetch_xor(true, Ordering::Relaxed))
    }
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

    #[test]
    fn targets_parse() {
        assert_eq!(parse_target("1.2.3.4:27016"), Some(("1.2.3.4", 27016)));
        assert_eq!(parse_target("::1:27016"), Some(("::1", 27016)));
        assert_eq!(parse_target("nope"), None);
        assert_eq!(parse_target("1.2.3.4:x"), None);
    }

    #[test]
    fn the_scan_state_follows_the_last_scan() {
        let ping = PingState::default();
        let none = ping.state();
        assert_eq!(none.progress.id, 0);
        assert!(!none.progress.running);
        assert_eq!(none.kind, None);

        let run = ping.begin(ScanKind::Whole);
        run.total.store(10, Ordering::Relaxed);
        run.done.store(4, Ordering::Relaxed);
        let s = ping.state();
        assert_eq!(s.progress.id, 1);
        assert_eq!((s.progress.done, s.progress.total), (4, 10));
        assert!(s.progress.running);
        assert_eq!(s.kind, Some(ScanKind::Whole));
        assert!(s.started_at.is_some());

        // Paused shows; stopped keeps the counts but no longer runs.
        ping.paused.store(true, Ordering::Relaxed);
        assert!(ping.state().progress.paused);
        ping.stop();
        let s = ping.state();
        assert!(!s.progress.running && !s.progress.paused);
        assert_eq!(s.progress.done, 4);

        // A later scan has a later id.
        let next = ping.begin(ScanKind::Added);
        assert_eq!(next.id, 2);
        assert_eq!(ping.state().kind, Some(ScanKind::Added));
    }
}
