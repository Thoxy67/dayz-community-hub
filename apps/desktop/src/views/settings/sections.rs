//! The sections other than appearance and language (`look.rs`): account, Steam &
//! SteamCMD, API keys, location, ping, excluded IPs, controller, BattlEye, profile data.

use dz_game::battleye::BattlEyeState;
use dz_hub::features::gamepad::PadAction;
use dz_hub::features::steamworks::ModDownloaderDto;
use egui::{Align, CornerRadius, Layout, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use serde_json::{Value, json};

use super::{SettingsView, block, boxed, icon, line, link, words};
use crate::app::Ctx;
use crate::commands::{self, PAD_GROUPS, SHORTCUTS, Scope};
use crate::i18n::{t, tf};
use crate::icons::lucide;
use crate::kit::paint::{self, alpha};
use crate::kit::{
    Button, Checkbox, Copy, Field, Flag, IconButton, Input, InputKind, Segment, Segmented, Size,
    Slider, Spinner, Switch, Tag, Tone, Variant,
};
use crate::parts::{
    PadButton, PadGlyph, PathInput, SecretInput, SettingsSection, ShortcutGroup, ShortcutRow,
    pad_legend,
};
use crate::prefs::{GlyphChoice, OskChoice, PadMode};
use crate::store::pad::InputMode;
use crate::theme::{TokensExt, fonts::Face, size};

/// A button that runs a command when clicked.
fn run_on(cx: &mut Ctx, clicked: bool, id: &str, args: Value) {
    if clicked {
        let _ = cx.run(id, &args);
    }
}

/// `ms` as seconds: "2 s", "2.25 s".
fn secs(ms: u32) -> String {
    if ms.is_multiple_of(1000) {
        format!("{} s", ms / 1000)
    } else {
        format!("{:.2} s", f64::from(ms) / 1000.0)
    }
}

/// The remaining width, less `keep` for what follows on the line.
fn room(ui: &Ui, keep: f32) -> f32 {
    (ui.available_width() - keep).max(40.0)
}

/// The width a small button takes (label, icon, padding).
fn button_w(ui: &Ui, label: &str, xs: bool) -> f32 {
    let (px, pad) = if xs {
        (size::TEXT_2XS, 12.0)
    } else {
        (size::TEXT_XS, 20.0)
    };
    let tw = paint::galley(
        ui,
        label,
        crate::theme::font(Face::SansMedium, px),
        ui.tokens().fg,
    )
    .size()
    .x;
    tw + 12.0 + 6.0 + pad
}

impl SettingsView {
    // ── account ─────────────────────────────────────────────────────────────

    pub(super) fn account(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let avatar = cx.stores.profile.avatar_url.as_deref().unwrap_or("");
        let avatar_key = cx.stores.profile.avatar_key;
        SettingsSection::new(t("settings", "sectionAccount"))
            .description(t("settings", "identity"))
            .icon(lucide::USER_ROUND)
            .show(
                ui,
                |_| {},
                |ui| {
                    block(ui, 12, true, None, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                            let p = ui.painter();
                            p.circle_filled(r.center(), 24.0, tk.raised);
                            if !crate::parts::image::paint_keyed(
                                p,
                                r,
                                avatar_key,
                                avatar,
                                CornerRadius::same(255),
                            ) {
                                lucide::USER_ROUND.paint(
                                    p,
                                    Rect::from_center_size(r.center(), Vec2::splat(24.0)),
                                    tk.fg_faint,
                                );
                            }
                            p.circle_stroke(r.center(), 23.5, Stroke::new(1.0, tk.border_strong));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.add_space(6.0);
                                let f = &self.form.f;
                                let name = if f.player.trim().is_empty() {
                                    t("settings", "unnamedPlayer").to_string()
                                } else {
                                    f.player.clone()
                                };
                                let g = paint::tracked(
                                    ui,
                                    &name,
                                    crate::theme::font(Face::DisplayBold, size::TEXT_LG),
                                    tk.fg,
                                    0.02,
                                    true,
                                    Some(ui.available_width()),
                                );
                                let (gr, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                                ui.painter().galley(gr.min, g, tk.fg);
                                let sub = if f.steam_login.trim().is_empty() {
                                    t("settings", "noSteamLinked").to_string()
                                } else {
                                    tf("settings", "steamLinked", &[("login", &f.steam_login)])
                                };
                                line(ui, &sub, Face::Mono, size::TEXT_2XS, tk.fg_faint);
                            });
                        });
                    });
                    Field::new(t("settings", "ingameName"))
                        .hint(t("settings", "ingameNameHint"))
                        .last()
                        .show(ui, |ui| {
                            ui.add(
                                Input::new(&mut self.form.f.player)
                                    .placeholder(t("settings", "ingameNamePlaceholder"))
                                    .id_salt("set-player"),
                            );
                        });
                },
            );
    }

    // ── Steam & SteamCMD ────────────────────────────────────────────────────

    pub(super) fn steam(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let detecting = self.steamcmd.loading();
        let status = self.steamcmd.value.clone();
        let busy = cx.stores.mods.busy();
        SettingsSection::new(t("settings", "sectionSteam"))
            .description(t("settings", "steamLoginDesc"))
            .icon(lucide::GAMEPAD_2)
            .show(
                ui,
                |ui| {
                    if detecting {
                        ui.add(Spinner::new().size(14.0).color(tk.fg_muted));
                    } else if let Some(s) = &status {
                        let (tone, i) = if s.found {
                            (Tone::Ok, lucide::CIRCLE_CHECK)
                        } else {
                            (Tone::Err, lucide::CIRCLE_X)
                        };
                        ui.add(Tag::new("SteamCMD").tone(tone).icon(i));
                    }
                },
                |ui| {
                    // What downloads mods.
                    let mut d = cx
                        .stores
                        .profile
                        .data
                        .as_ref()
                        .map_or(ModDownloaderDto::Steamcmd, |p| p.mod_downloader);
                    let steamworks = d == ModDownloaderDto::Steamworks;
                    Field::new(t("settings", "modDownloads"))
                        .hint(t("settings", "modDownloadsHint"))
                        .show(ui, |ui| {
                            let opts = [
                                Segment::new(ModDownloaderDto::Steamcmd, "SteamCMD").disabled(busy),
                                Segment::new(
                                    ModDownloaderDto::Steamworks,
                                    t("settings", "downloaderSteam"),
                                )
                                .disabled(busy),
                            ];
                            if ui.add(Segmented::new(&mut d, &opts)).changed() {
                                let v = if d == ModDownloaderDto::Steamworks {
                                    "steamworks"
                                } else {
                                    "steamcmd"
                                };
                                let _ = cx.run(
                                    "settings.steam.set_downloader",
                                    &json!({ "downloader": v }),
                                );
                            }
                        });
                    block(ui, 10, true, None, |ui| {
                        let explain = if steamworks {
                            "steamworksExplain"
                        } else {
                            "steamcmdExplain"
                        };
                        words(
                            ui,
                            t("settings", explain),
                            Face::Sans,
                            size::TEXT_2XS,
                            tk.fg_muted,
                        );
                        if steamworks {
                            ui.horizontal(|ui| {
                                let sw = self.steamworks.value.as_ref();
                                let test = t("settings", "steamworksTest");
                                let keep = button_w(ui, test, true) + 8.0;
                                ui.allocate_ui_with_layout(
                                    Vec2::new(room(ui, keep), size::CONTROL_SM),
                                    Layout::left_to_right(Align::Center),
                                    |ui| match sw {
                                        None => {
                                            ui.add(Spinner::new().size(14.0).color(tk.fg_muted));
                                        }
                                        Some(s) if !s.library => {
                                            icon(ui, lucide::CIRCLE_X, size::ICON_SM, tk.err);
                                            let e = tf(
                                                "settings",
                                                "steamworksUnavailable",
                                                &[("error", &s.error.clone().unwrap_or_default())],
                                            );
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(e)
                                                        .font(crate::theme::font(
                                                            Face::Sans,
                                                            size::TEXT_XS,
                                                        ))
                                                        .color(tk.err),
                                                )
                                                .truncate()
                                                .selectable(true),
                                            );
                                        }
                                        Some(s) if s.steam_running => {
                                            icon(ui, lucide::CIRCLE_CHECK, size::ICON_SM, tk.ok);
                                            line(
                                                ui,
                                                t("settings", "steamRunning"),
                                                Face::Sans,
                                                size::TEXT_XS,
                                                tk.fg,
                                            );
                                        }
                                        Some(_) => {
                                            icon(ui, lucide::CIRCLE_X, size::ICON_SM, tk.warn);
                                            line(
                                                ui,
                                                t("settings", "steamNotRunning"),
                                                Face::Sans,
                                                size::TEXT_XS,
                                                tk.warn,
                                            );
                                        }
                                    },
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let testing = self.testing.is_some();
                                    let lib = sw.is_some_and(|s| s.library);
                                    let b = Button::new(test)
                                        .icon(lucide::PLUG_ZAP)
                                        .variant(Variant::Ghost)
                                        .size(Size::Xs)
                                        .enabled(!testing && !busy && lib);
                                    let c = ui.add(b).clicked();
                                    if testing {
                                        ui.add(Spinner::new().size(12.0).color(tk.fg_muted));
                                    }
                                    run_on(cx, c, "settings.steam.test_steamworks", Value::Null);
                                });
                            });
                        }
                    });

                    // Steam's own server list.
                    Field::new(t("settings", "steamServerList"))
                        .hint(t("settings", "steamServerListHint"))
                        .show(ui, |ui| {
                            let mut on = cx
                                .stores
                                .profile
                                .data
                                .as_ref()
                                .is_none_or(|p| p.steam_server_list);
                            let changed = boxed(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(Switch::new(&mut on, t("settings", "steamServerListSwitch")))
                                    .changed()
                            });
                            if changed {
                                let _ = cx.run(
                                    "settings.steam.toggle_steam_server_list",
                                    &json!({ "on": on }),
                                );
                            }
                        });

                    Field::new(t("settings", "username"))
                        .hint(t("settings", "usernameHint"))
                        .show(ui, |ui| {
                            ui.add(
                                Input::new(&mut self.form.f.steam_login)
                                    .kind(InputKind::Mono)
                                    .id_salt("set-login"),
                            );
                        });

                    // The login: never a password here, SteamCMD asks once and caches it.
                    let saved = cx
                        .stores
                        .profile
                        .data
                        .as_ref()
                        .and_then(|p| p.steam_login.clone())
                        .unwrap_or_default();
                    let logged = !saved.is_empty()
                        && cx
                            .stores
                            .profile
                            .data
                            .as_ref()
                            .and_then(|p| p.steamcmd_logged_in.as_deref())
                            == Some(saved.as_str());
                    let pending_pw = cx
                        .stores
                        .profile
                        .data
                        .as_ref()
                        .is_some_and(|p| p.has_saved_password);
                    let found = status.as_ref().is_some_and(|s| s.found);
                    let dirty = self.form.dirty();
                    block(ui, 10, true, None, |ui| {
                        ui.horizontal(|ui| {
                            let label = t("settings", "loginButton");
                            let keep = button_w(ui, label, true) + 8.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(room(ui, keep), size::CONTROL_SM),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    if logged {
                                        icon(ui, lucide::CIRCLE_CHECK, size::ICON_SM, tk.ok);
                                        line(
                                            ui,
                                            &tf("settings", "loggedInAs", &[("name", &saved)]),
                                            Face::Sans,
                                            size::TEXT_XS,
                                            tk.fg,
                                        );
                                    } else {
                                        icon(ui, lucide::SHIELD_CHECK, size::ICON_SM, tk.fg_faint);
                                        let k = if saved.is_empty() {
                                            "loginNeedsUsername"
                                        } else {
                                            "notLoggedIn"
                                        };
                                        line(
                                            ui,
                                            t("settings", k),
                                            Face::Sans,
                                            size::TEXT_XS,
                                            tk.fg_muted,
                                        );
                                    }
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let c = ui
                                    .add(
                                        Button::new(label)
                                            .icon(lucide::LOG_IN)
                                            .variant(Variant::Ghost)
                                            .size(Size::Xs)
                                            .enabled(!saved.is_empty() && found && !busy && !dirty),
                                    )
                                    .clicked();
                                run_on(cx, c, "mods.steamcmd_login", Value::Null);
                            });
                        });
                        words(
                            ui,
                            t("settings", "loginOnceHint"),
                            Face::Sans,
                            size::TEXT_2XS,
                            tk.fg_faint,
                        );
                        if pending_pw {
                            words(
                                ui,
                                t("settings", "savedPasswordPending"),
                                Face::Sans,
                                size::TEXT_2XS,
                                tk.warn,
                            );
                        }
                    });

                    Field::new(t("settings", "steamRoot"))
                        .hint(t("settings", "steamRootHint"))
                        .show(ui, |ui| {
                            let r = PathInput::new(&mut self.form.f.steam_root)
                                .placeholder("~/.steam/steam")
                                .id_salt("set-root")
                                .show(ui);
                            run_on(cx, r.browse, "settings.steam.pick_steam_root", Value::Null);
                        });
                    let auto = status
                        .as_ref()
                        .and_then(|s| s.path.clone())
                        .unwrap_or_else(|| "auto".to_string());
                    Field::new(t("settings", "steamcmdPath"))
                        .hint(t("settings", "steamcmdPathHint"))
                        .show(ui, |ui| {
                            let r = PathInput::new(&mut self.form.f.steamcmd_path)
                                .placeholder(&auto)
                                .id_salt("set-steamcmd")
                                .show(ui);
                            run_on(cx, r.browse, "settings.steam.pick_steamcmd", Value::Null);
                        });

                    Field::new(t("settings", "contentDir"))
                        .hint(t("settings", "contentDirHint"))
                        .show(ui, |ui| {
                            let open = t("settings", "openFolder");
                            let keep = button_w(ui, open, true) + 8.0;
                            if let Some(dir) = &self.content_dir.value {
                                let w = room(ui, keep);
                                ui.add(Copy::new(dir).max_width(w).color(ui.tokens().fg_muted));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                // Game Mode has no file manager to show it in.
                                if !crate::platform::deck::folders() {
                                    return;
                                }
                                let c = ui
                                    .add(
                                        Button::new(open)
                                            .icon(lucide::FOLDER_OPEN)
                                            .variant(Variant::Ghost)
                                            .size(Size::Xs),
                                    )
                                    .clicked();
                                run_on(cx, c, "settings.steam.open_steamcmd_dir", Value::Null);
                            });
                        });

                    // What was found, and what to do when nothing was.
                    block(ui, 10, false, None, |ui| {
                        ui.horizontal(|ui| {
                            let again = t("settings", "steamcmdDetect");
                            let keep = button_w(ui, again, true) + 8.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(room(ui, keep), size::CONTROL_SM),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    if found {
                                        icon(ui, lucide::CIRCLE_CHECK, size::ICON_SM, tk.ok);
                                        line(
                                            ui,
                                            t("settings", "steamcmdFound"),
                                            Face::Sans,
                                            size::TEXT_XS,
                                            tk.fg,
                                        );
                                        if let Some(p) =
                                            status.as_ref().and_then(|s| s.path.as_deref())
                                        {
                                            let w = room(ui, 0.0);
                                            ui.add(Copy::new(p).max_width(w));
                                        }
                                    } else {
                                        icon(ui, lucide::CIRCLE_X, size::ICON_SM, tk.err);
                                        line(
                                            ui,
                                            t("settings", "steamcmdMissing"),
                                            Face::Sans,
                                            size::TEXT_XS,
                                            tk.err,
                                        );
                                    }
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let c = ui
                                    .add(
                                        Button::new(again)
                                            .icon(lucide::REFRESH_CW)
                                            .variant(Variant::Ghost)
                                            .size(Size::Xs)
                                            .enabled(!detecting),
                                    )
                                    .clicked();
                                run_on(cx, c, "settings.steam.detect_steamcmd", Value::Null);
                            });
                        });
                        if let Some(s) = status.as_ref().filter(|s| !s.found) {
                            words(
                                ui,
                                &tf(
                                    "about",
                                    "steamcmdDesc",
                                    &[("notFound", &t("about", "steamcmdNotFound"))],
                                ),
                                Face::Sans,
                                size::TEXT_2XS,
                                tk.fg_muted,
                            );
                            if s.platform == "windows" {
                                ui.horizontal(|ui| {
                                    let installing = self.install.is_some();
                                    let (label, ic) = if installing {
                                        (t("settings", "steamcmdInstalling"), lucide::LOADER_CIRCLE)
                                    } else {
                                        (t("settings", "steamcmdInstallWin"), lucide::DOWNLOAD)
                                    };
                                    let c = ui
                                        .add(
                                            Button::new(label)
                                                .icon(ic)
                                                .variant(Variant::Accent)
                                                .enabled(!installing),
                                        )
                                        .clicked();
                                    run_on(cx, c, "settings.steam.install_steamcmd", Value::Null);
                                    line(
                                        ui,
                                        t("about", "steamcmdWinDesc"),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_faint,
                                    );
                                });
                                if !self.install_error.is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&self.install_error)
                                                .font(crate::theme::font(
                                                    Face::Mono,
                                                    size::TEXT_2XS,
                                                ))
                                                .color(tk.err),
                                        )
                                        .wrap()
                                        .selectable(true),
                                    );
                                }
                            } else {
                                words(
                                    ui,
                                    t("settings", "steamcmdInstallLinux"),
                                    Face::Sans,
                                    size::TEXT_2XS,
                                    tk.fg_muted,
                                );
                                for (k, cmd) in [
                                    ("steamcmdLinuxDebian", "sudo apt install steamcmd"),
                                    ("steamcmdLinuxArch", "yay -S steamcmd"),
                                    ("steamcmdLinuxFedora", "sudo dnf install steamcmd"),
                                ] {
                                    ui.horizontal(|ui| {
                                        ui.allocate_ui_with_layout(
                                            Vec2::new(128.0, size::CONTROL_SM),
                                            Layout::left_to_right(Align::Center),
                                            |ui| {
                                                line(
                                                    ui,
                                                    t("about", k),
                                                    Face::Sans,
                                                    size::TEXT_2XS,
                                                    tk.fg_faint,
                                                );
                                            },
                                        );
                                        ui.add_space(4.0);
                                        ui.add(Copy::new(cmd).color(ui.tokens().fg_muted));
                                    });
                                }
                            }
                        }
                    });
                },
            );
    }

    // ── API keys ────────────────────────────────────────────────────────────

    pub(super) fn apis(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let ready = !self.form.f.steam_api_key.is_empty() && !self.form.f.steam_id.is_empty();
        SettingsSection::new(t("settings", "sectionApis"))
            .description(t("about", "apiKeys"))
            .icon(lucide::KEY_ROUND)
            .show(
                ui,
                |_| {},
                |ui| {
                    block(ui, 6, true, Some(alpha(tk.raised, 0.3)), |ui| {
                        ui.horizontal(|ui| {
                            let g = crate::kit::panel::stencil(
                                ui,
                                t("settings", "steamApi"),
                                tk.fg_muted,
                                None,
                            );
                            let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                            ui.painter().galley(r.min, g, tk.fg_muted);
                            // The font has no check mark: the icon stands for "✓".
                            if ready {
                                ui.add(Tag::new("").icon(lucide::CHECK).tone(Tone::Ok));
                            } else {
                                ui.add(Tag::new("—").tone(Tone::Neutral));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                line(
                                    ui,
                                    t("about", "apiSteamHint"),
                                    Face::Sans,
                                    size::TEXT_2XS,
                                    tk.fg_faint,
                                );
                            });
                        });
                    });
                    Field::new(t("settings", "apiKey"))
                        .hint(t("settings", "steamApiHint"))
                        .show(ui, |ui| {
                            let get = t("settings", "getKey");
                            let lw = paint::galley(
                                ui,
                                get,
                                crate::theme::font(Face::Sans, size::TEXT_2XS),
                                tk.accent,
                            )
                            .size()
                            .x + 16.0
                                + 6.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(room(ui, lw), size::CONTROL),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    ui.add(
                                        SecretInput::new(&mut self.form.f.steam_api_key)
                                            .placeholder("XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX")
                                            .id_salt("set-apikey"),
                                    );
                                },
                            );
                            link(ui, get, "https://steamcommunity.com/dev/apikey");
                        });
                    Field::new(t("settings", "steamId"))
                        .hint(t("settings", "steamIdHint"))
                        .last()
                        .show(ui, |ui| {
                            let detect = t("settings", "detectSteamId");
                            let find = t("settings", "findSteamId");
                            let lw = paint::galley(
                                ui,
                                find,
                                crate::theme::font(Face::Sans, size::TEXT_2XS),
                                tk.accent,
                            )
                            .size()
                            .x + 16.0;
                            let keep = lw + button_w(ui, detect, true) + 12.0;
                            ui.add(
                                Input::new(&mut self.form.f.steam_id)
                                    .kind(InputKind::Mono)
                                    .placeholder("76561198000000000")
                                    .width(room(ui, keep))
                                    .id_salt("set-steamid"),
                            );
                            let c = ui
                                .add(
                                    Button::new(detect)
                                        .icon(lucide::SCAN_SEARCH)
                                        .variant(Variant::Ghost)
                                        .size(Size::Xs)
                                        .enabled(self.detect_id.is_none())
                                        .tip(t("settings", "detectSteamIdTitle")),
                                )
                                .clicked();
                            run_on(cx, c, "settings.apis.detect_steam_id", Value::Null);
                            link(ui, find, "https://steamid.io");
                        });
                },
            );
    }

    // ── location ────────────────────────────────────────────────────────────

    pub(super) fn location(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        SettingsSection::new(t("settings", "sectionLocation"))
            .description(t("settings", "locationHelp"))
            .icon(lucide::MAP_PIN)
            .show(
                ui,
                |_| {},
                |ui| {
                    block(ui, 12, true, None, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            match self.form.f.location {
                                Some((lo, la)) => {
                                    let (r, _) =
                                        ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        r.center(),
                                        20.0,
                                        alpha(tk.ok, 0.10),
                                    );
                                    match self.place.as_ref().filter(|p| !p.code.is_empty()) {
                                        Some(p) => {
                                            let code = p.code.to_lowercase();
                                            let mut c = ui.new_child(UiBuilder::new().max_rect(
                                                Rect::from_center_size(
                                                    r.center(),
                                                    Vec2::splat(24.0),
                                                ),
                                            ));
                                            c.add(Flag::new(&code).size(24.0));
                                        }
                                        None => lucide::MAP_PIN.paint(
                                            ui.painter(),
                                            Rect::from_center_size(r.center(), Vec2::splat(20.0)),
                                            tk.ok,
                                        ),
                                    }
                                    let maps = t("settings", "openMaps");
                                    let clear = t("settings", "clearLocation");
                                    let keep = button_w(ui, maps, false)
                                        + button_w(ui, clear, false)
                                        + 16.0;
                                    ui.allocate_ui_with_layout(
                                        Vec2::new(room(ui, keep), 40.0),
                                        Layout::top_down(Align::Min),
                                        |ui| {
                                            ui.spacing_mut().item_spacing.y = 2.0;
                                            ui.add_space(3.0);
                                            let title = match &self.place {
                                                Some(p) => [p.city.as_str(), p.country.as_str()]
                                                    .iter()
                                                    .filter(|s| !s.is_empty())
                                                    .copied()
                                                    .collect::<Vec<_>>()
                                                    .join(", "),
                                                None => t("settings", "locationSet").to_string(),
                                            };
                                            line(
                                                ui,
                                                &title,
                                                Face::SansMedium,
                                                size::TEXT_SM,
                                                tk.fg,
                                            );
                                            let mut sub = format!("{la:.4}, {lo:.4}");
                                            if self.place.is_some() {
                                                sub.push_str(" · ");
                                                sub.push_str(t("settings", "locationDetected"));
                                            }
                                            line(ui, &sub, Face::Mono, size::TEXT_2XS, tk.fg_faint);
                                        },
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        let c = ui
                                            .add(
                                                Button::new(clear)
                                                    .icon(lucide::TRASH_2)
                                                    .variant(Variant::Danger),
                                            )
                                            .clicked();
                                        run_on(cx, c, "settings.location.clear", Value::Null);
                                        let c = ui
                                            .add(
                                                Button::new(maps)
                                                    .icon(lucide::MAP)
                                                    .variant(Variant::Ghost),
                                            )
                                            .clicked();
                                        run_on(cx, c, "settings.location.open_maps", Value::Null);
                                    });
                                }
                                None => {
                                    icon(ui, lucide::MAP_PIN, 20.0, tk.fg_faint);
                                    words(
                                        ui,
                                        t("settings", "locationNone"),
                                        Face::Sans,
                                        size::TEXT_XS,
                                        tk.fg_muted,
                                    );
                                }
                            }
                        });
                    });
                    let err = !self.loc_error.is_empty();
                    block(ui, 10, false, None, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                            let busy = self.locating.is_some();
                            let (label, ic) = if busy {
                                (t("settings", "detecting"), lucide::LOADER_CIRCLE)
                            } else {
                                (t("settings", "autodetectIp"), lucide::CROSSHAIR)
                            };
                            let c = ui
                                .add(
                                    Button::new(label)
                                        .icon(ic)
                                        .variant(Variant::Accent)
                                        .enabled(!busy),
                                )
                                .clicked();
                            run_on(cx, c, "settings.location.detect_ip", Value::Null);
                            let (r, _) = ui
                                .allocate_exact_size(Vec2::new(9.0, size::CONTROL), Sense::hover());
                            ui.painter().vline(
                                r.center().x,
                                (r.center().y - 8.0)..=(r.center().y + 8.0),
                                Stroke::new(1.0, tk.border),
                            );
                            let (mr, _) = ui.allocate_exact_size(
                                Vec2::new(
                                    paint::galley(
                                        ui,
                                        t("settings", "manual"),
                                        crate::theme::font(Face::Sans, size::TEXT_2XS),
                                        tk.fg_faint,
                                    )
                                    .size()
                                    .x,
                                    size::CONTROL,
                                ),
                                Sense::hover(),
                            );
                            let g = paint::galley(
                                ui,
                                t("settings", "manual"),
                                crate::theme::font(Face::Sans, size::TEXT_2XS),
                                tk.fg_faint,
                            );
                            paint::text_at(ui.painter(), &g, mr.left(), mr.center().y, tk.fg_faint);
                            ui.add(
                                Input::new(&mut self.lat)
                                    .kind(InputKind::Mono)
                                    .numeric(true)
                                    .placeholder(t("settings", "lat"))
                                    .width(96.0)
                                    .id_salt("set-lat"),
                            );
                            ui.add(
                                Input::new(&mut self.lon)
                                    .kind(InputKind::Mono)
                                    .numeric(true)
                                    .placeholder(t("settings", "lon"))
                                    .width(96.0)
                                    .id_salt("set-lon"),
                            );
                            let can = !self.lat.trim().is_empty() && !self.lon.trim().is_empty();
                            let c = ui
                                .add(
                                    Button::new(t("settings", "apply"))
                                        .icon(lucide::CHECK)
                                        .enabled(can),
                                )
                                .clicked();
                            run_on(cx, c, "settings.location.apply_manual", Value::Null);
                        });
                    });
                    if err {
                        super::rule(ui);
                        block(ui, 6, false, None, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&self.loc_error)
                                        .font(crate::theme::font(Face::Sans, size::TEXT_2XS))
                                        .color(tk.err),
                                )
                                .wrap()
                                .selectable(true),
                            );
                        });
                    }
                },
            );
    }

    // ── ping ────────────────────────────────────────────────────────────────

    pub(super) fn ping(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let mut v = self
            .ping_drag
            .unwrap_or_else(|| super::ping_of(&cx.stores.profile));
        let mut commit = false;
        let mut dragging = false;
        SettingsSection::new(t("settings", "sectionPing"))
            .description(t("settings", "pingIntro"))
            .icon(lucide::RADAR)
            .show(
                ui,
                |_| {},
                |ui| {
                    // No concurrency row: the shared A2S engine's pace and
                    // in-flight ceiling decide how many servers are asked at
                    // once (see `dz_profile::Profile::ping_concurrency`).
                    let rows: [(&str, &str, f64, f64, f64); 3] = [
                        (
                            "pingTimeoutAuto",
                            "pingTimeoutAutoHint",
                            1000.0,
                            5000.0,
                            250.0,
                        ),
                        (
                            "pingTimeoutManual",
                            "pingTimeoutManualHint",
                            1000.0,
                            30_000.0,
                            500.0,
                        ),
                        ("pingMaxRetries", "pingMaxRetriesHint", 0.0, 5.0, 1.0),
                    ];
                    for (i, (label, hint, min, max, step)) in rows.into_iter().enumerate() {
                        let slot = match i {
                            0 => &mut v.timeout_auto,
                            1 => &mut v.timeout_manual,
                            _ => &mut v.max_retries,
                        };
                        Field::new(t("about", label))
                            .hint(t("settings", hint))
                            .show(ui, |ui| {
                                let mut x = f64::from(*slot);
                                let r = ui.add(
                                    Slider::new(&mut x, min..=max)
                                        .step(step)
                                        .width(room(ui, 70.0))
                                        .label(t("about", label)),
                                );
                                *slot = x.round().clamp(min, max) as u32;
                                if r.dragged() {
                                    dragging = true;
                                }
                                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                                    commit = true;
                                }
                                let shown = match i {
                                    2 if *slot == 0 => t("about", "pingRetryDisabled").to_string(),
                                    2 => slot.to_string(),
                                    _ => secs(*slot),
                                };
                                // As tall as the slider: the value sits on its centre line.
                                ui.allocate_ui_with_layout(
                                    Vec2::new(64.0, r.rect.height()),
                                    Layout::right_to_left(Align::Center),
                                    |ui| line(ui, &shown, Face::Mono, size::TEXT_XS, tk.fg),
                                );
                            });
                    }
                    Field::new(t("about", "pingScanScope"))
                        .hint(t("settings", "pingScopeHint"))
                        .last()
                        .show(ui, |ui| {
                            let w = ui.available_width();
                            let cell = w / 3.0;
                            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::hover());
                            let scopes = [
                                ("favorites", "pingScanFavorites", v.scan_favorites),
                                ("history", "pingScanHistory", v.scan_history),
                                ("servers", "pingScanServers", v.scan_servers),
                            ];
                            for (i, (scope, k, on)) in scopes.into_iter().enumerate() {
                                let cr = Rect::from_min_size(
                                    Pos2::new(r.left() + i as f32 * cell, r.top()),
                                    Vec2::new(cell, r.height()),
                                );
                                let mut c = ui.new_child(
                                    UiBuilder::new()
                                        .max_rect(cr)
                                        .id_salt(("ping-scope", i))
                                        .layout(Layout::top_down(Align::Min)),
                                );
                                let mut on = on;
                                let label = format!("{}. {}", i + 1, t("about", k));
                                if c.add(Switch::new(&mut on, &label)).changed() {
                                    let _ = cx.run(
                                        "settings.ping.toggle_scope",
                                        &json!({ "scope": scope, "on": on }),
                                    );
                                }
                                if i > 0 {
                                    ui.painter().vline(
                                        cr.left() + 0.5,
                                        r.y_range(),
                                        Stroke::new(1.0, tk.border),
                                    );
                                }
                            }
                            ui.painter().rect_stroke(
                                r,
                                CornerRadius::same(size::RADIUS_SM),
                                Stroke::new(1.0, tk.border),
                                egui::StrokeKind::Inside,
                            );
                        });
                },
            );
        if commit {
            self.ping_drag = None;
            cx.stores.profile.save_ping(v);
        } else if dragging {
            self.ping_drag = Some(v);
        }
    }

    // ── excluded IPs ────────────────────────────────────────────────────────

    pub(super) fn excluded(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let ips: Vec<String> = cx
            .stores
            .profile
            .data
            .as_ref()
            .map(|p| p.excluded_ips.clone())
            .unwrap_or_default();
        let n = ips.len();
        let count = if n == 1 {
            tf("settings", "excludedIpsCountOne", &[("count", &1)])
        } else {
            tf("settings", "excludedIpsCountOther", &[("count", &n)])
        };
        SettingsSection::new(t("settings", "sectionExcluded"))
            .description(t("settings", "excludedHint"))
            .icon(lucide::BAN)
            .show(
                ui,
                |ui| {
                    ui.add(Tag::new(&count).tone(if n > 0 { Tone::Warn } else { Tone::Neutral }));
                },
                |ui| {
                    if ips.is_empty() {
                        block(ui, 16, false, None, |ui| {
                            ui.vertical_centered(|ui| {
                                line(
                                    ui,
                                    t("settings", "excludedIpsNone"),
                                    Face::Sans,
                                    size::TEXT_XS,
                                    tk.fg_faint,
                                );
                            });
                        });
                        return;
                    }
                    for (i, ip) in ips.iter().enumerate() {
                        block(ui, 4, i + 1 < n, Some(tk.bg), |ui| {
                            ui.horizontal(|ui| {
                                icon(ui, lucide::BAN, 14.0, tk.err);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(ip)
                                            .font(crate::theme::font(Face::Mono, size::TEXT_XS))
                                            .color(tk.fg),
                                    )
                                    .selectable(true),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let label = tf("settings", "excludedIpsRemove", &[("ip", ip)]);
                                    let c = ui
                                        .add(
                                            IconButton::new(lucide::X, &label)
                                                .size(Size::IconXs)
                                                .variant(Variant::Ghost),
                                        )
                                        .clicked();
                                    run_on(
                                        cx,
                                        c,
                                        "settings.excluded.remove_ip",
                                        json!({ "ip": ip }),
                                    );
                                });
                            });
                        });
                    }
                },
            );
    }

    // ── controller ──────────────────────────────────────────────────────────

    pub(super) fn controller(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let prefs = cx.prefs.get();
        let pad_mode = cx.stores.pad.mode(prefs.ui.pad_mode) == InputMode::Gamepad;
        let kind = cx.stores.pad.kind(prefs.pad_glyphs);
        let connected = !cx.stores.pad.pads.is_empty();
        let mut mode = prefs.ui.pad_mode;
        let mut scale = ((prefs.ui.pad_scale * 100.0).round() as i32).clamp(115, 300);
        let mut glyphs = prefs.pad_glyphs;
        let mut keyboard = prefs.ui.pad_keyboard;
        SettingsSection::new(t("pad", "section"))
            .description(t("pad", "sectionHint"))
            .icon(lucide::GAMEPAD_2)
            .show(
                ui,
                |ui| {
                    let label = if connected {
                        t("pad", "connected")
                    } else {
                        t("pad", "noPad")
                    };
                    ui.add(Tag::new(label).tone(if pad_mode {
                        Tone::Accent
                    } else {
                        Tone::Neutral
                    }));
                },
                |ui| {
                    Field::new(t("pad", "mode"))
                        .hint(t("pad", "modeHint"))
                        .show(ui, |ui| {
                            let opts = [
                                Segment::new(PadMode::Auto, t("pad", "modeAuto")),
                                Segment::new(PadMode::Always, t("pad", "modeAlways")),
                                Segment::new(PadMode::Never, t("pad", "modeNever")),
                            ];
                            if ui.add(Segmented::new(&mut mode, &opts)).changed() {
                                let m = match mode {
                                    PadMode::Auto => "auto",
                                    PadMode::Always => "always",
                                    PadMode::Never => "never",
                                };
                                let _ = cx.run("pad.set_mode", &json!({ "mode": m }));
                            }
                        });
                    Field::new(t("pad", "scale")).show(ui, |ui| {
                        let opts = [
                            // 115 % is what the pad density draws on its own: the least.
                            Segment::new(115, "115 %"),
                            Segment::new(130, "130 %"),
                            Segment::new(150, "150 %"),
                        ];
                        if ui.add(Segmented::new(&mut scale, &opts)).changed() {
                            let _ = cx.run(
                                "pad.set_scale",
                                &json!({ "scale": f64::from(scale) / 100.0 }),
                            );
                        }
                    });
                    Field::new(t("pad", "glyphs"))
                        .hint(t("pad", "glyphsHint"))
                        .show(ui, |ui| {
                            let opts = [
                                Segment::new(GlyphChoice::Auto, t("pad", "modeAuto")),
                                Segment::new(GlyphChoice::Xbox, "Xbox"),
                                Segment::new(GlyphChoice::PlayStation, "PlayStation"),
                                Segment::new(GlyphChoice::Nintendo, "Nintendo"),
                                Segment::new(GlyphChoice::SteamDeck, "Steam Deck"),
                            ];
                            if ui.add(Segmented::new(&mut glyphs, &opts)).changed() {
                                let g = match glyphs {
                                    GlyphChoice::Auto => "auto",
                                    GlyphChoice::Xbox => "xbox",
                                    GlyphChoice::PlayStation => "playStation",
                                    GlyphChoice::Nintendo => "nintendo",
                                    GlyphChoice::SteamDeck => "steamDeck",
                                };
                                let _ = cx.run("pad.set_glyphs", &json!({ "glyphs": g }));
                            }
                        });
                    // The keyboard a text field picked with the pad brings up.
                    Field::new(t("pad", "osk"))
                        .hint(t("pad", "oskHint"))
                        .show(ui, |ui| {
                            let opts = [
                                Segment::new(OskChoice::Auto, t("pad", "modeAuto")),
                                Segment::new(OskChoice::Steam, t("pad", "oskSteam")),
                                Segment::new(OskChoice::Builtin, t("pad", "oskBuiltin")),
                                Segment::new(OskChoice::None, t("pad", "oskNone")),
                            ];
                            if ui.add(Segmented::new(&mut keyboard, &opts)).changed() {
                                let k = match keyboard {
                                    OskChoice::Auto => "auto",
                                    OskChoice::Steam => "steam",
                                    OskChoice::Builtin => "builtin",
                                    OskChoice::None => "none",
                                };
                                let _ = cx.run("pad.set_keyboard", &json!({ "keyboard": k }));
                            }
                        });
                    // The launcher in Steam's library, to start it from Game Mode.
                    // Steam never closes in Game Mode: offered, but only from the desktop.
                    let can_add = !crate::platform::deck::game_mode();
                    let hint = if can_add {
                        "addToSteamHint"
                    } else {
                        "addToSteamGameMode"
                    };
                    Field::new(t("pad", "addToSteam"))
                        .hint(t("pad", hint))
                        .show(ui, |ui| {
                            let b = Button::new(t("pad", "addToSteam"))
                                .icon(lucide::PLUS)
                                .size(Size::Sm)
                                .enabled(can_add);
                            let c = ui.add(b).clicked();
                            run_on(cx, c, "steam.add_shortcut", Value::Null);
                        });
                    Field::new(t("pad", "connected")).show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            let pads = &cx.stores.pad.pads;
                            if pads.is_empty() {
                                let k = if cx.stores.pad.available {
                                    "noPad"
                                } else {
                                    "unavailable"
                                };
                                line(ui, t("pad", k), Face::Sans, size::TEXT_XS, tk.fg_faint);
                            }
                            for d in pads.iter().take(8) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    icon(ui, lucide::GAMEPAD_2, size::ICON_SM, tk.accent);
                                    line(ui, &d.name, Face::Sans, size::TEXT_XS, tk.fg);
                                });
                            }
                            if cx.stores.pad.steam_ui {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    icon(ui, lucide::KEYBOARD, size::ICON_SM, tk.fg_muted);
                                    line(
                                        ui,
                                        t("pad", "steamKeyboard"),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_muted,
                                    );
                                });
                            }
                        });
                    });
                    // The live test: the button behind the last action lights a moment.
                    let lit = cx.stores.pad.last_action.and_then(|(a, at)| {
                        let age = now_secs() - at;
                        (0.0..0.45).contains(&age).then(|| {
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_secs_f64(
                                    (0.45 - age).max(0.0),
                                ));
                            button_of(a)
                        })
                    });
                    Field::new(t("pad", "test"))
                        .hint(t("pad", "testHint"))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                for b in TEST {
                                    let slot = ui.painter().add(egui::Shape::Noop);
                                    let r = ui.add(PadGlyph::new(b, kind));
                                    if lit == Some(b) {
                                        ui.painter().set(
                                            slot,
                                            egui::Shape::rect_filled(
                                                r.rect.expand(2.0),
                                                CornerRadius::same(255),
                                                alpha(tk.accent, 0.4),
                                            ),
                                        );
                                    }
                                }
                            });
                        });
                    Field::new(t("pad", "legend"))
                        .stacked()
                        .last()
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                legend(ui, kind);
                            });
                        });
                },
            );
    }

    // ── BattlEye ────────────────────────────────────────────────────────────

    pub(super) fn battleye(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let st = cx.stores.battleye.status.as_ref();
        let be = st.map_or(BattlEyeState::NoGame, |s| s.state);
        let service = st.is_some_and(|s| s.service);
        let said = match be {
            BattlEyeState::Running => t("shell", "sbBeRunning"),
            BattlEyeState::Disabled => t("shell", "sbBeDisabled"),
            BattlEyeState::Missing if service => t("shell", "sbBeMissing"),
            BattlEyeState::Missing => t("shell", "sbBeMissingProton"),
            BattlEyeState::Ready if service => t("shell", "sbBeReady"),
            BattlEyeState::Ready => t("shell", "sbBeReadyProton"),
            BattlEyeState::NoGame => "",
        };
        let repairing = cx.stores.battleye.repairing;
        SettingsSection::new(t("settings", "sectionBattleye"))
            .description(t("settings", "battleyeHint"))
            .icon(lucide::SHIELD_CHECK)
            .show(
                ui,
                |_| {},
                |ui| {
                    if !said.is_empty() {
                        block(ui, 10, true, None, |ui| {
                            ui.horizontal(|ui| {
                                let dot = match be {
                                    BattlEyeState::Running => tk.ok,
                                    BattlEyeState::Ready => tk.fg_faint,
                                    _ => tk.warn,
                                };
                                let (r, _) =
                                    ui.allocate_exact_size(Vec2::new(6.0, 16.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 3.0, dot);
                                let ink = if matches!(
                                    be,
                                    BattlEyeState::Missing | BattlEyeState::Disabled
                                ) {
                                    tk.warn
                                } else {
                                    tk.fg_muted
                                };
                                words(ui, said, Face::Sans, size::TEXT_XS, ink);
                            });
                        });
                    }
                    let label = t("settings", "battleyeRepair");
                    action_row(
                        ui,
                        label,
                        t("settings", "battleyeRepairDesc"),
                        false,
                        |ui| {
                            if repairing {
                                ui.add(Spinner::new().size(12.0).color(tk.err));
                            }
                            let c = ui
                                .add(
                                    Button::new(label)
                                        .icon(lucide::WRENCH)
                                        .variant(Variant::Danger)
                                        .enabled(!repairing && be != BattlEyeState::NoGame),
                                )
                                .clicked();
                            run_on(cx, c, "battleye.repair_thorough", Value::Null);
                        },
                    );
                },
            );
    }

    // ── profile data ────────────────────────────────────────────────────────

    pub(super) fn data(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        SettingsSection::new(t("settings", "sectionData"))
            .description(t("settings", "dataHint"))
            .icon(lucide::DATABASE)
            .show(
                ui,
                |_| {},
                |ui| {
                    let file = [("file", &".dchub" as &dyn std::fmt::Display)];
                    let export = t("about", "profileExport");
                    let include = &mut self.include_mods;
                    block(ui, 10, true, None, |ui| {
                        ui.horizontal(|ui| {
                            let keep = button_w(ui, export, false) + 12.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(room(ui, keep), size::CONTROL),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    line(ui, export, Face::SansMedium, size::TEXT_XS, tk.fg);
                                    words(
                                        ui,
                                        &tf("about", "profileExportDesc", &file),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_faint,
                                    );
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        let word = t("about", "profileIncludeMods");
                                        ui.add(Checkbox::new(include).label(word));
                                        let r =
                                            line(ui, word, Face::Sans, size::TEXT_2XS, tk.fg_muted)
                                                .interact(Sense::CLICK);
                                        if r.clicked() {
                                            *include = !*include;
                                        }
                                    });
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let c = ui
                                    .add(
                                        Button::new(export)
                                            .icon(lucide::DOWNLOAD)
                                            .variant(Variant::Accent),
                                    )
                                    .clicked();
                                let inc = *include;
                                run_on(cx, c, "profile.export", json!({ "include_mods": inc }));
                            });
                        });
                    });
                    let import = t("about", "profileImport");
                    action_row(
                        ui,
                        import,
                        &tf("about", "profileImportDesc", &file),
                        true,
                        |ui| {
                            let c = ui.add(Button::new(import).icon(lucide::UPLOAD)).clicked();
                            run_on(cx, c, "profile.import", Value::Null);
                        },
                    );
                    let reset = t("about", "profileReset");
                    block(ui, 10, false, Some(alpha(tk.err, 0.05)), |ui| {
                        ui.horizontal(|ui| {
                            let keep = button_w(ui, reset, false) + 12.0;
                            ui.allocate_ui_with_layout(
                                Vec2::new(room(ui, keep), size::CONTROL),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let g = crate::kit::panel::stencil(
                                        ui,
                                        t("settings", "dangerZone"),
                                        tk.err,
                                        None,
                                    );
                                    let (r, _) = ui.allocate_exact_size(g.size(), Sense::hover());
                                    ui.painter().galley(r.min, g, tk.err);
                                    ui.add_space(4.0);
                                    line(ui, reset, Face::SansMedium, size::TEXT_XS, tk.fg);
                                    words(
                                        ui,
                                        t("about", "profileResetDesc"),
                                        Face::Sans,
                                        size::TEXT_2XS,
                                        tk.fg_faint,
                                    );
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let c = ui
                                    .add(
                                        Button::new(reset)
                                            .icon(lucide::ROTATE_CCW)
                                            .variant(Variant::Danger),
                                    )
                                    .clicked();
                                run_on(cx, c, "profile.reset", Value::Null);
                            });
                        });
                    });
                },
            );
    }
}

