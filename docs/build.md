# Build

One Rust workspace (`crates/`, `apps/desktop`, `xtask`) and one Python
project (`tools/`, run through [uv](https://docs.astral.sh/uv/)). The launcher
is the egui window `apps/desktop` (package `dayz-community-hub`) on the
backend `crates/app/hub`; it builds the executable `dayz-community-hub`
(`.exe` on Windows), the name every release has shipped (the Tauri/Svelte
interface it replaced is gone). `make` alone lists the targets;
this page explains what each one does and why.

## Layout

```
apps/desktop/        the egui window (the launcher's executable)
crates/app/hub/      the backend the window talks to: state, commands, updater
crates/<role>/<name> the domain libraries: server list, A2S, SteamCMD, profile…
xtask/               cargo xtask: ci, shots, parity, visual-diff, the updater's signer
packaging/           the AppImage container, the AUR PKGBUILD, the .desktop and MIME files
tools/dzch/          the release tool (`dzch release`, `aur`, `keys`)
scripts/             helpers the Makefile and the release tool call; icons.py
                     rewrites apps/desktop/src/icons/list.rs from the SVG folder
var/                 what builds leave behind (git-ignored): var/dist/
```

## Install

- Rust, as pinned in `rust-toolchain.toml` (rustup picks it up on its own),
  with `clippy` and `rustfmt`.
- On Linux, libudev's headers for the controller support (gilrs, through
  `libudev-sys`): `libudev-dev` on Debian/Ubuntu, `systemd-libs` on Arch
  (always installed), `systemd-devel` on Fedora; and zstd's (`libzstd-dev`).
  At run time the app reads pads from `/dev/input/event*`; without access to
  them it starts anyway and says so on stderr ("controllers unavailable").
- Nothing more for the tray, the notifications, Discord or the desktop
  shortcuts. On Linux the tray (ksni) and the notifications (notify-rust's
  zbus backend) speak D-Bus in pure Rust: no libdbus, GTK or
  libappindicator headers to install, nothing more linked. At run time the
  tray needs a StatusNotifierItem host (KDE, most panels, GNOME with the
  AppIndicator extension) and notifications a notification daemon; without
  them the launcher runs without (closing the window then quits). On
  Windows the tray is `Shell_NotifyIcon` through tray-icon (windows-sys) and
  the notifications WinRT toasts, both in the executable; the desktop
  shortcut is a `.lnk` written by mslnk, without COM. Discord's Rich
  Presence is its local IPC socket or named pipe, no SDK.
- uv for everything under `tools/`: `tools/pyproject.toml` and `tools/uv.lock`
  are the one Python project, and every `make` target that needs it runs it as
  `uv run --project tools ...`. Nothing here touches the system Python.
- For releases only: `podman` (the AppImage is built in a container, see
  below), `cargo install cargo-xwin` and `rustup target add
  x86_64-pc-windows-msvc` (Windows). `zig` and `cargo install
  cargo-zigbuild` for `make build` (Linux glibc floor). No bun, no Tauri CLI.

## Every day

### `make dev`, `make dev-mock`

`cargo run -p dayz-community-hub`, the window on the real backend, into
`target/debug`: plain cargo for the host, no zig and no glibc floor, a dev
build only has to run here. `make dev-mock` adds `--mock`, an invented world
(9 500 servers, mods, a profile) in a temporary directory.

### `make check`

Everything a commit must pass, fastest first: `cargo fmt --all --check`,
`ruff check tools`, `pytest` over `tools/`, then `cargo xtask ci` (fmt,
clippy `-D warnings`, the tests of `dz-hub`, the window and `xtask`, the
assets, layers and parity checks, and `cargo xwin check` for Windows when
cargo-xwin is installed), then clippy and the tests of the domain crates
`xtask ci` does not cover. All with an absolute
`CARGO_TARGET_DIR=$(CURDIR)/target/check`, so a check never fights `make dev`
over the build lock.

### `make fmt`

`cargo fmt --all` and `ruff format tools`.

### `make screenshots`

`cargo xtask shots`: every view rendered headless on the mock hub (the
`ui_shot` example) into `var/shots/<view>.png`; `VIEWS="servers mods"` limits
the views. `cargo xtask visual-diff` compares them with `var/reference/`, the
Svelte interface's screenshots kept as the visual record (files: nothing
regenerates them now).

