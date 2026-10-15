#!/bin/bash
# File import end to end: VAL-IMP-001 to 008 and 010, VAL-CROSS-003 and 004, and the thread count
# of the X11 flow bar watcher over 20 hide and show cycles (a hp4 scrutiny item folded into this
# feature). VAL-IMP-009 is the heavy cargo test on the Mac.
# 001 picks speech-short.wav in the portal file dialog that xdg-desktop-portal-gtk shows; every
# other file goes in through the hook action import-add (the same call a drop makes). Words are
# checked like the other suites do: at least 90% of the expected words, with one slip allowed.
# Needs the fixtures in /assets/fixtures, speech-short.aiff and speech-short-alac.m4a in /fixtures,
# and the base.en and base models in /assets/models/whisper.
# Busy: it transcribes a 10 minute file. It empties /data and restarts the app, so the slot must
# not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

DB=/data/history/history.db
OUT=/out
EXTRA=/fixtures
DIALOG_NAMES='^(Open File|Open Files|Select Files|Choose files|Choose audio files|zenity)$'

q() { sqlite3 "$DB" "$1"; }
imp() { $HC state | jq -r ".import$1"; }
item() { # <name> <jq field>: the newest queue item with that file name
  $HC state | jq -r --arg name "$1" "[.import.items[] | select(.name == \$name)] | last | $2"
}
item_is() { [ "$(item "$1" .state)" = "$2" ]; }
item_final() { case "$(item "$1" .state)" in done | failed | cancelled) return 0 ;; esac; return 1; }
queue_idle() { [ "$(imp .busy)" = false ]; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }
dialog() { xdotool search --onlyvisible --name "$DIALOG_NAMES" 2>/dev/null | head -1; }
dialog_open() { [ -n "$(dialog)" ]; }
dialog_gone() { [ -z "$(dialog)" ]; }
focus() { xdotool windowfocus --sync "$1"; }
ievents() { # since T0: "<t_ms> <detail>" lines of the import events
  $HC events | jq -r --argjson t0 "${T0:-0}" '.[] | select(.kind == "import" and .t_ms >= $t0) | "\(.t_ms) \(.detail)"'
}
words_of() { norm <<<"$1" | xargs; }
add() { # <path...>: like a drop on the view
  T0=$(now_ms)
  local paths
  paths=$(printf '%s\n' "$@" | jq -R . | jq -sc .)
  $HC action import-add "{\"paths\":$paths}"
}
import_one() { # <path>: adds one file and waits for its end
  local name
  name=$(basename "$1")
  add "$1" >/dev/null
  wait_until 300 item_final "$name"
}
text_of() { item "$1" .text; }
open_view() {
  focus "$(app_window)"
  $HC click sidebar.import >/dev/null
  wait_until 5 rendered import.view
  sleep 0.3
}
row_of() { q "select id from transcript where source_name = '$1' order by created_at desc limit 1"; }
export_to() { # <row id> <format> <file>
  rm -f "$3"
  $HC paths "$3" >/dev/null
  $HC action history-export "{\"format\":\"$2\",\"ids\":[\"$1\"]}" >/dev/null
  wait_until 20 test -s "$3"
}
ms_clock() {
  awk -v ms="$1" -v sep="$2" 'BEGIN {
    printf "%02d:%02d:%02d%s%03d", ms / 3600000, (ms / 60000) % 60, (ms / 1000) % 60, sep, ms % 1000
  }'
}

echo "== setup: base.en and base, the Import view"
fresh base.en base
use_model base.en
open_view
for file in "$FIX/speech-short.wav" "$FIX/speech-short.mp3" "$FIX/speech-short.flac" "$FIX/speech-short.m4a" \
  "$FIX/speech-short.ogg" "$EXTRA/speech-short.aiff" "$EXTRA/speech-short-alac.m4a" \
  "$FIX/dictation-45s.wav" "$FIX/long-600s.wav" "$FIX/speech-es.wav" "$FIX/speech-short.opus" \
  "$FIX/speech-short.webm" "$FIX/speech-short.mp4" "$FIX/corrupt-random.mp3" \
  "$FIX/corrupt-text-as.wav" "$FIX/empty.wav"; do
  [ -e "$file" ] || echo "missing fixture $file"
