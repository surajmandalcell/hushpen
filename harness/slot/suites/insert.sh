#!/bin/bash
# Insertion checks (VAL-INS-001 to 010, VAL-PTT-014). The hold key is Right Alt (X11 keycode 108),
# pressed with xdotool while a paste target has the focus; the words come from the virtual
# microphone. Targets: a GTK entry (Ctrl+V), stock xterm (Shift+Insert with PRIMARY), xterm with
# a Ctrl+Shift+V translation, and a window that never reads the clipboard. Busy: it transcribes
# with base. It empties /data and restarts the app, so the slot must not be shared. Runs in one
# slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
STOCK_FILE=/out/xterm-stock.txt
OVERRIDE_FILE=/out/xterm.txt

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() { # the pipeline states since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
pevent_time() { # <state> -> t_ms of its first pipeline event since T0
  $HC events | jq -r --argjson t0 "$T0" --arg d "$1" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0 and .detail == $d) | .t_ms] | first // empty'
}
insert_time() { # <detail prefix> -> t_ms of the first insert event since T0 that starts with it
  $HC events | jq -r --argjson t0 "$T0" --arg p "$1" \
    '[.[] | select(.kind == "insert" and .t_ms >= $t0 and (.detail | startswith($p))) | .t_ms] | first // empty'
}
chord_seen() { [ -n "$(insert_time chord)" ]; }
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
li() { $HC state | jq -r ".last_insert$1"; }
lij() { $HC state | jq -c ".last_insert | ${1:-.}"; }
focus() { xdotool windowfocus --sync "$1"; }
focus_is() { [ "$(xdotool getwindowfocus)" = "$1" ]; }
class_of() { xprop -id "$1" WM_CLASS | grep -o '"[^"]*"' | tail -1 | tr -d '"'; }
speech() { paplay --device=vmic "$(pad "$FIX/$1")"; }
clip_is() { [ "$(clip)" = "$1" ]; }
wait_clip() { wait_until "$2" clip_is "$1"; } # <value> <seconds>
html_is_old() { [ "$(xclip -selection clipboard -t text/html -o 2>/dev/null)" = '<b>OLD</b>' ]; }

# gtk_commit: Enter in the GTK entry writes what it holds to /out/gtk.txt; prints it.
gtk_commit() {
  rm -f "$GTK_FILE"
  focus "$GT"
  xdotool key Return
  wait_until 5 test -e "$GTK_FILE"
  cat "$GTK_FILE"
}

# stock_new <size before> -> the text xterm wrote since, without the final newline
file_new() { # <file> <size before>
  tail -c +$(($2 + 1)) "$1" 2>/dev/null | tr -d '\n'
}
file_size() { stat -c %s "$1" 2>/dev/null || echo 0; }

# ptt_run <wav>: hold the key for the whole clip and release; waits until the run has ended.
ptt_run() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  speech "$1"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

# ptt_to_chord <wav>: like ptt_run, but returns as soon as the paste key has been sent
ptt_to_chord() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  speech "$1"
  xdotool keyup $HOLD
  wait_until 240 chord_seen
}

sentinel_ok() { [ "$(clip)" = OLD ]; }

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh --stock)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
XT=$(grep -o 'xterm=[0-9]*' <<<"$ids" | cut -d= -f2)
ST=$(grep -o 'xterm_stock=[0-9]*' <<<"$ids" | cut -d= -f2)
GTK_CLASS=$(class_of "$GT")
python3 /harness/slot/target-idle.py >/logs/idle-target.log 2>&1 &
IDLE=$(timeout 10 xdotool search --sync --onlyvisible --name hushpen-idle-target | head -1)
sleep 1

echo "== VAL-INS-001 push-to-talk inserts the spoken words into the GTK entry"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="the run did not end; "
shot ins-001
text=$(dict .transcript)
got=$(gtk_commit)
events=$(pevents)
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="the entry holds '$got' but the transcript is '$text'; "
LC_ALL=C grep -q '[[:cntrl:]]' <<<"$got" && failures+="the entry text has control characters; "
[ -z "$(missing_words "$(norm <<<"$got")" "${SHORT_WORDS[@]}")" ] || failures+="the entry misses words: $got; "
[ "$events" = "listening,transcribing,cleaning,inserting,done,idle" ] || failures+="events were '$events'; "
[ "$(li .outcome)" = pasted ] && [ "$(li .chord)" = ctrl+v ] || failures+="receipt is $(lij); "
[ "$(li .target)" = "$GTK_CLASS" ] || failures+="receipt target '$(li .target)' is not '$GTK_CLASS'; "
check_failures "VAL-INS-001 GTK entry" "entry '$got', events $events, receipt $(lij '{outcome,chord,target}')" "$failures"

