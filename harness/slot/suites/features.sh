#!/bin/bash
# The newest features in the running app, each with a screenshot: the sidebar order, the Manual
# control in the title bar, the Audio enhance row and its sheet, a dictation fed with
# `hookctl feed-wav`, an instant restart (a new dictation 200 ms after the last one stopped, while
# the old one still transcribes; both rows land in History in the order they were spoken),
# silence skipping (30 s of silence and of faint noise give no words), a Chatterbox Turbo export
# with its Read aloud row in History, and the Manage Voices sheet. A dictation ends in the
# clipboard here, like the Home suite: no paste target is in front.
# Needs base from /assets/models/whisper, the chatterbox-turbo folder from /assets/models/tts, and
# speech-short.wav, speech-es.wav, and dictation-45s.wav in /assets/fixtures. Chatterbox wants
# more than the default slot: start it as `VAL_CPUS=4 VAL_MEM=4g harness/run-suite.sh features
# --slots 1` (a 4 CPU slot counts as 2 busy slots). It empties /data and restarts the app, so the
# slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

DB=/data/history/history.db
TTS_ID=chatterbox-turbo
SENTENCE="The quick brown fox jumps over the lazy dog."
EXPORT=/out/chatterbox-turbo.wav
SILENCE=$RUN_OUT/gen/silence-30s.wav
NOISE=$RUN_OUT/gen/noise-30s.wav

q() { sqlite3 "$DB" "$1"; }
st() { $HC state | jq -r "$1"; }
st_is() { [ "$(st "$1")" = "$2" ]; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }
tree_field() { # <id> <jq expression on the element>
  $HC tree | jq -r --arg id "$1" "[.[] | select(.id == \$id)] | first | $2"
}
rows_are() { [ "$(q "select count(*) from transcript where kind = '$1'")" = "$2" ]; }
open_view() { # <view key>
  $HC click "sidebar.$1" >/dev/null
  wait_until 5 st_is .view "$1"
  sleep 0.4
}
# real_click <id>: a mouse press at the middle of the element, the way a user clicks it
real_click() {
  local x y
  read -r x y <<<"$(tree_field "$1" '"\((.root_bounds.x + .root_bounds.width / 2) | floor) \((.root_bounds.y + .root_bounds.height / 2) | floor)"')"
  [ -n "$x" ] && [ "$x" != null ] || return 1
  xdotool mousemove "$x" "$y" click 1
  sleep 0.4
}
# feed <wav> [lead_ms]: plays the WAV into the capture path of the listening dictation, at real
# time, and returns when the file has ended
feed() {
  local padded secs
  padded=$(pad "$1" "${2:-1000}")
  secs=$(python3 /harness/slot/wavstat.py "$padded" | sed -n 's/.*secs=\([0-9.]*\).*/\1/p')
  $HC feed-wav "$padded" >"$RUN_OUT/feed.json" || return 1
  sleep "$(awk -v s="$secs" 'BEGIN {printf "%.2f", s + 0.4}')"
}
# play_mic <wav>: plays the padded WAV into the virtual microphone and returns when it has ended.
# The instant restart uses this and not feed(): a fed WAV leaves the real microphone at digital
# silence, and after 4 s of that the app does not trust the stream for another recording, so a
# start queued right behind the stop would fail with MIC_UNAVAILABLE.
play_mic() { paplay --device=vmic "$(pad "$1")"; }
dictate_feed() { # <label> <wav> [lead_ms]
  local label=$1
  RUN_T0=$(now_ms)
  hook_action dictation-record >/dev/null || return 1
  wait_until 5 dict_is .state listening || return 1
  shot "$label-listening"
  feed "$2" "${3:-1000}" || return 1
  hook_action dictation-record >/dev/null || return 1
  wait_until 240 dict_final || return 1
  sleep 0.3
  shot "$label-final"
}
event_times() { # since RUN_T0: "<t_ms> <detail>" of the dictation events
  $HC events | jq -r --argjson t0 "$RUN_T0" '.[] | select(.kind == "dictation" and .t_ms >= $t0) | "\(.t_ms) \(.detail)"'
}

echo "== setup: base and the Chatterbox Turbo files, WAVs of silence and noise"
fresh base
/harness/slot/app.sh stop
mkdir -p /data/models/tts "$RUN_OUT/padded" "$RUN_OUT/gen"
cp -r "/assets/models/tts/$TTS_ID" /data/models/tts/
start_app
use_model base
python3 - "$SILENCE" "$NOISE" <<'PY'
import random
import struct
import sys
import wave

