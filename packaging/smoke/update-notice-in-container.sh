#!/bin/bash
# The rpm and deb halves of VAL-AUP-005: installs the rc.1 deb or rpm of a test-feed build in a
# stock distro container, offers a newer version from a local signed feed, and checks that the
# app shows the notice only (no artifact request, the installed binary unchanged).
#
# usage: update-notice-in-container.sh <update-test dir> <image> <out dir>
#   <update-test dir> is what harness/update-fixtures.sh wrote. image ubuntu:22.04 or debian:12
#   installs the deb; fedora:40 installs the rpm. Results and screenshots land in <out dir>/<label>.
set -eu
ut=$1
image=$2
out=$3
repo=$(cd "$(dirname "$0")/../.." && pwd)

case $image in
  fedora*) kind=rpm ;;
  *) kind=deb ;;
esac
file=$(ls "$ut"/rc1/Hushpen-*."$kind" | head -n 1)
case $file in
  *-linux-x64.*) platform=linux/amd64 ;;
  *-linux-arm64.*) platform=linux/arm64 ;;
  *) echo "cannot read the target from $file" >&2; exit 1 ;;
esac
label=$kind-$(echo "$image" | tr ':' '-')
mkdir -p "$out"
echo "==> $image installs $(basename "$file") and gets a newer version offered"
docker rm -f hushpen-notice >/dev/null 2>&1 || true
docker run --rm --name hushpen-notice --platform "$platform" --cpus 2 --memory 3g \
  -e KIND="$kind" -e LABEL="$label" \
  -v "$repo/harness:/harness:ro" -v "$ut:/update-test:ro" -v "$out:/out" \
  "$image" bash /harness/slot/notice-container.sh
