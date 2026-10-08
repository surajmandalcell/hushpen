#!/bin/bash
# Audio retention and playback end to end: VAL-AUD-001 to 006 (the fake-clock test of the daily
# sweep in 005 is the cargo test hushpen-store retention::tests::*schedule*).
# The hold key is Right Alt (X11 keycode 108); the words come from the virtual microphone. The
# rows of the retention checks are seeded with sqlite3 and carry a copy of the audio that the
# dictation of 001 kept, backdated by whole days. The output sink is cues, read as cues.monitor.
# Needs speech-short.wav in /assets/fixtures and the tiny.en model in /assets/models/whisper.
# Busy: it transcribes. It empties /data and restarts the app, so the slot must not be shared.
# Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
SHORT_WAV=$FIX/speech-short.wav
DAY_MS=86400000

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
gtk_commit() {
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
col() { q "select coalesce($2, '') from transcript where id = '$1'"; }
newest() { q "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
hs() { $HC state | jq -r ".history$1"; }
row_ids() { $HC state | jq -r '[.history.rows[].id] | join(",")'; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }
tree_field() { $HC tree | jq -r ".[] | select(.id == \"$1\") | $2"; }
alive() { kill -0 "$(ui_pid)" 2>/dev/null; }
count_is() { [ "$(count)" = "$1" ]; }
playing_moved() { [ "$(hs .playback.playing)" = true ] && [ "$(hs .playback.position_ms)" != 0 ]; }
engine_up() { wait_until 90 engine_ready_with "$1"; }
history_search() { hook_action history-search "{\"query\":\"$1\"}" >/dev/null; sleep 0.5; }
record() { # <source> <seconds> <wav>
  timeout -s INT "$2" parecord --device="$1" --rate=16000 --channels=1 --format=s16le \
    --file-format=wav "$3" 2>/dev/null
}
stat_of() { python3 /harness/slot/wavstat.py "$1"; }
wav_info() { # <wav> -> "rate channels bits milliseconds"
  python3 -c '
import sys, wave
with wave.open(sys.argv[1], "rb") as w:
    print(w.getframerate(), w.getnchannels(), w.getsampwidth() * 8, round(w.getnframes() * 1000 / w.getframerate()))' "$1"
}
within() { # <a> <b> <tolerance>
  awk -v a="$1" -v b="$2" -v t="$3" 'BEGIN {d = a - b; if (d < 0) d = -d; exit !(d <= t)}'
}

open_history() {
  focus "$(app_window)"
  $HC click sidebar.history >/dev/null
  wait_until 5 rendered history.search
  sleep 0.4
}
open_detail() { # <id>
  open_history
  hook_action history-open "{\"id\":\"$1\"}" >/dev/null
  wait_until 5 rendered history.detail
  sleep 0.4
}

ptt_run() { # <wav>
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

set_retention() { # <30d|never|forever>: the app is stopped, so it does not write over the file
  jq --arg v "$1" '.values["history.audioRetention"] = $v' "$SETTINGS" >"$SETTINGS.tmp" &&
    cat "$SETTINGS.tmp" >"$SETTINGS" && rm -f "$SETTINGS.tmp"
}
retention_of() { jq -r '.values["history.audioRetention"] // "30d"' "$SETTINGS"; }

# seed_row <id> <days old> <word>: a completed row with a copy of the kept audio
seed_row() {
  local created=$(($(date +%s) * 1000 - $2 * DAY_MS))
  cp "$SOURCE_WAV" "/data/audio/$1.wav"
  q "insert into transcript (id, created_at, kind, status, duration_ms, model_id, language_detected,
       raw_text, rule_text, final_text, insert_outcome, audio_path)
     values ('$1', $created, 'dictation', 'completed', $SOURCE_MS, 'tiny.en', 'en',
       'the $3 row', 'the $3 row', 'the $3 row', 'pasted', 'audio/$1.wav')"
}
restart_app() {
  /harness/slot/app.sh stop
  start_app
}

echo "== setup: tiny.en and the paste targets"
fresh tiny.en
use_model tiny.en
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-AUD-001 a dictation keeps its audio as a 16 kHz mono 16-bit WAV"
failures=""
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
wait_until 10 count_is 1 || failures+="the table holds $(count) rows; "
AUD1=$(newest id)
audio_path=$(newest audio_path)
[ "$audio_path" = "audio/$AUD1.wav" ] || failures+="audio_path is '$audio_path', not audio/$AUD1.wav; "
[ -f "/data/$audio_path" ] || failures+="no file at /data/$audio_path; "
read -r rate channels bits wav_ms <<<"$(wav_info "/data/$audio_path")"
[ "$rate $channels $bits" = "16000 1 16" ] || failures+="the WAV is $rate Hz, $channels channels, $bits bits; "
row_ms=$(newest duration_ms)
within "$wav_ms" "$row_ms" 100 || failures+="the WAV is $wav_ms ms and the row says $row_ms ms; "
[ "$(retention_of)" = 30d ] || failures+="the default retention is $(retention_of), not 30d; "
stat_of "/data/$audio_path" | tee "$RUN_OUT/aud-001-wavstat.txt"
grep -q 'silent=false' "$RUN_OUT/aud-001-wavstat.txt" || failures+="the kept audio is silent; "
SOURCE_WAV=$RUN_OUT/source.wav
cp "/data/$audio_path" "$SOURCE_WAV"
SOURCE_MS=$row_ms
ls -l /data/audio >"$RUN_OUT/aud-001-audio.txt"
check_failures "VAL-AUD-001 audio kept" "audio/$AUD1.wav is $rate Hz mono ${bits}-bit, $wav_ms ms against the row's $row_ms ms" "$failures"

echo "== VAL-AUD-002 Play moves the position, the sink carries sound, the bar seeks, Pause stops"
failures=""
open_detail "$AUD1"
rendered history.play || failures+="no Play control; "
rendered history.seek || failures+="no seek bar; "
[ "$(tree_field history.play .text)" = Play ] || failures+="the button reads '$(tree_field history.play .text)'; "
shot aud-002-detail
record cues.monitor 5 "$RUN_OUT/aud-002-sink.wav" &
rec_pid=$!
sleep 0.5
$HC click history.play >/dev/null
wait_until 5 playing_moved || failures+="the position never moved after Play; "
p0=$(hs .playback.position_ms)
t0=$(now_ms)
sleep 2
p1=$(hs .playback.position_ms)
t1=$(now_ms)
shot aud-002-playing
grew=$((p1 - p0))
[ "$grew" -ge 1500 ] && [ "$grew" -le 2500 ] || failures+="the position grew $grew ms over $((t1 - t0)) ms; "
[ "$(tree_field history.play .text)" = Pause ] || failures+="the button reads '$(tree_field history.play .text)' while it plays; "
time_label=$(tree_text history.time)
case "$time_label" in [0-9]*:[0-9][0-9]\ /\ [0-9]*:[0-9][0-9]) ;; *) failures+="the time label reads '$time_label'; " ;; esac
duration=$(hs .playback.duration_ms)
bar=$($HC tree | jq -c '.[] | select(.id == "history.seek") | .root_bounds')
bx=$(jq -r '(.x + .width / 2) | floor' <<<"$bar")
by=$(jq -r '(.y + .height / 2) | floor' <<<"$bar")
xdotool mousemove "$bx" "$by" click 1
sleep 0.3
after_seek=$(hs .playback.position_ms)
shot aud-002-seeked
tolerance=$((duration / 10))
within "$after_seek" $((duration / 2)) "$tolerance" || failures+="after the click at the bar's middle the position is $after_seek of $duration ms; "
wait "$rec_pid"
stat_of "$RUN_OUT/aud-002-sink.wav" | tee "$RUN_OUT/aud-002-sink-stat.txt"
grep -q 'silent=false' "$RUN_OUT/aud-002-sink-stat.txt" || failures+="cues.monitor was silent while the audio played; "
$HC click history.play >/dev/null
sleep 0.3
paused_at=$(hs .playback.position_ms)
sleep 1
later=$(hs .playback.position_ms)
[ "$(hs .playback.playing)" = false ] || failures+="playing is $(hs .playback.playing) after Pause; "
within "$paused_at" "$later" 20 || failures+="the position moved from $paused_at to $later after Pause; "
[ "$(tree_field history.play .text)" = Play ] || failures+="the button reads '$(tree_field history.play .text)' after Pause; "
shot aud-002-paused
alive || failures+="the app is gone; "
check_failures "VAL-AUD-002 playback" "the position grew $grew ms in 2 s (of $duration ms), sink peak not silent, the click at the middle went to $after_seek ms, Pause held at $paused_at ms" "$failures"
$HC click history.back >/dev/null

