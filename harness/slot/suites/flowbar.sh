#!/bin/bash
# The flow bar: VAL-BAR-001 to 009, VAL-CROSS-010 and 011. The bar is a pop-up window that never
# takes the keyboard focus, so every check reads `xdotool getwindowfocus` while the GTK entry
# holds it. The bar's state, bounds, and elements come from the test hook (state section
# `overlay`, tree ids flowbar.*; its state changes are `overlay` events). Clicks and drags are
# real X pointer input from xdotool. Screenshots are cropped to the bar's bounds and checked
# for the dark pill and the colors of each state; the pixel baselines at scale 2 are held by the
# `test-pixel` gate. The words come from the virtual microphone. Needs speech-short.wav,
# dictation-45s.wav, and silence-5s.wav in /assets/fixtures and the base-q5_1 model in
# /assets/models/whisper. Busy: it transcribes. It empties /data and restarts
# the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
SHORT_WAV=$FIX/speech-short.wav
LONG_WAV=$FIX/dictation-45s.wav
SILENCE_WAV=$FIX/silence-5s.wav
SCREEN_W=1280
SCREEN_H=800

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() { # the pipeline states since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
li() { $HC state | jq -r ".last_insert$1"; }
lij() { $HC state | jq -c ".last_insert | ${1:-.}"; }
# is '<command>' <test operator> <value>: true when the command's output passes the test
is() { [ "$(eval "$1")" "$2" "$3" ]; }
focus() { xdotool windowfocus --sync "$1"; }
fid() { xdotool getwindowfocus; }
speech() { paplay --device=vmic "$(pad "$1")"; }
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
setting() { jq -c ".values[\"$1\"]" "$SETTINGS"; }
q() { sqlite3 "$DB" "$1"; }
newest() { q "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
count() { q "select count(*) from transcript"; }
gtk_commit() { # Enter in the GTK entry writes what it holds to /out/gtk.txt; prints it
  rm -f "$GTK_FILE"
  focus "$GT"
  xdotool key Return
  wait_until 5 test -e "$GTK_FILE"
  cat "$GTK_FILE"
}
gtk_clear() {
  focus "$GT"
  xdotool key ctrl+a BackSpace
  sleep 0.2
}

bar() { $HC state | jq -r ".overlay$1"; }
bar_is() { [ "$(bar .state)" = "$1" ]; }
bar_visible() { [ "$(bar .visible)" = "$1" ]; }
# settle: the bar's spring and fade have stopped, so its bounds and pixels are final
settle() { wait_until 2 is 'bar .settled' = true || true; }
bar_box() { settle; $HC state | jq -r '.overlay.bounds | "\(.width | floor) \(.height | floor) \(.x | floor) \(.y | floor)"'; }
bar_x() { settle; $HC state | jq -r '.overlay.bounds | (.x + .width / 2) | floor'; }
bar_top() { settle; $HC state | jq -r '.overlay.bounds.y | floor'; }
bar_bottom() { settle; $HC state | jq -r '.overlay.bounds | (.y + .height) | floor'; }
el_center() { # <tree id> -> "x y": the middle of the element on the screen
  $HC tree | jq -r ".[] | select(.id == \"$1\") | .root_bounds | \"\\((.x + .width / 2) | floor) \\((.y + .height / 2) | floor)\""
}
el_text() { $HC tree | jq -r ".[] | select(.id == \"$1\") | .text"; }
click_el() { # a real pointer click on a tree element; the pointer rests there for 0.4 s first
  local x y
  read -r x y < <(el_center "$1")
  [ -n "$x" ] || return 1
  xdotool mousemove "$x" "$y"
  sleep 0.4
  xdotool click 1
  sleep 0.2
}
park() { xdotool mousemove 5 5; sleep 0.2; }

overlay_since() { # the bar's state changes since T0, as "a@ms b@ms"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "overlay" and .t_ms >= $t0) | "\(.detail)@\(.t_ms - $t0)"] | join(" ")'
}
flash_ms() { # how long the bar showed result before it went back to idle, since T0
  $HC events | jq -r --argjson t0 "$T0" '
    [.[] | select(.kind == "overlay" and .t_ms >= $t0)] as $e
    | ($e | map(select(.detail == "result")) | first | .t_ms) as $r
    | ($e | map(select(.detail == "idle" and .t_ms > $r)) | first | .t_ms) as $i
    | if $r == null or $i == null then empty else $i - $r end'
}

shot_bar() { # <name>: the whole screen and the bar's crop
  local w h x y
  read -r w h x y < <(bar_box)
  shot "$1"
  convert "$RUN_OUT/$1.png" -crop "${w}x${h}+${x}+${y}" +repage "$RUN_OUT/$1-bar.png"
}
luma() { convert "$1" -colorspace Gray -format '%[fx:mean]' info:; }
is_dark() { awk -v v="$1" 'BEGIN { exit !(v < 0.35) }'; }
# reddish <png>: the number of pixels in the error color family (#FF9C8F text)
reddish() {
  convert "$1" -depth 8 txt:- |
    sed -n 's/^[0-9]*,[0-9]*: (\([0-9]*\),\([0-9]*\),\([0-9]*\).*/\1 \2 \3/p' |
    awk '$1 > 200 && $2 > 100 && $2 < 200 && $3 > 100 && $3 < 200 && $1 - $2 > 40 { n++ } END { print n + 0 }'
}
# lit <png>: the number of pixels clearly brighter than the pill (text, icons, bars)
lit() {
  convert "$1" -depth 8 txt:- |
    sed -n 's/^[0-9]*,[0-9]*: (\([0-9]*\),\([0-9]*\),\([0-9]*\).*/\1 \2 \3/p' |
    awk '$1 + $2 + $3 > 450 { n++ } END { print n + 0 }'
}
inner_extent() { # <png> -> the height of the waveform: the middle 48 px, clear of the cross and the tick
  local w h
  w=$(identify -format %w "$1")
  h=$(identify -format %h "$1")
  convert "$1" -crop "48x$((h - 8))+$((w / 2 - 24))+4" +repage -trim -format '%h' info:
}
differ() { # <png> <png>: true when they differ in at least one pixel
  local n
  n=$(compare -metric AE -fuzz 2% "$1" "$2" null: 2>&1 | awk '{ print $1 }')
  [ "${n:-0}" != 0 ]
}
same_pixels() { ! differ "$1" "$2"; }

bar_wid() { # the X window of the bar, found by its geometry
  local w h x y
  read -r w h x y < <(bar_box)
  python3 /harness/slot/xstack.py find "$x" "$y" "$w" "$h"
}

# hold_start <wav>: the hold key goes down and the wav plays; hold_stop lifts the key
hold_start() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
}
hold_stop() { xdotool keyup $HOLD; }
ptt_run() {
  hold_start || return 1
  speech "$1"
  hold_stop
  wait_until 240 run_ended
}
hook_hold_start() { # a hold through the hook, for the runs that a keyboard grab would block
  T0=$(now_ms)
  hook_action pipeline-event '{"event":"hold-down"}' >/dev/null
  wait_until 5 pstate_is listening || {
    hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
    return 1
  }
}
hook_hold_stop() { hook_action pipeline-event '{"event":"hold-up"}' >/dev/null; }

focus_log=$RUN_OUT/focus.log
focus_watch_start() { # writes each change of the focused window to focus.log until stopped
  : >"$focus_log"
  (
    last=""
    while true; do
      now=$(xdotool getwindowfocus)
      [ "$now" = "$last" ] || echo "$now" >>"$focus_log"
      last=$now
      sleep 0.05
    done
  ) &
  WATCH=$!
}
focus_watch_stop() {
  kill "$WATCH" 2>/dev/null
  wait "$WATCH" 2>/dev/null
}

echo "== setup: the base-q5_1 model, the paste targets"
fresh base-q5_1
use_model base-q5_1
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
# By default no bar shows at rest. The suite turns the idle pill on to measure and drag it.
sleep 1
bar_visible false || fail_note="a bar shows at rest with the default settings ($(bar_box)); "
[ "$(setting overlay.idleVisible)" = false ] || fail_note+="overlay.idleVisible is $(setting overlay.idleVisible) on a fresh start; "
set_setting overlay.idleVisible true
wait_until 10 bar_visible true
park

echo "== VAL-BAR-006 the default position is bottom center, top with overlay.position top"
failures=""
cx=$(bar_x)
top=$(bar_top)
bottom=$(bar_bottom)
[ "${cx#-}" -ge 638 ] && [ "$cx" -le 642 ] || failures+="the bottom bar's center x is $cx; "
[ "$top" -ge $((SCREEN_H * 3 / 4)) ] && [ "$bottom" -le "$SCREEN_H" ] || failures+="the bar spans $top to $bottom, not the bottom quarter; "
shot_bar bar-006-bottom
bottom_box=$(bar_box)
set_setting overlay.position '"top"'
wait_until 5 is 'bar_top' -lt 200
cx2=$(bar_x)
top2=$(bar_top)
bottom2=$(bar_bottom)
[ "$cx2" -ge 638 ] && [ "$cx2" -le 642 ] || failures+="the top bar's center x is $cx2; "
[ "$top2" -ge 0 ] && [ "$bottom2" -le $((SCREEN_H / 4)) ] || failures+="the bar spans $top2 to $bottom2, not the top quarter; "
shot_bar bar-006-top
set_setting overlay.position '"bottom"'
wait_until 5 is 'bar_box' = "$bottom_box" || failures+="the bar did not return to bottom: $(bar_box) instead of $bottom_box; "
failures+=${fail_note:-}
check_failures "VAL-BAR-006 default position" "no bar at rest by default; bottom bar $bottom_box (x center $cx, $top to $bottom of $SCREEN_H); top bar center $cx2, $top2 to $bottom2" "$failures"

echo "== VAL-BAR-001 the bar never takes the focus"
failures=""
gtk_clear
F0=$(fid)
[ "$F0" != "" ] || failures+="no focused window; "
set_setting overlay.idleVisible false
wait_until 5 bar_visible false || failures+="the bar did not hide; "
sleep 0.3
set_setting overlay.idleVisible true
wait_until 5 bar_visible true || failures+="the bar did not map again; "
sleep 0.6
F1=$(fid)
[ "$F1" = "$F0" ] || failures+="focus moved from $F0 to $F1 when the bar mapped; "
focus_watch_start
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
sleep 0.4
focus_watch_stop
distinct=$(sort -u "$focus_log" | tr '\n' ' ')
[ "$distinct" = "$F0 " ] || failures+="the focus read $distinct during the run (expected only $F0); "
events=$(overlay_since)
case "$events" in *listening*transcribing*) ;; *) failures+="the bar's states were '$events'; " ;; esac
got=$(gtk_commit)
[ -z "$(missing_words "$(norm <<<"$got")" "${SHORT_WORDS[@]}")" ] || failures+="the entry holds '$got'; "
gtk_clear
xdotool type --delay 40 xyz
typed=$(gtk_commit)
[ "$typed" = xyz ] || failures+="typing with the bar visible gave '$typed'; "
[ "$(fid)" = "$F0" ] || failures+="focus ended on $(fid); "
check_failures "VAL-BAR-001 no focus" "focus stayed $F0 at map, during listening, transcribing, and after the insert ($events); the entry holds '$got' and typed '$typed'" "$failures"