The README's pictures are separate: `uv run --with pillow
scripts/readme_shots.py` renders them (English, 1280 × 800 at 1.25 pixels per
point, the mock's update offer off) into `.forgejo/assets/screenshots/`, with
the theme and seasonal grids put together; name some (`servers themes`) to
redo only those. Retake them when a view the README shows changes.

## Publish

```sh
make publish DRY=1                    # the whole plan: version, files, uploads, manifest, notes
make publish                          # the next version
make publish VERSION=0.5.0            # a particular one
make publish SKIP_GITHUB=1            # git.thoxy.xyz only
make prerelease                       # a preview: 0.4.2-pre after 0.4.1
make prerelease VERSION=0.5.0-beta.1  # a named one; publishing it again replaces it
```

It builds the **AppImage** and the **Windows zip** (no .deb/.rpm: Arch users
have the AUR package), signs both, and publishes the release on
git.thoxy.xyz/thoxy/dayz-community-hub **and** its GitHub mirror
(Thoxy67/dayz-community-hub; both tokens are required unless `SKIP_GITHUB=1`).
`latest.json` (the manifest the updater reads: `platforms.linux-x86_64` is the
AppImage, `platforms.windows-x86_64` the zip, each with its signature) goes on
the release and on the rolling `latest` release, the endpoint built into
`dz-hub` (`crates/app/hub/src/features/updater/release.rs`, `ENDPOINT`). Names,
format and endpoint are the Tauri build's, so a copy of the Tauri launcher
updates to the egui one like to any other release.

**Notes** come from CHANGELOG.md: a `## x.y.z` section, else the `## Unreleased`
one (renamed to the version on release), else the jj log since the last tag
(`scripts/changelog.sh`). They are the release body on both forges and the
update banner's text.

### Signatures: `cargo xtask sign`

