#!/bin/bash
# The shortcut recorder in Settings > Shortcuts: VAL-KEY-001 to 006 and 008 (VAL-KEY-007 is the
# unit tests of hushpen-core). The recorder is opened with a click on the field (hookctl click)
# and the keys come from xdotool as keycodes: Right Alt 108, Right Ctrl 105, Right Shift 62, Esc 9.
# Another X client that holds a chord is grab-chord.py. Needs speech-short.wav in /assets/fixtures
# and the base model in /assets/models/whisper. Busy: it transcribes. It empties /data and
# restarts the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

GTK_FILE=/out/gtk.txt
GRAB="python3 /harness/slot/grab-chord.py"
HELPER_LOG=$RUN_OUT/grab-helper.log
SHORT_WAV=$FIX/speech-short.wav
HOLD=108

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
focus() { xdotool windowfocus --sync "$1"; }
speech() { paplay --device=vmic "$(pad "$1")"; }
li() { $HC state | jq -r ".last_insert$1"; }
fresh_paste() { [ "$(li .outcome)" = pasted ] && [ "$(li .ready_unix_ms)" -ge "$T0" ]; }
words_ok() { [ -z "$(missing_words "$(norm <<<"$1")" "${SHORT_WORDS[@]}")" ]; }

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

sc() { $HC state | jq -r ".shortcuts$1"; }
saved() { jq -r ".values[\"shortcut.$1\"]" "$SETTINGS"; }
field() { sc ".slots.$1.display"; }
slot_error() { sc ".slots.$1.error.code"; }
recording_is() { [ "$(sc .recording)" = "$1" ]; }
field_is() { [ "$(field "$1")" = "$2" ]; }
error_is() { [ "$(slot_error "$1")" = "$2" ]; }
open_settings() {
  $HC action open-view '{"view":"settings"}' >/dev/null
  wait_until 10 test "$($HC state | jq -r .view)" = settings
}
record_open() { # <slot>
  $HC click "settings.shortcut.$1" >/dev/null
  wait_until 5 recording_is "$1"
}
tap() { xdotool keydown "$@"; sleep 0.25; xdotool keyup "$@"; }
chord() { xdotool key --delay 120 "$1"; }
# After a recording the recorder closes on its own, or stays open with the error under the field.
recorder_done() { [ "$(sc .recording)" = null ] || [ "$(slot_error "$1")" != null ]; }

hold_run() { # <keycode> <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown "$1"
  wait_until 5 pstate_is listening || { xdotool keyup "$1"; return 1; }
  speech "$2"
  xdotool keyup "$1"
  wait_until 240 run_ended
}
no_session_for() { # <keycode>: a press and release of the key starts no dictation
  T0=$(now_ms)
  tap "$1"
  sleep 1
  [ -z "$(pevents)" ] && pstate_is idle
}
toggle_run() { # <chord>: the chord starts a session, speech, the same chord stops it
  T0=$(now_ms)
  chord "$1"
  wait_until 5 pstate_is listening || return 1
  speech "$2"
  chord "$1"
  wait_until 240 run_ended
}
helper_up() { # <chord>
  $GRAB hold "$1" >"$HELPER_LOG" 2>&1 &
  HELPER=$!
  wait_until 5 grep -q grabbed "$HELPER_LOG"
}
helper_down() { [ -n "${HELPER:-}" ] && kill "$HELPER" 2>/dev/null; wait "$HELPER" 2>/dev/null; HELPER=""; }
chord_free() { $GRAB probe "$1" >/dev/null 2>&1; }
shortcut_texts() { # the visible text of the four fields
  $HC tree | jq -r '.[] | select(.id == "settings.shortcut.hold" or .id == "settings.shortcut.handsFree"
    or .id == "settings.shortcut.pasteLast" or .id == "settings.shortcut.command")
    | (.id | ltrimstr("settings.shortcut.")) + ": " + .text'
}

echo "== setup: base model, the paste targets, the Settings view"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
open_settings
wait_until 10 test "$(sc .recorder_available)" = true
HELPER=""

