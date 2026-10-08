#!/bin/bash
# Helpers for the in-slot suite scripts. Source it after with-env.sh set RUN_OUT.
# A check is one named pass or fail with a detail line; finish() turns the list into result.json.

CHECKS=$RUN_OUT/checks.jsonl

check() { # <name> <pass|fail> <detail>
  jq -nc --arg name "$1" --arg status "$2" --arg detail "$3" \
    '{name: $name, status: $status, detail: $detail}' >>"$CHECKS"
  printf '%s %s: %s\n' "$([ "$2" = pass ] && echo PASS || echo FAIL)" "$1" "$3"
}

# check_failures <name> <ok detail> <failure lines...>: pass when no failure lines were collected
check_failures() {
  local name=$1 ok=$2 failures=$3
  if [ -z "$failures" ]; then check "$name" pass "$ok"; else check "$name" fail "$failures"; fi
}

shot() { import -window root "$RUN_OUT/$1.png"; }

# pixel <png> <x> <y>  ->  #RRGGBB at an absolute position
pixel() {
  convert "$1" -crop 1x1+"$2"+"$3" +repage -depth 8 -format '#%[hex:u.p{0,0}]' info: 2>/dev/null
}

# win_origin <wid> -> "X Y": absolute position of the client area. xdotool getwindowgeometry is
# 20 px off under openbox, so the origin comes from xwininfo.
win_origin() {
  xwininfo -id "$1" | awk '/Absolute upper-left X/ {x=$4} /Absolute upper-left Y/ {y=$4} END {print x, y}'
}

# settled_origin <wid> -> "X Y" once the position has not changed for 500 ms. The window is
# placed after it maps, so an origin read at map time can be stale.
settled_origin() {
  local last="" now same=0
  for _ in $(seq 80); do
    now=$(win_origin "$1")
    if [ "$now" = "$last" ]; then same=$((same + 1)); else same=0; fi
    [ $same -ge 5 ] && break
    last=$now
    sleep 0.1
  done
  echo "$now"
}

win_size() { # <wid> -> "WxH"
  xwininfo -id "$1" | awk '/Width:/ {w=$2} /Height:/ {h=$2} END {printf "%sx%s", w, h}'
}

app_window() { xdotool search --onlyvisible --class hushpen 2>/dev/null | head -1; }

# wait_until <seconds> <command...>: poll every 100 ms
wait_until() {
  local deadline=$((SECONDS + $1))
  shift
  while [ $SECONDS -le $deadline ]; do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; done
  return 1
}

finish() { # <suite>
  local passed
  jq -s --arg suite "$1" --arg run "${HUSHPEN_RUN_ID:-adhoc}" --arg slot "${SLOT:-}" \
    --arg commit "${HUSHPEN_COMMIT:-unknown}" \
    '{suite: $suite, run_id: $run, slot: $slot, commit: $commit,
      passed: (length > 0 and all(.[]; .status == "pass")), checks: .}' \
    "$CHECKS" >"$RUN_OUT/result.json"
  passed=$(jq -r .passed "$RUN_OUT/result.json")
  echo "result: $RUN_OUT/result.json passed=$passed"
  [ "$passed" = true ]
}

# Onboarding repair: a finished data folder that starts with no microphone opens the permissions
# step. After the microphone is back, Continue returns to the main window.
onboarding_field_is() { [ "$(/app/hookctl state | jq -r ".onboarding$1")" = "$2" ]; }
leave_onboarding_repair() {
  onboarding_field_is .mode repair || return 0
  wait_until 10 onboarding_field_is .can_continue true || return 1
  /app/hookctl click onboarding.continue >/dev/null
  wait_until 5 onboarding_field_is .active false
}
