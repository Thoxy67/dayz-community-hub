//! Work off the frame. The window never waits: every hub call runs on the tokio
//! runtime and its answer lands in a [`Task`] the caller polls each frame. A
//! finished task asks for a repaint, so an idle window draws nothing and an
//! answer is on screen the frame after it arrives.

use std::future::Future;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

static RUNTIME: OnceLock<Rt> = OnceLock::new();

/// The runtime and the context to wake.
#[derive(Clone)]
pub struct Rt {
    handle: tokio::runtime::Handle,
    ctx: egui::Context,
}

impl Rt {
    /// Install the runtime for this process. Later calls keep the first.
    pub fn install(handle: tokio::runtime::Handle, ctx: egui::Context) -> &'static Rt {
        RUNTIME.get_or_init(|| Rt { handle, ctx })
    }

    /// The installed runtime. `None` only before [`Rt::install`], which `main`
    /// and `ui_shot` call before the first frame.
    pub fn get() -> Option<&'static Rt> {
        RUNTIME.get()
    }

    pub fn handle(&self) -> &tokio::runtime::Handle {
        &self.handle
    }

    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// Run `fut` on the runtime; its output is the task's, and the window repaints.
    pub fn spawn<T, F>(&self, fut: F) -> Task<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let ctx = self.ctx.clone();
        self.handle.spawn(async move {
            let _ = tx.send(fut.await);
            ctx.request_repaint();
        });
        Task { rx: Some(rx) }
    }

    /// Run `fut` and forget its answer (it still wakes the window).
    pub fn fire<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let ctx = self.ctx.clone();
        self.handle.spawn(async move {
            fut.await;
            ctx.request_repaint();
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
        None => Task { rx: None },
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
}

impl<T> Task<T> {
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
    /// Start loading, superseding a load in flight.
    pub fn load<F>(&mut self, fut: F)
    where
        F: Future<Output = Result<T, String>> + Send + 'static,
    {
        self.task = Some(spawn(fut));
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
