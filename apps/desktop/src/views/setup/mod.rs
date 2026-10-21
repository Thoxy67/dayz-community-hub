//! The first-launch setup (= `features/setup`): the steps down the left, the one in hand on
//! the right, and a way out at every step. The language comes first, so the rest reads in
//! it; the server list loads behind all of it. Everything asked here can be changed later
//! in Settings.
//!
//! Not a place of the rail: the shell draws it over the window (below the title bar, which
//! stays usable) while `stores.app.show_wizard`, through [`crate::shell::set_wizard_host`].
//! Start-up sets that flag on a first launch (`store/app.rs`, as `App.svelte` did).
//!
//! Files: `wizard` (the state, `wizard.svelte.ts`), `frame` (the aside, the header, the
//! footer, and the welcome, appearance and done steps: `Wizard.svelte`), `steps` (game,
//! downloads, SteamCMD, account, services: `*Step.svelte`).
//!
//! The state lives in egui's memory, so the buttons and the commands of `commands.rs`
//! (`setup.*`: palette, pad, control channel) change the same thing. Buttons only queue
//! command ids while the state is locked; they run once it is released.

mod frame;
mod steps;
pub mod wizard;

use std::sync::Arc;

use dz_hub::features::steamworks::ModDownloaderDto;
use egui::mutex::Mutex;
use serde_json::Value;

use crate::app::Ctx;
use crate::i18n::t;
use crate::store::profile::AccountSettings;
use crate::views::ViewId;

pub use wizard::{Step, Wizard};

/// Commands the drawing queued: `(id, args)`, run once the state is unlocked.
pub type Acts = Vec<(&'static str, Value)>;

type Shared = Arc<Mutex<Option<Wizard>>>;

fn shared(ctx: &egui::Context) -> Shared {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Shared>(egui::Id::new("dzch.setup"))
            .clone()
    })
}

/// Run `f` on the wizard, made (from the profile, if one was read) on first use.
fn with<R>(cx: &mut Ctx, f: impl FnOnce(&mut Wizard) -> R) -> R {
    let st = shared(cx.egui);
    let mut slot = st.lock();
    let w = slot.get_or_insert_with(|| fresh(cx));
    f(w)
}

/// A new wizard; when a profile exists already (the setup opened again), what it holds.
fn fresh(cx: &Ctx) -> Wizard {
    let mut w = Wizard::new(cx.hub.clone());
    if let Some(p) = &cx.stores.profile.data {
        let s = |v: &Option<String>| v.clone().unwrap_or_default();
        w.player = s(&p.player);
        w.steam_login = s(&p.steam_login);
        w.steam_root = s(&p.steam_root);
        w.steamcmd_path = s(&p.steamcmd_path);
        w.steam_api_key = s(&p.steam_api_key);
        w.steam_id = s(&p.steam_id);
        w.downloader = p.mod_downloader;
    }
    w
}

/// Once at start: the shell draws the wizard while it is to be shown.
pub fn install() {
    crate::shell::set_wizard_host(host);
}

/// The wizard, over the window below the title bar.
fn host(ctx: &egui::Context, cx: &mut Ctx) {
    let st = shared(ctx);
    let mut acts = Acts::new();
    {
        let mut slot = st.lock();
        let w = slot.get_or_insert_with(|| fresh(cx));
        w.poll(ctx);
        frame::show(ctx, cx, w, &mut acts);
    }
    for (id, args) in acts {
        let _ = cx.run(id, &args);
    }
}

fn arg<'a>(a: &'a Value, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Value::as_str)
}

/// A typed value from the control channel: bounded.
fn bounded(s: &str) -> String {
    s.chars().take(512).collect()
}

/// Save what was asked, close, and open `view`.
fn finish(cx: &mut Ctx, view: ViewId) {
    let location = cx
        .stores
        .profile
        .data
        .as_ref()
        .and_then(|p| p.user_location);
    let (account, downloader) = with(cx, |w| {
        let blank = |v: &str| {
            let v = v.trim();
            (!v.is_empty()).then(|| v.to_string())
        };
        let account = AccountSettings {
            player: blank(&w.player),
            steam_login: if w.needs_login() {
                blank(&w.steam_login)
            } else {
                None
            },
            // A folder found on its own is found again at every start; only a picked
            // one is worth pinning.
            steam_root: blank(&w.steam_root),
            steamcmd_path: if w.via_steam() {
                None
            } else {
                blank(&w.steamcmd_path)
            },
            steam_api_key: blank(&w.steam_api_key),
            steam_id: blank(&w.steam_id),
            user_location: location,
        };
        w.close();
        (account, w.downloader)
    });
    // The downloader first: the account's write carries the profile as it is then.
    cx.stores.profile.set_mod_downloader(downloader);
    cx.stores.profile.save_account(account);
    cx.stores.app.show_wizard = false;
    *shared(cx.egui).lock() = None;
    cx.go(view);
}

