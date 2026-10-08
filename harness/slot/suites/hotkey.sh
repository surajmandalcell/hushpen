#!/bin/bash
# Hold-key checks (VAL-PTT-005 to 012, VAL-GRD-006, VAL-GRD-008). The real hold key is Right Alt
# (X11 keycode 108), pressed with xdotool while a paste target has the focus. Busy: it plays
# speech into the virtual microphone and transcribes with base. It empties /data and restarts
# the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
ESC=Escape
XTERM_FILE=/out/xterm-stock.txt

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pmode_is() { [ "$($HC state | jq -r .pipeline.mode)" = "$1" ]; }
pevents() { # the pipeline states since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
pevent_time() { # <state> -> t_ms of its first pipeline event since T0
  $HC events | jq -r --argjson t0 "$T0" --arg d "$1" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0 and .detail == $d) | .t_ms] | first // empty'
}
pevent_seen() { [ -n "$(pevent_time "$1")" ]; }
cue_count() { $HC events | jq -r --argjson t0 "$T0" '[.[] | select(.kind == "cue" and .t_ms >= $t0)] | length'; }
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
sessions() { ls "$SESSIONS" 2>/dev/null | wc -l; }
focus() { xdotool windowfocus --sync "$1"; }
focus_is() { [ "$(xdotool getwindowfocus)" = "$1" ]; }
xterm_size() { stat -c %s "$XTERM_FILE" 2>/dev/null || echo 0; }
xterm_new() { # <size before> -> hex of what xterm wrote since
  tail -c +$(($1 + 1)) "$XTERM_FILE" 2>/dev/null | od -An -v -tx1 | tr -d ' \n'
}
tap() { xdotool keydown $HOLD; sleep 0.1; xdotool keyup $HOLD; }
double_tap() { tap; sleep 0.15; tap; }
speech() { paplay --device=vmic "$(pad "$FIX/$1")"; }

# ptt_run <wav>: hold the key for the whole clip and release; waits until the run has ended.
ptt_run() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || return 1
  speech "$1"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

echo "== VAL-PTT-005 the hold key starts and stops a dictation while another window has focus"
fresh base
use_model base
ids=$(/harness/slot/targets.sh --stock)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
XT=$(grep -o 'xterm_stock=[0-9]*' <<<"$ids" | cut -d= -f2)
set_clip OLD
focus "$GT"
before=$(xdotool getwindowfocus)
T0=$(now_ms)
t_down=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
shot ptt-005-listening
listen_ms=$(($(pevent_time listening) - t_down))
focus_mid=$(xdotool getwindowfocus)
speech speech-short.wav
xdotool keyup $HOLD
wait_until 240 run_ended
shot ptt-005-final
events=$(pevents)
failures=""
[ "$listen_ms" -lt 500 ] || failures+="listening came ${listen_ms} ms after the key went down; "
case "$events" in listening,transcribing,*done,idle) ;; *) failures+="events were '$events'; " ;; esac
[ "$(xdotool getwindowfocus)" = "$before" ] && [ "$focus_mid" = "$before" ] || failures+="the focus moved from $before; "
[ "$before" = "$GT" ] || failures+="the GTK target did not have the focus ($before, $GT); "
text=$(dict .transcript)
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="transcript misses words: $text; "
check_failures "VAL-PTT-005 hold key" "listening ${listen_ms} ms after keydown, events $events, focus stayed on the GTK target, transcript '$text'" "$failures"

echo "== VAL-PTT-006 only the configured hold key counts; repeats and other keys do not break a hold"
failures=""
sleep 1
T0=$(now_ms)
xdotool keydown 64
sleep 2
xdotool keyup 64
events=$(pevents)
[ -z "$events" ] && pstate_is idle || failures+="Left Alt gave events '$events', state $(pstate); "
xset r on
T0=$(now_ms)
xdotool keydown $HOLD
sleep 3
xdotool keyup $HOLD
wait_until 240 run_ended
events=$(pevents)
listening=$(tr ',' '\n' <<<"$events" | grep -c '^listening$')
transcribing=$(tr ',' '\n' <<<"$events" | grep -c '^transcribing$')
[ "$listening" = 1 ] && [ "$transcribing" = 1 ] || failures+="auto-repeat gave events '$events'; "
sleep 1
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
xdotool key a
sleep 0.5
pstate_is listening || failures+="another key during the hold left listening (state $(pstate)); "
xdotool keyup $HOLD
wait_until 240 run_ended
events=$(pevents)
listening=$(tr ',' '\n' <<<"$events" | grep -c '^listening$')
[ "$listening" = 1 ] || failures+="another key during the hold gave events '$events'; "
check_failures "VAL-PTT-006 hold key only" "Left Alt stayed idle, a 3 s hold with auto-repeat gave one listening and one transcribing, key 38 during a hold changed nothing" "$failures"

