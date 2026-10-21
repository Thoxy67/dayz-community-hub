# DayZ Community Hub roadmap

**Stage: rewrite: M2 views done** · next: M3 platform (links, updater, gamepad, window), then M4 switch-over

> **Last updated:** 2026-10-22 · **Change:** M2 views landed

The launcher's Tauri/Svelte interface (`apps/gui`) is being rebuilt 1:1 in egui
(`apps/desktop`, backend `crates/app/hub`), features and looks, on Windows and Linux alike.
Detail: [UI parity](docs/ui-parity.md) (view by view, porting notes, visual diff),
[gaps](docs/gaps.md) (the ranked work list), [parity checklist](docs/parity-checklist.md)
(generated from `apps/desktop/src/catalog.rs`), [AGENTS.md](AGENTS.md) (how agents work here).

Hours are Opus agent wall-clock hours for one agent working sequentially; about 70% of the
remaining work parallelizes (the views fan out once the shell exists).

## Headline numbers

| | Value | Kind |
|---|---|---|
| **Catalog features done** | **701 / 716 (97.9%)** | measured: `catalog.rs`, `cargo xtask parity` |
| Views ported | 13 / 13 (shell 66 / 77, the rest is gamepad) | measured: [parity checklist](docs/parity-checklist.md) |
| Visual diff, mean SSIM | 0.847 over 13 views (0.75–0.95) | measured: `cargo xtask visual-diff` 2026-10-22 |
| Tests (new crates) | 131 desktop + 60 hub + xtask, all passing | `cargo xtask ci` 2026-10-22 |
| Remaining to switch-over | ~56–98 h | estimated: sum of [gaps](docs/gaps.md) |

## Milestones

| Milestone | What | State |
|---|---|---|
| M0 Foundation | skeleton, hub port, mock, assets, i18n, xtask, catalog and docs | done |
| M1 Kit and shell | widget kit, app parts, stores, shell, control channel + ui_shot | done |
| M2 Views | servers, favorites, history, friends, connect, offline, mods, options, news, stats, settings, about, setup | done |
| M3 Platform | single instance + links, updater, gamepad, window | next |
| M4 Switch-over | packaging under the old name, remove `apps/gui` | queued |

## Progress log

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
