#!/bin/bash
# Runs the AppImage, tar.gz, and release deb checks of a dist folder in one harness slot.
# The slot starts with no app (--no-app), so nothing but the packages under test is installed.
#
# usage: slot-checks.sh <dist dir> <slot>      (after sourcing env.sh; run from the repo root)
# Output: $HUSHPEN_ROOT/slots/<slot>/out/pkg-<target>/ (logs, xwininfo, screenshots, xterm.txt)
set -eu
set -o pipefail

dist=$1
slot=$2
repo=$(cd "$(dirname "$0")/../.." && pwd)
. "$repo/harness/lib.sh"
need_slot "$slot"

target=$(basename "$dist")
export HUSHPEN_RUN_ID=pkg-$target
run_dir=$(slot_run_dir "$slot")
mkdir -p "$run_dir"

"$repo/harness/start.sh" --no-app "$slot"
trap '"$repo/harness/stop.sh" "$slot"' EXIT

inputs=$(slot_dir "$slot")/out/inputs
mkdir -p "$inputs"
cp "$dist"/Hushpen-*.AppImage "$dist"/Hushpen-*.tar.gz "$dist"/Hushpen-*.deb "$inputs/"
cp "$repo"/packaging/smoke/appimage-xvfb.sh "$repo"/packaging/smoke/release-deb-in-slot.sh "$inputs/"
version=$(basename "$(ls "$inputs"/Hushpen-*.deb)" | sed -E 's/^Hushpen-(.*)-linux-(x64|arm64)\.deb$/\1/')

echo "==> AppImage maps 940x600 and stays up (xvfb-run inside the slot)"
slot_exec "$slot" sh -c "chmod +x /out/inputs/*.AppImage /out/inputs/*.sh &&
  /out/inputs/appimage-xvfb.sh /out/inputs/Hushpen-$version-$target.AppImage /out/pkg-$target/appimage.png" |
  tee "$run_dir/appimage.log"

echo "==> tar.gz layout and --version"
slot_exec "$slot" sh -ec "
  rm -rf /tmp/tar && mkdir /tmp/tar && cd /tmp/tar
  tar -xzf /out/inputs/Hushpen-$version-$target.tar.gz
  tops=\$(ls)
  [ \"\$(echo \"\$tops\" | wc -l)\" = 1 ] || { echo \"more than one top entry: \$tops\"; exit 1; }
  cd \"\$tops\"
  ls -R | head -40
  for f in hushpen hushpen-llm THIRD-PARTY-NOTICES.html icons install-kind; do
    [ -e \"\$f\" ] || { echo \"missing \$f\"; exit 1; }
  done
  [ \"\$(cat install-kind)\" = tar ]
  echo \"install-kind: \$(cat install-kind)\"
  got=\$(./hushpen --version)
  echo \"./hushpen --version: \$got\"
  [ \"\$got\" = 'hushpen $version' ]
" | tee "$run_dir/tar.log"

echo "==> release deb: no hook, push-to-talk into xterm"
slot_exec "$slot" bash "/out/inputs/release-deb-in-slot.sh" "/out/inputs/Hushpen-$version-$target.deb" |
  tee "$run_dir/release-deb.log"
