#!/bin/bash
# Paste last transcript and the cue sounds (VAL-INS-013, 015 to 019), plus the start-latency
# checks of the warm microphone stream. The hold key is Right Alt (X11 keycode 108) and the
# shortcut is Ctrl+Alt+V, both pressed with xdotool; the words come from the virtual microphone.
# Cue sounds are read from `cues.monitor` and `vmic.monitor` with cue-record.py. Busy: it
# transcribes with base. It empties /data and restarts the app, so the slot must not be shared.
# Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
BURST_DB=-40
SILENT_DB=-60

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
event_time() { # <kind> <detail> -> t_ms of the first such event since T0
  $HC events | jq -r --argjson t0 "$T0" --arg k "$1" --arg d "$2" \
    '[.[] | select(.kind == $k and .t_ms >= $t0 and .detail == $d) | .t_ms] | first // empty'
}
# A start that blocks the window (opening the PulseAudio stream on the main thread took about
# 1 s) begins its stall at the key press. The hook records every beat gap over 100 ms, and a slot
# on a busy host with software rendering shows gaps of 100 to 200 ms while it paints the overlay,
# with cues on or off, so only a stall of STALL_LIMIT ms or more that begins within STALL_WINDOW ms
# of T0 counts as a freeze caused by the key press.
STALL_WINDOW=50
STALL_LIMIT=250
# Those stalls since T0, as "a@+x|b@+y" (x is where the stall ended, in ms after T0).
stalls() {
  $HC events | jq -r --argjson t0 "$T0" --argjson w "$STALL_WINDOW" --argjson l "$STALL_LIMIT" \
    '[.[] | select(.kind == "ui-stall" and .t_ms >= $t0)
      | (.detail | rtrimstr("ms") | tonumber) as $gap
      | select($gap >= $l and (.t_ms - $t0 - $gap) < $w)
      | "\(.detail)@+\(.t_ms - $t0)"] | join("|")'
}
timeline() { # every event since T0 as "+ms kind detail", one line each
  $HC events | jq -r --argjson t0 "$T0" \
    '.[] | select(.t_ms >= $t0) | "    +\(.t_ms - $t0) \(.kind) \(.detail)"'
}
li() { $HC state | jq -r ".last_insert$1"; }
lij() { $HC state | jq -c ".last_insert | ${1:-.}"; }
focus() { xdotool windowfocus --sync "$1"; }
class_of() { xprop -id "$1" WM_CLASS 2>/dev/null | grep -o '"[^"]*"' | tail -1 | tr -d '"'; }
speech() { paplay --device=vmic "$(pad "$FIX/$1")"; }
clip_is() { [ "$(clip)" = "$1" ]; }
wait_clip() { wait_until "$2" clip_is "$1"; } # <value> <seconds>
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
pid_alive() { [ -n "$1" ] && [ "$1" != null ] && kill -0 "$1" 2>/dev/null; }
notice_code() { dict .notice.code; }
notice_is() { [ "$(notice_code)" = "$1" ]; }
# A paste receipt written since T0: the receipt of an earlier run does not count.
fresh_paste() { [ "$(li .outcome)" = pasted ] && [ "$(li .ready_unix_ms)" -ge "$T0" ]; }

gtk_commit() { # Enter writes the entry to /out/gtk.txt; prints it
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
paste_last() { xdotool key ctrl+alt+v; }

ptt_run() { # <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  speech "$1"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

# One second of held key with nothing said, and the key released.
silent_hold() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  sleep 1.2
  xdotool keyup $HOLD
  wait_until 60 run_ended
}

cancelled_hold() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  sleep 1
  xdotool key Escape
  xdotool keyup $HOLD
  wait_until 30 run_ended
}

