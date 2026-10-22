# UI parity: the egui window against the Svelte interface

> **Last updated:** 2026-10-21 · **Change:** created

How close the egui window (`apps/desktop`) is to the Tauri/Svelte interface (`apps/gui/src`),
view by view. The Svelte interface was the specification until M4.2 removed it (2026-10-22):
every view, dialog, menu, tooltip, empty state, shortcut and word was ported 1:1, features and
looks. This page is now a record; the `apps/gui/…` paths below name files in the history
(`jj file show -r 67e3940c <path>`).

What is measured:

- **Features**: `apps/desktop/src/catalog.rs` lists every user-visible feature of the Svelte
  interface (716 entries, from five inventory passes over `apps/gui/src`). A command feature is
  done when its id is in the command registry; a display feature is done when its flag is set,
  which a screenshot comparison justifies. Generated checklist:
  [`parity-checklist.md`](parity-checklist.md) (`cargo xtask parity`).
- **Looks**: `cargo xtask shots` renders every view on the mock hub, `cargo xtask visual-diff`
  compares the shots with `var/reference/<view>.png` (Tauri mock build, chernarus theme,
  1400×860) and writes the SSIM table at the end of this file.

Hours are Opus agent wall-clock hours for one agent, including tests and the screenshot pass.
They assume M1 (kit, parts, stores, shell) has landed; the shell row is M1.4 itself.

## Summary