Every file the updater downloads has a `.sig` beside it: base64 of a minisign
signature (prehashed, trusted comment `timestamp:…\tfile:<name>`), the format
`tauri signer sign` wrote. `cargo xtask sign --key <key file> <file>` makes it
(`xtask/src/sign.rs`, the `minisign` crate the Tauri CLI uses), from the same
key file (base64 of a minisign secret key, scrypt-encrypted) and the same
password, read from `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; it never prompts,
prints or copies the key. Its tests sign with a throwaway key and check the
result with `dz-hub`'s updater verification and, when installed, the
`minisign` CLI. `dzch release` signs on the host after each build: the key
never enters the AppImage container.

### AUR: `make aur`

```sh
make aur DRY=1     # generate PKGBUILD + .SRCINFO in var/aur/, show the diff, push nothing
make aur-build     # build the package locally (var/aur/build), install with pacman -U
make aur           # commit "<pkgver>: <latest commit>" and push to git.thoxy.xyz/AUR/dayz-community-hub-git
make aur FORCE=1   # push even when nothing changed
```

The PKGBUILD lives in `packaging/aur/PKGBUILD.in`; `make aur` fills in the
version (`<workspace version>.r<commits on master>.g<hash>`, what its
pkgver() computes from `Cargo.toml`), writes `.SRCINFO` with `makepkg
--printsrcinfo`, and pushes from a clone in var/aur/ (over SSH;
`DZCH_AUR_REMOTE` overrides the remote). The package builds with cargo only
(`cargo build --frozen --release -p dayz-community-hub --bin
dayz-community-hub`) and installs the executable, the .desktop and MIME files
(`packaging/linux/`), the hicolor icons (`apps/desktop/assets/app/icon-*.png`)
and the licence. The executable links glibc, libgcc, zstd and libudev only;
libxkbcommon, OpenGL/EGL (libglvnd), Vulkan, Wayland and X11 are loaded with
dlopen, hence `depends`/`optdepends`. Publish the source first (`make
publish` pushes `master`): the package builds what is on `master`.

### Keys: `make keys`

`make keys` makes a new updater signing key with `cargo xtask keygen` (no
password, Tauri's format), where publish reads it (`TAURI_SIGNING_KEY_FILE`,
else `~/.config/dayz-community-hub/updater.key`), moving any existing key
aside as `updater.key.bak-<timestamp>`, and writes the public half into
`crates/app/hub/src/features/updater/release.rs` (`PUBKEY`). Commit that before the next publish.
**Every copy installed before the new key refuses updates signed with it**:
those need one manual update.

### `dzch release`

`tools/dzch/release/release.py` bumps the version, builds both platforms,
signs them for the updater, commits and tags the version, pushes, and
publishes. Run it again after anything fails and it carries on from where it
stopped: a version already tagged is not bumped again, files already built and
signed in `var/dist/vX.Y.Z/` are not rebuilt, an asset already uploaded is not
sent again.

**Always on this machine, never in CI**: it needs the signing key, and one
host building both platforms (Windows through cargo-xwin) is what makes the
two halves of `latest.json` agree. `mirror-releases.yml` stays, a manual
workflow that back-fills every past Forgejo release onto GitHub.

**What it needs** (each looked up in the environment, then a git-ignored
`.env` at the repository root, names in `.env.example`, then
`~/.config/dayz-community-hub/`):

- **The signing key.** `TAURI_SIGNING_KEY_FILE` (its password, if any, in
  `TAURI_SIGNING_KEY_PASS`), else `~/.config/dayz-community-hub/updater.key`.
  Its public half is built into `dz-hub` (`release.rs`, `PUBKEY`); every
  installed copy refuses an update not signed by it.
- **A Forgejo token** that can write releases on `thoxy/dayz-community-hub`:
  `FORGEJO_TOKEN`, else `~/.config/dayz-community-hub/forge-token`.
- **A GitHub token** for the `Thoxy67/dayz-community-hub` mirror:
  `GH_PAT` (or `GITHUB_TOKEN`), else `~/.config/dayz-community-hub/github-token`.
- **A clean working copy.** A release carries only its version bump, its
  changelog and the lock file; anything else stops it. The tool commits with jj
  (the repository is a colocated jj + git repo) and falls back to git.

**What it does:** picks the version (the current one if never released,
otherwise the next patch) and writes it to `Cargo.toml` (the one place it
lives); writes
the notes into `CHANGELOG.md`; builds

- Linux: `scripts/appimage.sh` in the Ubuntu 22.04 container (the same build
  as `make appimage`, see below), then `cargo xtask sign` on the AppImage;
- Windows: `cargo xwin build --release -p dayz-community-hub --bin
  dayz-community-hub` (with `DZCH_REQUIRE_WINRES=1`: no executable without its
  icon, version information and manifest), zips `dayz-community-hub.exe` and
  signs the zip. The zip is what the Windows updater downloads, checks against
  the public key and swaps over the running executable;

stages everything in `var/dist/vX.Y.Z/` under the names releases have always
used (`dayz-community-hub-vX.Y.Z-x86_64-windows.zip`, `…-x86_64.AppImage` and
their two `.sig`); commits `release: vX.Y.Z`, tags it, pushes `master` and the
tag; publishes the release on Forgejo with every file and `latest.json`,
uploads `latest.json` to the rolling `latest` release (the endpoint installed
copies poll:
`https://git.thoxy.xyz/thoxy/dayz-community-hub/releases/download/latest/latest.json`),
then does the same on the GitHub mirror.

**Previews** (`make prerelease`) build the same files with the preview's
version given to the build as `DZCH_VERSION` (what the executable reports and
the updater compares; `dz_hub::VERSION`), and publish them as a Forgejo (and
GitHub) pre-release `vX.Y.Z-pre`. They touch nothing in the repository and
**never `latest.json`**: the updater has a single channel, so a preview is
only ever installed by hand. Publishing the same preview again replaces it,
and each new preview or release removes the previews before it; stable
releases are never removed.

