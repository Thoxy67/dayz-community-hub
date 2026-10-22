#!/usr/bin/env bash
# Build the release AppImage inside Ubuntu 22.04 (packaging/appimage/
# Containerfile), so it runs on any distribution with glibc 2.35 or newer.
#
#   scripts/appimage.sh [output.AppImage]   (default var/dist/dayz-community-hub-x86_64.AppImage)
#
# The output must be inside the repository, which is mounted at /src. Cargo's
# output goes to target/appimage/ so it never shares artifacts with builds
# made on the host. The toolchain and the cargo registry live in podman
# volumes and survive between runs. DZCH_VERSION, when set, is passed on (the
# version a preview reports).
#
# Nothing is signed in here: the signing key never enters the container.
# `dzch release` signs the AppImage afterwards with `cargo xtask sign`.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
image=dzch-appimage
engine=${CONTAINER_ENGINE:-podman}
out=${1:-var/dist/dayz-community-hub-x86_64.AppImage}

if ! command -v "$engine" >/dev/null; then
  echo "appimage.sh: $engine not found; the AppImage is built in a container (docs/build.md)" >&2
  exit 1
fi
# An absolute path must be under the repository; the container sees it relative to /src.
case "$out" in
  "$repo"/*) out=${out#"$repo"/} ;;
  /*) echo "appimage.sh: $out is outside the repository ($repo)" >&2; exit 1 ;;
esac

# Rebuilt when the Containerfile changes (its hash is the tag).
tag=$(sha256sum "$repo/packaging/appimage/Containerfile" | cut -c1-12)
if ! "$engine" image exists "$image:$tag" 2>/dev/null; then
  "$engine" build -t "$image:$tag" -f "$repo/packaging/appimage/Containerfile" "$repo/packaging/appimage"
fi

env_args=(-e CARGO_TARGET_DIR=/src/target/appimage -e APPIMAGE_EXTRACT_AND_RUN=1)
if [[ -n "${DZCH_VERSION:-}" ]]; then
  env_args+=(-e DZCH_VERSION)
fi

# --userns=keep-id: files written into the repository stay the user's.
exec "$engine" run --rm --userns=keep-id \
  -v "$repo:/src" -w /src \
  -v dzch-appimage-rustup:/opt/rustup:U \
  -v dzch-appimage-cargo:/opt/cargo:U \
  -v dzch-appimage-cache:/home/builder/.cache:U \
  -e HOME=/home/builder \
  "${env_args[@]}" \
  "$image:$tag" \
  packaging/appimage/build.sh "$out"
