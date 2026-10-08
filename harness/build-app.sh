#!/bin/bash
# usage: build-app.sh
# Builds the debug app (feature test-automation) and hookctl for the harness platform (linux/arm64 unless HUSHPEN_PLATFORM says otherwise) in the hushpen-build
# container and copies both to $HUSHPEN_ROOT/harness-bin, the folder every slot mounts as /app.
# The repo mounts read-only. Heavy (4 CPUs): counts as 2 busy slots.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

docker image inspect "$HARNESS_IMAGE" >/dev/null 2>&1 || die "no image $HARNESS_IMAGE; run harness-image"
mkdir -p "$HARNESS_BIN" "$HARNESS_TARGET" "$HUSHPEN_ROOT/tmp-linux" \
  "$HUSHPEN_ROOT/cargo-home-linux/registry" "$HUSHPEN_ROOT/cargo-home-linux/git"
docker rm -f hushpen-build >/dev/null 2>&1
docker run --rm --platform "$HARNESS_PLATFORM" --name hushpen-build --cpus 4 --memory 6g \
  -v "$HUSHPEN_REPO:/src:ro" \
  -v "$HARNESS_TARGET:/target" \
  -v "$HUSHPEN_ROOT/cargo-home-linux/registry:/opt/cargo/registry" \
  -v "$HUSHPEN_ROOT/cargo-home-linux/git:/opt/cargo/git" \
  -v "$HUSHPEN_ROOT/tmp-linux:/scratch" \
  -v "$HARNESS_BIN:/harness-bin" \
  -e CARGO_TARGET_DIR=/target -e TMPDIR=/scratch -e GGML_NATIVE=OFF -e LANG=C.UTF-8 \
  -w /src "$HARNESS_IMAGE" \
  sh -c 'cargo build --locked -j 4 -p hushpen-app --features test-automation -p hushpen-testhook &&
         cp /target/debug/hushpen /target/debug/hookctl /harness-bin/' ||
  die "build failed"
ls -l "$HARNESS_BIN"
