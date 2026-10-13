#!/bin/bash
# First end-to-end smoke (UI only): the window maps at 940x600 with no frame and pinned size
# hints, the DESIGN.md colors are where they belong, the drawn close and minimize lights are the
# only window controls and the Linux sidebar is solid, every main view keeps the size, a real
# mouse click on a sidebar item opens its view, and no window-manager maximize path changes the
# size. Runs in one slot through with-env.sh.
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
extents=$(xprop -id "$wid" _NET_FRAME_EXTENTS 2>/dev/null | sed -n 's/.*= *//p')
hints=$(xprop -id "$wid" WM_NORMAL_HINTS 2>/dev/null)
failures=""
[ "$size" = 940x600 ] || failures+="size is $size, want 940x600; "
case "$extents" in "" | "0, 0, 0, 0") ;; *) failures+="_NET_FRAME_EXTENTS is $extents; " ;; esac
grep -q "program specified minimum size: 940 by 600" <<<"$hints" || failures+="no program minimum of 940 by 600 in WM_NORMAL_HINTS; "
grep -q "program specified maximum size: 940 by 600" <<<"$hints" || failures+="no program maximum of 940 by 600 in WM_NORMAL_HINTS; "
check_failures "window maps" "wid=$wid size=$size mapped in ${map_ms} ms, frame extents '${extents:-absent}', min and max 940 by 600" "$failures"

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
sample sidebar 100 450 "#2B2B2D"
sample "active Home fill" 190 66 "#686868"
sample "close light" 26 26 "#FF5F57"
sample "minimize light" 46 26 "#FEBC2E"
sample "History fill (inactive)" 190 106 "#2B2B2D"
check_failures "token colors" "6 samples match DESIGN.md at origin $ox,$oy" "$failures"

echo "== window controls and solid sidebar"
failures=$(chrome_problems "Home")
# No third circle: where a zoom light would sit the pixel is sidebar.
third=$(pixel "$RUN_OUT/first.png" $(at 66 26))
near "$third" "#2B2B2D" || failures+="a third control shows at (66,26): $third; "
for point in "5 100" "5 200" "100 350" "100 450" "100 550"; do
  read -r px_x px_y <<<"$point"
  got=$(pixel "$RUN_OUT/first.png" $(at "$px_x" "$px_y"))
  near "$got" "#2B2B2D" || failures+="sidebar at ($px_x,$px_y) is $got, want #2B2B2D within 2; "
done
check_failures "window controls and solid sidebar" "hook tree lists only window.close and window.minimize (12 px at 20,20 and 40,20), no circle at 66,26, 5 sidebar samples are #2B2B2D" "$failures"

echo "== every main view keeps 940x600"
failures=""
for view in home history dictionary import models settings about; do
  /app/hookctl action open-view "{\"view\":\"$view\"}" >/dev/null
  /app/hookctl wait "view=$view" 5000 >/dev/null 2>&1 || failures+="$view did not open; "
  sleep 0.3
  failures+=$(chrome_problems "$view")
done
check_failures "every main view keeps 940x600" "7 views: 940x600 on the display and in the hook state, resizable, maximized, and fullscreen false, only close and minimize" "$failures"
/app/hookctl action open-view '{"view":"home"}' >/dev/null
/app/hookctl wait view=home 5000 >/dev/null 2>&1

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
    if [ "$fill" = "#686868" ] && [ "$home" = "#2B2B2D" ]; then
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
  [ "$got" = 940x600 ] || failures+="$1 gave $got; "
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
check_failures "maximize check" "5 paths (wmctrl maximize and fullscreen, xdotool windowsize, Alt+F10, toolbar double click) kept 940x600" "$failures"

echo "== drawn lights act"
failures=""
wm_state() { xprop -id "$wid" WM_STATE 2>/dev/null | sed -n 's/.*window state: *//p'; }
map_state() { xwininfo -id "$wid" 2>/dev/null | awk '/Map State/ {print $3}'; }
is_iconic() { [ "$(wm_state)" = Iconic ]; }
is_unmapped() { [ "$(map_state)" = IsUnMapped ]; }
is_mapped() { [ "$(map_state)" = IsViewable ]; }
light_click() { # <hook id>: a real pointer click at the center of the drawn light
  local bounds bx by
  xdotool windowraise "$wid"
  read -r ox oy < <(settled_origin "$wid")
  bounds=$(/app/hookctl tree | jq -r --arg id "$1" '.[] | select(.id == $id) | "\(.bounds.x + .bounds.width / 2 | floor) \(.bounds.y + .bounds.height / 2 | floor)"')
  [ -n "$bounds" ] || return 1
  read -r bx by <<<"$bounds"
  xdotool mousemove $((ox + bx)) $((oy + by))
  sleep 0.3
  xdotool click 1
}
light_click window.minimize || failures+="no minimize light to click; "
wait_until 5 is_iconic || failures+="minimize left WM_STATE '$(wm_state)', not Iconic; "
shot after-minimize
xdotool windowmap "$wid"
wait_until 5 is_mapped || failures+="windowmap did not restore the window ($(map_state)); "
sleep 0.5
light_click window.close || failures+="no close light to click; "
wait_until 2 is_unmapped || failures+="close left the window $(map_state) after 2 s; "
shot after-close
ui_ok=$(pgrep -x hushpen >/dev/null && echo yes || echo no)
[ "$ui_ok" = yes ] || failures+="the UI process ended on close; "
check_failures "drawn lights act" "minimize click gave WM_STATE Iconic, windowmap restored the window, close click unmapped it within 2 s and the process kept running" "$failures"
