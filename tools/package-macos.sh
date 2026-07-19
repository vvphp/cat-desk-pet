#!/usr/bin/env bash
# Build an unsigned .app (+ .dmg) from the release binary.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/dist/macos"
# Finder / Dock display name (Chinese product name).
APP_NAME="摸鱼猫"
BIN_NAME="cat-desk-pet"
APP="$OUT/$APP_NAME.app"
BIN="$ROOT/target/release/$BIN_NAME"

if [[ "${SKIP_BUILD:-}" == "1" ]]; then
  echo "==> skip cargo build (SKIP_BUILD=1)"
else
  echo "==> cargo build --release"
  cargo build --release --manifest-path "$ROOT/Cargo.toml"
fi
if [[ ! -x "$BIN" ]]; then
  echo "missing binary: $BIN" >&2
  exit 1
fi

rm -rf "$APP"
# Also drop the old English-named bundle if present.
rm -rf "$OUT/CatDeskPet.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BIN" "$APP/Contents/MacOS/$BIN_NAME"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

ICON_KEY=""
if [[ -f "$ROOT/assets/icon/AppIcon.icns" ]]; then
  cp "$ROOT/assets/icon/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
  ICON_KEY=$'  <key>CFBundleIconFile</key>\n  <string>AppIcon</string>\n'
fi

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>zh-Hans</string>
  <key>CFBundleExecutable</key>
  <string>${BIN_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>com.catdeskpet.app</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>${APP_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${APP_NAME}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>1.0.2</string>
  <key>CFBundleVersion</key>
  <string>1.0.2</string>
  <key>LSMinimumSystemVersion</key>
  <string>11.0</string>
  <key>LSUIElement</key>
  <true/>
${ICON_KEY}</dict>
</plist>
EOF

echo "==> app: $APP"

if command -v hdiutil >/dev/null 2>&1; then
  DMG="$OUT/$APP_NAME.dmg"
  rm -f "$DMG" "$OUT/CatDeskPet.dmg"
  hdiutil create -volname "$APP_NAME" -srcfolder "$APP" -ov -format UDZO "$DMG" >/dev/null
  echo "==> dmg: $DMG"
fi

echo "done."