echo "== VAL-PTT-007 a tap under 250 ms is not a recording"
sleep 1
rm -f /out/gtk.txt
sessions_before=$(sessions)
T0=$(now_ms)
tap
wait_until 1 pstate_is idle
events=$(pevents)
failures=""
case ",$events," in *,transcribing,*) failures+="the tap reached transcribing ($events); " ;; esac
pstate_is idle || failures+="state is $(pstate); "
[ "$(sessions)" = "$sessions_before" ] || failures+="a session file was left behind; "
[ ! -e /out/gtk.txt ] || failures+="the GTK target got text; "
check_failures "VAL-PTT-007 tap" "events '$events', idle again, no session file, nothing inserted" "$failures"

echo "== VAL-PTT-008 a double tap starts hands-free and the next press stops it"
sleep 1
T0=$(now_ms)
double_tap
sleep 3
shot ptt-008-hands-free
failures=""
pstate_is listening || failures+="state is $(pstate) 3 s after the second tap; "
pmode_is handsFree || failures+="mode is $($HC state | jq -r .pipeline.mode); "
speech speech-short.wav
tap
wait_until 5 pevent_seen transcribing || failures+="the next press did not reach transcribing; "
wait_until 240 run_ended
events=$(pevents)
text=$(dict .transcript)
case "$events" in listening,idle,listening,transcribing,*done,idle) ;; *) failures+="events were '$events'; " ;; esac
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="transcript misses words: $text; "
check_failures "VAL-PTT-008 hands-free" "listening in handsFree mode after the double tap, the next press stopped it, events $events, transcript '$text'" "$failures"

echo "== VAL-PTT-009 Esc passes through while idle"
failures=""
focus "$XT"
for round in 1 2; do
  [ "$round" = 1 ] || { sleep 1; ptt_run speech-short.wav; focus "$XT"; }
  size=$(xterm_size)
  T0=$(now_ms)
  xdotool key $ESC
  xdotool key Return
  sleep 0.5
  got=$(xterm_new "$size")
  [ "$got" = "1b0a" ] || failures+="round $round: xterm got '$got' instead of 1b0a; "
  pevent_seen cancelled && failures+="round $round: a cancelled event appeared; "
done
check_failures "VAL-PTT-009 idle Esc" "both idle Esc presses reached xterm as 0x1b and made no pipeline event" "$failures"

echo "== VAL-PTT-010 Esc in listening cancels and discards the audio"
failures=""
for mode in hold hands-free; do
  focus "$XT"
  sleep 1
  size=$(xterm_size)
  files=$(sessions)
  T0=$(now_ms)
  if [ "$mode" = hold ]; then xdotool keydown $HOLD; else double_tap; fi
  wait_until 5 pstate_is listening || failures+="$mode: never listened; "
  t_esc=$(now_ms)
  xdotool key $ESC
  wait_until 2 pevent_seen cancelled || failures+="$mode: no cancelled event; "
  cancelled=$(pevent_time cancelled)
  [ -z "$cancelled" ] || [ $((cancelled - t_esc)) -lt 1000 ] || failures+="$mode: cancelled came $((cancelled - t_esc)) ms after Esc; "
  [ "$mode" != hold ] || xdotool keyup $HOLD
  sleep 1.5
  shot "ptt-010-$mode"
  case ",$(pevents)," in *,transcribing,*) failures+="$mode: a transcription started; " ;; esac
  [ "$(sessions)" = "$files" ] || failures+="$mode: a session file remains; "
  # Alt+Esc is openbox's own window-cycling key, so a hold of Right Alt moves the focus.
  focus "$XT"
  xdotool key Return
  sleep 0.3
  [ "$(xterm_new "$size")" = "0a" ] || failures+="$mode: xterm got '$(xterm_new "$size")' instead of only the Return; "
  wait_until 10 pstate_is idle
