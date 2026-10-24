//! Every widget of the kit in every variant, rendered headless (egui_kittest + wgpu) into
//! PNGs to compare with the Svelte interface (`var/reference/*.png`).
//!
//! `cargo run -p dayz-community-hub --example kit_gallery -- /abs/out.png`
//!
//! Writes `out.png` (chernarus, the dark default), `out-topo.png` (the light preset),
//! `out-float0.png` (dropdown, tooltip, toasts), `out-float1.png` (select),
//! `out-float2.png` (popover), `out-float3.png` (tooltip) and `out-dialog.png`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::too_many_lines)]

use dzch_ui::icons::lucide;
use dzch_ui::kit::{self, *};
use dzch_ui::theme::{self, Tokens, TokensExt, presets};
use egui::{Popup, Ui, Vec2};
use egui_kittest::kittest::Queryable;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Gallery,
    /// Which floating thing is opened: 0 the dropdown, 1 the select, 2 the popover, 3 a tooltip.
    Float(u8),
    Dialog,
}

struct State {
    mode: Mode,
    frame: u32,
    preset: &'static str,
    text: String,
    search: String,
    empty: String,
    code: String,
    pass: String,
    port: f64,
    notes: String,
    map: Option<&'static str>,
    sw_on: bool,
    sw_off: bool,
    sw_warn: bool,
    mini_on: bool,
    mini_off: bool,
    cb_on: bool,
    cb_off: bool,
    cb_mixed: bool,
    tg_on: bool,
    tg_off: bool,
    seg: u8,
    seg_icons: u8,
    tab: u8,
    tab2: u8,
    slider: f64,
    dialog: bool,
    toasts_said: bool,
}

impl State {
    fn new(mode: Mode, preset: &'static str) -> Self {
        Self {
            mode,
            frame: 0,
            preset,
            text: "Survivor".into(),
            search: "chernarus".into(),
            empty: String::new(),
            code: "-mod=@CF;@VPPAdminTools".into(),
            pass: "secret".into(),
            port: 2302.0,
            notes: "Two lines\nof notes".into(),
            map: Some("chernarusplus"),
            sw_on: true,
            sw_off: false,
            sw_warn: true,
            mini_on: true,
            mini_off: false,
            cb_on: true,
            cb_off: false,
            cb_mixed: false,
            tg_on: true,
            tg_off: false,
            seg: 0,
            seg_icons: 1,
            tab: 0,
            tab2: 1,
            slider: 0.4,
            dialog: true,
            toasts_said: false,
        }
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(6.0);
    ui.add(SectionLabel::new(text));
    ui.add_space(4.0);
}

fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
        add(ui);
    });
}

fn buttons(ui: &mut Ui) {
    heading(ui, "Buttons");
    for (v, name) in [
        (Variant::Default, "Default"),
        (Variant::Accent, "Accent"),
        (Variant::Ghost, "Ghost"),
        (Variant::Danger, "Danger"),
        (Variant::Play, "Play"),
    ] {
        row(ui, |ui| {
            ui.add(
                Button::new(name)
                    .variant(v)
                    .size(Size::Lg)
                    .icon(lucide::PLAY),
            );
            ui.add(Button::new(name).variant(v).icon(lucide::REFRESH_CW));
            ui.add(Button::new(name).variant(v).size(Size::Xs));
            ui.add(Button::new(name).variant(v).enabled(false));
        });
    }
    row(ui, |ui| {
        ui.add(Button::new("Active").active(true));
        ui.add(Button::chip("1PP", true));
        ui.add(Button::chip("3PP", false));
        ui.add(Button::new("Maps").trailing(lucide::CHEVRON_DOWN));
    });
    row(ui, |ui| {
        ui.add(IconButton::new(lucide::SETTINGS, "Settings"));
        ui.add(IconButton::new(lucide::REFRESH_CW, "Refresh").variant(Variant::Default));
        ui.add(IconButton::new(lucide::PANEL_RIGHT, "Detail").active(true));
        ui.add(IconButton::new(lucide::PLAY, "Join").variant(Variant::Accent));
        ui.add(IconButton::new(lucide::TRASH_2, "Delete").variant(Variant::Danger));
        ui.add(IconButton::new(lucide::X, "Close").size(Size::IconXs));
        ui.add(IconButton::new(lucide::STAR, "Favourite").enabled(false));
    });
}

