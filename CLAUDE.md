# DayZ Community Hub

A DayZ launcher (server browser, mod manager, offline mode) for **Windows and
Linux**, both with the same features. Tauri 2 + Rust, Svelte 5 + Vite.

## Layout

```
apps/gui/                 the interface (bun project) and its Tauri shell
  src-tauri/src/features/ one module per feature: its commands and DTOs
  src/content/            intlayer dictionaries, 5 languages (en fr de es ru)
  src/features/<view>/    one folder per view: View.svelte, its parts, its store
  src/shell/              title bar, rail, status bar, global dialogs, nav registry
  src/lib/components/ui/  the generic kit (bits-ui + tailwind-variants)
  src/lib/components/app/ components that know this app, shared by every view
  src/lib/ipc/            typed wrappers over the generated bindings.ts
  src/lib/stores/         app-wide state (servers, profile, mods, connect…)
crates/core/*             dz-* libraries: api, a2s, profile, steamcmd, game, news, common
crates/app/*              app-level libraries (battlemetrics)
tools/                    uv project: `dzch release`
docs/build.md             build, check, publish, known failures
```

## Commands

`make` lists them. `make dev` runs the app; `make check` is what every change
must pass (cargo test + clippy, svelte-check, pytest, ruff); `make bindings`
regenerates `apps/gui/src/lib/ipc/bindings.ts` after a command or DTO changes;
`make fmt` formats everything. `make publish` (AppImage + Windows zip on
git.thoxy.xyz and GitHub, plus latest.json), `make aur` and `make keys` are
the owner's to run (`DRY=1` shows the plan); never run them or push without
being asked.

## Version control

Colocated **jj** repo. One jj change per logical step, committing only the
paths you touched: `jj commit <paths> -m "…"`. Message style:
`<scope>: <what is now true, in plain prose>`, a body saying why, then the
Co-Authored-By trailer. `scripts/link-mods.bat` is the owner's file: never
commit or edit it.

## Rules learned the hard way

- **Heavy work belongs in Rust; the interface only displays results.** The
  server list (9k+ rows) is filtered, sorted, counted and paged by
  `servers_query`; the UI holds only the rows on screen. Ping scans report
  progress, not per-server results; `servers-changed` (≤ 2/s) tells views to
  re-query.
- **Virtual lists need a bounded scroller.** A scroller whose height follows
  its content makes every row "visible": the server view once mounted all
  9 499 rows (66 MB of DOM) and froze the window. Pin scrollers with
  `absolute inset-0` inside a sized box; `Split` panes need a flex parent.
- **Read words with `dict(key)` from `$lib/i18n`, never `useIntlayer`**: the
  latter re-transforms the whole dictionary per component instance. Outside
  components use `words(key)`. Insert keys are functions:
  `$c.loaded({ count }).value`. Attribute values need `.value`.
- **Tokens only.** Tailwind's stock colours, text sizes and radii are wiped
  (`styles/tokens.css`); `text-white`, `text-lg` etc. render nothing. Use
  `bg-bg/panel/raised`, `text-fg/fg-muted/fg-faint`, `accent`, `ok/warn/err/info`,
  `map`, `mods`, `h-control`, `h-row`, `px-pad`…
- **Reuse before writing.** A header, a figure, a server's flags, a ping or
  players cell, an empty state: they exist in `lib/components/app`. Extend
  the shared component rather than copying it into a feature.
- **Never pass `name` to a bits-ui primitive** (hidden inputs crash
  WebKitGTK); `tv` comes from `tailwind-variants/lite`.
- **Cross-platform**: every `#[cfg(windows)]` path must keep working
  (`cargo xwin check --target x86_64-pc-windows-msvc`). SteamCMD must run in
  its own PTY/ConPTY; do not replace it with piped I/O.
- **No `.svelte` file may import from the old paths** (`lib/actions`,
  `lib/services`, paraglide): they are gone.

## Looking at the interface without Tauri

`VITE_MOCK=1 bunx vite build --outDir dist-mock` then `bunx vite preview
--outDir dist-mock` serves the app on a pretend backend (`lib/ipc/mock.ts`,
9 500 invented servers). URL flags: `?theme=<preset>`, `?view=<id>`,
`?focus=<section>`, `?rail=collapsed`, `?wizard=1`. Headless Brave screenshots
it: `brave --headless=new --screenshot=out.png --window-size=1400,860
--virtual-time-budget=6000 <url>`. In development an uncaught error is written
into the page, so a screenshot shows it.
