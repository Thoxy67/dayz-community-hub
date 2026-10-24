//! Settings (= `src/features/settings`): every setting on one page, with its own index on
//! the left that scrolls to a section and follows the scroll. The account fields
//! (identity, Steam, API keys, location) are edited together and saved together through
//! the unsaved bar; everything else (downloader, ping, theme, language, controller) takes
//! effect as it is changed.
//!
//! The window frame section (corner radius, border) is not ported: egui draws a square,
//! opaque window (`plan/architecture.md`, Theme). The theme editor is the simplified one:
//! presets, the follow-system pair, custom themes with twelve egui colour pickers.
//!
//! Commands (`settings.*`, a few `theme.*`, `battleye.repair_thorough`) are registered in
//! `commands.rs` and land in [`command`]: what a store can do alone is done there; what
//! needs the page's own state (the form, a detection in flight) is queued with
//! [`request`] and taken in on the page's next frame.

mod derive;
mod form;
pub mod integrations;
mod life;
mod look;
mod sections;

use std::sync::Arc;

use dz_hub::HubEvent;
use egui::mutex::Mutex;
use egui::{
    Align, Color32, CornerRadius, Frame, Id, Layout, Margin, Pos2, Rect, Response, RichText,
    ScrollArea, Sense, Shape, Stroke, Ui, UiBuilder, Vec2,
};
use serde_json::Value;

use super::{View, ViewId};
use crate::app::{Ctx, reduced_motion};
use crate::i18n::{t, tf};
use crate::icons::{Icon, lucide};
use crate::kit::paint::{self, alpha};
use crate::kit::{Button, Variant};
use crate::parts::{NavEntry, PageHeader, SectionNav};
use crate::rt::{self, Fetch, Task};
use crate::store::ConfirmRequest;
use crate::store::profile::PingSettings;
use crate::theme::{TokensExt, font, fonts::Face, size};

pub use derive::derive;

/// The sections, in the page's order: id, words (dictionary, key).
const SECTIONS: [(&str, &str, &str); 13] = [
    ("account", "settings", "sectionAccount"),
    ("steam", "settings", "sectionSteam"),
    ("apis", "settings", "sectionApis"),
    ("location", "settings", "sectionLocation"),
    ("ping", "settings", "sectionPing"),
    ("excluded", "settings", "sectionExcluded"),
    ("appearance", "settings", "sectionAppearance"),
    ("language", "settings", "sectionLanguage"),
    ("window", "settings", "sectionWindow"),
    ("controller", "pad", "section"),
    ("battleye", "settings", "sectionBattleye"),
    ("discord", "settings", "sectionDiscord"),
    ("data", "settings", "sectionData"),
];

/// The sections the account form covers: a dot in the index while it has edits.
const FORM: [&str; 4] = ["account", "steam", "apis", "location"];

/// Room kept above a section scrolled to (`scroll-mt-4`).
const SCROLL_MARGIN: f32 = 16.0;
/// How long a smooth scroll to a section takes.
const SCROLL_SECS: f64 = 0.35;
/// The page's column (`max-w-4xl`).
const COLUMN: f32 = 896.0;
/// Below this width of the page's body (the window less the rail) the index gives its
/// room to the fields.
const INDEX_MIN: f32 = 900.0;

// ── requests (commands that need the page's state) ──────────────────────────

/// An account field, for the commands that type into one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldId {
    Player,
    SteamLogin,
    SteamRoot,
    SteamcmdPath,
    ApiKey,
    SteamId,
}

/// What a command asks of the page.
#[derive(Clone, Debug)]
pub enum Req {
    Goto(String, bool),
    Save,
    Discard,
    Set(FieldId, String),
    Pick(FieldId),
    DetectSteamcmd,
    InstallSteamcmd,
    TestSteamworks,
    DetectSteamId,
    OpenMaps,
    ClearLocation,
    DetectLocation,
    ApplyLocation(Option<(String, String)>),
}

type Queue = Arc<Mutex<Vec<Req>>>;

fn queue(ctx: &egui::Context) -> Queue {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Queue>(Id::new("dzch.settings.queue"))
            .clone()
    })
}

/// Ask the page for something; it is done on its next frame.
pub fn request(ctx: &egui::Context, r: Req) {
    let q = queue(ctx);
    let mut q = q.lock();
    if q.len() < 64 {
        q.push(r);
    }
    drop(q);
    ctx.request_repaint();
}

fn arg<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn arg_num(a: &Value, k: &str) -> Option<f64> {
    a.get(k)
        .and_then(|v| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()))
        .filter(|v| v.is_finite())
}

