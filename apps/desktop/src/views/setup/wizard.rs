//! The setup's state (= `features/setup/wizard.svelte.ts`): which step, what has been typed
//! in, where DayZ is, and how mods will download.
//!
//! DayZ is looked for when its step opens, and again 500 ms after the library field stops
//! changing (the newest search wins). With SteamCMD chosen, SteamCMD is looked for too;
//! while it is missing the wizard keeps looking every 3 s (the player may be installing it
//! in a terminal right now) and the step flips by itself. With the Steam client chosen,
//! SteamCMD and the Steam login are not needed: whether Steam runs is checked instead.
//!
//! Every hub call is a [`Task`] polled each frame: nothing here waits.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use dz_hub::Hub;
use dz_hub::events::SteamcmdStatusDto;
use dz_hub::features::steam::SteamMeDto;
use dz_hub::features::steamworks::{ModDownloaderDto, SteamworksStatusDto};

use crate::rt::{self, Debounce, Task};

/// The steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Step {
    Welcome,
    Game,
    Downloads,
    Account,
    Services,
    Appearance,
    Done,
}

impl Step {
    pub const ALL: [Step; 7] = [
        Step::Welcome,
        Step::Game,
        Step::Downloads,
        Step::Account,
        Step::Services,
        Step::Appearance,
        Step::Done,
    ];

    /// As the commands and the control channel write it.
    pub fn id(self) -> &'static str {
        match self {
            Step::Welcome => "welcome",
            Step::Game => "game",
            Step::Downloads => "downloads",
            Step::Account => "account",
            Step::Services => "services",
            Step::Appearance => "appearance",
            Step::Done => "done",
        }
    }

    pub fn from_id(id: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|s| s.id() == id)
    }

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn at(i: usize) -> Option<Step> {
        Step::ALL.get(i).copied()
    }

    /// Its name in the list of steps (`setup` dictionary).
    pub fn label_key(self) -> &'static str {
        match self {
            Step::Welcome => "stepWelcome",
            Step::Game => "stepGame",
            Step::Downloads => "stepDownloads",
            Step::Account => "stepAccount",
            Step::Services => "stepServices",
            Step::Appearance => "stepAppearance",
            Step::Done => "stepDone",
        }
    }
}

/// Where DayZ is (the hub's `DayzDetectDto`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GameFound {
    /// The `steamapps` folder in use.
    pub steamapps: Option<String>,
    /// The game's folder, when it is installed.
    pub dayz_dir: Option<String>,
    /// Mods the Steam client already downloaded.
    pub workshop_mods: u32,
}

/// SteamCMD is looked for this often while it is missing…
const WATCH_EVERY: Duration = Duration::from_secs(3);
/// …and for this long at most: the setup is not left open for hours.
const WATCH_FOR: Duration = Duration::from_secs(15 * 60);
/// A typed library is searched once the typing stops this long.
const ROOT_QUIET: Duration = Duration::from_millis(500);
/// A copied command shows its check this long.
pub const COPIED_FOR: Duration = Duration::from_millis(1400);

pub struct Wizard {
    hub: Hub,
    pub step: Step,

    pub player: String,
    pub steam_login: String,
    pub steam_root: String,
    pub steamcmd_path: String,
    pub steam_api_key: String,
    pub steam_id: String,
    pub downloader: ModDownloaderDto,

    pub game: Option<GameFound>,
    game_task: Option<Task<GameFound>>,
    pub steam: Option<SteamworksStatusDto>,
    steam_task: Option<Task<SteamworksStatusDto>>,

    pub status: Option<SteamcmdStatusDto>,
    detect_task: Option<Task<SteamcmdStatusDto>>,
    watch: Option<(Task<Option<SteamcmdStatusDto>>, Arc<AtomicBool>)>,
    download_task: Option<Task<Result<String, String>>>,
    pub download_error: String,

    /// The Steam id and name came from the running Steam client.
    pub from_steam: bool,
    asked_steam: bool,
    me_task: Option<Task<Option<SteamMeDto>>>,

    /// The install command copied last, and when.
    pub copied: Option<(String, Instant)>,
    root_quiet: Debounce,
    /// The library field as last searched (a change starts the 500 ms wait).
    root_seen: String,

    /// When the wizard opened and when the step changed (egui time), for their motion.
    pub opened_at: Option<f64>,
    pub step_at: Option<f64>,
}

impl Wizard {
    pub fn new(hub: Hub) -> Self {
        Self {
            hub,
            step: Step::Welcome,
            player: String::new(),
            steam_login: String::new(),
            steam_root: String::new(),
            steamcmd_path: String::new(),
            steam_api_key: String::new(),
            steam_id: String::new(),
            downloader: ModDownloaderDto::Steamcmd,
            game: None,
            game_task: None,
            steam: None,
            steam_task: None,
            status: None,
            detect_task: None,
            watch: None,
            download_task: None,
            download_error: String::new(),
            from_steam: false,
            asked_steam: false,
            me_task: None,
            copied: None,
            root_quiet: Debounce::new(ROOT_QUIET),
            root_seen: String::new(),
            opened_at: None,
            step_at: None,
        }
    }