done

echo "== VAL-IMP-001 WAV import through the file dialog"
failures=""
T0=$(now_ms)
shot imp-001-before
$HC click import.choose >/dev/null
wait_until 20 dialog_open || failures+="no file dialog appeared; "
sleep 1
shot imp-001-dialog
wid=$(dialog)
if [ -n "$wid" ]; then
  xdotool windowfocus --sync "$wid"
  sleep 0.5
  xdotool key ctrl+l
  sleep 0.3
  xdotool type --delay 20 "$FIX/speech-short.wav"
  sleep 0.3
  xdotool key Return
fi
wait_until 10 dialog_gone || failures+="the dialog stayed open; "
seen=""
deadline=$((SECONDS + 120))
while [ $SECONDS -le $deadline ]; do
  state=$(item speech-short.wav '"\(.state):\(.progress)"')
  case " $seen " in *" $state "*) ;; *) seen+="$state " ;; esac
  case "$state" in done:* | failed:* | cancelled:*) break ;; esac
  sleep 0.1
done
shot imp-001-after
item_is speech-short.wav done || failures+="the item ended '$(item speech-short.wav .state)': $(item speech-short.wav .message); "
text=$(words_of "$(text_of speech-short.wav)")
missed=$(word_recall_ok "$text" "${SHORT_WORDS[@]}") || failures+="missing words: $missed; "
ievents | grep -q ' queued speech-short.wav' || failures+="no queued event; "
ievents | grep -q ' done speech-short.wav' || failures+="no done event; "
[ "$(item speech-short.wav .progress)" = 100 ] || failures+="progress ended at $(item speech-short.wav .progress); "
$HC tree >"$RUN_OUT/imp-001-tree.json"
check_failures "VAL-IMP-001 WAV through the dialog" "the dialog took the path; states seen: $seen; text '$text'" "$failures"

echo "== VAL-IMP-008 the row in History"
failures=""
id=$(row_of speech-short.wav)
[ -n "$id" ] || failures+="no transcript row for speech-short.wav; "
[ "$(q "select kind || '|' || status || '|' || source_name from transcript where id = '$id'")" = 'import|completed|speech-short.wav' ] ||
  failures+="the row reads '$(q "select kind, status, source_name from transcript where id = '$id'")'; "
[ "$(q "select count(*) from segment where transcript_id = '$id'")" -ge 1 ] || failures+="the row has no segments; "
[ -z "$(q "select audio_path from transcript where id = '$id'")" ] || failures+="the row keeps an audio path; "
focus "$(app_window)"
$HC click sidebar.history >/dev/null
wait_until 5 rendered history.search
$HC action history-search '{"query":"speech-short"}' >/dev/null
sleep 0.6
shot imp-008-search
[ "$($HC state | jq -r '.history.rows | map(select(.id == "'"$id"'")) | length')" = 1 ] || failures+="the search for speech-short did not find the row; "
meta=$($HC state | jq -r '.history.rows[0].meta')
case "$meta" in *Import*speech-short.wav*) ;; *) failures+="the list line reads '$meta'; " ;; esac
$HC action history-open "{\"id\":\"$id\"}" >/dev/null
wait_until 5 rendered history.detail
sleep 0.4
shot imp-008-detail
nseg=$($HC state | jq -r '.history.detail.segments | length')
[ "$nseg" = "$(q "select count(*) from segment where transcript_id = '$id'")" ] || failures+="the detail lists $nseg segments; "
[ "$($HC state | jq -r '.history.detail.segments[0] | has("start_ms") and has("end_ms")')" = true ] || failures+="a segment has no times; "
rendered history.detail.segment.0 || failures+="no segment row is drawn; "
[ "$($HC state | jq -r '.history.detail.audio')" = false ] || failures+="the detail says audio is there; "
$HC click history.reprocess >/dev/null 2>&1 && failures+="a click on Reprocess was accepted; "
[ "$($HC state | jq -r '.history.reprocess')" = null ] || failures+="a reprocess started; "
rendered history.audio-missing || failures+="no note says why Reprocess is off; "
rendered history.export || failures+="no Export button; "
$HC action history-close >/dev/null
$HC action history-search '{"query":""}' >/dev/null
check_failures "VAL-IMP-008 History row" "row $id is kind import, completed, source speech-short.wav; search found it; the detail lists $nseg segments with times, Reprocess off, Export offered" "$failures"

