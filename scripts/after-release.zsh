#!/bin/zsh
# Last release step. Usage: zsh scripts/after-release.zsh <version>   such as: zsh scripts/after-release.zsh 2.6.0
# Run it after you dispatch release.yml for the tag. It waits (at most 30 minutes) until that workflow run succeeds
# and the GitHub release for the tag exists, then runs samhq_backend's publish-release.zsh in no-upload mode: it
# writes and pushes releases.json and the Homebrew cask, and waits until api.samhq.app serves the version.
# CI never pushes to the private samhq_backend, so this local step does it. A -rc.N tag is not published.
set -euo pipefail
setopt extended_glob
die() { print -u2 "after-release: $*"; exit 1 }
(( # == 1 )) || die "usage: <version>"
version=$1 repo=samhqapp/hushpen tag=v$1
[[ $version == (#b)([0-9]##).([0-9]##).([0-9]##) ]] || { print "after-release: $version is a prerelease or not X.Y.Z; nothing to publish"; exit 0 }
build=$(( match[1] * 1000000 + match[2] * 1000 + match[3] ))  # Hushpen's formula, hushpen-core/src/update.rs
backend=${0:A:h:h:h}/samhq_backend
helper=$backend/scripts/publish-release.zsh
[[ -f $helper ]] || die "$helper is missing"
command -v gh >/dev/null || die "install the GitHub CLI"

deadline=$(( SECONDS + 1800 ))
while :; do
  read -r state conclusion <<<"$(gh run list -R $repo --workflow release.yml --limit 1 --json status,conclusion --jq '.[0] | "\(.status) \(.conclusion)"')"
  [[ $state == completed && $conclusion != success ]] && die "the latest release.yml run ended with $conclusion"
  [[ $state == completed ]] && gh release view $tag -R $repo >/dev/null 2>&1 && break
  (( SECONDS < deadline )) || die "no finished release.yml run and $tag release within 30 minutes (last run: ${state:-none})"
  sleep 20
done

asset=Hushpen-$version-macos-arm64.zip  # the file the Homebrew cask downloads
sha=$(gh release download $tag -R $repo -p $asset -O - | shasum -a 256 | cut -d' ' -f1)
[[ $sha =~ ^[0-9a-f]{64}$ ]] || die "could not hash $asset from $tag"
zsh $helper --no-upload --url https://github.com/$repo/releases/tag/$tag --sha256 $sha hushpen $version $build hushpen
