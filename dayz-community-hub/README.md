# DayZ Community Hub

A desktop app for DayZ Standalone — server browser, mod manager, and launcher in one.

> Windows & Linux · Built with Tauri 2, SvelteKit 5, Rust

---

## Features

- **Server browser** — live public server list with search and filters (map, mods, 1st person, password, BattleEye)
- **Favorites & history** — star servers, reconnect with one click, see relative timestamps
- **Mod manager** — install, update, and clean up Workshop mods via SteamCMD
- **One-click launch** — connects to a server and installs missing mods automatically
- **BattleMetrics** — rank, uptime %, country, and 24h player graph per server (optional API token)
- **News** — latest DayZ articles fetched from the official site
- **Direct connect** — join by IP:port without browsing the list
- **Offline mode** — download and launch DayZ Community Offline Mode missions
- **Auto-updater** — Windows only; checks for new versions in the About tab

---

## Download

Go to [Releases](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases) and grab the latest zip for your platform.

On Windows, extract and run `dayz-community-hub.exe`.  
On Linux, install via your package manager or run the binary directly.

---

## Building from source

**Prerequisites:** Rust, Bun, [Tauri prerequisites](https://tauri.app/start/prerequisites/)

```bash
git clone https://git.thoxy.xyz/thoxy/dayz-community-hub
cd dayz-community-hub
bun install
bun tauri build
```

**Windows cross-compile from Linux** (requires [cargo-xwin](https://github.com/rust-cross/cargo-xwin)):

```bash
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/dayz-community-hub.key)" \
  bun tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
```

---

## Release checklist

1. Bump `version` in `tauri.conf.json`, `Cargo.toml`, and `package.json`
2. Commit, tag `vX.Y.Z`, and push
3. Build the Windows binary with the signing key set
4. Sign the `.exe` with `minisign`
5. Zip the binary and craft `latest.json` (see format below)
6. Create a Forgejo release `vX.Y.Z` and upload the zip + `latest.json`
7. Update the `latest` release with the new `latest.json` (used by the updater endpoint)

**`latest.json` format:**
```json
{
  "version": "0.2.0",
  "notes": "Release notes",
  "pub_date": "2026-01-01T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contents of .exe.sig>",
      "url": "https://git.thoxy.xyz/thoxy/dayz-community-hub/releases/download/vX.Y.Z/dayz-community-hub-vX.Y.Z-x86_64-windows.zip"
    }
  }
}
```

---

## Tech stack

| | |
|---|---|
| [Tauri 2](https://tauri.app) | Desktop shell + native APIs |
| [SvelteKit 5](https://svelte.dev) | Frontend (runes, no SSR) |
| [Rust](https://www.rust-lang.org) | Backend logic |
| [DaisyUI 5](https://daisyui.com) + [Tailwind](https://tailwindcss.com) | Styling |

---

## License

MIT — see [LICENSE](LICENSE)
