//! The window section: closing to the tray, what the launcher does when DayZ starts
//! and stops, system notifications (`platform::life`, kept in the preferences).

use egui::Ui;
use serde_json::json;

use super::{SettingsView, boxed};
use crate::app::Ctx;
use crate::i18n::t;
use crate::icons::lucide;
use crate::kit::{Field, Select, SelectOption, Switch, Tag, Tone};
use crate::parts::SettingsSection;
use crate::platform::life::{self, OnGameStart, OnGameStop};

/// The width of both dropdowns, so they line up.
const SELECT_W: f32 = 240.0;

fn start_word(v: OnGameStart) -> (&'static str, &'static str) {
    match v {
        OnGameStart::Nothing => ("nothing", "lifeStartNothing"),
        OnGameStart::Minimize => ("minimize", "lifeStartMinimize"),
        OnGameStart::Tray => ("tray", "lifeStartTray"),
        OnGameStart::Close => ("close", "lifeStartClose"),
    }
}

fn stop_word(v: OnGameStop) -> (&'static str, &'static str) {
    match v {
        OnGameStop::Nothing => ("nothing", "lifeStopNothing"),
        OnGameStop::Restore => ("restore", "lifeStopRestore"),
        OnGameStop::Front => ("front", "lifeStopFront"),
    }
}

impl SettingsView {
    pub(super) fn window_life(&mut self, ui: &mut Ui, cx: &mut Ctx) {
        let p = cx.prefs.get().life;
        let env = life::shared(cx.egui).env();
        SettingsSection::new(t("settings", "sectionWindow"))
            .description(t("settings", "sectionWindowHint"))
            .icon(lucide::APP_WINDOW)
            .show(
                ui,
                |ui| {
                    let (label, tone) = if env.tray {
                        (t("settings", "lifeTrayOn"), Tone::Accent)
                    } else {
                        (t("settings", "lifeTrayOff"), Tone::Neutral)
                    };
                    ui.add(Tag::new(label).tone(tone));
                },
                |ui| {
                    let hint = if !env.tray {
                        t("settings", "lifeNoTrayHint")
                    } else if env.wayland {
                        t("settings", "lifeWaylandHint")
                    } else {
                        t("settings", "lifeCloseToTrayHint")
                    };
                    Field::new(t("settings", "lifeCloseToTray"))
                        .hint(hint)
                        .show(ui, |ui| {
                            // Wayland quits on close whatever the setting
                            // (`life::on_close`): shown off and locked.
                            let can = env.tray && !env.wayland;
                            let mut on = p.close_to_tray && can;
                            let changed = boxed(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    Switch::new(&mut on, t("settings", "lifeCloseToTraySwitch"))
                                        .enabled(can),
                                )
                                .changed()
                            });
                            if changed {
                                let _ =
                                    cx.run("settings.life.set_close_to_tray", &json!({ "on": on }));
                            }
                        });

                    // A choice of hiding to the tray made while there was one reads as
                    // minimising when there is none: shown as such.
                    let choices = life::start_choices(env.tray);
                    let mut start = if choices.contains(&p.on_game_start) {
                        p.on_game_start
                    } else {
                        OnGameStart::Minimize
                    };
                    let opts: Vec<SelectOption<OnGameStart>> = choices
                        .iter()
                        .map(|v| SelectOption::new(*v, t("settings", start_word(*v).1)))
                        .collect();
                    Field::new(t("settings", "lifeOnGameStart"))
                        .hint(t("settings", "lifeOnGameStartHint"))
                        .show(ui, |ui| {
                            let r = ui.add(
                                Select::new("settings.life.start", &mut start, &opts)
                                    .width(SELECT_W)
                                    .label(t("settings", "lifeOnGameStart")),
                            );
                            if r.changed() {
                                let _ = cx.run(
                                    "settings.life.set_on_game_start",
                                    &json!({ "action": start_word(start).0 }),
                                );
                            }
                        });

                    let mut stop = p.on_game_stop;
                    let opts: Vec<SelectOption<OnGameStop>> =
                        [OnGameStop::Nothing, OnGameStop::Restore, OnGameStop::Front]
                            .into_iter()
                            .map(|v| SelectOption::new(v, t("settings", stop_word(v).1)))
                            .collect();
                    Field::new(t("settings", "lifeOnGameStop")).show(ui, |ui| {
                        let r = ui.add(
                            Select::new("settings.life.stop", &mut stop, &opts)
                                .width(SELECT_W)
                                .label(t("settings", "lifeOnGameStop")),
                        );
                        if r.changed() {
                            let _ = cx.run(
                                "settings.life.set_on_game_stop",
                                &json!({ "action": stop_word(stop).0 }),
                            );
                        }
                    });

                    Field::new(t("settings", "lifeNotify"))
                        .hint(t("settings", "lifeNotifyHint"))
                        .last()
                        .show(ui, |ui| {
                            let mut on = p.notify;
                            let changed = boxed(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(Switch::new(&mut on, t("settings", "lifeNotifySwitch")))
                                    .changed()
                            });
                            if changed {
                                let _ = cx.run("settings.life.set_notify", &json!({ "on": on }));
                            }
                        });
                },
            );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_has_the_word_its_command_reads() {
        // The words `platform::life::command` matches, one per choice.
        for v in [
            OnGameStart::Nothing,
            OnGameStart::Minimize,
            OnGameStart::Tray,
            OnGameStart::Close,
        ] {
            let json = serde_json::to_value(v).unwrap();
            assert_eq!(json, start_word(v).0);
        }
        for v in [OnGameStop::Nothing, OnGameStop::Restore, OnGameStop::Front] {
            let json = serde_json::to_value(v).unwrap();
            assert_eq!(json, stop_word(v).0);
        }
    }
}