rate = 16000
for path, amplitude in ((sys.argv[1], 0), (sys.argv[2], 300)):
    random.seed(7)
    with wave.open(path, "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(rate)
        out.writeframes(b"".join(
            struct.pack("<h", int(random.gauss(0, amplitude)) if amplitude else 0)
            for _ in range(rate * 30)))
PY
wait_until 120 model_ready "$TTS_ID" || echo "the $TTS_ID files did not verify: $(m_state "$TTS_ID")"

echo "== sidebar order"
open_view home
shot feat-01-sidebar
$HC tree >"$RUN_OUT/feat-01-tree.json"
order=$(jq -r '[.[] | select(.id | test("^sidebar\\.(home|history|files|dictionary|models|settings|about)$"))]
  | sort_by(.bounds.y) | map(.text) | join(",")' "$RUN_OUT/feat-01-tree.json")
rows=$(jq -r '[.[] | select(.id | test("^sidebar\\.(home|history|files|dictionary|models|settings|about)$"))]
  | sort_by(.bounds.y) | map("\(.bounds.y | floor)+\(.bounds.height | floor)") | join(" ")' "$RUN_OUT/feat-01-tree.json")
failures=""
[ "$order" = "Home,History,Manual,Dictionary,Models,Settings,About" ] || failures+="the sidebar reads '$order'; "
[ "$rows" = "48+28 78+28 108+28 138+28 168+28 198+28 228+28" ] || failures+="the rows sit at '$rows', not 28 px high 2 px apart from y 48; "
check_failures "sidebar order" "$order, 28 px rows 2 px apart from y 48" "$failures"

echo "== Manual control in the title bar"
open_view files
shot feat-02-manual
$HC tree >"$RUN_OUT/feat-02-tree.json"
failures=""
read -r mode_y mode_h mode_x mode_w <<<"$(jq -r '[.[] | select(.id == "files.mode")] | first | "\(.bounds.y) \(.bounds.height) \(.bounds.x) \(.bounds.width)"' "$RUN_OUT/feat-02-tree.json")"
read -r title_y title_h title_x title_w <<<"$(jq -r '[.[] | select(.id == "view.title")] | first | "\(.bounds.y) \(.bounds.height) \(.bounds.x) \(.bounds.width)"' "$RUN_OUT/feat-02-tree.json")"
read -r drop_y <<<"$(jq -r '[.[] | select(.id == "import.drop")] | first | .bounds.y' "$RUN_OUT/feat-02-tree.json")"
if [ -z "$mode_y" ] || [ "$mode_y" = null ]; then
  failures+="no files.mode element in the tree; "
else
  mode_mid=$(awk -v y="$mode_y" -v h="$mode_h" 'BEGIN {printf "%.1f", y + h / 2}')
  title_mid=$(awk -v y="$title_y" -v h="$title_h" 'BEGIN {printf "%.1f", y + h / 2}')
  pane_mid=$(awk -v x="$mode_x" -v w="$mode_w" 'BEGIN {printf "%.1f", x + w / 2}')
  awk -v m="$mode_mid" 'BEGIN {exit !(m >= 25.5 && m <= 26.5)}' || failures+="the control is centered on y $mode_mid, not the 26 px chrome line; "
  awk -v m="$title_mid" 'BEGIN {exit !(m >= 25 && m <= 26.5)}' || failures+="the view title is centered on y $title_mid; "
  awk -v m="$pane_mid" 'BEGIN {exit !(m >= 585 && m <= 587)}' || failures+="the control is centered on x $pane_mid, not the pane center 586; "
  awk -v y="$mode_y" -v h="$mode_h" 'BEGIN {exit !(y >= 0 && y + h <= 52)}' || failures+="the control leaves the 52 px title bar band ($mode_y + $mode_h); "
  awk -v t="$title_x" -v w="$title_w" -v m="$mode_x" 'BEGIN {exit !(m >= t + w)}' || failures+="the control starts at x $mode_x, before the title ends at $(awk -v t="$title_x" -v w="$title_w" 'BEGIN {print t + w}'); "
  awk -v d="$drop_y" 'BEGIN {exit !(d >= 52)}' || failures+="the content starts at y $drop_y, inside the band; "
fi
[ "$(jq -r '[.[] | select(.id == "view.title")] | first | .text' "$RUN_OUT/feat-02-tree.json")" = Manual ] || failures+="the title reads '$(jq -r '[.[] | select(.id == "view.title")] | first | .text' "$RUN_OUT/feat-02-tree.json")'; "
[ "$(jq -r '[.[] | select(.id | test("^files\\.mode\\.(transcribe|readAloud)$"))] | sort_by(.bounds.x) | map(.text) | join(",")' "$RUN_OUT/feat-02-tree.json")" = "Transcribe,Read aloud" ] ||
  failures+="the segments read something else; "
# A press on the control must pick the segment, not drag the window.
real_click files.mode.readAloud
wait_until 5 st_is .files.mode readAloud || failures+="a real click on Read aloud left the mode on $(st .files.mode); "
shot feat-02-manual-read-aloud
rendered read.text || failures+="Read aloud shows no text area; "
real_click files.mode.transcribe
wait_until 5 st_is .files.mode transcribe || failures+="a real click on Transcribe left the mode on $(st .files.mode); "
check_failures "Manual control in the title bar" "files.mode is centered on y $mode_mid (chrome line 26) and x $pane_mid (pane 586), inside the 52 px band after the title; real clicks switch Read aloud and Transcribe" "$failures"

echo "== Audio enhance row and its sheet"
open_view settings
hook_action settings-search '{"query":"enhance"}' >/dev/null
sleep 0.5
shot feat-03-enhance-row
failures=""
segments=$($HC tree | jq -r '[.[] | select(.id | test("^settings\\.audio\\.enhance\\.(off|auto|custom)$"))] | sort_by(.bounds.x) | map(.text) | join(",")')
[ "$segments" = "Off,Auto,Custom" ] || failures+="the segments read '$segments'; "
[ "$(st .enhance.mode)" = auto ] || failures+="the mode is $(st .enhance.mode), not auto; "
[ "$(st '.enhance.effective | .lowCutHz == 80 and .bassDb == -6 and .trebleDb == 0 and .noiseReduction == false')" = true ] ||
  failures+="Auto is $($HC state | jq -c .enhance.effective); "
rendered settings.audio.enhance.adjust && failures+="Adjust shows with Auto; "
real_click settings.audio.enhance.custom
wait_until 5 st_is .enhance.sheet tune || failures+="Custom did not open the sheet (sheet is $(st .enhance.sheet)); "
sleep 0.4
shot feat-03-enhance-sheet
rendered enhance.sheet || failures+="the sheet is not in the tree; "
[ "$(st .enhance.mode)" = custom ] || failures+="the mode after the click is $(st .enhance.mode); "
for id in enhance.lowCut enhance.bass enhance.treble enhance.noiseReduction enhance.record enhance.done; do
  rendered "$id" || failures+="the sheet has no $id; "
done
real_click enhance.done
wait_until 5 st_is .enhance.sheet null || failures+="Done left the sheet open; "
hook_action enhance-set '{"mode":"auto"}' >/dev/null
wait_until 5 st_is .enhance.mode auto || failures+="could not return to Auto; "
hook_action settings-search '{"query":""}' >/dev/null
check_failures "Audio enhance row" "Off|Auto|Custom with Auto chosen (low cut 80 Hz, bass -6 dB); Custom opened the tuning sheet with its sliders and record button; Done closed it" "$failures"

echo "== dictation from a fed WAV"
open_view home
hook_action dictation-language '{"code":"auto"}' >/dev/null
set_clip OLD
dictate_feed feat-04 "$FIX/speech-short.wav"
text=$(dict .transcript)
failures=""
[ "$(dict .state)" = done ] || failures+="state is $(dict .state); "
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="transcript misses: $(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}") ($text); "
[ "$(clip)" = "$text" ] || failures+="the clipboard holds '$(clip)'; "
[ "$(st '.enhance.mode')" = auto ] || failures+="audio enhance is $(st .enhance.mode); "
check_failures "dictation from feed-wav" "transcript '$text', clipboard equal, audio enhance Auto" "$failures"

echo "== instant restart"
open_view home
set_clip OLD
rows_before=$(q "select count(*) from transcript where kind = 'dictation'")
RUN_T0=$(now_ms)
hook_action dictation-record >/dev/null
wait_until 5 dict_is .state listening
play_mic "$FIX/dictation-45s.wav"
hook_action dictation-record >/dev/null
hook_action dictation-record >/dev/null
state_after=$(dict .state)
shot feat-05-restart-listening
play_mic "$FIX/speech-es.wav"
hook_action dictation-record >/dev/null
shot feat-05-restart-stopped
count_is() { [ "$(q "select count(*) from transcript where kind = 'dictation'")" = "$1" ]; }
wait_until 300 count_is $((rows_before + 2))
pipeline_idle() { [ "$(st .pipeline.state)" = idle ]; }
wait_until 60 pipeline_idle
sleep 0.5
shot feat-05-restart-done
event_times >"$RUN_OUT/feat-05-events.txt"
failures=""
[ "$state_after" = listening ] || failures+="the second press left the state on $state_after; "
stopped_at=$(awk '$2 == "transcribing" {print $1; exit}' "$RUN_OUT/feat-05-events.txt")
second_at=$(awk '$2 == "listening" {n++; if (n == 2) {print $1; exit}}' "$RUN_OUT/feat-05-events.txt")
if [ -z "$stopped_at" ] || [ -z "$second_at" ]; then
  failures+="the dictation events are '$(awk '{print $2}' "$RUN_OUT/feat-05-events.txt" | xargs)'; "
else
  gap=$((second_at - stopped_at))
  [ "$gap" -ge 0 ] && [ "$gap" -le 200 ] || failures+="the new dictation listened ${gap} ms after the old one stopped; "
fi
first_id=$(q "select id from transcript where kind = 'dictation' order by created_at asc, id asc limit 1 offset $rows_before")
second_id=$(q "select id from transcript where kind = 'dictation' order by created_at asc, id asc limit 1 offset $((rows_before + 1))")
first_text=$(q "select final_text from transcript where id = '$first_id'")
second_text=$(q "select final_text from transcript where id = '$second_id'")
[ -z "$(missing_words "$(norm <<<"$first_text")" lighthouse)" ] || failures+="the first row is '$first_text', not the 45 s dictation; "
[ -z "$(missing_words "$(norm <<<"$second_text")" hola paulina)" ] || failures+="the second row is '$second_text', not the Spanish dictation; "
[ "$(q "select count(*) from transcript where id in ('$first_id', '$second_id') and status = 'completed'")" = 2 ] || failures+="a row is not completed; "
[ "$(clip)" = "$second_text" ] || failures+="the clipboard ends as '$(clip)', not the text spoken last; "
open_view history
shot feat-05-history
shown=$($HC state | jq -r '[.history.rows[0].id, .history.rows[1].id] | join(" ")')
[ "$shown" = "$second_id $first_id" ] || failures+="History lists '$shown', newest first should be '$second_id $first_id'; "
[ "$(st .history.rows[0].status)" = completed ] || failures+="the newest row is $(st .history.rows[0].status); "
check_failures "instant restart" "the second dictation listened ${gap:-?} ms after the first stopped; both rows are in History in the order spoken (lighthouse, then Hola); the clipboard ends with the second text" "$failures"

echo "== silence skipping"
open_view home
failures=""
for kind in silence noise; do
  wav=$SILENCE
  [ "$kind" = noise ] && wav=$NOISE
  set_clip OLD
  dictate_feed "feat-06-$kind" "$wav" 0
  [ "$(dict .state)" = no-speech ] || failures+="30 s of $kind ended in $(dict .state), transcript '$(dict .transcript)'; "
  [ "$(dict .notice.code)" = ENGINE_NO_SPEECH ] || failures+="30 s of $kind gave notice '$(dict .notice.code)'; "
  [ -z "$(dict .transcript)" ] || failures+="30 s of $kind gave the words '$(dict .transcript)'; "
  [ "$(clip)" = OLD ] || failures+="30 s of $kind changed the clipboard to '$(clip)'; "
done
[ -f /app/ggml-silero-v5.1.2.bin ] || failures+="the speech detector model is not next to the app; "
check_failures "silence skipping" "30 s of digital silence and 30 s of faint noise both ended in no-speech with no words and the clipboard untouched; the Silero model is in /app" "$failures"

echo "== Chatterbox Turbo export and its History row"
failures=""
open_view files
hook_action files-mode '{"mode":"readAloud"}' >/dev/null
hook_action read-text "{\"text\":\"$SENTENCE\"}" >/dev/null
hook_action read-engine "{\"engine\":\"$TTS_ID\"}" >/dev/null || failures+="the engine was refused; "
read_loaded() { [ "$(st .read.loading)" = false ]; }
wait_until 20 read_loaded
sleep 0.4
shot feat-07-chatterbox-engine
[ "$(st .read.engine)" = "$TTS_ID" ] || failures+="the engine is $(st .read.engine); "
[ "$(st .read.needs_model)" = false ] || failures+="the view says the model is missing: $(st .read.needs_model); "
reads_before=$(q "select count(*) from transcript where kind = 'read'")
rm -f "$EXPORT"
$HC paths "$EXPORT" >/dev/null
hook_action read-export >/dev/null || failures+="Export Audio was refused: $(st .read.error); "
sleep 3
shot feat-07-chatterbox-working
exported() { [ -s "$EXPORT" ] || [ -n "$(st .read.error | grep -v '^null$')" ]; }
wait_until 400 exported
sleep 1
shot feat-07-chatterbox-done
if [ -s "$EXPORT" ]; then
  python3 /harness/slot/wavstat.py "$EXPORT" >"$RUN_OUT/feat-07-wav.txt"
  secs=$(sed -n 's/.*secs=\([0-9.]*\).*/\1/p' "$RUN_OUT/feat-07-wav.txt")
  awk -v s="${secs:-0}" 'BEGIN {exit !(s > 0.5)}' || failures+="the WAV is ${secs:-0} s long; "
  grep -q 'silent=false' "$RUN_OUT/feat-07-wav.txt" || failures+="the WAV is silent ($(cat "$RUN_OUT/feat-07-wav.txt")); "
else
  failures+="no WAV was written: $(st .read.error); "
fi
wait_until 20 rows_are read $((reads_before + 1)) || failures+="History holds $(q "select count(*) from transcript where kind = 'read'") read rows, not $((reads_before + 1)); "
row=$(q "select id from transcript where kind = 'read' order by created_at desc limit 1")
[ "$(q "select model_id || '|' || status || '|' || final_text from transcript where id = '$row'")" = "$TTS_ID|completed|$SENTENCE" ] ||
  failures+="the row reads '$(q "select model_id, status, final_text from transcript where id = '$row'")'; "
[ "$(q "select duration_ms > 500 from transcript where id = '$row'")" = 1 ] || failures+="the row length is $(q "select duration_ms from transcript where id = '$row'") ms; "
open_view history
shot feat-07-history
[ "$($HC state | jq -r --arg id "$row" '[.history.rows[] | select(.id == $id)] | first | .kind')" = read ] || failures+="History does not list the read row; "
meta=$($HC state | jq -r --arg id "$row" '[.history.rows[] | select(.id == $id)] | first | .meta')
case "$meta" in *"Read aloud"*) ;; *) failures+="the list line reads '$meta'; " ;; esac
hook_action history-open "{\"id\":\"$row\"}" >/dev/null
wait_until 5 rendered history.detail
sleep 0.5
shot feat-07-history-detail
check_failures "Chatterbox Turbo export" "wrote $(cat "$RUN_OUT/feat-07-wav.txt" 2>/dev/null) and added one Read aloud row ('$meta')" "$failures"