    pub fn index(&self) -> usize {
        self.step.index()
    }

    /// `windows` or `linux`: what the hub said, else what this build runs on.
    pub fn windows(&self) -> bool {
        match &self.status {
            Some(s) => s.platform == "windows",
            None => cfg!(windows),
        }
    }

    pub fn found(&self) -> bool {
        self.status.as_ref().is_some_and(|s| s.found)
    }

    pub fn via_steam(&self) -> bool {
        self.downloader == ModDownloaderDto::Steamworks
    }

    /// SteamCMD needs a Steam login to download mods; the Steam client does not.
    pub fn needs_login(&self) -> bool {
        !self.via_steam()
    }

    /// The account step cannot be passed without a Steam login, when one is needed.
    pub fn can_advance(&self) -> bool {
        self.step != Step::Account || !self.needs_login() || !self.steam_login.trim().is_empty()
    }

    /// Whether the list of steps lets the player jump to `i`: back anywhere, forward
    /// only to the next one, and not while the current one blocks.
    pub fn reachable(&self, i: usize) -> bool {
        let cur = self.index();
        !(i > cur + 1 || (i > cur && !self.can_advance()))
    }

    pub fn finding_game(&self) -> bool {
        self.game_task.is_some()
    }

    pub fn checking_steam(&self) -> bool {
        self.steam_task.is_some()
    }

    pub fn detecting(&self) -> bool {
        self.detect_task.is_some()
    }

    pub fn downloading(&self) -> bool {
        self.download_task.is_some()
    }

    pub fn watching(&self) -> bool {
        self.watch.is_some()
    }

    // ── moving ──────────────────────────────────────────────────────────────

    pub fn go(&mut self, step: Step) {
        if step != self.step {
            self.step_at = None;
        }
        self.step = step;
        if matches!(step, Step::Account | Step::Services) {
            self.ask_steam();
        }
        if step == Step::Game && self.game.is_none() && !self.finding_game() {
            self.find_game();
        }
        if step == Step::Downloads {
            self.prepare_downloads();
        }
    }

    pub fn next(&mut self) {
        if !self.can_advance() {
            return;
        }
        if let Some(s) = Step::at(self.index() + 1) {
            self.go(s);
        }
    }

    pub fn back(&mut self) {
        if let Some(s) = self.index().checked_sub(1).and_then(Step::at) {
            self.go(s);
        }
    }

    fn prepare_downloads(&mut self) {
        if self.via_steam() {
            if self.steam.is_none() && !self.checking_steam() {
                self.check_steam();
            }
        } else if !self.found() && !self.detecting() {
            self.detect();
        }
    }

    pub fn choose(&mut self, d: ModDownloaderDto) {
        self.downloader = d;
        if d == ModDownloaderDto::Steamworks {
            self.stop_watching();
        }
        self.prepare_downloads();
    }

    // ── what the running Steam client knows ─────────────────────────────────

    /// Fill in the name and Steam id, once, where nothing was typed.
    fn ask_steam(&mut self) {
        if self.asked_steam {
            return;
        }
        self.asked_steam = true;
        let hub = self.hub.clone();
        self.me_task = Some(rt::spawn(async move { hub.steam_me().await.ok() }));
    }

    // ── the game ────────────────────────────────────────────────────────────

    /// Where DayZ is: in the library typed or picked, or wherever Steam is installed.
    /// A found folder only shows as the field's placeholder, so `steam_root` holds
    /// nothing but what the player gave. Supersedes a search in flight.
    pub fn find_game(&mut self) {
        let root = self.steam_root.trim().to_string();
        self.root_seen.clone_from(&self.steam_root);
        let hub = self.hub.clone();
        self.game_task = Some(rt::spawn(async move {
            let path = (!root.is_empty()).then_some(root);
            match hub.detect_dayz(path).await {
                Ok(d) => GameFound {
                    steamapps: d.steamapps,
                    dayz_dir: d.dayz_dir,
                    workshop_mods: d.workshop_mods,
                },
                Err(_) => GameFound::default(),
            }
        }));
    }

    /// The library field changed (typed, or picked): search once it is quiet.
    pub fn root_changed(&mut self, ctx: &egui::Context) {
        self.root_quiet.touch(ctx);
    }

    // ── the Steam client ────────────────────────────────────────────────────

    /// Whether Valve's library loads and the Steam client runs.
    pub fn check_steam(&mut self) {
        let hub = self.hub.clone();
        self.steam_task = Some(rt::spawn(async move {
            match hub.steamworks_status().await {
                Ok(s) => s,
                Err(e) => SteamworksStatusDto {
                    library: false,
                    error: Some(e),
                    steam_running: false,
                },
            }
        }));
    }

