#!/bin/bash
# Export of history rows end to end: VAL-EXP-005 to 008 (the writers of 001 to 004 are cargo
# tests; 009 is the hp3 CI run).
# 005 seeds 3 rows with segments in the database and exports them in all 4 formats through
# hookctl paths. 006 dictates one row for real (hold key Right Alt, keycode 108, words from the
# virtual microphone) and saves it from the portal dialog that xdg-desktop-portal-gtk shows.
# 007 takes the portal away (the processes end and a bare name owner holds the portal name on the
# session bus) so rfd has to open zenity. 008 answers one dialog through the hook and cancels
# another with Escape.
# Needs speech-short.wav in /assets/fixtures and the tiny.en model in /assets/models/whisper.
# Busy: it transcribes. It empties /data and restarts the app, so the slot must not be shared.
# Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
OUT=/out
DB=/data/history/history.db
SHORT_WAV=$FIX/speech-short.wav
DIALOG_NAMES='^(Save File|Export transcripts|zenity)$'

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
q() { sqlite3 "$DB" "$1"; }
hs() { $HC state | jq -r ".history$1"; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }
dialog() { xdotool search --onlyvisible --name "$DIALOG_NAMES" 2>/dev/null | head -1; }
dialog_open() { [ -n "$(dialog)" ]; }
dialog_gone() { [ -z "$(dialog)" ]; }
history_idle() { [ "$(hs .export_busy)" = false ]; }
engine_up() { wait_until 90 engine_ready_with "$1"; }
out_files() { find "$OUT" -maxdepth 1 -type f \( -name '*.txt' -o -name '*.srt' -o -name '*.vtt' -o -name '*.json' \) | sort; }

open_history() {
  focus "$(app_window)"
  $HC click sidebar.history >/dev/null
  wait_until 5 rendered history.search
  wait_until 10 rendered history.row.0
  sleep 0.4
}

open_export_menu() {
  rendered history.export-txt || $HC click history.export >/dev/null
  wait_until 5 rendered history.export-txt
}

ptt_run() { # <wav>
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

# save_in_dialog <path>: type the path into the name field of the open dialog and confirm it
save_in_dialog() {
  local wid
  wid=$(dialog)
  xdotool windowfocus --sync "$wid"
  sleep 0.5
  xdotool key ctrl+a
  xdotool type --delay 20 "$1"
  sleep 0.3
  xdotool key Return
}

# ms_clock <ms> <separator>: HH:MM:SS<sep>mmm
ms_clock() {
  awk -v ms="$1" -v sep="$2" 'BEGIN {
    printf "%02d:%02d:%02d%s%03d", ms / 3600000, (ms / 60000) % 60, (ms / 1000) % 60, sep, ms % 1000
  }'
}

echo "== setup: three seeded rows with segments, then tiny.en and the paste targets"
fresh tiny.en
/harness/slot/app.sh stop
now=$(($(date +%s) * 1000))
sqlite3 "$DB" <<SQL
INSERT INTO transcript (id, created_at, kind, status, duration_ms, model_id, language_detected, raw_text, rule_text, final_text, insert_outcome, target_app) VALUES
  ('SEED0000000000000000000001', $((now - 30000)), 'dictation', 'completed', 2000, 'base', 'en', 'first seeded row', 'first seeded row', 'First seeded row.', 'pasted', 'seed-target'),
  ('SEED0000000000000000000002', $((now - 20000)), 'dictation', 'completed', 3000, 'base', 'en', 'second seeded row', 'second seeded row', 'Second seeded row.', 'pasted', 'seed-target'),
  ('SEED0000000000000000000003', $((now - 10000)), 'dictation', 'completed', 1000, 'base', 'en', 'third seeded row', 'third seeded row', 'Third seeded row, café ☕.', 'pasted', 'seed-target');
INSERT INTO segment (transcript_id, idx, start_ms, end_ms, text) VALUES
  ('SEED0000000000000000000001', 0, 0, 900, 'First seeded'),
  ('SEED0000000000000000000001', 1, 900, 2000, 'row.'),
  ('SEED0000000000000000000002', 0, 0, 3000, 'Second seeded row.'),
  ('SEED0000000000000000000003', 0, 0, 1000, 'Third seeded row, café ☕.');
