#!/bin/bash
# Sourced by the host-side harness scripts. Everything the harness writes lives under
# $HUSHPEN_ROOT on the data drive; nothing is written into the repo.

HUSHPEN_ROOT=${HUSHPEN_ROOT:-/Volumes/External1TB/data/_custom/hushpen}
HARNESS_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
HUSHPEN_REPO=${HUSHPEN_REPO:-$(dirname "$HARNESS_DIR")}
HUSHPEN_TEST_ASSETS=${HUSHPEN_TEST_ASSETS:-/Volumes/External1TB/data/_custom/asrpro-test-assets}
HUSHPEN_TEST_FIXTURES=${HUSHPEN_TEST_FIXTURES:-/Volumes/External1TB/data/_custom/hushpen-test-assets}
HARNESS_IMAGE=${HUSHPEN_IMAGE:-hushpen-val:noble}
# linux/arm64 on the Mac; the x64 CI runner sets linux/amd64.
HARNESS_PLATFORM=${HUSHPEN_PLATFORM:-linux/arm64}
HARNESS_TARGET=$HUSHPEN_ROOT/target/linux-${HARNESS_PLATFORM#linux/}
# The debug app and hookctl the slots run. build-app.sh fills it; slots mount it read-only so a
# rebuild never changes a binary under a running slot.
HARNESS_BIN=$HUSHPEN_ROOT/harness-bin
SLOT_MIN=1
SLOT_MAX=6

die() {
  echo "harness: $*" >&2
  exit 1
}

valid_slot() { [[ $1 =~ ^[1-6]$ ]]; }

need_slot() {
  valid_slot "${1:-}" || die "slot must be a number from $SLOT_MIN to $SLOT_MAX, got '${1:-}'"
}

slot_name() { echo "hushpen-val-$1"; }

slot_dir() { echo "$HUSHPEN_ROOT/slots/$1"; }

slot_exists() { docker inspect "$(slot_name "$1")" >/dev/null 2>&1; }

need_running() {
  need_slot "$1"
  [ "$(docker inspect -f '{{.State.Running}}' "$(slot_name "$1")" 2>/dev/null)" = true ] ||
    die "$(slot_name "$1") is not running; start it with harness/start.sh $1"
}

# Run a command inside a slot with the desktop environment. HUSHPEN_RUN_ID picks the output folder.
slot_exec() {
  local slot=$1
  shift
  docker exec -e HUSHPEN_RUN_ID="${HUSHPEN_RUN_ID:-adhoc}" "$(slot_name "$slot")" \
    /harness/slot/with-env.sh "$@"
}

# The folder, on the host, that the slot sees as /out/<run id>.
slot_run_dir() { echo "$(slot_dir "$1")/out/${HUSHPEN_RUN_ID:-adhoc}"; }

# Maps a host path to the path the slot sees; copies the file in when no mount covers it.
to_slot_path() {
  local slot=$1 path=$2
  case "$path" in
    "$HUSHPEN_TEST_ASSETS"/*) echo "/assets/${path#"$HUSHPEN_TEST_ASSETS"/}" ;;
    "$HUSHPEN_TEST_FIXTURES"/*) echo "/fixtures/${path#"$HUSHPEN_TEST_FIXTURES"/}" ;;
    "$(slot_dir "$slot")/out"/*) echo "/out/${path#"$(slot_dir "$slot")/out"/}" ;;
    /assets/* | /fixtures/* | /out/* | /data/*) echo "$path" ;;
    *)
      [ -f "$path" ] || die "no such file: $path"
      mkdir -p "$(slot_dir "$slot")/out/inputs"
      cp "$path" "$(slot_dir "$slot")/out/inputs/"
      echo "/out/inputs/$(basename "$path")"
      ;;
  esac
}

running_slots() {
  docker ps --filter name=hushpen-val- --format '{{.Names}}' | sed 's/^hushpen-val-//' | sort
}
