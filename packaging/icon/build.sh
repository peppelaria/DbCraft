#!/bin/sh
# Regenerates the app icons from dbcraft.svg (macOS: needs inkscape, ImageMagick and iconutil).
#   packaging/macos/DbCraft.icns            the .app bundle
#   packaging/windows/dbcraft.ico           embedded in dbcraft.exe (crates/app/build.rs)
#   packaging/linux/icons/hicolor/*/apps/   the .deb and the .tar.gz
#   crates/app/assets/icon.png              the window icon (256 px)
set -e
cd "$(dirname "$0")/../.."
SVG=packaging/icon/dbcraft.svg
TMP=$(mktemp -d)
for s in 16 32 48 64 128 256 512 1024; do
  inkscape "$SVG" -w $s -h $s -o "$TMP/$s.png" >/dev/null 2>&1
done
# macOS
mkdir -p "$TMP/DbCraft.iconset"
for s in 16 32 128 256 512; do
  cp "$TMP/$s.png" "$TMP/DbCraft.iconset/icon_${s}x${s}.png"
  cp "$TMP/$((s * 2)).png" "$TMP/DbCraft.iconset/icon_${s}x${s}@2x.png"
done
iconutil -c icns "$TMP/DbCraft.iconset" -o packaging/macos/DbCraft.icns
# Windows
magick "$TMP/16.png" "$TMP/32.png" "$TMP/48.png" "$TMP/64.png" "$TMP/128.png" "$TMP/256.png" packaging/windows/dbcraft.ico
# Linux
for s in 16 32 48 64 128 256 512; do
  mkdir -p "packaging/linux/icons/hicolor/${s}x${s}/apps"
  cp "$TMP/$s.png" "packaging/linux/icons/hicolor/${s}x${s}/apps/dbcraft.png"
done
mkdir -p packaging/linux/icons/hicolor/scalable/apps
cp "$SVG" packaging/linux/icons/hicolor/scalable/apps/dbcraft.svg
# the window
cp "$TMP/256.png" crates/app/assets/icon.png
rm -rf "$TMP"
echo "icons regenerated"