SQL
start_app
use_model tiny.en
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
rm -f "$OUT"/*.txt "$OUT"/*.srt "$OUT"/*.vtt "$OUT"/*.json

echo "== VAL-EXP-005 three selected rows make one file in each format"
failures=""
open_history
$HC click history.select >/dev/null
sleep 0.5
for index in 0 1 2; do
  $HC click "history.row.$index" >/dev/null
  sleep 0.2
done
[ "$(hs '.selection | length')" = 3 ] || failures+="the selection holds $(hs '.selection | length') rows; "
shot exp-005-selected
for format in txt srt vtt json; do
  before=$(out_files | wc -l)
  $HC paths "$OUT/all.$format" >/dev/null
  $HC click "history.export-$format" >/dev/null
  wait_until 5 test -s "$OUT/all.$format" || failures+="all.$format was not written; "
  [ "$(($(out_files | wc -l) - before))" = 1 ] || failures+="$format wrote $(($(out_files | wc -l) - before)) files; "
  [ "$(hs .export.rows)" = 3 ] || failures+="$format export rows is $(hs .export.rows); "
done
dialog_gone || failures+="a dialog is still open; "
[ "$(cat "$OUT/all.txt")" = "$(printf 'First seeded row.\n\nSecond seeded row.\n\nThird seeded row, café ☕.')" ] ||
  failures+="all.txt differs; "
expected_srt='1
00:00:00,000 --> 00:00:00,900
First seeded

2
00:00:00,900 --> 00:00:02,000
row.

3
00:00:02,000 --> 00:00:05,000
Second seeded row.

4
00:00:05,000 --> 00:00:06,000
Third seeded row, café ☕.'
[ "$(cat "$OUT/all.srt")" = "$expected_srt" ] || failures+="all.srt differs: $(head -c 400 "$OUT/all.srt" | tr '\n' '|'); "
[ "$(head -2 "$OUT/all.vtt" | tr '\n' '|')" = 'WEBVTT||' ] || failures+="all.vtt has no header; "
grep -q '00:00:02.000 --> 00:00:05.000' "$OUT/all.vtt" || failures+="all.vtt has no dot times; "
[ "$(jq '.rows | length' "$OUT/all.json")" = 3 ] || failures+="all.json holds $(jq '.rows | length' "$OUT/all.json") rows; "
[ "$(jq -r '[.rows[].final_text] | join("|")' "$OUT/all.json")" = 'First seeded row.|Second seeded row.|Third seeded row, café ☕.' ] ||
  failures+="all.json texts differ; "
check_failures "VAL-EXP-005 three rows" "each of the 4 formats wrote one file with all 3 rows; SRT cues 1 to 4 with the durations before each row added; JSON holds 3 row objects" "$failures"

echo "== VAL-EXP-006 export through the portal dialog"
failures=""
$HC click history.select >/dev/null
gtk_row_count=$(q "select count(*) from transcript")
focus "$GT"
xdotool key ctrl+a BackSpace
ptt_run "$SHORT_WAV" || failures+="the dictation did not end; "
newest=$(q "select id from transcript order by created_at desc, id desc limit 1")
[ "$(q "select count(*) from transcript")" = $((gtk_row_count + 1)) ] || failures+="the dictation made no new row; "
[ "$(q "select count(*) from segment where transcript_id = '$newest'")" -ge 1 ] || failures+="the new row has no segments; "
open_history
$HC click history.row.0 >/dev/null
wait_until 5 rendered history.detail || failures+="no detail view; "
open_export_menu || failures+="no format buttons; "
rm -f "$OUT/one.srt"
$HC click history.export-srt >/dev/null
wait_until 15 dialog_open || failures+="no save dialog appeared; "
sleep 1
shot exp-006-dialog
save_in_dialog "$OUT/one.srt"
wait_until 10 test -s "$OUT/one.srt" || failures+="one.srt was not written; "
wait_until 5 history_idle
expected=""
index=1
while IFS='|' read -r start end; do
  expected+="$index|$(ms_clock "$start" ,) --> $(ms_clock "$end" ,)"$'\n'
  index=$((index + 1))
done < <(q "select start_ms, end_ms from segment where transcript_id = '$newest' order by idx")
actual=$(awk 'BEGIN {n = 0} /^[0-9]+$/ {num = $0; next} /-->/ {print num "|" $0}' "$OUT/one.srt")
[ "$actual" = "${expected%$'\n'}" ] || failures+="cue times '$(tr '\n' ';' <<<"$actual")' differ from SQLite '$(tr '\n' ';' <<<"$expected")'; "
$HC tree >"$RUN_OUT/exp-006-tree.json"
shot exp-006-done
check_failures "VAL-EXP-006 portal dialog" "the dialog showed, one.srt holds $(grep -c -- '-->' "$OUT/one.srt") cue(s) whose times equal the segment rows of $newest" "$failures"

echo "== VAL-EXP-007 without a portal the rfd fallback dialog writes the file"
failures=""
pkill -f xdg-desktop-portal 2>/dev/null
sleep 1
# A name owner without a FileChooser keeps D-Bus from starting the real portal again.
python3 - <<'PY' >/dev/null 2>&1 &
from gi.repository import Gio, GLib
Gio.bus_own_name(Gio.BusType.SESSION, 'org.freedesktop.portal.Desktop', Gio.BusNameOwnerFlags.NONE, None, None, None)
GLib.MainLoop().run()
PY
HOLDER=$!
sleep 1
pgrep -f 'xdg-desktop-portal-gtk' >/dev/null && failures+="the portal backend still runs; "
final=$(q "select final_text from transcript where id = '$newest'")
rm -f "$OUT/fallback.txt"
open_export_menu
$HC click history.export-txt >/dev/null
wait_until 20 dialog_open || failures+="no fallback dialog appeared; "
sleep 1
shot exp-007-dialog
save_in_dialog "$OUT/fallback.txt"
wait_until 10 test -s "$OUT/fallback.txt" || failures+="fallback.txt was not written; "
[ "$(cat "$OUT/fallback.txt")" = "$final" ] || failures+="fallback.txt holds '$(cat "$OUT/fallback.txt")', not '$final'; "
kill "$HOLDER" 2>/dev/null
shot exp-007-done
check_failures "VAL-EXP-007 rfd fallback" "with the portal gone a dialog still appeared and fallback.txt holds the final text" "$failures"

echo "== VAL-EXP-008 an injected path needs no dialog and a cancelled dialog writes nothing"
failures=""
rm -f "$OUT/inj.vtt"
$HC paths "$OUT/inj.vtt" >/dev/null
open_export_menu
$HC click history.export-vtt >/dev/null
wait_until 5 test -s "$OUT/inj.vtt" || failures+="inj.vtt was not written; "
dialog_gone || failures+="a dialog showed for the injected path; "
head -1 "$OUT/inj.vtt" | grep -q '^WEBVTT$' || failures+="inj.vtt has no header; "
shot exp-008-injected
before=$(out_files | md5sum)
open_export_menu
$HC click history.export-txt >/dev/null
wait_until 20 dialog_open || failures+="no dialog to cancel; "
sleep 1
xdotool windowfocus --sync "$(dialog)"
sleep 0.5
xdotool key Escape
wait_until 10 dialog_gone || failures+="the dialog stayed after Escape; "
sleep 1
wait_until 5 history_idle || failures+="the export state stayed busy; "
[ "$(out_files | md5sum)" = "$before" ] || failures+="the output folder changed after a cancel; "
[ "$(hs .message)" = null ] || failures+="an error shows: $(hs .message); "
rendered history.message && failures+="a message element is drawn; "
$HC tree >"$RUN_OUT/exp-008-tree.json"
shot exp-008-cancelled
check_failures "VAL-EXP-008 injected and cancelled" "the injected path wrote inj.vtt with no dialog; Escape closed the next dialog with no new file and no error" "$failures"

finish export