echo "== VAL-KEY-006 Linux shows Linux key names"
failures=""
shortcut_texts >"$RUN_OUT/006-fields.txt"
$HC tree >"$RUN_OUT/006-tree.json"
shot 006-shortcuts
texts=$(cat "$RUN_OUT/006-fields.txt")
case "$texts" in *"hold: Right Alt"*) ;; *) failures+="the hold field does not read Right Alt: $texts; " ;; esac
case "$texts" in *"pasteLast: Ctrl+Alt+V"*) ;; *) failures+="the paste last field does not read Ctrl+Alt+V: $texts; " ;; esac
case "$texts" in *"command: Right Alt+Right Shift"*) ;; *) failures+="the Command Mode field does not read Right Alt+Right Shift: $texts; " ;; esac
for bad in ⌘ ⌥ ⌃ ⇧ Option Command; do
  case "$texts" in *"$bad"*) failures+="the fields contain '$bad'; " ;; esac
done
check_failures "VAL-KEY-006 Linux names" "fields read: $(tr '\n' '|' <"$RUN_OUT/006-fields.txt")" "$failures"

echo "== VAL-KEY-001 a new hold key recorded in Settings starts dictation"
failures=""
focus "$GT"
record_open hold || failures+="the recorder did not open for hold (recording=$(sc .recording)); "
sleep 0.6
field_text=$(shortcut_texts | grep '^hold:')
case "$field_text" in *"Press"*) ;; *) failures+="the open recorder shows '$field_text'; " ;; esac
shot 001-recording
tap 105
wait_until 5 field_is hold "Right Ctrl" || failures+="the hold field reads '$(field hold)'; "
wait_until 5 test "$(sc .recording)" = null || failures+="the recorder is still open; "
[ "$(saved hold)" = RightCtrl ] || failures+="shortcut.hold is '$(saved hold)'; "
shot 001-recorded
gtk_clear
hold_run 105 "$SHORT_WAV" || failures+="the hold run with Right Ctrl did not end; "
got=$(gtk_commit)
words_ok "$got" || failures+="the entry holds '$got'; "
no_session_for 108 || failures+="Right Alt still starts a dictation: $(pevents); "
check_failures "VAL-KEY-001 record hold key" "field 'Right Ctrl', shortcut.hold RightCtrl, Right Ctrl typed '$got', Right Alt starts nothing" "$failures"

echo "== VAL-KEY-002 chords for hands-free, paste last, and Command Mode"
failures=""
record_open handsFree || failures+="no recorder for handsFree; "
chord ctrl+alt+h
wait_until 5 field_is handsFree "Ctrl+Alt+H" || failures+="the hands-free field reads '$(field handsFree)'; "
record_open pasteLast || failures+="no recorder for pasteLast; "
chord ctrl+alt+p
wait_until 5 field_is pasteLast "Ctrl+Alt+P" || failures+="the paste last field reads '$(field pasteLast)'; "
record_open command || failures+="no recorder for command; "
xdotool keydown 105 keydown 62
sleep 0.3
xdotool keyup 62 keyup 105
wait_until 5 field_is command "Right Ctrl+Right Shift" || failures+="the Command Mode field reads '$(field command)'; "
shot 002-recorded
[ "$(saved handsFree)" = Ctrl+Alt+H ] || failures+="shortcut.handsFree is '$(saved handsFree)'; "
[ "$(saved pasteLast)" = Ctrl+Alt+P ] || failures+="shortcut.pasteLast is '$(saved pasteLast)'; "
[ "$(saved command)" = RightCtrl+RightShift ] || failures+="shortcut.command is '$(saved command)'; "
gtk_clear
toggle_run ctrl+alt+h "$SHORT_WAV" || failures+="the hands-free chord did not run a session: $(pevents); "
first=$(gtk_commit)
words_ok "$first" || failures+="the hands-free session typed '$first'; "
first_text=$(dict .transcript)
gtk_clear
set_clip OLD
focus "$GT"
T0=$(now_ms)
chord ctrl+alt+p
wait_until 10 fresh_paste || failures+="Ctrl+Alt+P made no paste: $(li .); "
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$first_text" ] || failures+="Ctrl+Alt+P typed '$got', the transcript is '$first_text'; "
gtk_clear
focus "$GT"
T0=$(now_ms)
chord ctrl+alt+v
sleep 1.5
old=$(gtk_commit)
[ -z "$old" ] || failures+="the old chord Ctrl+Alt+V still pasted '$old'; "
[ -z "$(pevents)" ] || failures+="the old chord moved the pipeline: $(pevents); "
chord_free ctrl+alt+v || failures+="another client cannot grab Ctrl+Alt+V; "
check_failures "VAL-KEY-002 chords" "saved Ctrl+Alt+H, Ctrl+Alt+P, RightCtrl+RightShift; hands-free ran and typed '$first'; Ctrl+Alt+P pasted the last transcript; Ctrl+Alt+V does nothing and can be grabbed" "$failures"

