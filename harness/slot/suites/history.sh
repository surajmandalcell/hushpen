#!/bin/bash
# The dictation history end to end: VAL-HIST-001 to 018 (006 and 007 on a database that
# slot/seed-history.sh fills with 10,000 rows; the store benchmark of 007 is a cargo test).
# The hold key is Right Alt (X11 keycode 108); the words come from the virtual microphone.
# Every outcome is read back with sqlite3 from /data/history/history.db, and the History view
# is read through the test hook (tree ids history.* and the state section history).
# Needs speech-short.wav, dictation-45s.wav, and silence-5s.wav in /assets/fixtures and the
# tiny.en and base.en models in /assets/models/whisper. Busy: it transcribes. It empties /data and
# restarts the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
ESC=Escape
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
SHORT_WAV=$FIX/speech-short.wav
LONG_WAV=$FIX/dictation-45s.wav
SILENCE_WAV=$FIX/silence-5s.wav
APP_LOG=/data/logs/hushpen.log

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
li() { $HC state | jq -r ".last_insert$1"; }
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

q() { sqlite3 "$DB" "$1"; }
count() { q "select count(*) from transcript"; }
newest() { q "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
col() { q "select coalesce($2, '') from transcript where id = '$1'"; }
segs() { q "select count(*) from segment where transcript_id = '$1'"; }
hs() { $HC state | jq -r ".history$1"; }
row_ids() { $HC state | jq -r '[.history.rows[].id] | join(",")'; }
row_titles() { $HC state | jq -r '[.history.rows[].title] | join("|")'; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }
row_elements() { $HC tree | jq '[.[] | select(.id | test("^history\\.row\\.[0-9]+$"))] | length'; }
count_is() { [ "$(count)" = "$1" ]; }
engine_up() { wait_until 90 engine_ready_with "$1"; }
history_search() { hook_action history-search "{\"query\":\"$1\"}" >/dev/null; sleep 0.5; }
secs_of() { awk -v ms="$1" 'BEGIN {printf "%.1f s", ms / 1000}'; }
alive() { kill -0 "$(ui_pid)" 2>/dev/null; }

open_history() {
  focus "$(app_window)"
  $HC click sidebar.history >/dev/null
  wait_until 5 rendered history.search
  sleep 0.4
}

# ptt_run <wav>: hold the key for the whole clip; HOLD_MS is the time the key was down
ptt_run() {
  local down
  T0=$(now_ms)
  xdotool keydown $HOLD
  down=$(now_ms)
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  HOLD_MS=$(($(now_ms) - down))
  wait_until 240 run_ended
}

# hook_run <wav>: the hold key's path through the hook; a keyboard grab stops real key delivery
hook_run() {
  T0=$(now_ms)
  hook_action pipeline-event '{"event":"hold-down"}' >/dev/null
  wait_until 5 pstate_is listening || {
    hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
    return 1
  }
  paplay --device=vmic "$(pad "$1")"
  hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
  wait_until 240 run_ended
}

# hold_then_cancel_transcribing <wav>: a run that Esc cancels while the engine works on it
long_run_until_transcribing() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  wait_until 10 pstate_is transcribing
}

reprocessed() { # <id> <model>: the row holds the new model and no reprocess is running
  [ "$(col "$1" model_id)" = "$2" ] && [ "$(col "$1" status)" = completed ] && [ "$(hs .reprocess)" = null ]
}

recall() { # <text> <reference>: the share of the reference words that the text has
  awk -v text="$1" -v ref="$2" 'BEGIN {
    n = split(text, t, " "); for (i = 1; i <= n; i++) seen[t[i]] = 1
    m = split(ref, r, " "); hit = 0
    for (i = 1; i <= m; i++) if (r[i] in seen) hit++
    printf "%.3f", (m ? hit / m : 0)
  }'
}

echo "== setup: tiny.en and base.en, and the paste targets"
fresh tiny.en base.en
use_model tiny.en
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-HIST-005 an empty History view on a fresh data folder"
failures=""
[ "$(count)" = 0 ] || failures+="the table is not empty at the start; "
open_history
rendered history.empty || failures+="no empty-state element; "
[ "$(row_elements)" = 0 ] || failures+="the tree has $(row_elements) row elements; "
[ "$(hs .count)" = 0 ] || failures+="the state lists $(hs .count) rows; "
$HC tree >"$RUN_OUT/hist-005-tree.json"
shot hist-005-empty
check_failures "VAL-HIST-005 empty state" "empty message shown, no row element in the tree" "$failures"

