# Attribution

Every non-code asset the egui window (`apps/desktop`) ships, with its author, source and
licence. The launcher's own code and original assets are MIT (see [`LICENSE`](LICENSE)). When
you add an asset, add a row here in the same change; third-party assets must be permissively
licensed and keep their licence file next to them.

The icons are the ones the Svelte interface (`apps/gui`) imports as `~icons/<set>/<name>`,
extracted from its `@iconify-json/*` packages by [`scripts/icons.py`](scripts/icons.py), which
also writes `apps/desktop/src/icons/list.rs`. Rerun it after the interface's icons change, and
update the rows below.

## Bundled in the window

| Path | Title | Author | Source | Licence |
|---|---|---|---|---|
| `apps/desktop/assets/fonts/IBMPlexSans-*.ttf` (400, 500, 600, 700; latin, latin-ext, cyrillic, cyrillic-ext subsets) | IBM Plex Sans (UI face) | IBM Corp. ("Copyright 2019 IBM Corp. All rights reserved.", Reserved Font Name "Plex") | <https://github.com/IBM/plex> | SIL OFL 1.1, [`apps/desktop/assets/fonts/OFL.txt`](apps/desktop/assets/fonts/OFL.txt) |
| `apps/desktop/assets/fonts/IBMPlexMono-*.ttf` (400, 500, 600; same subsets) | IBM Plex Mono (numbers, addresses) | IBM Corp. ("Copyright 2017 IBM Corp. All rights reserved.", Reserved Font Name "Plex") | <https://github.com/IBM/plex> | SIL OFL 1.1, [`apps/desktop/assets/fonts/OFL.txt`](apps/desktop/assets/fonts/OFL.txt) |
| `apps/desktop/assets/fonts/SofiaSansExtraCondensed-*.ttf` (600, 800; same subsets) | Sofia Sans Extra Condensed (display face) | The Sofia Sans Project Authors (Lettersoup: Botio Nikoltchev, Ani Petrova) ("Copyright 2019 The Sofia Sans Project Authors") | <https://github.com/lettersoup/Sofia-Sans> | SIL OFL 1.1, [`apps/desktop/assets/fonts/OFL.txt`](apps/desktop/assets/fonts/OFL.txt) |
| `apps/desktop/assets/icons/lucide/*.svg` (161 files) | Lucide icons | Lucide Contributors; the icons derived from Feather (listed in the licence file, e.g. `server`, `search`, `x`) by Cole Bemis | <https://github.com/lucide-icons/lucide> via `@iconify-json/lucide` | ISC (Lucide) and MIT (Feather), [`apps/desktop/assets/icons/LICENSE-lucide.txt`](apps/desktop/assets/icons/LICENSE-lucide.txt) |
| `apps/desktop/assets/icons/game-icons/camping-tent.svg` | Camping tent (offline mode) | Delapouite | <https://game-icons.net/1x1/delapouite/camping-tent.html> via `@iconify-json/game-icons` | CC BY 3.0, <https://creativecommons.org/licenses/by/3.0/> (attribution: this row) |
| `apps/desktop/assets/icons/boxicons/*.svg` (none today: the interface imports no boxicons; the row is kept for when it does) | Boxicons | Boxicons | <https://github.com/box-icons/boxicons-core> via `@iconify-json/boxicons` | MIT |
| `apps/desktop/src/kit/marks.rs` (`Brand`: Steam, Linux/Tux and Windows marks, inline SVG paths, as `apps/gui/src/lib/components/ui/brand`) | Simple Icons | Simple Icons contributors | <https://simpleicons.org> | CC0 1.0 (the marks themselves are their owners' trademarks) |
| `apps/desktop/assets/icons/circle-flags/*.svg` (316 files: every two-letter code of the set, as `ui/flag` draws a server's country) | Circle Flags | HatScripts | <https://github.com/HatScripts/circle-flags> via `@iconify-json/circle-flags` | MIT, [`apps/desktop/assets/icons/LICENSE-circle-flags.txt`](apps/desktop/assets/icons/LICENSE-circle-flags.txt) |

The SVG files are the iconify bodies wrapped in an `<svg>` with the set's box; monochrome sets
get `color="#ffffff"` so their `currentColor` rasterises white and is tinted when drawn. No
other change is made to the drawings.
