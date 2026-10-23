//! The [`Hub`]: what the Tauri shell's managed state and `AppHandle` were,
//! as one cloneable handle the window keeps.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::events::HubEvent;
use crate::features::dzch_cli::CliArgs;
use crate::features::ping::PingState;
use crate::features::updater::Pending;
use crate::state::SharedState;

/// Where the hub's events go: the window's sink pushes them into a channel
/// and asks for a repaint. Called from any thread, never on the UI thread's
/// behalf, so it must not block.
pub type EventSink = Arc<dyn Fn(HubEvent) + Send + Sync>;

/// Which world the hub talks to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HubMode {
    /// The real one: the user's profile, the network, Steam, the game.
    #[default]
    Live,
    /// An invented world for screenshots (`--mock`, `ui_shot`): see `mock`.
    Mock,
}

/// How a [`Hub`] is set up.
#[derive(Clone, Debug, Default)]
pub struct HubConfig {
    pub mode: HubMode,
    /// The data directory, in place of `dz_common::paths::default_data_dir()`
    /// (and of Tauri's `app_data_dir()` for the image cache). The core crates'
    /// own SteamCMD and Steamworks directories are not moved by it.
    pub data_dir: Option<PathBuf>,
    /// The arguments this instance started with, handed once by
    /// [`Hub::get_cli_args`].
    pub args: CliArgs,
}

impl HubConfig {
    pub fn is_mock(&self) -> bool {
        self.mode == HubMode::Mock
    }
}

/// The backend the window talks to. Cheap to clone: an `Arc` inside.
#[derive(Clone)]
pub struct Hub {
    pub(crate) inner: Arc<Inner>,
}

pub(crate) struct Inner {
    pub(crate) config: HubConfig,
    sink: EventSink,
    /// Created by `initialize`, as Tauri's `app.manage(state)` was.
    state: OnceLock<SharedState>,
    pub(crate) ping: Arc<PingState>,
    pub(crate) pending: Pending,
    /// The start-up arguments, until `get_cli_args` hands them out.
    pub(crate) args: Mutex<Option<CliArgs>>,
    /// Whether this is a first launch, taken by [`Hub::prewarm`] before its
    /// work can create the profile, until `check_first_launch` takes it.
    first_launch: Mutex<Option<bool>>,
    started: AtomicBool,
    /// The invented world, in Mock mode.
    #[cfg(feature = "mock")]
    mock: Option<Arc<crate::mock::World>>,
}

impl Hub {
    /// A hub with no state yet: `initialize` creates it. Call [`Hub::start`]
    /// from inside the tokio runtime once.
    ///
    /// In Mock mode the invented world is made first and its profile and
    /// play log written into its data directory: `config.data_dir`, else a
    /// fresh temporary one (removed when the hub goes). A build without the
    /// `mock` feature refuses Mock mode rather than touch the user's files.
    pub fn new(config: HubConfig, sink: EventSink) -> Result<Hub, String> {
        #[cfg(not(feature = "mock"))]
        if config.is_mock() {
            return Err("This build has no mock world (dz-hub's `mock` feature)".to_string());
        }
        #[cfg(feature = "mock")]
        let mock = if config.is_mock() {
            Some(Arc::new(crate::mock::World::create(
                config.data_dir.clone(),
            )?))
        } else {
            None
        };
        #[cfg(feature = "mock")]
        let data_dir = match &mock {
            Some(world) => Some(world.data_dir().to_path_buf()),
            None => config.data_dir.clone(),
        };
        #[cfg(not(feature = "mock"))]
        let data_dir = config.data_dir.clone();
        crate::paths::set_override(data_dir);
        let args = Some(config.args.clone());
        Ok(Hub {
            inner: Arc::new(Inner {
                config,
                sink,
                state: OnceLock::new(),
                ping: Arc::new(PingState::default()),
                pending: Pending::default(),
                args: Mutex::new(args),
                first_launch: Mutex::new(None),
                started: AtomicBool::new(false),
                #[cfg(feature = "mock")]
                mock,
            }),
        })
    }

    /// What Tauri's `setup()` started: the `servers-changed` and
    /// `sources-changed` notifiers, the gamepad thread and the play-time
    /// tracker. Must run inside the tokio runtime; a second call does nothing.
    pub fn start(&self) {
        if self.inner.started.swap(true, Ordering::SeqCst) {
            return;
        }
        crate::features::browser::spawn_change_notifier(self.clone());
        crate::features::servers::sources::spawn_notifier(self.clone());
        if !self.is_mock() {
            crate::features::gamepad::spawn(self.clone());
            crate::features::stats::tracker::spawn(self.clone());
            crate::features::discord::spawn(self.clone());
        }
    }

    pub fn config(&self) -> &HubConfig {
        &self.inner.config
    }

    pub fn is_mock(&self) -> bool {
        self.inner.config.is_mock()
    }

    /// The invented world, in Mock mode.
    #[cfg(feature = "mock")]
    pub(crate) fn mock(&self) -> Option<&crate::mock::World> {
        self.inner.mock.as_deref()
    }

    /// Send an event to the window.
    pub(crate) fn emit(&self, event: HubEvent) {
        (self.inner.sink)(event);
    }

    /// The shared state, once `initialize` has made it.
    pub(crate) fn try_state(&self) -> Option<SharedState> {
        self.inner.state.get().map(Arc::clone)
    }

    /// The shared state, or the error a command gets before `initialize`.
    pub(crate) fn state(&self) -> Result<SharedState, String> {
        self.try_state()
            .ok_or_else(|| "The launcher has not started yet".to_string())
    }

    /// Keep the state `initialize` made. The first one stays.
    pub(crate) fn set_state(&self, state: SharedState) {
        let _ = self.inner.state.set(state);
    }

    /// Note now whether the profile exists (one stat), before anything started
    /// here can write a default one: `initialize` does, and the window asks
    /// `check_first_launch` only on its first frame, after it.
    pub(crate) fn note_first_launch(&self) {
        let first = !crate::paths::profile_path().exists();
        let mut f = self
            .inner
            .first_launch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f.get_or_insert(first);
    }

    /// The answer [`Hub::note_first_launch`] kept, once (a later ask, after the
    /// wizard wrote the profile, looks at the disk again).
    pub(crate) fn take_first_launch(&self) -> Option<bool> {
        self.inner
            .first_launch
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub(crate) fn ping(&self) -> &Arc<PingState> {
        &self.inner.ping
    }

    /// A second launch handed its arguments (or a `dzch://` link) to this
    /// one: tell the window, as the single-instance plugin's `cli-args` did.
    pub fn hand_off(&self, args: CliArgs) {
        self.emit(HubEvent::CliArgs(args));
    }

    /// The window gained or lost focus: the pads speak only to a focused one.
    pub fn set_focused(&self, focused: bool) {
        crate::features::gamepad::set_focused(focused);
    }
}
