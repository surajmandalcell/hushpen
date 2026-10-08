#!/bin/bash
# Insertion guards (VAL-GRD-001, 002, 003, 007). A helper holds an X11 keyboard grab while a
# dictation finishes, and a Wayland session (XDG_SESSION_TYPE=wayland with an X display) gets
# copy only. Runs are driven with the pipeline hook events, the same path the hold key feeds,
# because a grab stops key delivery to the target. Busy: it transcribes with base. It empties
# /data and restarts the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

GTK_FILE=/out/gtk.txt

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() { # the pipeline states since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
insert_events() { # the insert events since T0, as "a|b|c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "insert" and .t_ms >= $t0) | .detail] | join("|")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
li() { $HC state | jq -r ".last_insert$1"; }
lij() { $HC state | jq -c ".last_insert | ${1:-.}"; }
focus() { xdotool windowfocus --sync "$1"; }
speech() { paplay --device=vmic "$(pad "$FIX/$1")"; }
clip_is() { [ "$(clip)" = "$1" ]; }

# hook_run <wav>: the hold key's path (hold-down, speech, hold-up) through the hook; waits until
# the run has ended.
hook_run() {
  T0=$(now_ms)
  hook_action pipeline-event '{"event":"hold-down"}' >/dev/null
  wait_until 5 pstate_is listening || {
    hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
    return 1
  }
  speech "$1"
  hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
  wait_until 240 run_ended
}

# gtk_commit: Enter in the GTK entry writes what it holds to /out/gtk.txt; prints it.
gtk_commit() {
  rm -f "$GTK_FILE"
  focus "$GT"
  xdotool key Return
  wait_until 5 test -e "$GTK_FILE"
  cat "$GTK_FILE"
}

# gtk_clear: empties the GTK entry
gtk_clear() {
  focus "$GT"
  xdotool key ctrl+a BackSpace
  sleep 0.2
}

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-GRD-003 the permission state is in the test hook state"
failures=""
perms=$($HC state | jq -c .permissions)
for key in microphone accessibility inputMonitoring; do
  [ "$(jq -r --arg k "$key" '.[$k] // empty' <<<"$perms")" != "" ] || failures+="the key $key is missing; "
done
[ "$(jq -c .lost <<<"$perms")" = "[]" ] || failures+="the lost list is $(jq -c .lost <<<"$perms"); "
[ "$(jq -r .microphone <<<"$perms")" = notApplicable ] || failures+="microphone is $(jq -r .microphone <<<"$perms"); "
check_failures "VAL-GRD-003 permissions state" "permissions $perms" "$failures"

echo "== VAL-GRD-001 a held keyboard grab blocks insertion and shows the error"
failures=""
gtk_clear
set_clip OLD
python3 /harness/slot/grab-keyboard.py >"$RUN_OUT/grab-helper.log" 2>&1 &
GRAB_PID=$!
wait_until 10 grep -q 'grab status 0' "$RUN_OUT/grab-helper.log" || failures+="the grab helper did not get the grab: $(cat "$RUN_OUT/grab-helper.log"); "
hook_run speech-short.wav || failures+="the run did not end; "
shot grd-001
text=$(dict .transcript)
[ "$(li .outcome)" = blocked_grab ] && [ "$(li .code)" = INSERT_KEYBOARD_GRABBED ] || failures+="receipt is $(lij); "
[ -z "$(insert_events | grep -o 'chord [^|]*')" ] || failures+="a paste chord was sent: $(insert_events); "
clip_is OLD || failures+="the clipboard reads '$(clip)' instead of OLD; "
[ "$(dict .notice.code)" = INSERT_KEYBOARD_GRABBED ] || failures+="notice code is '$(dict .notice.code)'; "
case "$(dict .notice.message)" in *"Another app holds the keyboard"*) ;; *) failures+="notice says '$(dict .notice.message)'; " ;; esac
chars=$($HC state | jq -r '.pipeline.last_text_chars')
[ "$chars" != null ] && [ "$chars" -gt 0 ] || failures+="the last transcript is not kept (chars $chars); "
kill "$GRAB_PID" 2>/dev/null
wait "$GRAB_PID" 2>/dev/null
got=$(gtk_commit)
[ -z "$got" ] || failures+="the entry holds '$got'; "
check_failures "VAL-GRD-001 keyboard grab" "receipt $(lij '{outcome,code}'), entry empty, clipboard OLD, notice shown, last transcript $chars chars" "$failures"

echo "== VAL-GRD-002 insertion works again after the grab ends"
failures=""
gtk_clear
set_clip OLD
hook_run speech-short.wav || failures+="the run did not end; "
text=$(dict .transcript)
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="the entry holds '$got' but the transcript is '$text'; "
[ -z "$(missing_words "$(norm <<<"$got")" "${SHORT_WORDS[@]}")" ] || failures+="the entry misses words: $got; "
[ "$(li .outcome)" = pasted ] || failures+="receipt is $(lij); "
gtk_clear
focus "$GT"
xdotool key 38
sleep 0.2
typed=$(gtk_commit)
[ "$typed" = a ] || failures+="key 38 typed '$typed' instead of a; "
check_failures "VAL-GRD-002 insertion after the grab" "entry '$got', receipt $(lij '{outcome}'), key 38 typed '$typed'" "$failures"

echo "== VAL-GRD-007 a Wayland session falls back to copy only"
failures=""
/harness/slot/app.sh stop
XDG_SESSION_TYPE=wayland /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
wait_until 90 engine_ready_with base || failures+="the engine did not load again; "
sleep 1
gtk_clear
before=$(gtk_commit)
set_clip OLD
hook_run speech-short.wav || failures+="the run did not end; "
shot grd-007
text=$(dict .transcript)
[ -n "$text" ] && [ "$(clip)" = "$text" ] || failures+="the clipboard reads '$(clip)' but the transcript is '$text'; "
[ -z "$(missing_words "$(norm <<<"$(clip)")" "${SHORT_WORDS[@]}")" ] || failures+="the clipboard misses words: $(clip); "
after=$(gtk_commit)
[ "$after" = "$before" ] || failures+="the entry changed from '$before' to '$after'; "
[ -z "$(insert_events | grep -o 'chord [^|]*')" ] || failures+="a paste chord event appeared: $(insert_events); "
[ "$(li .outcome)" = copied_only ] || failures+="receipt is $(lij); "
check_failures "VAL-GRD-007 Wayland copy only" "clipboard holds the transcript, the entry stayed '$after', no chord, receipt $(lij '{outcome,note}')" "$failures"
