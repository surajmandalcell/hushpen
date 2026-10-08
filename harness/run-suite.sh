#!/bin/bash
# usage: run-suite.sh <smoke|env|mic|models|home|engine|core|full> --slots N [--build]
# Starts slots 1..N, runs the suite in every slot at once, prints each slot's checks, keeps only
# the newest run per suite under slots/<n>/out/, and removes the slots again (also on failure).
# smoke and env are UI-only, so N may be up to 6; any other suite is limited to 3 busy slots.
# Exit 0 only when every check in every slot passed.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

suite=${1:-}
shift || true
slots=${SLOTS:-3}
build=0
while [ $# -gt 0 ]; do
  case "$1" in
    --slots) slots=${2:-}; shift ;;
    --build) build=1 ;;
    *) die "unknown argument $1" ;;
  esac
  shift
done
case "$suite" in
  smoke | env) ui_only=1 ;;
  mic | models | home | engine | core | full) ui_only=0 ;;
  *) die "usage: run-suite.sh <smoke|env|mic|models|home|engine|core|full> --slots N [--build]" ;;
esac
[[ $slots =~ ^[1-6]$ ]] || die "--slots must be 1 to 6, got '$slots'"
if [ "$slots" -gt 3 ] && [ "$ui_only" != 1 ]; then
  die "$suite is not UI-only: at most 3 busy slots (an LLM slot counts as 2)"
fi

[ "$build" = 0 ] && [ -x "$HARNESS_BIN/hushpen" ] || "$HARNESS_DIR/build-app.sh" || exit 1

commit=$(git -C "$HUSHPEN_REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)
[ -z "$(git -C "$HUSHPEN_REPO" status --porcelain 2>/dev/null)" ] || commit+="-dirty"
export HUSHPEN_RUN_ID="$suite-$(date +%Y%m%d-%H%M%S)"
export HUSHPEN_UI_ONLY=$ui_only
ids=$(seq 1 "$slots")

cleanup() { "$HARNESS_DIR/stop-all.sh" >/dev/null; }
trap cleanup EXIT

echo "run $HUSHPEN_RUN_ID on $slots slot(s), commit $commit"
"$HARNESS_DIR/start.sh" $ids || exit 1
docker ps --filter name=hushpen-val- --format '  running: {{.Names}} ({{.Status}})' | sort

pids=()
for slot in $ids; do
  mkdir -p "$(slot_run_dir "$slot")"
  (
    docker exec -e HUSHPEN_RUN_ID="$HUSHPEN_RUN_ID" -e HUSHPEN_COMMIT="$commit" -e SLOT="$slot" \
      "$(slot_name "$slot")" /harness/slot/with-env.sh /harness/slot/run-suite.sh "$suite" \
      >"$(slot_run_dir "$slot")/suite.log" 2>&1
  ) &
  pids+=($!)
done
for pid in "${pids[@]}"; do wait "$pid"; done

status=0
for slot in $ids; do
  result=$(slot_run_dir "$slot")/result.json
  echo "--- slot $slot: $result"
  if [ -f "$result" ]; then
    jq -r '.checks[] | "  \(.status | ascii_upcase)  \(.name): \(.detail)"' "$result"
    [ "$(jq -r .passed "$result")" = true ] || status=1
  else
    echo "  FAIL  no result.json; see $(slot_run_dir "$slot")/suite.log"
    status=1
  fi
  # Keep only the newest run of this suite.
  for old in "$(slot_dir "$slot")/out/$suite"-*; do
    [ -d "$old" ] && [ "$old" != "$(slot_run_dir "$slot")" ] && rm -rf "$old"
  done
done
[ $status = 0 ] && echo "suite $suite: all slots passed" || echo "suite $suite: FAILED"
exit $status