echo "== VAL-BAR-002 each state, VAL-BAR-009 Open history, VAL-CROSS-010 a blocked insert"
failures=""
gtk_clear
set_clip OLD
wait_until 5 bar_is idle || failures+="the bar is not idle; "
shot_bar bar-002-idle
idle_state=$(bar .state)
idle_luma=$(luma "$RUN_OUT/bar-002-idle-bar.png")
is_dark "$idle_luma" || failures+="the idle bar is not a dark pill (mean $idle_luma); "

# listening, transcribing, and result in one run of the 45 s dictation
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="the run did not reach listening; "
wait_until 5 bar_is listening || failures+="the bar did not show listening; "
shot_bar bar-002-listening
listen_state=$(bar .state)
listen_luma=$(luma "$RUN_OUT/bar-002-listening-bar.png")
is_dark "$listen_luma" || failures+="the listening bar is not a dark pill (mean $listen_luma); "
paplay --device=vmic "$(pad "$LONG_WAV")"
xdotool keyup $HOLD
wait_until 20 bar_is transcribing || failures+="the bar did not show transcribing; "
shot_bar bar-002-transcribing
trans_state=$(bar .state)
trans_luma=$(luma "$RUN_OUT/bar-002-transcribing-bar.png")
is_dark "$trans_luma" || failures+="the transcribing bar is not a dark pill (mean $trans_luma); "
differ "$RUN_OUT/bar-002-listening-bar.png" "$RUN_OUT/bar-002-transcribing-bar.png" || failures+="listening and transcribing look the same; "
wait_until 240 bar_is result || failures+="the bar never showed result; "
shot_bar bar-002-result
result_state=$(bar .state)
result_luma=$(luma "$RUN_OUT/bar-002-result-bar.png")
is_dark "$result_luma" || failures+="the result bar is not a dark pill (mean $result_luma); "
wait_until 240 run_ended || failures+="the run did not end; "
wait_until 10 bar_is idle || failures+="the bar did not return to idle; "
flash=$(flash_ms)
[ -n "$flash" ] && [ "$flash" -ge 800 ] && [ "$flash" -le 1800 ] || failures+="the result flash lasted '${flash}' ms ($(overlay_since)); "
long_got=$(gtk_commit)
case " $(norm <<<"$long_got") " in *" lighthouse "*) ;; *) failures+="the long dictation's last word is missing: $long_got; " ;; esac
gtk_clear