echo "== VAL-IMP-002 six more formats, one at a time"
failures=""
detail=""
open_view
for file in "$FIX/speech-short.mp3" "$FIX/speech-short.flac" "$FIX/speech-short.m4a" "$FIX/speech-short.ogg" \
  "$EXTRA/speech-short.aiff" "$EXTRA/speech-short-alac.m4a"; do
  name=$(basename "$file")
  import_one "$file" || failures+="$name did not end; "
  if ! item_is "$name" done; then
    failures+="$name ended '$(item "$name" .state)' $(item "$name" .code): $(item "$name" .message); "
    continue
  fi
  text=$(words_of "$(text_of "$name")")
  missed=$(word_recall_ok "$text" "${SHORT_WORDS[@]}") || failures+="$name misses: $missed; "
  detail+="$name ok; "
done
shot imp-002-finished
grep -E 'speech-short\.(aiff)|alac' "$EXTRA/MANIFEST.sha256" >"$RUN_OUT/imp-002-manifest.txt"
[ "$(wc -l <"$RUN_OUT/imp-002-manifest.txt")" = 2 ] || failures+="MANIFEST.sha256 does not list both generated files; "
check_failures "VAL-IMP-002 six formats" "$detail" "$failures"

echo "== VAL-CROSS-003 SRT of an imported MP3"
failures=""
id=$(row_of speech-short.mp3)
export_to "$id" srt "$OUT/import.srt" || failures+="import.srt was not written; "
expected=""
index=1
while IFS='|' read -r start end; do
  expected+="$index|$(ms_clock "$start" ,) --> $(ms_clock "$end" ,)"$'\n'
  index=$((index + 1))
done < <(q "select start_ms, end_ms from segment where transcript_id = '$id' order by idx")
actual=$(awk '/^[0-9]+$/ {num = $0; next} /-->/ {print num "|" $0}' "$OUT/import.srt")
[ "$actual" = "${expected%$'\n'}" ] || failures+="cue times '$(tr '\n' ';' <<<"$actual")' differ from SQLite '$(tr '\n' ';' <<<"$expected")'; "
[ "$(head -1 "$OUT/import.srt")" = 1 ] || failures+="the first index is $(head -1 "$OUT/import.srt"); "
missed=$(word_recall_ok "$(words_of "$(grep -v -e '-->' -e '^[0-9]*$' "$OUT/import.srt")")" "${SHORT_WORDS[@]}") || failures+="the SRT text misses: $missed; "
cp "$OUT/import.srt" "$RUN_OUT/cross-003.srt"
check_failures "VAL-CROSS-003 SRT export" "$(grep -c -- '-->' "$OUT/import.srt") cue(s) with the times of the segment rows, text complete" "$failures"

echo "== VAL-IMP-006 Opus, WebM, and video"
failures=""
ui=$(ui_pid)
epid=$(engine .pid)
detail=""
for name in speech-short.opus speech-short.webm speech-short.mp4; do
  started=$(now_ms)
  import_one "$FIX/$name" || failures+="$name did not end within 300 s; "
  took=$(($(now_ms) - started))
  [ "$took" -lt 10000 ] || failures+="$name took $took ms; "
  [ "$(item "$name" .state)" = failed ] || failures+="$name ended '$(item "$name" .state)'; "
  [ "$(item "$name" .code)" = IMPORT_UNSUPPORTED_CODEC ] || failures+="$name code is $(item "$name" .code); "
  message=$(item "$name" .message)
  case "$message" in *[Oo]pus* | *[Ww]eb[Mm]* | *[Vv]ideo* | *MP4* | *mp4*) ;; *) failures+="$name message names no format: $message; " ;; esac
  [ "$(q "select count(*) from transcript where source_name = '$name'")" = 0 ] || failures+="a row exists for $name; "
  detail+="$name: $(item "$name" .code) '$message'; "