`--dry-run` (`uv run --project tools dzch release --dry-run`) says which
version would go out and with which notes, and changes nothing.

## The AppImage (container)

`make appimage` (`scripts/appimage.sh [output]`, default
`var/dist/dayz-community-hub-x86_64.AppImage`) builds the executable in an
**Ubuntu 22.04** container (`packaging/appimage/Containerfile`), so it asks
for glibc 2.35 and runs on any distribution with glibc 2.35 or newer (Ubuntu
22.04+, Debian 12+, Fedora 36+, Mint 21+, Arch, a current SteamOS…), then
`packaging/appimage/build.sh` assembles the AppDir and runs appimagetool.

- **What is in it:** `usr/bin/dayz-community-hub`, the .desktop
  (`packaging/linux/dayz-community-hub.desktop`: same name, `Exec`, icon and
  MIME types `x-scheme-handler/dzch;application/x-dzch` as before), the MIME
  package for `.dzch` files, the hicolor icons (32 to 256) and `AppRun` (a
  link to the executable). **No library**: the executable needs only glibc,
  libgcc, libzstd and libudev, which every desktop has (Mesa itself links
  libzstd), and wgpu/winit load Vulkan, OpenGL, Wayland, X11 and xkbcommon
  with dlopen, which must be the host's own (its drivers, its compositor).
