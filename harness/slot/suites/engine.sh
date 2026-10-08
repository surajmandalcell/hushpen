#!/bin/bash
# Engine supervisor checks (VAL-ENG-011). Starts a fresh app, so no earlier crash counts toward the
# backoff, then kills the engine child three times in a row. It restarts the app and empties
# /data, so the slot must not be shared. Takes about 80 s.
. /harness/slot/lib.sh

HC=/app/hookctl
now_ms() { echo $(($(date +%s%N) / 1000000)); }

engine() { $HC state | jq -r ".engine$1"; }
engine_ready() { [ "$(engine .state)" = ready ]; }
ready_with_new_pid() { [ "$(engine .state)" = ready ] && [ "$(engine .pid)" != "$1" ]; }

/harness/slot/app.sh stop
find /data -mindepth 1 -delete
/harness/slot/app.sh start >/dev/null
wait_until 20 engine_ready || { check "VAL-ENG-011 backoff" fail "the engine never became ready"; exit 0; }

ui_pid=$(cat /tmp/app.pid)
expected=(1000 5000 30000)
tolerance=(500 1000 3000)
gaps=() failures=""
for i in 0 1 2; do
  pid=$(engine .pid)
  : >"$RUN_OUT/011-gap-$i.txt"
  killed=$(now_ms)
  kill -9 "$pid"
  answered=0 polls=0
  deadline=$((SECONDS + 50))
  while [ $SECONDS -le $deadline ]; do
    polls=$((polls + 1))
    if state=$($HC state 2>/dev/null) && [ -n "$state" ]; then answered=$((answered + 1)); fi
    [ "$(jq -r .engine.state <<<"$state")" = ready ] &&
      [ "$(jq -r .engine.pid <<<"$state")" != "$pid" ] && break
    sleep 0.1
  done
  gap=$(($(now_ms) - killed))
  gaps+=("$gap")
  echo "kill $((i + 1)): pid $pid, gap $gap ms, hook answered $answered of $polls polls" >>"$RUN_OUT/011-gap-$i.txt"
  diff=$((gap - expected[i]))
  [ "$diff" -lt 0 ] && diff=$((-diff))
  [ "$diff" -le "${tolerance[i]}" ] || failures+="gap $((i + 1)) was $gap ms, expected ${expected[i]} ms +-${tolerance[i]}; "
  [ "$answered" = "$polls" ] || failures+="the hook missed $((polls - answered)) of $polls polls in gap $((i + 1)); "
done
[ "$(cat /tmp/app.pid)" = "$ui_pid" ] && kill -0 "$ui_pid" 2>/dev/null || failures+="the UI pid changed; "
[ -n "$(app_window)" ] || failures+="the window is not mapped; "
cp /data/logs/engine.log "$RUN_OUT/011-engine.log" 2>/dev/null
check_failures "VAL-ENG-011 backoff" "gaps ${gaps[*]} ms for 1000, 5000, 30000; UI pid $ui_pid unchanged, window mapped, hook answered in every gap" "$failures"