done
shot imp-006-errors
[ "$(ui_pid)" = "$ui" ] && kill -0 "$ui" 2>/dev/null || failures+="the UI pid changed; "
[ "$(engine .state)" = ready ] && [ "$(engine .pid)" = "$epid" ] || failures+="the engine is $(engine .state) pid $(engine .pid), was $epid; "
check_failures "VAL-IMP-006 unsupported formats" "$detail UI pid $ui unchanged, engine $epid ready" "$failures"

echo "== VAL-IMP-007 corrupt and empty files"
failures=""
detail=""
for name in corrupt-random.mp3 corrupt-text-as.wav empty.wav; do
  import_one "$FIX/$name" || failures+="$name did not end; "
  [ "$(item "$name" .state)" = failed ] || failures+="$name ended '$(item "$name" .state)'; "
  [ "$(item "$name" .code)" = IMPORT_DECODE_FAILED ] || failures+="$name code is $(item "$name" .code); "
  [ "$(q "select count(*) from transcript where source_name = '$name'")" = 0 ] || failures+="a row exists for $name; "
  detail+="$name: $(item "$name" .code); "
done
shot imp-007-errors
[ "$(ui_pid)" = "$ui" ] && kill -0 "$ui" 2>/dev/null || failures+="the UI pid changed; "
wait_until 60 engine_ready_with base.en || failures+="the engine is not ready again; "
import_one "$FIX/speech-short.wav" || failures+="the next import did not end; "
item_is speech-short.wav done || failures+="the next import ended '$(item speech-short.wav .state)'; "
missed=$(word_recall_ok "$(words_of "$(text_of speech-short.wav)")" "${SHORT_WORDS[@]}") || failures+="the next import misses: $missed; "
cp "$ENGINE_LOG" "$RUN_OUT/imp-007-engine.log" 2>/dev/null
check_failures "VAL-IMP-007 decode errors" "$detail UI pid $ui unchanged; a following WAV import completed" "$failures"

echo "== VAL-IMP-010 devices and FIFOs"
failures=""
rm -f /tmp/in.wav
mkfifo /tmp/in.wav
epid=$(engine .pid)
strace -f -qq -e trace=openat,open -o "$RUN_OUT/imp-010-open.strace" -p "$(ui_pid)" -p "$epid" >/dev/null 2>&1 &
TRACE=$!
sleep 1
kill -0 "$TRACE" 2>/dev/null || failures+="strace could not attach; "
count_before=$(imp '.items | length')
rows_before=$(q "select count(*) from transcript")
T0=$(now_ms)
$HC action import-add '{"paths":["/dev/zero","/tmp/in.wav"]}' >/dev/null
sleep 1.5
shot imp-010-refused
[ "$(imp '.items | length')" = "$count_before" ] || failures+="a queue item was made; "
[ "$(q "select count(*) from transcript")" = "$rows_before" ] || failures+="a History row was made; "
notice=$(imp .notice)
case "$notice" in *"cannot be opened"*) ;; *) failures+="the notice reads '$notice'; " ;; esac
[ "$(imp .busy)" = false ] || failures+="the queue is busy; "
ievents | grep -c ' refused ' | grep -q '^2$' || failures+="the events do not show 2 refusals; "
import_one "$FIX/speech-short.wav" || failures+="the following import did not end; "
item_is speech-short.wav done || failures+="the following import ended '$(item speech-short.wav .state)'; "
missed=$(word_recall_ok "$(words_of "$(text_of speech-short.wav)")" "${SHORT_WORDS[@]}") || failures+="the following import misses: $missed; "
sleep 0.5
kill "$TRACE" 2>/dev/null
wait "$TRACE" 2>/dev/null
grep -F -e '/tmp/in.wav' -e '/dev/zero' "$RUN_OUT/imp-010-open.strace" >"$RUN_OUT/imp-010-hits.txt"
[ ! -s "$RUN_OUT/imp-010-hits.txt" ] || failures+="the path was opened: $(head -c 300 "$RUN_OUT/imp-010-hits.txt"); "
grep -q 'speech-short.wav' "$RUN_OUT/imp-010-open.strace" || failures+="the strace log shows no open of the later WAV, so it saw nothing; "
rm -f /tmp/in.wav
check_failures "VAL-IMP-010 devices and FIFOs" "notice '$notice', no item, no row, no openat of either path in $(wc -l <"$RUN_OUT/imp-010-open.strace") traced calls; the next WAV completed" "$failures"

