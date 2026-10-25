# Documentation

## Using the launcher

- [User guide](guide.md): the setup wizard, finding a server, mods, launching,
  offline mode, settings, shortcuts and the controller, where your data lives,
  and what to do when something goes wrong.
- [Steam Deck](steam-deck.md): installing the AppImage on SteamOS, adding it to
  Game Mode, and how the controls map.
- [Changelog](../CHANGELOG.md): what changed in each release.

## Working on it

- [Build](build.md): the layout, every `make` target, checks, releases and
  signatures, the glibc floor, Windows cross-compilation and known failures.
- [Control protocol](control-protocol.md): driving a running window (or the
  headless `ui_shot` example) with JSON lines, for agents, scripts and tests.
- [What DayZ says over A2S](a2s-dayz.md): the server queries, DayZ's keywords
  and its binary rules block, and how `dz-a2s` reads them.
- [AGENTS.md](../AGENTS.md): the window's rules (never crash, the frame never
  waits, tokens, commands, words, icons, parity).
- README screenshots: `uv run --with pillow scripts/readme_shots.py` renders
  them into `.forgejo/assets/screenshots/` on the mock hub.

## Records of the egui rewrite

The Svelte/Tauri interface was rebuilt in egui and removed in October 2026.
These pages record how; they are not kept up to date with later work.

- [Roadmap](../ROADMAP.md): the rewrite's milestones and progress log.
- [UI parity](ui-parity.md): the egui window against the Svelte interface,
  view by view, with the visual diff.
- [Gaps](gaps.md): the ranked work list the rewrite followed.
- [Parity checklist](parity-checklist.md): every feature of the Svelte
  interface, generated from `apps/desktop/src/catalog.rs`.
