# Control protocol

The window can be driven from outside: by an agent, a script or a test. The same requests run
in a live window (`--control <port>`) and in the headless `ui_shot` example. The code is
`apps/desktop/src/control.rs`.

## Starting

```sh
cargo run -p dayz-community-hub -- --mock --control 7981
```

The window listens on `127.0.0.1:<port>` only. The server runs on the tokio runtime, never on
the UI thread. If the port is taken, or no key can be made or written, the window says why on
stderr and runs without the channel.

Each request is one JSON object per line, and each gets one reply line:

```json
{"id": 1, "key": "…", "method": "ui.view", "params": {"id": "mods"}}
{"id": 1, "ok": true, "result": {"view": "mods"}}
```

Errors come back as `{"id": …, "ok": false, "error": "message"}`. The `id` is echoed and can be
any JSON value, or left out.

## Keys

Any local program (and possibly a web page) can reach a loopback port, so every request needs the
window's key. Once the port is bound, the window writes a random key (256 bits from the OS, as
hex) to `<data dir>/control-key.local-<port>`. The file is created with mode 0600 on Unix, a
stale one is replaced, and the file is removed when the window closes. The data directory is the
hub's: `dz_hub::paths::data_dir()`, which is `--data-dir` when given and a fresh temporary
directory in `--mock` mode. The path is printed on stderr at start:

```text
control: listening on 127.0.0.1:7981, key in /tmp/…/control-key.local-7981
```

- Send the key as top-level `"key"` with every request, or once per connection with
  `{"method": "auth", "key": "…"}` (reply `{"ok": true, "result": {}}`). After that, requests on
  the connection may leave `key` out. Only `auth` reads `params.key`; for any other method,
  `params.key` is an ordinary parameter (`ui.key {"key": "Enter"}`).
- A missing or wrong key gets `{"ok": false, "error": "unauthorized"}`, and the connection is
  closed. Keys are compared in constant time.
- A line that is not a JSON object gets one error reply, then the connection is closed. This way
  an HTTP request sent to the port cannot carry a command.
- A line over 1 MiB gets `"line too long"`, then the connection is closed.
- A connection that sends no authorised request within 30 s of being accepted is closed. Blank
  lines and bad JSON do not extend that limit. After an authorised request, the connection may
  stay idle.

```sh
KEY=$(cat "$DATA_DIR/control-key.local-7981")
printf '{"id":1,"key":"%s","method":"ui.state"}\n' "$KEY" | nc -q1 127.0.0.1 7981
```

## How requests reach the window

- **`hub.*`** requests are answered on the runtime, straight from the hub. The UI thread never
  sees them, and they work even while the window is minimised.
- **Every other request** goes through a channel to the UI thread and wakes the window. It is
  handled at the start of the next frame, after the hub's events and before the frame draws (at
  most 64 per frame), and the reply comes back on a oneshot channel. An idle window stays idle:
  nothing polls.
- A request that gets no answer within 60 s is answered with `"timeout"`.
- Input methods (`ui.click`, `ui.key`, …) reply `{"queued": true}` once the events are queued.
  The events reach the widgets on the following frame, so read `ui.state` afterwards instead of
  assuming the effect is already there.

## Methods