done
check_failures "VAL-PTT-010 Esc in listening" "hold and hands-free: cancelled within 1 s, no transcription, no session file, xterm got no Esc" "$failures"

echo "== VAL-PTT-011 Esc in transcribing cancels in under 1 s and the engine stays usable"
failures=""
focus "$GT"
rm -f /out/gtk.txt
set_clip OLD
sleep 1
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
speech dictation-45s.wav
xdotool keyup $HOLD
wait_until 5 pstate_is transcribing || failures+="never reached transcribing; "
t_esc=$(now_ms)
xdotool key $ESC
wait_until 5 pevent_seen cancelled || failures+="no cancelled event; "
cancelled=$(pevent_time cancelled)
[ -n "$cancelled" ] && [ $((cancelled - t_esc)) -lt 1000 ] || failures+="cancelled came '${cancelled:+$((cancelled - t_esc))}' ms after Esc; "
shot ptt-011-cancelled
[ ! -e /out/gtk.txt ] || failures+="the GTK target got text; "
[ "$(clip)" = OLD ] || failures+="the clipboard is '$(clip)'; "
wait_until 10 pstate_is idle
sleep 1
ptt_run speech-short.wav
text=$(dict .transcript)
[ "$(dict .state)" = done ] || failures+="the next run ended $(dict .state); "
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="the next run's transcript misses words: $text; "
check_failures "VAL-PTT-011 Esc in transcribing" "cancelled $((cancelled - t_esc)) ms after Esc, nothing inserted, clipboard OLD, the next run said '$text'" "$failures"

echo "== VAL-PTT-012 a failed preflight keeps the pipeline idle with a notice"
fresh
failures=""
sleep 1
files=$(sessions)
T0=$(now_ms)
xdotool keydown $HOLD
sleep 2
xdotool keyup $HOLD
sleep 0.5
shot ptt-012
[ -z "$(pevents)" ] && pstate_is idle || failures+="events '$(pevents)', state $(pstate); "
[ "$(cue_count)" = 0 ] || failures+="a cue played; "
[ "$(sessions)" = "$files" ] || failures+="a capture started; "
notice=$($HC tree | jq -c '.[] | select(.id == "home.notice")')
case "$notice" in *"speech model"*) ;; *) failures+="the notice is '$notice'; " ;; esac
check_failures "VAL-PTT-012 preflight" "no pipeline event, no cue, no capture, notice '$notice'" "$failures"

echo "== VAL-GRD-006 a Wayland session shows global keys as not available"
/harness/slot/app.sh stop
XDG_SESSION_TYPE=wayland /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
failures=""
sleep 1
keys=$($HC state | jq -c .global_keys)
[ "$(jq -r .reason <<<"$keys")" = wayland ] && [ "$(jq -r .available <<<"$keys")" = false ] || failures+="global_keys is $keys; "
row=$($HC tree | jq -c '.[] | select(.id == "home.keys-notice")')
case "$row" in *"Not available on Wayland"*) ;; *) failures+="the screen has no Wayland notice: $row; " ;; esac
T0=$(now_ms)
xdotool keydown $HOLD
sleep 2
xdotool keyup $HOLD
sleep 0.5
shot grd-006
[ -z "$(pevents)" ] && pstate_is idle || failures+="the hold key moved the pipeline: '$(pevents)'; "
check_failures "VAL-GRD-006 Wayland session" "global_keys $keys, notice on screen, the hold key left the pipeline idle" "$failures"

echo "== VAL-GRD-008 Wayland with no X display does not crash"
/harness/slot/app.sh stop
: >/data/logs/hushpen.log
env -u DISPLAY WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland /app/hushpen >/logs/app-no-display.log 2>&1 &
pid=$!
for _ in $(seq 100); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
failures=""
if kill -0 "$pid" 2>/dev/null; then
  outcome="still running after 10 s"
  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null
else
  wait "$pid"
  code=$?
  outcome="exited with code $code"
  [ "$code" = 0 ] || failures+="exit code $code; "
fi
grep -qi panick /logs/app-no-display.log /data/logs/hushpen.log 2>/dev/null && failures+="the app panicked; "
grep -q 'global keys not available: wayland' /data/logs/hushpen.log /logs/app-no-display.log 2>/dev/null ||
  failures+="the log does not report global keys not available with reason wayland; "
check_failures "VAL-GRD-008 no X display" "$outcome; the log reports global keys not available: wayland" "$failures"