echo "== VAL-INS-002 stock xterm gets the text through Shift+Insert with PRIMARY"
failures=""
set_clip OLD
size=$(file_size "$STOCK_FILE")
focus "$ST"
ptt_run speech-short.wav || failures+="the run did not end; "
text=$(dict .transcript)
sleep 0.3
xdotool key Return
sleep 0.5
shot ins-002
got=$(file_new "$STOCK_FILE" "$size")
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="xterm got '$got' but the transcript is '$text'; "
[ "$(li .chord)" = shift+insert ] && [ "$(li .selection)" = primary ] || failures+="receipt is $(lij); "
wait_clip OLD 3 || failures+="the clipboard reads '$(clip)' instead of OLD; "
check_failures "VAL-INS-002 stock xterm" "xterm got '$got', receipt $(lij '{outcome,chord,selection}'), clipboard OLD" "$failures"

echo "== VAL-INS-003 a per-app override changes the paste chord"
failures=""
/harness/slot/app.sh stop
jq '.values["insert.appChords"] = {"xterm": "ctrl+shift+v"}' "$SETTINGS" >"$SETTINGS.tmp" && cat "$SETTINGS.tmp" >"$SETTINGS"
cp "$SETTINGS" "$RUN_OUT/003-settings.json"
start_app
wait_until 90 engine_ready_with base || failures+="the engine did not load again; "
[ "$($HC state | jq -c '.settings["insert.appChords"]')" = '{"xterm":"ctrl+shift+v"}' ] || failures+="the setting is $($HC state | jq -c '.settings["insert.appChords"]'); "
set_clip OLD
size=$(file_size "$OVERRIDE_FILE")
focus "$XT"
ptt_run speech-short.wav || failures+="the run did not end; "
text=$(dict .transcript)
sleep 0.3
xdotool key Return
sleep 0.5
shot ins-003
got=$(file_new "$OVERRIDE_FILE" "$size")
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="xterm got '$got' but the transcript is '$text'; "
[ "$(li .chord)" = ctrl+shift+v ] || failures+="receipt is $(lij); "
wait_clip OLD 3 || failures+="the clipboard reads '$(clip)' instead of OLD; "
check_failures "VAL-INS-003 per-app chord" "settings.json holds the override, receipt $(lij '{outcome,chord,selection}'), xterm got '$got', clipboard OLD" "$failures"

echo "== VAL-INS-004 the text clipboard is restored, and an empty clipboard stays empty"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="(a) the run did not end; "
t_done=$(now_ms)
wait_clip OLD 3 || failures+="(a) the clipboard reads '$(clip)' instead of OLD; "
restored_after=$(($(now_ms) - t_done))
gtk_commit >/dev/null
xclip -selection clipboard -i /dev/null
sleep 0.3
focus "$GT"
ptt_run speech-short.wav || failures+="(b) the run did not end; "
text=$(dict .transcript)
sleep 3
after=$(clip)
[ -z "$after" ] || failures+="(b) the clipboard holds '$after' 3 s after done; "
case "$after" in *"$text"* | *quick*) failures+="(b) the clipboard holds the transcript; " ;; esac
gtk_commit >/dev/null
check_failures "VAL-INS-004 text restore" "(a) OLD was back ${restored_after} ms after done; (b) the empty clipboard did not get the transcript" "$failures"

echo "== VAL-INS-005 an image or rich text clipboard is restored with its formats"
failures=""
convert -size 64x64 gradient:red-blue "$RUN_OUT/005-in.png"
xclip -selection clipboard -t image/png -i "$RUN_OUT/005-in.png"
sleep 0.3
focus "$GT"
ptt_run speech-short.wav || failures+="(a) the run did not end; "
sleep 3
targets=$(xclip -selection clipboard -t TARGETS -o 2>&1)
case "$targets" in *image/png*) ;; *) failures+="(a) TARGETS is '$targets'; " ;; esac
xclip -selection clipboard -t image/png -o >"$RUN_OUT/005-out.png" 2>/dev/null
diff=$(compare -metric AE "$RUN_OUT/005-in.png" "$RUN_OUT/005-out.png" null: 2>&1)
[ "$diff" = 0 ] || failures+="(a) the restored image differs from the original (AE $diff); "
gtk_commit >/dev/null
python3 /harness/slot/clip-multi.py >/logs/clip-multi.log 2>&1 &
multi=$!
wait_until 5 html_is_old || failures+="(b) the rich text sentinel was not set; "
focus "$GT"
ptt_run speech-short.wav || failures+="(b) the run did not end; "
sleep 3
html=$(xclip -selection clipboard -t text/html -o 2>&1)
plain=$(xclip -selection clipboard -t UTF8_STRING -o 2>&1)
[ "$html" = '<b>OLD</b>' ] || failures+="(b) text/html reads '$html'; "
[ "$plain" = OLD ] || failures+="(b) UTF8_STRING reads '$plain'; "
kill "$multi" 2>/dev/null
gtk_commit >/dev/null
check_failures "VAL-INS-005 rich clipboard" "(a) image/png back with AE $diff; (b) html '$html' and plain '$plain' back" "$failures"

