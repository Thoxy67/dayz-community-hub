//! The running mod operation, kept by the backend so a window that reloads
//! finds it as it was: what it works on, what came through, the phase (a
//! password or a Steam Guard prompt included), the end of the log.
//!
//! Its steps go out as `HubEvent::ModProgress` (they went over a Tauri
//! `Channel`), and every step is folded into a [`ModOpSnapshot`] here first,
//! under the same lock, so `mod_op_attach` hands a window that starts
//! following late the operation as it stands. Steps carry a `seq` (the
//! snapshot's last) so the window can drop those the snapshot already holds,
//! whichever arrives first.

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::time::Instant;

use serde::Serialize;

use super::{ModBytes, ModOpType, ModProgressEvent, ModProgressKind, QueuedModDto};

/// Lines of output kept: the window's `LOG_MAX` (mods.svelte.ts).
pub(crate) const LOG_MAX: usize = 5000;
/// How far past [`LOG_MAX`] the log grows before it is cut back to it, so
/// a long batch is not shifted line by line.
const LOG_SLACK: usize = 500;

/// What downloads.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModOpVia {
    Steamcmd,
    Steamworks,
}

/// Where the operation stands, as the dialog shows it.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModOpPhase {
    Downloading,
    SteamGuardMobile,
    PasswordRequired,
    Finished,
}

/// A mod that came through or failed, and how long it took.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct CompletedModDto {
    pub id: u64,
    pub name: String,
    pub ok: bool,
    pub ms: u64,
}

/// A line of output, and when it came (ms after the operation started).
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct LogLineDto {
    pub at_ms: u64,
    pub line: String,
}

/// How far the mod being downloaded has got, and when that was said (ms
/// after the operation started).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveBytesDto {
    pub mod_id: u64,
    pub bytes: ModBytes,
    pub at_ms: u64,
}

/// The operation as it stands: everything the progress dialog shows.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ModOpSnapshot {
    /// Which operation: one more for each started in this process.
    pub id: u32,
    /// The last step folded in; steps arrive numbered from 1.
    pub seq: u32,
    pub kind: ModOpType,
    pub via: ModOpVia,
    pub phase: ModOpPhase,
    pub current: usize,
    pub total: usize,
    pub current_id: u64,
    pub current_name: String,
    /// Every mod it works on, named (its `queued` step).
    pub queue: Vec<QueuedModDto>,
    pub completed: Vec<CompletedModDto>,
    pub ok: usize,
    pub failed: usize,
    pub hint: Option<String>,
    /// The last lines of output, [`LOG_MAX`] at most (a little more between
    /// two cuts).
    pub log: Vec<LogLineDto>,
    /// The last line is SteamCMD's live progress line, which the next one
    /// rewrites.
    pub log_live: bool,
    /// When it started, Unix ms.
    pub started_at: i64,
    /// When the mod being worked on started, ms after `started_at`.
    pub item_started_ms: u64,
    /// The last bytes said of the mod being worked on; none until the
    /// first after it started.
    pub bytes: Option<LiveBytesDto>,
}

impl ModOpSnapshot {
    pub(crate) fn new(id: u32, kind: ModOpType, via: ModOpVia, started_at: i64) -> Self {
        Self {
            id,
            seq: 0,
            kind,
            via,
            phase: ModOpPhase::Downloading,
            current: 0,
            total: 0,
            current_id: 0,
            current_name: String::new(),
            queue: Vec::new(),
            completed: Vec::new(),
            ok: 0,
            failed: 0,
            hint: None,
            log: Vec::new(),
            log_live: false,
            started_at,
            item_started_ms: 0,
            bytes: None,
        }
    }

    pub(crate) fn finished(&self) -> bool {
        self.phase == ModOpPhase::Finished
    }

    /// Fold one step in, `at` ms after the start: what the window does with
    /// the same step (mods.svelte.ts, `#apply`).
    pub(crate) fn apply(&mut self, ev: &ModProgressEvent, at: u64) {
        use ModProgressKind as K;
        self.seq = ev.seq;
        match ev.kind {
            K::Queued => self.queue = ev.queue.clone(),
            K::LoggedIn => {}
            K::SteamGuardMobileRequired => self.phase = ModOpPhase::SteamGuardMobile,
            K::PasswordRequired => self.phase = ModOpPhase::PasswordRequired,
            K::Starting => {
                self.phase = ModOpPhase::Downloading;
                self.current = ev.current;
                self.total = ev.total;
                self.current_name = ev.name.clone();
                self.current_id = ev.mod_id;
                self.item_started_ms = at;
                self.bytes = None;
            }
            K::Done | K::Failed => {
                self.current = ev.current;
                self.total = ev.total;
                self.completed.push(CompletedModDto {
                    id: ev.mod_id,
                    name: ev.name.clone(),
                    ok: ev.kind == K::Done,
                    ms: at.saturating_sub(self.item_started_ms),
                });
                self.item_started_ms = at;
            }
            K::LogLine => {
                if let Some(line) = &ev.log_line {
                    self.push_line(line, at);
                    self.log_live = false;
                }
            }
            K::LogProgress => {
                if let Some(line) = &ev.log_line {
                    match self.log.last_mut() {
                        Some(last) if self.log_live => {
                            last.line.clone_from(line);
                            last.at_ms = at;
                        }
                        _ => self.push_line(line, at),
                    }
                    self.log_live = true;
                }
            }
            K::Bytes => {
                if let Some(bytes) = ev.bytes {
                    self.bytes = Some(LiveBytesDto {
                        mod_id: ev.mod_id,
                        bytes,
                        at_ms: at,
                    });
                }
            }
            K::Finished => {
                self.phase = ModOpPhase::Finished;
                self.ok = ev.ok;
                self.failed = ev.failed;
                self.hint = ev.hint.clone();
            }
        }
    }