/// Pick a folder or a file with the system's dialog, then hand it to the wizard.
fn pick(cx: &mut Ctx, folder: bool, then: fn(&mut Wizard, String)) {
    let windows = with(cx, |w| w.windows());
    let bus = cx.stores.bus.clone();
    crate::rt::fire(async move {
        let d = rfd::AsyncFileDialog::new();
        let picked = if folder {
            d.set_title(t("setup", "selectSteamRoot"))
                .pick_folder()
                .await
        } else {
            let d = d.set_title(t("setup", "selectSteamcmd"));
            let d = if windows {
                d.add_filter(t("setup", "steamcmdFilter"), &["exe"])
            } else {
                d
            };
            d.pick_file().await
        };
        if let Some(f) = picked {
            let p = f.path().to_string_lossy().into_owned();
            bus.run(move |cx| with(cx, |w| then(w, p)));
        }
    });
}

fn set_root(w: &mut Wizard, p: String) {
    w.steam_root = p;
    w.find_game();
}

fn set_steamcmd(w: &mut Wizard, p: String) {
    w.chose(p);
}

/// Whether the setup is on screen (every `setup.*` command but `setup.open` needs it).
pub fn shown(cx: &Ctx) -> bool {
    cx.stores.app.show_wizard
}

/// The `setup.*` commands (`commands.rs`).
pub fn command(cx: &mut Ctx, what: &str, a: &Value) {
    match what {
        "open" => {
            if !cx.stores.app.show_wizard {
                *shared(cx.egui).lock() = None;
                cx.stores.app.show_wizard = true;
            }
        }
        "next" => {
            if with(cx, |w| w.step) == Step::Done {
                finish(cx, ViewId::Servers);
            } else {
                with(cx, Wizard::next);
            }
        }
        "back" => with(cx, Wizard::back),
        "go_step" => {
            if let Some(s) = arg(a, "step").and_then(Step::from_id) {
                with(cx, |w| {
                    if w.reachable(s.index()) {
                        w.go(s);
                    }
                });
            }
        }
        "choose_downloader" => {
            let d = match arg(a, "downloader") {
                Some("steamcmd") => ModDownloaderDto::Steamcmd,
                Some("steamworks" | "steam") => ModDownloaderDto::Steamworks,
                _ => return,
            };
            with(cx, |w| w.choose(d));
        }
        "pick_steam_library" => match arg(a, "path") {
            Some(p) => with(cx, |w| set_root(w, bounded(p))),
            None => pick(cx, true, set_root),
        },
        "pick_steamcmd" => match arg(a, "path") {
            Some(p) => with(cx, |w| set_steamcmd(w, bounded(p))),
            None => pick(cx, false, set_steamcmd),
        },
        "copy_install_command" => {
            if let Some(c) = arg(a, "command") {
                let c = bounded(c);
                cx.egui.copy_text(c.clone());
                with(cx, |w| w.copied = Some((c, std::time::Instant::now())));
            }
        }
        "download_steamcmd" => with(cx, Wizard::download),
        "rescan_steamcmd" => with(cx, Wizard::detect),
        "rescan_game" => with(cx, Wizard::find_game),
        "check_steam" => with(cx, Wizard::check_steam),
        "edit_login" => {
            if let Some(v) = arg(a, "login") {
                with(cx, |w| w.steam_login = bounded(v));
            }
        }
        "edit_name" => {
            if let Some(v) = arg(a, "name") {
                with(cx, |w| w.player = bounded(v));
            }
        }
        "edit_api" => {
            with(cx, |w| {
                if let Some(v) = arg(a, "key") {
                    w.steam_api_key = bounded(v);
                }
                if let Some(v) = arg(a, "steam_id") {
                    w.steam_id = bounded(v);
                }
            });
        }
        "finish" => {
            let view = match arg(a, "view") {
                Some("mods") => ViewId::Mods,
                Some("favorites") => ViewId::Favorites,
                _ => ViewId::Servers,
            };
            finish(cx, view);
        }
        _ => {}
    }
}
