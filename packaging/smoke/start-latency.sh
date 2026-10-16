#!/bin/bash
# Runs inside a harness slot (through with-env.sh). Presses the hold key RUNS times with nothing
# said and prints, for each press, the time from key down to the listening state and every UI
# stall (a main-thread beat gap over 100 ms, recorded by the test hook) in the first 1.5 s.
# Fails when a press takes longer than LISTEN_LIMIT ms to reach listening or any stall is over
# STALL_LIMIT ms.
#
# The hook exists only in debug builds, so the build under test is a release-profile build with
# debug assertions switched on (`CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true cargo build
# --release --features test-automation`), copied to /app/hushpen-hookrel (APP_BIN).
#
# usage: start-latency.sh [runs]       env: APP_BIN, LISTEN_LIMIT (ms), STALL_LIMIT (ms)
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

runs=${1:-10}
HOLD=108
LISTEN_LIMIT=${LISTEN_LIMIT:-500}
STALL_LIMIT=${STALL_LIMIT:-100}
export APP_BIN=${APP_BIN:-/app/hushpen}

pstate_is() { [ "$($HC state | jq -r .pipeline.state)" = "$1" ]; }
event_ms() { # <kind> <detail> -> unix ms of the first such event since T0
  $HC events | jq -r --argjson t0 "$T0" --arg k "$1" --arg d "$2" \
    '[.[] | select(.kind == $k and .t_ms >= $t0 and .detail == $d) | .t_ms] | first // empty'
}
stalls_ms() { # every ui-stall since T0 in ms, one per line
  $HC events | jq -r --argjson t0 "$T0" \
    '.[] | select(.kind == "ui-stall" and .t_ms >= $t0) | .detail | rtrimstr("ms")'
}

fresh tiny.en
use_model tiny.en
sleep 5

failures=""
worst_listen=0
worst_stall=0
latencies=()
for run in $(seq "$runs"); do
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; failures+="run $run never reached listening; "; continue; }
  sleep 1.5
  at=$(event_ms pipeline listening)
  gap=$((at - T0))
  stall=0
  while read -r ms; do
    [ -n "$ms" ] && [ "$ms" -gt "$stall" ] && stall=$ms
  done < <(stalls_ms)
  xdotool keyup $HOLD
  wait_until 30 pstate_is idle
  echo "run $run: key down to listening $gap ms, worst UI stall ${stall} ms"
  latencies+=("$gap")
  [ "$gap" -le "$worst_listen" ] || worst_listen=$gap
  [ "$stall" -le "$worst_stall" ] || worst_stall=$stall
  [ "$gap" -le "$LISTEN_LIMIT" ] || failures+="run $run took $gap ms to listen; "
  [ "$stall" -le "$STALL_LIMIT" ] || failures+="run $run stalled the UI for $stall ms; "
  sleep 1
done

echo "binary: $APP_BIN"
echo "key down to listening: ${latencies[*]} ms (worst $worst_listen ms, limit $LISTEN_LIMIT ms)"
echo "worst UI stall: $worst_stall ms (limit $STALL_LIMIT ms; 0 means no beat gap over 100 ms)"
check_failures "start latency" "$runs presses, worst key down to listening $worst_listen ms, worst UI stall $worst_stall ms" "$failures"
finish start-latency
