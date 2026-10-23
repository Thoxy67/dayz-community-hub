//! The account's Steam friends, kept fresh while they matter (=
//! `stores/friends.svelte.ts`): who plays DayZ and where, for the Friends view, the
//! rail's count and the server list's "friends here" marker.
//!
//! Asked on a cadence, never while the window is in the background: every 20 s while
//! the Friends view shows and a friend is in DayZ, every 60 s while it shows otherwise,
//! every 2 min behind other views (Web API only: the Steam client path shows DayZ
//! running for a moment, so it is only taken while the view is open). The next ask is a
//! `request_repaint_after`, not a timer.

use std::time::{Duration, Instant};

use dz_hub::Hub;
use dz_hub::features::friends::{FriendDto, FriendStatus, FriendsDto, FriendsSource};
use rustc_hash::FxHashMap;

use super::Bus;
use crate::i18n::{t, tf};
use crate::kit::ToastTone;
use crate::rt::{self, Task};

const SHOWN_DAYZ: Duration = Duration::from_secs(20);
const SHOWN: Duration = Duration::from_secs(60);
const BEHIND: Duration = Duration::from_secs(120);
/// More arrivals than this at once are said in one toast.
const TOASTS_MAX: usize = 2;

fn server_key(ip: &str, port: u16) -> String {
    format!("{ip}:{port}")
}

pub struct Friends {
    hub: Hub,
    bus: Bus,
    pub data: Option<FriendsDto>,
    pub loading: bool,
    pub failed: Option<String>,
    /// When the last answer arrived.
    pub updated_at: Option<Instant>,
    /// When the last ask ended, answered or not: the cadence counts from it.
    asked_at: Option<Instant>,
    /// A background ask found no working Web API: no more until the view is opened.
    behind_failed: bool,
    /// When a friend was seen arriving in DayZ, by Steam id: only arrivals this run saw.
    pub since: FxHashMap<String, Instant>,
    in_dayz: usize,
    online: usize,
    /// Indexes into `data.friends` by "ip:port".
    by_server: FxHashMap<String, Vec<usize>>,
    task: Option<(bool, Task<Result<FriendsDto, String>>)>,
    /// Bumped whenever `data` changes: what the view derived from it is redone.
    pub generation: u64,
    /// The Friends view's search, as typed (kept here so commands reach it).
    pub query: String,
    /// The server whose details the Friends view shows.
    pub selected: Option<Selected>,
}

/// A server picked in the Friends view: its address (query port when listed) and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    pub ip: String,
    pub port: u16,
    pub name: String,
}

impl Friends {
    pub fn new(hub: Hub, bus: Bus) -> Self {
        Self {
            hub,
            bus,
            data: None,
            loading: false,
            failed: None,
            updated_at: None,
            asked_at: None,
            behind_failed: false,
            since: FxHashMap::default(),
            in_dayz: 0,
            online: 0,
            by_server: FxHashMap::default(),
            task: None,
            generation: 0,
            query: String::new(),
            selected: None,
        }
    }

    pub fn list(&self) -> &[FriendDto] {
        self.data.as_ref().map_or(&[], |d| d.friends.as_slice())
    }

    pub fn in_dayz(&self) -> usize {
        self.in_dayz
    }

    pub fn online(&self) -> usize {
        self.online
    }

    /// The friends on a server, by its query port (or game port when unlisted).
    pub fn on(&self, ip: &str, port: u16) -> impl Iterator<Item = &FriendDto> {
        self.on_key(&server_key(ip, port))
    }

