#!/bin/bash
# Rule cleanup end to end (the harness half of VAL-CLN-008): a dictated filler fixture reaches the
# GTK entry without um, uh, or er, and with cleanup.rules off the same dictation reaches it raw.
# The hold key is Right Alt (X11 keycode 108); the words come from the virtual microphone.
# Needs filler-dictation.wav in HUSHPEN_TEST_FIXTURES (mounted at /fixtures). Busy: it
# transcribes with base. It empties /data and restarts the app, so the slot must not be shared.
# Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
FILLER_WAV=/fixtures/filler-dictation.wav

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() {
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
focus() { xdotool windowfocus --sync "$1"; }
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
has_filler() { grep -Eiwq 'um+|uh+|er+m?' <<<"$1"; }

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

ptt_run() { # <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-CLN-008 (1) rule cleanup is on by default and the filler dictation is inserted clean"
failures=""
[ -e "$FILLER_WAV" ] || failures+="$FILLER_WAV is missing; "
[ "$($HC state | jq -c '.settings["cleanup.rules"]')" = true ] || failures+="cleanup.rules is not true by default; "
set_clip OLD
focus "$GT"
ptt_run "$FILLER_WAV" || failures+="the run did not end; "
shot cln-on
clean_text=$(dict .transcript)
clean_got=$(gtk_commit)
[ -n "$clean_got" ] && [ "$clean_got" = "$clean_text" ] || failures+="the entry holds '$clean_got' but the transcript is '$clean_text'; "
has_filler "$clean_got" && failures+="the entry still has a filler: $clean_got; "
case "$clean_got" in [A-Z]*) ;; *) failures+="the entry does not start with a capital letter: $clean_got; " ;; esac
check_failures "VAL-CLN-008 filler fixture inserted clean" "entry '$clean_got'" "$failures"

echo "== VAL-CLN-008 (2) cleanup.rules off inserts the raw text, which holds a filler"
failures=""
set_setting cleanup.rules false
[ "$($HC state | jq -c '.settings["cleanup.rules"]')" = false ] || failures+="cleanup.rules did not change; "
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$FILLER_WAV" || failures+="the run did not end; "
shot cln-off
raw_text=$(dict .transcript)
raw_got=$(gtk_commit)
[ -n "$raw_got" ] && [ "$raw_got" = "$raw_text" ] || failures+="the entry holds '$raw_got' but the transcript is '$raw_text'; "
has_filler "$raw_got" || failures+="the raw text has no filler, so the fixture is invalid: $raw_got; "
check_failures "VAL-CLN-008 cleanup off inserts raw" "entry '$raw_got'" "$failures"
set_setting cleanup.rules true