echo "== VAL-HIST-001 five dictations make five complete rows that survive a restart"
failures=""
before=$(count)
for run in 1 2 3 4 5; do
  gtk_clear
  set_clip OLD
  focus "$GT"
  ptt_run "$SHORT_WAV" || failures+="run $run did not end; "
done
shot hist-001-after-runs
inserted=$(gtk_commit)
after=$(count)
[ $((after - before)) = 5 ] || failures+="the row count grew from $before to $after; "
first_id=$(newest id)
[ "$(newest kind)" = dictation ] || failures+="kind is $(newest kind); "
[ "$(newest status)" = completed ] || failures+="status is $(newest status); "
[ "$(newest insert_outcome)" = pasted ] || failures+="insert_outcome is $(newest insert_outcome); "
target=$(li .target)
[ -n "$target" ] && [ "$(newest target_app)" = "$target" ] || failures+="target_app is '$(newest target_app)', not '$target'; "
dur=$(newest duration_ms)
diff=$((dur > HOLD_MS ? dur - HOLD_MS : HOLD_MS - dur))
[ "$diff" -le 500 ] || failures+="duration_ms $dur is $diff ms from the hold time $HOLD_MS; "
[ "$(newest model_id)" = tiny.en ] || failures+="model_id is $(newest model_id); "
[ -n "$(newest language_detected)" ] || failures+="no language; "
for column in raw_text rule_text final_text; do
  [ -n "$(newest $column)" ] || failures+="$column is empty; "
done
[ -n "$inserted" ] && [ "$(newest final_text)" = "$inserted" ] || failures+="final_text '$(newest final_text)' is not the inserted '$inserted'; "
/harness/slot/app.sh stop
start_app
engine_up tiny.en || failures+="the engine did not load again; "
open_history
sleep 0.5
shot hist-001-restarted
[ "$(count)" = "$after" ] || failures+="the count changed over the restart; "
[ "$(row_elements)" -ge 5 ] || failures+="after the restart the list has $(row_elements) rows; "
check_failures "VAL-HIST-001 complete rows" "$((after - before)) new rows; newest: ${dur} ms (hold $HOLD_MS ms), target '$target', final '$inserted'; 5 rows listed after a restart" "$failures"

echo "== VAL-HIST-011 the detail view shows the fields of the row"
failures=""
$HC click history.row.0 >/dev/null
wait_until 5 rendered history.detail || failures+="no detail view; "
sleep 0.4
shot hist-011-detail
[ "$(tree_text history.detail.raw)" = "$(newest raw_text)" ] || failures+="raw shows '$(tree_text history.detail.raw)'; "
[ "$(tree_text history.detail.rule)" = "$(newest rule_text)" ] || failures+="rule shows '$(tree_text history.detail.rule)'; "
[ "$(tree_text history.detail.final)" = "$(newest final_text)" ] || failures+="final shows '$(tree_text history.detail.final)'; "
[ "$(tree_text history.detail.app)" = "$(newest target_app)" ] || failures+="app shows '$(tree_text history.detail.app)'; "
[ "$(tree_text history.detail.model)" = "$(newest model_id)" ] || failures+="model shows '$(tree_text history.detail.model)'; "
[ "$(tree_text history.detail.language)" = "$(newest language_detected)" ] || failures+="language shows '$(tree_text history.detail.language)'; "
[ "$(tree_text history.detail.duration)" = "$(secs_of "$(newest duration_ms)")" ] || failures+="duration shows '$(tree_text history.detail.duration)'; "
$HC tree >"$RUN_OUT/hist-011-tree.json"
check_failures "VAL-HIST-011 detail fields" "raw, rule, final, app, model, language, and duration equal the SQLite row" "$failures"

