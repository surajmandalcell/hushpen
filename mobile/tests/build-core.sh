#!/usr/bin/env bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
scratch=$(mktemp -d "${TMPDIR:?Set TMPDIR to the shared scratch folder.}/hushpen-mobile-build.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/tools" "$scratch/Android SDK/ndk/28.2.13676358/build/cmake"
touch "$scratch/Android SDK/ndk/28.2.13676358/build/cmake/android.toolchain.cmake"
for tool in cargo cargo-ndk xcrun xcodebuild uname rustup; do
  ln -s "$repo/mobile/tests/build-core-tool.sh" "$scratch/tools/$tool"
done
export BUILD_CORE_TEST_LOG=$scratch/commands
export CARGO_TARGET_DIR=$scratch/cargo\ target
export ANDROID_HOME=$scratch/Android\ SDK
export PATH=$scratch/tools:$PATH
"$repo/mobile/build-core.sh" all
commands=$(cat "$BUILD_CORE_TEST_LOG")
[[ $commands = *'cargo ndk -t arm64-v8a -t x86_64 -P 29 --link-libcxx-shared'* ]]
[[ $commands = *'--target aarch64-apple-ios'* ]]
[[ $commands = *'--target aarch64-apple-ios-sim'* ]]
[[ $commands = *'xcodebuild -create-xcframework'* ]]
[[ -f $CARGO_TARGET_DIR/mobile/ios/headers/hushpen_mobileFFI.h ]]
[[ -f $CARGO_TARGET_DIR/mobile/ios/headers/module.modulemap ]]
[[ -f $CARGO_TARGET_DIR/mobile/bindings/kotlin/com/mandalsuraj/hushpen/core/hushpen_mobile.kt ]]
[[ -f $CARGO_TARGET_DIR/mobile/bindings/swift/HushpenCore.swift ]]
if "$repo/mobile/build-core.sh" unsupported 2>/dev/null; then
  echo "An unsupported build mode must fail." >&2; exit 1
fi
if BUILD_CORE_TEST_NDK_VERSION='cargo-ndk 4.0.0' "$repo/mobile/build-core.sh" android 2>/dev/null; then
  echo "A different cargo-ndk version must fail." >&2; exit 1
fi
CARGO_TARGET_DIR='' HUSHPEN_ROOT="$scratch/canonical" AGENT_DATA_DIR="$scratch/other" "$repo/mobile/build-core.sh" bindings
[[ -f $scratch/canonical/target/macos-arm64/mobile/bindings/swift/HushpenCore.swift ]]
[[ ! -e $scratch/other ]]
echo "Mobile build command checks passed."
