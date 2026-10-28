#!/bin/sh
# usage: harness/mac-test-app.sh
# Rebuilds the Mac test app ($HUSHPEN_ROOT/mac-check/Hushpen.app: debug, test-automation hook) from
# this checkout and signs it with the owner's Apple Development identity. macOS keys its privacy
# grants (Accessibility, Input Monitoring, Microphone) to the signature's designated requirement: an
# ad-hoc signature pins them to one build hash, a certificate keeps them across rebuilds.
# Quit the app before the copy step. HUSHPEN_SIGN_IDENTITY overrides the identity name.
set -eu
ROOT=${HUSHPEN_ROOT:-/Volumes/External1TB/data/_custom/droid/hushpen}
REPO=$(cd "$(dirname "$0")/.." && pwd)
APP=$ROOT/mac-check/Hushpen.app
# A keychain with more than one Apple Development certificate makes the bare name ambiguous, so
# the default is the SHA-1 of the first one.
IDENTITY=${HUSHPEN_SIGN_IDENTITY:-$(security find-identity -v -p codesigning | awk '/"Apple Development/ {print $2; exit}')}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target/macos-arm64}
die() { echo "mac-test-app: $*" >&2; exit 1; }
[ -n "$IDENTITY" ] || die "no Apple Development identity in the keychain"

cd "$REPO"
cargo build --locked -j "${JOBS:-4}" -p hushpen-app --features test-automation -p hushpen-testhook -p hushpen-llm -p hushpen-tts
ps -axo command | grep -q "^$APP/Contents/MacOS/hushpen" && die "built; Hushpen is running, quit it and run again"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$CARGO_TARGET_DIR/debug/hushpen" "$CARGO_TARGET_DIR/debug/hushpen-llm" "$CARGO_TARGET_DIR/debug/hushpen-tts" "$APP/Contents/MacOS/"
cp assets/hushpen-app-icon.icns "$APP/Contents/Resources/Hushpen.icns"
cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key><string>Hushpen</string>
  <key>CFBundleExecutable</key><string>hushpen</string>
  <key>CFBundleIconFile</key><string>Hushpen</string>
  <key>CFBundleIdentifier</key><string>app.hushpen.test</string>
  <key>CFBundleName</key><string>Hushpen</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>2.1.0</string>
  <key>CFBundleVersion</key><string>$(git rev-parse --short HEAD)</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Hushpen listens to the microphone while you dictate.</string>
</dict>
</plist>
PLIST
# The helpers first: a bundle signature covers its nested code but does not sign it.
codesign --force --timestamp=none --sign "$IDENTITY" "$APP/Contents/MacOS/hushpen-llm"
codesign --force --timestamp=none --sign "$IDENTITY" "$APP/Contents/MacOS/hushpen-tts"
codesign --force --timestamp=none --sign "$IDENTITY" "$APP"
codesign --verify --deep --strict "$APP"
codesign -d -r- "$APP" 2>&1 | grep designated
echo "built $APP at $(git rev-parse --short HEAD)"