# error after silence, and Open history (VAL-BAR-009)
c0=$(count)
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="the silent run did not reach listening; "
paplay --device=vmic "$(pad "$SILENCE_WAV")"
xdotool keyup $HOLD
wait_until 60 bar_is error || failures+="the bar did not show error after silence ($(overlay_since)); "
shot_bar bar-002-error
error_state=$(bar .state)
error_message=$(bar .message)
error_open=$(bar .open_history)
error_red=$(reddish "$RUN_OUT/bar-002-error-bar.png")
[ "$error_red" -gt 0 ] || failures+="the error bar has no error-colored pixels; "
[ "$error_open" = true ] || failures+="the error state has no Open history control; "
[ -n "$(el_center flowbar.open-history)" ] || failures+="the tree has no flowbar.open-history; "
wait_until 10 run_ended
sleep 0.3
[ "$(count)" = $((c0 + 1)) ] || failures+="the silent run made $(($(count) - c0)) rows; "
silent_id=$(newest id)
[ "$(newest status)" = failed ] && [ "$(newest error_code)" = ENGINE_NO_SPEECH ] || failures+="the silent row is $(newest status)/$(newest error_code); "
shot bar-009-before
click_el flowbar.open-history || failures+="no Open history control to click; "
wait_until 10 is '$HC state | jq -r .view' = history || failures+="the view is $($HC state | jq -r .view), not history; "
wait_until 10 is "\$HC state | jq -r '.history.detail.id'" = "$silent_id" || failures+="the History detail shows $($HC state | jq -r '.history.detail.id') instead of $silent_id; "
[ "$($HC state | jq -r '.history.detail.error_code')" = ENGINE_NO_SPEECH ] || failures+="the detail code is $($HC state | jq -r '.history.detail.error_code'); "
[ "$($HC state | jq -r '.history.detail.status')" = failed ] || failures+="the detail status is $($HC state | jq -r '.history.detail.status'); "
sleep 0.5
shot bar-009-history
wait_until 10 bar_is idle || failures+="the bar did not return to idle after Open history; "
nothing=$(gtk_commit)
[ -z "$nothing" ] || failures+="the entry holds '$nothing' after a silent run; "
park
check_failures "VAL-BAR-009 Open history" "the error bar offered Open history ('$error_message'); the click showed History with the failed row $silent_id (ENGINE_NO_SPEECH); the entry stayed empty" "$failures"