echo "== VAL-AUD-003 the default sweep removes audio older than 30 days and keeps the text"
failures=""
/harness/slot/app.sh stop
seed_row OLD31 31 lighthouse31
seed_row NEW29 29 meadow29
text31=$(col OLD31 final_text)
start_app
wait_until 20 test ! -e /data/audio/OLD31.wav || failures+="the 31-day WAV is still there; "
sleep 1
ls -l /data/audio >"$RUN_OUT/aud-003-audio.txt"
[ ! -e /data/audio/OLD31.wav ] || failures+="the 31-day WAV exists; "
[ -e /data/audio/NEW29.wav ] || failures+="the 29-day WAV is gone; "
[ -e "/data/audio/$AUD1.wav" ] || failures+="the new dictation's WAV is gone; "
[ "$(q "select audio_path is null and audio_removed_at is not null from transcript where id = 'OLD31'")" = 1 ] || failures+="OLD31 audio_path/audio_removed_at: $(q "select coalesce(audio_path,'NULL') || '/' || coalesce(audio_removed_at,'NULL') from transcript where id = 'OLD31'"); "
[ "$(q "select audio_path from transcript where id = 'NEW29'")" = audio/NEW29.wav ] || failures+="NEW29 lost its audio_path; "
[ "$(col OLD31 final_text)" = "$text31" ] || failures+="the 31-day text changed; "
q "select id, audio_path, audio_removed_at from transcript order by created_at" >"$RUN_OUT/aud-003-rows.txt"
open_history
history_search lighthouse31
[ "$(row_ids)" = OLD31 ] || failures+="search lists '$(row_ids)' for lighthouse31; "
history_search ""
old_index=$($HC state | jq -r '[.history.rows[].id] | index("OLD31")')
[ "$($HC state | jq -r '.history.rows[] | select(.id == "OLD31") | .audio_removed')" = true ] || failures+="the list does not flag OLD31; "
shot aud-003-list
open_detail OLD31
shot aud-003-detail-31
rendered history.play && failures+="the 31-day detail has a Play control; "
case "$(tree_text history.audio-missing)" in *"Audio removed"*) ;; *) failures+="the 31-day detail says '$(tree_text history.audio-missing)'; " ;; esac
[ "$(tree_text history.detail.final)" = "$text31" ] || failures+="the 31-day detail shows '$(tree_text history.detail.final)'; "
open_detail NEW29
shot aud-003-detail-29
rendered history.play || failures+="the 29-day detail has no Play control; "
rendered history.audio-missing && failures+="the 29-day detail says its audio is missing; "
$HC state | jq '{settings: .settings, history: {rows: .history.rows, detail: .history.detail}}' >"$RUN_OUT/aud-003-state.json"
check_failures "VAL-AUD-003 default sweep" "retention $(retention_of): the 31-day WAV is gone with audio_path NULL and audio_removed_at set, text unchanged and found by search, 'Audio removed' and no Play; the 29-day WAV and Play remain (list index of OLD31: $old_index)" "$failures"
$HC click history.back >/dev/null

