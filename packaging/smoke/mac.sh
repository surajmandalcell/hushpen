#!/bin/sh
# CI macOS runner only: unpacks the zip, mounts the dmg, and checks both apps.
# The dmg layout step and the mount both touch the desktop, so this never runs
# on a developer Mac.
#
# usage: mac.sh <dist dir>
set -eu

if [ "${CI:-}" != "true" ] || [ "${GITHUB_ACTIONS:-}" != "true" ]; then
  echo "mac.sh runs on the GitHub Actions macOS runner only" >&2
  exit 2
fi

dist=$1
scratch=$(mktemp -d)
trap 'hdiutil detach "$scratch/mnt" >/dev/null 2>&1 || true; rm -rf "$scratch"' EXIT

zip=$(find "$dist" -maxdepth 1 -name 'Hushpen-*-macos-arm64.zip' | head -n 1)
dmg=$(find "$dist" -maxdepth 1 -name 'Hushpen-*-macos-arm64.dmg' | head -n 1)
version=$(basename "$zip" | sed -E 's/^Hushpen-(.*)-macos-arm64\.zip$/\1/')

check_app() {
  app=$1
  ls -l "$app/Contents/MacOS"
  plutil -p "$app/Contents/Info.plist"
  codesign --verify --deep --strict "$app"
  codesign -dv "$app" 2>&1 | tee "$scratch/sig.txt"
  grep -q 'Signature=adhoc' "$scratch/sig.txt"
  got=$("$app/Contents/MacOS/hushpen" --version)
  echo "hushpen --version: $got"
  [ "$got" = "hushpen $version" ]
  [ -f "$app/Contents/MacOS/hushpen-llm" ]
}

echo "==> zip"
ditto -x -k "$zip" "$scratch/zip"
check_app "$scratch/zip/Hushpen.app"

echo "==> dmg"
hdiutil imageinfo "$dmg"
mkdir "$scratch/mnt"
hdiutil attach -nobrowse -readonly -mountpoint "$scratch/mnt" "$dmg"
ls -la "$scratch/mnt"
[ -d "$scratch/mnt/Hushpen.app" ]
[ -e "$scratch/mnt/Applications" ]
check_app "$scratch/mnt/Hushpen.app"
echo "mac smoke ok"
