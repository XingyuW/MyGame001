#!/bin/sh
# Builds the macOS release package of MyGame001 and checks it with the smoke test (PLAN.md section 7,
# M7: "release builds"; docs/DECISIONS.md D54).
#
# Run from anywhere: `sh scripts/package_macos.sh [frames]` (frames of the smoke test, default 600).
# Output: target/package/MyGame001.app and target/package/MyGame001-macos.zip.
#
# The executable finds its assets next to itself: Bevy reads the asset directory from
# BEVY_ASSET_ROOT, then CARGO_MANIFEST_DIR, then the executable's directory (docs/DECISIONS.md D1),
# so the package puts assets/ beside the executable in Contents/MacOS. The smoke test runs the
# packaged executable as a player would start it: with neither variable set, from another working
# directory. The package ships assets/LICENSES.md and assets/licenses/ (the licence of the font the
# game embeds) with the assets. Exit code 0 only if the build, the package, and the smoke test
# succeed.
set -eu

frames="${1:-600}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

cargo build --release -p game

out="$root/target/package"
app="$out/MyGame001.app"
rm -rf "$app" "$out/MyGame001-macos.zip"
mkdir -p "$app/Contents/MacOS"
cp "$root/target/release/game" "$app/Contents/MacOS/MyGame001"
cp -R "$root/assets" "$app/Contents/MacOS/assets"
cp "$root/README.md" "$app/Contents/MacOS/README.md"
version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -n 1)"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>MyGame001</string>
  <key>CFBundleDisplayName</key>
  <string>MyGame001</string>
  <key>CFBundleIdentifier</key>
  <string>local.mygame001.game</string>
  <key>CFBundleExecutable</key>
  <string>MyGame001</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

# The smoke test of the packaged executable, started outside the repository and without the
# variables cargo sets, so that it can only find the assets beside itself.
elsewhere="$(mktemp -d)"
(cd "$elsewhere" && env -u BEVY_ASSET_ROOT -u CARGO_MANIFEST_DIR \
    "$app/Contents/MacOS/MyGame001" --smoke-test "$frames")
rmdir "$elsewhere"

ditto -c -k --keepParent "$app" "$out/MyGame001-macos.zip"
echo "package: $app"
echo "archive: $out/MyGame001-macos.zip"