echo "== VAL-AUD-006 missing audio is handled without a crash"
failures=""
rm -f "/data/audio/$AUD1.wav"
open_detail "$AUD1"
shot aud-006-deleted-by-hand
[ "$(hs .detail.audio)" = false ] || failures+="the state says the deleted file is available; "
rendered history.play && failures+="Play shows for the deleted file; "
note=$(tree_text history.audio-missing)
[ -n "$note" ] || failures+="no audio-missing note for the deleted file; "
[ "$(tree_field history.reprocess .enabled)" = false ] || failures+="Reprocess is enabled for the deleted file; "
hook_action history-play "{\"id\":\"$AUD1\"}" >/dev/null 2>&1 && failures+="history-play did not refuse; "
alive || failures+="the app is gone after the play attempt; "
open_detail OLD31
shot aud-006-swept
swept_note=$(tree_text history.audio-missing)
case "$swept_note" in *"Audio removed"*) ;; *) failures+="the swept row's note is '$swept_note'; " ;; esac
rendered history.play && failures+="Play shows for the swept row; "
[ "$(tree_field history.reprocess .enabled)" = false ] || failures+="Reprocess is enabled for the swept row; "
$HC tree >"$RUN_OUT/aud-006-tree.json"
ps -o pid,stat,etime,cmd -p "$(ui_pid)" >"$RUN_OUT/aud-006-ps.txt"
alive || failures+="the app is gone; "
check_failures "VAL-AUD-006 missing audio" "hand-deleted: '$note'; swept: '$swept_note'; no Play, Reprocess disabled, app pid $(ui_pid) up" "$failures"
$HC click history.back >/dev/null