rec_start() { # <name>
  python3 /harness/slot/cue-record.py cues.monitor "$RUN_OUT/$1-cues.tsv" &
  CUE_REC=$!
  python3 /harness/slot/cue-record.py vmic.monitor "$RUN_OUT/$1-vmic.tsv" &
  VMIC_REC=$!
  sleep 0.7
}
rec_stop() {
  sleep 0.7
  kill "$CUE_REC" "$VMIC_REC" 2>/dev/null
  wait "$CUE_REC" "$VMIC_REC" 2>/dev/null
}
peak_in() { # <tsv> <from ms> <to ms> -> the highest peak in dBFS in the window
  awk -F'\t' -v a="$2" -v b="$3" 'BEGIN { m = -120 } $1 >= a && $1 <= b && $2 > m { m = $2 } END { print m }' "$1"
}
peak_all() { awk -F'\t' 'BEGIN { m = -120 } $2 > m { m = $2 } END { print m }' "$1"; }
above() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a > b) }'; }
below() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a < b) }'; }
median() { printf '%s\n' "$@" | sort -n | awk '{ v[NR] = $1 } END { print v[int((NR + 1) / 2)] }'; }

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
T0=$(now_ms)

echo "== VAL-INS-016 paste last with no transcript yet inserts nothing"
failures=""
set_clip OLD
focus "$GT"
T0=$(now_ms)
paste_last
wait_until 5 notice_is INSERT_NO_TRANSCRIPT || failures+="the notice code is '$(notice_code)'; "
shot ins-016
got=$(gtk_commit)
[ -z "$got" ] || failures+="the entry holds '$got'; "
clip_is OLD || failures+="the clipboard reads '$(clip)' instead of OLD; "
case "$(dict .notice.message)" in *"No transcript is available"*) ;; *) failures+="notice says '$(dict .notice.message)'; " ;; esac
case "$(tree_text home.notice)" in *"No transcript is available"*) ;; *) failures+="the screen shows '$(tree_text home.notice)'; " ;; esac
pstate_is idle || failures+="the pipeline is $(pstate); "
[ -z "$(pevents)" ] || failures+="the pipeline moved: $(pevents); "
check_failures "VAL-INS-016 no transcript yet" "entry empty, clipboard OLD, notice '$(dict .notice.message)', pipeline idle" "$failures"

echo "== VAL-INS-015 paste last inserts the last final text and skips cancelled and empty runs"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="the first run did not end; "
first=$(gtk_commit)
first_text=$(dict .transcript)
[ -n "$first" ] && [ "$first" = "$first_text" ] || failures+="the first run put '$first' in the entry, transcript '$first_text'; "
cancelled_hold || failures+="the cancelled run did not end; "
ptt_run silence-5s.wav || failures+="the silent run did not end; "
[ "$(notice_code)" = ENGINE_NO_SPEECH ] || failures+="the silent run left notice '$(notice_code)'; "
gtk_clear
set_clip OLD
focus "$GT"
T0=$(now_ms)
paste_last
wait_until 10 fresh_paste || failures+="no paste receipt: $(lij); "
got=$(gtk_commit)
shot ins-015
[ "$got" = "$first" ] || failures+="the entry holds '$got' instead of '$first'; "
wait_clip OLD 3 || failures+="the clipboard reads '$(clip)' instead of OLD; "
[ -z "$(notice_code | grep -v '^null$')" ] || failures+="a notice is shown: $(notice_code); "
pstate_is idle || failures+="the pipeline is $(pstate); "
check_failures "VAL-INS-015 paste last" "entry '$got' equals the first run, clipboard OLD, receipt $(lij '{outcome,chord}')" "$failures"