echo "== VAL-HIST-012 Copy and Re-paste from the detail view"
failures=""
final=$(newest final_text)
$HC click history.copy >/dev/null
sleep 0.5
[ "$(clip)" = "$final" ] || failures+="after Copy the clipboard reads '$(clip)'; "
set_clip OLD
gtk_clear
focus "$GT"
T0=$(now_ms)
hook_action history-repaste "{\"id\":\"$first_id\"}" >/dev/null
sleep 1
[ "$(gtk_commit)" = "$final" ] || failures+="the entry did not get the text; "
wait_until 3 test "$(clip)" = OLD || failures+="the clipboard reads '$(clip)' instead of OLD; "
[ "$(count)" = "$after" ] || failures+="Re-paste changed the row count; "
shot hist-012-repaste
$HC click history.back >/dev/null
check_failures "VAL-HIST-012 copy and re-paste" "Copy put '$final' on the clipboard; Re-paste typed it into the entry and the clipboard came back to OLD" "$failures"

echo "== VAL-HIST-003 cancelled runs: Esc in transcribing makes a row, Esc in listening and a tap make none"
failures=""
c0=$(count)
gtk_clear
focus "$GT"
long_run_until_transcribing "$LONG_WAV" || failures+="never reached transcribing; "
xdotool key $ESC
wait_until 15 pstate_is idle || failures+="the run did not go back to idle; "
sleep 1
[ "$(count)" = $((c0 + 1)) ] || failures+="Esc in transcribing made $(($(count) - c0)) rows; "
cancel_id=$(newest id)
[ "$(newest status)" = cancelled ] || failures+="status is $(newest status); "
cancel_audio=/data/$(newest audio_path)
[ -f "$cancel_audio" ] || failures+="no audio file at $cancel_audio; "
open_history
sleep 0.4
[ "$(tree_text history.name.0)" = "Not transcribed" ] || failures+="the list label is '$(tree_text history.name.0)'; "
shot hist-003-cancelled
c1=$(count)
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="no listening for the Esc run; "
sleep 1
xdotool key $ESC
sleep 0.5
xdotool keyup $HOLD
wait_until 10 pstate_is idle || failures+="the Esc run did not end; "
[ "$(count)" = "$c1" ] || failures+="Esc in listening made a row; "
xdotool keydown $HOLD
sleep 0.1
xdotool keyup $HOLD
sleep 1
wait_until 10 pstate_is idle || failures+="the tap did not end; "
[ "$(count)" = "$c1" ] || failures+="a 100 ms tap made a row; "
check_failures "VAL-HIST-003 cancel rows" "one cancelled row with audio $cancel_audio labelled 'Not transcribed'; Esc in listening and the tap left the count at $c1" "$failures"

echo "== VAL-HIST-004 failed runs keep their audio and code"
failures=""
c0=$(count)
ptt_run "$SILENCE_WAV" || failures+="the silence run did not end; "
silence_id=$(newest id)
[ "$(count)" = $((c0 + 1)) ] || failures+="silence made $(($(count) - c0)) rows; "
[ "$(newest status)" = failed ] && [ "$(newest error_code)" = ENGINE_NO_SPEECH ] || failures+="silence row is $(newest status)/$(newest error_code); "
[ -f "/data/$(newest audio_path)" ] || failures+="the silence row has no audio file; "
c0=$(count)
long_run_until_transcribing "$LONG_WAV" || failures+="the engine kill run never reached transcribing; "
engine_pid_before=$(engine .pid)
kill -9 "$engine_pid_before"
wait_until 60 run_ended || failures+="the killed run did not end; "
kill_id=$(newest id)
[ "$(count)" = $((c0 + 1)) ] || failures+="the kill made $(($(count) - c0)) rows; "
[ "$(newest status)" = failed ] && [ -n "$(newest error_code)" ] || failures+="kill row is $(newest status)/'$(newest error_code)'; "
kill_audio=/data/$(newest audio_path)
[ -f "$kill_audio" ] || failures+="the kill row has no audio file; "
open_history
sleep 0.4
titles=$(row_titles)
case "$titles" in Failed*) ;; *) failures+="the newest row is not listed as failed: $titles; " ;; esac
[ "$($HC state | jq -r '[.history.rows[] | select(.status == "failed")] | length')" -ge 2 ] || failures+="the list does not show two failed rows; "
shot hist-004-failed
engine_up tiny.en || failures+="the engine did not come back; "
engine_pid_after=$(engine .pid)
[ "$engine_pid_after" != "$engine_pid_before" ] || failures+="the engine pid did not change; "
check_failures "VAL-HIST-004 failed rows" "silence row ENGINE_NO_SPEECH and kill row '$(col "$kill_id" error_code)', both failed with audio; engine pid $engine_pid_before then $engine_pid_after" "$failures"

