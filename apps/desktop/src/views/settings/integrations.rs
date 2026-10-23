//! The Discord section: Rich Presence on or off (off by default), whether it names the
//! server, and where it is (connected, waiting for Discord, unavailable in this build).
//! The presence's words go to the hub in the window's language ([`push_words`]).

use dz_hub::Hub;
use dz_hub::features::discord::PresenceWords;
use egui::{Sense, Ui, Vec2};
use serde_json::{Value, json};

use super::{SettingsView, block, boxed, words};
use crate::app::Ctx;
use crate::i18n::t;
use crate::icons::lucide;
use crate::kit::{Field, Switch};
use crate::parts::SettingsSection;
use crate::theme::{TokensExt, fonts::Face, size};

/// The presence's words in the current language.
pub fn words_now() -> PresenceWords {
    PresenceWords {
        launcher: t("settings", "discordInLauncher").to_string(),
        offline: t("settings", "discordOffline").to_string(),
        playing: t("settings", "discordPlaying").to_string(),
        playing_on: t("settings", "discordPlayingOn").to_string(),
    }
}

/// Hand the hub the presence's words (at start, and when the language changes).
pub fn push_words(hub: &Hub) {
    hub.set_discord_words(words_now());
}

/// (presence on, server named), as the profile has them.
fn current(cx: &Ctx) -> (bool, bool) {
    cx.stores.profile.data.as_ref().map_or((false, true), |p| {
        (p.discord_presence, p.discord_show_server)
    })
}

/// Whether this build can show a presence (an application id, not the invented
/// world): the Discord commands' enabled state.
pub fn available(cx: &Ctx) -> bool {
    cx.hub.discord_status().available
}

/// `settings.discord.toggle_presence {"on"?}`.
pub fn toggle_presence(cx: &mut Ctx, a: &Value) {
    if cx.stores.profile.data.is_none() {
        return;
    }
    let (on, show) = current(cx);
    let on = a.get("on").and_then(Value::as_bool).unwrap_or(!on);
    cx.stores.profile.set_discord(on, show);
}

/// `settings.discord.toggle_show_server {"on"?}`.
pub fn toggle_show_server(cx: &mut Ctx, a: &Value) {
    if cx.stores.profile.data.is_none() {
        return;
    }
    let (on, show) = current(cx);
    let show = a.get("on").and_then(Value::as_bool).unwrap_or(!show);
    cx.stores.profile.set_discord(on, show);
}

impl SettingsView {
    pub(super) fn discord(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let tk = ui.tokens();
        let status = cx.hub.discord_status();
        let (on, show) = current(cx);
        let loaded = cx.stores.profile.data.is_some();
        SettingsSection::new(t("settings", "sectionDiscord"))
            .description(t("settings", "discordHint"))
            .icon(lucide::RADIO)
            .show(
                ui,
                |_| {},
                |ui| {
                    // Where the presence is.
                    let (said, dot) = if !status.available {
                        (t("settings", "discordUnavailable"), tk.warn)
                    } else if !on {
                        ("", tk.fg_faint)
                    } else if status.connected {
                        (t("settings", "discordConnected"), tk.ok)
                    } else {
                        (t("settings", "discordWaiting"), tk.fg_faint)
                    };
                    if !said.is_empty() {
                        block(ui, 10, true, None, |ui| {
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(Vec2::new(6.0, 16.0), Sense::hover());
                                ui.painter().circle_filled(r.center(), 3.0, dot);
                                let ink = if status.available {
                                    tk.fg_muted
                                } else {
                                    tk.warn
                                };
                                words(ui, said, Face::Sans, size::TEXT_XS, ink);
                            });
                        });
                    }
                    Field::new(t("settings", "discordPresence")).show(ui, |ui| {
                        let mut v = on;
                        let changed = boxed(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                Switch::new(&mut v, t("settings", "discordPresenceSwitch"))
                                    .enabled(loaded && status.available),
                            )
                            .changed()
                        });
                        if changed {
                            let _ = cx.run("settings.discord.toggle_presence", &json!({ "on": v }));
                        }
                    });
                    Field::new(t("settings", "discordShowServer"))
                        .hint(t("settings", "discordShowServerHint"))
                        .last()
                        .show(ui, |ui| {
                            let mut v = show;
                            let changed = boxed(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    Switch::new(&mut v, t("settings", "discordShowServerSwitch"))
                                        .enabled(loaded && status.available && on),
                                )
                                .changed()
                            });
                            if changed {
                                let _ = cx.run(
                                    "settings.discord.toggle_show_server",
                                    &json!({ "on": v }),
                                );
                            }
                        });
                },
            );
    }
}
