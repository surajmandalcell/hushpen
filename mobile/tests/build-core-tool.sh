#!/usr/bin/env bash
set -euo pipefail
printf '%s %s\n' "$(basename "$0")" "$*" >> "$BUILD_CORE_TEST_LOG"
case "$(basename "$0")" in
  uname) echo Darwin ;;
  xcrun) echo "/sdk/$2" ;;
  cargo)
    case "$1" in
      ndk)
        [[ ${2:-} != --version ]] || echo "${BUILD_CORE_TEST_NDK_VERSION:-cargo-ndk 4.1.2}"
        ;;
      build)
        case "$*" in
          *'--target aarch64-apple-ios-sim') [[ $SDKROOT = /sdk/iphonesimulator ]] ;;
          *'--target aarch64-apple-ios') [[ $SDKROOT = /sdk/iphoneos ]] ;;
        esac
        mkdir -p "$CARGO_TARGET_DIR/debug"
        touch "$CARGO_TARGET_DIR/debug/libhushpen_mobile.dylib"
        ;;
      run)
        while (($#)); do
          case "$1" in
            --language) language=$2; shift ;;
            --out-dir) directory=$2; shift ;;
          esac
          shift
        done
        mkdir -p "$directory"
        if [[ $language = swift ]]; then
          touch "$directory/hushpen_mobileFFI.h" "$directory/hushpen_mobileFFI.modulemap" "$directory/HushpenCore.swift"
        else
          mkdir -p "$directory/com/mandalsuraj/hushpen/core"
          touch "$directory/com/mandalsuraj/hushpen/core/hushpen_mobile.kt"
        fi
        ;;
    esac
    ;;
  xcodebuild)
    [[ ${SDKROOT:-} != /sdk/iphoneos && ${SDKROOT:-} != /sdk/iphonesimulator ]]
    ;;
esac
