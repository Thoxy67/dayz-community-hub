//! The about view (= `features/about/View.svelte`, `UpdateCard.svelte`): what this launcher
//! is and whether it is current, the machine it runs on (and a copy of it for a bug
//! report), where its data comes from, the keys and the controller's buttons, then the
//! guide, one subject at a time.
//!
//! Files: `hero` (the card at the top and its links), `update` (the updater's card, driven
//! by `store::updater`), `cards` (machine, sources, shortcuts), `guide` (the tabbed guide
//! and the footer). The view's own state (the guide's tab, the shortcuts' mode, the
//! machine's specs) lives in one [`AboutState`] in egui's memory, so the commands of
//! `commands.rs` and the buttons change the same thing.

mod cards;
mod guide;
mod hero;
mod update;

use std::sync::Arc;

use dz_hub::features::system::SystemSpecsDto;
use egui::mutex::Mutex;
use egui::{Id, Rect, Ui, UiBuilder, pos2, vec2};
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::{format, t};
use crate::icons::{Icon, lucide};
use crate::parts::ShortcutMode;
use crate::rt::Fetch;
use crate::store::updater::UpdateState;
use crate::views::View;
use crate::views::news::open_url;

/// This build's version.
pub const VERSION: &str = dz_hub::VERSION;
/// Where the project lives (`links.ts`).
pub const AUTHOR: &str = "Thoxy";
pub const REPO_URL: &str = "https://git.thoxy.xyz/thoxy/dayz-community-hub";
pub const ISSUES_URL: &str = "https://git.thoxy.xyz/thoxy/dayz-community-hub/issues";
pub const RELEASES_URL: &str = "https://git.thoxy.xyz/thoxy/dayz-community-hub/releases";
pub const WIKI_URL: &str = "https://git.thoxy.xyz/thoxy/dayz-community-hub/wiki";
pub const LICENSE_URL: &str =
    "https://git.thoxy.xyz/thoxy/dayz-community-hub/src/branch/master/LICENSE";

/// The page's widest content (`max-w-6xl`) and its side and top air (`px-6 py-5`).
const MAX_W: f32 = 1152.0;
const SIDE: f32 = 24.0;
const TOP: f32 = 20.0;
/// Between the page's blocks (`gap-4`).
const GAP: f32 = 16.0;
/// How long the update card glows when the title bar's badge opened the page.
const GLOW_SECS: f64 = 2.5;

/// One source of the app's data (`SOURCES`): icon, words keys, the site it opens.
pub struct Source {
    pub icon: Icon,
    pub title: &'static str,
    pub desc: &'static str,
    pub url: Option<&'static str>,
}

pub const SOURCES: [Source; 7] = [
    Source {
        icon: lucide::SERVER,
        title: "srcServers",
        desc: "srcServersDesc",
        url: Some("https://dayzsalauncher.com"),
    },
    Source {
        icon: lucide::STAR,
        title: "srcOfficial",
        desc: "srcOfficialDesc",
        url: Some("https://store.steampowered.com/app/221100"),
    },
    Source {
        icon: lucide::RADAR,
        title: "srcLive",
        desc: "srcLiveDesc",
        url: None,
    },
    Source {
        icon: lucide::CHART_LINE,
        title: "srcStats",
        desc: "srcStatsDesc",
        url: Some("https://dayzmetrics.com"),
    },
    Source {
        icon: lucide::PUZZLE,
        title: "srcMods",
        desc: "srcModsDesc",
        url: Some("https://steamcommunity.com/app/221100/workshop/"),
    },
    Source {
        icon: lucide::WIFI_OFF,
        title: "srcOffline",
        desc: "srcOfflineDesc",
        url: Some("https://github.com/Arkensor/DayZCommunityOfflineMode"),
    },
    Source {
        icon: lucide::NEWSPAPER,
        title: "srcNews",
        desc: "srcNewsDesc",
        url: Some("https://dayz.com/news"),
    },
];

/// What the window is built with, quietly at the end (the egui stack now).
pub const STACK: [(&str, &str); 6] = [
    ("Rust", "https://www.rust-lang.org"),
    ("egui", "https://www.egui.rs"),
    ("wgpu", "https://wgpu.rs"),
    ("Tokio", "https://tokio.rs"),
    ("resvg", "https://github.com/linebender/resvg"),
    ("Lucide", "https://lucide.dev"),
];

/// The guide's examples of `dzch://` links: (words key, link).
pub const EXAMPLES: [(&str, &str); 3] = [
    ("sharingBasic", "dzch://1.2.3.4:2302"),
    (
        "sharingWithMods",
        "dzch://1.2.3.4:2302?mods=1559212036,1564026768",
    ),
    (
        "sharingFull",
        "dzch://1.2.3.4:2302?qport=27016&name=My%20Server&password=secret&mods=1559212036",
    ),
];

/// The guide's subjects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Guide {
    #[default]
    Start,
    Mods,
    Share,
    Offline,
    Tips,
}