    // ── SteamCMD ────────────────────────────────────────────────────────────

    pub fn detect(&mut self) {
        self.download_error.clear();
        let hub = self.hub.clone();
        let platform = if self.windows() { "windows" } else { "linux" };
        self.detect_task = Some(rt::spawn(async move {
            hub.detect_steamcmd()
                .await
                .unwrap_or_else(|_| SteamcmdStatusDto {
                    found: false,
                    path: None,
                    platform: platform.to_string(),
                })
        }));
    }

    fn apply(&mut self, s: SteamcmdStatusDto) {
        if s.found
            && let Some(p) = &s.path
        {
            self.steamcmd_path.clone_from(p);
            self.stop_watching();
        }
        self.status = Some(s);
    }

    /// Keep looking for SteamCMD until it appears (or a quarter of an hour passes).
    fn watch(&mut self) {
        if self.watch.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let hub = self.hub.clone();
        let task = rt::spawn(async move {
            let started = Instant::now();
            while started.elapsed() < WATCH_FOR {
                tokio::time::sleep(WATCH_EVERY).await;
                if flag.load(Ordering::Acquire) {
                    return None;
                }
                if let Ok(s) = hub.detect_steamcmd().await
                    && s.found
                {
                    return Some(s);
                }
            }
            None
        });
        self.watch = Some((task, stop));
    }

    pub fn stop_watching(&mut self) {
        if let Some((_, stop)) = self.watch.take() {
            stop.store(true, Ordering::Release);
        }
    }

    /// Windows: fetch SteamCMD from Valve into the app's folder.
    pub fn download(&mut self) {
        if self.downloading() {
            return;
        }
        self.download_error.clear();
        let hub = self.hub.clone();
        self.download_task = Some(rt::spawn(
            async move { hub.download_steamcmd_windows().await },
        ));
    }

    /// A path chosen by hand counts as found.
    pub fn chose(&mut self, path: String) {
        let platform = if self.windows() { "windows" } else { "linux" };
        self.apply(SteamcmdStatusDto {
            found: true,
            path: Some(path),
            platform: platform.to_string(),
        });
    }

    // ── the frame ───────────────────────────────────────────────────────────

    /// Take finished answers in; true when something on screen changed.
    pub fn poll(&mut self, ctx: &egui::Context) -> bool {
        let mut changed = false;
        if self.steam_root != self.root_seen {
            self.root_seen.clone_from(&self.steam_root);
            self.root_changed(ctx);
        }
        if self.root_quiet.ready(ctx) {
            self.find_game();
            changed = true;
        }
        if let Some(t) = &mut self.game_task
            && let Some(g) = t.poll()
        {
            self.game_task = None;
            self.game = Some(g);
            changed = true;
        }
        if let Some(t) = &mut self.steam_task
            && let Some(s) = t.poll()
        {
            self.steam_task = None;
            self.steam = Some(s);
            changed = true;
        }
        if let Some(t) = &mut self.me_task
            && let Some(me) = t.poll()
        {
            self.me_task = None;
            if let Some(me) = me {
                if self.player.trim().is_empty() && !me.name.is_empty() {
                    self.player = me.name;
                }
                if self.steam_id.trim().is_empty() {
                    self.steam_id = me.steam_id;
                }
                self.from_steam = true;
            }
            changed = true;
        }
        if let Some(t) = &mut self.detect_task
            && let Some(s) = t.poll()
        {
            self.detect_task = None;
            self.apply(s);
            if !self.found() && !self.via_steam() {
                self.watch();
            }
            changed = true;
        }
        if let Some((t, _)) = &mut self.watch
            && let Some(s) = t.poll()
        {
            self.watch = None;
            if let Some(s) = s
                && !self.via_steam()
            {
                self.apply(s);
            }
            changed = true;
        }
        if let Some(t) = &mut self.download_task
            && let Some(r) = t.poll()
        {
            self.download_task = None;
            match r {
                Ok(path) => self.apply(SteamcmdStatusDto {
                    found: true,
                    path: Some(path),
                    platform: "windows".to_string(),
                }),
                Err(e) => self.download_error = e,
            }
            changed = true;
        }
        if let Some((_, at)) = &self.copied {
            let left = COPIED_FOR.saturating_sub(at.elapsed());
            if left.is_zero() {
                self.copied = None;
                changed = true;
            } else {
                ctx.request_repaint_after(left);
            }
        }
        changed
    }

    /// The wizard closes: nothing keeps looking.
    pub fn close(&mut self) {
        self.stop_watching();
    }
}

impl Drop for Wizard {
    fn drop(&mut self) {
        self.stop_watching();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_round_trip() {
        for (i, s) in Step::ALL.iter().enumerate() {
            assert_eq!(s.index(), i);
            assert_eq!(Step::from_id(s.id()), Some(*s));
            assert!(crate::i18n::has("setup", s.label_key()));
        }
        assert_eq!(Step::at(7), None);
    }
}
