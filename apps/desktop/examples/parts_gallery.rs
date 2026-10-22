//! Every app part (`dzch_ui::parts`) with realistic data, rendered headless (egui_kittest +
//! wgpu) into PNGs to compare with the Svelte interface (`var/reference/*.png`).
//!
//! `cargo run -p dayz-community-hub --example parts_gallery -- /abs/out.png`
//!
//! Writes, beside `out.png` (every part, chernarus) and `out-light.png` (the light preset):
//! `out-servers.png`, `out-history.png`, `out-settings.png`, `out-stats.png` and
//! `out-options.png`: the parts laid out where `var/reference/<view>.png` has them, in
//! French as the references are, to compare crops; `out-menu.png` opens a row's menu.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use dz_hub::features::browser::ServerRow;
use dzch_ui::i18n::{self, Locale, t};
use dzch_ui::icons::lucide;
use dzch_ui::kit::{self, Field, IconButton, Size, Tag, Tone, Variant};
use dzch_ui::parts::*;
use dzch_ui::theme::{self, Tokens, TokensExt, presets};
use egui::{Pos2, Rect, Ui, UiBuilder, Vec2};

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Parts,
    Servers,
    History,
    Settings,
    Stats,
    Options,
    Menu,
}

struct State {
    mode: Mode,
    frame: u32,
    preset: &'static str,
    rows: Vec<ServerRow>,
    cache: RowCache,
    secret: String,
    path: String,
    name: String,
    color: theme::Oklch,
    keys_mode: ShortcutMode,
    pad_mode: ShortcutMode,
    search: String,
    log: Vec<String>,
    times: Vec<f64>,
    mods: Vec<ModRef>,
    doc: HtmlDoc,
    points: Vec<(i64, f64)>,
    heat: [[f64; 24]; 7],
}

#[allow(clippy::too_many_arguments)]
fn server(
    name: &str,
    ip: &str,
    port: i64,
    env: &str,
    map: &str,
    time: &str,
    ping: Option<u32>,
    players: i64,
    max: i64,
) -> ServerRow {
    ServerRow {
        game_port: port,
        ip: ip.to_owned(),
        query_port: port + 1,
        name: name.to_owned(),
        map: map.to_owned(),
        players,
        max_players: max,
        environment: env.to_owned(),
        password: false,
        version: "1.28.162391".to_owned(),
        first_person_only: name.contains("1st Person"),
        time: time.to_owned(),
        mods_count: 0,
        mods_missing: None,
        mods_stale: 0,
        vac: true,
        battl_eye: Some(true),
        bots: 0,
        ping_ms: ping,
        ping_failed: false,
        favorite: false,
        excluded: false,
        unverified_full: false,
        official: !name.contains("x10 Loot"),
        mimics_official: name.contains("x10 Loot"),
    }
}

/// The rows of `var/reference/servers.png`.
fn rows() -> Vec<ServerRow> {
    let to = Some(9999);
    let mut v = vec![
        server(
            "4100 | EUROPE - DE",
            "45.0.0.0",
            2302,
            "w",
            "chernarusplus",
            "17:40",
            Some(92),
            41,
            60,
        ),
        server(
            "4107 | EUROPE - FR",
            "46.7.13.31",
            2402,
            "w",
            "enoch",
            "15:03",
            Some(166),
            57,
            127,
        ),
        server(
            "4114 | EUROPE - GB | 1st Person Only",
            "47.14.26.62",
            2502,
            "l",
            "sakhal",
            "05:50",
            Some(203),
            0,
            100,
        ),
        server(
            "4203 | UNITED STATES - NY | x10 Loot",
            "48.21.39.93",
            2602,
            "w",
            "chernarusplus",
            "17:35",
            Some(203),
            0,
            100,
        ),
        server(
            "4128 | AUSTRALIA - SYD",
            "49.28.52.124",
            2702,
            "w",
            "enoch",
            "00:08",
            Some(240),
            0,
            40,
        ),
        server(
            "4135 | EUROPE - DE | 1st Person Only",
            "50.35.65.155",
            2302,
            "w",
            "sakhal",
            "18:37",
            Some(240),
            37,
            60,
        ),
        server(
            "4142 | EUROPE - FR",
            "51.42.78.186",
            2402,
            "w",
            "chernarusplus",
            "02:49",
            Some(240),
            11,
            40,
        ),
        server(
            "4207 | EUROPE - GB | x10 Loot",
            "52.49.91.217",
            2502,
            "w",
            "namalsk",
            "12:26",
            Some(240),
            10,
            80,
        ),
        server(
            "4156 | UNITED STATES - NY",
            "53.56.104.248",
            2602,
            "l",
            "sakhal",
            "05:42",
            to,
            66,
            127,
        ),
        server(
            "4163 | AUSTRALIA - SYD",
            "54.63.117.29",
            2702,
            "l",
            "chernarusplus",
            "00:14",
            Some(240),
            0,
            127,
        ),
        server(
            "4170 | EUROPE - DE | 1st Person Only",
            "55.70.130.60",
            2302,
            "w",
            "enoch",
            "04:46",
            Some(240),
            2,
            50,
        ),
        server(
            "4211 | EUROPE - FR | x10 Loot",
            "56.77.143.91",
            2402,
            "w",
            "pripyat",
            "01:53",
            Some(240),
            43,
            60,
        ),
        server(
            "4184 | EUROPE - GB",
            "57.84.156.122",
            2502,
            "l",
            "chernarusplus",
            "15:01",
            to,
            4,
            100,
        ),
        server(
            "4191 | UNITED STATES - NY | 1st Person Only",
            "58.91.169.153",
            2602,
            "w",
            "enoch",
            "00:08",
            to,
            0,
            50,
        ),
    ];
    for (i, r) in v.iter_mut().enumerate() {
        r.favorite = (3..=8).contains(&i);
    }
    v[3].mods_count = 24;
    v[3].mods_missing = Some(0);
    v[7].mods_count = 1;
    v[7].mods_missing = Some(0);
    v[7].mods_stale = 1;
    v[7].first_person_only = true;
    v[7].battl_eye = Some(false);
    v[11].mods_count = 22;
    v[11].mods_missing = Some(22);
    v[11].password = true;
    v
}

