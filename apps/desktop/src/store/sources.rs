//! Where the server list comes from and how far each source has got (=
//! `stores/sources.svelte.ts`): asked once at start, then `HubEvent::SourcesChanged`.

use dz_hub::events::SourcesChanged;
use dz_hub::{Hub, HubEvent};

use crate::rt::{self, Task};

pub struct Sources {
    hub: Hub,
    pub now: Option<Box<SourcesChanged>>,
    task: Option<Task<SourcesChanged>>,
}

impl Sources {
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            now: None,
            task: None,
        }
    }

    pub fn start(&mut self) {
        let hub = self.hub.clone();
        self.task = Some(rt::spawn(async move { hub.sources_status().await }));
    }

    pub fn on_event(&mut self, ev: &HubEvent) {
        if let HubEvent::SourcesChanged(s) = ev {
            self.now = Some(s.clone());
            self.task = None;
        }
    }

    pub fn poll(&mut self) -> bool {
        match self.task.as_mut().and_then(Task::poll) {
            Some(s) => {
                self.task = None;
                self.now = Some(Box::new(s));
                true
            }
            None => false,
        }
    }
}