echo "== VAL-INS-013 a target that closes during transcription loses no text"
failures=""
set_clip OLD
focus "$GT"
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="no listening state; "
speech dictation-45s.wav
xdotool keyup $HOLD
wait_until 10 pstate_is transcribing || failures+="never reached transcribing; "
xdotool windowclose "$GT"
sleep 0.5
UI_PID=$(ui_pid)
ENGINE_PID=$($HC state | jq -r .engine.pid)
wait_until 240 run_ended || failures+="the run did not end; "
fc=$(class_of "$(xdotool getwindowfocus 2>/dev/null)")
shot ins-013-after-close
[ "$(dict .state)" = done ] || failures+="the run ended $(dict .state); "
pid_alive "$UI_PID" || failures+="the app is gone; "
pid_alive "$ENGINE_PID" || failures+="the engine $ENGINE_PID is gone; "
outcome=$(li .outcome)
case "$outcome" in
  copied_only) ;;
  pasted) [ "$(li .target)" = "$fc" ] || failures+="receipt target '$(li .target)' is not the focused '$fc'; " ;;
  *) failures+="receipt outcome is '$outcome': $(lij); " ;;
esac
long_text=$(dict .transcript)
ids=$(/harness/slot/targets.sh)
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
set_clip OLD
focus "$GT"
T0=$(now_ms)
paste_last
wait_until 10 fresh_paste || failures+="no paste receipt for paste last: $(lij); "
got=$(gtk_commit)
shot ins-013-paste-last
[ "$(norm <<<"$got" | xargs)" = "$(norm <<<"$long_text" | xargs)" ] || failures+="the entry holds '$got' but the transcript is '$long_text'; "
case " $(norm <<<"$got") " in *" lighthouse "*) ;; *) failures+="the last sentence is missing: $got; " ;; esac
[ "$(wc -w <<<"$got")" -ge 60 ] || failures+="only $(wc -w <<<"$got") words; "
wait_clip OLD 3 || failures+="the clipboard reads '$(clip)' instead of OLD; "
check_failures "VAL-INS-013 closed target" "run done, app and engine alive, receipt $outcome, paste last put $(wc -w <<<"$got") words in the new entry" "$failures"

echo "== VAL-INS-017 cues play on the cue sink and can be turned off"
failures=""
set_setting audio.cueSounds true
set_setting audio.cueVolume 0.5
rec_start 017-on
silent_hold || failures+="the held run did not end; "
start_t=$(event_time cue start)
stop_t=$(event_time cue stop)
cancelled_hold || failures+="the cancelled run did not end; "
cancel_t=$(event_time cue cancel)
rec_stop
for cue in start stop cancel; do
  t=$(eval echo "\$${cue}_t")
  if [ -z "$t" ]; then failures+="no $cue cue event; "; continue; fi
  peak=$(peak_in "$RUN_OUT/017-on-cues.tsv" $((t - 100)) $((t + 500)))
  above "$peak" "$BURST_DB" || failures+="no burst after the $cue cue (peak $peak dBFS); "
  echo "  $cue cue at $t: peak $peak dBFS"
done
vmic_peak=$(peak_all "$RUN_OUT/017-on-vmic.tsv")
below "$vmic_peak" "$SILENT_DB" || failures+="the virtual microphone heard a cue (peak $vmic_peak dBFS); "
check_failures "VAL-INS-017 cues on" "bursts after start, stop, and cancel; vmic peak $vmic_peak dBFS" "$failures"

failures=""
set_setting audio.cueSounds false
rec_start 017-off
silent_hold || failures+="the held run did not end; "
cancelled_hold || failures+="the cancelled run did not end; "
rec_stop
off_peak=$(peak_all "$RUN_OUT/017-off-cues.tsv")
below "$off_peak" "$SILENT_DB" || failures+="the cue sink is not silent with cues off (peak $off_peak dBFS); "
check_failures "VAL-INS-017 cues off" "cue sink peak $off_peak dBFS with cues off" "$failures"
set_setting audio.cueSounds true