/// The rows of `var/reference/history.png`.
fn history_rows() -> Vec<ServerRow> {
    let to = Some(9999);
    let mut v = vec![
        server(
            "4170 | EUROPE - DE | 1st Person Only",
            "55.70.130.60",
            2302,
            "w",
            "enoch",
            "04:46",
            Some(240),
            2,
            50,
        ),
        server(
            "4211 | EUROPE - FR | x10 Loot",
            "56.77.143.91",
            2402,
            "w",
            "pripyat",
            "01:53",
            Some(240),
            43,
            60,
        ),
        server(
            "4184 | EUROPE - GB",
            "57.84.156.122",
            2502,
            "l",
            "chernarusplus",
            "15:01",
            to,
            4,
            100,
        ),
        server(
            "4191 | UNITED STATES - NY | 1st Person Only",
            "58.91.169.153",
            2602,
            "w",
            "enoch",
            "00:08",
            to,
            0,
            50,
        ),
        server(
            "4198 | AUSTRALIA - SYD",
            "59.98.182.184",
            2702,
            "w",
            "sakhal",
            "04:44",
            to,
            37,
            60,
        ),
        server(
            "4215 | EUROPE - DE | x10 Loot",
            "60.105.195.215",
            2302,
            "w",
            "banov",
            "12:44",
            Some(54),
            9,
            60,
        ),
        server(
            "4212 | EUROPE - FR",
            "61.112.208.246",
            2402,
            "w",
            "enoch",
            "13:25",
            Some(54),
            80,
            80,
        ),
    ];
    v[1].mods_count = 22;
    v[1].mods_missing = Some(22);
    v[1].password = true;
    v
}

fn setup(ctx: &egui::Context, preset: &str) {
    ctx.set_fonts(theme::fonts::definitions());
    let tokens = presets::by_id(preset)
        .map(|p| Tokens::derive(&p.raw, p.scheme))
        .unwrap_or_default();
    theme::apply(ctx, tokens);
    kit::install(ctx);
}

/// A child `Ui` over `rect`, clipped to it, painted `fill`.
fn area(ui: &mut Ui, rect: Rect, fill: egui::Color32) -> Ui {
    ui.painter().rect_filled(rect, 0.0, fill);
    let mut c = ui.new_child(UiBuilder::new().max_rect(rect));
    c.set_clip_rect(rect);
    c
}

