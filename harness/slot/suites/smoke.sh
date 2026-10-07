#!/bin/bash
# First end-to-end smoke (UI only): the window maps at 780x520, the DESIGN.md colors are where
# they belong, a real mouse click on a sidebar item opens its view, and no window-manager
# maximize path changes the size. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh

echo "== window maps"
app_out=$(/harness/slot/app.sh restart 2>&1)
echo "$app_out"
wid=$(app_window)
if [ -z "$wid" ]; then
  check "window maps" fail "no hushpen window: $app_out"
  exit 1
fi
size=$(win_size "$wid")
map_ms=$(sed -n 's/.*map_ms=\([0-9]*\).*/\1/p' <<<"$app_out")
if [ "$size" = 780x520 ]; then
  check "window maps" pass "wid=$wid size=$size mapped in ${map_ms} ms"
else
  check "window maps" fail "wid=$wid size=$size, want 780x520"
fi

echo "== token colors"
read -r ox oy < <(settled_origin "$wid")
at() { echo $((ox + $1)) $((oy + $2)); }
# The first frame can land a moment after the window maps: wait for the active Home fill.
rendered() { shot first && [ "$(pixel "$RUN_OUT/first.png" $(at 190 66))" = "#686868" ]; }
wait_until 8 rendered
shot first
failures=""
sample() { # <label> <x> <y> <want>
  local got
  got=$(pixel "$RUN_OUT/first.png" $(at "$2" "$3"))
  [ "$got" = "$4" ] || failures+="$1 at ($2,$3) is $got, want $4; "
}
sample content 220 300 "#2F2F2F"
sample sidebar 100 450 "#3C3C3C"
sample "active Home fill" 190 66 "#686868"
sample "close light" 26 24 "#FF5F57"
sample "minimize light" 47 24 "#FEBC2E"
sample "History fill (inactive)" 190 106 "#3C3C3C"
check_failures "token colors" "6 samples match DESIGN.md at origin $ox,$oy" "$failures"

echo "== click"
/app/hookctl tree >"$RUN_OUT/tree.json" 2>"$RUN_OUT/tree.err"
/app/hookctl state >"$RUN_OUT/state-before.json" 2>>"$RUN_OUT/tree.err"
bounds=$(jq -c '.[] | select(.id == "sidebar.settings") | .bounds' "$RUN_OUT/tree.json")
if [ -z "$bounds" ]; then
  check click fail "sidebar.settings is not in the hook tree"
else
  bx=$(jq -r '.x + 20 | floor' <<<"$bounds")
  by=$(jq -r '.y + .height / 2 | floor' <<<"$bounds")
  xdotool mousemove $(at "$bx" "$by") click 1
  if /app/hookctl wait view=settings 5000 >/dev/null 2>&1; then
    sleep 0.3
    shot after-click
    /app/hookctl state >"$RUN_OUT/state-after.json"
    fill=$(pixel "$RUN_OUT/after-click.png" $(at 190 266))
    home=$(pixel "$RUN_OUT/after-click.png" $(at 190 66))
    if [ "$fill" = "#686868" ] && [ "$home" = "#3C3C3C" ]; then
      check click pass "mouse click at ($bx,$by) opened Settings: its fill is $fill, Home is $home"
    else
      check click fail "view is settings but fills are Settings=$fill Home=$home"
    fi
  else
    check click fail "the view did not become settings after a click at ($bx,$by)"
  fi
fi

echo "== maximize check"
state() { xprop -id "$wid" _NET_WM_STATE | cut -d= -f2; }
failures=""
expect_size() { # <label>
  local got
  got=$(win_size "$wid")
  [ "$got" = 780x520 ] || failures+="$1 gave $got; "
  echo "$1: $got state=$(state)"
}
wmctrl -i -r "$wid" -b add,maximized_vert,maximized_horz
sleep 0.8
expect_size "wmctrl maximize"
shot after-maximize
wmctrl -i -r "$wid" -b add,fullscreen
sleep 0.8
expect_size "wmctrl fullscreen"
wmctrl -i -r "$wid" -b remove,fullscreen
sleep 0.4
xdotool windowsize "$wid" 1000 700
sleep 0.8
expect_size "xdotool windowsize 1000x700"
xdotool windowfocus "$wid"
xdotool key alt+F10
sleep 0.8
expect_size "Alt+F10 (openbox ToggleMaximize)"
read -r ox oy < <(win_origin "$wid")
xdotool mousemove $((ox + 500)) $((oy + 17)) click --repeat 2 --delay 80 1
sleep 0.8
expect_size "toolbar double click"
shot after-maximize-attempts
case "$(state)" in
  *MAXIMIZED* | *FULLSCREEN*) failures+="final state is $(state); " ;;
esac
check_failures "maximize check" "5 paths (wmctrl maximize and fullscreen, xdotool windowsize, Alt+F10, toolbar double click) kept 780x520" "$failures"
