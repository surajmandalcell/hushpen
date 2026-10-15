#!/bin/sh
# Installs the deb or rpm from a dist folder in a stock distro container and
# checks what the install put on disk.
#
# usage: install-in-container.sh <dist dir> <image>
#   image debian:12 or ubuntu:22.04 installs the deb; fedora:40 installs the rpm.
# The container is named hushpen-build and removed when it exits.
set -eu

dist=$1
image=$2

case $image in
  fedora*) kind=rpm ;;
  *) kind=deb ;;
esac

file=$(find "$dist" -maxdepth 1 -name "Hushpen-*.$kind" | head -n 1)
[ -n "$file" ] || { echo "no $kind in $dist" >&2; exit 1; }
name=$(basename "$file")
version=$(echo "$name" | sed -E 's/^Hushpen-(.*)-linux-(x64|arm64)\.(deb|rpm)$/\1/')
case $name in
  *-linux-x64.*) platform=linux/amd64 ;;
  *-linux-arm64.*) platform=linux/arm64 ;;
  *) echo "cannot read the target from $name" >&2; exit 1 ;;
esac

echo "==> $image installs $name (expects hushpen $version)"
docker run --rm --name "${HUSHPEN_PKG_CONTAINER:-hushpen-build}" --platform "$platform" \
  -e PKG="$name" -e KIND="$kind" -e VERSION="$version" \
  -v "$dist:/pkg:ro" "$image" sh -ec '
    cp "/pkg/$PKG" "/tmp/$PKG"
    chmod 644 "/tmp/$PKG"
    if [ "$KIND" = deb ]; then
      apt-get update -qq
      DEBIAN_FRONTEND=noninteractive apt-get install -y "/tmp/$PKG"
      files=$(dpkg -L hushpen)
    else
      dnf install -y "/tmp/$PKG"
      files=$(rpm -ql hushpen)
    fi
    echo "$files"
    for want in /usr/lib/hushpen/hushpen /usr/lib/hushpen/hushpen-llm /usr/bin/hushpen \
      /usr/share/applications/hushpen.desktop /usr/share/icons/hicolor/512x512/apps/hushpen.png \
      /usr/lib/hushpen/THIRD-PARTY-NOTICES.html; do
      echo "$files" | grep -qx "$want" || { echo "package lists no $want" >&2; exit 1; }
    done
    [ "$(cat /usr/lib/hushpen/install-kind)" = "$KIND" ]
    got=$(hushpen --version)
    echo "hushpen --version: $got"
    [ "$got" = "hushpen $VERSION" ]
    [ "$(/usr/lib/hushpen/hushpen-llm --version)" = "hushpen-llm $VERSION" ]
    echo "install ok"
  '