echo "== VAL-IMP-004 the queue runs files in order"
failures=""
open_view
$HC action import-clear >/dev/null
T0=$(now_ms)
paths=$(printf '%s\n' "$FIX/speech-short.wav" "$FIX/speech-short.mp3" "$FIX/dictation-45s.wav" | jq -R . | jq -sc .)
$HC action import-add "{\"paths\":$paths}" >/dev/null
seen_two=""
shot_two=0
deadline=$((SECONDS + 300))
running_max=0
progress_seen=""
while [ $SECONDS -le $deadline ]; do
  snap=$($HC state | jq -c '.import.items | map(select(.name == "speech-short.wav" or .name == "speech-short.mp3" or .name == "dictation-45s.wav")) | .[-3:]')
  running=$(jq '[.[] | select(.state == "running")] | length' <<<"$snap")
  [ "$running" -gt "$running_max" ] && running_max=$running
  if [ "$shot_two" = 0 ] && [ "$(jq -r '.[1].state' <<<"$snap")" = running ]; then
    shot imp-004-second-running
    shot_two=1
  fi
  progress_seen+="$(jq -r 'map("\(.progress)") | join("/")' <<<"$snap") "
  [ "$(jq '[.[] | select(.state == "done" or .state == "failed" or .state == "cancelled")] | length' <<<"$snap")" = 3 ] && break
  sleep 0.15
done
order=$(ievents | awk '$2 == "start" || $2 == "done" {print $2 ":" $3}' | tr '\n' ' ')
[ "$order" = "start:speech-short.wav done:speech-short.wav start:speech-short.mp3 done:speech-short.mp3 start:dictation-45s.wav done:dictation-45s.wav " ] ||
  failures+="the events read: $order; "
[ "$running_max" -le 1 ] || failures+="$running_max jobs ran at once; "
[ "$shot_two" = 1 ] || failures+="the second item was never seen running; "
for name in speech-short.wav speech-short.mp3 dictation-45s.wav; do
  item_is "$name" done || failures+="$name ended '$(item "$name" .state)'; "
done
shot imp-004-finished
check_failures "VAL-IMP-004 queue order" "events: $order; at most $running_max job running at once; progress samples: $(tr ' ' '\n' <<<"$progress_seen" | sort -u | head -8 | tr '\n' ' ')" "$failures"

