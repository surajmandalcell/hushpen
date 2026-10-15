#!/bin/bash
# usage: update-fixtures.sh <rc.1 dist dir> <rc.2 dist dir> <out dir>
# Writes what the self-update suite reads from /update-test (set HUSHPEN_UPDATE_TEST to <out dir>
# before harness/run-suite.sh):
#   rc1/    the rc.1 installers (AppImage, deb, rpm, tar.gz), copied from the first dist dir
#   rc2/    the rc.2 AppImage, copied from the second dist dir; the feed server hands it out
#   feed/good/    a signed manifest that offers rc.2 with that AppImage and its real sha256
#   feed/badsha/  the same manifest with a sha256 the file does not have
#   test.pub      the public key of the signature
# Both dist dirs hold builds from `xtask package --test-feed`. The secret key is made fresh in
# <out dir>.keys, outside the mounted folder, and is not kept between runs.
set -euo pipefail
rc1=${1:?usage: update-fixtures.sh <rc.1 dist dir> <rc.2 dist dir> <out dir>}
rc2=${2:?usage: update-fixtures.sh <rc.1 dist dir> <rc.2 dist dir> <out dir>}
out=${3:?usage: update-fixtures.sh <rc.1 dist dir> <rc.2 dist dir> <out dir>}
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
keys=$out.keys

rm -rf "$out" "$keys"
mkdir -p "$out/rc1" "$out/rc2" "$out/feed" "$keys"
cp "$rc1"/Hushpen-*-linux-*.{AppImage,deb,rpm,tar.gz} "$out/rc1/"
appimage=$(ls "$rc2"/Hushpen-*-linux-*.AppImage)
[ -f "$appimage" ] || { echo "no rc.2 AppImage in $rc2" >&2; exit 1; }
cp "$appimage" "$out/rc2/"
file=$out/rc2/$(basename "$appimage")
target=$(basename "$file" | sed -E 's/^Hushpen-.*-(linux-(x64|arm64))\.AppImage$/\1/')
version=$(basename "$file" | sed -E 's/^Hushpen-(.*)-linux-(x64|arm64)\.AppImage$/\1/')

xtask() { (cd "$repo" && cargo run -q -p xtask --locked -- "$@"); }
xtask update-test-keygen --out "$keys" --name test
cp "$keys/test.pub" "$out/test.pub"
notes=https://github.com/surajmandalcell/asrpro/releases/tag/v$version
for kind in good badsha; do
  xtask update-feed --out "$out/feed/$kind" --key "$keys/test.key" --version "$version" \
    --notes-url "$notes" --artifact "$target=$file" --base-url http://127.0.0.1:8765 \
    $([ "$kind" = badsha ] && echo --wrong-sha)
done
echo "fixtures in $out: $(ls "$out/rc1" | wc -l | tr -d ' ') rc.1 installers, $(basename "$file"), feeds $(ls "$out/feed" | tr '\n' ' ')"
