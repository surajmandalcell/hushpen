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
hint_state() {
  xprop -id "$wid" WM_NORMAL_HINTS _MOTIF_WM_HINTS _NET_FRAME_EXTENTS 2>/dev/null
}
no_frame() {
  local got
  got=$(xprop -id "$wid" _NET_FRAME_EXTENTS 2>/dev/null | sed -n 's/.*= *//p')
  case "$got" in "" | "0, 0, 0, 0") return 0 ;; *) return 1 ;; esac
}
hints_locked() {
  local now
  now=$(hint_state)
  grep -q "program specified minimum size: 940 by 600" <<<"$now" &&
    grep -q "program specified maximum size: 940 by 600" <<<"$now" &&
    grep -Eq "_MOTIF_WM_HINTS\(_MOTIF_WM_HINTS\) = (0x3, 0x2c|3, 44), 0x?0, 0x?0, 0x?0" <<<"$now" &&
    no_frame
}
# GPUI maps the window with its own hints (maximum 16384, a server frame) and the lock writes ours
# a moment later, so give the lock up to 2 s to land before the hints are judged.
lock_start=$(date +%s%3N)
wait_until 2 hints_locked
lock_ms=$(($(date +%s%3N) - lock_start))
size=$(win_size "$wid")
map_ms=$(sed -n 's/.*map_ms=\([0-9]*\).*/\1/p' <<<"$app_out")
extents=$(xprop -id "$wid" _NET_FRAME_EXTENTS 2>/dev/null | sed -n 's/.*= *//p')
hints=$(xprop -id "$wid" WM_NORMAL_HINTS 2>/dev/null)
failures=""
[ "$size" = 940x600 ] || failures+="size is $size, want 940x600; "
case "$extents" in "" | "0, 0, 0, 0") ;; *) failures+="_NET_FRAME_EXTENTS is $extents; " ;; esac
grep -q "program specified minimum size: 940 by 600" <<<"$hints" || failures+="no program minimum of 940 by 600 in WM_NORMAL_HINTS; "
grep -q "program specified maximum size: 940 by 600" <<<"$hints" || failures+="no program maximum of 940 by 600 in WM_NORMAL_HINTS; "
check_failures "window maps" "wid=$wid size=$size mapped in ${map_ms} ms, lock settled ${lock_ms} ms later, frame extents '${extents:-absent}', min and max 940 by 600" "$failures"

echo "== hint lock restores"
# A later write of the hints, as GPUI does, must not win: overwrite both properties and expect
# the lock to write ours back.
failures=""
# flags = PMinSize | PMaxSize, max 16384 x 16384, the 18 WM_SIZE_HINTS fields.
xprop -id "$wid" -format WM_NORMAL_HINTS 32i -set WM_NORMAL_HINTS "48, 0, 0, 0, 0, 1, 1, 16384, 16384, 0, 0, 0, 0, 0, 0, 0, 0, 0" ||
  failures+="could not rewrite WM_NORMAL_HINTS; "
xprop -id "$wid" -format _MOTIF_WM_HINTS 32c -set _MOTIF_WM_HINTS "2, 0, 1, 0, 0" ||
  failures+="could not rewrite _MOTIF_WM_HINTS; "
restore_start=$(date +%s%3N)
wait_until 2 hints_locked
restore_ms=$(($(date +%s%3N) - restore_start))
[ "$restore_ms" -le 1000 ] && hints_locked ||
  failures+="the lock did not restore both hints and the no-frame state within 1 s ($restore_ms ms): $(hint_state | tr '\n' ' '); "
check_failures "hint lock restores" "an overwritten WM_NORMAL_HINTS (max 16384) and _MOTIF_WM_HINTS (server frame) were written back in ${restore_ms} ms and the window kept no frame" "$failures"