echo "== VAL-IMP-005 cancel stops in under 1 s and the queue continues"
failures=""
open_view
$HC action import-clear >/dev/null
epid=$(engine .pid)
T0=$(now_ms)
$HC action import-add "{\"paths\":[\"$FIX/long-600s.wav\",\"$FIX/speech-short.wav\"]}" >/dev/null
wait_until 60 item_is long-600s.wav running || failures+="the long file never ran; "
wait_until 90 sh -c "[ \"\$($HC state | jq -r '[.import.items[] | select(.name == \"long-600s.wav\")] | last | .progress')\" -gt 5 ]" || failures+="the long file showed no progress; "
shot imp-005-running
long_id=$(item long-600s.wav .id)
$HC action import-cancel "{\"id\":$long_id}" >/dev/null
wait_until 5 item_is long-600s.wav cancelled || failures+="the item did not reach cancelled in 5 s; "
cancel_t=$(ievents | awk '$2 == "cancel" && $3 == "long-600s.wav" {print $1; exit}')
done_t=$(ievents | awk '$2 == "cancelled" && $3 == "long-600s.wav" {print $1; exit}')
gap=$((done_t - cancel_t))
[ "${gap:-99999}" -lt 1000 ] || failures+="the cancel took $gap ms; "
shot imp-005-cancelled
wait_until 120 item_final speech-short.wav || failures+="the queued file did not finish; "
item_is speech-short.wav done || failures+="the queued file ended '$(item speech-short.wav .state)'; "
missed=$(word_recall_ok "$(words_of "$(text_of speech-short.wav)")" "${SHORT_WORDS[@]}") || failures+="the queued file misses: $missed; "
[ "$(q "select count(*) from transcript where source_name = 'long-600s.wav' and status = 'completed'")" = 0 ] || failures+="a completed row exists for long-600s.wav; "
new_pid=$(engine .pid)
[ "$new_pid" = "$epid" ] || echo "engine pid changed from $epid to $new_pid (state $(engine .state))" >"$RUN_OUT/imp-005-engine-pid.txt"
check_failures "VAL-IMP-005 cancel" "the cancel reached cancelled in $gap ms; the queued WAV completed; no completed row for long-600s.wav; engine pid $epid then $new_pid" "$failures"

echo "== VAL-IMP-003 a 10 minute file keeps every word"
failures=""
open_view
import_one "$FIX/long-600s.wav" || failures+="the long file did not end in 300 s; "
item_is long-600s.wav done || failures+="the long file ended '$(item long-600s.wav .state)' $(item long-600s.wav .message); "
id=$(row_of long-600s.wav)
export_to "$id" json "$OUT/long.json" || failures+="long.json was not written; "
cp "$OUT/long.json" "$RUN_OUT/imp-003-row.json"
text=$(jq -r '.rows[0].final_text' "$OUT/long.json" | norm)
lighthouse=$(grep -o 'lighthouse' <<<"$text" | wc -l)
harbor=$(grep -o -e 'harbor' -e 'harbour' <<<"$text" | wc -l)
[ "$lighthouse" -ge 11 ] && [ "$lighthouse" -le 13 ] || failures+="lighthouse appears $lighthouse times; "
[ "$harbor" -ge 11 ] && [ "$harbor" -le 13 ] || failures+="harbor appears $harbor times; "
[ "$(q "select count(*) from segment where transcript_id = '$id'")" -ge 20 ] || failures+="few segments; "
increasing=$(q "select start_ms from segment where transcript_id = '$id' order by idx" | awk 'NR > 1 && $1 <= prev {bad = 1} {prev = $1} END {print bad ? "no" : "yes"}')
[ "$increasing" = yes ] || failures+="segment starts do not increase; "
last_end=$(q "select max(end_ms) from segment where transcript_id = '$id'")
[ "${last_end:-0}" -gt 590000 ] || failures+="the last segment ends at $last_end ms; "
grep 'window ' "$ENGINE_LOG" | tail -6 >"$RUN_OUT/imp-003-windows.log"
[ "$(grep -c 'window [0-9]*/6' "$RUN_OUT/imp-003-windows.log")" = 6 ] || failures+="the engine log does not show 6 windows; "
grep -q 'window 1/6 0..120 s, overlap 10 s' "$RUN_OUT/imp-003-windows.log" || failures+="no 120 s window with 10 s overlap in the log; "
grep -q 'window 2/6 110..230 s' "$RUN_OUT/imp-003-windows.log" || failures+="the second window does not start at 110 s; "
check_failures "VAL-IMP-003 long file" "lighthouse $lighthouse, harbor $harbor (11 to 13 each); starts strictly increase; last segment ends at $last_end ms; 6 windows of 120 s with 10 s overlap in engine.log" "$failures"