/// The ping settings as the profile has them (with the defaults).
pub fn ping_of(cx_profile: &crate::store::profile::Profile) -> PingSettings {
    cx_profile.data.as_ref().map_or(
        PingSettings {
            concurrency: 64,
            timeout_auto: 2000,
            timeout_manual: 10_000,
            max_retries: 0,
            scan_favorites: true,
            scan_history: true,
            scan_servers: true,
        },
        |p| PingSettings {
            concurrency: p.ping_concurrency,
            timeout_auto: p.ping_timeout_auto,
            timeout_manual: p.ping_timeout_manual,
            max_retries: p.ping_max_retries,
            scan_favorites: p.ping_scan_favorites,
            scan_history: p.ping_scan_history,
            scan_servers: p.ping_scan_servers,
        },
    )
}

/// A setting's command (`commands.rs` registers each id and calls this with its tail:
/// `settings.ping.set_concurrency` → `ping.set_concurrency`). Arguments are hostile.
pub fn command(cx: &mut Ctx, what: &str, a: &Value) {
    let field = |k: &str| arg(a, k).map(|s| s.chars().take(512).collect::<String>());
    match what {
        "goto_section" => {
            let Some(id) = arg(a, "id").filter(|id| SECTIONS.iter().any(|s| s.0 == *id)) else {
                return;
            };
            if cx.nav.view == ViewId::Settings {
                let smooth = a.get("smooth").and_then(Value::as_bool).unwrap_or(true);
                request(cx.egui, Req::Goto(id.to_string(), smooth));
            } else {
                cx.go_focus(ViewId::Settings, id);
            }
        }
        "account.save" => request(cx.egui, Req::Save),
        "account.discard" => request(cx.egui, Req::Discard),
        "account.edit_name" => {
            if let Some(v) = field("name") {
                request(cx.egui, Req::Set(FieldId::Player, v));
            }
        }
        "steam.edit_login" => {
            if let Some(v) = field("login") {
                request(cx.egui, Req::Set(FieldId::SteamLogin, v));
            }
        }
        "apis.edit_key" => {
            if let Some(v) = field("key") {
                request(cx.egui, Req::Set(FieldId::ApiKey, v));
            }
        }
        "apis.edit_steam_id" => {
            if let Some(v) = field("id") {
                request(cx.egui, Req::Set(FieldId::SteamId, v));
            }
        }
        "steam.pick_steam_root" | "steam.pick_steamcmd" => {
            let id = if what.ends_with("root") {
                FieldId::SteamRoot
            } else {
                FieldId::SteamcmdPath
            };
            match field("path") {
                Some(p) => request(cx.egui, Req::Set(id, p)),
                None => request(cx.egui, Req::Pick(id)),
            }
        }
        "steam.set_downloader" => {
            use dz_hub::features::steamworks::ModDownloaderDto as D;
            let d = match arg(a, "downloader") {
                Some("steamcmd") => D::Steamcmd,
                Some("steamworks") => D::Steamworks,
                _ => return,
            };
            if cx.stores.mods.busy() {
                return;
            }
            // On a Steam Deck the Steam client is the way (Game Mode included):
            // leaving it for SteamCMD is asked first, unless the caller already did.
            let from_steam = cx
                .stores
                .profile
                .data
                .as_ref()
                .is_some_and(|p| p.mod_downloader == D::Steamworks);
            let confirmed = a.get("confirmed").and_then(Value::as_bool) == Some(true);
            if d == D::Steamcmd && from_steam && cx.stores.pad.steam_deck && !confirmed {
                let req = crate::store::dialogs::ConfirmRequest::new(
                    t("settings", "steamcmdOnDeckTitle"),
                    t("settings", "steamcmdOnDeckMessage"),
                )
                .confirm_label(t("settings", "steamcmdOnDeckConfirm"));
                cx.stores.bus.confirm_then(req, |cx| {
                    if !cx.stores.mods.busy() {
                        cx.stores.profile.set_mod_downloader(D::Steamcmd);
                    }
                });
                return;
            }
            cx.stores.profile.set_mod_downloader(d);
        }
        "steam.toggle_steam_server_list" => {
            let now = cx
                .stores
                .profile
                .data
                .as_ref()
                .is_none_or(|p| p.steam_server_list);
            let on = a.get("on").and_then(Value::as_bool).unwrap_or(!now);
            cx.stores.profile.set_steam_server_list(on);
        }
        "steam.open_steamcmd_dir" => {
            let hub = cx.hub.clone();
            rt::fire(async move {
                if let Ok(p) = hub.open_steamcmd_dir().await {
                    let _ = crate::platform::deck::open(p);
                }
            });
        }
        "steam.detect_steamcmd" => request(cx.egui, Req::DetectSteamcmd),
        "steam.install_steamcmd" => request(cx.egui, Req::InstallSteamcmd),
        "steam.test_steamworks" => request(cx.egui, Req::TestSteamworks),
        "apis.detect_steam_id" => request(cx.egui, Req::DetectSteamId),
        "location.open_maps" => request(cx.egui, Req::OpenMaps),
        "location.clear" => request(cx.egui, Req::ClearLocation),
        "location.detect_ip" => request(cx.egui, Req::DetectLocation),
        "location.apply_manual" => {
            let typed = match (arg_num(a, "lat"), arg_num(a, "lon")) {
                (Some(la), Some(lo)) => Some((la.to_string(), lo.to_string())),
                _ => None,
            };
            request(cx.egui, Req::ApplyLocation(typed));
        }
        "ping.set_concurrency"
        | "ping.set_timeout_auto"
        | "ping.set_timeout_manual"
        | "ping.set_retries" => {
            let Some(v) = arg_num(a, "value") else {
                return;
            };
            let mut p = ping_of(&cx.stores.profile);
            let v = v.round();
            match what {
                "ping.set_concurrency" => p.concurrency = v.clamp(5.0, 100.0) as u32,
                "ping.set_timeout_auto" => p.timeout_auto = v.clamp(1000.0, 5000.0) as u32,
                "ping.set_timeout_manual" => p.timeout_manual = v.clamp(1000.0, 30_000.0) as u32,
                _ => p.max_retries = v.clamp(0.0, 5.0) as u32,
            }
            cx.stores.profile.save_ping(p);
        }
        "ping.toggle_scope" => {
            let mut p = ping_of(&cx.stores.profile);
            let slot = match arg(a, "scope") {
                Some("favorites") => &mut p.scan_favorites,
                Some("history") => &mut p.scan_history,
                Some("servers") => &mut p.scan_servers,
                _ => return,
            };
            *slot = a.get("on").and_then(Value::as_bool).unwrap_or(!*slot);
            cx.stores.profile.save_ping(p);
        }
        "excluded.remove_ip" => {
            if let Some(ip) = arg(a, "ip") {
                cx.stores.profile.unexclude_ip(ip);
            }
        }
        _ => {}
    }
}

