#!/bin/bash
# usage: start.sh [--no-app] [--keep-data] <slot>...      slots are 1 to 6
# Starts hushpen-val-<slot> (2 CPUs, 2 GB), waits until its desktop is ready, and starts the app.
# Slot data is wiped first unless --keep-data. More than 3 slots at once needs HUSHPEN_UI_ONLY=1
# (the suites set it): only UI-only suites may use up to 6. An LLM slot counts as 2 of the 3.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

start_app=1
keep_data=0
slots=()
while [ $# -gt 0 ]; do
  case "$1" in
    --no-app) start_app=0 ;;
    --keep-data) keep_data=1 ;;
    *) need_slot "$1"; slots+=("$1") ;;
  esac
  shift
done
[ ${#slots[@]} -gt 0 ] || die "usage: start.sh [--no-app] [--keep-data] <slot>..."
if [ ${#slots[@]} -gt 3 ] && [ "${HUSHPEN_UI_ONLY:-0}" != 1 ]; then
  die "more than 3 slots is for UI-only suites; set HUSHPEN_UI_ONLY=1 if this one is"
fi
[ "$start_app" = 0 ] || [ -x "$HARNESS_BIN/hushpen" ] ||
  die "no app in $HARNESS_BIN; run harness/build-app.sh first (or pass --no-app)"
docker image inspect "$HARNESS_IMAGE" >/dev/null 2>&1 || die "no image $HARNESS_IMAGE; run harness-image"

start_one() {
  local slot=$1 name dir
  name=$(slot_name "$slot")
  dir=$(slot_dir "$slot")
  docker rm -f "$name" >/dev/null 2>&1
  mkdir -p "$dir/data" "$dir/out" "$dir/logs"
  # Empty the folders but keep them: a folder deleted and recreated under a fresh bind mount
  # once showed up as a missing /data in the next container.
  [ "$keep_data" = 1 ] || find "${dir:?}/data" "${dir:?}/logs" -mindepth 1 -delete
  local fixtures=()
  [ -d "$HUSHPEN_TEST_FIXTURES" ] && fixtures=(-v "$HUSHPEN_TEST_FIXTURES:/fixtures:ro")
  docker run -d --rm --init --platform linux/arm64 --name "$name" \
    --cpus "${VAL_CPUS:-2}" --memory "${VAL_MEM:-2g}" \
    -e SLOT="$slot" \
    -v "$HARNESS_DIR:/harness:ro" \
    -v "$HARNESS_BIN:/app:ro" \
    -v "$dir/data:/data" -v "$dir/out:/out" -v "$dir/logs:/logs" \
    -v "$HUSHPEN_TEST_ASSETS:/assets:ro" "${fixtures[@]}" \
    "$HARNESS_IMAGE" /harness/slot/init.sh >/dev/null || return 1
  for _ in $(seq 300); do
    docker exec "$name" test -f /tmp/slot-ready 2>/dev/null && break
    sleep 0.1
  done
  docker exec "$name" test -f /tmp/slot-ready 2>/dev/null || {
    echo "$name did not become ready; init log:" >&2
    cat "$dir/logs/init.log" >&2
    return 1
  }
  if [ "$start_app" = 1 ]; then
    slot_exec "$slot" /harness/slot/app.sh start | sed "s/^/$name: /" || return 1
  fi
  echo "$name ready"
}

pids=()
for slot in "${slots[@]}"; do
  start_one "$slot" &
  pids+=($!)
done
status=0
for pid in "${pids[@]}"; do wait "$pid" || status=1; done
exit $status