echo "== Manage Voices sheet"
failures=""
hook_action history-close >/dev/null
open_view files
hook_action files-mode '{"mode":"readAloud"}' >/dev/null
sleep 0.4
rendered read.manage || failures+="no Manage Voices button for Chatterbox; "
hclick read.manage || failures+="could not reach Manage Voices; "
sleep 0.4
wait_until 5 st_is .voices.sheet list || failures+="the sheet is '$(st .voices.sheet)', not the list; "
sleep 0.5
shot feat-08-voices
$HC tree >"$RUN_OUT/feat-08-tree.json"
rendered voices.sheet || failures+="the sheet is not in the tree; "
built_in=$(jq -r '[.[] | select(.id == "voices.name.0")] | first | .text // ""' "$RUN_OUT/feat-08-tree.json")
detail=$(jq -r '[.[] | select(.id == "voices.detail.0")] | first | .text // ""' "$RUN_OUT/feat-08-tree.json")
default=$(jq -r '[.[] | select(.id == "voices.default.0")] | first | .text // ""' "$RUN_OUT/feat-08-tree.json")
[ "$built_in" = Built-in ] || failures+="the first row is '$built_in', not Built-in; "
[ "$detail" = "Comes with the model" ] || failures+="the first row says '$detail'; "
[ "$default" = Default ] || failures+="the first row has '$default' in the default column; "
[ "$(st .voices.default_name)" = Built-in ] || failures+="the default voice is '$(st .voices.default_name)'; "
real_click voices.done
wait_until 5 st_is .voices.sheet null || failures+="Done left the sheet open; "
shot feat-08-voices-closed
check_failures "Manage Voices sheet" "the sheet opened on the list with the Built-in row ('$detail') marked Default; Done closed it" "$failures"
