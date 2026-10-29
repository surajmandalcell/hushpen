#!/bin/sh
# usage: harness/mac-icon.sh
# Compiles assets/hushpen-app-icon.icon (the Icon Composer source) into assets/hushpen-app-icon.car,
# which the Mac bundle ships as Contents/Resources/Assets.car with CFBundleIconName Hushpen. macOS 26
# and later draw that icon at full size; without it they shrink the .icns into a gray tile. Needs
# Xcode 26 or later, so the compiled file is checked in.
set -eu
REPO=$(cd "$(dirname "$0")/.." && pwd)
OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT
xcrun actool "$REPO/assets/hushpen-app-icon.icon" --compile "$OUT" \
  --output-format human-readable-text --errors --warnings \
  --output-partial-info-plist "$OUT/partial.plist" --app-icon Hushpen --include-all-app-icons \
  --enable-on-demand-resources NO --development-region en --target-device mac \
  --minimum-deployment-target 13.0 --platform macosx >/dev/null
cp "$OUT/Assets.car" "$REPO/assets/hushpen-app-icon.car"
echo "wrote assets/hushpen-app-icon.car"