fn inputs(ui: &mut Ui, s: &mut State) {
    heading(ui, "Inputs");
    ui.add(Input::new(&mut s.text).width(220.0));
    ui.add_space(6.0);
    ui.add(
        Input::new(&mut s.search)
            .search()
            .placeholder("Search name, IP, map…")
            .width(260.0),
    );
    ui.add_space(6.0);
    ui.add(
        Input::new(&mut s.empty)
            .search()
            .placeholder("Search name, IP, map…")
            .width(260.0),
    );
    ui.add_space(6.0);
    row(ui, |ui| {
        ui.add(Input::new(&mut s.code).kind(InputKind::Code).width(150.0));
        ui.add(
            Input::new(&mut s.pass)
                .kind(InputKind::Password)
                .width(100.0),
        );
        ui.add(
            NumberInput::new(&mut s.port)
                .range(1.0, 65535.0)
                .width(70.0),
        );
    });
    row(ui, |ui| {
        ui.add(Input::new(&mut s.text).size(FieldSize::Xs).width(120.0));
        ui.add(
            Input::new(&mut s.empty)
                .placeholder("Disabled")
                .enabled(false)
                .width(120.0),
        );
    });
    ui.add(TextArea::new(&mut s.notes).rows(2));
    heading(ui, "Select");
    let maps = [
        SelectOption::new(Some("chernarusplus"), "chernarusplus"),
        SelectOption::new(Some("enoch"), "enoch"),
        SelectOption::new(Some("sakhal"), "sakhal").group("DLC"),
    ];
    row(ui, |ui| {
        let r = ui.add(
            Select::new("g-map", &mut s.map, &maps)
                .placeholder("All maps")
                .none(None)
                .width(160.0),
        );
        if s.mode == Mode::Float(1) && s.frame == 3 {
            Popup::open_id(ui.ctx(), egui::Id::unique("g-map").with("popup"));
        }
        let _ = r;
        let mut none: Option<&str> = None;
        ui.add(
            Select::new("g-map2", &mut none, &maps)
                .placeholder("All maps")
                .width(160.0),
        );
    });
}

fn toggles(ui: &mut Ui, s: &mut State) {
    heading(ui, "Switches");
    ui.add(Switch::new(&mut s.sw_on, "Add Steam's servers"));
    ui.add(
        Switch::new(&mut s.sw_off, "Auto-finish")
            .hint("Writes to the game's profile when it closes."),
    );
    ui.add(Switch::new(&mut s.sw_warn, "Write to the game").tone(Tone::Warn));
    let mut d = false;
    ui.add(Switch::new(&mut d, "Disabled").enabled(false));
    row(ui, |ui| {
        ui.add(MiniSwitch::new(&mut s.mini_on));
        ui.add(MiniSwitch::new(&mut s.mini_off));
        ui.add(Checkbox::new(&mut s.cb_on));
        ui.add(Checkbox::new(&mut s.cb_off));
        ui.add(Checkbox::new(&mut s.cb_mixed).indeterminate(true));
    });
    ui.add(Toggle::new(&mut s.tg_on, "Official").count(412));
    let c = ui.tokens().map;
    ui.add(
        Toggle::new(&mut s.tg_off, "Community")
            .count(9087)
            .swatch(c),
    );
}

fn choices(ui: &mut Ui, s: &mut State) {
    heading(ui, "Segmented");
    let segs = [
        Segment::new(0u8, "SteamCMD"),
        Segment::new(1, "Steam client"),
    ];
    ui.add(Segmented::new(&mut s.seg, &segs));
    ui.add_space(6.0);
    let icons = [
        Segment::new(0u8, "List").icon(lucide::LIST),
        Segment::new(1, "Grid").icon(lucide::LAYOUT_DASHBOARD),
        Segment::new(2, "Map").icon(lucide::MAP),
    ];
    row(ui, |ui| {
        ui.add(Segmented::new(&mut s.seg_icons, &icons).icon_only());
        ui.add(Segmented::new(&mut s.seg_icons, &icons).size(FieldSize::Xs));
    });
    ui.add(Segmented::new(&mut s.seg, &segs).fill());
    heading(ui, "Tabs");
    let tabs = [
        Tab::new(0u8, "Installed").count(42),
        Tab::new(1, "Updates").count(4),
        Tab::new(2, "Workshop"),
    ];
    ui.add(TabStrip::new("g-tabs", &mut s.tab, &tabs));
    ui.add_space(6.0);
    let tabs2 = [
        Tab::new(0u8, "Overview").icon(lucide::INFO),
        Tab::new(1, "Players").icon(lucide::USERS),
        Tab::new(2, "Mods").icon(lucide::PUZZLE),
    ];
    ui.add(
        TabStrip::new("g-tabs2", &mut s.tab2, &tabs2)
            .fill()
            .size(FieldSize::Xs),
    );
}

