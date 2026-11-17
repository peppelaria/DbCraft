#!/usr/bin/env bash
# Builds DbCraft.app from one or more `dbcraft` binaries (merged into a universal binary when
# there are several), signs it ad hoc and zips it.
#   scripts/package-macos.sh <version> <out-dir> <binary> [<binary>...]
set -euo pipefail

version="$1"; out="$2"; shift 2
app="$out/DbCraft.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

if [ "$#" -gt 1 ]; then
  lipo -create "$@" -output "$app/Contents/MacOS/dbcraft"
else
  cp "$1" "$app/Contents/MacOS/dbcraft"
fi
chmod +x "$app/Contents/MacOS/dbcraft"

cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>DbCraft</string>
  <key>CFBundleDisplayName</key><string>DbCraft</string>
  <key>CFBundleIdentifier</key><string>io.github.rmpaolillo.dbcraft</string>
  <key>CFBundleExecutable</key><string>dbcraft</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  <key>NSHumanReadableCopyright</key><string>MIT OR Apache-2.0 — The DbCraft contributors</string>
</dict>
</plist>
PLIST

# Ad-hoc signature: Apple silicon refuses to run unsigned code. Not notarised.
codesign --force --deep --sign - "$app"
codesign --verify --deep "$app"

zip_name="DbCraft-${version}-macos-universal.zip"
(cd "$out" && rm -f "$zip_name" && ditto -c -k --keepParent DbCraft.app "$zip_name")
lipo -info "$app/Contents/MacOS/dbcraft"
echo "$out/$zip_name"