    fn push_line(&mut self, line: &str, at: u64) {
        self.log.push(LogLineDto {
            at_ms: at,
            line: line.to_owned(),
        });
        if self.log.len() > LOG_MAX + LOG_SLACK {
            self.log.drain(..self.log.len() - LOG_MAX);
        }
    }
}

/// Where an operation's steps go.
pub(crate) type StepSink = Arc<dyn Fn(ModProgressEvent) + Send + Sync>;

/// The operation, and where its steps go.
struct Slot {
    op: Option<ModOpSnapshot>,
    /// The hub that started it.
    sink: Option<StepSink>,
    started: Instant,
    ids: u32,
}

static SLOT: LazyLock<Mutex<Slot>> = LazyLock::new(|| {
    Mutex::new(Slot {
        op: None,
        sink: None,
        started: Instant::now(),
        ids: 0,
    })
});

fn hub() -> MutexGuard<'static, Slot> {
    SLOT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A new operation replaces whatever was kept (a finished one not dismissed).
/// Returns its id, which its steps are sent under.
pub(crate) fn begin(kind: ModOpType, via: ModOpVia, sink: StepSink) -> u32 {
    let mut h = hub();
    h.ids += 1;
    h.started = Instant::now();
    h.op = Some(ModOpSnapshot::new(
        h.ids,
        kind,
        via,
        crate::features::ping::now_ms(),
    ));
    h.sink = Some(sink);
    h.ids
}

/// Fold a step of operation `id` in and send it to the page listening,
/// numbered.
pub(crate) fn emit(id: u32, mut ev: ModProgressEvent) {
    let mut h = hub();
    let at = h.started.elapsed().as_millis() as u64;
    let Slot { op, sink, .. } = &mut *h;
    // Cancelled, or cancelled and replaced: nobody wants its last words.
    let Some(op) = op.as_mut().filter(|op| op.id == id) else {
        return;
    };
    ev.seq = op.seq + 1;
    op.apply(&ev, at);
    if let Some(sink) = sink {
        sink(ev);
    }
}

pub(crate) fn snapshot() -> Option<ModOpSnapshot> {
    hub().op.clone()
}

/// The operation as it stands, taken under the lock the steps are folded
/// and sent under: every step after the snapshot is numbered above its `seq`.
pub(crate) fn attach() -> Option<ModOpSnapshot> {
    hub().op.clone()
}

/// Forget a finished operation (its dialog was closed). One still running
/// stays: only `cancel` stops it.
pub(crate) fn dismiss() {
    let mut h = hub();
    if h.op.as_ref().is_some_and(ModOpSnapshot::finished) {
        h.op = None;
        h.sink = None;
    }
}

