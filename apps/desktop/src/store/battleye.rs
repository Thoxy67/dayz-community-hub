//! BattlEye, which most servers require (= `stores/battleye.svelte.ts`): its service on
//! Windows, the Proton runtime on Linux. Read by the status bar and the settings,
//! repaired from either.

use dz_game::battleye::{BattlEyeState, BattlEyeStatus};
use dz_hub::Hub;

use super::Bus;
use crate::i18n::{t, tf};
use crate::rt::{self, Task};

pub struct BattlEye {
    hub: Hub,
    bus: Bus,
    pub status: Option<BattlEyeStatus>,
    pub repairing: bool,
    refresh: Option<Task<Option<BattlEyeStatus>>>,
    repair: Option<Task<Result<BattlEyeStatus, String>>>,
}

impl BattlEye {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            status: None,
            repairing: false,
            refresh: None,
            repair: None,
        }
    }

    /// Read the status again, unless a read is still under way (asked every 5 s while
    /// DayZ runs; a slow service query must not stack reads).
    pub fn refresh(&mut self) {
        if self.refresh.as_ref().is_some_and(Task::is_pending) {
            return;
        }
        let hub = self.hub.clone();
        self.refresh = Some(rt::spawn(async move { hub.battleye_status().await.ok() }));
    }

    /// Repair it; `clear` first deletes the game's data outside its folder.
    pub fn repair(&mut self, clear: bool) {
        if self.repairing {
            return;
        }
        self.repairing = true;
        let hub = self.hub.clone();
        self.repair = Some(rt::spawn(async move { hub.repair_battleye(clear).await }));
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(t) = &mut self.refresh
            && let Some(s) = t.poll()
        {
            self.refresh = None;
            self.status = s;
            changed = true;
        }
        if let Some(task) = &mut self.repair
            && let Some(r) = task.poll()
        {
            self.repair = None;
            self.repairing = false;
            changed = true;
            match r {
                Ok(s) => {
                    if matches!(s.state, BattlEyeState::Running | BattlEyeState::Ready) {
                        self.bus.ok(t("shell", "sbBeRepaired"));
                    } else if !s.service {
                        self.bus.info(t("shell", "sbBeInstalling"));
                    } else {
                        self.bus.warn(t("shell", "sbBeStillMissing"));
                    }
                    self.status = Some(s);
                }
                Err(e) => self
                    .bus
                    .err(tf("shell", "sbBeRepairFailed", &[("error", &e)])),
            }
        }
        changed
    }
}