impl Guide {
    pub const ALL: [Guide; 5] = [
        Guide::Start,
        Guide::Mods,
        Guide::Share,
        Guide::Offline,
        Guide::Tips,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Guide::Start => "start",
            Guide::Mods => "mods",
            Guide::Share => "share",
            Guide::Offline => "offline",
            Guide::Tips => "tips",
        }
    }

    pub fn from_id(id: &str) -> Option<Guide> {
        Guide::ALL.into_iter().find(|g| g.id() == id)
    }
}

/// The view's state, shared with its commands.
#[derive(Default)]
pub struct AboutState {
    pub guide: Guide,
    /// The shortcuts card's tab; `None` until first drawn (then the input in use).
    pub keys_mode: Option<ShortcutMode>,
    pub specs: Fetch<SystemSpecsDto>,
    started: bool,
    /// Until when (egui time) the update card glows.
    pub glow_until: f64,
    /// Bring the update card to the middle of the page on the next frame.
    pub center_update: bool,
}

type Shared = Arc<Mutex<AboutState>>;

pub fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("dzch.about.view"), || {
            Arc::new(Mutex::new(AboutState::default()))
        })
        .clone()
    })
}

fn with<R>(ctx: &egui::Context, f: impl FnOnce(&mut AboutState) -> R) -> R {
    let s = shared(ctx);
    let mut st = s.lock();
    f(&mut st)
}

/// `12 GB`, from megabytes.
pub fn gb(mb: u64) -> String {
    format!("{} GB", format::num((mb as f64 / 1024.0).round()))
}

/// The machine and the app, as a bug report wants them.
pub fn system_info(specs: Option<&SystemSpecsDto>, via_steam: bool) -> String {
    let mut lines = vec![format!("DayZ Community Hub v{VERSION}")];
    if let Some(s) = specs {
        if let Some(os) = s.os.as_deref() {
            lines.push(format!("OS: {os}"));
        }
        lines.push(format!(
            "CPU: {} ({}c/{}t)",
            s.cpu_name.as_deref().unwrap_or("?"),
            s.physical_cores,
            s.logical_cores
        ));
        lines.push(format!("RAM: {}", gb(s.total_memory_mb)));
        for g in s.gpus.iter().take(16) {
            match g.vram_mb {
                Some(v) => lines.push(format!("GPU: {} ({})", g.name, gb(v))),
                None => lines.push(format!("GPU: {}", g.name)),
            }
        }
    }
    let mods = if via_steam {
        "Steam client"
    } else {
        "SteamCMD"
    };
    lines.push(format!("Mods: {mods}"));
    lines.join("\n")
}

/// What a click asks for: a command id and its arguments, run after drawing.
pub type Run = Vec<(&'static str, Value)>;

pub struct AboutView;

impl AboutView {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AboutView {
    fn default() -> Self {
        Self::new()
    }
}

impl View for AboutView {
    fn on_show(&mut self, cx: &mut Ctx) {
        if cx.stores.updater.state == UpdateState::Idle {
            cx.stores.updater.check();
        }
        // Opened from the title bar's update badge: bring the update card to the eye.
        let glow = cx.nav.take_focus().is_some_and(|f| f == "update");
        let now = cx.egui.input(|i| i.time);
        let hub = cx.hub.clone();
        with(cx.egui, |st| {
            if !std::mem::replace(&mut st.started, true) {
                st.specs.load(async move { hub.get_system_specs().await });
            }
            if glow {
                st.glow_until = now + GLOW_SECS;
                st.center_update = true;
            }
        });
    }

    fn ui(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let shared = shared(ui.ctx());
        let mut run: Run = Vec::new();
        {
            let mut st = shared.lock();
            st.specs.poll();
            draw(ui, cx, &mut st, &mut run);
        }
        for (id, args) in run {
            if let Err(e) = cx.run(id, &args) {
                cx.stores.bus.err(e);
            }
        }
    }
}

fn draw(ui: &mut Ui, cx: &mut Ctx, st: &mut AboutState, run: &mut Run) {
    egui::ScrollArea::vertical()
        .id_salt("dzch.about.scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let full = ui.available_width();
            let boxed = full.min(MAX_W);
            let w = (boxed - 2.0 * SIDE).max(240.0);
            let left = ui.cursor().left() + ((full - boxed) / 2.0).max(0.0) + SIDE;
            ui.add_space(TOP);
            let top = ui.cursor().top();
            let rect = Rect::from_min_size(pos2(left, top), vec2(w, f32::INFINITY));
            ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                ui.set_width(w);
                ui.spacing_mut().item_spacing = vec2(0.0, GAP);
                hero::show(ui, run);
                update::show(ui, cx, st, run);
                cards::show(ui, cx, st, run);
                guide::show(ui, st);
                guide::footer(ui, run);
            });
            ui.add_space(TOP);
        });
}

// ── commands ────────────────────────────────────────────────────────────────

fn arg<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

fn index(a: &Value) -> Option<usize> {
    a.get("index")
        .and_then(Value::as_u64)
        .and_then(|i| usize::try_from(i).ok())
}

pub fn cmd_open_repo(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, REPO_URL);
}