echo "== token colors"
read -r ox oy < <(settled_origin "$wid")
at() { echo $((ox + $1)) $((oy + $2)); }
# The sidebar rows are 28 px high, 2 px apart, from y 48; the selection is white 10% over the sidebar.
SELECTED="#404042"
SIDEBAR="#2B2B2D"
row_y() { echo $((48 + 30 * $1 + 14)); }
# The first frame can land a moment after the window maps: wait for the selected Home fill.
rendered() { shot first && near "$(pixel "$RUN_OUT/first.png" $(at 200 "$(row_y 0)"))" "$SELECTED"; }
wait_until 8 rendered
shot first
failures=""
sample() { # <label> <x> <y> <want> [tolerance]
  local got
  got=$(pixel "$RUN_OUT/first.png" $(at "$2" "$3"))
  near "$got" "$4" "${5:-2}" || failures+="$1 at ($2,$3) is $got, want $4; "
}
sample content 925 300 "#1E1E1E"
sample sidebar 100 450 "$SIDEBAR"
sample "selected Home fill" 200 "$(row_y 0)" "$SELECTED"
sample "close light" 26 26 "#FF5F57"
sample "minimize light" 46 26 "#FEBC2E"
sample "History fill (inactive)" 200 "$(row_y 1)" "$SIDEBAR"
sample "sidebar at x 231" 231 450 "$SIDEBAR"
got=$(pixel "$RUN_OUT/first.png" $(at 234 450))
near "$got" "$SIDEBAR" && failures+="the sidebar color goes past x 232: (234,450) is $got; "
fill=$(pixel "$RUN_OUT/first.png" $(at 200 "$(row_y 0)"))
read -r fr fg fb <<<"$(printf '%d %d %d' 0x${fill:1:2} 0x${fill:3:2} 0x${fill:5:2})"
[ $((fb - fr)) -le 4 ] && [ $((fr - fb)) -le 4 ] && [ $((fg - fr)) -le 4 ] || failures+="the Home fill $fill is not neutral; "
# No toolbar bottom line: between the title and the first group every row is the content color.
for line_x in 300 586 900; do
  for line_y in $(seq 38 54); do
    sample "toolbar band" "$line_x" "$line_y" "#1E1E1E" 4
  done
done
check_failures "token colors" "content #1E1E1E, sidebar $SIDEBAR to x 231 and not at 234, selected Home fill neutral $fill, drawn lights, no line in the toolbar band, at origin $ox,$oy" "$failures"

echo "== window controls and solid sidebar"
failures=$(chrome_problems "Home")
# No third circle: where a zoom light would sit the pixel is sidebar.
third=$(pixel "$RUN_OUT/first.png" $(at 66 26))
near "$third" "$SIDEBAR" || failures+="a third control shows at (66,26): $third; "
for point in "5 100" "5 200" "100 350" "100 450" "100 550"; do
  read -r px_x px_y <<<"$point"
  got=$(pixel "$RUN_OUT/first.png" $(at "$px_x" "$px_y"))
  near "$got" "$SIDEBAR" || failures+="sidebar at ($px_x,$px_y) is $got, want $SIDEBAR within 2; "
done
check_failures "window controls and solid sidebar" "hook tree lists only window.close and window.minimize (12 px at 20,20 and 40,20), no circle at 66,26, 5 sidebar samples are $SIDEBAR" "$failures"

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

title_of() { /app/hookctl tree | jq -r '[.[] | select(.id == "view.title") | .text] | first // ""'; }
focused_item() { /app/hookctl tree | jq -r '[.[] | select((.id | startswith("sidebar.")) and .focused == true) | .id] | first // ""'; }

echo "== sidebar tree and hookctl click"
failures=""
/app/hookctl tree >"$RUN_OUT/tree-sidebar.json"
index=0
for view in home history dictionary import models settings about; do
  row=$(jq -c --arg id "sidebar.$view" '[.[] | select(.id == $id)] | first // empty' "$RUN_OUT/tree-sidebar.json")
  if [ -z "$row" ]; then
    failures+="sidebar.$view is not in the tree; "
  else
    jq -e '(.text // "") != "" and .bounds.x >= 0 and (.bounds.x + .bounds.width) <= 232 and .enabled != null and .focused != null' <<<"$row" >/dev/null ||
      failures+="sidebar.$view lacks text, enabled, focused, or bounds inside 0-232: $row; "
  fi
  /app/hookctl click "sidebar.$view" >/dev/null
  /app/hookctl wait "view=$view" 5000 >/dev/null 2>&1 || failures+="hookctl click sidebar.$view did not open the view; "
  sleep 0.3
  shot click-$view
  got=$(pixel "$RUN_OUT/click-$view.png" $(at 200 "$(row_y $index)"))
  near "$got" "$SELECTED" || failures+="after hookctl click the $view fill is $got, want $SELECTED; "
  [ "$(title_of)" = "${view^}" ] || failures+="the title after hookctl click on $view is '$(title_of)'; "
  index=$((index + 1))
