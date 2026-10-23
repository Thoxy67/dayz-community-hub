//! The launcher's own updates (= `stores/updater.svelte.ts`). A Windows zip and an
//! AppImage replace themselves (downloaded, checked against the signing key, then a
//! restart); a distribution package or a dev build is only told a new version exists,
//! with the reason it cannot install it here. A download's progress arrives as
//! `HubEvent::UpdateDownload`, whoever started it.

use dz_hub::events::DownloadEvent;
use dz_hub::features::updater::{DownloadState, UpdateInfo, UpdateSupport};
use dz_hub::{Hub, HubEvent};

use crate::rt::{self, Task};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Done,
    Error,
}

type CheckAnswer = (Option<UpdateSupport>, Result<Option<UpdateInfo>, String>);

pub struct Updater {
    hub: Hub,
    pub state: UpdateState,
    pub info: Option<UpdateInfo>,
    pub error: String,
    pub received: u64,
    pub total: u64,
    /// Whether this copy can install an update itself; `None` until asked.
    pub support: Option<UpdateSupport>,
    check: Option<Task<CheckAnswer>>,
    install: Option<Task<Result<(), String>>>,
    resume: Option<Task<DownloadState>>,
    restart: Option<Task<Result<bool, String>>>,
}

impl Updater {
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            state: UpdateState::Idle,
            info: None,
            error: String::new(),
            received: 0,
            total: 0,
            support: None,
            check: None,
            install: None,
            resume: None,
            restart: None,
        }
    }

    /// Percent of the download, 0 when its size is unknown.
    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        let p = (self.received.saturating_mul(100) / self.total).min(100);
        u32::try_from(p).unwrap_or(100)
    }

    pub fn check(&mut self) {
        if matches!(self.state, UpdateState::Checking | UpdateState::Downloading) {
            return;
        }
        self.state = UpdateState::Checking;
        self.error.clear();
        let hub = self.hub.clone();
        let ask_support = self.support.is_none();
        self.check = Some(rt::spawn(async move {
            // Both questions at once: neither waits on the other.
            let support = async {
                if ask_support {
                    Some(hub.update_support().await)
                } else {
                    None
                }
            };
            let (support, info) = tokio::join!(support, hub.check_for_update());
            (support, info)
        }));
    }

    pub fn install(&mut self) {
        if self.support.as_ref().is_some_and(|s| !s.supported)
            || self.state == UpdateState::Downloading
        {
            return;
        }
        self.state = UpdateState::Downloading;
        self.received = 0;
        self.total = 0;
        self.error.clear();
        let hub = self.hub.clone();
        self.install = Some(rt::spawn(async move { hub.install_update().await }));
    }

    /// At start: a download the hub runs is shown and followed, otherwise a check.
    pub fn resume(&mut self) {
        let hub = self.hub.clone();
        self.resume = Some(rt::spawn(async move { hub.update_download_state().await }));
    }

    /// Start the new version: the hub starts the replaced executable or AppImage,
    /// then this window closes the normal way (its preferences and size saved); the
    /// new instance waits for this one to be gone. The mock starts nothing.
    pub fn restart(&mut self) {
        if self.restart.is_some() {
            return;
        }
        let hub = self.hub.clone();
        self.restart = Some(rt::spawn(async move { hub.relaunch_app().await }));
    }

    pub fn on_event(&mut self, ev: &HubEvent) {
        let HubEvent::UpdateDownload(ev) = ev else {
            return;
        };
        match ev {
            DownloadEvent::Started { content_length } => {
                self.state = UpdateState::Downloading;
                self.total = content_length.unwrap_or(0);
                self.received = 0;
            }
            DownloadEvent::Progress { chunk_length } => {
                self.received = self
                    .received
                    .saturating_add(u64::try_from(*chunk_length).unwrap_or(0));
            }
            DownloadEvent::Finished => self.state = UpdateState::Done,
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(t) = &mut self.check
            && let Some((support, info)) = t.poll()
        {
            self.check = None;
            changed = true;
            if support.is_some() {
                self.support = support;
            }
            match info {
                Ok(info) => {
                    self.state = if info.is_some() {
                        UpdateState::Available
                    } else {
                        UpdateState::UpToDate
                    };
                    self.info = info;
                }
                Err(e) => {
                    self.error = e;
                    self.state = UpdateState::Error;
                }
            }
        }
        if let Some(t) = &mut self.install
            && let Some(r) = t.poll()
        {
            self.install = None;
            changed = true;
            if let Err(e) = r {
                self.error = e;
                self.state = UpdateState::Error;
            }
        }
        if let Some(t) = &mut self.resume
            && let Some(d) = t.poll()
        {
            self.resume = None;
            changed = true;
            if d.running || d.done {
                // No check meanwhile: on Windows it clears the last update's
                // leftovers, which a download in flight may be writing.
                self.received = d.received;
                self.total = d.total.unwrap_or(0);
                self.state = if d.done {
                    UpdateState::Done
                } else {
                    UpdateState::Downloading
                };
            } else {
                self.check();
            }
        }
        if let Some(t) = &mut self.restart
            && let Some(r) = t.poll()
        {
            self.restart = None;
            changed = true;
            match r {
                Ok(true) => {
                    if let Some(rt) = rt::Rt::get() {
                        crate::platform::life::quit(rt.ctx());
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    self.error = e;
                    self.state = UpdateState::Error;
                }
            }
        }
        changed
    }
}