    /// [`Friends::on`] by a key the caller already holds ("ip:port", the row's own
    /// buffer): nothing is formatted, for lists that ask once per row each frame.
    pub fn on_key<'a>(&'a self, key: &str) -> impl Iterator<Item = &'a FriendDto> + use<'a> {
        let list = self.list();
        self.by_server
            .get(key)
            .into_iter()
            .flatten()
            .filter_map(move |i| list.get(*i))
    }

    fn derive(&mut self) {
        let list = self.data.as_ref().map_or(&[][..], |d| d.friends.as_slice());
        self.in_dayz = list.iter().filter(|f| f.in_dayz).count();
        self.online = list
            .iter()
            .filter(|f| f.status != FriendStatus::Offline || f.game.is_some())
            .count();
        let mut m: FxHashMap<String, Vec<usize>> = FxHashMap::default();
        for (i, f) in list.iter().enumerate() {
            if let (true, Some(s)) = (f.in_dayz, &f.server) {
                m.entry(server_key(&s.ip, s.port)).or_default().push(i);
            }
        }
        self.by_server = m;
    }

    /// How long between asks, or `None` for none.
    fn interval(&self, focused: bool, shown: bool, web: bool) -> Option<Duration> {
        if !focused {
            return None;
        }
        let source = self.data.as_ref().and_then(|d| d.source);
        if shown {
            if source == Some(FriendsSource::SteamClient) {
                return Some(SHOWN);
            }
            return Some(if self.in_dayz > 0 { SHOWN_DAYZ } else { SHOWN });
        }
        if !web || self.behind_failed {
            return None;
        }
        (self.data.is_none() || source == Some(FriendsSource::WebApi)).then_some(BEHIND)
    }

    /// Ask now; `shown` when the Friends view is on screen (the Steam client may be asked).
    pub fn load(&mut self, shown: bool) {
        if self.loading {
            return;
        }
        self.loading = true;
        let hub = self.hub.clone();
        self.task = Some((
            shown,
            rt::spawn(async move { hub.steam_friends(shown).await }),
        ));
    }

    pub fn poll(&mut self, focused: bool, shown: bool, web: bool) -> bool {
        let mut changed = false;
        if let Some((was_shown, task)) = &mut self.task
            && let Some(r) = task.poll()
        {
            let was_shown = *was_shown;
            self.task = None;
            self.loading = false;
            self.asked_at = Some(Instant::now());
            changed = true;
            match r {
                Ok(next) => {
                    if was_shown {
                        self.behind_failed = false;
                    }
                    if !was_shown && next.source.is_none() {
                        // Not set up, or Steam refused: what the view showed stays.
                        self.behind_failed = true;
                    } else {
                        if let Some(before) = &self.data
                            && before.source.is_some()
                            && next.source.is_some()
                        {
                            let before = before.clone();
                            self.arrivals(&before, &next);
                        }
                        self.data = Some(next);
                        self.generation = self.generation.wrapping_add(1);
                        self.derive();
                        self.failed = None;
                        self.updated_at = Some(Instant::now());
                    }
                }
                Err(e) => self.failed = Some(e),
            }
        }
        if !self.loading
            && let Some(every) = self.interval(focused, shown, web)
        {
            let due = self.asked_at.map_or(Instant::now(), |a| a + every);
            let now = Instant::now();
            if now >= due {
                self.load(shown);
            } else {
                self.bus.ctx().request_repaint_after(due - now);
            }
        }
        changed
    }

    /// Friends who started DayZ or moved to another server since the last answer.
    fn arrivals(&mut self, before: &FriendsDto, after: &FriendsDto) {
        let was: FxHashMap<&str, &FriendDto> = before
            .friends
            .iter()
            .map(|f| (f.steam_id.as_str(), f))
            .collect();
        let now = Instant::now();
        let mut came: Vec<&FriendDto> = Vec::new();
        for f in &after.friends {
            if !f.in_dayz {
                self.since.remove(&f.steam_id);
                continue;
            }
            let Some(p) = was.get(f.steam_id.as_str()) else {
                continue;
            };
            let started = !p.in_dayz;
            let moved = !started
                && f.server.as_ref().is_some_and(|s| {
                    p.server
                        .as_ref()
                        .is_none_or(|ps| server_key(&ps.ip, ps.port) != server_key(&s.ip, s.port))
                });
            if started {
                self.since.insert(f.steam_id.clone(), now);
            }
            if started || moved {
                came.push(f);
            }
        }
        if came.is_empty() {
            return;
        }
        if came.len() > TOASTS_MAX {
            self.bus.say_with_action(
                tf("friends", "toastMany", &[("count", &came.len())]),
                ToastTone::Neutral,
                t("friends", "toastShow"),
                "view.friends",
            );
            return;
        }
        for f in came {
            match &f.server {
                Some(s) => {
                    let where_ = s
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("{}:{}", s.ip, s.port));
                    let args = serde_json::json!({ "ip": s.ip, "port": s.port });
                    self.bus.say_with_action(
                        tf(
                            "friends",
                            "toastJoined",
                            &[("name", &f.name), ("server", &where_)],
                        ),
                        ToastTone::Neutral,
                        t("friends", "toastJoin"),
                        format!("connect.address {args}"),
                    );
                }
                None => self
                    .bus
                    .info(tf("friends", "toastStarted", &[("name", &f.name)])),
            }
        }
    }
}