# blocked: a keyboard grab holds while the run ends (VAL-CROSS-010)
failures2=""
gtk_clear
set_clip OLD
focus "$GT"
python3 /harness/slot/grab-keyboard.py >"$RUN_OUT/grab-helper.log" 2>&1 &
GRAB_PID=$!
wait_until 10 grep -q 'grab status 0' "$RUN_OUT/grab-helper.log" || failures2+="the grab helper did not get the grab; "
hook_hold_start || failures2+="the grab run did not reach listening; "
speech "$SHORT_WAV"
hook_hold_stop
wait_until 120 bar_is blocked || failures2+="the bar did not show blocked ($(overlay_since)); "
shot_bar bar-002-blocked
blocked_state=$(bar .state)
blocked_message=$(bar .message)
blocked_red=$(reddish "$RUN_OUT/bar-002-blocked-bar.png")
[ "$blocked_red" -gt 0 ] || failures2+="the blocked bar has no notice-colored pixels; "
wait_until 60 run_ended || failures2+="the grab run did not end; "
blocked_text=$(newest final_text)
[ "$(newest insert_outcome)" = blocked_grab ] || failures2+="the row's insert_outcome is '$(newest insert_outcome)'; "
[ -n "$blocked_text" ] || failures2+="the blocked row has no final_text; "
[ "$(li .code)" = INSERT_KEYBOARD_GRABBED ] || failures2+="the receipt code is '$(li .code)'; "
[ "$(clip)" = OLD ] || failures2+="the clipboard reads '$(clip)' after the grab; "
kill "$GRAB_PID" 2>/dev/null
wait "$GRAB_PID" 2>/dev/null
case "$blocked_message" in *eyboard*) ;; *) failures2+="the bar says '$blocked_message', not the keyboard-grab notice; " ;; esac
none=$(gtk_commit)
[ -z "$none" ] || failures2+="the entry holds '$none' after the blocked run; "
gtk_clear
focus "$GT"
xdotool key ctrl+alt+v
wait_until 10 is 'li .outcome' = pasted
sleep 0.5
pasted=$(gtk_commit)
[ "$pasted" = "$blocked_text" ] || failures2+="Paste last put '$pasted' in the entry instead of '$blocked_text'; "
wait_until 5 is clip = OLD || failures2+="the clipboard reads '$(clip)' after Paste last; "
check_failures "VAL-CROSS-010 blocked insert" "bar notice '$blocked_message', row blocked_grab with '$blocked_text', clipboard OLD, Paste last gave the entry '$pasted' and the clipboard is OLD again" "$failures2"
wait_until 15 bar_is idle
check_failures "VAL-BAR-002 each state" "idle '$idle_state', listening '$listen_state', transcribing '$trans_state', result '$result_state', error '$error_state', blocked '$blocked_state'; the result flash lasted $flash ms; luma idle $idle_luma listening $listen_luma transcribing $trans_luma result $result_luma; error pixels $error_red, notice pixels $blocked_red" "$failures"