echo "== VAL-KEY-003 a chord held by another X client shows the in-use error"
failures=""
helper_up ctrl+alt+j || failures+="the helper did not grab Ctrl+Alt+J ($(cat "$HELPER_LOG")); "
record_open pasteLast || failures+="no recorder for pasteLast; "
chord ctrl+alt+j
wait_until 5 error_is pasteLast SHORTCUT_IN_USE || failures+="pasteLast error is '$(slot_error pasteLast)'; "
$HC state | jq -c .shortcuts >"$RUN_OUT/003-state.json"
shot 003-in-use
[ "$(saved pasteLast)" = Ctrl+Alt+P ] || failures+="shortcut.pasteLast changed to '$(saved pasteLast)'; "
$HC action shortcut-cancel >/dev/null
helper_down
gtk_clear
set_clip OLD
focus "$GT"
T0=$(now_ms)
chord ctrl+alt+p
wait_until 10 fresh_paste || failures+="the previous chord Ctrl+Alt+P no longer pastes; "
got=$(gtk_commit)
[ "$got" = "$first_text" ] || failures+="Ctrl+Alt+P typed '$got'; "
check_failures "VAL-KEY-003 in use" "SHORTCUT_IN_USE for pasteLast, setting stayed Ctrl+Alt+P, the old chord still pasted" "$failures"

echo "== VAL-KEY-004 a Hushpen shortcut or a reserved chord is rejected"
failures=""
hands0=$(saved handsFree)
record_open handsFree || failures+="no recorder for handsFree; "
chord ctrl+alt+p
wait_until 5 error_is handsFree SHORTCUT_RESERVED || failures+="the paste last chord gave '$(slot_error handsFree)'; "
shot 004-taken
$HC state | jq -c .shortcuts >"$RUN_OUT/004-state-taken.json"
[ "$(saved handsFree)" = "$hands0" ] || failures+="shortcut.handsFree changed to '$(saved handsFree)'; "
$HC action shortcut-cancel >/dev/null
record_open handsFree || failures+="no second recorder for handsFree; "
chord ctrl+q
wait_until 5 error_is handsFree SHORTCUT_RESERVED || failures+="Ctrl+Q gave '$(slot_error handsFree)'; "
shot 004-reserved
$HC state | jq -c .shortcuts >"$RUN_OUT/004-state-reserved.json"
[ "$(saved handsFree)" = "$hands0" ] || failures+="shortcut.handsFree changed to '$(saved handsFree)' after Ctrl+Q; "
$HC action shortcut-cancel >/dev/null
gtk_clear
set_clip OLD
focus "$GT"
T0=$(now_ms)
chord ctrl+alt+p
wait_until 10 fresh_paste || failures+="Ctrl+Alt+P no longer pastes; "
toggle_run ctrl+alt+h "$SHORT_WAV" || failures+="Ctrl+Alt+H no longer runs a session: $(pevents); "
check_failures "VAL-KEY-004 reserved" "both attempts gave SHORTCUT_RESERVED, handsFree stayed $hands0, paste last and hands-free still work" "$failures"

