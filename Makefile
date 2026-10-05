# DayZ Community Hub. `make` alone lists the targets.
#
#   make dev            the app, rebuilding as you edit
#   make check          everything a commit must pass: Rust, interface, tools
#   make fmt            format every file
#   make bindings       regenerate apps/gui/src/lib/ipc/bindings.ts from the Rust commands
#   make windows        the Windows executable (MSVC, via cargo-xwin), zipped into var/dist
#   make publish        build the AppImage and the Windows zip, sign, tag, publish on
#                       git.thoxy.xyz and GitHub, update latest.json (the updater's manifest)
#   make prerelease     the same as a pre-release, touching nothing in the repository
#   make aur            generate the AUR package (dayz-community-hub-git) and push it
#   make aur-build      build that package locally, to test it before `make aur`
#   make keys           a new updater signing key (the old one is kept aside)
#   make build | appimage                           see docs/build.md
#
# Knobs: VERSION=… ZIG=0 DRY=1 (publish, prerelease, aur, keys: say what would happen)
#        SKIP_GITHUB=1 (publish, prerelease: git.thoxy.xyz only)  FORCE=1 (aur)

DZCH := uv run --project tools dzch
CHECK_TARGET := $(CURDIR)/target/check
# `make build` links through zig for a glibc 2.35 floor (see
# scripts/cargo-zigbuild.sh and docs/build.md); ZIG=0 builds with plain cargo.
# The AppImage is built in an Ubuntu 22.04 container instead (scripts/appimage.sh):
# it carries libraries, and those must be old too.
ZIG ?= 1
LINUX_BUILD := ZIG=$(ZIG) NO_STRIP=true bun run tauri build \
               --runner $(CURDIR)/scripts/cargo-zigbuild.sh --target x86_64-unknown-linux-gnu

.PHONY: help dev check fmt bindings windows build appimage publish prerelease aur aur-build keys
help: ; @sed -n '2,/^$$/p' Makefile | sed 's/^# \{0,1\}//'

dev:        ; cd apps/gui && bun run tauri dev
check:
	cargo fmt --all --check
	CARGO_TARGET_DIR=$(CHECK_TARGET) cargo test --workspace
	CARGO_TARGET_DIR=$(CHECK_TARGET) cargo clippy --workspace --all-targets
	cd apps/gui && bunx svelte-check --threshold error
	cd apps/gui && bun test
	uv run --project tools pytest
	uv tool run ruff check tools
fmt:
	cargo fmt --all
	cd apps/gui && ./node_modules/.bin/prettier --write --log-level warn \
	  "src/**/*.{svelte,ts,js,css}" "*.html" "*.ts" "*.json" ../../*.md ../../docs/*.md
	uv tool run ruff format tools
bindings:   ; CARGO_TARGET_DIR=$(CHECK_TARGET) cargo test -p dayz-community-hub export_bindings
windows:
	cd apps/gui && bunx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --no-bundle
	mkdir -p var/dist
	cd target/x86_64-pc-windows-msvc/release && \
	  rm -f $(CURDIR)/var/dist/dayz-community-hub-x86_64-windows.zip && \
	  zip -9 $(CURDIR)/var/dist/dayz-community-hub-x86_64-windows.zip dayz-community-hub.exe
build:      ; cd apps/gui && $(LINUX_BUILD)
appimage:   ; scripts/appimage.sh
RELEASE_FLAGS = $(if $(VERSION),--version $(VERSION),) $(if $(DRY),--dry-run,) $(if $(SKIP_GITHUB),--skip-github,)
publish:    ; $(DZCH) release $(RELEASE_FLAGS)
prerelease: ; $(DZCH) release --pre $(RELEASE_FLAGS)
aur:        ; $(DZCH) aur $(if $(DRY),--dry-run,) $(if $(FORCE),--force,)
aur-build:  ; $(DZCH) aur --build
keys:       ; $(DZCH) keys $(if $(DRY),--dry-run,)