fn list(
    ui: &mut Ui,
    rows: &[ServerRow],
    cache: &mut RowCache,
    history: bool,
    selected: Option<usize>,
) {
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    TableHead::new(history).show(ui, |ui, cols| {
        let c = |k: &str| t("servers", k);
        head_cell(ui, cols.ping, |ui| {
            ui.add(SortHead::new(c("colPing"), false, true))
        });
        head_cell(ui, cols.players, |ui| {
            ui.add(SortHead::new(c("colPlayers"), false, true))
        });
        head_cell(ui, cols.server, |ui| {
            ui.add(SortHead::new(c("colServer"), false, true))
        });
        if let Some(m) = cols.map {
            head_cell(ui, m, |ui| {
                ui.add(SortHead::new(c("colMap"), false, true));
                ui.add(SortHead::new(c("colTime"), false, true).tip(c("colTimeTitle")));
            });
        }
        head_cell(ui, cols.mods, |ui| {
            ui.add(SortHead::new(c("colMods"), false, true))
        });
        if let Some(x) = cols.extra {
            head_cell(ui, x, |ui| {
                ui.add(SortHead::new(t("history", "colLastPlayed"), true, false))
            });
        }
    });
    let cols = ListColumns::new(ui.max_rect().x_range(), history);
    let direct = [RowButton {
        id: 1,
        icon: lucide::PLUG_ZAP,
        label: t("servers", "directConnect"),
        danger: false,
        tint: None,
    }];
    let ago = [
        "il y a 10 minutes",
        "il y a 2 heures",
        "il y a 4 heures",
        "il y a 6 heures",
        "il y a 8 heures",
        "il y a 10 heures",
        "il y a 12 heures",
    ];
    let when = [
        "21/10/2026 18:48:19",
        "21/10/2026 16:48:19",
        "21/10/2026 14:48:19",
        "21/10/2026 12:48:19",
        "21/10/2026 10:48:19",
        "21/10/2026 8:48:19",
        "21/10/2026 6:48:19",
    ];
    for (i, r) in rows.iter().enumerate() {
        let mut row = ServerListRow::new(RowData::from_row(r))
            .selected(selected == Some(i))
            .buttons(&direct);
        if history {
            row = row.extra(RowExtra {
                top: ago.get(i).copied().unwrap_or(""),
                bottom: when.get(i).copied().unwrap_or(""),
                tip: "",
            });
        }
        let _ = row.show(ui, &cols, cache);
    }
}

fn servers_view(ui: &mut Ui, s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);
    let r = Rect::from_min_max(Pos2::new(208.0, 146.0), full.max);
    let mut lui = area(ui, r, tk.panel);
    list(&mut lui, &s.rows, &mut s.cache, false, None);
}

fn history_view(ui: &mut Ui, s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);
    let view = Rect::from_min_max(Pos2::new(208.0, 34.0), Pos2::new(1400.0, 832.0));
    let mut vui = area(ui, view, tk.panel);
    vui.spacing_mut().item_spacing = Vec2::ZERO;
    PageHeader::new(t("nav", "history")).show(
        &mut vui,
        |ui| {
            ui.add(
                Figure::new(t("history", "statEntries"))
                    .amount(12.0)
                    .tone(tk.accent),
            );
            ui.add(Figure::new(t("history", "statLastPlayed")).value("il y a 10 minutes"));
            ui.add(Figure::new(t("history", "statThisWeek")).amount(12.0));
            ui.add(Figure::new(t("favorites", "statOnline")).value("12/12"));
            ui.add(
                Figure::new(t("favorites", "statPlayers"))
                    .amount(284.0)
                    .tone(tk.ok),
            );
            ui.add(Figure::new(t("favorites", "statBestPing")).value("54 ms"));
        },
        |ui| {
            ui.add(ShortcutsButton::new("?"));
            ui.add(IconButton::new(lucide::PANEL_RIGHT, t("servers", "details")).active(true));
            ui.add(
                kit::Button::new(t("history", "clearAll"))
                    .icon(lucide::TRASH_2)
                    .variant(Variant::Danger),
            );
        },
        Some(|ui: &mut Ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add(
                    kit::Input::new(&mut s.search)
                        .search()
                        .placeholder(t("favorites", "searchPlaceholder"))
                        .width(320.0),
                );
                ui.add(kit::Button::chip(t("favorites", "onlineOnly"), false));
            });
        }),
    );
    let rows = history_rows();
    let rest = Rect::from_min_max(Pos2::new(208.0, vui.cursor().top()), view.max);
    let mut lui = area(&mut vui, rest, tk.panel);
    list(&mut lui, &rows, &mut s.cache, true, None);
}