| Area | Svelte (what exists) | egui | Parity | Hours left |
|---|---|---|---:|---:|
| servers | Backend-paged virtual list (9.5k rows), figures, toolbar with tri-state chips, map select, more-filters popover, active-filter chips, sources bar, stale banner, sortable columns, rows with hover actions and context menu, keys and pad, master/detail with the server panel (hero, Overview/Players/Mods/Rules/Stats tabs, action bar), join flow — 99 features | ported (`views/servers`): snapshot + order list (no paging), header figures, filters, sources bar, stale strip, table, keys, shared detail panel with all tabs; controller focus navigation waits for M3.3 | 98% | 1–2 |
| favorites | Same row and detail as servers, client-side search/sort, "in the list only", remove with confirm, auto-ping of visible rows, empty states — 21 | all 21 (`views/favorites` on the shared saved list `views/saved`) | 100% | done |
| history | Favorites' list plus Last played column, clear all, remove entry, Rejoin focus — 14 | all 14 (`views/history` on `views/saved`): figures, Clear all (confirm), Last played column, remove (hover, menu with Del), Rejoin focus, empty state; repaints once a minute for relative times; SSIM 0.83 | 100% | — |
| friends | Grouped friend list (DayZ blocks clustered by server, other games, online, away, offline), server rows, detail panel, auto-refresh cadence, arrival toasts, error/empty states — 47 | all 47 (`views/friends`); the server detail is the servers view's shared host (a name/address fallback until it is installed) | 100% | — |
| connect | Direct connect form (address split, ports disclosure, password from favourites), query, extra launch args with auto-detect, recent servers, .dzch open/export, dzch:// copy, server detail — 46 | nothing yet | 0% | 10–15 |
| offline | Mission cards (map banner, terrain, mode, save, folder), launch, clear save, remove, install/update with status strip, maintenance aside — 27 | all 27 (`views/offline`), SSIM 0.90; the status bar's install segment. Clearing saves is optimistic; pad X launches the card under the pointer. Mock: `DZCH_MOCK=offline-empty`, `offline-installing` | 100% | — |
| mods | Figures, update/check/install actions, more menu, search + filter, sortable virtual list with status groups, ticks and bulk bar, mod menu, keys and pad, detail pane, install and review dialogs, mod-op dialog (Steam Guard, password, queue, transfer, live log) — 116 | nothing yet | 0% | 30–45 |
| options | 20 launch options in group cards, toggles and values, recommendations from hardware, command-line preview, Linux launch card (Steam, Proton, prefix, Steam launch options) — 33 | nothing yet | 0% | 8–12 |
| news | Category chips, article list with thumbnails, reader (hero, meta, sanitised HTML body with images and links), lightbox, keys, image cache — 41 | ported (`views/news`): all but the pad scroll target (M3.3); Cloudflare blocks show "Open dayz.com" instead of the WebView | 98% | — |
| stats | Figures, range selector, playing-now pill, year calendar heatmap with keys and tooltips, places and maps with tooltips, week heatmap, full history (search, day filter, pages, forget), profile export/import — 52 | all 52 (`views/stats`): calendar, places, maps, week heat, history (only on-screen rows, optimistic forget); SSIM 0.79. Mock: `DZCH_MOCK=stats-empty` for the empty state | 100% | — |
| settings | Section index with scroll spy, unsaved bar, account, Steam & SteamCMD, API keys, location, ping, excluded IPs, appearance (presets, custom themes, theme editor with legibility checks), language, window frame, controller, BattlEye, profile data — 77 | nothing yet | 0% | 20–30 |
| about | Hero, links, update card (state machine), this machine, data sources, shortcuts list, guide tabs, footer — 38 | nothing yet | 0% | 6–10 |
| setup | Seven-step first-run wizard (welcome + language, game, downloads, SteamCMD, account, services, appearance, done) — 28 | all 28 (`views/setup`, drawn by the shell's wizard host); welcome SSIM 0.92 against a `?wizard=1` reference. Finishing saves optimistically and closes at once (no "Saving…" wait); SteamCMD is watched by the wizard's own 3 s poll, not the hub event. Mock: `DZCH_MOCK=wizard,steamcmd-missing` | 100% | — |
| shell | Title bar (drag, figures, search, account, language, theme menu, window buttons), rail (groups, counts, active bar, collapse, kill game, rejoin card), status bar segments, confirm/password/connect dialogs, command palette, shortcuts sheet, global keys, pad navigation, init screen, toasts, paste-to-connect, CLI — 77 | theme tokens, presets and fonts (M0.1) | 0% | 35–50 |
| **Total** | **716 features** | **0 done** | **0%** | **~200–300** |

## Notes for porting

The behaviours below are the ones that are easy to lose in a port. Sizes, strings and timings
come from the inventory of `apps/gui/src`; read the named `.svelte`/`.ts` files before porting.

### servers

- Svelte paged `servers_query` (100-row blocks, skeleton rows); **egui does not** (M2.0): the
  `servers` store holds the hub's `ServerSnapshot` and asks for a `ServerOrder` (an
  `Arc<[u32]>` of indices + the header's stats) when the list, the profile's flags or the query
  change (`Servers::set_query(q, debounce)`; typing 80 ms, toggles at once, newest wins). Rows
  are read by index: `servers.order()?.get(n)` → `ServerRef` → `RowData::from_server`. Pings
  and live counts land in the snapshot's atomics and show on the next frame; under a query
  that depends on live data (ping/players sort, hide empty/full, max ping) the order is redone
  at most every 500 ms. Row height 48 px, overscan 8.
- A new request (any filter or sort change) scrolls to the top; list growth does not.
- Search is debounced 200 ms and trimmed. Tri-state chips cycle all → only → none → all.
  Sort: the same column flips; a new column starts ascending for name/map/ping and descending
  for players/time/mods. Default sort is "none".
- Session state (filters with the raw typed search, sort, scroll, selected `ip:query_port`,
  details visibility) survived a window reload in Svelte; in egui it simply lives in app state.
  Pane widths persist: `servers-detail` initial 400, min 320, max 620.
- Detail fetches are debounced so arrowing through rows does not query every server: A2S
  150 ms, server mods 180 ms, DayZ Metrics 250 ms, heatmap and rank history 400 ms after
  metrics. Caches: A2S 30 s, metrics 5 min, long history/heat/rank 15 min, server mods max 150
  entries. The chosen detail tab is global across servers and not persisted.
- Ping ≥ 5000 is a timeout (×, struck bars), 9999 on failure; manual ping uses
  `ping_timeout_manual` (10 s default). Copy feedback tick 1.4 s. Stale banner after 10 min
  (checked every 30 s); the sources bar ticks every second for the Steam throttle countdown.
- Optimistic profile writes (favourite, exclude IP) with toasts; the star never confirms.
- Keys are ignored while typing, while a dialog or connect request is open, or when the view
  is not current; Enter on a focused button does not also join. Esc in search empties, then
  blurs.
- Responsive thresholds: map column hidden < 720 px list width; tri-chip labels hidden
  < 1500 px window; "More filters" label hidden < 1536 px; secondary figures hidden < 1240 px;
  header figures wrap < 760 px header width; sources bar hides Steam figures < 900 px and
  official/duplicates < 760 px; master/detail becomes an overlay drawer < 980 px; detail tab
  icons hidden < 480 px panel width; hero live strip has 4 columns from 26rem.
- Known quirk to keep or fix knowingly: the excluded-IPs row says "hold 1.5s to manage" but no
  hold gesture exists.
- The join flow (password prompt with save-to-favourite, mods-unknown confirm, missing/update
  mods dialog, toasts) is shared by every view; it belongs in a store, not in the view.

### favorites

- Search is immediate and client-side (name, ip:port, listed map). Default sort Server
  ascending; a new column is ascending for name/map/time/ping, descending otherwise; ping
  timeouts sort last.
- Rows on screen without a ping are pinged 200 ms after the visible range settles.
- Removing from this list confirms; toggling the star elsewhere does not. Unlisted favourites
  are dimmed with a "Not listed" tag but stay joinable. The join port is the saved port, while
  ping and players use the listed query port.
- Pane `favorites-detail`: initial 380, min 300. Details toggle not persisted.

### history

- Default sort last played descending; relative times refresh every minute.
- Every removal and Clear all go through a danger confirm. The context menu adds "Remove from
  history" (Del); favourites add nothing.
- The rail Rejoin card opens History with focus "last": reset search and filter, sort by last
  played, select the newest entry, open details. Pane `history-detail`.

### friends

- Refresh cadence, only while the window is focused: 20 s on the view when a friend is in DayZ
  (Web API), 60 s on the view otherwise or when the source is the Steam client, 120 s behind
  other views via the Web API only and only once it has worked. A failed background ask stops
  background asks until the view is opened. The timer runs from the end of the last ask;
  `load()` is a no-op while loading.
- Arrival detection only diffs answers that both had a source; `since` (session-only) drives
  "joined x ago" and is cleared when the friend leaves DayZ. More than 2 arrivals at once give
  one toast with a "Show" action.
- DayZ blocks are keyed by the friend's server `ip:port` (query port when listed), sorted by
  friend count then first name; group order is fixed. Relative times re-tick every 15 s while
  the view shows. Steam links open through `steam://`.
- Pane `friends-detail`: initial 380, min 300, max 620; drawer below 980 px.

### connect

- The form is a store that survives switching views (not restarts). Validation: address
  non-empty after trim, port 1..65535; errors show only after a submit, Enter or Connect.
- Listed lookup only for strict IPv4 (no per-keystroke lookups). Query: split the address,
  guard stale answers with an incrementing id, resolve the listed entry, A2S on the listed
  query port or typed query port or typed port, adopt the game port from the answer, fetch
  details in parallel, ping once; after details arrive sync the detected mod args.
- A favourite's saved password fills only an empty field. The ports disclosure opens itself on
  a port error. Copy URL and export include the query port only when it differs.
- Enter in the address, password or port fields joins; Shift+Enter does not. Window-wide paste
  ignores text fields; pattern `^(ipv4|hostname):(\d{2,5})$`.
- Recent: favourites then history, deduped; 24 cards wide, 14 rows compact.

### offline

- Missions are described by parsing `<mode>.<map>` or `<map>.<mode>` folder names (mode regex
  offline|coop|pvp|surviv|mission), a known-map table with names and terrain areas, else a
  CamelCase split with "plus" → "+"; sorted by map name.
- `busy = loading || installing` disables most actions. The status strip is in-view state,
  separate from toasts; launch uses toasts. Install finishes through the
  `offline-mode-updated` / `offline-mode-error` events.

### mods

- Filter, sort, folded groups, ticks and query are session-only; only the pane width persists
  (`mods-detail`: initial 360, min 300, max 520; drawer below 900 px). Default sort status
  ascending; first click ascending for name/status/managed, descending for size/local. Group
  heads only when sorted by status. Columns collapse < 820 px (Your version) and < 620 px
  (In game).
- Incoming rows are Steam-subscribed items not on disk that are downloading, pending, need an
  update or are not installed.
- Update checks need a Steam API key, TTL 5 min (forced by the button, Rescan and Dismiss), and
  every 30 min while focused. Steam polling: on view show, every 5 s while Steam downloads are
  active, every 15 s while Steam did not answer; requests are shared, a failed answer keeps the
  last good one.
- The review dialog gates every update and install (not repair). Delete removes link and files
  and unsubscribes; a failed unsubscribe is an info toast. Toggling the link updates the row in
  place; bulk link/unlink toggle sequentially.
- The mod-op dialog is global and mounted while an op is active: Esc/close minimises while
  running and dismisses after; clicks outside are ignored; one op at a time (starting another
  un-minimises). Log capped at 5000 lines (+500 slack trimmed in batches); `log_progress`
  overwrites the last line; Steam Guard countdown 65 s, reset on "retrying"; elapsed ticks
  every 500 ms; speed over a 5 s window, reset when bytes go backwards; overall progress stops
  counting the current mod's fraction once it completes. A running op is re-attached from a
  backend snapshot.
- Keys are ignored while typing, in dialogs, or with a menu focused.

### options

- Toggles and values are optimistic with rollback and an error toast; setting a non-null value
  turns the option on. Values commit on blur or Enter, Esc reverts; number inputs for
  mb/count/threads.
- Recommendations: `cpuCount = min(physical, logical)`, `exThreads` 7 (≥ 4 cores) / 3 (≥ 2) / 1,
  `maxMem` = 60% of RAM rounded to 256 and clamped 2048–16384, plus `-high` and `-noBenchmark`.
- Hardware and Steam launch info are fetched once per visit; the Linux card shows only when
  launch info has a Linux part. Copy ticks 1.4 s.

### news

- Articles load once per run; Refresh forces. Changing category resets the selection to 0.
- Keys only while the view shows, with no dialog or lightbox, not typing, no modifier.
- Images go through the backend cache (`fetch_image` / `resolve_cached_images`), 6 at once,
  de-duplicated; the hero image is requested on selection; the lightbox shows the displayed
  image first, then swaps to the original.
- The body needs an HTML → rich text renderer (h1–h4, p, a, strong, ul/ol/li, img), sanitised,
  links resolved against the article URL and opened in the browser.

### stats

- Lazy start on first activation, then re-ask `play_stats(range)` and the history's first page
  on every `play-sessions-changed` event. The range resets to "all" each run.
- History search debounced 200 ms; picks (place, map, day) apply at once and scroll to the
  history. Pages of 100. Day numbers count days since the epoch in local time; the backend
  filter takes `[day*86400 + tzOffset, +86400)`. The running duration refreshes every 30 s.
- Forgetting a session has no confirmation. Quirk: the offline Play button in a places row also
  narrows the history (no stopPropagation).
- Calendar opacity steps 0/.3/.5/.75/1 by quartile of played days; week heatmap opacity
  0.12 + 0.88·v/max. Tooltips anchor to the mark and never take the pointer. Average/Longest
  hide < 1240 px window; places/maps stack < 1100 px.

### settings

- Account fields are saved together through the unsaved bar; dirty compares the fields with
  the profile; the form follows the profile only while clean; the SteamCMD login button is
  disabled while dirty.
- Ping sliders save on release (no toast); downloader, Steam server list and excluded IPs save
  at once, optimistic with reload + toast on failure.
- Theme preferences (preset or follow system, dark/light pair, custom themes, frame radius and
  border colours) persist with the theme. Hover preview in the theme menu after 150 ms.
  Controller test light 450 ms.
- The index is hidden < 1024 px window. Deep links (account chip, theme menu Edit, Steam menu,
  palette) jump without smooth scroll.
- The theme editor is simplified for egui (architecture.md): keep tokens, derive, reset,
  export/import and the legibility checks.

### about

- Version from `env!("CARGO_PKG_VERSION")`, specs from the hub. The updater is a state machine
  idle → checking → up_to_date | available | error, then downloading → done → restart; it
  checks when the view shows while idle. The title-bar badge opens About with focus "update",
  which glows the card for 2.5 s and centres it.
- The shortcut list comes from the one registry shared with the `?` sheet and the controller
  settings: keep a single registry. "Built with" should list the egui stack.

### setup

- Shown on first launch over the main window, title bar still usable, while the server list
  loads behind. Only the next step is reachable, and not while the current one blocks
  (Account without a Steam login when SteamCMD is chosen).
- The game path re-detects 500 ms after typing stops. When SteamCMD is missing, the hub keeps
  polling (`steamcmd-detected`) and the step flips by itself. Entering Account or Services asks
  the Steam client once to fill an empty name and Steam ID.

### shell

- Persisted preferences: pane widths, rail collapsed, dismissed rejoin, pad mode
  (auto/always/never), pad scale (1.15), pad glyphs, palette recent (max 8, 5 shown).
- Timings: palette server query 120 ms debounce, limit 6, players descending; status clock 30 s
  (ages, BattlEye, Steam running, only while focused); BattlEye every 5 s while DayZ runs, 20 s
  grace before "not started"; status flashes armed after 1.5 s; game poll 5 s for 3 min after
  launch; on return after 5 min refresh servers, after 10 min check mods; rolling numbers
  400 ms cubic-out, instant first value and with reduced motion.
- Breakpoints: title-bar figures hidden < 1180 px; status words hidden when the bar is
  < 1100 px, Steam and healthy BattlEye < 900 px, mods-behind < 800 px, version < 1000 px;
  rejoin card on one line when the window is < 760 px high; rail auto-collapses in pad mode
  < 1200 px.
- Keys: Ctrl+K works over everything; any open dialog, popup or menu blocks other shortcuts;
  non-Ctrl keys never fire in text fields; Esc clears a search then blurs; Ctrl+digit matches
  the physical digit (AZERTY). A second confirm request answers the first "no".
- Gamepad (M3.3): spatial focus graph with regions (title bar, rail, status bar, nav/aside);
  lists walk by index with acceleration 1 → 3 → 10; dialogs scope focus; X in a dialog is the
  last footer button.
- egui specifics: views stay alive when switching (state kept); the status bar hides segments
  by width; the rail's active bar animates its position; the title bar is a drag region with
  double-click maximise and custom window buttons (undecorated window); virtual lists bounded.

<!-- visual-diff:start -->
Generated by `cargo xtask visual-diff`: `var/shots/<view>.png` against `var/reference/<view>.png`, greyscale SSIM over 8×8 windows, share of pixels differing by more than 32 levels.

| view | SSIM | diff px | size (shot / reference) |
|---|---|---|---|
| servers | 0.8205 | 3.28 % | 1400×860 / 1400×860 |
| favorites | 0.9020 | 1.99 % | 1400×860 / 1400×860 |
| history | 0.8263 | 3.60 % | 1400×860 / 1400×860 |
| friends | 0.9140 | 2.14 % | 1400×860 / 1400×860 |
| connect | 0.8168 | 4.43 % | 1400×860 / 1400×860 |
| offline | 0.9002 | 3.21 % | 1400×860 / 1400×860 |
| mods | 0.9288 | 2.46 % | 1400×860 / 1400×860 |
| options | 0.7542 | 5.38 % | 1400×860 / 1400×860 |
| news | 0.9474 | 2.00 % | 1400×860 / 1400×860 |
| stats | 0.7809 | 8.25 % | 1400×860 / 1400×860 |
| settings | 0.8185 | 4.26 % | 1400×860 / 1400×860 |
| about | 0.7888 | 5.71 % | 1400×860 / 1400×860 |
| setup | 0.9220 | 2.10 % | 1400×860 / 1400×860 |

Mean SSIM 0.8554 over 13 view(s).
<!-- visual-diff:end -->
