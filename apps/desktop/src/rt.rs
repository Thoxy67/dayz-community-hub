//! Work off the frame. The window never waits: every hub call runs on the tokio
//! runtime and its answer lands in a [`Task`] the caller polls each frame. A
//! finished task asks for a repaint, so an idle window draws nothing and an
//! answer is on screen the frame after it arrives.

use std::future::Future;
use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError, RwLock};
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

static RUNTIME: OnceLock<Rt> = OnceLock::new();

/// Whoever is to be woken when work finishes or something arrives: the window's
/// context while there is a window, and, between windows (Wayland's close to the
/// tray, where the window is really closed and made again), whoever waits in
/// [`Waker::wait`] (`main`). Cheap to clone, `Send`; the context can be replaced.
#[derive(Clone, Default)]
pub struct Waker(Arc<WakerInner>);

#[derive(Default)]
struct WakerInner {
    ctx: RwLock<Option<egui::Context>>,
    nudged: Mutex<bool>,
    cv: Condvar,
}

impl Waker {
    /// A waker already pointing at `ctx`.
    pub fn with(ctx: &egui::Context) -> Self {
        let w = Self::default();
        w.set(ctx);
        w
    }

    /// The window's context from now on (a new window).
    pub fn set(&self, ctx: &egui::Context) {
        *self.0.ctx.write().unwrap_or_else(PoisonError::into_inner) = Some(ctx.clone());
    }

    /// No window any more: wakes go to [`Waker::wait`] only.
    pub fn clear(&self) {
        *self.0.ctx.write().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// The window's context, while there is a window.
    pub fn ctx(&self) -> Option<egui::Context> {
        self.0
            .ctx
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Wake now.
    pub fn repaint(&self) {
        self.repaint_after(Duration::ZERO);
    }

    /// Wake the window after `d` (folding a burst into one frame); a waiter between
    /// windows at once.
    pub fn repaint_after(&self, d: Duration) {
        if let Some(ctx) = self.ctx() {
            if d.is_zero() {
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(d);
            }
        }
        let mut n = self.0.nudged.lock().unwrap_or_else(PoisonError::into_inner);
        *n = true;
        self.0.cv.notify_all();
    }

    /// Between windows: wait until something wakes, or `timeout`. True when woken.
    pub fn wait(&self, timeout: Duration) -> bool {
        let n = self.0.nudged.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut n, _) = self
            .0
            .cv
            .wait_timeout_while(n, timeout, |n| !*n)
            .unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut *n)
    }
}

/// The runtime and who to wake.
#[derive(Clone)]
pub struct Rt {
    handle: tokio::runtime::Handle,
    waker: Waker,
}

impl Rt {
    /// Install the runtime for this process, waking `ctx`. Later calls keep the first.
    pub fn install(handle: tokio::runtime::Handle, ctx: egui::Context) -> &'static Rt {
        RUNTIME.get_or_init(|| Rt {
            handle,
            waker: Waker::with(&ctx),
        })
    }

    /// Install the runtime for this process, waking whoever `waker` points at (the
    /// window of the moment). Later calls keep the first.
    pub fn install_with(handle: tokio::runtime::Handle, waker: Waker) -> &'static Rt {
        RUNTIME.get_or_init(|| Rt { handle, waker })
    }

    /// The installed runtime. `None` only before [`Rt::install`], which `main`
    /// and `ui_shot` call before the first frame.
    pub fn get() -> Option<&'static Rt> {
        RUNTIME.get()
    }

    pub fn handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }

    /// The window's context; `None` between windows.
    pub fn ctx(&self) -> Option<egui::Context> {
        self.waker.ctx()
    }

    pub fn waker(&self) -> &Waker {
        &self.waker
    }

    /// Run `fut` on the runtime; its output is the task's, and the window repaints.
    pub fn spawn<T, F>(&self, fut: F) -> Task<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let waker = self.waker.clone();
        self.handle.spawn(async move {
            let _ = tx.send(fut.await);
            waker.repaint();
        });
        Task {
            rx: Some(rx),
            abort: None,
        }
    }

    /// [`Rt::spawn`] for a read whose answer only matters to its task: dropping or
    /// replacing the task stops the work at its next await (an A2S query, a fetch),
    /// instead of letting it run to an answer nobody takes. Never for a write.
    pub fn spawn_abortable<T, F>(&self, fut: F) -> Task<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let waker = self.waker.clone();
        let join = self.handle.spawn(async move {
            let _ = tx.send(fut.await);
            waker.repaint();
        });
        Task {
            rx: Some(rx),
            abort: Some(join.abort_handle()),
        }
    }

    /// Run `fut` and forget its answer (it still wakes the window).
    pub fn fire<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let waker = self.waker.clone();
        self.handle.spawn(async move {
            fut.await;
            waker.repaint();
        });
    }
}