/// `theme.derive`: every colour of the worn custom theme (a copy of what is worn if none
/// is) made again from its floor and accent.
pub fn cmd_theme_derive(cx: &mut Ctx, _: &Value) {
    let w = cx.stores.theme.current(cx.prefs);
    let (scheme, raw) = derive(w.raw.bg, w.raw.accent);
    cx.stores.theme.set_custom(cx.prefs, scheme, raw, None);
}

/// `theme.import_file`: a theme file the player picks (or `{"path"}`), added and worn.
pub fn cmd_theme_import(cx: &mut Ctx, a: &Value) {
    let bus = cx.stores.bus.clone();
    let given = arg(a, "path").map(std::path::PathBuf::from);
    rt::fire(async move {
        let path = match given {
            Some(p) => p,
            None => {
                let pick = crate::parts::file_picker::Pick::file(t("theme", "import"))
                    .filter("JSON", &["json"]);
                let Some(f) = crate::parts::file_picker::pick(pick).await else {
                    return;
                };
                f
            }
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let lower = name.to_lowercase();
        let cut = [".dzch-theme.json", ".json"]
            .iter()
            .find_map(|s| lower.strip_suffix(s).map(str::len))
            .unwrap_or(name.len());
        let fallback = name
            .get(..cut)
            .filter(|n| !n.is_empty())
            .map_or_else(|| t("theme", "untitled").to_string(), str::to_string);
        // A theme is a few hundred bytes; anything far larger is not one.
        let text = match tokio::fs::metadata(&path).await {
            Ok(m) if m.len() <= 256 * 1024 => tokio::fs::read_to_string(&path).await.ok(),
            _ => None,
        };
        let b = bus.clone();
        bus.run(move |cx| {
            let ok =
                text.is_some_and(|text| cx.stores.theme.import_json(cx.prefs, &text, &fallback));
            if ok {
                b.ok(t("theme", "imported"));
            } else {
                b.err(t("theme", "importError"));
            }
        });
    });
}

/// `theme.export_file`: the worn theme (or `{"id"}`) saved as `<name>.dzch-theme.json`.
pub fn cmd_theme_export(cx: &mut Ctx, a: &Value) {
    let id = arg(a, "id");
    let json = cx.stores.theme.export_json(cx.prefs, id);
    let name = match id {
        Some(id) => cx.stores.theme.look(cx.prefs, Some(id)).name,
        None => cx.stores.theme.current(cx.prefs).name,
    };
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = if slug.trim_matches('-').is_empty() {
        "theme".to_string()
    } else {
        slug
    };
    let bus = cx.stores.bus.clone();
    let given = arg(a, "path").map(std::path::PathBuf::from);
    rt::fire(async move {
        let path = match given {
            Some(p) => p,
            None => {
                let pick = crate::parts::file_picker::Pick::save(
                    t("theme", "export"),
                    format!("{slug}.dzch-theme.json"),
                )
                .filter("JSON", &["json"]);
                let Some(f) = crate::parts::file_picker::pick(pick).await else {
                    return;
                };
                f
            }
        };
        match tokio::fs::write(&path, json).await {
            Ok(()) => bus.ok(t("theme", "exported")),
            Err(e) => bus.err(format!("{}: {e}", t("shell", "errorFailed"))),
        }
    });
}

/// `battleye.repair_thorough`: the game's data outside its folder deleted, then BattlEye
/// reinstalled, after a confirmation that lists those folders.
pub fn cmd_battleye_thorough(cx: &mut Ctx, _: &Value) {
    if cx.stores.battleye.repairing {
        return;
    }
    let hub = cx.hub.clone();
    let bus = cx.stores.bus.clone();
    rt::fire(async move {
        // Asked again by the repair itself when this fails.
        let dirs = hub.game_data_dirs().await.unwrap_or_default();
        let message = if dirs.is_empty() {
            t("settings", "battleyeConfirmNoData").to_string()
        } else {
            tf(
                "settings",
                "battleyeConfirmMessage",
                &[("dirs", &dirs.join(", "))],
            )
        };
        let req = ConfirmRequest::new(t("settings", "battleyeConfirmTitle"), message)
            .confirm_label(t("settings", "battleyeConfirmAction"))
            .danger();
        if bus.confirm(req).await {
            bus.run(|cx| cx.stores.battleye.repair(true));
        }
    });
}

/// Whether BattlEye can be repaired now.
pub fn battleye_repairable(cx: &Ctx) -> bool {
    use dz_game::battleye::BattlEyeState;
    !cx.stores.battleye.repairing
        && cx
            .stores
            .battleye
            .status
            .as_ref()
            .is_some_and(|s| s.state != BattlEyeState::NoGame)
}

// ── the page's own state ────────────────────────────────────────────────────

/// SteamCMD as found (the hub's `SteamcmdStatusDto`, copied: its module is private).
#[derive(Clone, Debug, Default)]
pub struct SteamcmdStatus {
    pub found: bool,
    pub path: Option<String>,
    pub platform: String,
}

/// A place the IP lookup found.
#[derive(Clone, Debug)]
pub struct Place {
    pub city: String,
    pub country: String,
    pub code: String,
}

/// A smooth scroll in flight: from, to, started (egui time).
#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    to: f32,
    t0: f64,
}