done
check_failures "sidebar tree and hookctl click" "7 items have text, enabled, focused, and bounds inside 0-232; hookctl click moved the neutral fill and the title to each" "$failures"

echo "== sidebar keyboard"
failures=""
/app/hookctl action open-view '{"view":"home"}' >/dev/null
/app/hookctl wait view=home 5000 >/dev/null 2>&1
xdotool windowfocus "$wid"
read -r ox oy < <(settled_origin "$wid")
for _ in $(seq 15); do
  [ -n "$(focused_item)" ] && break
  xdotool key Tab
  sleep 0.2
done
# A hookctl click leaves the focus on its item; Home moves it to the first row by key.
xdotool key Home
sleep 0.3
for view in home history dictionary import models settings about; do
  [ "$(focused_item)" = "sidebar.$view" ] || failures+="focus is on '$(focused_item)', not sidebar.$view; "
  box=$(/app/hookctl tree | jq -r --arg id "sidebar.$view" '[.[] | select(.id == $id) | "\(.bounds.x | floor) \(.bounds.y | floor)"] | first // empty')
  if [ -n "$box" ]; then
    read -r kx ky <<<"$box"
    shot key-$view
    got=$(pixel "$RUN_OUT/key-$view.png" $(at $((kx + 1)) $((ky + 14))))
    near "$got" "#0A84FF" || failures+="the focus ring on sidebar.$view is $got, want #0A84FF within 2; "
  fi
  xdotool key Return
  /app/hookctl wait "view=$view" 5000 >/dev/null 2>&1 || failures+="Enter on sidebar.$view did not open it; "
  sleep 0.3
  [ "$(title_of)" = "${view^}" ] || failures+="the title after Enter on $view is '$(title_of)'; "
  xdotool key Down
  sleep 0.2
done
check_failures "sidebar keyboard" "Tab reached the sidebar; each of the 7 items showed the accent focus ring (#0A84FF), Enter opened it, and Down moved on" "$failures"
/app/hookctl action open-view '{"view":"home"}' >/dev/null
/app/hookctl wait view=home 5000 >/dev/null 2>&1

echo "== light appearance"
failures=""
rendered_is() { [ "$(/app/hookctl state | jq -r .appearance.rendered)" = "$1" ]; }
/app/hookctl action set-appearance '{"appearance":"light"}' >/dev/null || failures+="set-appearance light failed; "
wait_until 5 rendered_is light || failures+="the window did not render light; "
sleep 0.3
shot light
sample_light() { # <label> <x> <y> <want>
  local got
  got=$(pixel "$RUN_OUT/light.png" $(at "$2" "$3"))
  near "$got" "$4" || failures+="light $1 at ($2,$3) is $got, want $4; "
}
sample_light content 925 300 "#F5F5F5"
sample_light sidebar 100 450 "#E8E8EA"
sample_light "close light" 26 26 "#FF5F57"
/app/hookctl action set-appearance '{"appearance":"system"}' >/dev/null || failures+="set-appearance system failed; "
wait_until 5 rendered_is dark || failures+="the window did not return to dark; "
check_failures "light appearance" "set-appearance light drew content #F5F5F5 and the solid light sidebar #E8E8EA; system returned to dark (unknown appearance)" "$failures"

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
    fill=$(pixel "$RUN_OUT/after-click.png" $(at 200 "$(row_y 5)"))
    home=$(pixel "$RUN_OUT/after-click.png" $(at 200 "$(row_y 0)"))
    if near "$fill" "$SELECTED" && near "$home" "$SIDEBAR"; then
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
