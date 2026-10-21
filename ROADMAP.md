# DayZ Community Hub roadmap

**Stage: rewrite: M0 foundation** · next: M1 kit and shell, then the views (M2) in parallel

> **Last updated:** 2026-10-21 · **Change:** created

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
| **Catalog features done** | **0 / 716 (0%)** | measured: `catalog.rs`, `cargo xtask parity` |
| Views ported | 0 / 13 (+ shell) | measured: rows of [ui-parity](docs/ui-parity.md) at 100% |
| Visual diff, mean SSIM | n/a yet | measured once `cargo xtask shots` runs (needs M1.5) |
| Tests (new crates) | 91 `#[test]` in `apps/desktop`, `crates/app/hub`, `xtask` | counted 2026-10-21, moving while M0 runs |
| Remaining to switch-over | ~310–480 h | estimated: sum of [gaps](docs/gaps.md) |

## Milestones

| Milestone | What | State |
|---|---|---|
| M0 Foundation | skeleton, hub port, mock, assets, i18n, xtask, catalog and docs | running |
| M1 Kit and shell | widget kit, app parts, stores, shell, control channel + ui_shot | next |
| M2 Views | servers, favorites, history, friends, connect, offline, mods, options, news, stats, settings, about, setup | queued |
| M3 Platform | single instance + links, updater, gamepad, window | queued |
| M4 Switch-over | packaging under the old name, remove `apps/gui` | queued |

## Progress log

### 2026-10-21

- M0 started. M0.1 skeleton landed (dfd8dd5d): workspace members, `dz-hub` and
  `apps/desktop` crates, `xtask`, `rt.rs` (tasks), theme tokens and presets, fonts, an app shell
  that opens a window, `AGENTS.md`, the plan.
- M0.2 hub port, M0.4 assets, M0.5 i18n and M0.6 xtask running in parallel.
- M0.7: five inventory passes over `apps/gui/src` listed 716 features; they are the catalog
  (`apps/desktop/src/catalog.rs`, 365 command features, 351 display features), the generated
  checklist, `docs/ui-parity.md` with porting notes per view, and `docs/gaps.md`. Parity floor
  pinned at 0.