pub struct SettingsView {
    pub(crate) form: form::AccountForm,
    // Steam & SteamCMD
    pub(crate) steamcmd: Fetch<SteamcmdStatus>,
    pub(crate) install: Option<Task<Result<String, String>>>,
    pub(crate) install_error: String,
    pub(crate) content_dir: Fetch<String>,
    pub(crate) steamworks: Fetch<dz_hub::features::steamworks::SteamworksStatusDto>,
    /// The downloader the Steamworks status was read for.
    sw_for: Option<bool>,
    pub(crate) testing: Option<Task<Result<(), String>>>,
    // API keys
    pub(crate) detect_id: Option<Task<Result<String, String>>>,
    // Location
    pub(crate) locating: Option<Task<Result<dz_hub::features::native::GeoLocation, String>>>,
    pub(crate) place: Option<Place>,
    pub(crate) loc_error: String,
    pub(crate) lat: String,
    pub(crate) lon: String,
    loc_shown: Option<Option<(f64, f64)>>,
    // Ping: the values while a slider is held (written when it is let go).
    pub(crate) ping_drag: Option<PingSettings>,
    // Data
    pub(crate) include_mods: bool,
    // Theme editor: the name being typed, for which theme.
    pub(crate) theme_name: Option<(String, String)>,
    // The index
    tops: Vec<(&'static str, f32)>,
    active: &'static str,
    goto: Option<(String, bool)>,
    glide: Option<Glide>,
    offset: f32,
    shown: bool,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            form: form::AccountForm::default(),
            steamcmd: Fetch::default(),
            install: None,
            install_error: String::new(),
            content_dir: Fetch::default(),
            steamworks: Fetch::default(),
            sw_for: None,
            testing: None,
            detect_id: None,
            locating: None,
            place: None,
            loc_error: String::new(),
            lat: String::new(),
            lon: String::new(),
            loc_shown: None,
            ping_drag: None,
            include_mods: true,
            theme_name: None,
            tops: Vec::new(),
            active: "account",
            goto: None,
            glide: None,
            offset: 0.0,
            shown: false,
        }
    }

    fn detect_steamcmd(&mut self, cx: &Ctx) {
        let hub = cx.hub.clone();
        self.steamcmd.load(async move {
            let s = hub.detect_steamcmd().await?;
            Ok(SteamcmdStatus {
                found: s.found,
                path: s.path,
                platform: s.platform,
            })
        });
    }

    fn load_steamworks(&mut self, cx: &Ctx) {
        let hub = cx.hub.clone();
        self.steamworks
            .load(async move { hub.steamworks_status().await });
    }

    /// Requests, finished tasks, the form following the profile.
    fn pump(&mut self, cx: &mut Ctx) {
        let reqs = std::mem::take(&mut *queue(cx.egui).lock());
        for r in reqs {
            self.handle(r, cx);
        }
        self.form.sync(&cx.stores.profile);
        self.steamcmd.poll();
        self.content_dir.poll();
        self.steamworks.poll();
        if let Some(r) = self.install.as_mut().and_then(Task::poll) {
            self.install = None;
            match r {
                Ok(_) => {
                    self.detect_steamcmd(cx);
                    cx.stores.servers.load_stats();
                }
                Err(e) => self.install_error = e,
            }
        }
        if let Some(r) = self.testing.as_mut().and_then(Task::poll) {
            self.testing = None;
            match r {
                Ok(()) => cx.stores.bus.ok(t("settings", "steamworksTestOk")),
                Err(e) => cx.stores.bus.err(e),
            }
            self.load_steamworks(cx);
        }
        if let Some(r) = self.detect_id.as_mut().and_then(Task::poll) {
            self.detect_id = None;
            match r {
                Ok(id) => self.form.f.steam_id = id,
                Err(e) => cx.stores.bus.err(e),
            }
        }
        if let Some(r) = self.locating.as_mut().and_then(Task::poll) {
            self.locating = None;
            match r {
                Ok(g) if g.lat.is_finite() && g.lon.is_finite() => {
                    self.form.f.location = Some((g.lon, g.lat));
                    self.place = Some(Place {
                        city: g.city,
                        country: g.country,
                        code: g.country_code,
                    });
                }
                Ok(_) => self.loc_error = t("settings", "locationInvalid").to_string(),
                Err(e) => self.loc_error = e,
            }
        }
        // The Steam client's status, read when it becomes the downloader.
        let steamworks = cx.stores.profile.via_steam();
        if self.sw_for != Some(steamworks) {
            self.sw_for = Some(steamworks);
            if steamworks {
                self.load_steamworks(cx);
            }
        }
        // The manual fields follow whatever location is set.
        if self.loc_shown != Some(self.form.f.location) {
            self.loc_shown = Some(self.form.f.location);
            match self.form.f.location {
                Some((lo, la)) => {
                    self.lat = format!("{la:.4}");
                    self.lon = format!("{lo:.4}");
                }
                None => {
                    self.lat.clear();
                    self.lon.clear();
                }
            }
        }
    }

    fn handle(&mut self, r: Req, cx: &mut Ctx) {
        match r {
            Req::Goto(id, smooth) => self.goto = Some((id, smooth)),
            Req::Save => self.form.save(&mut cx.stores.profile),
            Req::Discard => self.form.discard(&cx.stores.profile),
            Req::Set(id, v) => *self.field(id) = v,
            Req::Pick(id) => {
                let ctx = cx.egui.clone();
                let (title, folder) = match id {
                    FieldId::SteamRoot => (t("settings", "selectSteamRoot"), true),
                    _ => (t("settings", "selectSteamcmd"), false),
                };
                rt::fire(async move {
                    let pick = if folder {
                        crate::parts::file_picker::Pick::folder(title)
                    } else {
                        crate::parts::file_picker::Pick::file(title)
                    };
                    if let Some(f) = crate::parts::file_picker::pick(pick).await {
                        let p = f.to_string_lossy().into_owned();
                        request(&ctx, Req::Set(id, p));
                    }
                });
            }
            Req::DetectSteamcmd => self.detect_steamcmd(cx),
            Req::InstallSteamcmd => {
                if self.install.is_none() {
                    self.install_error.clear();
                    let hub = cx.hub.clone();
                    self.install = Some(rt::spawn(
                        async move { hub.download_steamcmd_windows().await },
                    ));
                }
            }
            Req::TestSteamworks => {
                if self.testing.is_none() && !cx.stores.mods.busy() {
                    let hub = cx.hub.clone();
                    self.testing = Some(rt::spawn(async move { hub.steamworks_check().await }));
                }
            }
            Req::DetectSteamId => {
                if self.detect_id.is_none() {
                    let hub = cx.hub.clone();
                    self.detect_id = Some(rt::spawn(async move {
                        hub.steam_me().await.map(|m| m.steam_id)
                    }));
                }
            }
            Req::OpenMaps => {
                if let Some((lo, la)) = self.form.f.location {
                    let _ = crate::platform::deck::open(format!(
                        "https://www.google.com/maps?q={la},{lo}"
                    ));
                }
            }
            Req::ClearLocation => {
                self.form.f.location = None;
                self.place = None;
                self.loc_error.clear();
            }
            Req::DetectLocation => {
                if self.locating.is_none() {
                    self.loc_error.clear();
                    let hub = cx.hub.clone();
                    self.locating = Some(rt::spawn(async move { hub.geolocate_ip().await }));
                }
            }
            Req::ApplyLocation(typed) => {
                if let Some((la, lo)) = typed {
                    self.lat = la;
                    self.lon = lo;
                }
                self.apply_location();
            }
        }
    }

    pub(crate) fn field(&mut self, id: FieldId) -> &mut String {
        let f = &mut self.form.f;
        match id {
            FieldId::Player => &mut f.player,
            FieldId::SteamLogin => &mut f.steam_login,
            FieldId::SteamRoot => &mut f.steam_root,
            FieldId::SteamcmdPath => &mut f.steamcmd_path,
            FieldId::ApiKey => &mut f.steam_api_key,
            FieldId::SteamId => &mut f.steam_id,
        }
    }

    /// The typed latitude and longitude, checked.
    pub(crate) fn apply_location(&mut self) {
        let la = self.lat.trim().replace(',', ".").parse::<f64>();
        let lo = self.lon.trim().replace(',', ".").parse::<f64>();
        match (la, lo) {
            (Ok(la), Ok(lo))
                if la.is_finite()
                    && lo.is_finite()
                    && (-90.0..=90.0).contains(&la)
                    && (-180.0..=180.0).contains(&lo) =>
            {
                self.loc_error.clear();
                self.place = None;
                self.form.f.location = Some((lo, la));
            }
            _ => self.loc_error = t("settings", "locationInvalid").to_string(),
        }
    }

    /// The page: header, index, the sections in their scroller, the unsaved bar.
    fn page(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        PageHeader::new(t("nav", "settings")).show(ui, |_| {}, |_| {}, None::<fn(&mut Ui)>);
        let body = ui.available_rect_before_wrap();
        ui.allocate_rect(body, Sense::hover());
        let wide = body.width() >= INDEX_MIN;
        let dirty = self.form.dirty();

        // The index.
        let mut index_w = 0.0;
        if wide {
            index_w = 208.0;
            let entries: Vec<NavEntry<'_>> = SECTIONS
                .iter()
                .map(|(id, d, k)| NavEntry {
                    id,
                    label: t(d, k),
                    dot: dirty && FORM.contains(id),
                })
                .collect();
            let mut nui = ui.new_child(
                UiBuilder::new()
                    .id_salt("settings-index")
                    .max_rect(Rect::from_min_max(
                        body.min,
                        Pos2::new(body.left() + index_w, body.bottom()),
                    ))
                    .layout(Layout::top_down(Align::Min)),
            );
            if let Some(id) =
                SectionNav::new(t("settings", "onThisPage"), &entries, self.active).show(&mut nui)
            {
                let _ = cx.run(
                    "settings.goto_section",
                    &serde_json::json!({ "id": id, "smooth": true }),
                );
            }
        }

        // The scroller.
        let srect = Rect::from_min_max(Pos2::new(body.left() + index_w, body.top()), body.max);
        let now = ui.input(|i| i.time);
        let wheel = ui.input(|i| i.smooth_scroll_delta.y != 0.0);
        if wheel && ui.rect_contains_pointer(srect) {
            self.glide = None;
        }
        if let Some((id, smooth)) = self.goto.clone()
            && let Some(top) = self.tops.iter().find(|(s, _)| *s == id).map(|(_, y)| *y)
        {
            self.goto = None;
            let to = (top - SCROLL_MARGIN).max(0.0);
            if smooth && !reduced_motion(ui.ctx()) {
                self.glide = Some(Glide {
                    from: self.offset,
                    to,
                    t0: now,
                });
            } else {
                self.glide = Some(Glide {
                    from: to,
                    to,
                    t0: now - SCROLL_SECS,
                });
            }
        }
        let mut set = None;
        if let Some(g) = self.glide {
            let p = (((now - g.t0) / SCROLL_SECS) as f32).clamp(0.0, 1.0);
            let e = 1.0 - (1.0 - p).powi(3);
            set = Some(g.from + (g.to - g.from) * e);
            if p >= 1.0 {
                self.glide = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        let mut sui = ui.new_child(
            UiBuilder::new()
                .id_salt("settings-scroller")
                .max_rect(srect)
                .layout(Layout::top_down(Align::Min)),
        );
        sui.set_clip_rect(srect.intersect(ui.clip_rect()));
        let mut area = ScrollArea::vertical()
            .id_salt("settings.scroll")
            .auto_shrink([false, false]);
        if let Some(y) = set {
            area = area.vertical_scroll_offset(y);
        }
        let mut tops: Vec<(&'static str, f32)> = Vec::with_capacity(SECTIONS.len());
        let out = area.show(&mut sui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let origin = ui.cursor().top();
            let full = ui.available_width();
            let pad = if wide { 24.0 } else { 16.0 };
            let col = (full.min(COLUMN) - 2.0 * pad).max(160.0);
            let margin = ((full - col) / 2.0).max(0.0);
            ui.add_space(20.0);
            ui.horizontal_top(|ui| {
                ui.add_space(margin);
                ui.vertical(|ui| {
                    ui.set_width(col);
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    for (i, (id, _, _)) in SECTIONS.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(28.0);
                        }
                        tops.push((id, ui.cursor().top() - origin));
                        self.section(id, ui, cx);
                    }
                });
            });
            // Room for the unsaved bar under the last field (`pb-24`).
            ui.add_space(96.0);
        });
        self.offset = out.state.offset.y;
        self.tops = tops;
        // The section being read: the last whose top passed the scroller's top.
        let screen: Vec<(&str, f32)> = self
            .tops
            .iter()
            .map(|(id, y)| (*id, *y - self.offset))
            .collect();
        if let Some(a) = SectionNav::spy(0.0, &screen)
            && let Some(s) = SECTIONS.iter().find(|s| s.0 == a)
        {
            self.active = s.0;
        }

        // The unsaved bar.
        let shown = paint::ease(ui, Id::new("settings.unsaved"), dirty);
        if shown > 0.0 {
            self.unsaved_bar(ui, cx, srect, shown, &tk);
        }
    }

    fn unsaved_bar(
        &mut self,
        ui: &mut Ui,
        cx: &mut Ctx,
        srect: Rect,
        shown: f32,
        tk: &crate::theme::Tokens,
    ) {
        let w = (srect.width() - 48.0).clamp(200.0, COLUMN);
        let h = 44.0;
        let rise = (1.0 - shown) * 6.0;
        let rect = Rect::from_min_size(
            Pos2::new(srect.center().x - w / 2.0, srect.bottom() - 16.0 - h + rise),
            Vec2::new(w, h),
        );
        let mut bui = ui.new_child(
            UiBuilder::new()
                .id_salt("settings-unsaved")
                .max_rect(rect)
                .layout(Layout::left_to_right(Align::Center)),
        );
        bui.set_opacity(shown);
        let p = bui.painter().clone();
        p.add(paint::shadow_pop().as_shape(rect, CornerRadius::same(size::RADIUS_MD)));
        paint::plate(&p, rect, size::RADIUS_MD, tk.overlay, alpha(tk.warn, 0.5));
        // Clicks on the bar never reach the page under it.
        bui.interact(rect, Id::new("settings-unsaved-bg"), Sense::CLICK);
        bui.add_space(12.0);
        let (ir, _) = bui.allocate_exact_size(Vec2::splat(size::ICON), Sense::hover());
        lucide::TRIANGLE_ALERT.paint(&p, ir, tk.warn);
        bui.add_space(12.0);
        let room = (rect.right() - bui.cursor().left() - 230.0).max(40.0);
        bui.add_sized(
            Vec2::new(room, h),
            egui::Label::new(
                RichText::new(t("settings", "unsaved"))
                    .font(font(Face::Sans, size::TEXT_XS))
                    .color(tk.fg),
            )
            .truncate(),
        );
        bui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 12.0;
            if ui
                .add(
                    Button::new(t("settings", "save"))
                        .icon(lucide::SAVE)
                        .variant(Variant::Accent),
                )
                .clicked()
            {
                let _ = cx.run("settings.account.save", &Value::Null);
            }
            if ui
                .add(
                    Button::new(t("settings", "discard"))
                        .icon(lucide::UNDO_2)
                        .variant(Variant::Ghost),
                )
                .clicked()
            {
                let _ = cx.run("settings.account.discard", &Value::Null);
            }
        });
    }

    fn section(&mut self, id: &str, ui: &mut Ui, cx: &mut Ctx) {
        match id {
            "account" => self.account(ui, cx),
            "steam" => self.steam(ui, cx),
            "apis" => self.apis(ui, cx),
            "location" => self.location(ui, cx),
            "ping" => self.ping(ui, cx),
            "excluded" => self.excluded(ui, cx),
            "appearance" => self.appearance(ui, cx),
            "language" => self.language(ui, cx),
            "window" => self.window_life(ui, cx),
            "controller" => self.controller(ui, cx),
            "battleye" => self.battleye(ui, cx),
            "discord" => self.discord(ui, cx),
            _ => self.data(ui, cx),
        }
    }
}