/// [`Rt::spawn`] on the installed runtime; a task that never finishes before it is installed.
pub fn spawn<T, F>(fut: F) -> Task<T>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    match Rt::get() {
        Some(rt) => rt.spawn(fut),
        None => Task::never(),
    }
}

/// [`Rt::spawn_abortable`] on the installed runtime.
pub fn spawn_abortable<T, F>(fut: F) -> Task<T>
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    match Rt::get() {
        Some(rt) => rt.spawn_abortable(fut),
        None => Task::never(),
    }
}

/// [`Rt::fire`] on the installed runtime.
pub fn fire<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Some(rt) = Rt::get() {
        rt.fire(fut);
    }
}

/// An answer on its way.
#[must_use]
pub struct Task<T> {
    rx: Option<oneshot::Receiver<T>>,
    /// Set by [`spawn_abortable`]: the work stops when the task goes unanswered.
    abort: Option<tokio::task::AbortHandle>,
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        if let Some(a) = &self.abort {
            a.abort();
        }
    }
}

impl<T> Task<T> {
    /// A task with no work behind it: never answers.
    fn never() -> Self {
        Self {
            rx: None,
            abort: None,
        }
    }

    /// The answer, once, on the first frame it is there.
    pub fn poll(&mut self) -> Option<T> {
        let rx = self.rx.as_mut()?;
        match rx.try_recv() {
            Ok(v) => {
                self.rx = None;
                Some(v)
            }
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                self.rx = None;
                None
            }
        }
    }

    pub fn is_pending(&self) -> bool {
        self.rx.is_some()
    }
}

/// A value that is loaded, reloaded and superseded: the newest request wins, an
/// older answer that arrives late is dropped. Keeps the last good value while a
/// new one loads, so a list never blinks empty.
pub struct Fetch<T> {
    pub value: Option<T>,
    pub error: Option<String>,
    task: Option<Task<Result<T, String>>>,
}

impl<T> Default for Fetch<T> {
    fn default() -> Self {
        Self {
            value: None,
            error: None,
            task: None,
        }
    }
}

impl<T: Send + 'static> Fetch<T> {
    /// Start loading, superseding a load in flight (whose work stops: a load is a
    /// read, and only the newest answer is taken).
    pub fn load<F>(&mut self, fut: F)
    where
        F: Future<Output = Result<T, String>> + Send + 'static,
    {
        self.task = Some(spawn_abortable(fut));
    }

    /// Take a finished answer in; true when the value or error changed.
    pub fn poll(&mut self) -> bool {
        let Some(task) = self.task.as_mut() else {
            return false;
        };
        match task.poll() {
            Some(Ok(v)) => {
                self.value = Some(v);
                self.error = None;
                self.task = None;
                true
            }
            Some(Err(e)) => {
                self.error = Some(e);
                self.task = None;
                true
            }
            None => {
                if !task.is_pending() {
                    self.task = None;
                }
                false
            }
        }
    }

    pub fn loading(&self) -> bool {
        self.task.is_some()
    }

    /// Nothing loaded yet and nothing failed: show a skeleton.
    pub fn first_load(&self) -> bool {
        self.value.is_none() && self.error.is_none()
    }
}

/// Fires once input has been quiet for `delay` (typing in a search box).
pub struct Debounce {
    delay: Duration,
    due: Option<Instant>,
}

impl Debounce {
    pub fn new(delay: Duration) -> Self {
        Self { delay, due: None }
    }

    /// Something changed: start (or restart) the wait.
    pub fn touch(&mut self, ctx: &egui::Context) {
        self.due = Some(Instant::now() + self.delay);
        ctx.request_repaint_after(self.delay);
    }

    /// True once, when the wait is over.
    pub fn ready(&mut self, ctx: &egui::Context) -> bool {
        match self.due {
            Some(t) if Instant::now() >= t => {
                self.due = None;
                true
            }
            Some(t) => {
                ctx.request_repaint_after(t.saturating_duration_since(Instant::now()));
                false
            }
            None => false,
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_waker_wakes_the_window_of_the_moment_or_the_waiter() {
        let w = Waker::default();
        assert!(w.ctx().is_none());
        assert!(!w.wait(Duration::ZERO), "nothing woke it yet");
        // Between windows: a wake from another thread ends the wait.
        let w2 = w.clone();
        let t = std::thread::spawn(move || w2.repaint());
        assert!(w.wait(Duration::from_secs(5)));
        t.join().unwrap();
        assert!(!w.wait(Duration::ZERO), "taken once");
        // A window, then another: the newest is the one woken.
        let (a, b) = (egui::Context::default(), egui::Context::default());
        w.set(&a);
        w.set(&b);
        assert!(w.ctx().is_some_and(|c| c == b));
        w.clear();
        assert!(w.ctx().is_none());
    }
}