fn marks(ui: &mut Ui) {
    heading(ui, "Tags and keys");
    row(ui, |ui| {
        ui.add(Tag::new("NEUTRAL"));
        ui.add(
            Tag::new("OFFICIEL")
                .tone(Tone::Ok)
                .icon(lucide::BADGE_CHECK),
        );
        ui.add(Tag::new("1PP").tone(Tone::Warn));
        ui.add(
            Tag::new("Imite l'officiel")
                .tone(Tone::Warn)
                .icon(lucide::TRIANGLE_ALERT),
        );
        ui.add(Tag::new("ERR").tone(Tone::Err));
        ui.add(Tag::new("ACCENT").tone(Tone::Accent));
    });
    row(ui, |ui| {
        ui.add(Tag::new("neutral").filled());
        ui.add(Tag::new("ok").tone(Tone::Ok).filled());
        ui.add(Tag::new("warn").tone(Tone::Warn).filled());
        ui.add(Tag::new("err").tone(Tone::Err).filled());
        ui.add(Tag::new("accent").tone(Tone::Accent).filled());
        ui.add(Kbd::new("Ctrl K"));
        ui.add(Kbd::new("Esc"));
    });
    heading(ui, "Signal, players, flags");
    row(ui, |ui| {
        for ms in [Some(32), Some(92), Some(166), Some(240), Some(9999), None] {
            ui.add(Signal::new(ms));
        }
        ui.add(Signal::new(None).pending(true));
    });
    row(ui, |ui| {
        ui.add(Signal::new(Some(32)).size(SignalSize::Xs).bars_only());
        ui.add(Signal::new(Some(92)).size(SignalSize::Md));
        ui.add(Players::new(41, 60));
        ui.add(Players::new(0, 100));
        ui.add(Players::new(66, 127).bots(12).queue(3));
        ui.add(Players::new(60, 60).compact());
    });
    row(ui, |ui| {
        for c in ["fr", "de", "gb", "us", "au", "ru", "zz"] {
            ui.add(Flag::new(c));
        }
        let ink = ui.tokens().fg_muted;
        Brand::Steam.show(ui, 16.0, ink);
        Brand::Linux.show(ui, 16.0, ink);
        Brand::Windows.show(ui, 16.0, ink);
    });
}

fn progress(ui: &mut Ui, s: &mut State) {
    heading(ui, "Meters");
    for (tone, f) in [
        (Tone::Accent, 0.62),
        (Tone::Ok, 0.33),
        (Tone::Warn, 0.8),
        (Tone::Err, 1.0),
        (Tone::Muted, 0.2),
        (Tone::Info, 0.5),
    ] {
        ui.add(Meter::new(f).tone(tone).animate(false));
        ui.add_space(4.0);
    }
    ui.add(Meter::new(0.5).size(MeterSize::Xs).animate(false));
    ui.add_space(4.0);
    ui.add(Meter::new(0.5).size(MeterSize::Md).animate(false));
    ui.add_space(4.0);
    ui.add(Meter::new(0.0).indeterminate());
    row(ui, |ui| {
        ui.add(Spinner::new());
        ui.add(Spinner::new().size(20.0));
        ui.add(Skeleton::new(120.0, 10.0));
        ui.add(Skeleton::new(32.0, 32.0).radius(4));
    });
    ui.add(SkeletonRows::new(2).media());
    heading(ui, "Slider");
    ui.add(Slider::new(&mut s.slider, 0.0..=1.0));
}

