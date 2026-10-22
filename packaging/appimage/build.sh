#!/usr/bin/env bash
# Inside the Ubuntu 22.04 container (scripts/appimage.sh runs it, with the
# repository at /src): build the release executable and make the AppImage.
#
#   packaging/appimage/build.sh <output .AppImage, relative to /src>
#
# The AppDir is what the Tauri bundle's held for the launcher, minus the
# libraries (see the Containerfile): usr/bin/dayz-community-hub, the .desktop
# (same name, Exec, MIME types, icon), the hicolor icons, the MIME package for
# .dzch files, and AppRun. DZCH_VERSION, when set, is the version the build
# reports (a preview).
set -euo pipefail

out=${1:?usage: build.sh <output .AppImage>}
bin=dayz-community-hub
target=${CARGO_TARGET_DIR:-/src/target/appimage}

# `rustup` installs the toolchain rust-toolchain.toml names on first use.
cargo build --release -p dayz-community-hub --bin "$bin"

appdir=$target/AppDir
rm -rf "$appdir"
install -Dm755 "$target/release/$bin" "$appdir/usr/bin/$bin"
install -Dm644 packaging/linux/$bin.desktop "$appdir/usr/share/applications/$bin.desktop"
install -Dm644 packaging/linux/$bin.xml "$appdir/usr/share/mime/packages/$bin.xml"
for size in 32 64 128 256; do
  install -Dm644 "apps/desktop/assets/app/icon-$size.png" \
    "$appdir/usr/share/icons/hicolor/${size}x${size}/apps/$bin.png"
done
# What the AppImage runtime and desktop integrations look for at the root.
cp "$appdir/usr/share/applications/$bin.desktop" "$appdir/$bin.desktop"
cp apps/desktop/assets/app/icon-256.png "$appdir/$bin.png"
ln -s "$bin.png" "$appdir/.DirIcon"
ln -s "usr/bin/$bin" "$appdir/AppRun"

mkdir -p "$(dirname "$out")"
rm -f "$out"
ARCH=x86_64 appimagetool --no-appstream --runtime-file /opt/appimage/runtime-x86_64 \
  "$appdir" "$out"
echo "appimage: $out"
