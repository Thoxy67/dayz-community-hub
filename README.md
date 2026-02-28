# DayZ Community Hub

[![CI](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions/workflows/ci.yml/badge.svg)](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions?workflow=ci.yml)
[![Release](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions/workflows/release.yml/badge.svg)](https://git.thoxy.xyz/thoxy/dayz-community-hub/actions?workflow=release.yml)
[![Version](https://img.shields.io/badge/version-0.3.3-blue?style=flat-square)](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20(AppImage%2C%20deb%2C%20rpm)-lightgrey?style=flat-square)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)

[![Rust](https://img.shields.io/badge/Rust-2024-orange?style=flat-square&logo=rust&logoColor=white)](https://rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/SvelteKit-5-FF3E00?style=flat-square&logo=svelte&logoColor=white)](https://svelte.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![TailwindCSS](https://img.shields.io/badge/Tailwind-4-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![DaisyUI](https://img.shields.io/badge/DaisyUI-5-5A0EF8?style=flat-square&logo=daisyui&logoColor=white)](https://daisyui.com/)

A fast, feature-rich DayZ launcher that replaces the official one — browse servers, manage mods, and connect in one click.

![DayZ Community Hub screenshot](.forgejo/assets/screenshot.png)

---

## Quick Start

1. **Download** — grab the [latest release](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases) for your platform
2. **Run** — extract and launch the app, complete the setup wizard
3. **Play** — find a server, hit Connect, and the app handles the rest

---

## Features

### Browse
- **18,000+ servers** — full public server list with live ping
- **Filter & search** — by map, mods, 1PP, password, BattleEye
- **Favorites & history** — star servers and track your sessions
- **BattleMetrics** — rank, uptime, and 24h player graphs (API key required)

### Mods
- **Auto-install** — missing mods download via SteamCMD before you connect
- **Update checker** — detect stale mods with one click
- **Bulk operations** — update, link, or delete multiple mods at once
- **Symlink management** — clean mod organization without duplicating files

### Launch
- **One-click connect** — starts Steam if needed, sets up mods, and joins
- **Direct connect** — join any server by IP:port
- **Launch options** — full control over DayZ startup flags
- **Offline mode** — play [DayZ Community Offline Mode](https://github.com/Arkensor/DayZCommunityOfflineMode) missions locally

### Extras
- **News feed** — latest DayZ articles in-app
- **Auto-updater** — Windows: updates apply in-place
- **Cross-platform** — runs on Windows and Linux
- **Keyboard-driven** — full keyboard navigation for power users

---

## Download

Go to [Releases](https://git.thoxy.xyz/thoxy/dayz-community-hub/releases) and grab the latest version.

### Windows
Extract the `.zip` and run `dayz-community-hub.exe`

### Linux

| Format | Distros | Install |
|--------|---------|---------|
| `.AppImage` | Any | `chmod +x *.AppImage && ./dayz-community-hub.AppImage` |
| `.deb` | Debian, Ubuntu, Mint, Pop!_OS | `sudo dpkg -i dayz-community-hub.deb` |
| `.rpm` | Fedora, openSUSE, RHEL, CentOS | `sudo rpm -i dayz-community-hub.rpm` |

#### Arch Linux (AUR)

```bash
git clone https://git.thoxy.xyz/AUR/dayz-community-hub-git.git
cd dayz-community-hub-git
makepkg -si
```

---

## Documentation

Full docs available in the [Wiki](https://git.thoxy.xyz/thoxy/dayz-community-hub/wiki) — setup, configuration, mod management, and more.

---

## Building from Source

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

## License

MIT — see [LICENSE](LICENSE)