echo "== VAL-INS-018 the cue volume changes the loudness"
failures=""
peaks=()
for volume in 1.0 0.25; do
  set_setting audio.cueVolume "$volume"
  rec_start "018-$volume"
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || failures+="no listening state at volume $volume; "
  sleep 1.5
  t=$(event_time cue start)
  xdotool keyup $HOLD
  wait_until 60 run_ended
  rec_stop
  [ -n "$t" ] || { failures+="no start cue event at volume $volume; "; peaks+=(-120); continue; }
  peak=$(peak_in "$RUN_OUT/018-$volume-cues.tsv" $((t - 100)) $((t + 700)))
  peaks+=("$peak")
  echo "  volume $volume: start cue peak $peak dBFS"
done
if [ "${#peaks[@]}" = 2 ]; then
  diff=$(awk -v a="${peaks[0]}" -v b="${peaks[1]}" 'BEGIN { printf "%.1f", a - b }')
  awk -v d="$diff" 'BEGIN { exit !(d >= 6) }' || failures+="the peak is only $diff dB lower at 0.25; "
fi
set_setting audio.cueVolume 0.5
check_failures "VAL-INS-018 cue volume" "peak ${peaks[0]} dBFS at 1.0 and ${peaks[1]} dBFS at 0.25" "$failures"

echo "== VAL-INS-019 cue sounds do not delay the capture start"
failures=""
gaps_on=()
gaps_off=()
for mode in on off; do
  if [ "$mode" = on ]; then set_setting audio.cueSounds true; else set_setting audio.cueSounds false; fi
  for run in 1 2 3 4 5; do
    T0=$(now_ms)
    xdotool keydown $HOLD
    wait_until 5 pstate_is listening || failures+="$mode run $run: no listening state; "
    sleep 0.7
    xdotool keyup $HOLD
    wait_until 60 run_ended
    open=$(event_time capture mic-open)
    if [ -z "$open" ]; then failures+="$mode run $run: no mic-open event; "; continue; fi
    gap=$((open - T0))
    echo "  cues $mode run $run: key down to capture start $gap ms"
    if [ "$mode" = on ]; then gaps_on+=("$gap"); else gaps_off+=("$gap"); fi
    s=$(stalls)
    if [ -n "$s" ]; then
      failures+="$mode run $run: the window stalled ($s); "
      timeline
    fi
    sleep 0.3
  done
done
set_setting audio.cueSounds true
if [ "${#gaps_on[@]}" = 5 ] && [ "${#gaps_off[@]}" = 5 ]; then
  med_on=$(median "${gaps_on[@]}")
  med_off=$(median "${gaps_off[@]}")
  [ $((med_on - med_off)) -le 50 ] || failures+="the median gap is $med_on ms with cues and $med_off ms without; "
else
  med_on="?"
  med_off="?"
  failures+="not every run has a gap; "
fi
check_failures "VAL-INS-019 cues and capture start" "median key down to capture start $med_on ms with cues, $med_off ms without" "$failures"

echo "== start latency: speech 100 ms after the key keeps its first word, and the window never stalls"
failures=""
set_clip OLD
focus "$GT"
ptt_run speech-short.wav || failures+="the padded baseline run did not end; "
base_text=$(dict .transcript)
sleep 0.5
T0=$(now_ms)
xdotool keydown $HOLD
sleep 0.1
paplay --device=vmic "$FIX/speech-short.wav"
xdotool keyup $HOLD
wait_until 240 run_ended || failures+="the early-speech run did not end; "
early_text=$(dict .transcript)
echo "  baseline: $base_text"
echo "  early:    $early_text"
case "$(norm <<<"$early_text" | xargs)" in "the quick"*) ;; *) failures+="the early transcript does not start with 'the quick': $early_text; " ;; esac
[ -z "$(missing_words "$(norm <<<"$early_text")" "${SHORT_WORDS[@]}")" ] || failures+="the early transcript misses words: $early_text; "
s=$(stalls)
[ -z "$s" ] || failures+="the window stalled during the run ($s); "
check_failures "start latency first word" "early transcript '$early_text'" "$failures"
