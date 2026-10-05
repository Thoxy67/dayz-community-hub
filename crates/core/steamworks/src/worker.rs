//! Every Steamworks session runs in a child process: the launcher's own
//! executable started with [`WORKER_ARG`]. Steam shows DayZ running until
//! the process that connected as DayZ exits, `SteamAPI_Shutdown` or not, so
//! a session inside the launcher kept Steam saying DayZ was running (and
//! refusing to start it) for as long as the launcher stayed open.
//!
//! The parent writes one [`Request`] as a JSON line on the child's stdin,
//! then `cancel` lines if it wants the child to stop; the child answers with
//! [`Reply`] lines on stdout, prefixed with [`MARK`] since Valve's library
//! prints on stdout too, and exits once it has sent its outcome.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::session::{self, Details, Event, ItemResult, Subscribed, Unsubscribed};

/// The first argument that makes the launcher's executable a worker.
pub const WORKER_ARG: &str = "--steamworks-worker";
/// What starts each of the worker's lines meant for the parent.
const MARK: &str = "@dzsw ";
/// What the parent writes to have the worker stop.
const CANCEL: &str = "cancel";

#[derive(Serialize, Deserialize)]
enum Request {
    /// Load the library and look for Steam, without connecting.
    Status,
    Check,
    Download {
        ids: Vec<u64>,
    },
    Unsubscribe {
        ids: Vec<u64>,
    },
    Subscriptions {
        also: Vec<u64>,
        known: Vec<u64>,
    },
}

#[derive(Serialize, Deserialize)]
enum Reply {
    Event(Event),
    Status {
        library: Result<(), String>,
        steam_running: bool,
    },
    Check(Result<(), String>),
    Download(Result<Vec<ItemResult>, String>),
    Unsubscribe(Result<Vec<Unsubscribed>, String>),
    Subscriptions(Result<(Vec<Subscribed>, Vec<Details>), String>),
}

// ── the launcher's side ─────────────────────────────────────────────────

/// One worker at a time.
static ONE: Mutex<()> = Mutex::new(());
/// The worker alive now, if any.
static CURRENT: Mutex<Option<Arc<Worker>>> = Mutex::new(None);

struct Worker {
    child: Mutex<Child>,
    stdin: Mutex<Option<ChildStdin>>,
}

impl Worker {
    fn cancel(&self) {
        if let Ok(mut stdin) = self.stdin.lock()
            && let Some(w) = stdin.as_mut()
        {
            let _ = writeln!(w, "{CANCEL}").and_then(|()| w.flush());
        }
    }

    fn exited(&self) -> bool {
        self.child
            .lock()
            .map_or(true, |mut c| !matches!(c.try_wait(), Ok(None)))
    }
}

/// A session is connected to Steam right now (Steam shows DayZ running).
pub fn session_open() -> bool {
    CURRENT.lock().is_ok_and(|c| c.is_some())
}