fn frames(ui: &mut Ui, s: &mut State) {
    heading(ui, "Panel and fields");
    let h = 120.0;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), egui::Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    Panel::new("g-panel").title("Filtres").show_with(
        &mut child,
        |ui| {
            ui.add(IconButton::new(lucide::FILTER_X, "Reset").size(Size::IconXs));
        },
        |ui| {
            Field::new("Nom en jeu")
                .hint("Transmis à DayZ via -name=")
                .show(ui, |ui| {
                    ui.add(Input::new(&mut s.text));
                });
            Field::new("Téléchargement").last().show(ui, |ui| {
                let segs = [
                    Segment::new(0u8, "SteamCMD"),
                    Segment::new(1, "Client Steam"),
                ];
                ui.add(Segmented::new(&mut s.seg, &segs));
            });
        },
    );
    ui.add_space(6.0);
    SectionLabel::new("Cartes")
        .icon(lucide::MAP)
        .show_with(ui, |ui| {
            ui.add(Button::new("tout").size(Size::Xs).variant(Variant::Ghost));
        });
    let ok = ui.tokens().ok;
    let warn = ui.tokens().err;
    StatStrip::show(
        ui,
        &[
            Stat::new("Affichés", "9 499"),
            Stat::new("Joueurs", "258,6 k").tone(ok),
            Stat::new("Pleins", "793").tone(warn),
            Stat::new("Carte", "chernarusplus").icon(lucide::MAP).grow(),
        ],
    );
    Disclosure::new("g-disc", "Détails")
        .default_open(true)
        .show(ui, |ui| {
            ui.add(Separator::horizontal());
        });
    Disclosure::new("g-disc2", "Closed disclosure").show(ui, |_| {});
}

fn alerts(ui: &mut Ui) {
    heading(ui, "Alerts");
    Alert::new(Tone::Warn, lucide::TRIANGLE_ALERT, "Steam is not running")
        .text("Mods cannot be downloaded through the Steam client until it is started.")
        .dismissible("Dismiss")
        .show(ui, |ui| {
            ui.add(Button::new("Start Steam").variant(Variant::Accent));
        });
    ui.add_space(6.0);
    Alert::new(Tone::Err, lucide::CIRCLE_X, "BattlEye is broken")
        .banner()
        .show(ui, |ui| {
            ui.add(CommandLine::new(
                "sudo setcap cap_net_raw+ep ./DayZ",
                "Copy",
            ));
        });
    ui.add_space(6.0);
    Alert::new(Tone::Info, lucide::INFO, "Update available")
        .text("v0.5.0")
        .show(ui, |_| {});
    ui.add_space(6.0);
    Alert::new(Tone::Ok, lucide::CIRCLE_CHECK, "Connected as survivor_42").show(ui, |_| {});
    row(ui, |ui| {
        ui.add(Copy::new("45.0.0.0:2302").labels("Copy", "Copied"));
        ui.add(Separator::vertical());
        ui.add(Kbd::new("F5"));
    });
    heading(ui, "Empty states");
    EmptyState::new()
        .icon(lucide::STAR)
        .title("No favourites")
        .text("Star a server to keep it here.")
        .compact()
        .show(ui, |_| {});
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 170.0), egui::Sense::hover());
    let t = ui.tokens();
    ui.painter().rect_filled(rect, 0, t.panel);
    kit::topo::paint(ui.painter(), rect, &t, 0.5);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    EmptyState::new()
        .icon(lucide::STAR)
        .title("No favourites yet")
        .text("Star a server in the list to keep it here.")
        .show(&mut child, |ui| {
            ui.add(Button::new("Browse servers").icon(lucide::SERVER));
        });
}

fn floating(ui: &mut Ui, s: &mut State) {
    let ctx = ui.ctx().clone();
    row(ui, |ui| {
        let r = ui.add(Button::new("Menu").trailing(lucide::CHEVRON_DOWN));
        if s.frame == 3 && s.mode == Mode::Float(0) {
            Popup::open_id(&ctx, Popup::default_response_id(&r));
        }
        menu::dropdown(&r, Side::Bottom, menu::Align::Start, |ui| {
            menu::label(ui, "Server");
            ui.add(
                MenuItem::new("Copy address")
                    .icon(lucide::COPY)
                    .kbd("Ctrl C"),
            );
            ui.add(MenuItem::new("Open on Steam").icon(lucide::EXTERNAL_LINK));
            ui.add(MenuItem::new("Disabled").enabled(false));
            ui.add(MenuItem::new("Theme: chernarus").checked(true));
            menu::separator(ui);
            ui.add(
                MenuItem::new("Remove from history")
                    .icon(lucide::TRASH_2)
                    .danger(),
            );
        });
        ui.add_space(220.0);
        ui.add(Button::new("Hover me").tip_kbd("Refresh the server list", "F5", Side::Right));
        ui.add_space(120.0);
        let p = menu::popover_trigger(ui, lucide::PALETTE, "Theme", 28.0);
        if s.frame == 3 && s.mode == Mode::Float(2) {
            Popup::open_id(&ctx, Popup::default_response_id(&p));
        }
        menu::popover(&p, Side::Bottom, menu::Align::Start, |ui| {
            egui::Frame::new().inner_margin(8).show(ui, |ui| {
                ui.set_width(200.0);
                ui.add(SectionLabel::new("Thème"));
                ui.add(Switch::new(&mut s.sw_on, "Follow the system"));
            });
        });
    });
    ui.add_space(250.0);
    inputs(ui, s);
    if !s.toasts_said {
        s.toasts_said = true;
        toast::ok(&ctx, "Theme copied to the clipboard.");
        toast::warn(&ctx, "Steam is not running: the mods cannot be checked.");
        toast::err(&ctx, "Could not write the profile.");
        toast::say_with_action(
            &ctx,
            "Server removed from favourites.",
            ToastTone::Neutral,
            "Undo",
            "favorites.undo",
        );
    }
}