echo "== VAL-BAR-003 the waveform follows the level"
failures=""
hold_start || failures+="the run did not reach listening; "
wait_until 5 bar_is listening || failures+="the bar did not show listening; "
sleep 0.5
shot_bar bar-003-silent
sleep 0.3
shot_bar bar-003-silent-2
paplay --device=vmic "$SHORT_WAV" &
PLAY=$!
sleep 1.0
shot_bar bar-003-speech-a
sleep 0.3
shot_bar bar-003-speech-b
wait "$PLAY"
hold_stop
wait_until 120 run_ended
sleep 0.3
differ "$RUN_OUT/bar-003-speech-a-bar.png" "$RUN_OUT/bar-003-speech-b-bar.png" || failures+="the two speech shots are the same; "
differ "$RUN_OUT/bar-003-silent-bar.png" "$RUN_OUT/bar-003-speech-a-bar.png" || failures+="the silent shot equals a speech shot; "
same_pixels "$RUN_OUT/bar-003-silent-bar.png" "$RUN_OUT/bar-003-silent-2-bar.png" || failures+="the bars move while nothing plays; "
flat=$(inner_extent "$RUN_OUT/bar-003-silent-bar.png")
tall_a=$(inner_extent "$RUN_OUT/bar-003-speech-a-bar.png")
tall_b=$(inner_extent "$RUN_OUT/bar-003-speech-b-bar.png")
tall=$((tall_a > tall_b ? tall_a : tall_b))
[ "$flat" -lt "$tall" ] || failures+="the silent bars are $flat px high but speech bars reach $tall px; "
check_failures "VAL-BAR-003 waveform" "silent bars $flat px and steady, speech bars $tall_a and $tall_b px and different from each other" "$failures"
gtk_clear