echo "== VAL-INS-006 with no paste receipt, the clipboard is restored at the 2 s cap"
failures=""
set_clip OLD
focus "$IDLE"
ptt_to_chord speech-short.wav || failures+="no paste key was sent; "
t_chord=$(insert_time chord)
restored=""
while [ $(($(now_ms) - t_chord)) -lt 6000 ]; do
  if [ "$(clip)" = OLD ]; then restored=$(now_ms); break; fi
  sleep 0.1
done
wait_until 30 run_ended || failures+="the pipeline did not return to idle; "
shot ins-006
delay=""
if [ -z "$restored" ]; then failures+="OLD never came back; "; else
  delay=$((restored - t_chord))
  { [ "$delay" -ge 1950 ] && [ "$delay" -le 3500 ]; } || failures+="the clipboard came back ${delay} ms after the paste key; "
fi
[ "$(li .outcome)" = failed ] && [ "$(li .code)" = INSERT_NO_RECEIPT ] || failures+="receipt is $(lij); "
notice=$($HC tree | jq -r '.[] | select(.id == "home.notice") | .text')
case "$notice" in *"Paste last transcript"*) ;; *) failures+="the notice is '$notice'; " ;; esac
check_failures "VAL-INS-006 no receipt" "OLD came back ${delay} ms after the paste key, receipt $(lij '{outcome,code}'), notice '$notice'" "$failures"

echo "== VAL-INS-007 a newer user copy is never overwritten by the restore"
failures=""
set_clip OLD
focus "$IDLE"
ptt_to_chord speech-short.wav || failures+="no paste key was sent; "
t_chord=$(insert_time chord)
while [ $(($(now_ms) - t_chord)) -lt 500 ]; do sleep 0.02; done
printf NEW | xclip -selection clipboard
while [ $(($(now_ms) - t_chord)) -lt 5000 ]; do sleep 0.1; done
after=$(clip)
[ "$after" = NEW ] || failures+="5 s after the paste key the clipboard reads '$after'; "
wait_until 30 run_ended || failures+="the pipeline did not return to idle; "
[ "$(li .restore)" = skipped_newer_copy ] || failures+="restore is '$(li .restore)'; "
check_failures "VAL-INS-007 newer copy" "the clipboard still read NEW 5 s after the paste key, restore $(li .restore)" "$failures"

echo "== VAL-INS-008 focus stays on the target and the Hushpen window is not raised"
failures=""
set_clip OLD
focus "$GT"
xdotool windowraise "$GT"
HUSHPEN_WID=$(app_window)
active() { printf '%d' "$(xprop -root _NET_ACTIVE_WINDOW | awk '{print $NF}')"; }
stack_rel() { # "above" when the Hushpen window is above the GTK target in the stacking order
  xprop -root _NET_CLIENT_LIST_STACKING | sed 's/.*# //' | tr ',' '\n' | tr -d ' ' |
    while read -r id; do printf '%d\n' "$id"; done |
    awk -v h="$HUSHPEN_WID" -v g="$GT" '{ if ($1 == h) ph = NR; if ($1 == g) pg = NR }
      END { print (ph > pg) ? "above" : "below" }'
}
before_active=$(active)
before_rel=$(stack_rel)
focus_is "$GT" || failures+="the GTK target did not have the focus at the start; "
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
speech speech-short.wav
xdotool keyup $HOLD
wait_until 5 pstate_is transcribing
focus_is "$GT" || failures+="the focus left the target while transcribing ($(xdotool getwindowfocus)); "
[ "$(active)" = "$before_active" ] || failures+="the active window changed while transcribing; "
wait_until 240 run_ended
shot ins-008
focus_is "$GT" || failures+="the focus left the target after done ($(xdotool getwindowfocus)); "
[ "$(active)" = "$before_active" ] || failures+="the active window changed after done ($(active), was $before_active); "
after_rel=$(stack_rel)
[ "$after_rel" = "$before_rel" ] && [ "$after_rel" = below ] || failures+="the Hushpen window is $after_rel the target (was $before_rel); "
gtk_commit >/dev/null
check_failures "VAL-INS-008 focus" "focus stayed on $GT, active window $before_active, Hushpen window $after_rel the target" "$failures"