| Method | Params | Result |
|---|---|---|
| `auth` | `key` | `{}`; the connection is authorised |
| `ui.state` | — | `view`, `focus`, `locale`, `theme` (worn), `themeSelected` (chosen; `null` = follow the system), `railCollapsed`, `scale`, `window` [w, h], `initialized`, `initError`, `wizard`, `dialogs` {palette, shortcuts, confirm}, `counts` {servers, maps, favorites, history, mods, pinged}, `busy` {servers, mods}, `pad` {mode (`gamepad`/`pointer`), focus {kind (`widget`/`text`/`list`), rect [x, y, w, h], `resting` (a text field the pad rests on without typing), cursor (a list's)}, pads} |
| `ui.commands` | — | every command of the registry: `id`, `label` (current language), `group`, `enabled` |
| `ui.view` | `id` (a rail id: `servers`, `favorites`, `history`, `friends`, `connect`, `offline`, `mods`, `options`, `news`, `stats`, `settings`, `about`), `focus?` (a part, as in `settings#apis`) | `{"view": id}` |
| `ui.command` | `id`, `args?` (JSON object) | `{}`, or the command's error. Unknown and disabled commands are refused |
| `<command id>` | the command's arguments | as `ui.command`: `{"method": "rail.toggle_collapse"}` works |
| `ui.locale` | `code` (`en` `fr` `de` `es` `ru`) | `{"locale": code}`; the choice is kept |
| `ui.theme` | `id`: a preset (`chernarus`, `topo`, …), a custom theme's id, `custom` (the first custom theme) or `system` | `{"theme": selected}`; a retired preset becomes its successor |
| `ui.size` | `w`, `h` (points, clamped to 320–16384 × 240–16384) | the size asked for |
| `ui.screenshot` | `path` (absolute) | `{"path", "width", "height"}`: the next frame as PNG |
| `ui.click` | `x`, `y` (points), `button?` (`left`, `right`, `middle`), `count?` (1–3), `ctrl?` `shift?` `alt?` | `{"queued": true}` |
| `ui.move` | `x`, `y` | `{"queued": true}` |
| `ui.wheel` | `x`, `y`, `dx?`, `dy` (points; `dy` < 0 scrolls down), modifiers | `{"queued": true}` |
| `ui.key` | `key` (`Enter`, `Escape`, `F1`, `A`, `ArrowDown`…), `ctrl?` `shift?` `alt?` | `{"queued": true}`; shortcuts apply |
| `ui.text` | `text` (≤ 64 KiB) | `{"queued": true}`: typed into the focused field |
| `ui.pad` | `button` (`dpad_up` `dpad_down` `dpad_left` `dpad_right` `a` `b` `x` `y` `lb` `rb` `lt` `rt` `menu` `view` `l3` `r3` `rs_up` `rs_down`, or an action's name: `down`, `accept`, `prevTab`…), `repeat?` | `{"queued": true}`: acted on next frame as if a pad sent it (the input mode follows unless the setting is Never); `ui.state`'s `pad` says where the focus went |
| `hub.servers_query` | a `ServerQuery`, camelCase; every field is optional (`search`, `maps` (a list of maps, none for all; `map`, one name or null, still reads), `firstPerson` / `password` / `battleye` / `modded` / `official`: `all` `only` `none`, `hideEmpty`, `hideFull`, `maxPing`, `showExcluded`, `timeOfDay`: `any` `day` (06:00–20:00 in game) `night`, `withMod` (a Workshop id or part of a mod's name), `ready` (every mod on disk and up to date, and the server's version the game's when both are known) with `gameVersion` (the installed game's, or null), `sort`: `none` `ping` `players` `name` `map` `mods` `time`, `asc`, `offset`, `limit` ≤ 1000, 50 by default) | a `ServerPage`: `total`, `rows`, `stats`, `generation` |
| `hub.servers_lookup` | `keys`: `["ip:port", …]` (≤ 1000) | a row or `null` per key |
| `hub.server_maps` | — | `[{map, count}, …]` |
| `hub.get_profile` | — | the profile |
| `hub.get_installed_mods` / `hub.mods_usage` | — | the installed mods / where each mod is used |
| `hub.get_app_stats` / `hub.refresh_state` / `hub.sources_status` / `hub.ping_scan_state` | — | the hub's state |
| `hub.game_running` / `hub.gamepad_status` / `hub.get_system_specs` | — | as named |

`hub.*` methods are read-only on purpose. To change something, run the command a user would:
`ui.command`. The commands are not listed here: `ui.commands` lists every one the window
registers (`apps/desktop/src/commands.rs`), with its label and whether it is enabled now.

A click by label is not available on a live window, since egui does not expose the accessibility
tree to the app. `ui_shot` offers it through AccessKit: `{"click": "Rafraîchir"}`.

## Headless: `ui_shot`

```sh
cargo run -p dayz-community-hub --example ui_shot -- script.jsonl
```

This renders the whole window offscreen with wgpu, on a fresh mock hub, with default preferences
(the user's files are not touched). It drives the same `HubApp::ui` as the real window, and the
hub runs on a real tokio runtime, so the mock's timers take real time. The script holds one JSON
object per line; blank lines and `//` lines are skipped:

| Line | Effect |
|---|---|
| `{"size": [w, h]}` | the window's size in points (read before the first frame) |
| `{"ppp": 1.0}` | pixels per point (default 1) |
| `{"locale": "fr"}` | the language |
| `{"theme": "chernarus"}` | a theme (as `ui.theme`) |
| `{"view": "servers", "focus": "…"}` | open a view |
| `{"command": "id", "args": {…}}` | run a command |
| `{"steps": n}` | settle, then run `n` frames |
| `{"wait": ms}` | run frames for `ms` of real time |
| `{"click": "label"}` | click the widget with that accessible label |
| `{"method": "ui.…", "params": {…}}` | a control request, as above (`hub.*` excepted); the reply is printed (`UI_SHOT_FULL=1` prints it whole) |
| `{"shot": "/abs/out.png"}` | settle, then save the frame as a PNG |

**Settling.** Frames run, with real time passing, until the window has started (the mock's list,
profile and mods are in, or start-up failed; at most 30 s). They then keep running until the
window is quiet: no store loading, and three frames in a row that asked for no repaint within
500 ms (at most 4 s, because a spinner on screen repaints forever). Each frame advances egui's
clock by 1/60 s, so animations finish in a known number of frames.

The exit status is 0 when every line ran, 1 when a line failed (the other lines still run), and 2
when the script cannot be read.

`cargo xtask shots [views…]` writes one script per view and runs it (`xtask/src/shots.rs`).
`cargo xtask visual-diff` compares `var/shots/` with `var/reference/` and updates
`docs/ui-parity.md`.