echo "== VAL-BAR-004 a click starts and stops a dictation"
failures=""
gtk_clear
F0=$(fid)
park
wait_until 5 bar_is idle || failures+="the bar is not idle; "
T0=$(now_ms)
click_el flowbar.bar || failures+="no bar to click; "
wait_until 5 bar_is listening || failures+="the first click did not start listening (bar is $(bar .state)); "
wait_until 5 pstate_is listening || failures+="the pipeline is not listening; "
[ "$(fid)" = "$F0" ] || failures+="focus moved to $(fid) after the first click; "
shot_bar bar-004-listening
speech "$SHORT_WAV"
click_el flowbar.bar || failures+="no bar to click the second time; "
[ "$(fid)" = "$F0" ] || failures+="focus moved to $(fid) after the second click; "
wait_until 5 is pstate != listening || failures+="the second click did not end listening; "
wait_until 120 run_ended || failures+="the run did not end; "
got=$(gtk_commit)
[ -z "$(missing_words "$(norm <<<"$got")" "${SHORT_WORDS[@]}")" ] || failures+="the entry holds '$got'; "
click_events=$($HC events | jq -r --argjson t0 "$T0" '[.[] | select(.t_ms >= $t0 and (.kind == "pipeline" or .kind == "overlay")) | "\(.kind):\(.detail)"] | join(",")')
shot bar-004-after
# The cross throws the recording away; the tick puts the words in.
park
wait_until 10 bar_is idle
gtk_clear
rows0=$(count)
T0=$(now_ms)
click_el flowbar.bar || failures+="no bar to click for the cross run; "
wait_until 5 bar_is listening || failures+="the cross run did not start ($(bar .state)); "
speech "$SHORT_WAV"
shot_bar bar-004-cross
click_el flowbar.cancel || failures+="no cross to click; "
wait_until 10 run_ended || failures+="the cross did not end the run ($(pevents)); "
case ",$(pevents)," in *,cancelled,*) ;; *) failures+="the cross gave '$(pevents)', not cancelled; " ;; esac
[ "$(count)" = "$rows0" ] || failures+="the cross saved $(($(count) - rows0)) rows; "
[ "$(fid)" = "$F0" ] || failures+="focus moved to $(fid) after the cross; "
crossed=$(gtk_commit)
[ -z "$crossed" ] || failures+="the entry holds '$crossed' after the cross; "
park
wait_until 10 bar_is idle
T0=$(now_ms)
click_el flowbar.bar || failures+="no bar to click for the tick run; "
wait_until 5 bar_is listening || failures+="the tick run did not start ($(bar .state)); "
speech "$SHORT_WAV"
click_el flowbar.finish || failures+="no tick to click; "
wait_until 120 run_ended || failures+="the tick run did not end; "
[ "$(fid)" = "$F0" ] || failures+="focus moved to $(fid) after the tick; "
ticked=$(gtk_commit)
[ -z "$(missing_words "$(norm <<<"$ticked")" "${SHORT_WORDS[@]}")" ] || failures+="the entry holds '$ticked' after the tick; "
check_failures "VAL-BAR-004 click to dictate" "click one gave listening, click two ended it, the entry holds '$got', focus stayed $F0 ($click_events); the cross cancelled with no row and an empty entry; the tick inserted '$ticked'" "$failures"
park
wait_until 10 bar_is idle