echo "== VAL-KEY-005 Esc cancels the recorder and the recorder blocks the live shortcut"
failures=""
record_open hold || failures+="no recorder for hold; "
T0=$(now_ms)
xdotool keydown 105
sleep 0.5
pstate_is idle || failures+="Right Ctrl started a dictation while recording ($(pstate)); "
xdotool key Escape
sleep 0.3
xdotool keyup 105
sleep 0.7
shot 005-cancelled
[ -z "$(pevents)" ] || failures+="the pipeline moved while recording: $(pevents); "
recording_is null || failures+="the recorder is still open (recording=$(sc .recording)); "
[ "$(saved hold)" = RightCtrl ] || failures+="shortcut.hold is '$(saved hold)'; "
field_is hold "Right Ctrl" || failures+="the hold field reads '$(field hold)'; "
gtk_clear
hold_run 105 "$SHORT_WAV" || failures+="the hold key does not work after the recorder closed; "
got=$(gtk_commit)
words_ok "$got" || failures+="the entry holds '$got' after the recorder; "
check_failures "VAL-KEY-005 Esc cancels" "keydown during recording started nothing, Esc closed the recorder, shortcut.hold stayed RightCtrl and worked again (typed '$got')" "$failures"

echo "== VAL-KEY-008 Reset to default, and a chord in use at start"
failures=""
$HC state | jq -c '.shortcuts.slots | map_values(.setting)' >"$RUN_OUT/008-before.json"
$HC click settings.shortcut.reset >/dev/null || failures+="no Reset to default button; "
wait_until 5 test "$(saved hold)" = RightAlt || failures+="shortcut.hold is '$(saved hold)' after the reset; "
jq '.values | with_entries(select(.key | startswith("shortcut.")))' "$SETTINGS" >"$RUN_OUT/008-after.json"
[ "$(saved pasteLast)" = Ctrl+Alt+V ] || failures+="shortcut.pasteLast is '$(saved pasteLast)'; "
[ "$(saved command)" = RightAlt+RightShift ] || failures+="shortcut.command is '$(saved command)'; "
[ "$(saved handsFree)" = DoubleTap+Hold+Space ] || failures+="shortcut.handsFree is '$(saved handsFree)'; "
field_is hold "Right Alt" || failures+="the hold field reads '$(field hold)'; "
shot 008-reset
gtk_clear
hold_run 108 "$SHORT_WAV" || failures+="the default hold key did not run a dictation; "
got=$(gtk_commit)
words_ok "$got" || failures+="the default hold key typed '$got'; "
helper_up ctrl+alt+v || failures+="the helper did not grab Ctrl+Alt+V ($(cat "$HELPER_LOG")); "
/harness/slot/app.sh stop
start_app
wait_until 90 engine_ready_with base
open_settings
wait_until 10 error_is pasteLast SHORTCUT_IN_USE || failures+="after the restart the pasteLast error is '$(slot_error pasteLast)'; "
$HC state | jq -c .shortcuts >"$RUN_OUT/008-restart-state.json"
$HC tree >"$RUN_OUT/008-restart-tree.json"
shot 008-in-use-at-start
[ "$(saved pasteLast)" = Ctrl+Alt+V ] || failures+="shortcut.pasteLast is '$(saved pasteLast)' after the restart; "
gtk_clear
hold_run 108 "$SHORT_WAV" || failures+="the hold key does not work after the restart; "
got2=$(gtk_commit)
words_ok "$got2" || failures+="the hold key typed '$got2' after the restart; "
helper_down
check_failures "VAL-KEY-008 reset and in use at start" "Reset gave RightAlt, the default hands-free, Ctrl+Alt+V, RightAlt+RightShift and the hold key typed '$got'; with Ctrl+Alt+V held by a helper the app started, showed SHORTCUT_IN_USE for paste last, and the hold key typed '$got2'" "$failures"
