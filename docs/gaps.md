# Where the egui window falls short of the Svelte interface

> **Last updated:** 2026-10-22 · **Change:** G19–G22 and G25 closed (M3)

The ranked work list of the egui rewrite: one entry per remaining task of
`plan/execution-plan.md` M1–M4, with each view of M2 as its own entry. Take the highest-ranked
entry nobody is working on (its dependencies done); when you close part of one, update its
estimate here, the row in [`ui-parity.md`](ui-parity.md) and the progress log in
[`ROADMAP.md`](../ROADMAP.md).

Hours are Opus agent wall-clock hours for one agent, including tests and the screenshot pass.
Feature counts are entries of `apps/desktop/src/catalog.rs` ([checklist](parity-checklist.md)).

## Summary

| # | Gap | Task | Needs | Estimate | Parity doc |
|---|---|---|---|---:|---|
| G1 | Generic widget kit | M1.1 | M0.4, M0.1 theme | done | [ui-parity](ui-parity.md) |
| G2 | App parts | M1.2 | G1 | done | [ui-parity](ui-parity.md) |
| G3 | Stores on Task + HubEvent | M1.3 | M0.2, M0.3 | done | [ui-parity](ui-parity.md) |
| G4 | Shell (77 features) | M1.4 | G1–G3 | done | [ui-parity › shell](ui-parity.md#shell) |
| G5 | Control channel and ui_shot | M1.5 | G4 | done | [ui-parity](ui-parity.md) |
| G6 | Servers view (99) | M2 | G4, G5 | done | [ui-parity › servers](ui-parity.md#servers) |
| G7 | Mods view and mod-op dialog (116) | M2 | G4, G5 | done | [ui-parity › mods](ui-parity.md#mods) |
| G8 | Settings view (77) | M2 | G4 | done | [ui-parity › settings](ui-parity.md#settings) |
| G9 | Stats view (52) | M2 | G4 | done | [ui-parity › stats](ui-parity.md#stats) |
| G10 | Friends view (47) | M2 | G6 parts | done | [ui-parity › friends](ui-parity.md#friends) |
| G11 | Direct connect view (46) | M2 | G6 parts | done | [ui-parity › connect](ui-parity.md#connect) |
| G12 | News view (41) | M2 | G4 | done | [ui-parity › news](ui-parity.md#news) |
| G13 | Setup wizard (28) | M2 | G4 | done | [ui-parity › setup](ui-parity.md#setup) |
| G14 | Favorites view (21) | M2 | G6 | done | [ui-parity › favorites](ui-parity.md#favorites) |
| G15 | Launch options view (33) | M2 | G4 | done | [ui-parity › options](ui-parity.md#options) |
| G16 | About view (38) | M2 | G4 | done | [ui-parity › about](ui-parity.md#about) |
| G17 | Offline view (27) | M2 | G4 | done | [ui-parity › offline](ui-parity.md#offline) |
| G18 | History view (14) | M2 | G14 | done | [ui-parity › history](ui-parity.md#history) |
| G19 | Single instance, dzch:// and .dzch, paste-to-connect | M3.1 | G4, G11 | done (Wayland activation token left) | [ui-parity › shell](ui-parity.md#shell) |
| G20 | Updater | M3.2 | G16 | done | [ui-parity › about](ui-parity.md#about) |
| G21 | Gamepad | M3.3 | G4, views | done | [ui-parity › shell](ui-parity.md#shell) |
| G22 | Window: size, position, maximise, Windows icon, fatal box | M3.4 | G4 | done | [ui-parity › shell](ui-parity.md#shell) |
| G23 | Packaging and release tooling for the egui binary | M4.1 | M3 | done (owner: first release) | [build](build.md) |
| G24 | Remove the Svelte interface and Tauri, rename, update docs | M4.2 | all above | done | [build](build.md) |
| G25 | Visual polish: shared `Field` rows, page header, chip tints, `Copy` colours, Play lettering, symbol glyphs, wizard, palette search | M2 follow-up | — | done | [ui-parity](ui-parity.md) |
| | **Total left** | | | **owner checks on a real release** | |

About 70% parallelizes: the M2 views fan out once the shell and parts exist; G1–G4 and M4 are
the serial path.

## Foundation (M1)

### G1. Generic widget kit (M1.1)

- **Missing:** every component of `apps/gui/src/lib/components/ui` as an egui widget (button
  variants, input, select, segmented, switch, checkbox, chip, tag, tooltip, popover, menu,
  dialog, disclosure, split pane, scroll area, signal, players bar, meter, skeleton, copy) and
  a `kit_gallery` example screenshot.
- **Estimate:** 20–30 h.

### G2. App parts (M1.2)

- **Missing:** `apps/gui/src/lib/components/app` as egui parts: page header and figures
  (rolling numbers), table head and sort heads, server list row, server flags, ping/players/
  mods cells, favorite/exclude/join buttons, server menu, master/detail, empty state, facts,
  player chart, week heatmap, chart tooltip, log pane, transfer progress, command preview,
  theme preview, shortcut list, safe HTML.
- **Estimate:** 20–30 h.

### G3. Stores (M1.3)

- **Missing:** `apps/gui/src/lib/stores` as `store/*` on `rt::Task` + `HubEvent`: servers
  (blocks, generation), profile (optimistic writes), mods (op, Steam polling), connect (join
  flow), friends (cadence), game, battleye, updater, prefs, theme, session.
- **Estimate:** 15–25 h.

### G4. Shell (M1.4)

- **Missing:** custom title bar, rail with counts and rejoin card, status bar segments, view
  host (kept-alive views), confirm/password/connect dialogs, toasts, command palette,
  shortcuts sheet, keys and the command registry, init screen, setup and mod-op hosts —
  77 catalog features under `shell`.
- **Estimate:** 35–50 h.

### G5. Control channel and ui_shot (M1.5)

- **Missing:** `control.rs` (`ui.view`, `ui.command`, `ui.screenshot`, `ui.state`) and
  `examples/ui_shot.rs`, which `cargo xtask shots` and the visual diff need.
- **Estimate:** 6–10 h.

## Views (M2)

**Closed 2026-10-22**: every view is ported (701 / 716 catalog features, mean SSIM 0.847).
What is left of a view is either gamepad focus (G21) or polish (G25).

Each view is done when its catalog entries are done, its commands are registered and its shot
is compared with `var/reference/<view>.png`.

### G6. Servers (99 features)

- **Missing:** the whole view: backend-paged virtual list, figures, toolbar and filters,
  active-filter chips, sources bar, stale banner, rows with hover actions and context menu,
  keys, master/detail and the server panel (hero, five tabs, action bar), join flow.
- **Estimate:** 30–45 h. The server panel is shared by favorites, history, friends and connect.

### G7. Mods (116 features)

- **Missing:** figures and actions, sortable virtual list with status groups, ticks and bulk
  bar, mod menu, detail pane, install and review dialogs, the global mod-op dialog (Steam
  Guard, password, manual login, queue, transfer, live log).
- **Estimate:** 30–45 h.

### G8. Settings (77 features)

- **Missing:** section index with scroll spy, unsaved bar, the twelve sections, theme editor
  (simplified) with legibility checks.
- **Estimate:** 20–30 h.

### G9. Stats (52 features)

- **Missing:** figures, range, playing-now pill, year calendar, places and maps, week
  heatmap, full history with search, day filter, pages and forget, profile export/import.
- **Estimate:** 15–22 h.

### G10. Friends (47 features)

- **Missing:** grouped list with DayZ blocks and server rows, detail panel, refresh cadence,
  arrival toasts, error and empty states.
- **Estimate:** 10–15 h.

### G11. Direct connect (46 features)

- **Missing:** form, query, launch args, recent servers, .dzch open/export, dzch:// copy,
  server detail.
- **Estimate:** 10–15 h.

### G12. News (41 features)

- **Missing:** category chips, article list, reader with an HTML → rich text renderer,
  lightbox, image cache use, keys.
- **Estimate:** 10–16 h.

### G13. Setup wizard (28 features)

- **Missing:** the seven steps, sidebar, progress, footer navigation, SteamCMD watch, Steam
  prefill, finish.
- **Estimate:** 10–14 h.

### G14. Favorites (21 features)

- **Missing:** the list on the shared row and panel, client-side search and sort, remove with
  confirm, auto-ping of visible rows.
- **Estimate:** 6–10 h.

### G15. Launch options (33 features)

- **Missing:** option cards, toggles and values, hardware recommendations, command-line
  preview, Linux launch card.
- **Estimate:** 8–12 h.

### G16. About (38 features)

- **Missing:** hero, links, update card, this machine, data sources, shortcut list, guide,
  footer.
- **Estimate:** 6–10 h.

### G17. Offline (27 features)

- **Missing:** mission cards, launch, clear save, remove, install/update with status strip,
  maintenance aside.
- **Estimate:** 5–8 h.

### G18. History (14 features)

- **Missing:** favorites' list plus Last played, remove, clear all, Rejoin focus.
- **Estimate:** 4–6 h.

## Platform (M3)

### G19. Single instance and links (M3.1)

- **Done (M3.1):** `apps/desktop/src/platform/`: single instance (per-user lock file +
  127.0.0.1 port file with a random token; a second launch hands its arguments over, the
  window unminimises and focuses), `file://` and relative `.dzch` paths normalised,
  registration at start on a background thread, only when it differs (Linux handler
  `.desktop`, `application/x-dzch` mime package, `xdg-mime default`; Windows HKCU scheme
  and `.dzch` class), never in `--mock` or debug builds. Paste-to-connect was already in
  `Ctx::paste`.
- **Left:** raising the window on Wayland needs an activation token (winit
  `ActivationToken`), not passed yet: GNOME may only flag the window.

### G20. Updater (M3.2)

- **Done:** latest.json (same endpoint and format), semver compare, download with
  `HubEvent::UpdateDownload` progress, minisign verification against the same public key
  (checked on the published v0.5.4 Windows zip), AppImage replaced atomically (temp file +
  rename, the plugin's move-aside as fallback), Windows zip swap, restart that closes the
  window normally while the new instance waits for it (`DZCH_RESTART_OF`); About card and
  status bar. Tests run on a local HTTP server with a throwaway key.
- **Owner checks on a real release:** see the M3.2 report (AppImage and zip from a release
  built with the egui binary).

### G21. Gamepad (M3.3)

- **Done (M3.3a, `apps/desktop/src/pad`):** gilrs in the hub, the input mode, spatial focus
  navigation over egui's widget rects with regions (shell parts, split panes, section
  index, the narrow drawer), lists walked row by row (`VirtualList::cursor`, 1→3→10),
  A/B, LB/RB with landing, LT/RT paging, the right stick on reading panes, the views'
  X/Y/R3 and the app's Menu/View/L3, the focus ring, pad scale, rail auto-collapse,
  `ui.pad` / `{"pad": …}` for tests.
- **Done (M3.3b):** the hint bar says what applies now (`pad::view_hints`: X/Y/R3 only
  when the view's command applies, as Svelte's `when`; A says Join on a server list's
  row), the `?` sheet in two balanced columns (one below 640 points), Ctrl+F / `/` in
  Mods and Statistics, the page header's figures no longer grow without end when they
  wrap (pad scale).
- **Missing:** the settings' live test polish.

### G22. Window (M3.4)

- **Done:** Tauri's default (1280×800 centred) and minimum (900×600) size; size, position
  and maximised state restored from eframe's `window.ron` beside the data (off the UI thread,
  written only on change; on Windows moved onto a connected monitor; minimised or corrupt
  geometry never restored, a maximised window gets back its earlier size), see
  `apps/desktop/src/platform/window.rs`; window icon (`assets/app/icon-256.png`); Windows
  icon, VERSIONINFO and the Tauri manifest embedded by `apps/desktop/build.rs` (winresource,
  `llvm-rc` under cargo-xwin, `DZCH_REQUIRE_WINRES=1` for releases); the fatal message box
  (panic hook, start-up failures and a window that cannot open) in Windows release builds,
  stderr elsewhere, as Tauri did.
- **Left:** on X11 a position on a since-unplugged monitor is left to the window manager
  (eframe clamps on Windows only); Wayland places windows itself. Not run live here (an
  agent-launched window gets no frames on this machine); covered by unit tests.

## Polish

### G25. Visual polish (M2 follow-up)

What the lanes measured but left in shared widgets so other views stayed stable: `kit::Field`
label/hint lines are 4–5 px shorter than the web version (settings, options rows drift);
`PageHeader` sits 2 px low; `Button::chip` has no tint (servers' Missing / To update counts);
`Copy` uses faint where Svelte uses muted; the Play button's lettering is ~12 px wider; the
bundled subsets lack `→`. Fix in the shared widget, re-run `cargo xtask shots` and
`visual-diff`, check every view.

- **Done:** `Field` and `SettingsSection` text on CSS line heights (`paint::galley_lh`), their
  1px borders outside the rows; the header's name row as tall as its tallest cell (30 px
  without figures) and its `border-b` a pixel of its own; `Button::tint`, used by the server
  panel's Missing / To update chips; `Copy::color` / `text_size` per use site; Play lettering
  at the size's own text size; a DejaVu Sans subset for `← → ↵ ⋯ ✓ ✕ □ △ ○` (a test checks
  every face); the view is not drawn under the setup wizard; the palette's server search
  filters the store's snapshot (`browser::order`). Settings SSIM 0.758 → 0.819, connect
  0.764 → 0.817; the other views unchanged.
- **Left:** the toolbars of mods, news, friends (1 px tall) and stats (1 px short) were tuned
  against the old header height and now land on the same border by compensation; the
  figures row is 32.5 px where the browser makes 33.

## Switch-over (M4)

### G23. Packaging (M4.1)

- **Done:** `dzch release`, Makefile, AppImage (container, appimagetool pinned), Windows zip
  and AUR build the egui binary `dayz-community-hub` with cargo only; signatures by
  `cargo xtask sign` (Tauri's format, same key). Publishing stays the owner's.
- **Left for the owner:** the first egui release: an installed Tauri AppImage and Windows zip
  update to it and restart into one window; AUR builds; `latest.json` keys and signatures.

### G24. Remove the Svelte interface (M4.2)

- **Done:** `apps/gui` (Svelte, bun, Tauri shell) deleted, with specta and the Tauri crates;
  the package and binary are `dayz-community-hub`; the JSON words, `presets.rs` and the SVG
  icon folder are the sources (tests check the words are complete and `list.rs` matches the
  folder); `scripts/screenshots.sh` and `tools/i18n-export` retired; `var/reference/` and the
  catalog are the record; the release secrets' `.env` moved to the repository root; CI and
  its image run the Rust checks only; CLAUDE.md, AGENTS.md, README, docs/build.md and the
  CHANGELOG's Unreleased notes updated.
- **Left for the owner:** rebuild the CI image (push `.forgejo/ci-image/Dockerfile`); move a
  release `.env`, if one is kept, to the repository root; the release checks of G23. (The
  README's Svelte screenshot was replaced by the egui views in `.forgejo/assets/screenshots/`
  for 0.6.0.)