echo "== VAL-AUD-004 forever keeps old audio, never keeps none"
failures=""
/harness/slot/app.sh stop
set_retention forever
seed_row OLD400 400 harbor400
seed_row OLD40 40 orchard40
start_app
sleep 4
ls -l /data/audio >"$RUN_OUT/aud-004-forever-audio.txt"
[ "$(retention_of)" = forever ] || failures+="the setting is $(retention_of); "
[ -e /data/audio/OLD400.wav ] || failures+="the 400-day WAV is gone with forever; "
[ -e /data/audio/OLD40.wav ] || failures+="the 40-day WAV is gone with forever; "
[ "$(col OLD400 audio_path)" = audio/OLD400.wav ] && [ "$(col OLD400 audio_removed_at)" = "" ] || failures+="the 400-day row changed; "
open_detail OLD400
shot aud-004-forever-detail
rendered history.play || failures+="the 400-day detail has no Play control; "
$HC click history.back >/dev/null

# 005 first half: the same rows, retention now 30d
echo "== VAL-AUD-005 a retention change from forever to 30d applies at the next start"
failures5=""
/harness/slot/app.sh stop
set_retention 30d
start_app
wait_until 20 test ! -e /data/audio/OLD40.wav || failures5+="the 40-day WAV is still there after the change; "
sleep 1
ls -l /data/audio >"$RUN_OUT/aud-005-audio.txt"
[ ! -e /data/audio/OLD40.wav ] || failures5+="the 40-day WAV exists; "
[ ! -e /data/audio/OLD400.wav ] || failures5+="the 400-day WAV exists; "
[ -e /data/audio/NEW29.wav ] || failures5+="the 29-day WAV was removed; "
[ "$(q "select audio_path is null and audio_removed_at is not null from transcript where id = 'OLD40'")" = 1 ] || failures5+="OLD40 still has an audio_path; "
[ "$(col OLD40 final_text)" = "the orchard40 row" ] || failures5+="the 40-day text changed; "
open_detail OLD40
shot aud-005-detail
case "$(tree_text history.audio-missing)" in *"Audio removed"*) ;; *) failures5+="the 40-day detail says '$(tree_text history.audio-missing)'; " ;; esac
$HC click history.back >/dev/null
check_failures "VAL-AUD-005 change applies at the next sweep" "forever kept the 40-day WAV; after the change to 30d and a restart it is gone and the 29-day WAV stays (the daily repeat is the fake-clock cargo test)" "$failures5"
check_failures "VAL-AUD-004 forever" "with forever the 400-day and 40-day WAVs and their Play controls stayed after a restart" "$failures"

failures=""
/harness/slot/app.sh stop
set_retention never
start_app
engine_up tiny.en || failures+="the engine did not load; "
[ "$(retention_of)" = never ] || failures+="the setting is $(retention_of); "
[ -e /data/audio/NEW29.wav ] && failures+="the 29-day WAV survived the never sweep; "
before=$(count)
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the never run did not end; "
wait_until 10 count_is $((before + 1)) || failures+="the run made $(($(count) - before)) rows; "
NEVER_ID=$(newest id)
[ "$(newest status)" = completed ] || failures+="status is $(newest status); "
[ "$(newest final_text)" != "" ] || failures+="the text was not saved; "
[ "$(newest audio_path)" = "" ] || failures+="audio_path is '$(newest audio_path)'; "
[ ! -e "/data/audio/$NEVER_ID.wav" ] || failures+="the WAV of the never run exists; "
[ -z "$(ls /data/audio/ 2>/dev/null | grep "$NEVER_ID")" ] || failures+="a file for the never run is in the audio folder; "
inserted=$(gtk_commit)
[ -n "$inserted" ] || failures+="the text was not inserted; "
ls -l /data/audio >"$RUN_OUT/aud-004-never-audio.txt"
open_detail "$NEVER_ID"
shot aud-004-never-detail
rendered history.play && failures+="the detail of the never run has a Play control; "
[ "$(tree_text history.detail.final)" = "$(newest final_text)" ] || failures+="the detail shows '$(tree_text history.detail.final)'; "
jq '{retention: .values["history.audioRetention"]}' "$SETTINGS" >"$RUN_OUT/aud-004-settings.json"
alive || failures+="the app is gone; "
check_failures "VAL-AUD-004 never" "with never the new completed row has final text '$(newest final_text)', no audio_path, no WAV, and no Play control" "$failures"