pub fn cmd_open_releases(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, RELEASES_URL);
}

pub fn cmd_report_bug(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, ISSUES_URL);
}

pub fn cmd_open_wiki(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, WIKI_URL);
}

pub fn cmd_open_license(cx: &mut Ctx, _: &Value) {
    open_url(&cx.stores.bus, LICENSE_URL);
}

/// `{"url": …}` or `{"index": n}`: one of the data sources' sites, nothing else.
pub fn cmd_open_source_link(cx: &mut Ctx, a: &Value) {
    let url = match (arg(a, "url"), index(a)) {
        (Some(u), _) => SOURCES.iter().filter_map(|s| s.url).find(|x| *x == u),
        (None, Some(i)) => SOURCES.get(i).and_then(|s| s.url),
        _ => None,
    };
    match url {
        Some(u) => open_url(&cx.stores.bus, u),
        None => cx.stores.bus.err("unknown source"),
    }
}

/// `{"name": "egui"}`, `{"url": …}` or `{"index": n}`: one of the stack's sites.
pub fn cmd_open_stack_link(cx: &mut Ctx, a: &Value) {
    let found = STACK.iter().enumerate().find(|(i, (name, url))| {
        arg(a, "name").is_some_and(|n| n.eq_ignore_ascii_case(name))
            || arg(a, "url") == Some(*url)
            || index(a) == Some(*i)
    });
    match found {
        Some((_, (_, url))) => open_url(&cx.stores.bus, url),
        None => cx.stores.bus.err("unknown link"),
    }
}

pub fn cmd_copy_system_info(cx: &mut Ctx, _: &Value) {
    let via = cx.stores.profile.via_steam();
    let text = with(cx.egui, |st| system_info(st.specs.value.as_ref(), via));
    cx.egui.copy_text(text);
    cx.stores.bus.ok(t("about", "infoCopied"));
}

/// `{"mode": "keys" | "pad"}`; without one, the other tab.
pub fn cmd_shortcuts_mode(cx: &mut Ctx, a: &Value) {
    let pick = match arg(a, "mode") {
        Some("keys") => Some(ShortcutMode::Keys),
        Some("pad") => Some(ShortcutMode::Pad),
        _ => None,
    };
    with(cx.egui, |st| {
        st.keys_mode = Some(pick.unwrap_or(match st.keys_mode {
            Some(ShortcutMode::Pad) => ShortcutMode::Keys,
            _ => ShortcutMode::Pad,
        }));
    });
}

/// `{"tab": "start" | "mods" | "share" | "offline" | "tips"}`; without one, the next.
pub fn cmd_guide_tab(cx: &mut Ctx, a: &Value) {
    let pick = arg(a, "tab").and_then(Guide::from_id);
    with(cx.egui, |st| {
        st.guide = pick.unwrap_or_else(|| {
            let i = Guide::ALL.iter().position(|g| *g == st.guide).unwrap_or(0);
            Guide::ALL
                .get((i + 1) % Guide::ALL.len())
                .copied()
                .unwrap_or_default()
        });
    });
}

/// `{"index": n}` or `{"url": …}`: one of the guide's example links, to the clipboard.
pub fn cmd_copy_example(cx: &mut Ctx, a: &Value) {
    let found = match (arg(a, "url"), index(a)) {
        (Some(u), _) => EXAMPLES.iter().map(|e| e.1).find(|x| *x == u),
        (None, Some(i)) => EXAMPLES.get(i).map(|e| e.1),
        (None, None) => EXAMPLES.first().map(|e| e.1),
    };
    match found {
        Some(u) => {
            cx.egui.copy_text(u.to_owned());
            cx.stores.bus.ok(t("common", "copied"));
        }
        None => cx.stores.bus.err("unknown example"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dz_hub::features::system::GpuDto;

    #[test]
    fn the_report_lists_the_machine() {
        let specs = SystemSpecsDto {
            logical_cores: 16,
            physical_cores: 8,
            total_memory_mb: 32_768,
            cpu_name: None,
            os: Some("Linux".into()),
            gpus: vec![GpuDto {
                name: "GPU".into(),
                vram_mb: Some(12_282),
            }],
        };
        let s = system_info(Some(&specs), false);
        assert!(s.starts_with("DayZ Community Hub v"));
        assert!(s.contains("OS: Linux"));
        assert!(s.contains("CPU: ? (8c/16t)"));
        assert!(s.contains("RAM: 32 GB"));
        assert!(s.contains("GPU: GPU (12 GB)"));
        assert!(s.ends_with("Mods: SteamCMD"));
        assert!(system_info(None, true).ends_with("Mods: Steam client"));
    }

    #[test]
    fn every_word_exists() {
        for s in &SOURCES {
            assert!(crate::i18n::has("about", s.title));
            assert!(crate::i18n::has("about", s.desc));
        }
        for (k, _) in EXAMPLES {
            assert!(crate::i18n::has("about", k));
        }
        for g in Guide::ALL {
            assert_eq!(Guide::from_id(g.id()), Some(g));
        }
    }
}
