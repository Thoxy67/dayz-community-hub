# DayZ Community Hub

[![CI](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions/workflows/ci.yml/badge.svg)](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions?workflow=ci.yml)
[![Release](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions/workflows/release.yml/badge.svg)](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions?workflow=release.yml)
[![Version](https://img.shields.io/badge/version-0.3.3-blue?style=flat-square)](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-lightgrey?style=flat-square)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square&logo=rust&logoColor=white)](https://rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/SvelteKit-5-FF3E00?style=flat-square&logo=svelte&logoColor=white)](https://svelte.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-4-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![DaisyUI](https://img.shields.io/badge/DaisyUI-5-5A0EF8?style=flat-square&logo=daisyui&logoColor=white)](https://daisyui.com/)

A desktop launcher for DayZ Standalone that replaces the official launcher with a faster, more complete experience — server browser, mod manager, and auto-updater in a single app.

![DayZ Community Hub screenshot](.forgejo/assets/screenshot.png)

---

## What it does

**Browse servers** — pulls the full public server list (~18 000 servers), lets you filter by map, mods, 1st-person, password, and BattleEye, and shows live ping for every server.

**Manage mods** — installs, updates, and removes Workshop mods through SteamCMD with a live progress feed. Missing mods for a server are detected automatically before you connect.

**Launch in one click** — picks up your launch options, sets up mod symlinks, starts Steam if needed, and connects directly to the server.

**Track your servers** — star favorites, browse your session history with timestamps, and reconnect to your last server from the top bar.

**BattleMetrics** — shows rank, status, country, uptime percentage, and a 24-hour player count graph per server (requires a free BattleMetrics API token).

**News** — latest articles from the official DayZ website, in-app.

**Direct connect** — join any server by IP and port without browsing the list.

**Offline mode** — installs and launches [DayZ Community Offline Mode](https://github.com/Arkensor/DayZCommunityOfflineMode) missions, with one-click save wipe.

**Auto-updater** — Windows only; checks for new releases and applies them in-place without reinstalling.

---

## Documentation

Full documentation is available in the [Wiki](https://git.thoxy.xyz/thoxy/dayz-community-hub/wiki) — setup guide, configuration reference, mod manager, architecture, and CI/CD details.

---

## Download

Go to [Releases](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases) and grab the latest zip for your platform.

- **Windows** — extract and run `dayz-community-hub.exe`
- **Linux** — run the binary directly or install the `.deb` package
- **Arch Linux** — install via the AUR package [`dayz-community-hub-git`](https://git.thoxy.xyz/AUR/dayz-community-hub-git):

```bash
git clone https://git.thoxy.xyz/AUR/dayz-community-hub-git.git
cd dayz-community-hub-git
makepkg -si
```

---

## Building from source

**Prerequisites:** Rust (nightly), Bun, [Tauri v2 system deps](https://tauri.app/start/prerequisites/)

```bash
git clone https://git.thoxy.xyz/thoxy/dayz-community-hub
cd dayz-community-hub
bun install
bun tauri dev        # development
bun tauri build      # production
```

**Cross-compile for Windows from Linux** (requires [cargo-xwin](https://github.com/rust-cross/cargo-xwin)):

```bash
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin
TAURI_SIGNING_PRIVATE_KEY="" bun tauri build \
  --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
```

---

## Tech stack

| | |
|---|---|
| [Tauri 2](https://tauri.app) | Desktop shell, native APIs, auto-updater |
| [SvelteKit 5](https://svelte.dev) | Frontend (runes, SPA mode) |
| [Rust](https://www.rust-lang.org) | All backend logic |
| [DaisyUI 5](https://daisyui.com) + [Tailwind 4](https://tailwindcss.com) | Styling |

---

## License

MIT — see [LICENSE](LICENSE)
