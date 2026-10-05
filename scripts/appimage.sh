#!/usr/bin/env bash
# Build the release AppImage inside Ubuntu 22.04 (packaging/appimage/
# Containerfile), so it runs on any distribution with glibc 2.35 or newer.
#
# The repository is mounted at /src; cargo's output goes to target/appimage/
# (bundles in target/appimage/release/bundle/appimage/) so it never shares
# artifacts with builds made on the host. The toolchain, the cargo registry
# and linuxdeploy's downloads live in podman volumes and survive between runs.
#
# Extra arguments go to `tauri build`. TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)
# are passed through when set, so `createUpdaterArtifacts` can sign; without
# them the AppImage is built unsigned.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
image=dzch-appimage
engine=${CONTAINER_ENGINE:-podman}

if ! command -v "$engine" >/dev/null; then
  echo "appimage.sh: $engine not found; the AppImage is built in a container (docs/build.md)" >&2
  exit 1
fi

# Rebuilt when the Containerfile changes (its hash is the tag).
tag=$(sha256sum "$repo/packaging/appimage/Containerfile" | cut -c1-12)
if ! "$engine" image exists "$image:$tag" 2>/dev/null; then
  "$engine" build -t "$image:$tag" -f "$repo/packaging/appimage/Containerfile" "$repo/packaging/appimage"
fi

env_args=(-e NO_STRIP=true -e CARGO_TARGET_DIR=/src/target/appimage -e APPIMAGE_EXTRACT_AND_RUN=1)
tauri_args=()
if [[ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  env_args+=(-e TAURI_SIGNING_PRIVATE_KEY -e TAURI_SIGNING_PRIVATE_KEY_PASSWORD)
else
  # No key (`make appimage`): an unsigned AppImage rather than a failed build.
  tauri_args+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi

# --userns=keep-id: files written into the repository stay the user's.
exec "$engine" run --rm --userns=keep-id \
  -v "$repo:/src" -w /src/apps/gui \
  -v dzch-appimage-rustup:/opt/rustup:U \
  -v dzch-appimage-cargo:/opt/cargo:U \
  -v dzch-appimage-cache:/home/builder/.cache:U \
  -e HOME=/home/builder \
  "${env_args[@]}" \
  "$image:$tag" \
  bun run tauri build --bundles appimage "${tauri_args[@]}" "$@"
