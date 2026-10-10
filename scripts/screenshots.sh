#!/usr/bin/env bash
# Screenshot every view of the interface without Tauri: `make screenshots`.
#
# Builds the interface on its pretend backend (VITE_MOCK=1, lib/ipc/mock.ts)
# into apps/gui/dist-mock-shots, serves it with `vite preview`, and has a
# headless Chromium (brave, chromium or google-chrome; BROWSER=… to choose)
# photograph each view into var/screenshots/<view>.png.
#
#   VIEWS="servers mods"   only these views (default: every one in src/shell/nav.ts)
#   QUERY="theme=…"        extra URL flags (see CLAUDE.md: theme, focus, rail, wizard)
#   SIZE=1400,860          the window size
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
gui=$repo/apps/gui
out=$repo/var/screenshots
port=${PORT:-4179}
size=${SIZE:-1400,860}

browser=${BROWSER:-}
if [[ -z "$browser" ]]; then
  for b in brave chromium google-chrome google-chrome-stable; do
    if command -v "$b" >/dev/null; then browser=$b; break; fi
  done
fi
if [[ -z "$browser" ]]; then
  echo "screenshots.sh: no headless Chromium found (brave, chromium, google-chrome); set BROWSER=" >&2
  exit 1
fi

views=${VIEWS:-$(grep -oP '^\s*id: "\K[a-z]+' "$gui/src/shell/nav.ts" | tr '\n' ' ')}

cd "$gui"
VITE_MOCK=1 bunx vite build --outDir dist-mock-shots --logLevel warn
bunx vite preview --outDir dist-mock-shots --port "$port" --strictPort >/dev/null &
preview=$!
trap 'kill "$preview" 2>/dev/null || true' EXIT

for _ in $(seq 50); do
  curl -sf "http://localhost:$port/" >/dev/null && break
  sleep 0.2
done

mkdir -p "$out"
for view in $views; do
  "$browser" --headless=new --hide-scrollbars --screenshot="$out/$view.png" \
    --window-size="$size" --virtual-time-budget=6000 \
    "http://localhost:$port/?view=$view${QUERY:+&$QUERY}" 2>/dev/null
  echo "$out/$view.png"
done
