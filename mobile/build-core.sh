#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
mode=${1:-all}
case "$mode" in
  all|android|ios|bindings|check) ;;
  *) echo "Usage: mobile/build-core.sh [all|android|ios|bindings|check]" >&2; exit 2 ;;
esac

cd "$repo"
if [[ -z ${CARGO_TARGET_DIR:-} ]]; then
  case "$(uname -s)" in
    Darwin) export CARGO_TARGET_DIR="${HUSHPEN_ROOT:-/Volumes/External1TB/data/_custom/droid/hushpen}/target/macos-arm64" ;;
    *) echo "Set CARGO_TARGET_DIR to the shared Hushpen target folder on the data drive." >&2; exit 1 ;;
  esac
fi
[[ $CARGO_TARGET_DIR = /* ]] || { echo "CARGO_TARGET_DIR must be absolute." >&2; exit 1; }
out=$CARGO_TARGET_DIR/mobile
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}

android_setup() {
  export ANDROID_HOME=${ANDROID_HOME:-/Volumes/External1TB/Data/Library/Android/sdk}
  export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/28.2.13676358
  [[ -f $ANDROID_NDK_HOME/build/cmake/android.toolchain.cmake ]] || {
    echo "Install Android NDK 28.2.13676358 with sdkmanager." >&2; exit 1;
  }
  command -v cargo-ndk >/dev/null || cargo install cargo-ndk --version 4.1.2 --locked --target-dir "$CARGO_TARGET_DIR"
  local ndk_version
  ndk_version=$(cargo ndk --version)
  [[ $ndk_version = 'cargo-ndk 4.1.2' ]] || {
    echo "Use cargo-ndk 4.1.2. Found: $ndk_version" >&2; exit 1;
  }
  rustup target add aarch64-linux-android x86_64-linux-android
}

ios_setup() {
  [[ $(uname -s) = Darwin ]] || { echo "Build iOS on the Mac with Xcode." >&2; exit 1; }
  xcrun --sdk iphoneos --show-sdk-path >/dev/null
  xcrun --sdk iphonesimulator --show-sdk-path >/dev/null
  rustup target add aarch64-apple-ios aarch64-apple-ios-sim
}

bindings() {
  cargo build --locked -p hushpen-mobile --lib --features bindgen
  local library=$CARGO_TARGET_DIR/debug/libhushpen_mobile.so
  [[ $(uname -s) != Darwin ]] || library=$CARGO_TARGET_DIR/debug/libhushpen_mobile.dylib
  rm -rf "$out/bindings"
  mkdir -p "$out/bindings/swift" "$out/bindings/kotlin"
  for language in swift kotlin; do
    cargo run --locked -p hushpen-mobile --features bindgen --bin uniffi-bindgen -- \
      generate --library "$library" --language "$language" --config "$repo/mobile/uniffi.toml" \
      --out-dir "$out/bindings/$language" --no-format
  done
  [[ -f $out/bindings/swift/HushpenCore.swift ]]
  [[ -f $out/bindings/kotlin/com/mandalsuraj/hushpen/core/hushpen_mobile.kt ]]
  cp "$out/bindings/swift/hushpen_mobileFFI.modulemap" "$out/bindings/swift/module.modulemap"
}

android() {
  android_setup
  cargo ndk -t arm64-v8a -t x86_64 -P 29 --link-libcxx-shared -o "$out/android/jniLibs" \
    build --locked --release -p hushpen-mobile --lib
}

ios() {
  ios_setup
  export IPHONEOS_DEPLOYMENT_TARGET=26.0
  for target in aarch64-apple-ios aarch64-apple-ios-sim; do
    local sdk=iphoneos
    [[ $target != *-sim ]] || sdk=iphonesimulator
    SDKROOT=$(xcrun --sdk "$sdk" --show-sdk-path) \
      cargo build --locked --release -p hushpen-mobile --lib --target "$target"
  done
  local headers=$out/ios/headers
  mkdir -p "$headers"
  cp "$out/bindings/swift/hushpen_mobileFFI.h" "$headers/"
  cp "$out/bindings/swift/module.modulemap" "$headers/"
  rm -rf "$out/ios/HushpenCore.xcframework"
  xcodebuild -create-xcframework \
    -library "$CARGO_TARGET_DIR/aarch64-apple-ios/release/libhushpen_mobile.a" -headers "$headers" \
    -library "$CARGO_TARGET_DIR/aarch64-apple-ios-sim/release/libhushpen_mobile.a" -headers "$headers" \
    -output "$out/ios/HushpenCore.xcframework"
}

case "$mode" in
  check) android_setup; ios_setup ;;
  bindings) bindings ;;
  android) android; bindings ;;
  ios) bindings; ios ;;
  all) android; bindings; ios ;;
esac
echo "Mobile core output: $out"
