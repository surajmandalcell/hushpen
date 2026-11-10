#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
export HUSHPEN_MOBILE_CORE=${HUSHPEN_MOBILE_CORE:-${CARGO_TARGET_DIR:-/Volumes/External1TB/data/_custom/droid/hushpen/target/macos-arm64}/mobile}
[[ -f $HUSHPEN_MOBILE_CORE/bindings/swift/HushpenCore.swift ]] || {
  echo "Build the iOS core first: bash mobile/build-core.sh ios" >&2
  exit 1
}
[[ -d $HUSHPEN_MOBILE_CORE/ios/HushpenCore.xcframework ]] || {
  echo "The iOS core XCFramework is missing." >&2
  exit 1
}
xcodegen generate --spec project.yml