/// A row with a title, a line under it and an action at the right.
fn action_row(
    ui: &mut Ui,
    title: &str,
    desc: &str,
    line_under: bool,
    action: impl FnOnce(&mut Ui),
) {
    let tk = ui.tokens();
    block(ui, 10, line_under, None, |ui| {
        ui.horizontal(|ui| {
            let keep = button_w(ui, title, false) + 40.0;
            ui.allocate_ui_with_layout(
                Vec2::new(room(ui, keep), size::CONTROL),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    line(ui, title, Face::SansMedium, size::TEXT_XS, tk.fg);
                    words(ui, desc, Face::Sans, size::TEXT_2XS, tk.fg_faint);
                },
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                action(ui);
            });
        });
    });
}

/// The buttons of the live test, in its order.
const TEST: [PadButton; 14] = [
    PadButton::Dpad,
    PadButton::A,
    PadButton::B,
    PadButton::X,
    PadButton::Y,
    PadButton::Lb,
    PadButton::Rb,
    PadButton::Lt,
    PadButton::Rt,
    PadButton::Menu,
    PadButton::View,
    PadButton::L3,
    PadButton::R3,
    PadButton::Rs,
];

/// The button behind an action.
fn button_of(a: PadAction) -> PadButton {
    match a {
        PadAction::Up | PadAction::Down | PadAction::Left | PadAction::Right => PadButton::Dpad,
        PadAction::Accept => PadButton::A,
        PadAction::Back => PadButton::B,
        PadAction::Primary => PadButton::X,
        PadAction::Secondary => PadButton::Y,
        PadAction::Menu => PadButton::Menu,
        PadAction::View => PadButton::View,
        PadAction::PrevTab => PadButton::Lb,
        PadAction::NextTab => PadButton::Rb,
        PadAction::PageUp => PadButton::Lt,
        PadAction::PageDown => PadButton::Rt,
        PadAction::ScrollUp | PadAction::ScrollDown => PadButton::Rs,
        PadAction::LeftStick => PadButton::L3,
        PadAction::RightStick => PadButton::R3,
    }
}

