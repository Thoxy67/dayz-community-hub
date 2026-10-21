# DayZ Community Hub: instructions for agents

The launcher is being rebuilt in egui, **1:1 with the Tauri/Svelte interface** (features and
looks), the way `~/wordcraft` and `~/photocraft` are built. Read `CLAUDE.md` (project rules),
then this file.

## Start every session here

1. Read `plan/STATUS.md` (milestone, next task), the task in `plan/execution-plan.md` and
   `plan/architecture.md`. `plan/` is local (gitignored); if missing, start from `ROADMAP.md`,
   `docs/gaps.md` and `docs/ui-parity.md`.
2. Follow the autonomous protocol (`plan/execution-plan.md` §5). Ask the owner only what is
   genuinely theirs (publishing, pushing, secrets, dropping a feature).

## The reference is the Svelte interface

`apps/gui/src` is the specification until it is removed (M4.2): every view, dialog, menu,
tooltip, empty state, shortcut and word. Port behaviour from the `.svelte`/`.ts` files, not
from memory. Its looks are in `var/reference/<view>.png` (mock backend, chernarus, 1400×860);
regenerate with `QUERY=theme=chernarus make screenshots` and copy into `var/reference`.

## Never crash

- No `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` outside tests:
  `dz-hub` and `apps/desktop` deny them at the crate root. No new `unsafe`.
- Errors are `Result<_, String>`, shown as a toast or inline, never swallowed silently.
- Data from the hub, files, the network and the control channel is hostile: `get()` over
  `[i]`, char-boundary slicing, checked arithmetic, clamped sizes.

## The window (apps/desktop)

- **The frame never waits.** Hub calls go through `rt::spawn` / `rt::Fetch` / `rt::fire`;
  nothing blocks, no disk or network on the UI thread. Repaint on events and finished tasks,
  not on a timer. Optimistic updates for toggles. Searches debounced (`rt::Debounce`) and
  superseded (newest request wins).
- **Heavy work stays in Rust below the window** (`dz-hub`): the server list is filtered,
  sorted, counted and paged by `servers_query`; the window holds only the rows on screen and
  draws them with `ScrollArea::show_rows` (or the kit's virtual list).
- **Tokens only.** Colours from `ui.tokens()` (`theme::Tokens`), sizes from `theme::size`,
  faces from `theme::font(Face, size)`, durations from `theme::motion`. Never a literal
  `Color32` in a view.
- **Reuse before writing.** Generic widgets live in `kit/`, app-aware parts in `parts/`
  (as `lib/components/ui` and `lib/components/app` did). Extend a shared one rather than
  copying it into a view.
- **Everything is a command.** A user-visible action that a shortcut, the palette, a pad or
  the control channel could trigger is a `CommandSpec` in `commands.rs`; buttons call it by id.
- **Words through `i18n`**, five languages (en fr de es ru), same keys as
  `apps/gui/src/content/*.content.ts`. No English literal in a view.
- **Icons through `icons`** (the same lucide / game-icons / boxicons / circle-flags icons
  the Svelte interface uses). Every asset file has a row in `ATTRIBUTION.md`.
- Motion is subtle: ≤ 6 px, ease-out, `theme::motion` durations, none when reduced motion is on.

## Parity is measured

- `apps/desktop/src/catalog.rs` lists every feature of the Svelte interface. `cargo xtask
  parity` writes `docs/parity-checklist.md`; a test pins a floor that only rises.
- `cargo xtask shots` renders every view headless on the mock hub into `var/shots/`;
  `cargo xtask visual-diff` compares them with `var/reference/` and updates `docs/ui-parity.md`.
- **For UI work, look at the result**: render it with `ui_shot`, read the PNG, compare with the
  reference side by side. Fix what differs before calling a view done.

## Running and looking

- `cargo run -p dayz-community-hub-egui` (real backend), `-- --mock` (invented world).
- Headless screenshots: `cargo run -p dayz-community-hub-egui --example ui_shot -- script.jsonl`
  (see the example's header: `{"view": "servers"}`, `{"command": …}`, `{"shot": "/abs.png"}`).
- Control channel: `--control <port>`; JSON lines on 127.0.0.1 keyed by the file the app writes
  (see `apps/desktop/src/control.rs`).

## Quality gates

`cargo xtask ci` before every commit: fmt check, clippy `-D warnings`, tests, assets, layers,
parity floor, and `cargo xwin check --target x86_64-pc-windows-msvc` for the new crates when
cargo-xwin is installed. `make check` keeps covering the old interface until M4.2.

## Version control

Colocated jj (see CLAUDE.md). One change per arc, only the paths you touched, task id first:
`jj commit <paths> -m "M2.3 servers: the filters bar is complete"`, body says why, then the
Co-Authored-By trailer. Parallel agents: **do not commit** (the orchestrator does); use your own
`CARGO_TARGET_DIR` (absolute path under `/home/thoxy/dev/dayz-community-hub/target/agents/<name>`)
and delete it when done; edit only the files you own; re-read a shared file right before a
minimal edit.

## Progress docs

`ROADMAP.md` (stage, headline numbers, progress log), `docs/ui-parity.md` (view by view),
`docs/gaps.md` (ranked work list), `docs/parity-checklist.md` (generated). When work lands,
update the gap, the parity row and the progress log.