- **appimagetool** 1.9.1 and the static type 2 runtime (no libfuse2 needed on
  the user's machine) are downloaded when the image is built, pinned by
  version and sha256 in the Containerfile; nothing is fetched at AppImage
  build time.
- **Needs** `podman` (`CONTAINER_ENGINE=docker` for docker). The first run
  builds the image (tagged with the Containerfile's hash, so editing it
  rebuilds it) and installs the toolchain `rust-toolchain.toml` names into a
  volume; later runs reuse both, plus the cargo registry (volumes
  `dzch-appimage-*`; `podman volume rm` them to start clean). The repository
  is mounted at `/src`; cargo writes to `target/appimage/`.
- **Nothing is signed in the container**: the key stays on the host.
- **Check an AppImage:** `./x.AppImage --version` prints the version;
  `./x.AppImage --appimage-extract`, then
  `objdump -T squashfs-root/usr/bin/dayz-community-hub | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1`
  must print `GLIBC_2.35` or lower.

## glibc floor (zig)

A binary linked on this machine (Arch) asks for the newest glibc symbol
versions the host has and refuses to start on anything older. `make build`
therefore links the executable with
[`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild) against zig's
glibc stubs at **2.35**, the AppImage's floor:
`scripts/cargo-zigbuild.sh build --release -p dayz-community-hub --bin
dayz-community-hub --target x86_64-unknown-linux-gnu`. The script adds the
floor as the triple's suffix (`x86_64-unknown-linux-gnu.2.35`, the only way
cargo-zigbuild takes it); the output lands in
`target/x86_64-unknown-linux-gnu/release/dayz-community-hub`.

- **Needs** `zig` and `cargo-zigbuild`. Without either, the script says so in
  one line and runs plain cargo with the same arguments.
- **Force plain cargo:** `make build ZIG=0`. The binary then needs this
  machine's glibc: fine for a local build, never for one handed to others.
- **No system library that needs a newer glibc.** zig refuses to link one
  (`referenced by /usr/lib/libssl.so (disallowed by
  --no-allow-shlib-undefined)`). That is why OpenSSL, which native-tls uses on
  Linux, is `vendored` in `crates/app/hub/Cargo.toml`: built from source by
  zig and linked statically.
- **Check a binary's floor:**
  `objdump -T target/x86_64-unknown-linux-gnu/release/dayz-community-hub | grep -o 'GLIBC_[0-9.]*' | sort -Vu | tail -1`
  must print `GLIBC_2.35` or lower.

## Windows

Cross-compiled from Linux with `cargo-xwin`:

```sh
rustup target add x86_64-pc-windows-msvc   # once
cargo install cargo-xwin                   # once
make windows
```

`make windows` runs `cargo xwin build --release -p dayz-community-hub
--bin dayz-community-hub --target x86_64-pc-windows-msvc` with
`DZCH_REQUIRE_WINRES=1`, and zips the executable into
`var/dist/dayz-community-hub-x86_64-windows.zip`. That zip is unsigned: the
signed one a release ships is made by `make publish`.

What makes the one executable enough on a bare Windows 10/11:

- **The resources** are embedded by `apps/desktop/build.rs` (winresource,
  `llvm-rc` under cargo-xwin): the icon (`apps/desktop/assets/app/icon.ico`,
  icon 0, which Explorer and the `dzch://`/`.dzch` registrations show), the
  VERSIONINFO (product "DayZ Community Hub", `OriginalFilename`
  `dayz-community-hub.exe`) and the manifest `apps/desktop/windows/app.manifest`
  (Common Controls v6, `asInvoker`, Windows 10/11 `supportedOS`,
  per-monitor-v2 DPI awareness, `longPathAware`). Check with
  `llvm-readobj --coff-resources <exe>`.
- **No console window**: `windows_subsystem = "windows"` in release builds; a
  panic or a start-up failure (no graphics adapter) is shown in a message box.
- **The window** is drawn with wgpu (Direct3D 12, else Vulkan): no WebView2.
- **The data** is where the Tauri build kept it: `%APPDATA%\dayz-community-hub`,
  the Steamworks library in `%LOCALAPPDATA%\dayz-community-hub\steamworks`.
- **Only the executable** goes in the zip. The updater swaps it in place: the
  running exe is renamed to `dayz-community-hub.old.exe` (removed at the next
  start), the new one moved in, and the old one moved back if that fails.

## Steamworks library

The optional "download mods through the Steam client" mode (`crates/core/steamworks`)
talks to Steam through Valve's redistributable, `libsteam_api.so` (Linux
x86_64) or `steam_api64.dll` (Windows x86_64). Nothing binary is committed:
`steamworks-sys` (pinned `=0.13.0` in the workspace `Cargo.toml`) is a
build-dependency of `dz-steamworks` only for its sources, which ship the SDK's
`lib/steam/redistributable_bin/`. Its build script finds them in cargo's
registry sources (`$CARGO_HOME/registry/src/*/steamworks-sys-0.13.0`), else
through `cargo metadata --offline` (vendored sources), and embeds the file for
the **target** with `include_bytes!`: the cross build through cargo-xwin gets
the DLL, the Linux build the `.so`, and the CI image needs nothing extra.
`DZ_STEAMWORKS_SDK=<sdk dir>` overrides the source. Other targets embed
nothing and the mode reports itself unavailable.

The app never links it. The first time the mode is used, the library is
written to `~/.local/share/dayz-community-hub/steamworks/<hash>/` (Linux) or
`%LOCALAPPDATA%\dayz-community-hub\steamworks\<hash>\` (Windows) and loaded
with `libloading`, which resolves the flat API by name (`SteamAPI_InitFlat`,
`SteamAPI_SteamUGC_v021`, …, checked against the SDK's `steam_api_flat.h`).
If it is missing or fails to load, only that mode fails, with a sentence
saying so. Moving to a newer `steamworks-sys` means checking those names
(the interface accessors are versioned) in `src/api.rs`. Valve's SDK licence
lets an application ship the redistributable.

## CI

`.forgejo/workflows/ci.yml` runs on every push to `master`, in the image
`.forgejo/ci-image/Dockerfile` builds (rebuilt by `ci-image.yml` when that
file changes): `cargo fmt --check`, the workspace's tests and clippy
`-D warnings`, `cargo xtask assets`, `layers` and `parity --check`, and
`cargo xwin check` of the window and its backend. It builds nothing that is
published.

## Known failures

- **Packaging with C LTO** (`makepkg` with `lto` on) breaks the link of
  `aws-lc-sys`: the AUR PKGBUILD sets `options=(!lto)`.
- **A release build without `-p dayz-community-hub --bin dayz-community-hub`**
  builds every crate of the workspace (xtask too): every build command here
  names both.
