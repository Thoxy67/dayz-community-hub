//! App parts: `apps/gui/src/lib/components/app` as egui, built from the kit and dressed
//! from `ui.tokens()`. A part takes plain data (dz-hub DTOs, borrowed strings) and answers
//! with a `Response` or what the user did (an action enum); it calls no hub method, so a
//! view decides what a click means. The server row is the hot path: see [`server_row`].
//!
//! Part (Svelte)            | egui
//! -------------------------|----------------------------------------------------------------
//! ServerListRow            | `ServerListRow::new(RowData::from_row(&row)).selected(b).friends(&names).tag(RowTag{..}).extra(RowExtra{..}).buttons(&[RowButton{..}]).menu(&[RowMenuItem{..}]).mods_clickable(b).show(ui, &cols, &mut cache) -> RowResponse { response, action: Option<RowAction> }`
//! server-list.ts           | `ListColumns::new(x_range, wide)` (`.get(Col::Map)`, `.narrow()`), `ROW_PX`, `NARROW_PX`, `RowCache::new()` (owned by the list)
//! ServerMenu               | `server_menu(ui, ip, favorite, excluded, show_mods, &more) -> Option<RowAction>` (inside `kit::menu::context`)
//! TableHead / SortHead     | `TableHead::new(wide).show(ui, \|ui, cols\| head_cell(ui, cols.ping, \|ui\| ui.add(SortHead::new("Ping", active, asc).tip(t))))`, `head_label(ui, "Mods")`
//! ServerFlags              | `ServerFlags::new(Flags { password, first_person, battleye, saved_password, official, mimics_official })`
//! TimeOfDay                | `TimeOfDay::new("17:40").small()`; `day_phase(time) -> Icon`
//! OsIcon                   | `OsIcon::new("w").size(12.0)`; `os_brand(env)`, `os_label(env)`
//! ModsCount                | `ModsCount::new(24).missing(Some(0)).stale(1).clickable(true)`; `mods_tone`, `mods_tip`, `mods_extra`
//! PingButton               | `PingCell::new(Some(92)).pending(b).warn(b).size(SignalSize::Sm).bars_only()` (clicked = ping again)
//! PlayersButton            | `PlayersCell::new(41, 60).bots(3).loading(b).compact()` (clicked = query again)
//! FavoriteButton           | `FavoriteButton::new(on).size(StarSize::Md)` (pops once when starred)
//! ExcludeButton            | `ExcludeButton::new(ip, on).size(Size::Icon).variant(Variant::Default)`
//! JoinButton               | `JoinButton::new().size(Size::Lg).compact().quiet().lit(b).group_hover(b)`
//! ShortcutsButton          | `ShortcutsButton::new("?")` (clicked = open the sheet)
//! PageHeader               | `PageHeader::new("Servers").show(ui, \|ui\| stats, \|ui\| actions, Some(\|ui\| toolbar))`
//! Figure                   | `Figure::new("Players").amount(n).format(format::compact).value("54 ms").tone(c).tip(t).secondary().clickable().active(b)`
//! rolling.svelte.ts        | `rolling(ui, id, value) -> f64` (400ms ease-out, none with motion off)
//! Section                  | `Section::new("Mods").icon(i).first().show(ui, \|ui\| actions, \|ui\| body)`
//! SectionCard              | `SectionCard::new("Launch").description("…").icon(i).tone(c).padded().show(ui, \|ui\| actions, \|ui\| body)`
//! SettingsSection          | `SettingsSection::new("Theme").description("…").icon(i).danger().show(ui, \|ui\| aside, \|ui\| body) -> Response`
//! SectionNav               | `SectionNav::new("On this page", &[NavEntry{id,label,dot}], active).show(ui) -> Option<&str>`; `SectionNav::spy(top, &tops)`
//! BulkBar                  | `BulkBar::new("3 mods selected").info("1.2 GB").show(ui, \|ui\| lead, \|ui\| actions)`
//! Empty                    | `Empty::new().icon(i).title("…").text("…").compact().show(ui, \|ui\| action)`
//! MasterDetail             | `MasterDetail::new("id").detail_shown(b).selected(b).initial(400.0).range(320.0, 620.0).breakpoint(980.0).show(ui, main, detail)`
//! Lightbox                 | `Lightbox::new(image_source, "Close").show(ctx, &mut open)`
//! Facts                    | `ui.add(Facts::new(&[Fact::new("Players", "41").tone(c).tip(t)]).columns(2))`
//! DetailList / DetailRow   | `DetailList::new("id").show(ui, \|rows\| { rows.add(DetailRow::new("Map", "enoch").tone(c)); rows.custom("Address", \|ui\| …); })`
//! ModStatusList            | `ModStatusList::new(&mods).summary(b).show(ui) -> Option<u64>` (Workshop page to open); `ModRef::new(id, name, ModState::Missing, bytes)`, `ModRef::sort`
//! TransferProgress         | `ui.add(TransferProgress { label, name, percent, done, total, speed, eta, elapsed, labels: TransferLabels {..} })`
//! CommandPreview           | `CommandPreview::new("Command line", &flags, "No arguments", "Copy").program("DayZ_x64.exe").hint("…").show(ui)`
//! PlayerChart              | `ui.add(PlayerChart::new(&points).max(Some(60.0)).label("…").since("…"))` (points oldest first)
//! WeekHeat                 | `WeekHeat::new(&values, \|day, hour, v\| tip, "hint").now(day, hour).show(ui)`; `weekday_short(locale, day)`
//! ChartTip                 | `chart_tip(ctx, id, anchor_rect, "text")`
//! LogPane                  | `LogPane::new("id", &lines).times(&ms).title("steamcmd").live(b).classify(f).show(ui)`
//! log.ts                   | `classify_line(&str) -> LineKind` (`.color(&tokens)`), `Rate::new(5000.0)` (`add`, `per_second`), `clock(ms)`
//! SecretInput              | `ui.add(SecretInput::new(&mut key).placeholder("…").id_salt(id))`
//! PathInput                | `PathInput::new(&mut path).placeholder("…").show(ui) -> PathResponse { field, browse }`
//! ColorToken               | `ui.add(ColorToken::new("Accent", &mut raw.accent))` (`changed()`); `oklch_text`, `parse_color`
//! LanguagePicker           | `LanguagePicker::new(i18n::locale()).show(ui) -> Option<Locale>`
//! ThemePreview             | `ui.add(ThemePreview::new(&tokens))`; `theme_preview(painter, rect, &tokens, round)`
//! ThemeSwatches            | `ThemeSwatches::new(selected_id, (dark, light), "Follow the system").columns(4).show(ui) -> Option<ThemePick>`; `preset_name(p)`
//! PadGlyph                 | `ui.add(PadGlyph::new(PadButton::A, PadKind::PlayStation))`; `pad_text(button, kind)`
//! ShortcutList             | `ShortcutList::new(&key_groups, &pad_groups, &mut mode).tabs(b).pad_kind(k).show(ui)`; `chips("Ctrl+K")`
//! PadLegend                | `pad_legend(ui, &pad_groups, kind)`
//! SafeHtml                 | `let doc = HtmlDoc::parse(html, base)` (once); `SafeHtml::new(&doc).zoomable().show(ui, \|src\| image_source) -> Option<HtmlAction>`
//! map-name.ts              | `map_name("enoch") -> "Livonia"`
//! text helpers             | `ph`, `ph_trunc`, `draw` (galleys coloured when drawn), `is_cut`

