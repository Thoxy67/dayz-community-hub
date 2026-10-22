# Svelte → egui parity checklist

Generated from `apps/desktop/src/catalog.rs` (`cargo xtask parity`, or `cargo test -p dayz-community-hub catalog::tests::write_checklist -- --ignored`). Do not edit by hand.

**714 of 719 features done (99.3%).**

| View | Done | Total | % |
|---|---:|---:|---:|
| servers | 99 | 99 | 100% |
| favorites | 21 | 21 | 100% |
| history | 14 | 14 | 100% |
| friends | 47 | 47 | 100% |
| connect | 46 | 46 | 100% |
| offline | 27 | 27 | 100% |
| mods | 116 | 116 | 100% |
| options | 33 | 33 | 100% |
| news | 41 | 41 | 100% |
| stats | 52 | 52 | 100% |
| settings | 73 | 77 | 95% |
| about | 38 | 38 | 100% |
| setup | 28 | 28 | 100% |
| shell | 79 | 80 | 99% |

## servers (99/99)

| id | what | kind / command | |
|---|---|---|---|
| `servers.header.title` | Page header with display-face title 'Servers' (nav.servers); header is a size container: below 760px of its width the figures move to their own line under title+actions | display | ✅ |
| `servers.fig.shown` | Figure 'Shown' (statShown): number of servers matching the current query (feed.total), number rolls/animates to new value (rolling) | display | ✅ |
| `servers.fig.players` | Figure 'Players' (compact format e.g. 258,6 k, green): total players on shown servers; click sets sort=players descending; tooltip figSortPlayers 'Busiest servers first'; accent underline+accent label while that sort is active; dotted underline on hover | `servers.sort_busiest` | ✅ |
| `servers.fig.full` | Figure 'Full': count of full servers (red when >0, muted when 0); click toggles hideFull filter; tooltip figHideFull/figShowFull; active state while hideFull | `servers.toggle_hide_full` | ✅ |
| `servers.fig.empty` | Figure 'Empty' (secondary, muted; hidden below 1240px window width): count of empty servers; click toggles hideEmpty; tooltip figHideEmpty/figShowEmpty; active while hideEmpty | `servers.toggle_hide_empty` | ✅ |
| `servers.fig.modded` | Figure 'Modded' (mods colour): percentage modded/shown rounded, '—' when none shown; click toggles modded filter between 'only' and 'all'; tooltip figAllMods when only, else filterModsTitleAll; active while modded=only | `servers.toggle_modded_only` | ✅ |
| `servers.fig.pinged` | Figure 'Pinged' (secondary, muted, hidden <1240px): how many shown servers have a ping | display | ✅ |
| `servers.fig.best_ping` | Figure 'Best ping' (secondary, green): '<n> ms' or '—'; click sets sort=ping ascending; tooltip figSortPing 'Closest servers first'; active when that sort is in force | `servers.sort_closest` | ✅ |
| `servers.header.shortcuts` | Keyboard icon button in header actions opening the shortcuts sheet dialog (tooltip keys.sheet 'Show the shortcuts' with kbd '?') [keys: ?] | `sheet.toggle` | ✅ |
| `servers.header.toggle_detail` | Panel-right icon toggle button (accent filled when on) showing/hiding the details pane; tooltip 'Toggle details panel' with kbd I; default on [keys: I] | `servers.toggle_detail` | ✅ |
| `servers.header.refresh` | 'Refresh' button with refresh icon (tooltip 'Refresh server list'); disabled and icon spinning while refreshing; toasts info 'Refreshing server list…', then ok 'Loaded {count} servers' or error 'Refresh failed: {error}' [keys: Ctrl+R] | `servers.refresh` | ✅ |
| `servers.filter.search` | Search field (placeholder 'Search name, IP, map…', search icon, clear ✕ button 'Clear search'); debounced 200 ms (trimmed) before the query is sent; Esc empties then leaves the field [keys: Ctrl+F or /] | `servers.search` | ✅ |
| `servers.filter.first_person` | Tri-state chip '1PP': click cycles all → only (green border/bg + check icon) → none (red + X icon) → all; tooltip '1PP — 1P = First-Person Only servers… Click to show only 1P servers.' | `servers.filter.cycle_1pp` | ✅ |
| `servers.filter.password` | Tri-state chip with lock icon for 'Password protected' (compact: label hidden below 1500px window width, icon only); tooltip 'Click to hide password servers' | `servers.filter.cycle_password` | ✅ |
| `servers.filter.battleye` | Tri-state chip with shield-check icon 'BattlEye' (compact); tooltip 'Click to show only BattlEye servers' | `servers.filter.cycle_battleye` | ✅ |
| `servers.filter.modded` | Tri-state chip with puzzle icon 'Mods' (compact); tooltip 'Click to show only modded servers' | `servers.filter.cycle_modded` | ✅ |
| `servers.filter.official` | Tri-state chip with badge-check icon 'Official' (compact); tooltip 'Click to show only official servers', or when no Steam API key in profile 'Official servers are listed once a Steam API key is set in Settings' | `servers.filter.cycle_official` | ✅ |
| `servers.filter.map` | Map select (w-40, placeholder 'All maps'), options from backend server_maps as '<map> · <count>'; selecting filters to that map; null = all | `servers.filter.map` | ✅ |
| `servers.filter.more` | 'More filters' popover trigger (sliders icon; label hidden below 2xl width; accent border/text plus round accent badge with count of active hidden filters: maxPing, hideEmpty, hideFull, showExcluded); opens a 288px panel bottom-end | `servers.filter.more_popover` | ✅ |
| `servers.filter.max_ping` | In More filters: 'Max ping' segmented control (fill, xs) with options Any ping / ≤50 / ≤100 / ≤150 / ≤250; hides servers slower than the ceiling | `servers.filter.max_ping` | ✅ |
| `servers.filter.hide_empty` | In More filters: switch 'Hide empty' | `servers.filter.hide_empty` | ✅ |
| `servers.filter.hide_full` | In More filters: switch 'Hide full' | `servers.filter.hide_full` | ✅ |
| `servers.filter.show_excluded` | In More filters, only when the profile has excluded IPs: row button with eye/eye-off icon, text 'Click to reveal excluded · hold 1.5s to manage' / 'Click to hide excluded · hold 1.5s to manage', and the excluded IP count at the right; click toggles showExcluded (hold behaviour is NOT implemented, text only) | `servers.filter.toggle_excluded` | ✅ |
| `servers.toolbar.count` | Right end of the toolbar: '<shown> / <total>' in mono (shown in fg, locale-formatted), never wraps | display | ✅ |
| `servers.active_filters.chips` | Line under the toolbar (only when any filter active): one accent chip per filter in words — 'Search: “q”', 'Map: <map>', '<label>: yes' / '<label>: no' for each tri filter, 'Ping ≤ N ms', 'Hide empty', 'Hide full' — each with an ✕ (tooltip 'Remove the filter <text>') that resets just that filter | `servers.filter.remove_one` | ✅ |
| `servers.active_filters.clear` | 'Clear filters' button (filter-x icon, accent) after the chips; resets search, map, all tri filters, hideEmpty/hideFull, maxPing (not showExcluded, not sort) | `servers.filter.clear` | ✅ |
| `servers.sources.public` | Sources bar (under header, only when status known): 'PUBLIC LIST' cell with phase dot (idle faint, running accent pulsing, done green, failed red, off very faint), spinner while running, server count; tooltip dzsaHint | display | ✅ |
| `servers.sources.steam` | Steam cell: phase dot + 'STEAM' + via ('Web API' / 'Steam client'); while running a small meter of requests done/total plus 'd/t'; otherwise phase word; then either warning 'Steam is rationing requests: next in N s' (1 s countdown) or '+added · answered/queried A2S · listed addresses listed' (hidden <900px container width); clips instead of overflowing | display | ✅ |
| `servers.sources.kept` | 'KEPT' cell (only when kept.total>0): phase dot, alive (or total) count, red '−removed' when any; tooltip keptHint | display | ✅ |
| `servers.sources.in_list` | Right side: 'In the list <total>' · '<n> official' · '<n> duplicates removed' (tooltip duplicatesHint); official/duplicates hidden below 760px | display | ✅ |
| `servers.sources.details` | 'ⓘ Details' popover (320px) 'Where the list comes from': per source phase, hint, lines — public list count, fetched (relative), error; Steam via/phase, note (warn), requests, client assists, now asking (while running), addresses listed, farms, already in list, queried, answered, retried, modded-unreadable, fakes, +added (green), took (s or 'm min s s'); duplicates text; kept checked/total, still answer, removed, fakes | `servers.sources.details` | ✅ |
| `servers.stale_banner` | Warning strip with clock icon 'Server data is N minutes old' when last refresh >10 min ago (re-checked every 30 s, hidden while loading) with a 'Refresh' button at the right | `servers.refresh` | ✅ |
| `servers.table.head` | Sticky table heading row (h-7, stencil uppercase): [star] PING · PLAYERS · SERVER · MAP + TIME (together) · MODS · [actions]; grid columns 1.5rem/4.75rem/6.25rem/1fr/8.5rem/4.5rem/7.5rem; map column hidden when list <720px wide | display | ✅ |
| `servers.table.sort` | Each column heading (Ping, Players, Server, Map, Time with tooltip 'In-game server time', Mods) sorts server-side; clicking the active column flips direction, a new column starts ascending for name/map/ping and descending for players/time/mods; up/down arrow on the active one (accent), faint up-down arrow otherwise; default sort 'none' | `servers.sort_by` | ✅ |
| `servers.table.virtual` | Virtualised list of all matching servers, 48 px rows, overscan 8; rows fetched from backend in blocks of 100 (in-view blocks ±1), rows >4 blocks away evicted when >1200 held; not-yet-arrived rows draw a skeleton placeholder of the row shape | display | ✅ |
| `servers.table.loading` | First load of a request: 14 skeleton rows (role status 'Loading servers…') instead of a spinner | display | ✅ |
| `servers.table.error` | When backend has no list: empty state with cloud-off icon 'The server list could not be loaded', the error text in mono (selectable), and accent 'Refresh' button (spinning/disabled while refreshing) | `servers.refresh` | ✅ |
| `servers.table.no_match` | Empty state with search-x icon 'No servers match your search' over contour texture; accent 'Clear filters' button only when filters are active | `servers.filter.clear` | ✅ |
| `servers.row.favorite` | Star button in the first column: filled amber when favourite, faint otherwise; tooltip 'Add to favorites'/'Remove from favorites'; pops once when starred by this click; toast 'Added {name} to favorites' / 'Removed from favorites'; optimistic [keys: F] | `list.fav` | ✅ |
| `servers.row.ping` | Ping cell: 4-bar signal (4 bars <50ms green, 3 <100, 2 <180 amber, 1 red, 0 faint) and 'Nms' / '—' / red '×' on timeout (bars struck through); bars sweep while pending; click re-pings with manual timeout (tooltip 'Click to ping'); amber triangle when a 'full' count could not be verified ('Player count could not be verified - may be stale') [keys: P] | `list.ping` | ✅ |
| `servers.row.players` | Players cell: 'players /max' (tone: muted empty, green ≤50%, amber >50%, red full), fill bar beneath with hatched bot portion, bot icon+count with tooltip '{n} bots reported (often fake players)'; dimmed while querying; click asks the server directly via A2S (tooltip 'Click to refresh player count') | `servers.row.refresh_players` | ✅ |
| `servers.row.name` | Server name (semibold, truncated, tooltip full name) | display | ✅ |
| `servers.row.flags` | Flags after the name: green 'OFFICIAL' badge (tooltip officialTitle) or amber outlined '⚠ Looks official' (tooltip mimicsOfficialTitle); red lock if password; green key if saved password; amber '1PP' badge; blue shield if BattlEye | display | ✅ |
| `servers.row.friends` | Green badge with users icon and count when Steam friends are on that server; tooltip 'Friends here: names' | display | ✅ |
| `servers.row.address` | Second line: ip:gamePort as a copy button (copy glyph on hover, turns to green tick 1.4 s; tooltip 'Copy {address} to clipboard'), game version in mono, OS icon (Windows/Linux) | `servers.row.copy_address` | ✅ |
| `servers.row.map_time` | Map column: map id in map colour (tooltip) and in-game time with sun (7–19h)/moon/sunrise (dawn 5–7, dusk 19–21) icon; '—' when no map; hidden <720px | display | ✅ |
| `servers.row.mods` | Mods cell: puzzle icon + count; green when all installed & current, amber '↑n' when n stale, red '−n' when n missing, mods colour when unknown; tooltip e.g. '{missing} of its {count} mods are not installed: downloaded when you join' · '{stale} of your copies need an update' / 'Its {count} mods are installed and current: joins at once' / 'Show mod list'; click selects row and opens details on Mods tab; '—' when no mods | `servers.row.show_mods` | ✅ |
| `servers.row.select` | Click a row selects it (accent wash + accent left bar fading in); hover lightens; excluded rows at 55% opacity | `servers.row.select` | ✅ |
| `servers.row.dblclick_join` | Double-click a row joins the server | `list.join` | ✅ |
| `servers.row.hover_actions` | Framed group of xs icon buttons, visible on row hover/focus and always on the selected row: copy dzch link, open in Direct Connect, exclude IP | display | ✅ |
| `servers.row.copy_link` | Link icon: copies dzch:// link (tooltip 'Copy a dzch:// link to send to a friend'); toast 'Link copied' [keys: L] | `list.link` | ✅ |
| `servers.row.direct` | Plug-zap icon 'Open in Direct Connect': prefills Direct Connect with ip/game port/query port and switches to that view [keys: D] | `list.direct` | ✅ |
| `servers.row.exclude` | Ban icon (danger hover): excludes every server at that IP from the browser, red when already excluded (tooltip 'Exclude {ip} from list' / '{ip} is excluded — click to remove'); toasts '{ip} excluded from server list' / '{ip} removed from exclusions' | `servers.row.exclude_ip` | ✅ |
| `servers.row.join` | Play icon button at row end (quiet outline that fills accent on row hover or when selected); tooltip 'Launch DayZ and connect to this server' [keys: Enter] | `list.join` | ✅ |
| `servers.menu.connect` | Right-click context menu: bold 'Connect' (play icon) [keys: Enter] | `list.join` | ✅ |
| `servers.menu.details` | Context menu 'Details' (panel-right icon): selects the row [keys: I] | `servers.row.select` | ✅ |
| `servers.menu.show_mods` | Context menu 'Show mod list' (only when the server has mods) | `servers.row.show_mods` | ✅ |
| `servers.menu.favorite` | Context menu 'Add to favorites' / 'Remove from favorites' (star) [keys: F] | `list.fav` | ✅ |
| `servers.menu.ping` | Context menu 'Ping this server' (activity icon) [keys: P] | `list.ping` | ✅ |
| `servers.menu.copy_address` | Context menu 'Copy IP:port'; toast 'Copied · ip:port' or error toast [keys: C] | `list.copy` | ✅ |
| `servers.menu.copy_link` | Context menu 'Copy dzch:// link' [keys: L] | `list.link` | ✅ |
| `servers.menu.direct` | Context menu 'Open in Direct Connect' (passes saved password if any) [keys: D] | `list.direct` | ✅ |
| `servers.menu.exclude` | Context menu 'Exclude {ip} from list' (danger) / 'Stop excluding {ip}' (neutral); separators group connect/details/mods \| fav/ping \| copy/link/direct \| exclude \| list extras; row shows raised bg while its menu is open | `servers.row.exclude_ip` | ✅ |
| `servers.keys.move` | Arrow keys move the selection one row and scroll it into view (inactive while typing, a dialog or connect request is open, or another view shows) [keys: Up/Down] | `list.move` | ✅ |
| `servers.keys.page` | Move selection by a screenful of rows [keys: PgUp/PgDn] | `list.page` | ✅ |
| `servers.keys.edges` | Jump selection to first/last row [keys: Home/End] | `list.edges` | ✅ |
| `servers.keys.close` | Deselect, closing the details [keys: Esc] | `list.close` | ✅ |
| `servers.keys.row_actions` | On the selected server: Enter join (ignored when focus is on a button/link/menu), F favourite, P ping, D Direct Connect, C copy address, L copy link [keys: Enter/F/P/D/C/L] | `list.join` | ✅ |
| `servers.pad` | Gamepad: X favourite / Y ping the focused (or selected) row, Y refreshes the list when no row, R3 toggles details, D-pad/LT/RT move through the list | `list.fav` | ✅ |
| `servers.detail.empty` | Details pane with no selection: empty state server icon 'Select a server to see its players, mods and rules' (only shown when pane visible; pane hidden until something selected) | display | ✅ |
| `servers.detail.layout` | Master/detail: wide (≥980px) side-by-side resizable pane (initial 400, min 320, max 620, width remembered in dzch.prefs panes['servers-detail']; double-click or Home on splitter resets; arrows resize); narrow: details slide over the list's right edge (min(24rem,88%)) while selected | `servers.detail.resize` | ✅ |
| `servers.detail.hero.map` | Hero: map name large in map colour ('Chernarus+', 'Livonia'… or capitalised id), raw map id beside it when different, over a map-coloured gradient and contour lines; server name below (2 lines max, selectable) | display | ✅ |
| `servers.detail.hero.refresh` | Hero refresh button 'Ask the server again' (spinner while A2S query loading, disabled) | `servers.detail.refresh_live` | ✅ |
| `servers.detail.hero.close` | Hero ✕ 'Close the details': deselects | `list.close` | ✅ |
| `servers.detail.hero.meta` | Hero meta line: server flags + OS icon (or A2S official/1PP/BattlEye for unlisted), country flag + localized country name (DayZ Metrics), 'v<version>', red 'Excluded' when IP excluded | display | ✅ |
| `servers.detail.hero.live_strip` | Live strip box: PING (clickable md signal), PLAYERS (clickable players cell, plus amber 'Login queue · N waiting' when queue), TIME (time of day from A2S or list), MODS (clickable ModsCount → Mods tab, or plain count for unlisted); 2 or 4 columns by width | display | ✅ |
| `servers.detail.hero.population` | Compact population warning in hero: red shield 'The player count looks fake' or amber 'The player count looks suspicious' from DayZ Metrics verdict | display | ✅ |
| `servers.detail.hero.not_listed` | Amber note 'Not in the public server list: only what the server answers directly is shown.' for unlisted servers | display | ✅ |
| `servers.detail.tabs` | Tab strip Overview / Players (count) / Mods (count; icon red-dot when missing, amber-dot when stale) / Rules (count) / Stats (icon becomes fake/suspect warning); icons dropped under 480px except warnings; chosen tab kept when moving to another server (module-level) | `servers.detail.tab` | ✅ |
| `servers.detail.autofetch` | On showing a server: A2S query after 150 ms, server mod list after 180 ms (listed with mods), DayZ Metrics after 250 ms — debounced so arrowing through rows doesn't query every one; A2S cached 30 s, metrics 5 min, history/heatmap/rank 15 min | display | ✅ |
| `servers.detail.overview.connection` | Overview 'Connection' section: Address (copyable), 'Game port · Query port' values, 'Password required' (red) if password, 'Share link' dzch:// (copyable, includes mod ids; tooltip 'Opens this server in DayZ Community Hub'), and 'Open in Direct connect' button | `servers.detail.copy_address` | ✅ |
| `servers.detail.overview.facts` | Overview 'Details' facts grid (2 pairs/line): Map, Version, Platform, BattlEye, 1st-person, VAC, Hive (Official/Private/Public, tooltip hiveHint), Login queue, Time speed '×d day · ×n night', Whitelist, Slots, Real players, Bots (amber), Rank #n, Uptime % (green ≥90, amber ≥70, red), Next restart 'in 2 h 15 min', Next wipe 'in N d' (tooltip date · announced/predicted); red box 'The server did not answer the query.' on A2S failure | display | ✅ |
| `servers.detail.overview.mods` | Overview 'Mods' section: spinner 'Loading mods (N)…'; chips 'n ✓', 'n Missing', 'n Update'; 'See all N' link to Mods tab; accent 'Download N missing & join' / 'Update N & join' button, or green 'Everything this server runs is installed'; 'N mod(s) — fetch failed'; 'Reported by the server itself…' for unlisted; 'No mods' | `servers.detail.download_and_join` | ✅ |
| `servers.detail.overview.players` | Overview 'On the server now': skeleton while loading, 'LONGEST HERE' top 5 players with rank, name, duration; 'See all N' → Players tab; else 'No players online' / 'Player names not reported by server' / 'Click Refresh to query live data' / error | `servers.detail.tab` | ✅ |
| `servers.detail.overview.you` | Overview 'You and this server': 'Last played <relative>' (tooltip date) · '1 session…' / 'N sessions in your history' or 'Never played here yet'; star line 'In your favourites' / 'Not in your favourites'; population warning (full, with reasons) on top of overview when fake/suspect | display | ✅ |
| `servers.detail.players` | Players tab: green 'Friends here: …' banner; skeleton while loading; error empty state with 'Retry'; empty state 'No players online' / names not reported; search field 'Search players'; table # / Players / Score / Time here sorted longest first, with a faint bar behind each row proportional to time; 'No match' | `servers.detail.search_players` | ✅ |
| `servers.detail.mods` | Mods tab: filter chips 'All · n', 'Missing · n' (red), 'To update · n' (amber); refresh/retry icon for listed servers; 'Install N missing' (accent) and 'Update N' buttons; summary 'x installed · y missing · z to update' + 'size on disk'; list sorted missing→stale→ok then name with state badge, name, size, download glyph for missing, copyable Workshop id ('Copy the Workshop id'), 'Open on Steam Workshop' button; skeletons while loading; failed empty state with Retry; 'N mod(s) — not reported by server'; A2S-only plain name list; 'Nothing in this filter'; 'No mods' | `servers.detail.install_missing` | ✅ |
| `servers.detail.rules` | Rules tab: skeleton while loading, 'The server reported no rules' empty state, search 'Search rules' (matches name or value), two-column mono name/value list, each value copyable ('Copy the value'), 'No match' | `servers.detail.search_rules` | ✅ |
| `servers.detail.stats.header` | Stats tab header 'DAYZ METRICS' with online/offline status tag (dot) and a refresh button 'Refresh the stats' (spinner while loading) | `servers.detail.refresh_stats` | ✅ |
| `servers.detail.stats.figures` | Four big figures: Rank #n with month rank change (trending up green / down red, tooltip 'Rank change over the last month'), Uptime · 7 days % (toned), Peak · 7 days n/max, Average · 7 days with week-over-week % trend; full population warning above | display | ✅ |
| `servers.detail.stats.chart` | Segmented range 24 h / 7 d / 2 wk / 1 mo and player-count area chart (min/avg/peak readout, avg line, hover readout, 'now' value, left-edge label '24h ago'/'7 days ago'…); skeleton while longer range loads; error text | `servers.detail.stats_range` | ✅ |
| `servers.detail.stats.busy_hours` | 'Busiest hours' week heatmap (Mon-first, local time, current hour ringed), hover/keyboard cell tooltip '{day} {hour}:00 · {count} players'; hint text; only when any data | display | ✅ |
| `servers.detail.stats.schedule` | 'Restarts and wipes': next restart 'in …' · 'every N h', UTC slots, '% on schedule', amber unscheduled restarts, red restart loops; next wipe date · 'in N d' + announced/predicted tag; last wipe · 'N d ago' + Fresh/Mid/Late wipe tag; past wipes list (player surge) | display | ✅ |
| `servers.detail.stats.details` | Stats 'Details': country flag+name, Time speed, Style (vanilla band, tooltip score/100), Playstyle, Mods to download (bytes), Tracked since, Ping measured by the site 'lo–hi ms ±jitter'; 'Notices' bullet list | display | ✅ |
| `servers.detail.stats.links` | 'Links' section buttons (Discord, Website, others, deduplicated) opening the URL externally; footer 'Open on DayZ Metrics' link and 'Source: dayzmetrics.com'; error box with Retry; 'This server is not listed on DayZ Metrics' empty state | `servers.detail.open_link` | ✅ |
| `servers.detail.actionbar` | Panel foot always visible: wide 'Connect' play button, favourite star, 'Open in Direct connect', exclude IP, copy IP:port (icon turns green check 1.4 s) | `list.join` | ✅ |
| `servers.join.flow` | Joining: asks password if needed (favourite's saved one used; option to save), fetches mods (confirm 'Mods unknown' if it fails), shows missing/update mods dialog before launch; toasts 'Launching {name}…', 'Waiting for Steam to open DayZ…', or launch failed | display | ✅ |
| `servers.focus_from_other_view` | Another view (Friends 'Show in the server list', palette) can open the browser on one server: it is resolved, selected and details opened | display | ✅ |
| `servers.session_restore` | Search text, filters, sort, scroll, selected server and details visibility restored after a window reload (session 'servers.filters', 'servers.view'); filters survive leaving the view | display | ✅ |

## favorites (21/21)

| id | what | kind / command | |
|---|---|---|---|
| `favorites.header.title` | Page header 'Favorites' with figures | display | ✅ |
| `favorites.fig.saved` | Figure 'Saved' (accent): number of favourites | display | ✅ |
| `favorites.fig.in_list` | Figure 'In the list': 'listed/total' favourites present in current server list | display | ✅ |
| `favorites.fig.players` | Figure 'Players on them' (green): sum of players on listed favourites | display | ✅ |
| `favorites.fig.best_ping` | Figure 'Best ping': min ping 'N ms' or '—' (timeouts ≥5000 ignored) | display | ✅ |
| `favorites.fig.avg_ping` | Figure 'Average ping': rounded mean 'N ms' or '—' | display | ✅ |
| `favorites.header.shortcuts` | Keyboard icon opening the shortcuts sheet [keys: ?] | `sheet.toggle` | ✅ |
| `favorites.header.toggle_detail` | Panel-right toggle for the details pane (on by default, not persisted) [keys: I] | `favorites.toggle_detail` | ✅ |
| `favorites.toolbar.search` | Search 'Search name, address, map…' (immediate, client-side; matches name, ip:port, listed map) with clear ✕ [keys: Ctrl+F or /] | `favorites.search` | ✅ |
| `favorites.toolbar.listed_only` | Chip 'In the list only' (tooltip 'Hide servers missing from the current server list'), active state toggles | `favorites.toggle_listed_only` | ✅ |
| `favorites.toolbar.count` | 'shown / total' count at toolbar right | display | ✅ |
| `favorites.table.head` | Same columns as the browser (Ping, Players, Server, Map, Time, Mods) sortable client-side; default sort Server ascending; new column ascending for name/map/time/ping, descending for players/mods; ping timeouts sort last | `favorites.sort_by` | ✅ |
| `favorites.row` | Same row as browser (star, ping, players, name+flags incl. green key for saved password, address, version, OS, map/time, mods); unlisted favourites dimmed with amber 'Not listed' tag (tooltip serverOfflineHint) but still joinable | display | ✅ |
| `favorites.row.hover_actions` | Hover actions: copy dzch link, 'Open in Direct Connect (D)' (passes saved password), trash 'Remove from favorites' (danger) which asks to confirm 'Remove Favorite' / "Remove '{name}' from favorites?" | `favorites.remove` | ✅ |
| `favorites.row.select_join` | Click selects (details shown), double-click / Join button / Enter joins via address (resolves list entry, uses saved password) | `list.join` | ✅ |
| `favorites.row.context_menu` | Right-click: the shared server menu (Connect, Details, Show mods, Favourite, Ping, Copy IP:port, Copy link, Direct Connect, Exclude IP); no extra item for favourites | `list.join` | ✅ |
| `favorites.keys` | Same list keys as the browser plus Del removes the selected favourite (with confirmation); gamepad X/Y/R3 [keys: Up/Down, PgUp/PgDn, Home/End, Esc, Enter, F, P, D, C, L, I, Del] | `list.move` | ✅ |
| `favorites.ping_visible` | Rows on screen that have no ping yet are pinged automatically 200 ms after the visible range settles | display | ✅ |
| `favorites.empty` | No favourites: star empty state 'No favorites yet', hint 'Press F on any server in the browser to add it', accent 'Browse Servers' button going to the server browser | `view.servers` | ✅ |
| `favorites.no_match` | Search/filter yields nothing: compact empty 'Nothing matches the search' | display | ✅ |
| `favorites.detail` | Details pane = the same ServerDetail panel (hero, tabs, action bar); empty pane text 'Select a server to see who is on it, its mods and its rules'; resizable (initial 380, min 300, remembered as 'favorites-detail') | display | ✅ |

## history (14/14)

| id | what | kind / command | |
|---|---|---|---|
| `history.header.title` | Page header 'History' | display | ✅ |
| `history.fig.entries` | Figure 'Servers played' (accent): number of history entries | display | ✅ |
| `history.fig.last` | Figure 'Last session': relative time of newest entry (tooltip full date-time), '—' when none; re-evaluated every minute | display | ✅ |
| `history.fig.week` | Figure 'This week': entries in the last 7 days | display | ✅ |
| `history.fig.in_list` | Figures 'In the list' listed/total, 'Players on them' (green), 'Best ping' | display | ✅ |
| `history.header.clear_all` | Danger 'Clear all history' button (trash; tooltip 'Permanently remove all entries from connection history'), only when entries exist; confirms 'Clear History' / 'Clear all {count} history entries?'; toast 'History cleared' | `history.clear` | ✅ |
| `history.header.tools` | Shortcuts sheet button and details-pane toggle, before Clear all [keys: I] | `history.toggle_detail` | ✅ |
| `history.toolbar` | Search 'Search name, address, map…', 'In the list only' chip, 'shown / total' count (same as favourites) | `history.search` | ✅ |
| `history.table.last_played` | Extra sortable column 'LAST PLAYED' before actions; default sort last played descending; each row shows relative time ('10 minutes ago') and full date-time in mono below (tooltip date-time), refreshing each minute | `history.sort_by` | ✅ |
| `history.row.remove` | Hover trash 'Remove from history' and context-menu danger item 'Remove from history' (kbd Del); confirms 'Remove Entry' / "Remove '{name}' from history?"; toast 'Removed from history' [keys: Del] | `history.remove` | ✅ |
| `history.row` | Same shared row as favourites (star toggles favourite, ping, players, flags, 'Not listed' tag, mods, direct connect, copy link, join) | display | ✅ |
| `history.focus_last` | Rail 'Rejoin' card opening History (focus 'last'): resets search/filter, sorts by last played desc, selects newest entry and opens details | display | ✅ |
| `history.empty` | No history: history icon empty state 'No connection history yet', hint 'Every server you join is kept here, newest first, so you can go back in one click.', 'Browse Servers' button | `view.servers` | ✅ |
| `history.detail` | Same detail panel and empty-pane text as favourites; pane width remembered as 'history-detail' | display | ✅ |

## friends (47/47)

| id | what | kind / command | |
|---|---|---|---|
| `friends.header.title` | Page header titled with nav.friends ('Friends'), stencil display font | display | ✅ |
| `friends.header.stat.in_dayz` | Figure 'In DayZ' (friends.statInDayz): count of friends with in_dayz, accent tone, rolling number animation | display | ✅ |
| `friends.header.stat.online` | Figure 'Online' (statOnline): friends whose status != offline OR who are in a game, ok/green tone | display | ✅ |
| `friends.header.stat.total` | Figure 'Friends' (statTotal): total friend count, default fg tone | display | ✅ |
| `friends.header.source_client_tag` | Tag 'From the Steam client' (sourceClient) shown only when data.source == steam_client; tooltip sourceClientHint (no Web API key: Steam client is asked and shows DayZ running for a moment; a key adds avatars) | display | ✅ |
| `friends.header.updated_ago` | Faint text 'Updated {when}' (friends.updated) with relative time since last answer; tooltip 'From Steam's Web API' (sourceWeb) when source is web_api; hidden before first answer; re-rendered every 15 s while view shows | display | ✅ |
| `friends.header.refresh` | Button 'Refresh' with refresh icon: calls friends.load(); disabled while loading; icon spins while loading | `friends.refresh` | ✅ |
| `friends.search` | Search input (placeholder 'Search a friend, a server, a map', clear button); shown only when the friend list is non-empty; case-insensitive substring match on friend name, game name, server name (list row name or friend's server name) and map; trimmed; live filter, not persisted | `friends.search` | ✅ |
| `friends.state.loading` | Before first data and while loading: skeleton rows (10 rows with avatar/media placeholder) on panel background | display | ✅ |
| `friends.state.unavailable` | Error empty state when fetch failed (friends.failed or data.error) and list is empty: user-x icon, title 'Friends unavailable', red selectable error text, hint 'Start Steam, or add a Steam Web API key and your Steam id in the settings.' | display | ✅ |
| `friends.state.unavailable.retry` | In error state: 'Retry' button (spinning refresh icon while loading, disabled while loading) re-runs friends.load() | `friends.refresh` | ✅ |
| `friends.state.unavailable.open_settings` | In error state: accent button 'Open the settings' navigates to settings view, section 'apis' | `view.settings.apis` | ✅ |
| `friends.state.empty` | Empty state (users icon) 'No friends to show'; when Steam answered, hint 'Steam answered, with no friends: the list is empty, or your profile's friends list is private.'; action = Refresh button | display | ✅ |
| `friends.state.no_match` | When search filters out everyone: search-x icon empty state 'No friend matches' inside the list pane | display | ✅ |
| `friends.layout.master_detail` | Master/detail layout (id 'friends-detail', detail initial 380px, min 300, max 620): list on left, server detail on right only while a server is selected; width resizable and persisted in dzch.prefs panes; double-click splitter resets; below 980px own width the detail slides over the list's right edge as a drawer (min(24rem,88%)) | display | ✅ |
| `friends.list.scroller` | List pane is a pinned bounded scroller on panel bg; groups rendered in fixed order: Playing DayZ, In another game, Online, Away or busy, Offline; empty groups hidden | display | ✅ |
| `friends.group.header` | Sticky group header per section (section label with accent bar + uppercase label, right-aligned mono count in faint), sticks to top while scrolling | display | ✅ |
| `friends.group.dayz` | Group 'Playing DayZ' (groupDayz) with count of friends in DayZ; friends clustered by server ip:port into blocks: friend rows of the block, then the server row under them; blocks sorted by number of friends desc then first friend's name; friends within block by name (case-insensitive) | display | ✅ |
| `friends.group.dayz.lost` | Friends in DayZ without a known server listed at end of the DayZ group, sorted by name | display | ✅ |
| `friends.group.in_game` | Group 'In another game' (groupInGame): non-DayZ friends with a game, by name | display | ✅ |
| `friends.group.online` | Group 'Online' (groupOnline): no game, status online, by name | display | ✅ |
| `friends.group.away` | Group 'Away or busy' (groupAway): no game, status away/busy/snooze, by name | display | ✅ |
| `friends.group.offline` | Group 'Offline' (groupOffline): no game, status offline, sorted by last_online desc then name | display | ✅ |
| `friends.row.avatar` | Friend row (h-row height, hover raised bg): 32px avatar image (lazy) or fallback square with first letter uppercased (or '?') in display font | display | ✅ |
| `friends.row.status_dot` | Status dot at avatar bottom-right with panel ring: online=ok green, away/snooze=warn, busy=err, offline=faint; tooltip is status word (Online/Busy/Away/Snoozing/Offline) | display | ✅ |
| `friends.row.name` | Friend name, truncated, selectable text | display | ✅ |
| `friends.row.subline.dayz` | Subline for in-DayZ friends: accent 'DayZ', then '· In the menus, or their profile hides the server' (noServer) if no server, then '· joined {when}' (inDayzSince) if the session saw them arrive (friends.since) | display | ✅ |
| `friends.row.subline.game` | Subline for friends in another game: game name in ok green | display | ✅ |
| `friends.row.subline.last_online` | Subline for offline friends with last_online: 'Last online {when}' relative time | display | ✅ |
| `friends.row.subline.status` | Otherwise subline is status word: busy in err, online in default muted, away/snooze in warn | display | ✅ |
| `friends.row.offline_dim` | Offline friends with no game are drawn at 60% opacity | display | ✅ |
| `friends.row.send_message` | Hover/focus-revealed icon button 'Send a message on Steam' (message-square) opens steam://friends/message/<steamid>; errors shown as error toast | `friends.message` | ✅ |
| `friends.row.open_profile` | Hover/focus-revealed icon button 'Steam profile' (user-round) opens steam://url/SteamIDPage/<steamid>; errors as error toast | `friends.profile` | ✅ |
| `friends.server_row` | Server row under each DayZ block, indented (pl-6), list-row tall, using the shared ServerListRow: favorite star, ping, players bar, name + flags (password, 1PP, BattlEye, official, mimics), friends-here badge, copyable address, version, OS icon, map + time of day, mods count (missing/stale); values from the server list row if listed, else friend's server name/map or ip:port | display | ✅ |
| `friends.server_row.unlisted` | Server not in the public list: row dimmed (opacity 55%) and warn tag 'Server not in the list' (unlisted) after flags | display | ✅ |
| `friends.server_row.friends_badge` | Green users badge with count of friends on that server; tooltip 'Friends here: {names}' (comma-separated) | display | ✅ |
| `friends.server_row.select` | Click on server row selects it: accent left bar + accent/10 bg, opens ServerDetail in the detail pane | `friends.select_server` | ✅ |
| `friends.server_row.join` | Double-click row or Join (play) button joins via connect.address(ip, port) (password prompt, mods check dialog, launch) | `connect.address` | ✅ |
| `friends.server_row.copy_link` | Hover action 'copy link' (link icon, kbd L) copies dzch:// link; toast 'Link copied' | `servers.copy_link` | ✅ |
| `friends.server_row.show_in_list` | Listed server: hover icon 'Show in the server list' (list icon) navigates to servers view focused on server:<ip>:<port> | `servers.reveal` | ✅ |
| `friends.server_row.details` | Unlisted server: hover icon 'Ask the server and show its details' (scan-search) selects it and opens detail | `friends.select_server` | ✅ |
| `friends.server_row.direct_connect` | Hover icon 'Direct connect' (plug-zap) opens the server in Direct Connect view prefilled with game/query ports | `connect.open_in_direct` | ✅ |
| `friends.server_row.context_menu` | Right-click the server row opens the shared ServerMenu (every server action) | `servers.context_menu` | ✅ |
| `friends.detail.server` | Detail pane shows shared ServerDetail (ip, port, name) with a close button that deselects; when nothing selected, empty state 'Select a server' (servers.selectServer, server icon) | display | ✅ |
| `friends.auto_refresh` | Auto refresh cadence only while window focused: every 20 s on Friends view if a friend is in DayZ (web API), 60 s on the view otherwise or when source is steam_client, 120 s behind other views but only via Web API (key+steam id set) and only once it has worked; a failed background ask stops background asks until view opened; counts from end of last ask | display | ✅ |
| `friends.toast.arrival` | Toast when a friend starts DayZ or moves to another server between answers: '{name} is playing on {server}' (server name or ip:port) with 'Join' action, or '{name} started DayZ' without action | display | ✅ |
| `friends.toast.many` | More than 2 arrivals at once: single toast '{count} friends joined DayZ servers' with 'Show' action navigating to Friends view | display | ✅ |

## connect (46/46)

| id | what | kind / command | |
|---|---|---|---|
| `connect.header.title` | Page header 'Direct Connect' (connect.title) | display | ✅ |
| `connect.header.stat.players` | Figure 'Players' 'n/max' (from query answer, else listed row) in ok tone; header figures only when the server answered or is listed | display | ✅ |
| `connect.header.stat.ping` | Figure 'Ping': '—' when unknown, '×' when >=5000, else 'N ms', coloured by signal level | display | ✅ |
| `connect.header.stat.map` | Figure 'Map' with map name in map tone, '—' when none | display | ✅ |
| `connect.header.stat.mods` | Figure labelled 'Mods ({count})': value '<n> missing' in warn when any not installed, else count in mods tone | display | ✅ |
| `connect.header.open_dzch` | Button 'Open .dzch' (file-up icon, tooltip 'Open a server config file someone shared'): native file picker filtered to .dzch ('DayZ Community Hub server'), then connect.openDzch -> navigates to connect and joins; error toast 'Failed to open .dzch config: {error}' | `connect.open_dzch` | ✅ |
| `connect.header.copy_url` | Button 'Copy URL' (link icon, tooltip 'Copy a dzch:// URL to share this server'), disabled until form valid; copies dzch:// link with ip, game port, query port (if different), name, password, mod ids; toast 'Link copied' | `connect.copy_link` | ✅ |
| `connect.header.export` | Button 'Export .dzch' (file-down, tooltip 'Save a .dzch file for one-click connect'), disabled until valid: save dialog with default name '<sanitized server name, 60 chars>.dzch' or '<ip>_<port>.dzch'; writes v1 config (ip, port, query_port, name, password, mods); toast 'Saved {file}' or error 'Could not save the file: {error}' | `connect.export_dzch` | ✅ |
| `connect.layout` | Two columns: fixed 25rem left (Connection panel, Extra mods disclosure, Recent list panel or filler) and flexible right (server detail / querying / recent cards) | display | ✅ |
| `connect.form.panel` | Panel 'Connection' containing the form; form tooltip is the intro text 'Join any server by its address, even one missing from the public list...' | display | ✅ |
| `connect.form.favorite` | Panel toolbar star icon: 'Add to favorites' / 'Already in favorites' (active/filled when the address is a favourite on either port); disabled until valid; adds favourite with name (or ip:gamePort), ip, game port, password | `connect.favorite` | ✅ |
| `connect.form.clear` | Panel toolbar eraser icon 'Clear' (common.clear), disabled when address empty: resets address, port to 2302, query port, password, removes server-detected args (keeps manual ones), clears query results | `connect.clear` | ✅ |
| `connect.form.address` | Field 'IP / Hostname', mono, placeholder 'e.g. 192.168.1.1', tooltip 'Paste ip:port and the port is split off by itself.'; no autocomplete/spellcheck; on blur 'host:port' (or [ipv6]:port) is split into address + port | `connect.address` | ✅ |
| `connect.form.address.error` | After a submit/join attempt with empty address: red border and red text 'Enter an IP address or a host name.' | display | ✅ |
| `connect.form.password` | Field 'Password (optional)', masked, placeholder 'Leave blank if none'; typing clears the 'from favourites' marker | `connect.password` | ✅ |
| `connect.form.password.reveal` | Eye icon button toggles password visibility: 'Show the password' / 'Hide the password' (eye/eye-off); not persisted | `connect.toggle_password` | ✅ |
| `connect.form.password.from_favorite` | When the address matches a favourite with a saved password and the field is empty, it auto-fills and shows warn line with key icon 'Filled in from your favourites' | display | ✅ |
| `connect.form.ports.disclosure` | Collapsed disclosure 'Ports: {ports}' (game port, or 'game / query', or '—'); opens automatically when the port is invalid; chevron rotates when open | `connect.toggle_ports` | ✅ |
| `connect.form.port` | Number field 'Port (game)', 1-65535, default 2302; red border when invalid | `connect.port` | ✅ |
| `connect.form.query_port` | Number field 'Query port (optional)', placeholder 27016; hint 'Only when the server answers queries on a port of its own. Usually found by itself.' (replaced by red 'A port is a number between 1 and 65535.' when port invalid) | `connect.query_port` | ✅ |
| `connect.form.query` | Button 'Query' (search icon), also the form submit; disabled while querying or invalid; shows spinner + 'Querying…' while running; resolves listed entry, A2S query, fills game/query ports from answer, fetches server details/mods, pings once; only latest query wins; gamepad X [keys: Pad X] | `connect.query` | ✅ |
| `connect.form.connect` | Play button 'Connect' (wider, flex 1.4), tooltip 'Enter connects'; disabled when invalid or no profile; joins via connect.direct with password and enabled extra args (password prompt for protected listed servers, missing/update mods dialog, launch toasts 'Launching {address}…' then 'Waiting for Steam to open DayZ…' or 'Launch failed: {error}'); gamepad Y [keys: Enter (in address/password/port fields, not Shift+Enter); Pad Y] | `connect.join` | ✅ |
| `connect.args.disclosure` | Collapsed disclosure 'Extra Mods · {count} active' (count = enabled args); content max 45vh scrollable | `connect.toggle_args` | ✅ |
| `connect.args.hint` | Faint hint 'Add extra -mod entries not auto-detected by the server query.' | display | ✅ |
| `connect.args.empty` | No args: 'No modes configured. Query the server to auto-detect mods, or add manually.' | display | ✅ |
| `connect.args.row` | Arg row: label, mono value ('-mod=@<id>' for mods, verbatim for custom), tag 'detected' (accent, from server) / '-mod' / 'custom'; disabled rows at 50% opacity; list max 16rem scroll | display | ✅ |
| `connect.args.toggle` | Checkbox per arg enables/disables it (aria 'Disable'/'Enable') | `connect.arg_toggle` | ✅ |
| `connect.args.move_up` | Hover chevron-up 'Move up', disabled on first row | `connect.arg_up` | ✅ |
| `connect.args.move_down` | Hover chevron-down 'Move down', disabled on last row | `connect.arg_down` | ✅ |
| `connect.args.remove` | Hover X 'Remove' (red hover) removes the arg | `connect.arg_remove` | ✅ |
| `connect.args.auto_detect` | After a query, the server's mods become enabled 'detected' args first; detected ones no longer reported are dropped; existing detected kept with their state; manual args kept after | display | ✅ |
| `connect.args.pick_mod` | Select 'Pick installed mod…' listing installed mods not already added (sorted by name) + 'Add mod' button (disabled until picked); adds enabled -mod arg; replaced by 'No installed mods found. Install mods from the Mods tab first.' or 'All installed mods have been added.' | `connect.arg_add_mod` | ✅ |
| `connect.args.custom` | Mono input 'Custom argument' placeholder '-doLogs' + 'Add argument' button (Enter submits; disabled when blank); value trimmed; '-' prefixed at launch if missing | `connect.arg_add_custom` | ✅ |
| `connect.args.preview` | 'Launch args preview' (terminal icon) code block: '-connect=<ip\|…> -port=<gamePort\|…>' + ' -password=••••' if set + enabled args in mods tone; selectable, max 6rem scroll, break-all | display | ✅ |
| `connect.recent.panel` | Once a server is shown/queried/errored, left column shows Panel 'Recent servers' with compact recent list; otherwise an empty filler | display | ✅ |
| `connect.recent.cards` | Idle right pane: plug-zap heading 'Recent servers' + 'Type an address on the left, paste one anywhere in the window, or start from a server you know.' and grid of up to 24 cards (auto-fill min 15rem) | display | ✅ |
| `connect.recent.items` | Items: favourites first (filled warn star, aria 'Favourite') then history (history icon), deduped by ip:port; card shows name (2 lines), signal bars if listed, ip:gamePort, relative join time for history, map and players/max if listed; compact list caps at 14 items, one-line name + address + time + signal | display | ✅ |
| `connect.recent.pick` | Click a recent item fills the form (ip, game port, query port, saved password) and queries immediately | `connect.load_recent` | ✅ |
| `connect.recent.empty` | No favourites/history: compact empty state (history icon) 'Servers you join appear here.' with accent button 'Browse servers' (favorites.browseServers) navigating to servers | display | ✅ |
| `connect.detail` | Right pane shows shared ServerDetail for the listed entry (ip, query port) or the answered address, remounted when the target changes | display | ✅ |
| `connect.querying` | While querying without a target: centred accent spinner 'Querying server…' | display | ✅ |
| `connect.error` | Query failure: error banner alert (triangle icon) 'Query failed' with mono selectable error text above the right pane | display | ✅ |
| `connect.prefill` | Arriving via 'open in Direct Connect' (rows, menus, palette, paste) fills ip/port/query port/password and queries automatically | display | ✅ |
| `connect.form.persist_session` | Form state (address, ports, password, args, results) lives in a store and survives leaving and returning to the view (not across restarts) | display | ✅ |
| `connect.cli` | Command line --connect host:port goes to Direct Connect and joins; --reconnect rejoins last history entry or warns 'No history entry to reconnect to'; --open <dzch> opens a file; held until app initialised | display | ✅ |
| `connect.mods_unknown_confirm` | If a listed server's mods can't be read at join: confirm 'Mods unknown' / 'The mods {name} needs could not be read ({error}). Joining without them will probably fail.' with 'Join anyway' | display | ✅ |

## offline (27/27)

| id | what | kind / command | |
|---|---|---|---|
| `offline.header.title` | Page header 'Offline Missions' | display | ✅ |
| `offline.header.stat.state` | Figure 'State': 'Installed' (ok) when any mission, else 'Not installed' (faint) | display | ✅ |
| `offline.header.stat.missions` | Figure 'Missions' mission count, accent, rolling number | display | ✅ |
| `offline.header.stat.maps` | Figure 'Maps' distinct map count, map tone | display | ✅ |
| `offline.header.refresh` | Button 'Refresh' (servers.refresh word), tooltip 'Refresh mission list', disabled while loading/installing, icon spins while loading | `offline.refresh` | ✅ |
| `offline.header.explore` | Button 'Explore' (folder-open), tooltip 'Open the Missions folder in file manager'; errors ignored | `offline.open_missions_dir` | ✅ |
| `offline.header.install` | Accent button 'Install / Update' (download icon), 'Installing…' with spinner while running; disabled when busy; starts background download; gamepad Y [keys: Pad Y] | `offline.install` | ✅ |
| `offline.status_bar` | Status strip under header (role=status) tinted by tone: neutral/ok/warn/err; icon spinner (busy) / check (ok) / info (neutral) / alert (warn,err); truncated message; indeterminate sweeping progress bar while installing. Messages: '{n} missions available', 'No missions found. Click 'Install / Update' to download.', 'Downloading DayZCommunityOfflineMode…', offline updated / 'update failed {error}', removing/removed/cleared messages | display | ✅ |
| `offline.state.loading` | Before first load: 6 skeleton rows | display | ✅ |
| `offline.state.empty` | No missions: tent icon empty state 'No missions installed', 'Click Install to download DayZ Community Offline Mode', accent 'Install' button (disabled when busy) | display | ✅ |
| `offline.hint.doubleclick` | Stencil label above cards 'Double-click to launch directly' | display | ✅ |
| `offline.cards` | Grid of mission cards (auto-fill min 16rem), sorted by map name; hover strengthens border | display | ✅ |
| `offline.card.banner` | Card banner (96px) with contour/topo texture tinted map colour, big display-font map name (Chernarus+, Livonia, Sakhal, Namalsk, Deer Isle, Takistan+, Esseker, Banov, Pripyat or prettified folder part) and filled accent tag of mode | display | ✅ |
| `offline.card.terrain` | Row 'Terrain' with map icon and area (e.g. 225 km², '—' unknown) | display | ✅ |
| `offline.card.mode` | Row 'Mode': Offline / Co-op / PvP / Survival / Mission parsed from folder name | display | ✅ |
| `offline.card.save` | Row 'Save': '{size} · played {when}' (bytes formatted, relative time or '—') or faint 'None yet' | display | ✅ |
| `offline.card.folder` | Row 'Folder': mono truncated folder id, full id tooltip, selectable | display | ✅ |
| `offline.card.launch` | Play button 'Launch' (also card double-click; gamepad X on focused card): toast 'Launching offline: {map}', marks game launched; error toast 'Launch failed: {error}' [keys: Double-click card; Pad X] | `offline.launch` | ✅ |
| `offline.card.open_folder` | Icon button 'Open mission folder' | `offline.open_dir` | ✅ |
| `offline.card.clear_save` | Eraser icon 'Clear this map's save', disabled when busy or no save; danger confirm 'Clear the {map} save?' / 'Your character, the loot and the world of {map} start over. The mission stays installed.'; status '{map}: save cleared' or 'Clear failed: {error}'; optimistic removal from saves map | `offline.clear_save` | ✅ |
| `offline.card.remove` | Danger trash icon 'Remove this mission', disabled when busy; danger confirm 'Remove Mission' / 'Delete the mission "{mission}"? This cannot be undone.' / 'Remove'; status 'Removing {mission}…' then 'Removed {mission}' or 'Remove failed: {error}'; reloads list | `offline.remove_mission` | ✅ |
| `offline.aside.how` | Right 18rem aside, panel 'How it works': intro paragraph and 3 numbered steps (install from GitHub into Missions folder; launch with your launch options; saves per mission, Clear saves keeps missions) | display | ✅ |
| `offline.aside.project` | Link card 'Project page' with github.com/Arkensor/DayZCommunityOfflineMode opens the repo in the browser | `offline.open_project` | ✅ |
| `offline.aside.clear_saves` | Panel 'Maintenance': button 'Clear saves' (eraser) with caption explaining storage_-1 deletion; disabled when busy or not installed; danger confirm 'Clear Saves' with message; status 'Cleared saves for {n} mission(s)' / 'No saves found' / 'Clear failed' | `offline.clear_saves` | ✅ |
| `offline.aside.remove_all` | Danger button 'Remove all' (trash) with caption; disabled when busy or not installed; danger confirm 'Remove Offline Mode' / 'Delete all DayZCommunityOfflineMode mission folders? ...' / 'Remove all'; status 'Removing offline mode…' then 'Removed {n} mission folder(s)' / 'Nothing to remove' / 'Remove failed' | `offline.remove_all` | ✅ |
| `offline.install_events` | Background install finishes via event offline-mode-updated (status ok + toast offline updated, reload keeping status) or offline-mode-error (status err 'update failed: {error}'); listener registered once | display | ✅ |
| `offline.autoload` | Missions loaded on first view open (then saves loaded separately, failure tolerated); not re-loaded on revisits unless Refresh | display | ✅ |

## mods (116/116)

| id | what | kind / command | |
|---|---|---|---|
| `mods.header.title` | Page header titled 'Mods' (mods.title). | display | ✅ |
| `mods.header.stat.installed` | Figure 'Installed' = number of installed mods; the whole stats strip only shows when at least one mod is installed. | display | ✅ |
| `mods.header.stat.size` | Figure 'On disk' = total size of installed mods (human bytes, e.g. 25.0 GB). | display | ✅ |
| `mods.header.stat.updates` | Figure 'Updates' = count of mods with update_available; tone warn when >0 else ok; tooltip '{count} to update' or 'All mods are up to date'. Only when a Steam Web API key is set. | display | ✅ |
| `mods.header.stat.checked` | Secondary Figure 'Last checked': 'Checking…' while checking, relative time of last check (e.g. 'just now'), or 'never'. Only with API key. | display | ✅ |
| `mods.header.update_stale` | Accent button 'Update {count}' (refresh icon), only when stale mods exist; tooltip 'Update only the {count} mods with available updates'; opens the review dialog (kind update_stale). | `mods.update_stale` | ✅ |
| `mods.header.check_updates` | Button 'Check updates' (cloud-download icon; spinner + disabled while checking), tooltip 'Check Steam Workshop for updates'; ghost variant when stale mods exist; forces a Workshop check. Only with API key and ≥1 mod. | `mods.check_updates` | ✅ |
| `mods.header.install` | Button 'Install mod' (download icon), tooltip 'Install a mod manually by Workshop ID or URL'; opens the Install dialog. | `mods.install_dialog` | ✅ |
| `mods.header.more_menu` | '⋯' icon button (tooltip 'More actions') opening a dropdown with Rescan / Download all again / Open folder / separator / Cleanup. | `mods.more_menu` | ✅ |
| `mods.menu.rescan` | Menu item 'Read the folders again' (rotate icon): reload disk list, re-ask Steam, force update check; disabled while loading. | `mods.refresh` | ✅ |
| `mods.menu.redownload_all` | Menu item 'Download all again' (refresh icon): opens review dialog kind update_all (description 'Re-validate every mod'); disabled when no mods. | `mods.update_all` | ✅ |
| `mods.menu.open_workshop_dir` | Menu item 'Open Workshop mods directory in file manager' (folder icon). | `mods.open_workshop_dir` | ✅ |
| `mods.menu.cleanup` | Danger menu item 'Delete the launcher's downloads and game links' (brush icon); confirm dialog (title 'Remove all managed mods and symlinks', message about Steam library mods staying), then backend cleanup, reload, success toast with backend's result text; error toast 'Cleanup failed: …'. | `mods.cleanup` | ✅ |
| `mods.toolbar.search` | Search input (w-72, placeholder 'Search name or Workshop ID…', clear button) filtering by name substring (case-insensitive) or id substring; toolbar only shows when there is ≥1 row. | `mods.search` | ✅ |
| `mods.toolbar.filter` | Segmented control (aria 'Show'): 'All · N' (all rows incl. incoming), 'Updates · N' (stale + incoming Steam items), 'Unlinked · N' (installed not managed). Not persisted (session state, defaults All). | `mods.filter` | ✅ |
| `mods.toolbar.steam_error` | Warn tag 'Steam did not answer' with Steam icon and retry glyph, shown when Steam's subscription query failed; tooltip = error/reason; click re-asks Steam. | `mods.steam_retry` | ✅ |
| `mods.toolbar.steam_downloads` | Accent tag with spinner + Steam icon 'Steam: {count} downloading' when the Steam client has active/pending downloads; tooltip explains the list updates when they finish. | display | ✅ |
| `mods.toolbar.counter` | Mono counter 'shown / total' rows at the right end of the toolbar (e.g. 26 / 26). | display | ✅ |
| `mods.banner.api_key_missing` | Info banner (key icon) 'Update checks are off' / 'Add a Steam Web API key to see which mods are behind the Workshop.' when no API key and mods installed. | display | ✅ |
| `mods.banner.add_key` | Banner button 'Add a key' navigates to Settings, section steam. | `mods.open_settings_steam` | ✅ |
| `mods.list.loading` | Before the disk is read: table header (Name, Status) and 10 skeleton rows with varying widths (busy, 'Loading mods…'). | display | ✅ |
| `mods.list.empty` | No mods at all: Empty state (puzzle icon) 'No mods installed' / 'Connect to a modded server to install mods' with accent 'Install mod' button opening the Install dialog. | display | ✅ |
| `mods.list.no_match` | Filters exclude every row: Empty (search-x icon) 'No mod matches these filters.' with button 'Clear filters' (resets query and filter to All). | display | ✅ |
| `mods.list.virtual` | Virtualised list, 48px rows, header sticky, bounded scroller; rows keyed by id, group heads keyed g<status>. | display | ✅ |
| `mods.list.col.tick_all` | Header checkbox: ticks/unticks all visible installed rows; indeterminate when some ticked; aria 'Select all'/'Deselect all'. | `mods.tick_all` | ✅ |
| `mods.list.col.name` | Sortable column 'Name' (localeCompare); first click ascending. | `mods.sort.name` | ✅ |
| `mods.list.col.status` | Sortable column 'Status' (default sort, ascending): order Steam incoming(-1), updates(0), not checked(1), up to date(2), tie by name. Sorting by status shows group headings. | `mods.sort.status` | ✅ |
| `mods.list.col.in_game` | Sortable column 'In game' (linked first when ascending, then name); hidden when list container < 620px. | `mods.sort.managed` | ✅ |
| `mods.list.col.size` | Sortable column 'Size', right-aligned; first click descending. | `mods.sort.size` | ✅ |
| `mods.list.col.version` | Sortable column 'Your version' (local_updated date); first click descending; hidden when container < 820px. | `mods.sort.local` | ✅ |
| `mods.list.sort_toggle` | Clicking the active column flips direction; active column highlighted with arrow, inactive show ↑↓ hint. Sort is not persisted. | display | ✅ |
| `mods.list.group_head` | When sorted by status: group heading rows 'Coming through Steam' (accent), 'Updates waiting' (warn), 'Not checked', 'Up to date' (muted) with count and total size at right; chevron; click folds/unfolds (tooltip 'Fold or unfold this group'). Folded set is session-only. | `mods.group.fold` | ✅ |
| `mods.row.tick` | Per-row checkbox (installed rows only) to tick for bulk actions; ticked row gets raised background. | `mods.tick` | ✅ |
| `mods.row.select` | Click an installed row to open it in the detail pane (row highlighted accent/10). Incoming Steam rows are not clickable. | `mods.select` | ✅ |
| `mods.row.name` | Mod name (bold, truncated; muted for incoming rows). | display | ✅ |
| `mods.row.source_line` | Line under name: icon (Steam or hard-drive) + 'Launcher's folder' / 'Subscribed on Steam' / 'Steam library, not subscribed' / 'Steam library', then ' · also subscribed on Steam', ' · required by {name} +N', ' · in N favourite(s)' or ' · played on N server(s)'. Tooltip: source hint, 'An older copy is also in the other folder', 'Run by: names'. | display | ✅ |
| `mods.row.state` | Status tag, first match: 'Downloading… N%' (launcher op, spinner, accent) + bar; 'Waiting' (queued in op); 'Steam downloading N%' + bar; 'Waiting for Steam'; 'Update available' (warn) + 2nd line '{days} d behind' or 'behind'; 'Up to date' (ok); 'Not checked'. | display | ✅ |
| `mods.row.in_game` | Link icon + 'Linked' or unlink icon + 'Not linked' with tooltip explaining the game-folder symlink. | display | ✅ |
| `mods.row.size` | Size (size_human) right-aligned mono; incoming rows show bytes_total or '—'. | display | ✅ |
| `mods.row.version` | Local download date; tooltip 'On the Workshop: {remote date}' when known. | display | ✅ |
| `mods.row.update_button` | Accent icon button (refresh, label 'Update', kbd U) on stale rows not already downloading via launcher or Steam; opens review for that mod. | `mods.update_one` | ✅ |
| `mods.row.menu` | '⋯' button per installed row (tooltip 'More actions', side left) opening the mod menu. | `mods.row_menu` | ✅ |
| `mods.row.context_menu` | Right-click on an installed row opens the same items as a context menu; row shows raised bg while open. | `mods.row_context_menu` | ✅ |
| `mods.row.incoming_workshop` | Incoming (Steam-fetching, not on disk) rows: name from Workshop or 'Workshop {id}', 'Subscribed on Steam' line, state tag, external-link button 'Open on Steam Workshop'. | `mods.open_workshop` | ✅ |
| `mods.menuitem.update` | Menu item 'Update' (if stale) or 'Download again', kbd chip U; opens review dialog. [keys: U] | `mods.update_one` | ✅ |
| `mods.menuitem.repair` | Menu item 'Repair (check every file)' (wrench): starts repair operation immediately (no review). | `mods.repair` | ✅ |
| `mods.menuitem.link` | Menu item 'Link into the game' / 'Remove the game link', kbd M; toast '{name} linked (symlink created)' / '{name} unlinked (symlink removed)'; error 'Failed: …'. Updates row in place without rescan. [keys: M] | `mods.toggle_managed` | ✅ |
| `mods.menuitem.open_folder` | Menu item 'Open its folder'. | `mods.open_mod_dir` | ✅ |
| `mods.menuitem.open_workshop` | Menu item 'Open on Steam Workshop' (opens steamcommunity filedetails URL). | `mods.open_workshop` | ✅ |
| `mods.menuitem.copy_id` | Menu item 'Copy the Workshop ID' to clipboard. | `mods.copy_id` | ✅ |
| `mods.menuitem.delete` | Danger menu item 'Delete mod', kbd Del; confirm 'Delete Mod' / "Delete '{name}' ({id})? Size: … Your Steam account is unsubscribed from it."; toast 'Deleted {name}' or info that files stay in Steam library when unsubscribe failed. [keys: Del] | `mods.delete` | ✅ |
| `mods.bulk.bar` | Bulk bar at list foot (slides up) when ≥1 ticked: '{n} mod(s) selected' and '{size} on disk'. | display | ✅ |
| `mods.bulk.select_updates` | Ghost button 'Select all with updates' (when some stale mods are not ticked): ticks exactly the stale mods. | `mods.tick_stale` | ✅ |
| `mods.bulk.update` | Accent 'Update {n}' for ticked stale mods (only when >0); opens review. | `mods.bulk_update` | ✅ |
| `mods.bulk.redownload` | 'Download {n} again' re-validates all ticked mods (review dialog). | `mods.bulk_redownload` | ✅ |
| `mods.bulk.link` | 'Link {n}' for ticked unlinked mods (sequential toggles); shown when some ticked are unlinked. | `mods.bulk_link` | ✅ |
| `mods.bulk.unlink` | 'Unlink {n}' for ticked linked mods; shown when some are linked. | `mods.bulk_unlink` | ✅ |
| `mods.bulk.delete` | Danger 'Delete {n}': confirm 'Delete Mods' with count and total size, confirm label 'Delete {n}'; toasts 'Deleted {n} mod(s)' and info '{k} of them stay in your Steam library…'; error 'Delete failed: …'. | `mods.bulk_delete` | ✅ |
| `mods.bulk.clear` | X icon button 'Clear selection' unticks all. | `mods.clear_ticks` | ✅ |
| `mods.bulk.prune` | Ticks of mods that disappear (deleted) are pruned automatically. | display | ✅ |
| `mods.keys.move` | Arrow keys move the detail selection through visible (unfolded) rows, scrolling it into view; ignored while typing, in dialogs/menus, or when another view shows. [keys: ↑/↓] | `mods.move` | ✅ |
| `mods.keys.edges` | Jump selection to first/last visible row. [keys: Home/End] | `mods.edges` | ✅ |
| `mods.keys.tick` | Tick/untick the selected mod. [keys: Space] | `mods.tick` | ✅ |
| `mods.keys.managed` | Toggle game link of selected mod. [keys: M] | `mods.toggle_managed` | ✅ |
| `mods.keys.delete` | Delete selected mod (with confirm). [keys: Del] | `mods.delete` | ✅ |
| `mods.keys.update` | Review update of selected mod. [keys: U] | `mods.update_one` | ✅ |
| `mods.keys.close` | Close detail pane (deselect). [keys: Esc] | `mods.close_detail` | ✅ |
| `mods.pad.tick` | Gamepad X ticks the mod row under pad focus (or the open one). [keys: Pad X] | `mods.tick` | ✅ |
| `mods.pad.update` | Gamepad Y: review ticked mods, else the focused mod, else update all stale directly. [keys: Pad Y] | `mods.update` | ✅ |
| `mods.pad.back` | Gamepad B deselects (registry mods.close). [keys: Pad B] | `mods.close_detail` | ✅ |
| `mods.palette.search` | Command palette entry for a mod navigates to mods with focus 'search:<name>': fills search, resets filter to All. | `view.mods` | ✅ |
| `mods.layout.master_detail` | Detail pane right of the list (resizable splitter, initial 360px, min 300, max 520, width persisted under pane id 'mods-detail'; double-click/Home resets, arrow keys resize). Below 900px container width the detail slides over as a drawer (min(24rem,88%)). | display | ✅ |
| `mods.detail.header` | Detail header: mod name (selectable, wraps), state tag, source tag (accent when subscribed; tooltip hints), size. | display | ✅ |
| `mods.detail.close` | X icon button 'Close the details' (kbd Esc). | `mods.close_detail` | ✅ |
| `mods.detail.update` | Button 'Update' (accent when stale) or 'Download again'; disabled while the mod is in the running op; opens review. | `mods.update_one` | ✅ |
| `mods.detail.workshop` | Button 'Workshop' (external-link, tooltip 'Open on Steam Workshop'). | `mods.open_workshop` | ✅ |
| `mods.detail.open_folder` | Folder icon button 'Open its folder' in header (and again beside the path). | `mods.open_mod_dir` | ✅ |
| `mods.detail.version` | Section 'Version': 'Your copy' date (+relative if <30 days); 'On the Workshop' date (warn + '{days} d behind'/'behind' when stale, relative if recent, or 'Not checked'); note 'Steam updates this copy itself…' when subscribed. | display | ✅ |
| `mods.detail.where` | Section 'Where it is': source hint sentence, click-to-copy path, extra notes (subscribed hint for launcher copies also subscribed, 'unknown while Steam is not running' with reason tooltip, 'An older copy is also in the other folder'), meter of share of all mods + 'size · x.x%'. | display | ✅ |
| `mods.detail.dependencies` | Section 'Dependencies' (only when any): 'Requires' and 'Required by' lists of names, each tagged 'not installed' (err) or 'Subscribed'. | display | ✅ |
| `mods.detail.your_servers` | Section 'Your servers': favourites running it (filled star) then played servers (history icon); else 'None of your favourites or recent servers runs it: deleting it frees {size}.' | display | ✅ |
| `mods.detail.in_game` | Section 'In game': 'Linked'/'Not linked' label with mini switch (aria 'Loaded in game') toggling the link; hint about joining creating the link. | `mods.toggle_managed` | ✅ |
| `mods.detail.ids` | Section 'Identifiers': 'Workshop ID' and 'Launch parameter' (@id), each click-to-copy with tick feedback 1.4s. | `mods.copy_id` | ✅ |
| `mods.detail.repair` | Section 'Repair or remove': 'Repair (check every file)' (tooltip repairHint, disabled while busy). | `mods.repair` | ✅ |
| `mods.detail.delete` | Danger 'Delete mod' button + hint 'Deletes its files and the game link, and unsubscribes your Steam account from it.' | `mods.delete` | ✅ |
| `mods.install.dialog` | Install dialog 'Install Mod' / 'Download via Steam Workshop ID or URL': monospace textarea (6 rows, label 'Workshop IDs or URLs', hint 'one per line, or comma-separated'), cleared on open; accepts ids or URLs with ?id=, split by whitespace/comma/semicolon, deduped. | `mods.install` | ✅ |
| `mods.install.parsed` | Live parse: '{n} IDs found' + up to 12 id tags (accent if new, neutral with name tooltip if installed) + '+N'; 'No valid IDs detected.' (err) when text has none. | display | ✅ |
| `mods.install.submit` | Footer 'Cancel' and accent 'Install mod'/'Install {n} mods' (disabled with no ids): closes and opens review dialog kind install. | `mods.install` | ✅ |
| `mods.review.dialog` | Review dialog before any SteamCMD op: title 'Update {n} mods' / 'Install {n} mods', description 'Re-validate every mod' for update_all; size tag '{size} to check' or 'Size known after download'; list rows: name, #id, 'Local {date}', 'Remote {date}', size, 'already installed' tag, '{days} days newer' warn tag. | display | ✅ |
| `mods.review.open_workshop` | Per-row external-link button 'Open Steam Workshop page'. | `mods.open_workshop` | ✅ |
| `mods.review.warning` | Footnote: Steam-client mode -> info 'Steam downloads the mods and shows you as playing DayZ…'; SteamCMD mode -> warn triangle 'SteamCMD logs in with your account, which disconnects the Steam client…'. | display | ✅ |
| `mods.review.confirm` | Footer 'Cancel' (ghost) and accent 'Update {n} mods'/'Install {n} mods' starting the operation. | `mods.review_confirm` | ✅ |
| `mods.op.dialog` | Modal operation window (global, shown while an op is active and not minimised): overlay, clicking outside ignored; width 68rem with log, 30rem without; height min(46rem, 100vh-3rem). | display | ✅ |
| `mods.op.header` | Header: status icon (spinner / red X if failed or hint / green check), title by kind ('Updating mods','Installing mods','Repairing mods','Signing in to SteamCMD'), status line (Downloading: {name} / Preparing… / Waiting for Steam Guard… / Waiting for password… / Done — N mod(s) completed / Done — N OK, M failed / Login failed… / Could not download through Steam / Logged in…), coloured err/ok/muted. | display | ✅ |
| `mods.op.figures` | Figures 'Elapsed' (m:ss clock ticking every 500ms), 'Succeeded' (ok), 'Failed' (err when >0). | display | ✅ |
| `mods.op.overall` | 'Overall' bar (warn when failures) with 'x% · done/total'; includes current mod's fraction until it completes, capped at 100%. | display | ✅ |
| `mods.op.steam_guard` | Steam Guard panel: title, 'Open the Steam Mobile app…', 3 numbered steps, 'Waiting for confirmation…' with 65s countdown (warn ≤30, err ≤15) and bar, reset on log line containing 'retrying'; 'Hurry…' at ≤15s, 'SteamCMD may have timed out' at 0. | display | ✅ |
| `mods.op.password` | Password panel: 'Steam Password Required' + desc; password input (reveal toggle eye button 'Show the password'), accent 'Send' (spinner while sending, disabled when empty); Enter submits; cleared after send. | `mods.op.send_password` | ✅ |
| `mods.op.dont_trust` | Danger 'I don't trust this': copies `[HOME="<steamcmd home>"] steamcmd +login <login> +quit` and switches to manual panel: 'Login Manually via Terminal', the command, 4 steps, note, accent 'Got it, cancel operation' (cancels op). | `mods.op.manual_login` | ✅ |
| `mods.op.hint` | Error hint box (err, preformatted, selectable) when backend reports a hint. | display | ✅ |
| `mods.op.current` | 'Current mod' transfer block: name, percent (1 decimal), bar (indeterminate before first progress), Size done/total, Speed (5s window rate), ETA, Elapsed for this item; dashes when unknown. | display | ✅ |
| `mods.op.queue` | 'Queue' list with 'done/total': each mod with icon (spinner/check/X/circle), name, id · size, right figure (percent / per-item time / 'Failed' / 'Queued'); current row highlighted and auto-scrolled into view; queued rows dimmed. | display | ✅ |
| `mods.op.log` | Live log pane titled '$ steamcmd' or '$ steam (steamworks)': line count tag, LIVE pulse, per-line +m:ss timestamps, lines coloured by kind (err/warn/ok/progress/login/dim), blinking cursor while live, progress lines rewritten in place, capped at 5000 lines. | display | ✅ |
| `mods.op.log.filter` | Log filter input ('Filter lines') substring filter. | `log.filter` | ✅ |
| `mods.op.log.copy` | 'Copy' button copies all lines with [+m:ss] prefixes; shows 'Copied' for 1.5s. | `log.copy` | ✅ |
| `mods.op.log.follow` | Auto-follows the end; scrolling up stops following and shows a floating 'LIVE' button to jump back. | `log.follow` | ✅ |
| `mods.op.footer.note` | Footer note: Steamworks note or the SteamCMD disconnect warning. | display | ✅ |
| `mods.op.toggle_log` | 'Show the log'/'Hide the log' button (panel icons); session-only, default shown. | `mods.op.toggle_log` | ✅ |
| `mods.op.minimise` | While running: 'Keep running in the background' minimises (also Esc/close); status bar keeps showing it; a finished op behind a minimised dialog auto-dismisses. | `mods.op.minimise` | ✅ |
| `mods.op.cancel` | Danger 'Cancel' while running: cancels the backend op and dismisses. | `mods.op.cancel` | ✅ |
| `mods.op.dismiss` | Accent 'Dismiss' once finished: closes, re-checks updates (or reloads), re-asks Steam, reloads server stats. | `mods.op.dismiss` | ✅ |
| `mods.op.toasts` | Toasts at finish: 'Mods: N updated successfully' / 'Mods: N OK, M failed' (warn) / 'SteamCMD is logged in' / 'SteamCMD did not log in'; 'Mod operation failed: …' on start error; 'Failed to send password: …'. | display | ✅ |
| `mods.op.single` | Only one op at a time: starting another while busy just un-minimises the running one. | display | ✅ |
| `mods.op.resume` | After a reload, the running/finished op is re-attached from a backend snapshot (queue, results, log). | display | ✅ |
| `mods.auto.load` | On view mount: load disk list once and check updates (5 min TTL, needs API key); load usage in background; errors toast 'Failed to load mods: …' / 'Update check failed: …'. | display | ✅ |
| `mods.auto.steam_poll` | Steam subscription view asked when the mods view shows, every 5s while Steam downloads are active, every 15s while Steam did not answer; when a Steam download ends the disk is re-read; a failed answer keeps the last good one. | display | ✅ |

## options (33/33)

| id | what | kind / command | |
|---|---|---|---|
| `options.header.title` | Page header 'Launch options'. | display | ✅ |
| `options.header.stat.active` | Figure 'Active' = enabled option count (accent). | display | ✅ |
| `options.header.stat.with_value` | Figure 'With a value' = enabled options that have a value. | display | ✅ |
| `options.header.stat.available` | Figure 'Available' = total options (muted). | display | ✅ |
| `options.toolbar.search` | Search input 'Search options…' matching key, backend description, flag, translated label and description (case-insensitive). | `options.search` | ✅ |
| `options.toolbar.active_only` | Chip toggle 'Active only' hides disabled options; session-only; gamepad Y toggles. [keys: Pad Y] | `options.active_only` | ✅ |
| `options.empty` | No match: Empty (search-x) with noMatch({search}) title. | display | ✅ |
| `options.groups` | Options in cards by group, 1 column (2 at xl): Performance (gauge, accent), Window (monitor, info), Startup (rocket, ok), World (globe, map), Developer (code, mods), Other (sliders, muted; any unknown backend keys). Cards with no matching option hidden. | display | ✅ |
| `options.group.count` | Card header right: '{enabled}/{total} enabled' (counts all options of the group, not just filtered). | display | ✅ |
| `options.row.info` | Option row: icon (group tone when on, faint when off), label, flag chip (accent when on), description (2-line clamp, tooltip full); row dimmed (opacity 70) when off; hover bg. | display | ✅ |
| `options.row.toggle` | Mini switch toggles the option; optimistic update, rolled back with toast 'profileOptionToggleFailed' on error. | `options.toggle` | ✅ |
| `options.row.value` | Value input for valued options (number for mb/count/threads, w-24; text for text/folder, w-40), placeholder 'value'; committed on blur or Enter (trimmed; empty = null); Escape reverts draft; setting a value turns the option on (optimistic, rollback + toast on error). | `options.set_value` | ✅ |
| `options.row.unit` | 'MB' unit suffix inside the input for maxMem / maxVRAM. | display | ✅ |
| `options.row.threads_meaning` | For -exThreads, description gains ' · none' (0), ' · file' (1), ' · file + geometry + texture' (7) or ' · 0bXXX' bitmask. | display | ✅ |
| `options.row.use_recommended` | When a recommendation exists and differs: accent sparkle chip with the value (tooltip 'Use this value') sets it; when equal: green check (tooltip 'Matches the recommendation'). | `options.use_recommended` | ✅ |
| `options.row.browse` | Folder button 'Choose a folder' for folder options (-profiles): directory picker, sets value. | `options.browse_folder` | ✅ |
| `options.row.clear` | X button 'Clear the value' (visible on hover/focus, only when a value is set) clears the value. | `options.clear_value` | ✅ |
| `options.catalog` | 20 known options with flags: -window, -noborder, -nosplash, -skipIntro, -nolauncher, -nocharmenu, -high, -maxMem, -maxVRAM, -cpuCount, -exThreads, -noBenchmark, -world, -noPause, -filePatching, -doLogs, -scriptDebug, -buldozer, -winxp, -profiles; unknown keys shown as -key. | display | ✅ |
| `options.hardware.loading` | 'This computer' card shows spinner 'Detecting hardware…' while specs load; nothing if detection fails. | display | ✅ |
| `options.hardware.specs` | Three figures: Cores (physical), Threads (logical), Memory (GB, 1 decimal under 10 GB). | display | ✅ |
| `options.hardware.recs` | 'Detected {cores} cores · {ram} GB RAM — apply tuned performance flags.' and chips of recommended flags (-cpuCount=, -exThreads=, -maxMem=, -high, -noBenchmark), green with check when already applied; hint about physical cores / 60% memory. | display | ✅ |
| `options.hardware.apply` | Full-width accent 'Apply' (wand) sets every recommendation (value or enable); becomes disabled 'Applied' with check when all are in place. [keys: Pad X] | `options.apply_recommended` | ✅ |
| `options.command_line` | 'Command line' card: 'DayZ_x64' dimmed then enabled flags coloured (flag accent, '=' faint, value ok), wrapping between flags; empty text 'No flags: DayZ starts with its defaults.'; hint 'What DayZ receives at launch…'. | display | ✅ |
| `options.command_line.copy` | Copy icon 'Copy the command line' (flags only, no program), check for 1.4s; disabled when no flags. | `options.copy_command` | ✅ |
| `options.linux.card` | Linux only 'Launch on Linux' card with description about Proton. | display | ✅ |
| `options.linux.steam` | 'Steam': Native install / Flatpak / Snap / Not found, with launcher program path below. | display | ✅ |
| `options.linux.proton` | 'Compatibility tool': tool name + tag 'Steam's default' or 'Chosen for DayZ' (accent); else warn 'None set: is Steam Play on?'. | display | ✅ |
| `options.linux.prefix` | 'Proton prefix': click-to-copy path + hint about pfx/drive_c; or 'Not created yet…'. | `options.copy_prefix` | ✅ |
| `options.linux.command` | 'Command the launcher runs': program, applaunch args (-malloc=system accented), page flags (info colour); notes about -malloc=system and server address/mods added on join. | display | ✅ |
| `options.linux.command.copy` | Copy button copies the full Linux command; check for 1.4s. | `options.copy_linux_command` | ✅ |
| `options.linux.steam_options` | 'Launch options in Steam' parsed shell-like: Environment (NAME=value list), Run inside (wrappers), Added to the game (args), 'No %command%…' note; 'None set.' when empty; hint how to set in Steam. | display | ✅ |
| `options.linux.raw` | Disclosure 'As written in Steam' reveals the raw launch options string. | `options.linux_raw` | ✅ |
| `options.layout.aside` | Right column w-80 scrolling independently (hardware, command line, linux cards keep height); main grid scrolls. | display | ✅ |

## news (41/41)

| id | what | kind / command | |
|---|---|---|---|
| `news.header.title` | PageHeader titled with nav.news ("News"/"Actualités") in the display face | display | ✅ |
| `news.header.fig.articles` | Figure 'Articles' (news.statArticles) = total article count (all categories), rolling number animation | display | ✅ |
| `news.header.fig.latest` | Figure 'Latest' (news.latest) in accent colour: relative age of the newest article (e.g. 'just now', '2 weeks ago'); tooltip = full calendar date; hidden when no articles | display | ✅ |
| `news.header.open_all` | Icon button (external-link) with label/tooltip news.openAllOnSite 'Open every article on dayz.com': opens https://dayz.com/news in the system browser | `news.open_site` | ✅ |
| `news.header.refresh` | 'Refresh' button with refresh icon: force-reloads articles (news.load(true)); disabled while loading, icon spins while loading; resets selection to first article | `news.refresh` | ✅ |
| `news.filter.categories` | Row of category chips under the header: 'All' (news.allCategories) + one chip per distinct article category in order of first appearance; active chip highlighted; picking one filters the list and resets selection to 0. Row hidden when fewer than 2 categories | `news.filter_category` | ✅ |
| `news.load.auto` | Articles load automatically the first time the view mounts; not re-fetched on later visits unless forced (cached in the store); concurrent loads are ignored | display | ✅ |
| `news.list.loading` | While first loading (no articles yet) the list shows 9 skeleton rows with a media placeholder | display | ✅ |
| `news.list.row` | Left pane (22rem fixed width, own scroller, listbox) of article rows; click a row to select it and show it in the reader | `news.select` | ✅ |
| `news.list.row.selected` | Selected row: raised background + thin accent bar on the left edge; title in full fg colour (others muted); hover shows half-raised background | display | ✅ |
| `news.list.row.thumb` | 80x56 thumbnail per row (cover-cropped cached image); placeholder image icon while not cached or absent | display | ✅ |
| `news.list.row.latest_badge` | Stencilled accent 'LATEST' label on the first row, only when no category filter is active | display | ✅ |
| `news.list.row.category` | Uppercase category name (truncated) in the row's meta line | display | ✅ |
| `news.list.row.age` | Relative age right-aligned in mono in the row meta line; tooltip = full date | display | ✅ |
| `news.list.row.title` | Article title, semibold, clamped to 2 lines | display | ✅ |
| `news.list.row.excerpt` | Excerpt clamped to 1 line in faint text, when the article has one | display | ✅ |
| `news.list.empty` | Empty state (newspaper icon, compact) 'No articles yet' with a Refresh button (spins/disabled while loading) when the filtered list is empty and there is no error | display | ✅ |
| `news.reader.error` | When loading failed and no articles: reader shows cloud-off empty state 'The news could not be loaded' + hint 'dayz.com did not answer or refused the request…' + the raw error text in selectable mono | display | ✅ |
| `news.reader.retry` | Accent 'Try again' button in the error state: force reload | `news.refresh` | ✅ |
| `news.reader.no_selection` | Empty state 'Select an article to read' when nothing to show and not loading | display | ✅ |
| `news.reader.fade` | Each newly selected article fades in; the reader scrolls back to top on every selection change | display | ✅ |
| `news.reader.hero` | Hero image (h-72, cover) with bottom-to-top gradient into panel colour, title block overlapping its lower part; when no cached image, a short (h-24) topographic-lines banner instead. Hero image download is requested on selection | display | ✅ |
| `news.reader.hero.zoom` | Clicking the hero (zoom-in cursor, aria 'Enlarge the image') opens it in the lightbox | `news.zoom_image` | ✅ |
| `news.reader.category_tag` | Accent tag with the article's category above the title | display | ✅ |
| `news.reader.title` | Article title in large display face (3xl, tight leading), reader column max-width 3xl centered | display | ✅ |
| `news.reader.meta.date` | Calendar icon + formatted date | display | ✅ |
| `news.reader.meta.readtime` | Clock icon + '{minutes} min read' computed at 200 words/min from content_text or excerpt, minimum 1 | display | ✅ |
| `news.reader.meta.author` | Pen icon + 'By {author}' when the article has an author | display | ✅ |
| `news.reader.open_browser` | Ghost xs button 'Open in browser' right-aligned in the meta row: opens the article URL in the system browser | `news.open_article` | ✅ |
| `news.reader.excerpt` | Excerpt as an italic lead paragraph with an accent left border | display | ✅ |
| `news.reader.body_html` | Article HTML rendered sanitised (no script/style/iframe/object/embed/form/link/meta/base, no on* handlers, style or srcset attributes, no javascript: links) in prose styling (headings in display face, links, lists); text selectable | display | ✅ |
| `news.reader.body_links` | Clicking any link in the article body opens it (resolved against the article URL) in the system browser instead of navigating | `news.open_link` | ✅ |
| `news.reader.body_images` | Inline images are loaded through the backend cache (blank until resolved, hidden if download fails) | display | ✅ |
| `news.reader.body_image_zoom` | Clicking an inline image opens it in the lightbox; the shown version opens immediately and is swapped for the original/full-size one once downloaded | `news.zoom_image` | ✅ |
| `news.reader.body_text` | When there is no HTML: plain text content (pre-wrapped, selectable), or 'No content available.' | display | ✅ |
| `news.lightbox` | Full-window lightbox: dimmed backdrop, image contained, fade-in; round close (X) button top-right labelled 'Close' | display | ✅ |
| `news.lightbox.close` | Lightbox closes on click anywhere, on the X button, or on Escape [keys: Esc] | `news.close_lightbox` | ✅ |
| `news.key.move` | Previous/next article in the (filtered) list, clamped at ends; only while the news view shows, no dialog/lightbox open, not typing, no Ctrl/Alt/Meta [keys: Up/Down, J/K; gamepad D-pad] | `news.move` | ✅ |
| `news.key.open` | Open the selected article on dayz.com (shown in shortcut sheet as 'Open the article on dayz.com'; pad hint bar shows it when an article has a URL) [keys: O; gamepad X] | `news.open` | ✅ |
| `news.reader.pad_scroll` | Reader pane is the gamepad scroll target (data-pad-scroll) | display | ✅ |
| `news.images.cache` | Images: already-cached ones resolved in one batch when the list arrives, then missing thumbnails downloaded max 6 concurrently, deduplicated in-flight | display | ✅ |

## stats (52/52)

| id | what | kind / command | |
|---|---|---|---|
| `stats.header.title` | PageHeader titled nav.stats ('Stats'/'Statistiques') | display | ✅ |
| `stats.header.fig.time` | Figure 'Time played' = total duration for the range (e.g. '296h 55m'), accent colour | display | ✅ |
| `stats.header.fig.sessions` | Figure 'Sessions' = number of sessions (rolling number) | display | ✅ |
| `stats.header.fig.places` | Figure 'Servers' = number of distinct places played | display | ✅ |
| `stats.header.fig.average` | Secondary figure 'Average session' (duration or '—'); hidden when window narrower than 1240px | display | ✅ |
| `stats.header.fig.longest` | Secondary figure 'Longest' (duration or '—'); tooltip '{place name} · {date time}'; hidden below 1240px | display | ✅ |
| `stats.header.fig.streak` | Figure 'Streak' = '{n} d' (green when >0, muted when 0); tooltip 'Days in a row with play, ending today. Best: {best} d.' | display | ✅ |
| `stats.header.export` | 'Export the profile' button (file-down icon), tooltip exportHint: save dialog (default 'dayz-community-hub-profile.dchub', filter .dchub) then exports without mods; toast 'Profile exported successfully' or 'Export failed: {error}' | `profile.export` | ✅ |
| `stats.header.import` | 'Import a profile' button (file-up icon), same tooltip: open dialog (.dchub) → confirm dialog 'Import Profile' ('This will overwrite your current profile, favorites, history and launch options. The app will restart…') → import and restart app; error toast 'Import failed: …' | `profile.import` | ✅ |
| `stats.range` | Segmented control 'Period': 7 days / 30 days / 1 year / All time (default All time); changes reload figures, places, maps, calendar data, week heat (history not affected); not persisted | `stats.set_range` | ✅ |
| `stats.playing_now` | Green pill right of the range selector with pulsing radio icon: 'Playing {name} for {duration}' while a session is running; duration ticks every 30 s locally | display | ✅ |
| `stats.load.lazy` | Data is fetched first time the view becomes active; afterwards refetched (figures and history first page) on every play-sessions-changed backend event | display | ✅ |
| `stats.loading` | 10 skeleton rows while the first answer is pending (later reloads keep old data on screen) | display | ✅ |
| `stats.error` | Error state: chart icon empty state 'No play time yet' with the raw error in selectable mono | display | ✅ |
| `stats.empty` | Empty state when no session ever: 'No play time yet' + hint 'Join a server or an offline map: the launcher writes down how long DayZ runs, where and when.' | display | ✅ |
| `stats.empty.browse` | Accent 'Browse servers' (favorites.browseServers) button in empty state: goes to the Servers view | `view.servers` | ✅ |
| `stats.activity.section` | Section 'Activity' (calendar icon) with the year calendar heatmap (max width 64rem) and below it hint 'Click a day, a server or a map (or press Enter on it) to list its sessions below.' | display | ✅ |
| `stats.calendar.grid` | 53 week columns x 7 rows (Monday on top), square cells; colour = accent at opacity 0/0.3/0.5/0.75/1 by quartile of played days' seconds; future days blank; today ringed | display | ✅ |
| `stats.calendar.months` | Short month names above the column in which each month starts (first 7 days) | display | ✅ |
| `stats.calendar.span` | Span label between arrows: 'oct. 2025 – oct. 2026' (short month + year of first and last shown day) | display | ✅ |
| `stats.calendar.prev_year` | ‹ icon button 'The year before': shifts shown window back 52 weeks, not past the first session's week; disabled at the first session | `stats.calendar_prev` | ✅ |
| `stats.calendar.next_year` | › icon button 'The year after': shifts forward, capped at today; disabled when showing today | `stats.calendar_next` | ✅ |
| `stats.calendar.hover` | Hovering a day rings it (2px) and shows a tooltip: long date ('Monday 21 October 2026', first letter capitalised), time played in accent mono (or '—'), '{n} sessions'/'1 session' when any; cleared on pointer leave | display | ✅ |
| `stats.calendar.pick` | Click a day (or Enter on the keyboard-selected day) narrows the full history to that day and smooth-scrolls to the history section | `stats.filter_day` | ✅ |
| `stats.calendar.keys` | Calendar grid is focusable: focus selects today (or last shown day); arrows move the selection (never past today), moving off the left edge shifts the year back; focus ring; blur clears selection [keys: Left/Right (week), Up/Down (day), Enter] | `stats.calendar_navigate` | ✅ |
| `stats.calendar.legend` | Footer: hint 'Time played each day over the last year, in your time. Darker is longer.' and legend 'Less ▢▢▢▢▢ More' with the 5 opacity steps | display | ✅ |
| `stats.places.section` | Section 'Where you play' (map-pin icon), left column of a 1.6fr/1fr grid (stacks to one column below 1100px) | display | ✅ |
| `stats.places.empty` | 'Nothing played in this period.' when no places in the range | display | ✅ |
| `stats.places.row` | Ranked rows (rank number, name bold + 'Offline' accent tag for offline missions, sub-line: map name in map colour, '{n} sessions'/'1 session', '· {relative last played}', proportional accent bar vs top place (min 2%), duration or '—'); name tooltip = raw name; unknown place shows 'Started outside the launcher'; offline shows the mission's map | display | ✅ |
| `stats.places.unmeasured_tip` | Duration tooltip 'A join from before the launcher measured play time…' when the place has unmeasured sessions | display | ✅ |
| `stats.places.join` | Per server row: compact quiet Join (play) button — outline that fills on row hover; tooltip 'Launch DayZ and connect to this server'; connects to ip:port (does not trigger row click) | `connect.address` | ✅ |
| `stats.places.launch_offline` | Per offline row: play icon button labelled with the map name, launches that offline mission (click also bubbles to the row, narrowing history) | `offline.launch` | ✅ |
| `stats.places.pick` | Click a place row (or Enter/Space when focused) sets the history search to its name immediately and scrolls to history; rows focusable with focus ring | `stats.filter_text` | ✅ |
| `stats.places.tooltip` | Hover/focus tooltip on a place: name, duration (accent) + '{share} of your time' (% of range total), sessions + '~{duration} per session' (excluding unmeasured), 'first played {date}' | display | ✅ |
| `stats.places.show_all` | Only top 10 places shown; link button 'Show all {count}' / 'Show fewer' toggles (shown only if >10) | `stats.toggle_all_places` | ✅ |
| `stats.when.section` | Section 'When you play' (clock icon): 7x24 heatmap Monday-first, rows labelled with short weekday, hour axis labels 0/6/12/18; opacity 0.12+0.88*(v/max); current hour ringed; hint 'Time played by weekday and hour, in your time.' | display | ✅ |
| `stats.when.hover` | Hover/keyboard-selected cell ringed, its weekday and hour labels bold (hour label appears even off 6-step), same-row/column cells slightly dimmed; tooltip '{day} {hour}:00 · {duration or —}' | display | ✅ |
| `stats.when.keys` | Heatmap focusable: focus selects current day/hour, arrows move cell, Home/End jump to hour 0/23; blur/pointer leave clears [keys: Arrows, Home/End] | `stats.week_navigate` | ✅ |
| `stats.maps.section` | Section 'Maps': rows of map name (map colour), proportional bar vs top map, duration or '—'; 'Nothing played in this period.' when empty | display | ✅ |
| `stats.maps.tooltip` | Hover/focus tooltip on a map: map name, duration + '{share} of your time', sessions count | display | ✅ |
| `stats.maps.pick` | Click/Enter/Space on a map row sets the history search to the map id and scrolls to history | `stats.filter_text` | ✅ |
| `stats.history.section` | Section 'Full history' (history icon) with right-aligned count '{shown} of {total}' | display | ✅ |
| `stats.history.search` | Search field 'Search a server, an address, a map…' (24rem); queries backend with 200 ms debounce, first page again | `stats.history_search` | ✅ |
| `stats.history.day_chip` | When a day filter is set: accent chip 'Day: {long date}' | display | ✅ |
| `stats.history.day_chip.remove` | X in the day chip (tooltip 'Remove this filter') clears the day filter and reloads history | `stats.clear_day` | ✅ |
| `stats.history.row` | Session rows: start date-time in mono (tooltip relative age), name (+Offline tag, tooltip raw name), map name, duration right-aligned | display | ✅ |
| `stats.history.row.duration_states` | Duration cell: running session green '{duration} · running'; measured normal; unmeasured faint 'length unknown' with unmeasured tooltip | display | ✅ |
| `stats.history.row.forget` | Trash icon button 'Forget this session' (danger hover) appears on row hover/focus for finished sessions; deletes immediately without confirmation; list refreshes via event; error toast on failure | `stats.forget_session` | ✅ |
| `stats.history.row.join` | Per history row Join button (server) or offline launch button (offline mission), same as places | `connect.address` | ✅ |
| `stats.history.more` | 'Show {count} more' button appends the next page (pages of 100); hidden when all loaded | `stats.history_more` | ✅ |
| `stats.history.error` | History fetch/delete failures surface as an error toast | display | ✅ |
| `stats.tooltip.plate` | Chart tooltips sit above the hovered mark (below if no room), clamped to window, never capture the pointer | display | ✅ |

## settings (73/77)

| id | what | kind / command | |
|---|---|---|---|
| `settings.page.header` | PageHeader band titled with nav 'settings' word over a two-pane body: left section index (hidden below 1024px window width) and a scrolling form (max-width ~4xl, centred, 28px gap between sections, bottom padding so the unsaved bar never covers the last field) | display | ✅ |
| `settings.nav.index` | 'On this page' index of 12 sections: Account, Steam & SteamCMD, API keys, Location, Ping, Excluded IPs, Appearance, Language, Window, Controller, BattlEye, Profile data; click smooth-scrolls the section to the top | `settings.goto_section` | ✅ |
| `settings.nav.scrollspy` | Index highlights the section being read (last section whose top passed scroller top+24px) with raised background and a 2px accent bar on its left; updates on scroll | display | ✅ |
| `settings.nav.dirty_dot` | Small amber dot beside Account, Steam, API keys and Location index entries while the account form has unsaved edits | display | ✅ |
| `settings.nav.focus_deeplink` | Opening settings with a focus (title-bar account chip, theme menu 'Edit', status-bar Steam menu, palette settings items) jumps instantly (no smooth) to that section on next frame, then clears the focus | display | ✅ |
| `settings.section.anatomy` | Every section: accent icon, display-font title, one-line muted description, optional right-side 'aside' (status tag/count), then a bordered rounded panel of rows (Field = label + hint left, control right; stacked variant) | display | ✅ |
| `settings.form.unsaved_bar` | While account fields (name, Steam login, Steam root, SteamCMD path, API key, Steam ID, location) differ from the profile, a floating amber-bordered bar slides up at the bottom: warning icon + 'unsaved' text + Discard + Save | display | ✅ |
| `settings.form.save` | Save (accent, save icon; spinner while saving, both buttons disabled) writes all account fields at once (blank trimmed strings saved as null), reloads profile + avatar + stats, toast 'settings saved' or 'save failed: error' | `settings.account.save` | ✅ |
| `settings.form.discard` | Discard (ghost, undo icon) puts every account field back to the profile's values | `settings.account.discard` | ✅ |
| `settings.form.follow_profile` | Account form re-reads the profile whenever it changes, but only overwrites the fields if the user has not started editing (dirty = JSON compare against base) | display | ✅ |
| `settings.account.identity_card` | Account header row: 48px round avatar (Steam avatar image or user-round placeholder), name in display font (typed name or 'unnamed player'), mono line 'Steam: <login>' or 'no Steam linked' — reflects unsaved typing live | display | ✅ |
| `settings.account.ingame_name` | In-game name text field (hint: passed to DayZ via -name=), placeholder, autocomplete nickname; part of the account form | `settings.account.edit_name` | ✅ |
| `settings.steam.steamcmd_tag` | Steam section aside: spinner while detecting, green 'SteamCMD' tag with check if found, red with X if not | display | ✅ |
| `settings.steam.downloader` | 'Mod downloads' segmented control SteamCMD \| Steam client; applies immediately (optimistic, reload+error toast on failure); both options disabled while a mod operation runs | `settings.steam.set_downloader` | ✅ |
| `settings.steam.downloader_explain` | Paragraph under the choice explaining the selected downloader (steamcmdExplain or steamworksExplain) | display | ✅ |
| `settings.steam.steamworks_status` | With Steam client chosen: spinner while loading Steamworks status, then red 'library unavailable: error' (selectable), green 'Steam running', or amber 'Steam not running' | display | ✅ |
| `settings.steam.steamworks_test` | 'Test' button (plug icon, spinner while testing) runs steamworksCheck; toast ok 'steamworksTestOk' or error; disabled while testing, mods busy or library missing; re-reads status after | `settings.steam.test_steamworks` | ✅ |
| `settings.steam.server_list_switch` | 'Steam server list' switch 'Add Steam's servers' (default on), applied immediately (optimistic) | `settings.steam.toggle_steam_server_list` | ✅ |
| `settings.steam.username` | Steam username field (mono, autocomplete username, hint: blank downloads anonymously); part of account form | `settings.steam.edit_login` | ✅ |
| `settings.steam.login_state` | Login row: green check 'Logged in as <name>' when SteamCMD's cached login matches the saved login, else shield 'not logged in' or 'enter a username first'; note 'password never stored, asked once'; amber 'saved password pending' line when the profile has a legacy saved password | display | ✅ |
| `settings.steam.login_button` | 'Log in to SteamCMD' ghost button runs mods.login (opens mod-op dialog); disabled without saved login, without SteamCMD found, while mods busy, or while the form has unsaved edits | `mods.steamcmd_login` | ✅ |
| `settings.steam.steam_root` | Steam folder path field + 'Browse…' (folder picker, title selectSteamRoot), placeholder ~/.steam/steam; part of account form | `settings.steam.pick_steam_root` | ✅ |
| `settings.steam.steamcmd_path` | SteamCMD path field + Browse (file picker); placeholder is detected path or 'auto'; part of account form | `settings.steam.pick_steamcmd` | ✅ |
| `settings.steam.content_dir` | 'Content folder' row: SteamCMD's own content directory as a click-to-copy chip, and 'Open folder' button opening it in the file manager | `settings.steam.open_steamcmd_dir` | ✅ |
| `settings.steam.detect` | Detection row: green 'SteamCMD found' + copyable path, or red 'SteamCMD missing'; 'Detect again' button (disabled while detecting); detection runs on section mount | `settings.steam.detect_steamcmd` | ✅ |
| `settings.steam.install_windows` | When missing on Windows: explanation + accent 'Install SteamCMD' button (spinner 'Installing…') downloading it from Valve, then re-detects and reloads stats; error shown in red mono selectable text | `settings.steam.install_steamcmd` | ✅ |
| `settings.steam.install_linux` | When missing on Linux: hint and a grid of distro labels (Debian/Ubuntu, Arch, Fedora) each with a click-to-copy command (sudo apt install steamcmd / yay -S steamcmd / sudo dnf install steamcmd) | display | ✅ |
| `settings.apis.status_strip` | API keys section strip 'Steam API' with tag ✓ (ok, when both key and Steam ID filled) or — (neutral), plus right-aligned hint | display | ✅ |
| `settings.apis.key` | API key secret field (masked, eye button toggles show/hide with tooltip), placeholder 32 X's, and 'Get a key' link opening steamcommunity.com/dev/apikey externally | `settings.apis.edit_key` | ✅ |
| `settings.apis.steam_id` | Steam ID numeric mono field (placeholder 76561198000000000), 'Find my Steam ID' link to steamid.io | `settings.apis.edit_steam_id` | ✅ |
| `settings.apis.detect_id` | 'Detect' ghost button (scan icon, tooltip detectSteamIdTitle) fills the Steam ID from the running Steam client; error toast if it fails; disabled while detecting | `settings.apis.detect_steam_id` | ✅ |
| `settings.location.summary` | Location header: when set, a green circle with country flag (if detected) or pin, 'City, Country' or 'Location set', mono 'lat, lon' to 4 decimals and '· detected'; when unset, pin + 'no location' text | display | ✅ |
| `settings.location.open_maps` | 'Open in Maps' opens google.com/maps?q=lat,lon externally (only when a location is set) | `settings.location.open_maps` | ✅ |
| `settings.location.clear` | 'Clear' danger button removes the location (form edit, needs Save) | `settings.location.clear` | ✅ |
| `settings.location.autodetect` | 'Auto-detect from IP' accent button (spinner 'Detecting…') geolocates by IP and sets location + city/country/flag; error shown under the row | `settings.location.detect_ip` | ✅ |
| `settings.location.manual` | 'Manual' lat and lon mono fields (follow the current location) and 'Apply' (disabled unless both filled); validates -90..90 / -180..180 else red 'invalid location' error | `settings.location.apply_manual` | ✅ |
| `settings.ping.concurrency` | Concurrency slider 5–100 step 1 (default 64), value shown right; saved when the thumb is released (no toast) | `settings.ping.set_concurrency` | ✅ |
| `settings.ping.timeout_auto` | Automatic-scan timeout slider 1000–5000 ms step 250 (default 2000) shown as seconds ('2 s', '2.25 s'); saved on release | `settings.ping.set_timeout_auto` | ✅ |
| `settings.ping.timeout_manual` | Manual-ping timeout slider 1000–30000 ms step 500 (default 10000) shown in seconds; saved on release | `settings.ping.set_timeout_manual` | ✅ |
| `settings.ping.retries` | Max retries slider 0–5 (default 0), 0 shown as 'disabled' word; saved on release | `settings.ping.set_retries` | ✅ |
| `settings.ping.scope` | Scan scope: three joined switches '1. Favorites', '2. History', '3. Servers' (all default on), each saved immediately | `settings.ping.toggle_scope` | ✅ |
| `settings.excluded.count` | Excluded IPs aside tag: '1 IP' / 'N IPs' (amber when any, neutral when none) | display | ✅ |
| `settings.excluded.list` | List of excluded IPs (red ban icon, selectable mono IP); empty state centred 'no excluded IPs' | display | ✅ |
| `settings.excluded.remove` | X button per IP (tooltip 'Remove <ip>') un-excludes it immediately (optimistic) with toast 'ip unexcluded' | `settings.excluded.remove_ip` | ✅ |
| `settings.appearance.worn_tag` | Appearance aside: accent tag with worn theme name ('Follow system', custom name, or preset name) | display | ✅ |
| `settings.appearance.follow_system` | 'Follow system' toggle button (monitor icon, check + accent glow when selected) — theme follows OS dark/light | `theme.follow_system` | ✅ |
| `settings.appearance.pair` | 'When dark' and 'When light' selects (moon/sun icon, the one matching the OS scheme tinted accent) picking which dark/light preset 'follow system' wears | `theme.set_pair` | ✅ |
| `settings.appearance.preset_grid` | Preset cards grouped 'Dark' (moon) and 'Light' (sun), auto-fill grid min 9.5rem: each a ThemePreview mini-window in its colours, name, 2-line blurb, check + accent border/glow when worn; click wears it (18 presets: chernarus, topo, oled, github_dark, tokyonight, nord, forest, military, blood_moon, sepia, gruvbox_dark, catppuccin, sand, github_light, gruvbox_light, catppuccin_latte, rosepine_dawn…) | `theme.use_preset` | ✅ |
| `settings.appearance.preview_card` | ThemePreview: tiny title bar with accent mark, two server rows (second selected) with ping signal (ok/warn colours), players bar, map name, mods mark, and an accent play button, all from the theme tokens | display | ✅ |
| `settings.appearance.your_themes` | 'Your themes' block with hint; grid of custom theme cards (sub = Dark/Light) or 'no custom themes yet' text | display | ✅ |
| `settings.appearance.duplicate` | 'Duplicate' accent button (only when a preset, not a custom, is worn) copies the worn theme into a new custom theme named '<name> *' and wears it | `theme.duplicate` | ✅ |
| `settings.appearance.import` | 'Import' ghost button opens a .json file picker; adds it as a custom theme (name from file or filename minus .dzch-theme.json) and wears it; toast 'imported' or 'import error' | `theme.import_file` | ✅ |
| `settings.theme_editor.name` | Theme editor (shown when a custom theme is worn): name field, renamed on change/blur (empty ignored) | `theme.rename` | ✅ |
| `settings.theme_editor.scheme` | Dark \| Light segmented (moon/sun icons) setting the custom theme's scheme | `theme.set_scheme` | ✅ |
| `settings.theme_editor.derive` | 'Derive' button (wand icon, tooltip deriveHint) regenerates all colours and scheme from the background and accent | `theme.derive` | ✅ |
| `settings.theme_editor.reset` | Reset icon button (only if the custom has a base preset; label resetHint) restores the base preset's colours and scheme | `theme.reset_custom` | ✅ |
| `settings.theme_editor.copy_json` | Copy icon button (label 'Export') copies the theme JSON {version:2, app, name, scheme, tokens} to clipboard; toast 'theme copied' | `theme.export_clipboard` | ✅ |
| `settings.theme_editor.download` | Download icon button 'Export (.json)' saves '<slug-name>.dzch-theme.json'; toast 'exported' | `theme.export_file` | ✅ |
| `settings.theme_editor.delete` | Trash icon button (danger hover, label 'Remove') deletes the custom theme immediately (no confirm); if worn, falls back to follow-system | `theme.remove_custom` | ✅ |
| `settings.theme_editor.tokens` | Two-column grid of the 12 colour tokens (Background, Panel, Raised, Text, Accent, Accent text, OK, Warn, Error, Info, Map, Mods): swatch opening the system colour picker (hex converted to oklch), label, and an editable text value accepted only if a valid CSS colour (else reverts) | `theme.set_token` | ✅ |
| `settings.theme_editor.legibility` | Live 'Legibility' checks: header with amber warning + failing count or green check; hint; list of contrast checks each with an 'Aa' sample in its colours, label (Text/Muted/Faint on floor/panel/control, accent label, token on panel), ratio floored to 1 decimal and '/min' (amber when failing); plus 'Too alike: A · B' rows for confusable token pairs with split swatch | display | ⬜ |
| `settings.language.grid` | 5-column grid of language buttons (circle flag, native name, mono code EN/FR/DE/ES/RU, check when current, accent border); switches the whole UI immediately | `i18n.set_locale` | ✅ |
| `settings.window.radius` | Window corner radius slider 0–16 px, value 'N px'; applied live (0 when maximised), persisted with the theme | `window.set_radius` | ⬜ |
| `settings.window.border` | Window border width slider 0–4 px; applied live (not when maximised) | `window.set_border` | ⬜ |
| `settings.window.border_colours` | When border > 0: two colour tokens 'Border (focused)' and 'Border (unfocused)'; the window frame uses one or the other by window focus | `window.set_border_colours` | ⬜ |
| `settings.controller.status_tag` | Controller aside tag 'Connected' / 'No controller' (accent tone while in gamepad mode) | display | ✅ |
| `settings.controller.mode` | Controller mode segmented Auto \| Always \| Never (prefs.padMode); Never ignores pad input entirely, Always forces gamepad UI | `pad.set_mode` | ✅ |
| `settings.controller.scale` | Interface scale while a controller drives: 100 % \| 115 % (default) \| 130 % — zoom | `pad.set_scale` | ✅ |
| `settings.controller.glyphs` | Button labels segmented Auto \| Xbox \| PlayStation \| Nintendo \| Steam Deck (persisted as padGlyphs) | `pad.set_glyphs` | ✅ |
| `settings.controller.connected` | Connected pads list (gamepad icon + name each), else 'No controller' or 'unavailable' when pads cannot be opened; 'Steam keyboard available' line inside Steam big-screen | display | ✅ |
| `settings.controller.test` | Live button test: 14 glyphs (dpad, A, B, X, Y, LB, RB, LT, RT, Menu, View, L3, R3, RS); the button behind the last pad action lights (accent halo) for 450 ms | display | ✅ |
| `settings.controller.legend` | Stacked legend of every controller shortcut by group in the current pad's glyphs (2 columns, 1 below 900px), from the shortcut registry | display | ✅ |
| `settings.battleye.state` | BattlEye state line with coloured dot (green running, grey ready, amber otherwise) and the status-bar wording (running / disabled / missing / missing Proton / ready / ready Proton); amber text when missing or disabled; hidden when no game; refreshed on mount | display | ✅ |
| `settings.battleye.repair` | 'Repair BattlEye' danger button (wrench, spinner while repairing; disabled when no game or repairing): lists DayZ data folders outside its install, confirm dialog (danger, lists dirs or 'no data'), then deletes them and reinstalls BattlEye; toast ready/installing/still missing/failed | `battleye.repair_thorough` | ✅ |
| `settings.data.export` | Export profile: description '.dchub', 'include mods' checkbox (default on), accent Export button opens save dialog (default dayz-community-hub-profile.dchub); toast exported/failed | `profile.export` | ✅ |
| `settings.data.import` | Import profile: file picker (.dchub), confirm dialog, then imports and restarts the app; error toast on failure | `profile.import` | ✅ |
| `settings.data.reset` | 'Danger zone' red-tinted row: Reset profile danger button → danger confirm → reset and restart app | `profile.reset` | ✅ |

## about (38/38)

| id | what | kind / command | |
|---|---|---|---|
| `about.hero.logo` | Hero card on topographic background: app icon (64px), title 'DayZ Community Hub' with 'Community Hub' in accent, tagline 'Server browser & mod manager for DayZ Standalone' | display | ✅ |
| `about.hero.meta` | Mono line 'v{version} · MIT licence · Made with ♥ by Thoxy' (version '—' until known) | display | ✅ |
| `about.hero.features` | Feature pills with coloured icons: Server Browser, Mod Manager, Server stats, One-click Launch, Offline mode, Share with dzch://, Controller support; hidden below xl width | display | ✅ |
| `about.links.repo` | Link button 'Source repository' → https://git.thoxy.xyz/thoxy/dayz-community-hub | `about.open_repo` | ✅ |
| `about.links.releases` | Link button 'Releases' → repo /releases | `about.open_releases` | ✅ |
| `about.links.bug` | Link button 'Report a Bug' → repo /issues | `about.report_bug` | ✅ |
| `about.links.wiki` | Link button 'Wiki' → repo /wiki | `about.open_wiki` | ✅ |
| `about.links.license` | Link button 'License' → repo LICENSE file | `about.open_license` | ✅ |
| `about.update.card` | Update card: 'Updates' label + tag 'Version {version}'; border green when an update is available; glows with accent border for 2.5 s and is scrolled into view when opened from the title bar's update badge (focus 'update') | display | ✅ |
| `about.update.check` | Ghost 'Check for updates' button (icon spins while checking); disabled while checking or downloading; a check runs automatically when the view shows and the updater is idle | `updates.check` | ✅ |
| `about.update.checking` | Spinner + 'Checking for updates…' | display | ✅ |
| `about.update.up_to_date` | Green check 'You're up to date' + 'v{version} is the latest version' | display | ✅ |
| `about.update.available` | Download icon, 'Update available — v{new}' (new version in green display face), 'Current: v{cur} · Released {date}', release notes in a scrollable mono box (max 10rem, selectable) | display | ✅ |
| `about.update.install` | Large play-style 'Install update' button when this copy can update itself | `updates.install` | ✅ |
| `about.update.unsupported` | Instead of the button: 'This is a development build: updates are not installed from here.' or 'On this system, updates come through your package manager or a new release download.' | display | ✅ |
| `about.update.downloading` | 'Downloading v{version}…' + progress meter, bytes received / total and percent; download resumed/followed after a window reload (polled every 250 ms) | display | ✅ |
| `about.update.done` | 'Download complete' with 'The installer is launching — the app will restart automatically.' | display | ✅ |
| `about.update.restart` | Large 'Restart now' button after download completes: restarts the app | `updates.restart` | ✅ |
| `about.update.error` | Error state: red warning icon + error text (mono, selectable) | display | ✅ |
| `about.update.retry` | 'Retry' button in the error state runs a new check | `updates.check` | ✅ |
| `about.system.card` | Card 'This machine' (cpu icon): System (OS, if known), Processor (name + '{p} cores · {l} threads'), Memory ('{n} GB'), Graphics (each GPU name + '{n} GB VRAM' or 'No graphics card detected'); values selectable | display | ✅ |
| `about.system.loading` | Three pulsing placeholder bars until specs arrive | display | ✅ |
| `about.system.copy` | 'Copy for a bug report' button: copies lines 'DayZ Community Hub v…', OS, CPU (cores c/threads t), RAM, GPU(s), 'Mods: Steam client\|SteamCMD'; toast 'Copied: paste it into your bug report.' | `about.copy_system_info` | ✅ |
| `about.system.hint` | Hint 'Used to suggest -cpuCount, -exThreads and -maxMem in Launch options.' | display | ✅ |
| `about.sources.card` | Card 'Where the data comes from' (database icon): 7 entries with icon, title, description: Server list, Official servers, Live details (A2S, not clickable), Server stats (DayZ Metrics), Mods, Offline mode, News | display | ✅ |
| `about.sources.open` | Clicking a source with a URL opens it (dayzsalauncher.com, Steam store app 221100, dayzmetrics.com, Workshop, Arkensor's GitHub, dayz.com/news); external-link glyph appears on hover | `about.open_source_link` | ✅ |
| `about.keys.card` | Card 'Shortcuts' (keyboard icon) with the full shortcut list grouped by scope (Everywhere, Views in two columns with Ctrl+digit chips, Lists, Mods, News, palette, dialogs…), key chips joined by '+', alternatives with 'or' or '/'; footer note about typing and right-click | display | ✅ |
| `about.keys.mode` | Segmented Keyboard / Controller tabs in the shortcuts card; defaults to Controller when the gamepad is the active input; Controller mode shows pad glyphs for the connected pad kind and the pad note | `about.shortcuts_mode` | ✅ |
| `about.keys.pad_name` | Name of the first connected controller with gamepad icon under the list, when one is connected | display | ✅ |
| `about.guide.tabs` | Guide card (book icon) with tab strip: Quick Start, Mod workflow, Sharing & Quick Connect, Offline mode, Tips (default Quick Start; not persisted) | `about.guide_tab` | ✅ |
| `about.guide.quickstart` | Three numbered steps (circled numbers): 'Open account settings — …', 'Set your in-game name — …', 'Browse & connect — …' | display | ✅ |
| `about.guide.mods` | 'What is SteamCMD?' paragraph, 3 numbered mod-workflow steps, tip about switching SteamCMD/Steam client from the status bar | display | ✅ |
| `about.guide.offline` | Three numbered offline-mode steps | display | ✅ |
| `about.guide.share` | Sharing tab: explanation of dzch:// URLs and .dzch files, example box (Basic/With mods/Full), parameter list (qport, name, password, mods with descriptions), '.dzch files' paragraph and 'Sharing from Direct Connect' 3 steps; two columns on large widths | display | ✅ |
| `about.guide.share.copy_example` | Each example dzch:// URL is a copy button (wrapped text); copy glyph on hover, turns into green tick for 1.4 s; tooltip 'Copy'/'Copied' | `about.copy_example` | ✅ |
| `about.guide.tips` | Tips tab: 'AuthPlayerLoginState Error' 4 numbered steps and 'Performance' note | display | ✅ |
| `about.footer.stack` | Footer 'Built with' chips Tauri, Rust, Svelte, bits-ui, Tailwind CSS, intlayer — each opens its website (should become the egui stack) | `about.open_stack_link` | ✅ |
| `about.footer.license` | Right side of footer: 'Free and open source · MIT licence · Forgejo' | display | ✅ |

## setup (28/28)

| id | what | kind / command | |
|---|---|---|---|
| `setup.overlay` | Full-window modal wizard below the title bar (title bar stays usable), fade in; shown on first launch while the server list loads behind | display | ✅ |
| `setup.sidebar.brand` | Left 288px aside over topographic texture: app icon, 'DayZ / Community Hub' (accent second line) | display | ✅ |
| `setup.sidebar.steps` | Numbered step list Welcome, Game, Downloads, Account, Services ('optional' italic), Appearance, Done: current = accent filled circle + bold, done = green check, future dimmed; click jumps; only the next step is reachable and not if the current blocks advancing | `setup.go_step` | ✅ |
| `setup.sidebar.import` | Footer of aside: hint + 'Import profile' button (file picker .dchub → confirm → import and restart) | `profile.import` | ✅ |
| `setup.progress` | 2px progress bar across the top of the content pane, width (index+1)/7, eased | display | ✅ |
| `setup.header` | 'Step N of 7' stencil label, step title (3xl display), description (account description differs when Steam client chosen; done description names the 'Go to servers' button) | display | ✅ |
| `setup.footer.nav` | Footer: Back (ghost, from step 2), 'n / 7' counter, and accent large 'Get started' (welcome) / 'Next' with arrow, disabled when the step blocks (Account without Steam login when SteamCMD is chosen) | `setup.next` | ✅ |
| `setup.enter_key` | Enter anywhere in the wizard (except on a button/textarea) advances, or on Done finishes to servers [keys: Enter] | `setup.next` | ✅ |
| `setup.welcome.features` | Welcome: 2x2 cards with tinted icons — server (info), puzzle (mods), star (warn), play (accent) and their feature sentences | display | ✅ |
| `setup.welcome.language` | Language card: 5 flag buttons (radio group, name under flag, accent glow when chosen) switching the UI language at once; hint line below | `i18n.set_locale` | ✅ |
| `setup.game.status` | Game step status box: spinner 'Looking for DayZ…'; green 'DayZ found' + selectable path + 'N Workshop mods' (mods colour) if any; amber 'DayZ not found' + hint (not blocking); 'Rescan' button when done | display | ✅ |
| `setup.game.auto_find` | DayZ is searched automatically when the Game step first opens (all Steam libraries on all drives) | display | ✅ |
| `setup.game.library` | 'Steam library' card: path field + Browse (folder); placeholder = found steamapps or 'auto-detect'; OS-specific example path hint; typing re-searches 500 ms after typing stops (debounced) | `setup.pick_steam_library` | ✅ |
| `setup.downloads.choice` | Two radio cards SteamCMD (terminal icon) and Steam client (gamepad icon) with descriptions and radio dot; choosing Steam stops SteamCMD watching and checks Steam | `setup.choose_downloader` | ✅ |
| `setup.downloads.steam_status` | Steam client chosen: status box spinner, green 'Steam OK', or amber 'Steam closed' / 'library failed: error'; 'Check again' button | display | ✅ |
| `setup.steamcmd.status` | SteamCMD chosen: status box spinner 'Detecting SteamCMD…', green 'SteamCMD detected' + path, or amber 'not found' + 'Platform: Windows/Linux' + Rescan | display | ✅ |
| `setup.steamcmd.watch` | When SteamCMD is missing the backend keeps polling (steamcmd-detected event) and the step flips to found by itself, e.g. after installing in a terminal; pulsing radar 'waiting' line on Linux | display | ✅ |
| `setup.steamcmd.linux_cmds` | Linux: rows Arch/Manjaro, Debian/Ubuntu, Fedora/RHEL with mono command and copy button (turns into green check for 1.4 s) | `setup.copy_install_command` | ✅ |
| `setup.steamcmd.windows_install` | Windows: full-width accent 'Install automatically' (spinner 'Downloading…') into %APPDATA%\dayz-community-hub\steamcmd; red mono error on failure | `setup.download_steamcmd` | ✅ |
| `setup.steamcmd.path` | 'SteamCMD path' override card: path field + Browse (.exe filter on Windows); a picked path counts as found | `setup.pick_steamcmd` | ✅ |
| `setup.account.login` | Account step (SteamCMD): 'SteamCMD login' card with username field; green 'login once' note when filled, faint 'required' note when empty; with Steam client chosen a green 'login not needed' note replaces the card | `setup.edit_login` | ✅ |
| `setup.account.name` | In-game name card (hint about -name), and 'filled from Steam' green note when prefilled | `setup.edit_name` | ✅ |
| `setup.account.steam_prefill` | Entering Account or Services asks the running Steam client once and fills empty name and Steam ID | display | ✅ |
| `setup.services.api` | Services (optional): Steam API card with 'Get API key' link button, description, secret API key field, Steam ID numeric field | `setup.edit_api` | ✅ |
| `setup.appearance.swatches` | Theme card: 5-column grid of 'Follow system' (split dark/light preview with monitor icon) and every preset preview (dark first), check on worn, blurb as tooltip; applies immediately | `theme.use_preset` | ✅ |
| `setup.done.ready` | Done: box 'N servers ready' (green server icon) once the list loaded, else spinner 'still loading' | display | ✅ |
| `setup.done.summary` | Summary card rows (icon green check / amber warning / dashed grey): DayZ path or not found; Downloads (Steam client or 'SteamCMD · path'/missing); Username (only SteamCMD); In-game name or 'not set'; API key 'configured'/'not set'; Language; Theme; then hints about Settings and the About tab | display | ✅ |
| `setup.done.finish` | Done footer: 'Go to Mods', 'Go to Favorites' (only if favorites exist), and large play 'Go to servers' (spinner 'Saving…'); saves account + downloader, closes wizard, reloads profile/stats/avatar, opens the chosen view | `setup.finish` | ✅ |

## shell (79/80)

| id | what | kind / command | |
|---|---|---|---|
| `friends.rail.count` | Rail 'Friends' item shows count of friends in DayZ in ok green when > 0 | display | ✅ |
| `connect.paste_anywhere` | Pasting outside any text field: a dzch:// link opens it (join); an 'ipv4\|hostname:port' opens it in Direct Connect with info toast 'Pasted {address}: opened in Direct Connect' [keys: Ctrl+V] | `connect.paste` | ✅ |
| `shell.titlebar.brand` | Title bar left: app icon, 'DayZ Community Hub' (accent second half), divider, current view name in stencil | display | ✅ |
| `shell.titlebar.drag` | Dragging any non-control part of the title bar moves the window; double-click toggles maximise | `window.drag` | ✅ |
| `shell.titlebar.figures` | Centred on the window (hidden below 1180px): server count (info), players in-game (ok), players on Steam (muted, Steam icon), each with tooltip; numbers roll to new values (400 ms tween, none with reduced motion) | display | ✅ |
| `shell.titlebar.search` | Search pill (magnifier, 'Search…' text hidden <1024px, Ctrl K kbd) opens the command palette; tooltip 'Search (Ctrl+K)' [keys: Ctrl+K] | `palette.toggle` | ✅ |
| `shell.titlebar.account` | Account chip: avatar or user icon, player name and mono Steam login (login hidden <1280px), or italic 'Set up account'; tooltip 'Edit account settings'; opens Settings at Account | `settings.open_account` | ✅ |
| `shell.titlebar.language` | Flag button (current locale's flag, tooltip 'Change language') opening a menu of 5 languages: flag, name, code, check on current | `i18n.set_locale` | ✅ |
| `shell.titlebar.theme_menu` | Palette icon button (tooltip 'Change theme') menu: Follow system (check), 'When dark'/'When light' submenus showing current pair name and listing presets with swatches; Dark and Light groups; Custom group if any; 'Edit / custom theme…' opens Settings › Appearance. Each entry shows a 5-colour swatch strip (bg, panel, accent, ok, err) | `theme.menu` | ✅ |
| `shell.titlebar.theme_hover_preview` | Hovering a theme entry for 150 ms wears it as a preview (not saved); leaving, choosing or closing the menu ends it | display | ✅ |
| `shell.titlebar.window_buttons` | Minimise, Maximise/Restore (icon switches to 'copy' when maximised), Close (red hover) | `window.minimize_maximize_close` | ✅ |
| `shell.rail.groups` | Left rail with stencil group captions Play (Servers, Favorites, History, Friends, Direct connect, Offline), Gear (Mods, Launch options), Intel (News, Stats); foot: Settings, About; collapsed shows a short rule instead of captions | display | ✅ |
| `shell.rail.entry` | Each entry: icon (accent when current), label, raised background when current; click opens view; tooltip (when collapsed: label) with the view's shortcut key | `view.<id>` | ✅ |
| `shell.rail.counts` | Counts beside entries: servers total compact ('9,5 k'), favorites count, history count, mods 'N↑' amber when updates pending else installed count, friends in DayZ (green); pop animation when value changes; collapsed shows an amber dot for toned counts | display | ✅ |
| `shell.rail.active_bar` | A single 2px accent bar beside the current entry that slides to the new entry on navigation; no slide on first placement; re-measured on resize/scroll | display | ✅ |
| `shell.rail.collapse` | 'Collapse menu' / expand button at the foot toggles icon-only rail (persisted prefs.railCollapsed); auto-collapsed in gamepad mode when window < 1200px | `rail.toggle_collapse` | ✅ |
| `shell.rail.kill_game` | While DayZ runs (not in gamepad mode): red block at bottom of the views 'Game running' (pulsing green dot) + 'Close DayZ' button; collapsed: icon-only red button; danger confirm before killing; disabled while closing | `game.close` | ✅ |
| `shell.rail.rejoin_card` | Rejoin card above the foot: 'Last played' + relative time (tooltip exact date-time), close X, server name (click opens History at that entry), live players bar + ping signal (click pings it), map and in-game time, or amber 'not listed'; big accent 'Rejoin' button | display | ✅ |
| `shell.rail.rejoin_compact` | Short window (<760px high): single-line accent 'play + server name' button; collapsed rail: play icon button; both tooltip 'Rejoin: name' | `rejoin` | ✅ |
| `shell.rail.rejoin_dismiss` | X hides the rejoin card for that address until another server becomes the last played (prefs.dismissedRejoin) | `rejoin.dismiss` | ✅ |
| `shell.status.layout` | 28px status bar: left 'what is happening' segments, divider, right 'state of things'; words hide when the bar is <1100px wide, less important segments hide progressively | display | ✅ |
| `shell.status.segment` | Segment anatomy: tone-coloured icon (idle/accent/info/ok/warn/err), short text, tooltip; clickable ones are buttons with hover; optional 2px bar along the foot (value 0–100 eased, or sweeping band when indeterminate); optional pulsing icon; optional small action buttons beside | display | ✅ |
| `shell.status.list` | Server-list segment: rolling count (flashes when changed after first 1.5 s) + 'servers' + '· age'; tooltip with count and age; click refreshes. Amber when older than 10 min. Refreshing: accent sweep 'Fetching the list…'. Failed: red 'Fetch failed' (or cloud-off 'list down' when empty), click retries. Steam listing: accent bar 'Steam N%' with requests done/total/added tooltip, click opens Servers | `servers.refresh` | ✅ |
| `shell.status.scan` | Ping segment: idle 'Ping' (click pings all); running: pulsing radar 'Ping done/total' with progress bar, plus pause/resume and restart mini buttons (tooltips); paused shows amber 'Paused' | `servers.scan` | ✅ |
| `shell.status.mod_op` | Running mod operation: kind (Login/Repair/Install/Update) + 'cur/total' + current mod name, info bar of batch progress (sweep when unknown); amber key icon 'Waiting for you' when Steam Guard/password needed; click restores the minimised operation dialog | `mods.reopen_op` | ✅ |
| `shell.status.steamcmd_missing` | Amber 'SteamCMD missing' segment when SteamCMD is the downloader and not installed; click opens Settings › Steam | `settings.open_steam` | ✅ |
| `shell.status.mods_behind` | 'Checking mods' sweeping segment while checking; else amber 'N to update' (flash on change) opening Mods; hidden below 800px bar width | `view.mods` | ✅ |
| `shell.status.offline_install` | While offline mode installs: info sweep 'Offline mode · installing', click opens Offline | `view.offline` | ✅ |
| `shell.status.updater_download` | While the launcher update downloads: 'Update N%' info bar, tooltip version+percent, click opens About › update | `about.open_update` | ✅ |
| `shell.status.game` | While DayZ runs: green pulsing dot 'DayZ' + played duration (refreshed every 30 s); tooltip 'running for …' + how to quit (rail or Steam overlay with a pad) | display | ✅ |
| `shell.status.battleye` | BattlEye segment (hidden when no game; hidden <900px unless faulty): dot green running / grey ready; amber 'off' when missing/disabled; red shield 'off' when DayZ has run 20 s without the service; tooltips per state + action; click repairs/installs when applicable; refreshed with the 30 s clock while focused and every 5 s while DayZ runs | `battleye.repair` | ✅ |
| `shell.status.steam` | Steam segment: 'Steam'/'SteamCMD' + dot (green Steam running, amber not, grey unknown; amber tone when not running), tooltip login/state; click opens upward menu: 'Downloader' label, SteamCMD and Steam client items with check and hint (disabled 'busy' while an op runs), separator, 'Steam settings'. Steam running polled every 30 s while focused | `steam.downloader_menu` | ✅ |
| `shell.status.update` | Green 'vX available' segment when an update exists, or 'Restart to update' when downloaded; click opens About › update | `about.open_update` | ✅ |
| `shell.status.version` | App version 'vX.Y.Z' at the far right (hidden below 1000px bar width) | display | ✅ |
| `shell.confirm` | Confirm dialog for any confirm(): title, pre-line message, Cancel (ghost, focused) and confirm (danger red or accent; custom label); Esc/close = no; a second request answers the first 'no' | display | ✅ |
| `shell.password` | Server password dialog: message naming the server, masked field with eye toggle (focused, emptied each time), 'save password' checkbox (default on), Cancel and 'Join' (disabled when empty); Enter submits | `connect.answer_password` | ✅ |
| `shell.connect_dialog` | Before joining a modded server: title + server name, tag 'N mods not installed'/'N mods to check', scrollable list (name, #id, local and Workshop dates, size, amber 'N days newer' tag, open-in-Workshop button), switch install/update mods (default on for missing) with warning hint, amber 'will fail without mods' line when off; Cancel and play-button 'Install & connect'/'Update & connect'/'Connect' | `connect.confirm_mods` | ✅ |
| `shell.palette.open` | Command palette dialog (top 12vh, max 42rem wide, 72vh tall, dim overlay, pop animation); input auto-focused, fresh (empty, first item) every open; Esc closes [keys: Ctrl+K] | `palette.toggle` | ✅ |
| `shell.palette.empty` | Empty query: sections Recent (last 5 picks, persisted, max 8, servers rebuilt even if no longer in lists), Actions, Views; plus 'Try' box (search a server, type an address, anything else) | display | ✅ |
| `shell.palette.actions` | Actions: Refresh list (Ctrl+R), Rejoin <last server> (Ctrl+L, if history), Update mods (amber, count, Ctrl+U, only when updates), Check mods for updates (opens Mods + forced check), Keyboard shortcuts (?) | `palette.actions` | ✅ |
| `shell.palette.search` | Typed query: fuzzy-matches views, actions, 12 settings sections ('appearance' also matches 'theme colours'), favorites (name or ip:port, star icon), history (excluding favorites), installed mods (name or id; opens Mods filtered by name), max 5 per section, matched characters accent-underlined; sections ordered by best score | display | ✅ |
| `shell.palette.servers` | Servers section: backend query 120 ms after typing pauses (≥2 chars), 6 most-populated matches with 'players/max · map · ip:port'; spinner replaces magnifier and footer says 'Searching…' while asking | display | ✅ |
| `shell.palette.address` | Typing an address (host:port) puts first an Address section: 'Connect to <addr>' and 'Open <addr> in Direct connect' | `palette.connect_address` | ✅ |
| `shell.palette.row` | Row: icon (tinted), label, mono sub-text, active row raised with accent left bar and '<verb> ↵' (Open/Run/Join/Show), shortcut kbd on the right; hovering sets the active row; 'nothing for <query>' when empty | display | ✅ |
| `shell.palette.keys` | ↑/↓ move (wrap), PgUp/PgDn by 5, Tab/Shift+Tab jump to next/previous section start, Enter runs and records to recent; footer key hints ↑↓ move, Enter run, Tab section, Esc close | `palette.navigate` | ✅ |
| `shell.shortcuts_dialog` | Keyboard shortcuts dialog (large): Keys \| Controller tabs (default by input mode), groups in 2 columns: global, views (2-col chips), server lists, saved, mods, news, palette, dialogs (controller tab adds pad, servers, connect, offline, options); key chips joined by + with '/' or 'or'; footer note about typing/right-click or pad [keys: ?] | `sheet.toggle` | ✅ |
| `shell.pad_hints` | In gamepad mode, a 28px hint bar above the status bar: up to 5 glyph+word hints in fixed order (A select/type/join, B back/done/close, view's X and Y when applicable, Menu search, LB/RB views, R3, View shortcuts); palette shows A/B/LB-RB/(Y keyboard); dialogs show A, B, X confirm; first pad name at the right | display | ✅ |
| `shortcuts.global.palette` | Toggle command palette from anywhere, even over dialogs/fields (closes shortcut sheet) [keys: Ctrl+K / pad Menu] | `palette.toggle` | ✅ |
| `shortcuts.global.search` | Focus and select the current view's visible search field [keys: Ctrl+F or /] | `view.focus_search` | ✅ |
| `shortcuts.global.sheet` | Toggle keyboard shortcut sheet (closes palette) [keys: ? / pad View] | `sheet.toggle` | ✅ |
| `shortcuts.global.refresh` | Refresh the server list (browser reload swallowed) [keys: Ctrl+R] | `servers.refresh` | ✅ |
| `shortcuts.global.update_mods` | If mods are behind: open Mods and update them all; else toast 'mods up to date' [keys: Ctrl+U] | `mods.update_behind` | ✅ |
| `shortcuts.global.rejoin` | Rejoin last played server; toast warn 'no history' if none [keys: Ctrl+L / pad L3] | `rejoin` | ✅ |
| `shortcuts.views.jump` | Ctrl+1 Servers, 2 Favorites, 3 History, 4 Friends, 5 Direct connect, 6 Offline, 7 Mods, 8 Options, 9 News, 0 Stats, Ctrl+, Settings, F1 About; digits also match physical key (AZERTY) [keys: Ctrl+1..9, Ctrl+0, Ctrl+, , F1] | `view.<id>` | ✅ |
| `shortcuts.views.cycle` | Previous/next view in rail order, wrapping [keys: Ctrl+PgUp / Ctrl+PgDn, pad LB/RB] | `view.next` | ✅ |
| `shortcuts.list` | Registry list keys (handled by views): ↑↓ move, Home/End, PgUp/PgDn (LT/RT), Enter join (A), F favorite (X), P ping (Y), I details (R3), C copy address, L copy link, D direct connect, Esc deselect (B), Del remove (favorites/history); mods: ↑↓, Home/End, Space tick (X), U update (Y), M managed, Del delete, Esc; news: ↑↓/J/K, O open (X); pad-only: servers Y refresh, connect X query / Y join (Enter), offline X launch / Y install, options X apply recommended / Y active only; dialog X confirm, B close | display | ✅ |
| `shortcuts.escape` | Esc closes topmost overlay only; in a view's search field with text it clears it, otherwise leaves (blurs) the field [keys: Esc] | `field.escape` | ✅ |
| `shortcuts.field_guard` | Shortcuts without Ctrl (and not F-keys) never fire while a text field has focus; nothing but Ctrl+K fires while any dialog/popup/menu is open | display | ✅ |
| `shortcuts.browser_keys` | Release builds swallow browser keys: F5, F3, F7, BrowserBack/Forward, Alt+←/→, Ctrl+R/P/F/G/J/S/O/0 (Ctrl+R/F still run the app's own); in egui only the app's own keys remain | display | ✅ |
| `pad.move` | D-pad/stick moves focus spatially to nearest focusable in that direction within the topmost dialog/popup or the app; prefers staying in the current region (title bar, rail, status bar, nav/aside); in lists goes row by row (scrolls virtual list); held direction accelerates 1→3→10 rows; sliders take left/right; menus/selects/palette receive arrow keys | `pad.move` | ✅ |
| `pad.accept` | A clicks the focused element; on a text field focuses it and opens Steam's on-screen keyboard in Steam big-screen; on a list row first selects, second A on selected row joins (double-click); focus restored after redraw | `pad.accept` | ✅ |
| `pad.back` | B closes topmost popup/dialog; on a text field leaves it downward; else sends Escape to the view (deselect/close details) | `pad.back` | ✅ |
| `pad.page_scroll` | LT/RT page a list by its page size (else 10) or scroll the focused scroller by 85% height, refocusing what is on screen; right stick scrolls the reading pane by 140px | `pad.page` | ✅ |
| `pad.landing` | When nothing is focused, a press lands on the element last focused in this view, else the view's list at its remembered/selected row, else the top-left focusable; switching view with LB/RB waits for it to load then lands | display | ✅ |
| `pad.view_buttons` | X/Y/R3/L3/Menu/View run the current view's registered command when it applies (first applicable wins), else app-wide (Menu palette, View sheet, L3 rejoin); in a dialog X clicks the confirm (or last footer button); in the palette Menu closes, Y opens Steam keyboard, LB/RB = Shift+Tab/Tab | display | ✅ |
| `pad.input_mode` | Input mode follows last input (pad action → gamepad; real pointer move/press → pointer) unless forced; gamepad mode shows focus ring, hint bar, zoom at pad scale, hides rail kill-game block; Steam big-screen starts in gamepad mode | display | ✅ |
| `app.init_screen` | Until initialised the main area shows topo texture with spinner 'Initializing…'; on failure red triangle 'Initialization Failed', selectable mono error and 'Retry' | display | ✅ |
| `app.views_kept` | Views load on first visit and stay mounted (scroll, filters, selection kept); shown with a short rise-in animation; current view restored after a reload (session) | display | ✅ |
| `app.paste_connect` | Pasting outside a text field: 'dzch://' link opens it in Direct connect; 'host:port' (IPv4 or hostname, port 1–65535) opens Direct connect prefilled with toast 'pasted address' | `connect.paste` | ✅ |
| `app.no_context_menu` | Release builds suppress the webview's right-click menu except on inputs, textareas and selectable text (egui: no stray context menu; text fields keep copy/paste) | display | ✅ |
| `app.launch_toasts` | Toasts on backend launch events: ok 'launched <name>' (then watches for game, reloads profile), error 'launch error: …' | display | ✅ |
| `app.cli` | Command-line/second-instance args: --connect host[:port] opens Direct connect and connects, --reconnect rejoins (warn if no history), open dzch:// or .dzch file; queued until initialised | display | ✅ |
| `app.single_instance` | Single instance (Tauri single-instance plugin): a second launch hands its args (dzch:// link, .dzch path made absolute, --connect, --reconnect) to the running window, which unminimises and focuses, then exits | display | ✅ |
| `app.link_registration` | At start, dzch:// links and .dzch files are pointed at this executable when they differ (deep-link register_all, fileAssociations): Linux handler .desktop + mime package + xdg-mime default, Windows HKCU classes; never in --mock | display | ✅ |
| `app.focus_return` | Window blur/hidden marks away; on return after ≥5 min refreshes the server list, after ≥10 min checks mod updates; focus state also drives window border colour and status polling | display | ✅ |
| `app.periodic_mod_check` | Every 30 min while the window is focused, mods are force-checked for Workshop updates (needs API key); status bar shows 'Checking mods' | display | ✅ |
| `app.frame` | Window frame: rounded corners (theme radius, 0 when maximised) and optional border in focus/blur colour; maximised state tracked on resize (once per frame) | display | ⬜ |
| `app.window` | The window (src-tauri: tauri.conf.json, main.rs, build.rs): 1280x800 centred, at least 900x600, no system decorations; app icon; Windows exe icon, VERSIONINFO and manifest; a panic or start-up failure in a Windows release build shows a message box; size, position and maximised state remembered from one start to the next | display | ✅ |
| `app.toasts` | Toaster for info/ok/warn/err messages used across the app | display | ✅ |
| `app.mod_op_dialog` | Mod operation dialog mounted whenever an operation is active (lazy); wizard mounted when showWizard | display | ✅ |
