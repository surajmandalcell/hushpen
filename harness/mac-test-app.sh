#!/bin/sh
# usage: harness/mac-test-app.sh [--live]
# Rebuilds the Mac test app ($HUSHPEN_ROOT/mac-check/Hushpen.app: debug, test-automation hook) from
# this checkout and signs it with the owner's Apple Development identity. macOS keys its privacy
# grants (Accessibility, Input Monitoring, Microphone) to the signature's designated requirement: an
# ad-hoc signature pins them to one build hash, a certificate keeps them across rebuilds.
# Quit the app before the copy step. HUSHPEN_SIGN_IDENTITY overrides the identity name.
# --live uses Hushpen's file-backed Developer ID signer and a fixed, WAV-only test bundle.
set -eu
ROOT=${HUSHPEN_ROOT:-/Volumes/External1TB/data/_custom/droid/hushpen}
REPO=$(cd "$(dirname "$0")/.." && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target/macos-arm64}
die() { echo "mac-test-app: $*" >&2; exit 1; }
case "${1:-}" in
  --live)
    LIVE=/Volumes/External1TB/data/_custom/orca/workspaces/hushpen/livetest
    NAME='Hushpen Live Test'
    BUNDLE=app.hushpen.livetest
    APP=$LIVE/$NAME.app
    PRIVACY_PLIST="<key>LSEnvironment</key><dict><key>HUSHPEN_DATA_DIR</key><string>$LIVE/data</string></dict>"
    SIGNING=$ROOT/signing
    RCODESIGN=$SIGNING/apple-codesign-0.29.0-aarch64-apple-darwin/rcodesign
    [ -x "$RCODESIGN" ] && [ -r "$SIGNING/devid.key" ] && [ -r "$SIGNING/devid.pem" ] ||
      die "Developer ID signing files are missing under $SIGNING"
    ;;
  '')
    NAME=Hushpen
    BUNDLE=app.hushpen.test
    APP=$ROOT/mac-check/Hushpen.app
    PRIVACY_PLIST='<key>NSMicrophoneUsageDescription</key><string>Hushpen listens to the microphone while you dictate.</string>'
    IDENTITY=${HUSHPEN_SIGN_IDENTITY:-$(security find-identity -v -p codesigning | awk '/"Apple Development/ {print $2; exit}')}
    [ -n "$IDENTITY" ] || die "no Apple Development identity in the keychain"
    ;;
  *) die 'usage: harness/mac-test-app.sh [--live]' ;;
esac

cd "$REPO"
cargo build --locked -j "${JOBS:-4}" -p hushpen-app --features test-automation -p hushpen-testhook -p hushpen-engine -p hushpen-llm -p hushpen-tts
cargo xtask tts-runtime --target macos-arm64
ps -axo command | grep -q "^$APP/Contents/MacOS/hushpen" && die "built; Hushpen is running, quit it and run again"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"
# The voice worker loads the voice runtime from Contents/Frameworks, as in the shipped bundle.
cp "$CARGO_TARGET_DIR"/tts-runtime/macos-arm64/*.dylib "$APP/Contents/Frameworks/"
cp "$CARGO_TARGET_DIR/debug/hushpen" "$CARGO_TARGET_DIR/debug/hushpen-asr" "$CARGO_TARGET_DIR/debug/hushpen-llm" "$CARGO_TARGET_DIR/debug/hushpen-tts" "$APP/Contents/MacOS/"
cp assets/hushpen-app-icon.icns "$APP/Contents/Resources/Hushpen.icns"
cp assets/hushpen-app-icon.car "$APP/Contents/Resources/Assets.car"
cp assets/ggml-silero-v5.1.2.bin "$APP/Contents/Resources/"
cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleExecutable</key><string>hushpen</string>
  <key>CFBundleIconFile</key><string>Hushpen</string>
  <key>CFBundleIconName</key><string>Hushpen</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE</string>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>2.5.0</string>
  <key>CFBundleVersion</key><string>$(git rev-parse --short HEAD)</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  $PRIVACY_PLIST
</dict>
</plist>
PLIST
if [ "$BUNDLE" = app.hushpen.livetest ]; then
  "$RCODESIGN" sign --timestamp-url none --pem-file "$SIGNING/devid.key" --pem-file "$SIGNING/devid.pem" "$APP"
else
  for lib in "$APP"/Contents/Frameworks/*.dylib; do codesign --force --timestamp=none --sign "$IDENTITY" "$lib"; done
  for worker in hushpen-asr hushpen-llm hushpen-tts; do
    codesign --force --timestamp=none --sign "$IDENTITY" "$APP/Contents/MacOS/$worker"
  done
  codesign --force --timestamp=none --sign "$IDENTITY" "$APP"
fi
codesign --verify --deep --strict "$APP"
codesign -d -r- "$APP" 2>&1 | grep designated
echo "built $APP at $(git rev-parse --short HEAD)"