echo "== VAL-HIST-002 blocked and copy-only outcomes make rows"
failures=""
python3 /harness/slot/grab-keyboard.py >"$RUN_OUT/grab-helper.log" 2>&1 &
GRAB_PID=$!
wait_until 10 grep -q 'grab status 0' "$RUN_OUT/grab-helper.log" || failures+="the grab helper did not get the grab; "
gtk_clear
set_clip OLD
hook_run "$SHORT_WAV" || failures+="the grab run did not end; "
kill "$GRAB_PID" 2>/dev/null
wait "$GRAB_PID" 2>/dev/null
[ "$(newest insert_outcome)" = blocked_grab ] || failures+="the grab row insert_outcome is '$(newest insert_outcome)'; "
[ -n "$(newest final_text)" ] || failures+="the grab row has no final_text; "
[ "$(clip)" = OLD ] || failures+="the clipboard reads '$(clip)' after the grab; "
shot hist-002-grab
/harness/slot/app.sh stop
XDG_SESSION_TYPE=wayland /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
engine_up tiny.en || failures+="the engine did not load in the wayland run; "
sleep 1
set_clip OLD
hook_run "$SHORT_WAV" || failures+="the wayland run did not end; "
[ "$(newest status)" = completed ] && [ "$(newest insert_outcome)" = copied_only ] || failures+="the copy-only row is $(newest status)/$(newest insert_outcome); "
[ -n "$(newest final_text)" ] || failures+="the copy-only row has no final_text; "
shot hist-002-copied
/harness/slot/app.sh stop
start_app
engine_up tiny.en || failures+="the engine did not load again; "
check_failures "VAL-HIST-002 outcomes" "grab row blocked_grab with text, clipboard OLD; wayland row completed/copied_only" "$failures"

echo "== VAL-HIST-013 Reprocess uses the current model and recovers a failed row"
failures=""
use_model base.en || failures+="base.en did not load; "
c0=$(count)
old_final=$(col "$first_id" final_text)
hook_action history-reprocess "{\"id\":\"$first_id\"}" >/dev/null || failures+="reprocess of the dictation row was refused; "
wait_until 120 reprocessed "$first_id" base.en || failures+="the dictation row did not get base.en; "
[ -n "$(col "$first_id" final_text)" ] || failures+="the dictation row has no text; "
hook_action history-reprocess "{\"id\":\"$kill_id\"}" >/dev/null || failures+="reprocess of the failed row was refused; "
wait_until 240 reprocessed "$kill_id" base.en || failures+="the failed row did not become completed with base.en; "
kill_text=$(col "$kill_id" final_text)
[ "$(col "$kill_id" error_code)" = "" ] || failures+="the failed row keeps error_code $(col "$kill_id" error_code); "
case " $(norm <<<"$kill_text") " in *" lighthouse "*) ;; *) failures+="'lighthouse' is missing from '$kill_text'; " ;; esac
reference=$(/app/hushpen engine --smoke "$kill_audio" --model /assets/models/whisper/ggml-base.en.bin 2>/dev/null | sed -n 's/^text: //p' | norm)
score=$(recall "$(norm <<<"$kill_text")" "$reference")
awk -v s="$score" 'BEGIN {exit !(s >= 0.9)}' || failures+="word recall is $score; "
[ "$(count)" = "$c0" ] || failures+="reprocess changed the row count from $c0 to $(count); "
[ "$(col "$kill_id" status)" = completed ] || failures+="the failed row is $(col "$kill_id" status); "
[ -f "$kill_audio" ] || failures+="the audio of the failed row is gone; "
shot hist-013-reprocessed
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the next run did not end; "
next=$(gtk_commit)
[ -z "$(missing_words "$(norm <<<"$next")" "${SHORT_WORDS[@]}")" ] || failures+="the next run inserted '$next'; "
check_failures "VAL-HIST-013 reprocess" "both rows now base.en, failed row completed (recall $score), count unchanged at $c0, engine pid $engine_pid_before then $engine_pid_after, next run '$next'" "$failures"