impl Default for SettingsView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for SettingsView {
    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        self.pump(cx);
        self.page(ui, cx);
    }

    fn on_show(&mut self, cx: &mut Ctx) {
        if !self.shown {
            self.shown = true;
            let hub = cx.hub.clone();
            self.content_dir
                .load(async move { hub.steamcmd_dirs().await.map(|d| d.content) });
        }
        self.detect_steamcmd(cx);
        cx.stores.battleye.refresh();
        // Asked to open at a section (the title bar's account chip, the theme menu…):
        // there at once, no glide across the whole page.
        if let Some(focus) = cx.nav.take_focus() {
            let id = focus.as_str();
            if SECTIONS.iter().any(|s| s.0 == id) {
                self.goto = Some((id.to_string(), false));
            }
        }
    }

    fn on_event(&mut self, _ev: &HubEvent, _cx: &mut Ctx) {}
}

// ── drawing helpers shared by the sections ──────────────────────────────────

/// A line of words.
pub(crate) fn words(ui: &mut Ui, s: &str, face: Face, px: f32, color: Color32) -> Response {
    ui.add(egui::Label::new(RichText::new(s).font(font(face, px)).color(color)).wrap())
}

/// A line of words cut to one line.
pub(crate) fn line(ui: &mut Ui, s: &str, face: Face, px: f32, color: Color32) -> Response {
    ui.add(egui::Label::new(RichText::new(s).font(font(face, px)).color(color)).truncate())
}