echo "== VAL-CROSS-004 a Spanish import keeps its language"
failures=""
use_model base || failures+="base did not load; "
hook_action dictation-language '{"code":"auto"}' >/dev/null
open_view
import_one "$FIX/speech-es.wav" || failures+="speech-es.wav did not end; "
item_is speech-es.wav done || failures+="speech-es.wav ended '$(item speech-es.wav .state)' $(item speech-es.wav .message); "
text=$(words_of "$(text_of speech-es.wav)")
case "$text" in *paulina*) ;; *) failures+="the text has no Paulina: $text; " ;; esac
id=$(row_of speech-es.wav)
[ "$(q "select language_detected from transcript where id = '$id'")" = es ] || failures+="the row says $(q "select language_detected from transcript where id = '$id'"); "
focus "$(app_window)"
$HC click sidebar.history >/dev/null
wait_until 5 rendered history.search
$HC action history-open "{\"id\":\"$id\"}" >/dev/null
wait_until 5 rendered history.detail
sleep 0.4
shot cross-004-detail
[ "$($HC state | jq -r '.history.detail.language')" = es ] || failures+="the detail shows $($HC state | jq -r '.history.detail.language'); "
export_to "$id" json "$OUT/es.json" || failures+="es.json was not written; "
cp "$OUT/es.json" "$RUN_OUT/cross-004-row.json"
[ "$(jq -r '.rows[0].language_detected' "$OUT/es.json")" = es ] || failures+="the JSON language is $(jq -r '.rows[0].language_detected' "$OUT/es.json"); "
check_failures "VAL-CROSS-004 language" "text '$text'; the row, the detail, and the JSON export say es" "$failures"

echo "== the flow bar watcher thread does not leak over 20 hide and show cycles"
failures=""
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
bar_visible() { [ "$($HC state | jq -r '.overlay.visible')" = "$1" ]; }
threads() { ls "/proc/$(ui_pid)/task" | wc -l; }
fds() { ls "/proc/$(ui_pid)/fd" | wc -l; }
set_setting overlay.idleVisible true
wait_until 5 bar_visible true || failures+="the bar is not visible; "
for _ in 1 2 3; do
  set_setting overlay.idleVisible false
  wait_until 5 bar_visible false
  set_setting overlay.idleVisible true
  wait_until 5 bar_visible true
done
sleep 1
pins() { cat /data/logs/*.log /logs/app.log 2>/dev/null | grep -c 'flow bar pinned on top'; }
base_threads=$(threads)
base_fds=$(fds)
base_pins=$(pins)
for _ in $(seq 20); do
  set_setting overlay.idleVisible false
  wait_until 5 bar_visible false || failures+="the bar did not hide; "
  set_setting overlay.idleVisible true
  wait_until 5 bar_visible true || failures+="the bar did not show; "
done
sleep 2
end_threads=$(threads)
end_fds=$(fds)
new_pins=$(($(pins) - base_pins))
[ "$new_pins" -ge 20 ] || failures+="the watcher started $new_pins times in 20 cycles, so the count proves nothing; "
[ "$end_threads" -le $((base_threads + 1)) ] || failures+="threads grew from $base_threads to $end_threads; "
[ "$end_fds" -le $((base_fds + 3)) ] || failures+="open files grew from $base_fds to $end_fds; "
# The thread name is cut to 15 characters in /proc.
watchers=$(grep -l '^x11-flow-bar' /proc/"$(ui_pid)"/task/*/comm 2>/dev/null | wc -l)
[ "$watchers" -le 1 ] || failures+="$watchers watcher threads are alive; "
echo "$watchers" >"$RUN_OUT/watcher-threads.txt"
check_failures "X11 watcher threads" "threads $base_threads before and $end_threads after 20 cycles ($new_pins watchers started); open files $base_fds and $end_fds; $(cat "$RUN_OUT/watcher-threads.txt") watcher thread(s) alive" "$failures"

finish import