fn settings_view(ui: &mut Ui, s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);
    let view = Rect::from_min_max(Pos2::new(208.0, 34.0), Pos2::new(1400.0, 832.0));
    let mut vui = area(ui, view, tk.panel);
    vui.spacing_mut().item_spacing = Vec2::ZERO;
    PageHeader::new(t("nav", "settings")).show(&mut vui, |_| {}, |_| {}, None::<fn(&mut Ui)>);
    let below = Rect::from_min_max(Pos2::new(208.0, vui.cursor().top()), view.max);
    let mut nui = area(&mut vui, below, tk.panel);
    let w = |k: &str| t("settings", k);
    let entries = [
        NavEntry {
            id: "account",
            label: w("sectionAccount"),
            dot: false,
        },
        NavEntry {
            id: "steam",
            label: w("sectionSteam"),
            dot: false,
        },
        NavEntry {
            id: "apis",
            label: w("sectionApis"),
            dot: false,
        },
        NavEntry {
            id: "location",
            label: w("sectionLocation"),
            dot: false,
        },
        NavEntry {
            id: "ping",
            label: w("sectionPing"),
            dot: false,
        },
        NavEntry {
            id: "excluded",
            label: w("sectionExcluded"),
            dot: false,
        },
        NavEntry {
            id: "appearance",
            label: w("sectionAppearance"),
            dot: true,
        },
        NavEntry {
            id: "language",
            label: w("sectionLanguage"),
            dot: false,
        },
        NavEntry {
            id: "window",
            label: w("sectionWindow"),
            dot: false,
        },
        NavEntry {
            id: "pad",
            label: t("pad", "section"),
            dot: false,
        },
        NavEntry {
            id: "battleye",
            label: w("sectionBattleye"),
            dot: false,
        },
        NavEntry {
            id: "data",
            label: w("sectionData"),
            dot: false,
        },
    ];
    let _ = SectionNav::new(w("onThisPage"), &entries, "account").show(&mut nui);
    let body = Rect::from_min_max(
        Pos2::new(484.0, below.top() + 22.0),
        Pos2::new(1332.0, view.bottom()),
    );
    let mut bui = nui.new_child(UiBuilder::new().max_rect(body));
    bui.spacing_mut().item_spacing.y = 0.0;
    SettingsSection::new(w("sectionAccount"))
        .description(w("identity"))
        .icon(lucide::USER_ROUND)
        .show(
            &mut bui,
            |_| {},
            |ui| {
                Field::new(w("ingameName"))
                    .hint(w("ingameNameHint"))
                    .last()
                    .show(ui, |ui| {
                        ui.add(kit::Input::new(&mut s.name));
                    });
            },
        );
    bui.add_space(36.0);
    SettingsSection::new(w("sectionSteam"))
        .description(w("steamLoginDesc"))
        .icon(lucide::GAMEPAD_2)
        .show(
            &mut bui,
            |ui| {
                ui.add(
                    Tag::new("SteamCMD")
                        .tone(Tone::Ok)
                        .icon(lucide::CIRCLE_CHECK),
                );
            },
            |ui| {
                Field::new(w("steamRoot"))
                    .hint(w("steamRootHint"))
                    .show(ui, |ui| {
                        let _ = PathInput::new(&mut s.path).show(ui);
                    });
                Field::new("API key").last().show(ui, |ui| {
                    ui.add(SecretInput::new(&mut s.secret));
                });
            },
        );
}

fn stats_view(ui: &mut Ui, s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);
    let r = Rect::from_min_max(Pos2::new(942.0, 393.0), Pos2::new(1400.0, 554.0));
    let mut sui = area(ui, r, tk.panel);
    let inner = Rect::from_min_max(Pos2::new(955.0, 401.0), Pos2::new(1388.0, 550.0));
    let mut c = sui.new_child(UiBuilder::new().max_rect(inner));
    c.spacing_mut().item_spacing.y = 6.0;
    c.add(kit::SectionLabel::new(t("stats", "sectionWhen")).icon(lucide::CLOCK));
    WeekHeat::new(
        &s.heat,
        |day, hour, v| format!("{day} {hour}h · {}", v.round()),
        t("stats", "whenHint"),
    )
    .now(2, 18)
    .show(&mut c);
}

fn options_view(ui: &mut Ui, _s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);
    let r = Rect::from_min_max(Pos2::new(1081.0, 122.0), Pos2::new(1400.0, 832.0));
    let mut oui = area(ui, r, tk.panel);
    let card = Rect::from_min_max(Pos2::new(1093.0, 390.0), Pos2::new(1388.0, 540.0));
    let mut c = oui.new_child(UiBuilder::new().max_rect(card));
    let flags: Vec<String> = [
        "-window",
        "-skipIntro",
        "-filePatching",
        "-world=empty",
        "-maxVRAM",
        "-noBenchmark",
        "-winxp",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    CommandPreview::new(
        t("options", "commandLine"),
        &flags,
        "—",
        t("common", "copy"),
    )
    .program("DayZ_x64")
    .hint(t("options", "commandLineHint"))
    .show(&mut c);
}

fn block(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    ui.add(kit::SectionLabel::new(title));
    ui.add_space(6.0);
    body(ui);
}