echo "== VAL-BAR-005 the bar has no language control"
failures=""
gtk_clear
for state in idle listening; do
  if [ "$state" = listening ]; then
    hold_start || failures+="the run did not reach listening; "
    wait_until 5 bar_is listening
  fi
  langs=$($HC tree | jq '[.[] | select(.id | startswith("flowbar.language"))] | length')
  [ "$langs" = 0 ] || failures+="the $state bar has $langs language elements; "
  [ "$(bar .language)" = null ] || failures+="the overlay state still reports a language; "
done
shot_bar bar-005-listening
hold_stop
wait_until 120 run_ended
check_failures "VAL-BAR-005 no language control" "no flowbar.language element at idle or while listening; the language is set on Home (VAL-HOME-006)" "$failures"
wait_until 20 bar_is idle
gtk_clear

echo "== VAL-BAR-007 drag, restart, and the Settings position"
failures=""
gtk_clear
F0=$(fid)
wait_until 5 bar_is idle
read -r bw bh bx by < <(bar_box)
read -r px py < <(el_center flowbar.bar)
rows0=$(count)
T0=$(now_ms)
xdotool mousemove "$px" "$py"
sleep 0.5
xdotool mousedown 1
sleep 0.2
for step in 1 2 3 4 5 6 7 8 9 10; do
  xdotool mousemove $((px - 30 * step)) $((py - 20 * step))
  sleep 0.08
done
sleep 0.2
xdotool mouseup 1
sleep 0.6
read -r bw1 bh1 bx1 by1 < <(bar_box)
dx=$((bx1 - bx))
dy=$((by1 - by))
[ "${dx#-}" -ge 296 ] && [ "${dx#-}" -le 304 ] && [ "$dx" -lt 0 ] || failures+="the bar moved by $dx in x, not -300; "
[ "${dy#-}" -ge 196 ] && [ "${dy#-}" -le 204 ] && [ "$dy" -lt 0 ] || failures+="the bar moved by $dy in y, not -200; "
[ "$(fid)" = "$F0" ] || failures+="focus is $(fid) after the drag, not $F0; "
[ "$(count)" = "$rows0" ] && ! pevents | grep -q listening || failures+="the drag started a dictation ($(pevents)); "
wait_until 5 is 'setting overlay.customPos' != null || failures+="overlay.customPos was not written; "
saved=$(setting overlay.customPos)
shot_bar bar-007-dragged
park
/harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle
wait_until 20 bar_visible true || failures+="the bar did not map after the restart; "
sleep 1
read -r bw2 bh2 bx2 by2 < <(bar_box)
[ "${bx2#-}" -ge $((bx1 - 4)) ] && [ "$bx2" -le $((bx1 + 4)) ] && [ "$by2" -ge $((by1 - 4)) ] && [ "$by2" -le $((by1 + 4)) ] || failures+="after the restart the bar is at $bx2,$by2 instead of $bx1,$by1; "
shot_bar bar-007-restarted
set_setting overlay.position '"top"'
wait_until 5 is 'setting overlay.customPos' = null || failures+="a Settings position left overlay.customPos at $(setting overlay.customPos); "
wait_until 5 is 'bar_top' -lt 200 || failures+="the bar did not go to the top preset ($(bar_box)); "
shot_bar bar-007-preset
set_setting overlay.position '"bottom"'
wait_until 5 is 'bar_box' = "$bw $bh $bx $by" || failures+="the bottom preset is $(bar_box), the default was $bw $bh $bx $by; "
check_failures "VAL-BAR-007 drag" "moved by $dx,$dy (saved $saved), no dictation, focus stayed $F0; restart gave $bx2,$by2 (was $bx1,$by1); the Settings position cleared customPos and moved the bar" "$failures"