pub mod cells;
pub mod charts;
pub mod facts;
pub mod header;
pub mod html;
pub mod inputs;
pub mod log_pane;
pub mod pad;
pub mod sections;
pub mod server_row;
pub mod text;
pub mod theme_cards;

pub use cells::{
    ExcludeButton, FavoriteButton, Flags, JoinButton, ModsCount, OsIcon, PingCell, PlayersCell,
    ServerFlags, ShortcutsButton, StarSize, TimeOfDay, day_phase, exclude_label, mods_extra,
    mods_tip, mods_tone, os_brand, os_label, ping_text, star_label,
};
pub use charts::{PlayerChart, WeekHeat, chart_tip, weekday_short};
pub use facts::{
    CommandPreview, DetailList, DetailRow, DetailRows, Fact, Facts, ModRef, ModState,
    ModStatusList, TransferLabels, TransferProgress,
};
pub use header::{Figure, PageHeader, rolling};
pub use html::{Block, HtmlAction, HtmlDoc, SafeHtml, Span, resolve_url};
pub use inputs::{
    ColorToken, LanguagePicker, PathInput, PathResponse, SecretInput, oklch_text, parse_color,
};
pub use log_pane::{LineKind, LogPane, Rate, classify_line};
pub use pad::{
    PadButton, PadGlyph, PadKind, ShortcutGroup, ShortcutList, ShortcutMode, ShortcutRow, chips,
    pad_legend, pad_text,
};
pub use sections::{
    BulkBar, Empty, Lightbox, MasterDetail, NavEntry, Section, SectionCard, SectionNav,
    SettingsSection,
};
pub use server_row::{
    Col, ListColumns, NARROW_PX, ROW_PX, RowAction, RowButton, RowCache, RowData, RowExtra,
    RowMenuItem, RowResponse, RowTag, ServerListRow, SortHead, TableHead, head_cell, head_label,
    server_menu,
};
pub use text::{clock, draw, is_cut, map_name, ph, ph_trunc};
pub use theme_cards::{ThemePick, ThemePreview, ThemeSwatches, preset_name, theme_preview};
