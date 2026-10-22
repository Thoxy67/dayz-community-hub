# DayZ Community Hub

A DayZ launcher (server browser, mod manager, offline mode) for **Windows and
Linux**, both with the same features. Rust throughout: an egui window
(`apps/desktop`) on a backend crate (`crates/app/hub`) and the domain
libraries (`crates/core/*`). The window's own rules (never crash, the frame
never waits, tokens, commands, words, icons, parity) are in `AGENTS.md`: read
it too. The local `plan/` (gitignored) holds the status and the plan.

## Layout

```
apps/desktop/             the window: package and executable `dayz-community-hub`, lib `dzch_ui`
  src/app.rs, main.rs     the frame loop; start-up (single instance, links, runtime, hub)
  src/kit/                generic widgets, dressed from the tokens only
  src/parts/              app-aware parts shared by every view (server row, header, figure…)
  src/views/<view>/       one folder per place of the rail, plus setup and the dialogs
  src/shell/              title bar, rail, status bar, palette, dialog host, pad hints
  src/store/              app-wide state on the hub (servers, profile, mods, connect…)
  src/commands.rs         every user-visible action, by id (shortcuts, palette, pad, control)
  src/theme/              tokens derived from 12 raw colours; presets.rs is the source of the presets
  src/i18n/ + i18n/*.json words in en fr de es ru; the JSON files are the source
  src/icons/ + assets/icons/  SVG icons; scripts/icons.py rewrites icons/list.rs from the folder
  src/catalog.rs          the parity catalog (the Svelte interface's features, as a record)
  examples/ui_shot.rs     headless screenshots driven by a JSON-lines script
  build.rs, windows/      Windows icon, VERSIONINFO and manifest
crates/app/hub/           the backend: shared state, every command as a method, events, mock, updater
crates/app/dayzmetrics/   DayZ Metrics lookups
crates/core/*             dz-* libraries: api, a2s, profile, steamcmd, steamworks, game, news, common
xtask/                    cargo xtask: ci, assets, layers, parity, shots, visual-diff, sign, keygen
tools/                    uv project: `dzch release`, `dzch aur`, `dzch keys` (+ pytest)
packaging/                AppImage container, AUR PKGBUILD, .desktop and MIME files
docs/build.md             build, check, publish, known failures
var/reference/*.png       the Svelte interface's screenshots, the visual record (files, keep them)
```

## Commands

`make` lists them. `make dev` runs the window on the real backend, `make
dev-mock` on an invented world (9 500 servers); `make check` is what every
change must pass (cargo xtask ci, the domain crates' clippy and tests,
pytest, ruff); `make fmt` formats everything; `make screenshots` renders every
view headless (`cargo xtask shots`), `cargo xtask visual-diff` compares them
with `var/reference/`. `make build`, `make appimage`, `make windows` build
releases locally. `make publish`, `make prerelease`, `make aur` and `make keys`
are the owner's to run (`DRY=1` shows the plan); never run them or push
without being asked.

## Version control

Colocated **jj** repo. One jj change per logical step, committing only the
paths you touched: `jj commit <paths> -m "…"`. Message style:
`<scope>: <what is now true, in plain prose>`, a body saying why, then the
Co-Authored-By trailer. `scripts/link-mods.bat` is the owner's file: never
commit or edit it.

## Rules learned the hard way

- **Heavy work belongs in Rust below the window.** The server list (9k+ rows)
  is a snapshot in `dz-hub`, filtered, sorted and counted into an index order;
  the window draws only the rows on screen.
- **Virtual lists need a bounded height.** A list whose height follows its
  content lays out every row: the server view once built all 9 499 rows and
  froze the window (in the web view: 66 MB of DOM). Give the scroller a fixed
  height and draw with `ScrollArea::show_rows` or the kit's virtual list.
- **Same identity as the Tauri build.** Executable `dayz-community-hub`,
  identifier `com.thoxy.dayz-community-hub` and its data folders
  (`dz_hub::paths`), the release file names, `latest.json`'s keys, the updater
  endpoint and public key (`crates/app/hub/src/features/updater/release.rs`):
  an installed copy updates across releases only while these hold.
- **Signatures**: `cargo xtask sign` writes the `.sig` files the updater
  checks, with the owner's existing key and password. Never read, print, copy
  or move a key or a secret (`.env`, `~/.config/dayz-community-hub/`,
  variables with KEY/PASSWORD/TOKEN in their name).
- **Cross-platform**: every `#[cfg(windows)]` path must keep working (`cargo
  xtask ci` runs `cargo xwin check --target x86_64-pc-windows-msvc`). SteamCMD
  must run in its own PTY/ConPTY; do not replace it with piped I/O.
- **glibc floor**: Linux releases are linked against glibc 2.35 (the Ubuntu
  22.04 container, or zig for `make build`); a system library that needs a
  newer glibc cannot be linked (OpenSSL is vendored for that reason).
- **One build per package**: name `-p dayz-community-hub --bin
  dayz-community-hub` in release builds; parallel agents use their own
  absolute `CARGO_TARGET_DIR` and never `target/check` or `target/debug`.

## Looking at the window without a display

`cargo run -p dayz-community-hub --example ui_shot -- script.jsonl` renders
on the mock hub (`{"view": "servers"}`, `{"theme": "chernarus"}`,
`{"locale": "fr"}`, `{"command": …}`, `{"shot": "/abs.png"}`; see the
example's header). A running window also answers JSON lines on
`--control <port>` (`docs/control-protocol.md`).