fn parts(ui: &mut Ui, s: &mut State) {
    let full = ui.ctx().content_rect();
    let tk = ui.tokens();
    ui.set_clip_rect(full);
    ui.painter().rect_filled(full, 0.0, tk.bg);

    // Column 1: the header, a list with every row variant, the log, cards.
    let c1 = Rect::from_min_max(full.min, Pos2::new(900.0, full.bottom()));
    let mut a = area(ui, c1, tk.panel);
    a.spacing_mut().item_spacing = Vec2::ZERO;
    PageHeader::new(t("nav", "servers")).show(
        &mut a,
        |ui| {
            ui.add(Figure::new(t("servers", "statShown")).amount(9499.0));
            ui.add(
                Figure::new(t("servers", "statPlayers"))
                    .value("258.6 k")
                    .tone(tk.ok),
            );
            ui.add(
                Figure::new(t("servers", "statFull"))
                    .amount(793.0)
                    .tone(tk.err)
                    .clickable()
                    .active(true),
            );
            ui.add(
                Figure::new(t("servers", "statEmpty"))
                    .amount(2841.0)
                    .clickable(),
            );
            ui.add(
                Figure::new(t("servers", "statModded"))
                    .value("62%")
                    .tone(tk.mods),
            );
        },
        |ui| {
            ui.add(ShortcutsButton::new("?"));
            ui.add(IconButton::new(lucide::PANEL_RIGHT, t("servers", "details")).active(true));
            ui.add(kit::Button::new(t("common", "refresh")).icon(lucide::REFRESH_CW));
        },
        None::<fn(&mut Ui)>,
    );
    {
        let cols = ListColumns::new(a.max_rect().x_range(), true);
        TableHead::new(true).show(&mut a, |ui, cols| {
            head_cell(ui, cols.ping, |ui| {
                ui.add(SortHead::new("Ping", true, true))
            });
            head_cell(ui, cols.server, |ui| {
                ui.add(SortHead::new("Server", false, true))
            });
        });
        let friends = vec!["Hicks".to_owned(), "Vasquez".to_owned()];
        let ban = [
            RowButton {
                id: 1,
                icon: lucide::PLUG_ZAP,
                label: "Direct",
                danger: false,
                tint: None,
            },
            RowButton {
                id: 2,
                icon: lucide::BAN,
                label: "Exclude",
                danger: true,
                tint: Some(tk.err),
            },
        ];
        let more = [RowMenuItem {
            id: 3,
            icon: lucide::TRASH_2,
            label: "Remove",
            danger: true,
            kbd: "Del",
        }];
        let mut pending = s.rows[0].clone();
        pending.ping_ms = None;
        let mut d0 = RowData::from_row(&pending);
        d0.ping_pending = true;
        d0.players_loading = true;
        d0.unverified_full = true;
        d0.flags.saved_password = true;
        let _ = ServerListRow::new(d0)
            .friends(&friends)
            .extra(RowExtra {
                top: "2 hours ago",
                bottom: "21/10/2026 16:48",
                tip: "",
            })
            .show(&mut a, &cols, &mut s.cache);
        let _ = ServerListRow::new(RowData::from_row(&s.rows[11]))
            .selected(true)
            .buttons(&ban)
            .menu(&more)
            .show(&mut a, &cols, &mut s.cache);
        let mut d2 = RowData::from_row(&s.rows[7]);
        d2.listed = false;
        let _ = ServerListRow::new(d2)
            .tag(RowTag {
                text: "not in list",
                tone: Tone::Warn,
                tip: "",
            })
            .show(&mut a, &cols, &mut s.cache);
        let mut un = s.rows[9].clone();
        un.map = String::new();
        un.excluded = true;
        un.bots = 6;
        un.players = 20;
        let _ = ServerListRow::new(RowData::from_row(&un)).show(&mut a, &cols, &mut s.cache);
    }
    a.add_space(8.0);
    let mut inner = a.new_child(
        UiBuilder::new().max_rect(a.available_rect_before_wrap().shrink2(Vec2::new(12.0, 0.0))),
    );
    inner.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
    let log_rect = Rect::from_min_size(
        inner.cursor().min,
        Vec2::new(inner.available_width(), 200.0),
    );
    let mut lui = inner.new_child(UiBuilder::new().max_rect(log_rect));
    LogPane::new("gallery-log", &s.log)
        .times(&s.times)
        .title("steamcmd")
        .live(true)
        .show(&mut lui);
    inner.add_space(208.0);
    let _ = BulkBar::new("3 mods selected").info("1.2 GB").show(
        &mut inner,
        |ui| {
            ui.add(
                kit::Button::new("Select updates")
                    .size(Size::Xs)
                    .variant(Variant::Ghost),
            );
        },
        |ui| {
            ui.add(
                kit::Button::new("Delete")
                    .size(Size::Xs)
                    .variant(Variant::Danger)
                    .icon(lucide::TRASH_2),
            );
            ui.add(
                kit::Button::new("Update")
                    .size(Size::Xs)
                    .icon(lucide::DOWNLOAD),
            );
        },
    );
    inner.add_space(8.0);
    inner.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(430.0);
            SectionCard::new("Launch under Linux")
                .description("what runs DayZ")
                .icon(lucide::TERMINAL)
                .padded()
                .show(
                    ui,
                    |ui| {
                        ui.add(Tag::new("Proton").tone(Tone::Accent));
                    },
                    |ui| {
                        DetailList::new("gallery-details").show(ui, |rows| {
                            rows.add(DetailRow::new("Steam", "Native install"));
                            rows.add(
                                DetailRow::new("Compatibility tool", "proton-cachyos-native")
                                    .tone(tk.ok),
                            );
                            rows.add(DetailRow::new("Prefix", None));
                            rows.custom("Address", |ui| {
                                ui.add(kit::Copy::new("45.0.0.0:2302").labels("Copy", "Copied"));
                            });
                        });
                    },
                );
            ui.add_space(8.0);
            let flags: Vec<String> = [
                "-window",
                "-skipIntro",
                "-world=empty",
                "-maxMem=16384",
                "-noBenchmark",
            ]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
            CommandPreview::new("Command line", &flags, "No arguments", "Copy")
                .program("DayZ_x64")
                .hint("What DayZ receives at launch, besides the address and the mods.")
                .show(ui);
            ui.add_space(8.0);
            Section::new("Mods").icon(lucide::PUZZLE).first().show(
                ui,
                |ui| {
                    ui.add(Tag::new("4"));
                },
                |ui| {
                    let _ = ModStatusList::new(&s.mods).show(ui);
                },
            );
        });
        ui.vertical(|ui| {
            ui.set_width(430.0);
            let labels = TransferLabels {
                size: "Size",
                speed: "Speed",
                eta: "Left",
                elapsed: "Elapsed",
            };
            ui.add(TransferProgress {
                label: "Downloading",
                name: "@CF · Community Framework",
                percent: Some(42.7),
                done: Some(120_000_000),
                total: Some(281_000_000),
                speed: Some(12_400_000.0),
                eta: Some(13_000.0),
                elapsed: Some(9_000.0),
                labels,
            });
            ui.add_space(8.0);
            ui.add(TransferProgress {
                label: "Verifying",
                name: "",
                percent: None,
                done: None,
                total: None,
                speed: None,
                eta: None,
                elapsed: Some(2_000.0),
                labels,
            });
            ui.add_space(8.0);
            ui.add(Facts::new(&[
                Fact::new("Players", "41 / 60").tone(tk.warn),
                Fact::new("Queue", "0"),
                Fact::new("Version", "1.28.162391"),
                Fact::new("Time", "17:40"),
                Fact::new("Perspective", "1PP"),
                Fact::new("BattlEye", "on").tone(tk.ok),
            ]));
            ui.add_space(8.0);
            let e = Rect::from_min_size(ui.cursor().min, Vec2::new(430.0, 192.0));
            let mut eui = ui.new_child(UiBuilder::new().max_rect(e));
            Empty::new()
                .icon(lucide::STAR)
                .title("No favourites yet")
                .text("Star a server in the list to keep it here.")
                .show(&mut eui, |ui| {
                    ui.add(kit::Button::new("Browse servers").variant(Variant::Accent));
                });
            ui.add_space(200.0);
        });
    });

    // Column 2: cells, charts, themes, languages, shortcuts, an article.
    let c2 = Rect::from_min_max(Pos2::new(912.0, 0.0), Pos2::new(1392.0, full.bottom()));
    let mut b = area(ui, c2, tk.bg);
    b.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
    block(&mut b, "Cells", |ui| {
        ui.horizontal(|ui| {
            ui.add(ServerFlags::new(Flags {
                password: true,
                first_person: true,
                battleye: true,
                saved_password: true,
                official: true,
                mimics_official: false,
            }));
            ui.add(ServerFlags::new(Flags {
                mimics_official: true,
                ..Flags::default()
            }));
            ui.add(TimeOfDay::new("17:40"));
            ui.add(TimeOfDay::new("05:50"));
            ui.add(TimeOfDay::new("00:08").small());
            ui.add(OsIcon::new("w"));
            ui.add(OsIcon::new("l"));
        });
        ui.horizontal(|ui| {
            ui.add(ModsCount::new(24).clickable(true));
            ui.add(ModsCount::new(22).missing(Some(22)).clickable(true));
            ui.add(ModsCount::new(1).missing(Some(0)).stale(1));
            ui.add(ModsCount::new(9).missing(Some(0)));
            ui.add(ModsCount::new(0));
            ui.add(PingCell::new(Some(42)));
            ui.add(PingCell::new(Some(166)).warn(true));
            ui.add(PingCell::new(Some(9999)));
            ui.add(PingCell::new(None).pending(true));
        });
        ui.horizontal(|ui| {
            ui.add(PlayersCell::new(41, 60).bots(3));
            ui.add(PlayersCell::new(60, 60).compact());
            ui.add(FavoriteButton::new(true));
            ui.add(FavoriteButton::new(false));
            ui.add(FavoriteButton::new(true).size(StarSize::Md));
            ui.add(ExcludeButton::new("45.0.0.0", true));
            ui.add(ExcludeButton::new("45.0.0.0", false));
        });
        ui.horizontal(|ui| {
            ui.add(JoinButton::new().compact().quiet());
            ui.add(JoinButton::new().compact().quiet().lit(true));
            ui.add(JoinButton::new().compact());
            ui.add(JoinButton::new());
            ui.add(JoinButton::new().size(Size::Lg));
            ui.add(ShortcutsButton::new("?"));
        });
    });
    block(&mut b, "PlayerChart", |ui| {
        ui.add(PlayerChart::new(&s.points).max(Some(60.0)));
    });
    block(&mut b, "WeekHeat", |ui| {
        WeekHeat::new(
            &s.heat,
            |d, h, v| format!("{d} {h}:00 · {v:.0} players"),
            "Average players by weekday and hour, in your local time.",
        )
        .now(2, 18)
        .show(ui);
    });
    block(&mut b, "ThemeSwatches", |ui| {
        let look = |id: &str| {
            presets::by_id(id)
                .map(|p| Tokens::derive(&p.raw, p.scheme))
                .unwrap_or_default()
        };
        let _ = ThemeSwatches::new(
            Some("chernarus"),
            (look("chernarus"), look("topo")),
            t("theme", "followSystem"),
        )
        .columns(5)
        .show(ui);
    });
    block(&mut b, "LanguagePicker", |ui| {
        let _ = LanguagePicker::new(i18n::locale()).show(ui);
    });
    block(&mut b, "ColorToken", |ui| {
        ui.add(ColorToken::new("Accent", &mut s.color));
    });
    block(&mut b, "PadGlyph", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for kind in [PadKind::Xbox, PadKind::PlayStation, PadKind::Nintendo] {
                for bt in [
                    PadButton::A,
                    PadButton::B,
                    PadButton::X,
                    PadButton::Y,
                    PadButton::Lb,
                    PadButton::Rt,
                    PadButton::Menu,
                    PadButton::View,
                    PadButton::Dpad,
                    PadButton::L3,
                    PadButton::Rs,
                ] {
                    ui.add(PadGlyph::new(bt, kind));
                }
                ui.add_space(6.0);
            }
        });
    });
    let rows_g = [
        ShortcutRow {
            label: "Command palette",
            keys: &["Ctrl+K"],
            pad: &[],
            pair: false,
        },
        ShortcutRow {
            label: "Search",
            keys: &["/", "Ctrl+F"],
            pad: &[PadButton::Y],
            pair: false,
        },
        ShortcutRow {
            label: "Move in the list",
            keys: &["↑", "↓"],
            pad: &[PadButton::Dpad],
            pair: true,
        },
        ShortcutRow {
            label: "Join",
            keys: &["Enter"],
            pad: &[PadButton::A],
            pair: false,
        },
    ];
    let rows_v = [
        ShortcutRow {
            label: "Servers",
            keys: &["1"],
            pad: &[],
            pair: false,
        },
        ShortcutRow {
            label: "Favourites",
            keys: &["2"],
            pad: &[],
            pair: false,
        },
        ShortcutRow {
            label: "History",
            keys: &["3"],
            pad: &[],
            pair: false,
        },
        ShortcutRow {
            label: "Friends",
            keys: &["4"],
            pad: &[],
            pair: false,
        },
    ];
    let keys = [
        ShortcutGroup {
            title: t("keys", "groupGlobal"),
            views: false,
            rows: &rows_g,
        },
        ShortcutGroup {
            title: t("keys", "groupViews"),
            views: true,
            rows: &rows_v,
        },
    ];
    let pads = [ShortcutGroup {
        title: t("keys", "groupGlobal"),
        views: false,
        rows: &rows_g,
    }];
    b.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(236.0);
            ShortcutList::new(&keys, &pads, &mut s.keys_mode).show(ui);
        });
        ui.vertical(|ui| {
            ui.set_width(236.0);
            ShortcutList::new(&keys, &pads, &mut s.pad_mode)
                .pad_kind(PadKind::PlayStation)
                .show(ui);
        });
    });
    block(&mut b, "SafeHtml", |ui| {
        let _ = SafeHtml::new(&s.doc).zoomable().show(ui, |_| None);
    });
}

