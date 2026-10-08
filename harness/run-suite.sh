#!/bin/bash
# usage: run-suite.sh <smoke|env|mic|models|home|tail|hotkey|insert|guards|engine|core|full> --slots N [--build]
# Starts slots 1..N, runs the suite in every slot at once, prints each slot's checks, keeps only
# the newest run per suite under slots/<n>/out/, and removes the slots again (also on failure).
# smoke and env are UI-only, so N may be up to 6; any other suite is limited to 3 busy slots.
# Exit 0 only when every check in every slot passed.
#
# The run itself happens in a detached process (its own session) that writes
# $HUSHPEN_ROOT/runs/<run id>.log and, last of all, <run id>.status (the exit code). This command
# only follows that log. A caller that is killed, for example a tool call that times out or ends
# (such a call kills its whole process group), cannot take the run or its slots down with it:
# read the log and the status file afterwards. SUITE_TIMEOUT (seconds, default 3600) bounds a run.
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
  mic | models | home | tail | hotkey | insert | guards | engine | core | full) ui_only=0 ;;
  *) die "usage: run-suite.sh <smoke|env|mic|models|home|tail|hotkey|insert|guards|engine|core|full> --slots N [--build]" ;;
esac
[[ $slots =~ ^[1-6]$ ]] || die "--slots must be 1 to 6, got '$slots'"
if [ "$slots" -gt 3 ] && [ "$ui_only" != 1 ]; then
  die "$suite is not UI-only: at most 3 busy slots (an LLM slot counts as 2)"
fi

runs_dir=$HUSHPEN_ROOT/runs

# The parent: start the detached runner, then follow its log until the status file appears.
if [ -z "${HUSHPEN_RUNNER:-}" ]; then
  run_id="$suite-$(date +%Y%m%d-%H%M%S)"
  mkdir -p "$runs_dir"
  log=$runs_dir/$run_id.log
  status_file=$runs_dir/$run_id.status
  rm -f "$status_file"
  args=("$suite" --slots "$slots")
  [ "$build" = 0 ] || args+=(--build)
  HUSHPEN_RUNNER=1 HUSHPEN_RUN_ID=$run_id \
    perl -e 'use POSIX; POSIX::setsid(); exec @ARGV' "${BASH_SOURCE[0]}" "${args[@]}" \
    >"$log" 2>&1 </dev/null &
  runner=$!
  echo "harness: run $run_id (log $log, status $status_file); the run goes on if this command is stopped"
  offset=0
  flush() {
    local size
    size=$(wc -c <"$log")
    if [ "$size" -gt "$offset" ]; then
      tail -c "+$((offset + 1))" "$log" | head -c "$((size - offset))"
      offset=$size
    fi
  }
  while [ ! -f "$status_file" ]; do
    flush
    if ! kill -0 "$runner" 2>/dev/null; then
      sleep 1
      [ -f "$status_file" ] && break
      flush
      echo "harness: the runner ended without a status; see $log" >&2
      exit 1
    fi
    sleep 1
  done
  flush
  exit "$(cat "$status_file")"
fi

# The runner.
status_file=$runs_dir/$HUSHPEN_RUN_ID.status
status=1
cleanup() {
  "$HARNESS_DIR/stop-all.sh" >/dev/null
  echo "$status" >"$status_file"
}
trap cleanup EXIT
trap 'echo "harness: run interrupted by a signal; slots removed"; exit 1' HUP INT TERM

[ "$build" = 0 ] && [ -x "$HARNESS_BIN/hushpen" ] || "$HARNESS_DIR/build-app.sh" || exit 1

commit=$(git -C "$HUSHPEN_REPO" rev-parse --short HEAD 2>/dev/null || echo unknown)
[ -z "$(git -C "$HUSHPEN_REPO" status --porcelain 2>/dev/null)" ] || commit+="-dirty"
export HUSHPEN_UI_ONLY=$ui_only
ids=$(seq 1 "$slots")

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
    echo $? >"$(slot_run_dir "$slot")/exec.status"
  ) &
  pids+=($!)
done

# Wait for every slot, with a deadline. A slot whose container is gone ends its docker exec, so
# the wait below returns; the report names that case.
deadline=$((SECONDS + ${SUITE_TIMEOUT:-3600}))
timed_out=0
while :; do
  alive=0
  for pid in "${pids[@]}"; do kill -0 "$pid" 2>/dev/null && alive=1; done
  [ "$alive" = 1 ] || break
  if [ $SECONDS -ge $deadline ]; then
    timed_out=1
    echo "harness: ${SUITE_TIMEOUT:-3600} s passed; removing the slots"
    break
  fi
  sleep 2
done
for pid in "${pids[@]}"; do
  if [ "$timed_out" = 1 ]; then kill "$pid" 2>/dev/null; else wait "$pid"; fi
done

status=0
for slot in $ids; do
  result=$(slot_run_dir "$slot")/result.json
  echo "--- slot $slot: $result"
  if [ -f "$result" ]; then
    jq -r '.checks[] | "  \(.status | ascii_upcase)  \(.name): \(.detail)"' "$result"
    passed=$(jq -r .passed "$result")
    echo "  result.json passed=$passed"
    [ "$passed" = true ] || status=1
  else
    if slot_exists "$slot"; then
      echo "  FAIL  no result.json; the container is still there; see $(slot_run_dir "$slot")/suite.log"
    else
      echo "  FAIL  no result.json; $(slot_name "$slot") is gone; see $(slot_run_dir "$slot")/suite.log"
    fi
    echo "  docker exec status: $(cat "$(slot_run_dir "$slot")/exec.status" 2>/dev/null || echo none)"
    status=1
  fi
  # Keep only the newest run of this suite.
  for old in "$(slot_dir "$slot")/out/$suite"-*; do
    [ -d "$old" ] && [ "$old" != "$(slot_run_dir "$slot")" ] && rm -rf "$old"
  done
done
[ $status = 0 ] && echo "suite $suite: all slots passed" || echo "suite $suite: FAILED"
exit $status