/// End the session open now, if any: ask it to stop, give it `grace`, then
/// kill it. Returns once its process is gone (Steam no longer sees DayZ
/// running), true when there was one. Blocking.
pub fn close_session(grace: Duration) -> bool {
    let Some(worker) = CURRENT.lock().ok().and_then(|c| c.clone()) else {
        return false;
    };
    worker.cancel();
    let deadline = Instant::now() + grace;
    while !worker.exited() {
        if Instant::now() > deadline {
            if let Ok(mut c) = worker.child.lock() {
                let _ = c.kill();
                let _ = c.wait();
            }
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

/// Run `request` in a worker, handing its events to `on`, and return its
/// outcome. Setting `cancel` asks the worker to stop.
fn run(
    request: &Request,
    cancel: Option<&AtomicBool>,
    on: &mut dyn FnMut(Event),
) -> Result<Reply, String> {
    // Only a worker that connects is one Steam sees as DayZ.
    let connects = !matches!(request, Request::Status);
    let _one = if connects {
        Some(match ONE.try_lock() {
            Ok(guard) => guard,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err("Another download through Steam is already running.".into());
            }
        })
    } else {
        None
    };
    let exe =
        std::env::current_exe().map_err(|e| format!("The Steam session could not start ({e})."))?;
    let mut cmd = Command::new(exe);
    cmd.arg(WORKER_ARG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("The Steam session could not start ({e})."))?;
    let stdout = child.stdout.take();
    let mut stdin = child.stdin.take();
    let line = serde_json::to_string(request).map_err(|e| e.to_string())?;
    let sent = stdin
        .as_mut()
        .map(|w| writeln!(w, "{line}").and_then(|()| w.flush()));
    let worker = Arc::new(Worker {
        child: Mutex::new(child),
        stdin: Mutex::new(stdin),
    });
    if connects && let Ok(mut c) = CURRENT.lock() {
        *c = Some(worker.clone());
    }

    let finished = AtomicBool::new(false);
    let outcome = std::thread::scope(|s| {
        if let Some(cancel) = cancel {
            s.spawn(|| {
                while !finished.load(Ordering::SeqCst) {
                    if cancel.load(Ordering::SeqCst) {
                        worker.cancel();
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
        }
        let outcome = match (sent, stdout) {
            (Some(Ok(())), Some(stdout)) => read_replies(stdout, on),
            _ => None,
        };
        finished.store(true, Ordering::SeqCst);
        outcome
    });

    // Its outcome sent, the worker is on its way out.
    drop(worker.stdin.lock().map(|mut s| s.take()));
    if let Ok(mut c) = worker.child.lock() {
        if outcome.is_none() {
            let _ = c.kill();
        }
        let _ = c.wait();
    }
    if connects && let Ok(mut c) = CURRENT.lock() {
        *c = None;
    }
    outcome.ok_or_else(|| "The Steam session stopped unexpectedly.".into())
}

/// Read the worker's lines until its outcome: events go to `on`, anything
/// not meant for the launcher to stderr. `None` when it ended without one.
fn read_replies(stdout: impl std::io::Read, on: &mut dyn FnMut(Event)) -> Option<Reply> {
    for line in BufReader::new(stdout).lines() {
        let line = line.ok()?;
        let Some(json) = line.strip_prefix(MARK) else {
            if !line.trim().is_empty() {
                eprintln!("[steamworks] {line}");
            }
            continue;
        };
        match serde_json::from_str::<Reply>(json) {
            Ok(Reply::Event(e)) => on(e),
            Ok(outcome) => return Some(outcome),
            Err(e) => eprintln!("[steamworks] unreadable reply ({e}): {json}"),
        }
    }
    None
}

const MIXED_UP: &str = "The Steam session answered something else than asked.";

/// Whether this mode can work here: Valve's library loads (written to disk
/// the first time), and whether the Steam client is running. Asked of a
/// worker, without connecting, so the launcher never loads the library.
pub fn status() -> (Result<(), String>, bool) {
    match run(&Request::Status, None, &mut |_| {}) {
        Ok(Reply::Status {
            library,
            steam_running,
        }) => (library, steam_running),
        Ok(_) => (Err(MIXED_UP.into()), false),
        Err(e) => (Err(e), false),
    }
}

/// Whether the Steam client is running, as Valve's library sees it.
pub fn steam_running() -> bool {
    status().1
}

/// [`session::download`], in a worker.
pub fn download(
    ids: &[u64],
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Event),
) -> Result<Vec<ItemResult>, String> {
    match run(&Request::Download { ids: ids.to_vec() }, Some(cancel), on)? {
        Reply::Download(r) => r,
        _ => Err(MIXED_UP.into()),
    }
}

/// [`session::check`], in a worker.
pub fn check() -> Result<(), String> {
    match run(&Request::Check, None, &mut |_| {})? {
        Reply::Check(r) => r,
        _ => Err(MIXED_UP.into()),
    }
}

/// [`session::unsubscribe`], in a worker.
pub fn unsubscribe(ids: &[u64]) -> Result<Vec<Unsubscribed>, String> {
    match run(
        &Request::Unsubscribe { ids: ids.to_vec() },
        None,
        &mut |_| {},
    )? {
        Reply::Unsubscribe(r) => r,
        _ => Err(MIXED_UP.into()),
    }
}

/// [`session::subscriptions`], in a worker.
pub fn subscriptions(
    also: &[u64],
    known: &[u64],
) -> Result<(Vec<Subscribed>, Vec<Details>), String> {
    let request = Request::Subscriptions {
        also: also.to_vec(),
        known: known.to_vec(),
    };
    match run(&request, None, &mut |_| {})? {
        Reply::Subscriptions(r) => r,
        _ => Err(MIXED_UP.into()),
    }
}

// ── the worker's side ───────────────────────────────────────────────────

/// When this process was started as a worker, serve its request and exit;
/// otherwise return at once. Called first thing in `main`.
pub fn serve_if_worker() {
    if std::env::args().nth(1).as_deref() != Some(WORKER_ARG) {
        return;
    }
    std::process::exit(serve());
}

fn send(reply: &Reply) {
    if let Ok(json) = serde_json::to_string(reply) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{MARK}{json}").and_then(|()| out.flush());
    }
}

fn serve() -> i32 {
    let mut lines = BufReader::new(std::io::stdin()).lines();
    let Some(Ok(first)) = lines.next() else {
        return 2;
    };
    let Ok(request) = serde_json::from_str::<Request>(&first) else {
        eprintln!("[steamworks] unreadable request: {first}");
        return 2;
    };
    // `cancel`, or the launcher gone (stdin closed): stop.
    let cancel: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
    std::thread::spawn(move || {
        for line in lines {
            match line {
                Ok(l) if l.trim() != CANCEL => continue,
                _ => break,
            }
        }
        cancel.store(true, Ordering::SeqCst);
    });
    let reply = match request {
        Request::Status => Reply::Status {
            library: crate::available(),
            steam_running: crate::local_steam_running(),
        },
        Request::Check => Reply::Check(session::check()),
        Request::Download { ids } => Reply::Download(session::download(&ids, cancel, &mut |e| {
            send(&Reply::Event(e));
        })),
        Request::Unsubscribe { ids } => Reply::Unsubscribe(session::unsubscribe(&ids)),
        Request::Subscriptions { also, known } => {
            Reply::Subscriptions(session::subscriptions(&also, &known))
        }
    };
    send(&reply);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_are_read_past_valves_own_lines() {
        let e = Reply::Event(Event::Log("Connected".into()));
        let done = Reply::Check(Err("Steam is not running.".into()));
        let text = format!(
            "Setting breakpad minidump AppID = 221100\n{MARK}{}\n\n{MARK}{}\n",
            serde_json::to_string(&e).unwrap(),
            serde_json::to_string(&done).unwrap(),
        );
        let mut events = Vec::new();
        let outcome = read_replies(text.as_bytes(), &mut |e| events.push(e));
        assert_eq!(events, vec![Event::Log("Connected".into())]);
        assert!(matches!(outcome, Some(Reply::Check(Err(e))) if e == "Steam is not running."));
    }

    #[test]
    fn a_worker_ending_without_an_outcome_reads_as_none() {
        assert!(read_replies(&b"garbage\n"[..], &mut |_| {}).is_none());
    }
}
