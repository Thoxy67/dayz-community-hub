# DayZ Community Hub. `make` alone lists the targets.
#
#   make dev            the app (the egui window, apps/desktop) on the real backend
#   make dev-mock       the app on an invented world (9 500 servers, mods, a profile)
#   make check          everything a commit must pass: Rust (cargo xtask ci and the
#                       core crates), tools (ruff, pytest)
#   make fmt            format every file
#   make screenshots    every view rendered headless on the mock hub into var/shots
#                       (VIEWS="servers mods"; cargo xtask shots)
#   make build          the Linux release executable, glibc 2.35 floor (zig)
#   make appimage       the AppImage, built in an Ubuntu 22.04 container, into var/dist
#   make windows        the Windows executable (MSVC, via cargo-xwin), zipped into var/dist
#   make publish        build the AppImage and the Windows zip, sign, tag, publish on
#                       git.thoxy.xyz and GitHub, update latest.json (the updater's manifest)
#   make prerelease     the same as a pre-release, touching nothing in the repository
#   make aur            generate the AUR package (dayz-community-hub-git) and push it
#   make aur-build      build that package locally, to test it before `make aur`
#   make keys           a new updater signing key (the old one is kept aside)
#
# See docs/build.md.
# Knobs: VERSION=… ZIG=0 DRY=1 (publish, prerelease, aur, keys: say what would happen)
#        SKIP_GITHUB=1 (publish, prerelease: git.thoxy.xyz only)  FORCE=1 (aur)

DZCH := uv run --project tools dzch
# The launcher's package (apps/desktop, the egui window) and its executable.
PKG := dayz-community-hub
BIN := dayz-community-hub
CHECK_TARGET := $(CURDIR)/target/check
TARGET_DIR := $(or $(CARGO_TARGET_DIR),$(CURDIR)/target)
# Crates `cargo xtask ci` does not cover: the domain crates.
CORE := --workspace --exclude dz-hub --exclude $(PKG) --exclude xtask
# `make build` links through zig for a glibc 2.35 floor (see
# scripts/cargo-zigbuild.sh and docs/build.md); ZIG=0 builds with plain cargo.
ZIG ?= 1

.PHONY: help dev dev-mock check fmt screenshots windows build appimage publish prerelease aur aur-build keys
help: ; @sed -n '2,/^$$/p' Makefile | sed 's/^# \{0,1\}//'

dev:        ; cargo run -p $(PKG)
dev-mock:   ; cargo run -p $(PKG) -- --mock
# Fastest first, so a failure shows before the long Rust build.
check:
	cargo fmt --all --check
	uv tool run ruff check tools
	uv run --project tools pytest -q
	CARGO_TARGET_DIR=$(CHECK_TARGET) cargo xtask ci
	CARGO_TARGET_DIR=$(CHECK_TARGET) cargo clippy $(CORE) --all-targets -- -D warnings
	CARGO_TARGET_DIR=$(CHECK_TARGET) cargo test $(CORE)
fmt:
	cargo fmt --all
	uv tool run ruff format tools
screenshots: ; cargo xtask shots $(VIEWS)
build:
	ZIG=$(ZIG) scripts/cargo-zigbuild.sh build --release -p $(PKG) --bin $(BIN) \
	  --target x86_64-unknown-linux-gnu
appimage:   ; scripts/appimage.sh
# DZCH_REQUIRE_WINRES: no executable without its icon, version and manifest.
windows:
	DZCH_REQUIRE_WINRES=1 cargo xwin build --release -p $(PKG) --bin $(BIN) \
	  --target x86_64-pc-windows-msvc
	mkdir -p var/dist
	cd $(TARGET_DIR)/x86_64-pc-windows-msvc/release && \
	  rm -f $(CURDIR)/var/dist/dayz-community-hub-x86_64-windows.zip && \
	  python3 -m zipfile -c $(CURDIR)/var/dist/dayz-community-hub-x86_64-windows.zip $(BIN).exe
RELEASE_FLAGS = $(if $(VERSION),--version $(VERSION),) $(if $(DRY),--dry-run,) $(if $(SKIP_GITHUB),--skip-github,)
publish:    ; $(DZCH) release $(RELEASE_FLAGS)
prerelease: ; $(DZCH) release --pre $(RELEASE_FLAGS)
aur:        ; $(DZCH) aur $(if $(DRY),--dry-run,) $(if $(FORCE),--force,)
aur-build:  ; $(DZCH) aur --build
keys:       ; $(DZCH) keys $(if $(DRY),--dry-run,)