echo "== VAL-HIST-014 Reprocess applies the current dictionary"
failures=""
gtk_clear
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
fox_id=$(newest id)
before_rule=$(col "$fox_id" rule_text)
case "$before_rule" in *Foxtrel*) failures+="the row has Foxtrel before the entry exists; " ;; esac
history_search "x jum"
case ",$(row_ids)," in *",$fox_id,"*) ;; *) failures+="the old text is not found by 'x jum' before the reprocess; " ;; esac
hook_action dictionary-add '{"phrase":"Foxtrel","heard_as":"fox"}' >/dev/null
hook_action history-reprocess "{\"id\":\"$fox_id\"}" >/dev/null || failures+="the reprocess was refused; "
wait_until 120 reprocessed "$fox_id" base.en || failures+="the reprocess did not finish; "
after_rule=$(col "$fox_id" rule_text)
after_final=$(col "$fox_id" final_text)
case "$after_rule" in *Foxtrel*) ;; *) failures+="rule_text has no Foxtrel: '$after_rule'; " ;; esac
case "$after_final" in *Foxtrel*) ;; *) failures+="final_text has no Foxtrel: '$after_final'; " ;; esac
history_search "x jum"
case ",$(row_ids)," in *",$fox_id,"*) failures+="'x jum' still finds the reprocessed row; " ;; esac
history_search foxtrel
case ",$(row_ids)," in *",$fox_id,"*) ;; *) failures+="'foxtrel' does not find the reprocessed row; " ;; esac
history_search ""
dictionary_id=$(q "select id from dictionary_entry where phrase = 'Foxtrel'")
hook_action dictionary-delete "{\"id\":$dictionary_id}" >/dev/null
check_failures "VAL-HIST-014 dictionary on reprocess, and the VAL-HIST-010 reprocess half" "rule '$before_rule' became '$after_rule'; the search finds the new word and not the old one" "$failures"

echo "== VAL-HIST-015 Delete with undo restores the row, its segments, and its audio"
failures=""
snapshot_before=$(q "select * from transcript where id = '$fox_id'")
segments_before=$(q "select group_concat(idx || ':' || start_ms || ':' || end_ms || ':' || text, '|') from segment where transcript_id = '$fox_id'")
fox_audio=/data/$(col "$fox_id" audio_path)
[ -f "$fox_audio" ] || failures+="the row has no audio before the delete; "
[ -n "$segments_before" ] || failures+="the reprocessed row has no segments; "
open_history
hook_action history-delete "{\"id\":\"$fox_id\"}" >/dev/null
sleep 0.5
case ",$(row_ids)," in *",$fox_id,"*) failures+="the row is still listed; " ;; esac
rendered history.undo || failures+="no Undo control; "
shot hist-015-deleted
sleep 5
$HC click history.undo >/dev/null
sleep 0.5
[ "$(q "select * from transcript where id = '$fox_id'")" = "$snapshot_before" ] || failures+="the row differs after the undo; "
[ "$(q "select group_concat(idx || ':' || start_ms || ':' || end_ms || ':' || text, '|') from segment where transcript_id = '$fox_id'")" = "$segments_before" ] || failures+="the segments differ after the undo; "
[ -f "$fox_audio" ] || failures+="the audio file is not back; "
case "$(file -b "$fox_audio")" in *WAVE*) ;; *) failures+="the restored audio is not a WAV: $(file -b "$fox_audio"); " ;; esac
case ",$(row_ids)," in *",$fox_id,"*) ;; *) failures+="the row is not listed after the undo; " ;; esac
shot hist-015-restored
check_failures "VAL-HIST-015 undo" "same row, $(wc -w <<<"${segments_before//|/ }") segment text words, and audio back 5 s after the delete" "$failures"

echo "== VAL-HIST-016 an undone-never delete removes row, segments, and WAV"
failures=""
history_search lighthouse
case ",$(row_ids)," in *",$kill_id,"*) ;; *) failures+="'lighthouse' does not find the recovered row before the delete; " ;; esac
hook_action history-delete "{\"id\":\"$kill_id\"}" >/dev/null
sleep 10
[ "$(col "$kill_id" id)" = "" ] || failures+="the row is still in SQLite after the window; "
[ "$(segs "$kill_id")" = 0 ] || failures+="its segments are still in SQLite; "
[ ! -e "$kill_audio" ] || failures+="its WAV still exists; "
history_search lighthouse
case ",$(row_ids)," in *",$kill_id,"*) failures+="'lighthouse' still finds the deleted row; " ;; esac
[ "$(q "select count(*) from transcript_fts where transcript_fts match 'lighthouse'")" = 0 ] || failures+="the search index still has it; "
history_search ""
second_id=$first_id
second_audio=/data/$(col "$second_id" audio_path)
[ -f "$second_audio" ] || failures+="the second row has no audio to delete; "
hook_action history-delete "{\"id\":\"$second_id\"}" >/dev/null
sleep 1
/harness/slot/app.sh stop
start_app
engine_up base.en || failures+="the engine did not load again; "
[ "$(col "$second_id" id)" = "" ] || failures+="after quit and start the row is still in SQLite; "
[ "$(segs "$second_id")" = 0 ] || failures+="after quit and start its segments remain; "
[ ! -e "$second_audio" ] || failures+="after quit and start its WAV still exists; "
ls -l /data/audio >"$RUN_OUT/hist-016-audio.txt" 2>&1
check_failures "VAL-HIST-016 delete not undone" "after 10 s and after a quit 1 s later the rows, segments, and WAVs are gone, and search no longer finds the first" "$failures"