echo "== VAL-INS-009 no modifier stays down after insertion"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="the run did not end; "
text=$(dict .transcript)
sleep 0.5
keys=$(xinput query-state "Virtual core XTEST keyboard" 2>&1 | grep -c 'down')
down=$(xinput query-state "Virtual core XTEST keyboard" 2>&1 | grep 'down' | tr '\n' ' ')
[ "$keys" = 0 ] || failures+="keys are down: $down; "
xdotool key 38
sleep 0.3
got=$(gtk_commit)
[ "$got" = "${text}a" ] || failures+="the entry holds '$got' instead of '${text}a'; "
check_failures "VAL-INS-009 no stuck keys" "no key down on the XTEST keyboard, key 38 added exactly one 'a'" "$failures"

echo "== VAL-INS-010 paste happens within 300 ms after the text is ready"
failures=""
gaps=()
worst=0
for run in 1 2 3 4 5; do
  set_clip OLD
  focus "$GT"
  ptt_run speech-short.wav || failures+="run $run did not end; "
  inserting=$(pevent_time inserting)
  chord=$(insert_time chord)
  if [ -z "$inserting" ] || [ -z "$chord" ]; then failures+="run $run: inserting '$inserting', chord '$chord'; "; continue; fi
  gap=$((chord - inserting))
  gaps+=("$gap")
  [ "$gap" -le "$worst" ] || worst=$gap
  gtk_commit >/dev/null
  sleep 1
done
[ "${#gaps[@]}" = 5 ] || failures+="only ${#gaps[@]} of 5 runs have a gap; "
[ "$worst" -le 300 ] || failures+="the largest gap is $worst ms; "
check_failures "VAL-INS-010 paste latency" "gaps ${gaps[*]} ms, largest $worst ms" "$failures"

echo "== VAL-PTT-014 an engine crash or no speech during a hotkey dictation inserts nothing"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="the first run did not end; "
first_text=$(dict .transcript)
[ "$(dict .state)" = done ] || failures+="the first run ended $(dict .state); "
set_clip OLD
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
speech dictation-45s.wav
xdotool keyup $HOLD
wait_until 10 pstate_is transcribing || failures+="(a) never reached transcribing; "
pid_before=$($HC state | jq -r .engine.pid)
kill -9 "$pid_before" 2>/dev/null || failures+="(a) could not kill the engine $pid_before; "
wait_until 60 run_ended || failures+="(a) the run did not end; "
shot ptt-014-a
case ",$(pevents)," in *,failed,*) ;; *) failures+="(a) events were '$(pevents)'; " ;; esac
[ "$(dict .notice.code)" = ENGINE_CRASHED ] || failures+="(a) notice is '$(dict .notice.code)'; "
sentinel_ok || failures+="(a) the clipboard reads '$(clip)'; "
sleep 1
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening
speech silence-5s.wav
xdotool keyup $HOLD
wait_until 120 run_ended || failures+="(b) the run did not end; "
shot ptt-014-b
case ",$(pevents)," in *,failed,*) ;; *) failures+="(b) events were '$(pevents)'; " ;; esac
[ "$(dict .notice.code)" = ENGINE_NO_SPEECH ] || failures+="(b) notice is '$(dict .notice.code)'; "
sentinel_ok || failures+="(b) the clipboard reads '$(clip)'; "
focus "$GT"
got=$(gtk_commit)
[ "$got" = "$first_text" ] || failures+="the entry holds '$got' instead of only the first transcript '$first_text'; "
pid_after=$($HC state | jq -r .engine.pid)
sleep 1
ptt_run speech-short.wav || failures+="the next run did not end; "
[ "$(dict .state)" = done ] || failures+="the next run ended $(dict .state) ($(dict .notice.code), pipeline events '$(pevents)'); "
next_text=$(dict .transcript)
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$next_text" ] || failures+="the next run put '$got' into the entry, the transcript is '$next_text'; "
pid_after=$($HC state | jq -r .engine.pid)
[ "$pid_after" != "$pid_before" ] || failures+="the engine pid did not change ($pid_after); "
check_failures "VAL-PTT-014 failed runs insert nothing" "engine $pid_before killed (ENGINE_CRASHED), silence gave ENGINE_NO_SPEECH, the entry kept only '$first_text', clipboard OLD, the next run said '$next_text' with engine $pid_after" "$failures"