/// A row of a section's panel (`px-pad py-…`), a line under it unless it is the last,
/// an optional ground.
pub(crate) fn block<R>(
    ui: &mut Ui,
    py: i8,
    line_under: bool,
    fill: Option<Color32>,
    body: impl FnOnce(&mut Ui) -> R,
) -> R {
    let tk = ui.tokens();
    let w = ui.available_width();
    let slot = ui.painter().add(Shape::Noop);
    // The `border-b` is a pixel of its own under the padding.
    let mut margin = Margin::symmetric(size::PAD as i8, py);
    margin.bottom = py.saturating_add(i8::from(line_under));
    let r = Frame::new().inner_margin(margin).show(ui, |ui| {
        ui.set_width(w - 2.0 * size::PAD);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
        body(ui)
    });
    let rect = r.response.rect;
    if let Some(f) = fill {
        ui.painter()
            .set(slot, Shape::rect_filled(rect, CornerRadius::ZERO, f));
    }
    if line_under {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, alpha(tk.border, 0.6)),
        );
    }
    r.inner
}

/// A line across the panel (`border-t border-border`).
pub(crate) fn rule(ui: &mut Ui) {
    let tk = ui.tokens();
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter()
        .hline(r.x_range(), r.center().y, Stroke::new(1.0, tk.border));
}

