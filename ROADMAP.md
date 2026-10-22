# DayZ Community Hub roadmap

**Stage: switch-over done** · the egui window is the launcher; next: the owner's release checks, then the first egui release

> **Last updated:** 2026-10-22 · **Change:** M4 switch-over landed (Svelte/Tauri removed)

The launcher's Tauri/Svelte interface was rebuilt 1:1 in egui (`apps/desktop`, backend
`crates/app/hub`), features and looks, on Windows and Linux alike, and replaced it: the
Svelte interface and the Tauri shell are gone, the catalog and `var/reference/` are their record.
Detail: [UI parity](docs/ui-parity.md) (view by view, porting notes, visual diff),
[gaps](docs/gaps.md) (the ranked work list), [parity checklist](docs/parity-checklist.md)
(generated from `apps/desktop/src/catalog.rs`), [AGENTS.md](AGENTS.md) (how agents work here).

Hours are Opus agent wall-clock hours for one agent working sequentially; about 70% of the
remaining work parallelizes (the views fan out once the shell exists).

## Headline numbers

| | Value | Kind |
|---|---|---|
| **Catalog features done** | **714 / 719 (99.3%)** | measured: `catalog.rs`, `cargo xtask parity` |
| Views ported | 13 / 13, shell complete but the window frame | measured: [parity checklist](docs/parity-checklist.md) |
| Visual diff, mean SSIM | 0.855 over 13 views (0.75–0.95) | measured: `cargo xtask visual-diff` 2026-10-22 |
| Tests (new crates) | 160 desktop + 66 hub + xtask, all passing | `cargo xtask ci` 2026-10-22 |
| Remaining to switch-over | owner checks on a real release | G23 in [gaps](docs/gaps.md) |

## Milestones

| Milestone | What | State |
|---|---|---|
| M0 Foundation | skeleton, hub port, mock, assets, i18n, xtask, catalog and docs | done |
| M1 Kit and shell | widget kit, app parts, stores, shell, control channel + ui_shot | done |
| M2 Views | servers, favorites, history, friends, connect, offline, mods, options, news, stats, settings, about, setup | done |
| M3 Platform | single instance + links, updater, gamepad, window | done |
| M4 Switch-over | packaging under the old name, remove the Svelte interface and Tauri | done |

## Progress log

### 2026-10-22 (M4)

- M4.1: the AppImage, the Windows zip, the AUR package and `dzch release` build the egui
  executable under the old name, with the same file names, `latest.json` keys, endpoint and
  key; `cargo xtask sign` writes the updater's signatures (Tauri's format) without the Tauri
  CLI; the AppImage carries no library (wgpu and winit load the host's with dlopen).
- M4.2: `apps/gui` (Svelte interface, bun project, Tauri shell) deleted, with specta and the
  Tauri crates (~2 900 lines of Cargo.lock); the package is now `dayz-community-hub`. The
  JSON words, `presets.rs` and the SVG icon folder are the sources (`scripts/icons.py`
  rewrites `list.rs` from the folder); `scripts/screenshots.sh` and `tools/i18n-export`
  retired; CI runs the Rust checks only. CLAUDE.md, AGENTS.md, README, docs/build.md rewritten.

### 2026-10-22 (M3)

- M3 landed (three lanes, six agents): one window per user with dzch:// links and .dzch
  files handed over a token-checked local socket, registration off the UI thread (skipped in
  debug and mock); the updater end to end without the Tauri plugin (same latest.json and key,
  atomic AppImage replace, Windows zip swap, relaunch); the gamepad (spatial focus, list
  stepping with acceleration, view buttons, hint bar, two-column shortcuts sheet); the window
  remembers its geometry, the Windows exe carries icon, VERSIONINFO and manifest.
- G25 polish: shared rows, headers, chips, Copy, Play lettering, symbol glyphs; mean SSIM
  0.847 → 0.855. Parity 714 / 719, floor 714; the 5 left are the window frame and the theme
  editor's legibility report, dropped by the simplified theme.

### 2026-10-22

- M2 landed (four parallel lanes, 14 agents). M2.0: the hub publishes the server list as an
  immutable snapshot and the window filters it into an `Arc<[u32]>` order (0.2–0.6 ms for
  9 500 servers in release); pings land in rows as they arrive; no paging. Then every view:
  servers, favorites and history (one saved list), mods with the operation dialog, options,
  offline, settings, setup wizard, about, Direct Connect, friends, news (no web view, "Open
  dayz.com" when Cloudflare blocks), stats. Parity 135 → 701 / 716, floor raised to 701;
  mean SSIM 0.79 → 0.847.
- Steam's master server is asked through the Web API only (no Steam client fallback).
- The Steam avatar shows in the title bar and in settings › account (shared picture loader,
  also used by friends and news).
- Left: gamepad focus navigation (M3.3, 12 features), the window frame and the legibility
  report (dropped by the simplified theme), visual polish listed in [gaps](docs/gaps.md).

### 2026-10-21 (M1)

- M1 landed (b03879e9): kit, parts, stores on the hub, shell, command registry, control
  channel and `ui_shot`; parity 135 / 716. Title-bar drag works on an unfocused window.

### 2026-10-21

- M0 started. M0.1 skeleton landed (dfd8dd5d): workspace members, `dz-hub` and
  `apps/desktop` crates, `xtask`, `rt.rs` (tasks), theme tokens and presets, fonts, an app shell
  that opens a window, `AGENTS.md`, the plan.
- M0.2 hub port, M0.4 assets, M0.5 i18n and M0.6 xtask running in parallel.
- M0.7: five inventory passes over `apps/gui/src` listed 716 features; they are the catalog
  (`apps/desktop/src/catalog.rs`, 365 command features, 351 display features), the generated
  checklist, `docs/ui-parity.md` with porting notes per view, and `docs/gaps.md`. Parity floor
  pinned at 0.