fn gallery(ui: &mut Ui, s: &mut State) {
    let t = ui.tokens();
    let ctx = ui.ctx().clone();
    ctx.layer_painter(egui::LayerId::background())
        .rect_filled(ctx.content_rect(), 0, t.bg);
    match s.mode {
        Mode::Float(_) => {
            egui::Frame::new()
                .inner_margin(16)
                .show(ui, |ui| floating(ui, s));
            let _ = toast::host(ui.ctx(), "Dismiss");
            return;
        }
        Mode::Dialog => {
            egui::Frame::new().inner_margin(16).show(ui, buttons);
            let ctx = ui.ctx().clone();
            let mut open = s.dialog;
            Dialog::new("g-dialog", "Wipe every setting?")
                .description("Every preference, theme and saved route goes back to its default. Mods and the game are left alone.")
                .close_label("Close")
                .show(
                    &ctx,
                    &mut open,
                    |ui, _| {
                        ui.label("The body scrolls when it is long. It is written in the small text size.");
                        ui.add(Switch::new(&mut s.sw_on, "Keep my favourites"));
                    },
                    |ui, open| {
                        if ui.add(Button::new("Cancel").variant(Variant::Ghost)).clicked() {
                            *open = false;
                        }
                        ui.add(Button::new("Wipe everything").variant(Variant::Danger));
                    },
                );
            return;
        }
        Mode::Gallery => {}
    }
    egui::Frame::new().inner_margin(16).show(ui, |ui| {
        ui.columns(3, |cols| {
            if let [a, b, c] = cols {
                buttons(a);
                inputs(a, s);
                toggles(b, s);
                choices(b, s);
                marks(b);
                progress(c, s);
                frames(c, s);
                alerts(c);
            }
        });
    });
    let _ = s.preset;
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
                    let ctx = ui.ctx();
                    ctx.set_fonts(theme::fonts::definitions());
                    let tokens = presets::by_id(s.preset)
                        .map(|p| Tokens::derive(&p.raw, p.scheme))
                        .unwrap_or_default();
                    theme::apply(ctx, tokens);
                    kit::install(ctx);
                    s.frame += 1;
                    return;
                }
                s.frame += 1;
                gallery(ui, s);
            },
            State::new(mode, preset),
        );
    harness.input_mut().max_texture_side = Some(8192);
    for _ in 0..8 {
        harness.step();
    }
    if mode == Mode::Float(3) {
        // Hover the button with the tooltip and wait out the delay.
        let pos = harness.get_by_label("Hover me").rect().center();
        harness
            .input_mut()
            .events
            .push(egui::Event::PointerMoved(pos));
        for _ in 0..40 {
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

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "kit_gallery.png".to_owned());
    let stem = out.strip_suffix(".png").unwrap_or(&out).to_owned();
    let size = Vec2::new(1400.0, 1300.0);
    render(Mode::Gallery, presets::DEFAULT_DARK, size, &out);
    render(
        Mode::Gallery,
        presets::DEFAULT_LIGHT,
        size,
        &format!("{stem}-topo.png"),
    );
    for k in 0..4u8 {
        render(
            Mode::Float(k),
            presets::DEFAULT_DARK,
            Vec2::new(900.0, 760.0),
            &format!("{stem}-float{k}.png"),
        );
    }
    render(
        Mode::Dialog,
        presets::DEFAULT_DARK,
        Vec2::new(900.0, 600.0),
        &format!("{stem}-dialog.png"),
    );
}