echo "== VAL-HIST-017 Clear all asks first and then removes every row and WAV"
failures=""
open_history
n=$(count)
[ "$n" -gt 0 ] || failures+="there are no rows to clear; "
audio_before=$(ls /data/audio/*.wav 2>/dev/null | wc -l)
$HC click history.clear >/dev/null
wait_until 5 rendered history.confirm || failures+="no confirmation; "
shot hist-017-confirm
$HC click history.clear-cancel >/dev/null
sleep 0.5
[ "$(count)" = "$n" ] || failures+="declining changed the count from $n to $(count); "
$HC click history.clear >/dev/null
wait_until 5 rendered history.confirm || failures+="no confirmation the second time; "
$HC click history.clear-confirm >/dev/null
wait_until 10 count_is 0 || failures+="the table still has $(count) rows; "
sleep 0.5
[ "$(ls /data/audio/*.wav 2>/dev/null | wc -l)" = 0 ] || failures+="WAV files remain: $(ls /data/audio); "
rendered history.empty || failures+="no empty state after clearing; "
shot hist-017-cleared
check_failures "VAL-HIST-017 clear all" "declined with $n rows kept; confirmed: table empty, $audio_before WAV files gone, empty state shown" "$failures"

echo "== VAL-HIST-008 and 009 search matches substrings, ignores case and accents, and shows no results"
failures=""
now=$(($(date +%s) * 1000))
q "insert into transcript (id, created_at, kind, status, duration_ms, final_text, raw_text, rule_text) values
  ('SEARCH-A', $((now - 3000)), 'dictation', 'completed', 1000, 'Café meeting at noon', 'Café meeting at noon', 'Café meeting at noon'),
  ('SEARCH-B', $((now - 2000)), 'dictation', 'completed', 1000, 'the lighthouse keeper', 'the lighthouse keeper', 'the lighthouse keeper'),
  ('SEARCH-C', $((now - 1000)), 'dictation', 'completed', 1000, 'ok then', 'ok then', 'ok then')"
history_search "cafe"
[ "$(row_ids)" = SEARCH-A ] || failures+="'cafe' lists '$(row_ids)'; "
shot hist-008-cafe
history_search "CAFÉ"
[ "$(row_ids)" = SEARCH-A ] || failures+="'CAFÉ' lists '$(row_ids)'; "
history_search "ghthou"
[ "$(row_ids)" = SEARCH-B ] || failures+="'ghthou' lists '$(row_ids)'; "
history_search "ok"
[ "$(row_ids)" = SEARCH-C ] || failures+="'ok' lists '$(row_ids)'; "
check_failures "VAL-HIST-008 search" "cafe and CAFÉ find the first row, ghthou the second, ok the third, each alone" "$failures"
failures=""
history_search "zzqqxx"
rendered history.no-results || failures+="no no-results message; "
[ "$(row_elements)" = 0 ] || failures+="rows are listed for zzqqxx; "
shot hist-009-none
history_search ""
[ "$(row_elements)" = 3 ] || failures+="clearing the search shows $(row_elements) rows, not 3; "
check_failures "VAL-HIST-009 no results" "the no-results state shows and clearing the search lists the 3 rows again" "$failures"

echo "== VAL-HIST-006 and 007 a 10,000-row history is paged, newest first, and searches fast"
failures=""
/harness/slot/app.sh stop
/harness/slot/seed-history.sh 10000 >"$RUN_OUT/hist-006-seed.log" 2>&1 || failures+="the seed failed: $(tail -1 "$RUN_OUT/hist-006-seed.log"); "
start_app
engine_up base.en || failures+="the engine did not load again; "
total=$(count)
hook_action frames-reset >/dev/null
open_history
sleep 1
expected_first=$(newest final_text)
[ "$(tree_text history.name.0)" = "$expected_first" ] || failures+="the first row shows '$(tree_text history.name.0)', not '$expected_first'; "
rows_page1=$(hs .count)
drawn_page1=$(row_elements)
[ "$rows_page1" = 50 ] || failures+="the first page holds $rows_page1 rows, not 50; "
[ "$drawn_page1" -gt 0 ] && [ "$drawn_page1" -lt 10000 ] || failures+="the tree has $drawn_page1 row elements; "
shot hist-006-first
open_gap=$($HC state | jq -r .frames.max_gap_ms)
hook_action frames-reset >/dev/null
xdotool mousemove --window "$(app_window)" 500 300
loaded_by=wheel
for _ in $(seq 200); do
  for _ in 1 2 3 4; do xdotool click 5; done
  sleep 0.3
  [ "$(hs .count)" -gt "$rows_page1" ] && break
done
sleep 1
rows_page2=$(hs .count)
drawn_page2=$(row_elements)
[ "$rows_page2" -gt "$rows_page1" ] || failures+="scrolling to the end did not load the next page ($rows_page1 then $rows_page2); "
want_ids=$(q "select group_concat(id, ',') from (select id from transcript order by created_at desc, id desc limit $rows_page2)")
[ "$(row_ids)" = "$want_ids" ] || failures+="the listed ids are not the newest $rows_page2 in order; "
shot hist-006-second
max_gap=$($HC state | jq -r .frames.max_gap_ms)
[ "$max_gap" != null ] && awk -v g="$max_gap" 'BEGIN {exit !(g < 100)}' || failures+="the UI stalled for $max_gap ms while the next page loaded; "
check_failures "VAL-HIST-006 paged list" "$total rows; first page $rows_page1 rows ($drawn_page1 drawn row elements), the next page loaded by $loaded_by ($rows_page2 rows, $drawn_page2 drawn), newest first, longest UI gap $open_gap ms opening and $max_gap ms scrolling and loading" "$failures"
failures=""
slowest=0
for query in lantern quartz "row 99" harbor saffron "ok" zzqqxx; do
  history_search "$query"
  ms=$(hs .search_ms)
  awk -v m="$ms" -v s="$slowest" 'BEGIN {exit !(m > s)}' && slowest=$ms
  awk -v m="$ms" 'BEGIN {exit !(m < 200)}' || failures+="'$query' took $ms ms; "
done
history_search ""
check_failures "VAL-HIST-007 search time in the app" "the slowest of 7 queries on $total rows took $slowest ms" "$failures"

echo "== VAL-HIST-018 a history write error never loses the transcript"
failures=""
gtk_clear
set_clip OLD
chmod 0444 "$DB"
chmod 0555 "$(dirname "$DB")"
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
written=$(gtk_commit)
[ -z "$(missing_words "$(norm <<<"$written")" "${SHORT_WORDS[@]}")" ] || failures+="the entry holds '$written'; "
alive || failures+="the app is gone; "
grep -h HISTORY_WRITE_FAILED "$APP_LOG" /data/logs/*.log 2>/dev/null | head -1 >"$RUN_OUT/hist-018-log.txt"
[ -s "$RUN_OUT/hist-018-log.txt" ] || failures+="no HISTORY_WRITE_FAILED in the log; "
hits=$(grep -rih -E 'quick|brown|lazy' "$APP_LOG" /data/logs/ 2>/dev/null | head -2)
[ -z "$hits" ] || failures+="transcript words in the log: $hits; "
gtk_clear
focus "$GT"
xdotool key ctrl+alt+v
again=$(gtk_commit)
[ "$again" = "$written" ] || failures+="Paste last transcript gave '$again', not '$written'; "
chmod 0644 "$DB"
chmod 0755 "$(dirname "$DB")"
shot hist-018
check_failures "VAL-HIST-018 write error" "the entry got '$written', the log has a coded error without text, Paste last gave the same text, the app is up" "$failures"