/// An icon, `px` square, in the flow.
pub(crate) fn icon(ui: &mut Ui, icon: Icon, px: f32, color: Color32) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(px), Sense::hover());
    icon.paint(ui.painter(), r, color);
    resp
}

/// A link that opens in the browser (`text-2xs text-accent`, an arrow out).
pub(crate) fn link(ui: &mut Ui, label: &str, url: &str) {
    let tk = ui.tokens();
    let g = paint::galley(ui, label, font(Face::Sans, size::TEXT_2XS), tk.accent);
    let w = g.size().x + 4.0 + 12.0;
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, size::CONTROL), Sense::click());
    let p = ui.painter();
    p.galley(
        Pos2::new(r.left(), r.center().y - g.size().y / 2.0),
        g.clone(),
        tk.accent,
    );
    lucide::EXTERNAL_LINK.paint(
        p,
        Rect::from_min_size(
            Pos2::new(r.left() + g.size().x + 4.0, r.center().y - 6.0),
            Vec2::splat(12.0),
        ),
        tk.accent,
    );
    if resp.hovered() {
        p.hline(
            r.left()..=(r.left() + g.size().x),
            r.center().y + g.size().y / 2.0,
            Stroke::new(1.0, tk.accent),
        );
    }
    if paint::keyboard_focus(ui, &resp) {
        paint::focus_ring(p, r, size::RADIUS_XS, &tk);
    }
    if resp
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        let _ = crate::platform::deck::open(url);
    }
}

/// A bordered box around `body` (`rounded-sm border border-border`).
pub(crate) fn boxed<R>(ui: &mut Ui, body: impl FnOnce(&mut Ui) -> R) -> R {
    let tk = ui.tokens();
    let r = Frame::new()
        .stroke(Stroke::new(1.0, tk.border))
        .corner_radius(CornerRadius::same(size::RADIUS_SM))
        .show(ui, body);
    r.inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_has_words() {
        for (id, d, k) in SECTIONS {
            assert!(crate::i18n::has(d, k), "{id}: {d}.{k}");
        }
    }
}
