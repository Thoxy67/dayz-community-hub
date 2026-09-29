#!/usr/bin/env bash
# Build the Linux app against an old glibc, through zig, when zig is here.
#
# Used as `tauri build --runner` (tauri calls it as `<runner> build <args>`).
# A binary linked on this machine (Arch) asks for the newest glibc symbol
# versions the host has, so it refuses to start on any older distribution.
# `cargo zigbuild` links against zig's own glibc stubs at a chosen version
# instead, which is the floor the binary then asks for.
#
# The floor is 2.35, Ubuntu 22.04: the oldest mainstream release that ships
# webkit2gtk-4.1, which Tauri 2 needs anyway, so going lower buys nothing.
#
# cargo-zigbuild takes the floor only as a suffix on the target triple
# (`x86_64-unknown-linux-gnu.2.35`), and tauri refuses that triple: it checks
# `--target` against `rustup target list`. So tauri is given the plain
# triple and this script adds the suffix. cargo-zigbuild hands cargo the
# plain triple again, so the output lands in target/x86_64-unknown-linux-gnu/,
# the directory tauri expects for that `--target`.
#
# Without zig or cargo-zigbuild, or with ZIG=0, it runs plain cargo with the
# same arguments: same output directory, but a binary tied to this machine's
# glibc.
set -euo pipefail

GLIBC_FLOOR=2.35
TRIPLE=x86_64-unknown-linux-gnu

if [[ "${ZIG:-1}" == 0 ]]; then
  exec cargo "$@"
fi
if ! command -v zig >/dev/null || ! command -v cargo-zigbuild >/dev/null; then
  echo "cargo-zigbuild.sh: zig or cargo-zigbuild not found, building with plain cargo (the binary needs this machine's glibc)" >&2
  exec cargo "$@"
fi
# Only a build links anything; whatever else tauri may ask of its runner goes
# to cargo untouched.
if [[ "${1:-}" != build ]]; then
  exec cargo "$@"
fi
shift

args=()
while (($#)); do
  case "$1" in
    --target)
      args+=(--target)
      shift
      if [[ "${1:-}" == "$TRIPLE" ]]; then args+=("$TRIPLE.$GLIBC_FLOOR"); else args+=("${1:-}"); fi
      ;;
    --target="$TRIPLE") args+=("--target=$TRIPLE.$GLIBC_FLOOR") ;;
    *) args+=("$1") ;;
  esac
  shift
done
exec cargo zigbuild "${args[@]}"