echo "== VAL-BAR-008 the bar stays above other windows"
failures=""
read -r bw bh bx by < <(bar_box)
xterm -T cover -geometry 80x12+$((bx - 120))+$((by - 120)) -bg white -fg black -e sleep 600 >/logs/cover.log 2>&1 &
COVER_PID=$!
CW=$(timeout 10 xdotool search --sync --onlyvisible --name '^cover$' | head -1)
[ -n "$CW" ] || failures+="the cover xterm did not map; "
xdotool windowraise "$CW"
sleep 1
BW=$(bar_wid)
[ -n "$BW" ] || failures+="no X window has the bar's geometry; "
python3 /harness/slot/xstack.py above "$BW" "$CW" || failures+="the idle bar is below the xterm; "
shot_bar bar-008-idle
idle_dark=$(luma "$RUN_OUT/bar-008-idle-bar.png")
is_dark "$idle_dark" || failures+="the bar does not show over the xterm at idle (mean $idle_dark); "
xdotool windowraise "$CW"
hold_start || failures+="the run did not reach listening; "
wait_until 5 bar_is listening
sleep 0.5
xdotool windowraise "$CW"
sleep 0.5
python3 /harness/slot/xstack.py above "$BW" "$CW" || failures+="the listening bar is below the xterm; "
shot_bar bar-008-listening
listen_dark=$(luma "$RUN_OUT/bar-008-listening-bar.png")
is_dark "$listen_dark" || failures+="the bar does not show over the xterm while listening (mean $listen_dark); "
hold_stop
wait_until 120 run_ended
kill "$COVER_PID" 2>/dev/null
check_failures "VAL-BAR-008 on top" "the bar window $BW is above the xterm $CW at idle and while listening; bar mean luma $idle_dark and $listen_dark over a white terminal" "$failures"
wait_until 15 bar_is idle
gtk_clear

echo "== VAL-CROSS-011 on Wayland a bar dictation copies the text and logs it"
failures=""
/harness/slot/app.sh stop
XDG_SESSION_TYPE=wayland /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
wait_until 90 engine_ready_with base-q5_1 || failures+="the engine did not load again; "
wait_until 20 bar_visible true || failures+="the bar did not map on Wayland; "
sleep 1
keys=$($HC state | jq -c .global_keys)
[ "$(jq -r .reason <<<"$keys")" = wayland ] && [ "$(jq -r .available <<<"$keys")" = false ] || failures+="global_keys is $keys; "
focus "$GT"
xdotool key ctrl+a BackSpace
before=$(gtk_commit)
set_clip OLD
park
T0=$(now_ms)
click_el flowbar.bar || failures+="no bar to click; "
wait_until 5 pstate_is listening || failures+="the click did not start listening; "
speech "$SHORT_WAV"
click_el flowbar.bar || failures+="no bar to click the second time; "
wait_until 120 run_ended || failures+="the run did not end; "
shot bar-011-wayland
text=$(dict .transcript)
[ -n "$text" ] && [ "$(clip)" = "$text" ] || failures+="the clipboard reads '$(clip)' but the transcript is '$text'; "
[ -z "$(missing_words "$(norm <<<"$(clip)")" "${SHORT_WORDS[@]}")" ] || failures+="the clipboard misses words: $(clip); "
after=$(gtk_commit)
[ "$after" = "$before" ] || failures+="the entry changed from '$before' to '$after'; "
[ "$(newest status)" = completed ] && [ "$(newest insert_outcome)" = copied_only ] || failures+="the row is $(newest status)/$(newest insert_outcome); "
[ "$(li .outcome)" = copied_only ] || failures+="the receipt is $(lij); "
wayland_notice=$(dict .notice.message)
check_failures "VAL-CROSS-011 Wayland" "global_keys $keys, clipboard holds '$text', the entry stayed '$after', row copied_only, notice '$wayland_notice'" "$failures"

/harness/slot/app.sh stop
start_app