/// Forget the operation, running or not: it was cancelled.
pub(crate) fn clear() {
    let mut h = hub();
    h.op = None;
    h.sink = None;
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

    fn step(kind: ModProgressKind, seq: u32) -> ModProgressEvent {
        ModProgressEvent {
            seq,
            ..ModProgressEvent::of(kind)
        }
    }

    fn about(
        kind: ModProgressKind,
        seq: u32,
        current: usize,
        id: u64,
        name: &str,
    ) -> ModProgressEvent {
        ModProgressEvent {
            seq,
            ..ModProgressEvent::about(kind, current, 2, id, name.into())
        }
    }

    fn line(kind: ModProgressKind, seq: u32, text: &str) -> ModProgressEvent {
        ModProgressEvent {
            seq,
            log_line: Some(text.into()),
            ..ModProgressEvent::of(kind)
        }
    }

    #[test]
    fn a_run_folds_into_the_snapshot_the_dialog_shows() {
        use ModProgressKind as K;
        let mut op = ModOpSnapshot::new(1, ModOpType::UpdateAll, ModOpVia::Steamcmd, 0);
        let queue = vec![
            QueuedModDto {
                id: 10,
                name: "CF".into(),
            },
            QueuedModDto {
                id: 20,
                name: "Expansion".into(),
            },
        ];
        op.apply(
            &ModProgressEvent {
                seq: 1,
                queue: queue.clone(),
                ..ModProgressEvent::of(K::Queued)
            },
            0,
        );
        op.apply(&step(K::PasswordRequired, 2), 5);
        assert_eq!(op.phase, ModOpPhase::PasswordRequired);
        op.apply(&step(K::LoggedIn, 3), 8);
        op.apply(&line(K::LogLine, 4, "Logging in...OK"), 10);
        op.apply(&about(K::Starting, 5, 1, 10, "CF"), 100);
        assert_eq!(op.phase, ModOpPhase::Downloading);
        op.apply(&line(K::LogProgress, 6, "progress: 10.00"), 150);
        op.apply(&line(K::LogProgress, 7, "progress: 90.00"), 200);
        op.apply(
            &ModProgressEvent {
                seq: 7,
                mod_id: 10,
                bytes: Some(ModBytes { done: 9, total: 10 }),
                ..ModProgressEvent::of(K::Bytes)
            },
            200,
        );
        assert_eq!(
            op.bytes,
            Some(LiveBytesDto {
                mod_id: 10,
                bytes: ModBytes { done: 9, total: 10 },
                at_ms: 200
            })
        );
        op.apply(&about(K::Done, 8, 1, 10, "CF"), 400);
        op.apply(&about(K::Starting, 9, 2, 20, "Expansion"), 400);
        // A new mod's bytes start from nothing.
        assert_eq!(op.bytes, None);
        op.apply(&line(K::LogLine, 10, "Download failed"), 450);
        op.apply(&about(K::Failed, 11, 2, 20, "Expansion (denied)"), 700);
        op.apply(
            &ModProgressEvent {
                seq: 12,
                ok: 1,
                failed: 1,
                hint: Some("private".into()),
                ..ModProgressEvent::of(K::Finished)
            },
            710,
        );

        assert_eq!(op.seq, 12);
        assert_eq!(op.queue, queue);
        assert_eq!((op.current, op.total), (2, 2));
        assert_eq!((op.current_id, op.current_name.as_str()), (20, "Expansion"));
        assert_eq!(
            op.completed,
            [
                CompletedModDto {
                    id: 10,
                    name: "CF".into(),
                    ok: true,
                    ms: 300
                },
                CompletedModDto {
                    id: 20,
                    name: "Expansion (denied)".into(),
                    ok: false,
                    ms: 300
                },
            ]
        );
        // The live progress line was rewritten in place, not appended.
        let lines: Vec<&str> = op.log.iter().map(|l| l.line.as_str()).collect();
        assert_eq!(
            lines,
            ["Logging in...OK", "progress: 90.00", "Download failed"]
        );
        assert_eq!(op.log[1].at_ms, 200);
        assert!(!op.log_live);
        assert!(op.finished());
        assert_eq!(
            (op.ok, op.failed, op.hint.as_deref()),
            (1, 1, Some("private"))
        );
    }

    #[test]
    fn the_log_keeps_its_last_lines() {
        let mut op = ModOpSnapshot::new(1, ModOpType::UpdateAll, ModOpVia::Steamcmd, 0);
        let n = LOG_MAX + LOG_SLACK + 1;
        for i in 0..n {
            op.apply(
                &line(ModProgressKind::LogLine, i as u32 + 1, &i.to_string()),
                i as u64,
            );
        }
        assert_eq!(op.log.len(), LOG_MAX);
        assert_eq!(op.log.last().unwrap().line, (n - 1).to_string());
        assert_eq!(op.log[0].line, (n - LOG_MAX).to_string());
    }

    /// One test for the one process-wide hub: tests run in parallel.
    #[test]
    fn the_hub_numbers_steps_and_keeps_a_finished_run_until_dismissed() {
        // A sink that drops what it is sent (no window behind it).
        let sink = || -> StepSink { Arc::new(|_| {}) };
        let id = begin(ModOpType::Login, ModOpVia::Steamcmd, sink());
        emit(id, ModProgressEvent::of(ModProgressKind::Queued));
        emit(
            id,
            ModProgressEvent::of(ModProgressKind::SteamGuardMobileRequired),
        );
        // A step of another operation is not this one's.
        emit(id + 1, ModProgressEvent::of(ModProgressKind::Finished));
        let s = attach().unwrap();
        assert_eq!(s.seq, 2);
        assert_eq!(s.phase, ModOpPhase::SteamGuardMobile);
        // Still running: a dismiss keeps it.
        dismiss();
        assert!(snapshot().is_some());
        emit(id, ModProgressEvent::of(ModProgressKind::Finished));
        assert!(snapshot().unwrap().finished());
        dismiss();
        assert!(snapshot().is_none());
        // Nothing kept, nothing folded.
        emit(id, ModProgressEvent::of(ModProgressKind::LoggedIn));
        assert!(snapshot().is_none());
        begin(ModOpType::UpdateAll, ModOpVia::Steamworks, sink());
        clear();
        assert!(attach().is_none());
    }
}