fn render(mode: Mode, preset: &'static str, size: Vec2, out: &str) {
    let mut harness = egui_kittest::Harness::builder()
        .with_size(size)
        .with_pixels_per_point(1.0)
        .with_step_dt(1.0 / 60.0)
        .wgpu()
        .build_ui_state(
            |ui, s: &mut State| {
                if s.frame == 0 {
                    setup(ui.ctx(), s.preset);
                    s.frame += 1;
                    return;
                }
                s.frame += 1;
                match s.mode {
                    Mode::Servers | Mode::Menu => servers_view(ui, s),
                    Mode::History => history_view(ui, s),
                    Mode::Settings => settings_view(ui, s),
                    Mode::Stats => stats_view(ui, s),
                    Mode::Options => options_view(ui, s),
                    Mode::Parts => parts(ui, s),
                }
            },
            state(mode, preset),
        );
    harness.input_mut().max_texture_side = Some(8192);
    for _ in 0..8 {
        harness.step();
    }
    if mode == Mode::Menu || mode == Mode::Stats {
        // Menu: hover the second row and right-click it. Stats: hover a heat cell.
        let at = if mode == Mode::Menu {
            Pos2::new(700.0, 174.0 + 48.0 + 24.0)
        } else {
            Pos2::new(1200.0, 470.0)
        };
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(at));
        harness.step();
        if mode == Mode::Menu {
            for pressed in [true, false] {
                harness.input_mut().events.push(egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        for _ in 0..12 {
            harness.step();
        }
    }
    match harness.render() {
        Ok(img) => {
            img.save(out).expect("save png");
            println!("{out}");
        }
        Err(e) => eprintln!("render failed: {e}"),
    }
}

fn state(mode: Mode, preset: &'static str) -> State {
    let log: Vec<String> = [
        "Redirecting stderr to '/home/player/.local/share/Steam/logs/stderr.txt'",
        "Logging directory: '/home/player/.local/share/Steam/logs'",
        "[  0%] Checking for available updates...",
        "[----] Verifying installation...",
        "Steam Console Client (c) Valve Corporation - version 1729890486",
        "Loading Steam API...OK",
        "Logging in user 'survivor_42' to Steam Public...",
        "Waiting for user info...OK",
        "Downloading item 1559212036 ...",
        " Update state (0x61) downloading, progress: 42.17 (120000000 / 281000000)",
        "Warning: rate limit reached, retrying in 5 s",
        "ERROR! Download item 1564026768 failed (Failure).",
        "Success. Downloaded item 1559212036 to \"/home/player/.local/share/dzch/steamapps/workshop/content/221100/1559212036\" (281000000 bytes)",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let times = (0..log.len()).map(|i| i as f64 * 1700.0).collect();
    let mut mods = vec![
        ModRef::new(1559212036, "@CF", ModState::Installed, 2_400_000),
        ModRef::new(1564026768, "@Community-Online-Tools", ModState::Missing, 0),
        ModRef::new(2545327648, "@Dabs Framework", ModState::Stale, 8_100_000),
        ModRef::new(
            1828439124,
            "@VPPAdminTools",
            ModState::Installed,
            61_000_000,
        ),
        ModRef::new(2116157322, "", ModState::Missing, 0),
    ];
    ModRef::sort(&mut mods);
    let doc = HtmlDoc::parse(
        r#"<h2>Update 1.28 &amp; Stable</h2><p>Survivors, <b>update 1.28</b> is now live on <a href="/news">all platforms</a>. It brings a new <i>weather</i> system and <code>-filePatching</code> fixes.</p><ul><li>Fixed vehicles sinking</li><li>Improved server performance</li></ul><blockquote>Thank you for your patience.</blockquote><script>alert(1)</script>"#,
        "https://dayz.com/article/updates/x",
    );
    let points = (0..96)
        .map(|i| {
            let h = i as f64 / 4.0;
            let v =
                30.0 + 25.0 * ((h - 20.0) / 24.0 * std::f64::consts::TAU).cos() + (i % 7) as f64;
            (1_760_000_000 + i * 900, v.max(0.0).round())
        })
        .collect();
    let mut heat = [[0.0; 24]; 7];
    for (d, row) in heat.iter_mut().enumerate() {
        for (h, v) in row.iter_mut().enumerate() {
            let evening = if h >= 18 {
                (h - 17) as f64 * 1.5
            } else if h < 1 {
                4.0
            } else {
                0.0
            };
            *v = if (h + d) % 5 == 0 || evening > 0.0 {
                evening + ((h * 7 + d * 3) % 4) as f64
            } else {
                0.0
            };
        }
    }
    State {
        mode,
        frame: 0,
        preset,
        rows: rows(),
        cache: RowCache::new(),
        secret: "hunter2".into(),
        path: "/home/player/.local/share/Steam".into(),
        name: "Survivor".into(),
        color: [0.788, 0.141, 85.0],
        keys_mode: ShortcutMode::Keys,
        pad_mode: ShortcutMode::Pad,
        search: String::new(),
        log,
        times,
        mods,
        doc,
        points,
        heat,
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "parts_gallery.png".to_owned());
    let stem = out.strip_suffix(".png").unwrap_or(&out).to_owned();
    let view = Vec2::new(1400.0, 860.0);
    i18n::set_locale(Locale::Fr);
    for (mode, name) in [
        (Mode::Servers, "servers"),
        (Mode::History, "history"),
        (Mode::Settings, "settings"),
        (Mode::Stats, "stats"),
        (Mode::Options, "options"),
        (Mode::Menu, "menu"),
    ] {
        render(
            mode,
            presets::DEFAULT_DARK,
            view,
            &format!("{stem}-{name}.png"),
        );
    }
    i18n::set_locale(Locale::En);
    let big = Vec2::new(1400.0, 1760.0);
    render(Mode::Parts, presets::DEFAULT_DARK, big, &out);
    render(
        Mode::Parts,
        presets::DEFAULT_LIGHT,
        big,
        &format!("{stem}-light.png"),
    );
}