fn pad_button(b: commands::PadButton) -> PadButton {
    use commands::PadButton as B;
    match b {
        B::A => PadButton::A,
        B::B => PadButton::B,
        B::X => PadButton::X,
        B::Y => PadButton::Y,
        B::Lb => PadButton::Lb,
        B::Rb => PadButton::Rb,
        B::Lt => PadButton::Lt,
        B::Rt => PadButton::Rt,
        B::Menu => PadButton::Menu,
        B::View => PadButton::View,
        B::L3 => PadButton::L3,
        B::R3 => PadButton::R3,
        B::Dpad => PadButton::Dpad,
        B::Rs => PadButton::Rs,
    }
}

fn scope_title(s: Scope) -> &'static str {
    match s {
        Scope::Servers => t("nav", "servers"),
        Scope::Connect => t("nav", "connect"),
        Scope::Offline => t("nav", "offline"),
        Scope::Options => t("nav", "options"),
        Scope::Friends => t("nav", "friends"),
        other => t("keys", other.label()),
    }
}

/// What a controller's buttons do, from the one shortcut registry.
fn legend(ui: &mut Ui, kind: crate::parts::PadKind) {
    struct Owned {
        label: &'static str,
        pad: Vec<PadButton>,
        pair: bool,
    }
    let groups: Vec<(Scope, Vec<Owned>)> = PAD_GROUPS
        .iter()
        .map(|s| {
            let rows = SHORTCUTS
                .iter()
                .filter(|x| x.scope == *s && !x.pad.is_empty())
                .map(|x| Owned {
                    label: x.words(),
                    pad: x.pad.iter().copied().map(pad_button).collect(),
                    pair: x.pair,
                })
                .collect();
            (*s, rows)
        })
        .collect();
    let rows: Vec<Vec<ShortcutRow<'_>>> = groups
        .iter()
        .map(|(_, rows)| {
            rows.iter()
                .map(|r| ShortcutRow {
                    label: r.label,
                    keys: &[],
                    pad: &r.pad,
                    pair: r.pair,
                })
                .collect()
        })
        .collect();
    let pad_groups: Vec<ShortcutGroup<'_>> = groups
        .iter()
        .zip(&rows)
        .filter(|(_, rows)| !rows.is_empty())
        .map(|((s, _), rows)| ShortcutGroup {
            title: scope_title(*s),
            views: *s == Scope::Views,
            rows,
        })
        .collect();
    pad_legend(ui, &pad_groups, kind);
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
