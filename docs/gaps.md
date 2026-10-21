# Where the egui window falls short of the Svelte interface

> **Last updated:** 2026-10-21 · **Change:** created

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
| G1 | Generic widget kit | M1.1 | M0.4, M0.1 theme | 20–30 h | [ui-parity](ui-parity.md) |
| G2 | App parts | M1.2 | G1 | 20–30 h | [ui-parity](ui-parity.md) |
| G3 | Stores on Task + HubEvent | M1.3 | M0.2, M0.3 | 15–25 h | [ui-parity](ui-parity.md) |
| G4 | Shell (77 features) | M1.4 | G1–G3 | 35–50 h | [ui-parity › shell](ui-parity.md#shell) |
| G5 | Control channel and ui_shot | M1.5 | G4 | 6–10 h | [ui-parity](ui-parity.md) |
| G6 | Servers view (99) | M2 | G4, G5 | 30–45 h | [ui-parity › servers](ui-parity.md#servers) |
| G7 | Mods view and mod-op dialog (116) | M2 | G4, G5 | 30–45 h | [ui-parity › mods](ui-parity.md#mods) |
| G8 | Settings view (77) | M2 | G4 | 20–30 h | [ui-parity › settings](ui-parity.md#settings) |
| G9 | Stats view (52) | M2 | G4 | 15–22 h | [ui-parity › stats](ui-parity.md#stats) |
| G10 | Friends view (47) | M2 | G6 parts | 10–15 h | [ui-parity › friends](ui-parity.md#friends) |
| G11 | Direct connect view (46) | M2 | G6 parts | 10–15 h | [ui-parity › connect](ui-parity.md#connect) |
| G12 | News view (41) | M2 | G4 | 10–16 h | [ui-parity › news](ui-parity.md#news) |
| G13 | Setup wizard (28) | M2 | G4 | 10–14 h | [ui-parity › setup](ui-parity.md#setup) |
| G14 | Favorites view (21) | M2 | G6 | 6–10 h | [ui-parity › favorites](ui-parity.md#favorites) |
| G15 | Launch options view (33) | M2 | G4 | 8–12 h | [ui-parity › options](ui-parity.md#options) |
| G16 | About view (38) | M2 | G4 | 6–10 h | [ui-parity › about](ui-parity.md#about) |
| G17 | Offline view (27) | M2 | G4 | 5–8 h | [ui-parity › offline](ui-parity.md#offline) |
| G18 | History view (14) | M2 | G14 | 4–6 h | [ui-parity › history](ui-parity.md#history) |
| G19 | Single instance, dzch:// and .dzch, paste-to-connect | M3.1 | G4, G11 | 8–14 h | [ui-parity › shell](ui-parity.md#shell) |
| G20 | Updater | M3.2 | G16 | 8–14 h | [ui-parity › about](ui-parity.md#about) |
| G21 | Gamepad | M3.3 | G4, views | 15–25 h | [ui-parity › shell](ui-parity.md#shell) |
| G22 | Window: size, position, maximise, Windows icon, fatal box | M3.4 | G4 | 5–8 h | [ui-parity › shell](ui-parity.md#shell) |
| G23 | Packaging and release tooling for the egui binary | M4.1 | M3 | 8–14 h + owner | — |
| G24 | Remove apps/gui, rename, update docs | M4.2 | all above | 4–8 h | — |
| | **Total** | | | **~310–480 h** | |

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

- **Missing:** local-socket single instance and argument hand-off (`--connect`,
  `--reconnect`, `dzch://`, `.dzch`), registration (Linux .desktop + xdg-mime, Windows
  registry), paste-to-connect.
- **Estimate:** 8–14 h.

### G20. Updater (M3.2)

- **Missing:** latest.json, minisign verification, AppImage replace and Windows zip install,
  wired to the About update card and the status bar.
- **Estimate:** 8–14 h.

### G21. Gamepad (M3.3)

- **Missing:** gilrs in the hub, spatial focus navigation with regions, view commands on
  X/Y/R3/L3, hint bar, larger scale in pad mode, controller settings test.
- **Estimate:** 15–25 h.

### G22. Window (M3.4)

- **Missing:** remembered size and position, maximise state, Windows icon and VERSIONINFO,
  fatal error box on Windows.
- **Estimate:** 5–8 h.

## Switch-over (M4)

### G23. Packaging (M4.1)

- **Missing:** `dzch release`, Makefile, AppImage, Windows zip and AUR build the egui binary
  under the old name. Publishing stays the owner's.
- **Estimate:** 8–14 h + owner.

### G24. Remove the Svelte interface (M4.2)

- **Missing:** delete `apps/gui`, the bun toolchain in `make check` and `bindings`; rename the
  package; update CLAUDE.md, README, docs/build.md, CHANGELOG.
- **Estimate:** 4–8 h.
