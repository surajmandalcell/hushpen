#!/bin/bash
# Home dictation checks (VAL-HOME-001 to 010 and VAL-MOD-010). Busy: it plays speech into the
# virtual microphone and transcribes with base-q5_1 and small-q5_1; Parakeet is only selected, to
# see how Home treats a language it does not read. It empties /data, restarts the app, and kills
# the engine child, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh

. /harness/slot/dictate-lib.sh

model_active() { [ "$($HC state | jq -r .models.active)" = "$1" ]; }

echo "== VAL-HOME-001 dictation shows the words and copies them"
fresh base-q5_1 small-q5_1 parakeet-v3-q4_k
use_model base-q5_1
set_clip OLD
dictate 001 "$FIX/speech-short.wav"
events=$(run_events)
text=$(dict .transcript)
clipboard=$(clip)
$HC state | jq .dictation >"$RUN_OUT/001-state.json"
failures=""
[ "$events" = "listening,transcribing,done" ] || failures+="events were '$events'; "
[ "$(dict .state)" = done ] || failures+="state is $(dict .state); "
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="transcript misses: $(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}") ($text); "
[ "$clipboard" = "$text" ] || failures+="clipboard is '$clipboard', transcript '$text'; "
[ "$(tree_text home.transcript)" = "$text" ] || failures+="the transcript in the tree is '$(tree_text home.transcript)'; "
check_failures "VAL-HOME-001 words and clipboard" "events $events, transcript '$text' equals the clipboard" "$failures"

echo "== VAL-HOME-002 a 45 s dictation keeps its last sentence"
dictate 002 "$FIX/dictation-45s.wav"
text=$(dict .transcript)
clipboard=$(clip)
failures=""
[ "$(dict .state)" = done ] || failures+="state is $(dict .state); "
case " $(norm <<<"$text") " in *" the final word of this dictation is lighthouse "*) ;; *) failures+="the last sentence is missing: $text; " ;; esac
[ "$clipboard" = "$text" ] || failures+="clipboard differs from the transcript; "
check_failures "VAL-HOME-002 45 s dictation" "transcript has the last sentence ($(wc -w <<<"$text") words) and the clipboard matches" "$failures"

echo "== VAL-HOME-005 auto-detect finds Spanish"
hook_action dictation-language '{"code":"auto"}' >/dev/null
dictate 005 "$FIX/speech-es.wav"
text=$(dict .transcript)
printf '%s' "$text" >"$RUN_OUT/005-transcript.txt"
clip >"$RUN_OUT/005-clipboard.txt"
detail=$(tree_text home.language.detail)
failures=""
[ "$(dict .language)" = es ] || failures+="detected language is $(dict .language); "
case "$detail" in *Spanish*) ;; *) failures+="screen says '$detail'; " ;; esac
[ -z "$(missing_words "$(norm <<<"$text")" hola paulina)" ] || failures+="transcript is not the Spanish sentence: $text; "
cmp -s "$RUN_OUT/005-transcript.txt" "$RUN_OUT/005-clipboard.txt" || failures+="clipboard bytes differ from the transcript; "
check_failures "VAL-HOME-005 Spanish auto-detect" "detected es, screen '$detail', transcript '$text', clipboard bytes equal" "$failures"

echo "== VAL-HOME-010 the logs never contain transcript text"
failures=""
hits=$(grep -ri -E 'quick|brown|lazy|lighthouse' /data/logs/ /logs/app.log 2>/dev/null | head -3)
[ -z "$hits" ] || failures+="transcript words in a log: $hits; "
for word in llamo paulina vamos probar reconocimiento ordenador; do
  hits=$(grep -ril "$word" /data/logs/ /logs/app.log 2>/dev/null | head -1)
  [ -z "$hits" ] || failures+="'$word' in $hits; "
done
riff=$(grep -rl RIFF /data/logs/ /logs/app.log 2>/dev/null | head -1)
[ -z "$riff" ] || failures+="WAV data in $riff; "
ls -l /data/logs >"$RUN_OUT/010-logs.txt"
check_failures "VAL-HOME-010 logs hold no transcript text" "no fixture word and no RIFF header in $(ls /data/logs | wc -l) log files" "$failures"

echo "== VAL-HOME-003 silence gives no-speech and never brings back earlier text"
dictate 003a "$FIX/speech-short.wav"
text_a=$(dict .transcript)
set_clip OLD
dictate 003 "$FIX/silence-5s.wav"
$HC state | jq .dictation >"$RUN_OUT/003-state.json"
shown=$(tree_text home.transcript)
failures=""
[ "$(dict .state)" = no-speech ] || failures+="state is $(dict .state); "
[ "$(dict .last_result)" = no-speech ] || failures+="last result is $(dict .last_result); "
[ "$(dict .notice.code)" = ENGINE_NO_SPEECH ] || failures+="notice code is $(dict .notice.code); "
[ -z "$(dict .transcript)" ] || failures+="the transcript holds '$(dict .transcript)'; "
case "$shown" in *quick* | *brown* | *'['* | *']'*) failures+="the transcript area shows '$shown'; " ;; esac
[ "$(clip)" = OLD ] || failures+="clipboard is '$(clip)', want OLD; "
[ -n "$text_a" ] || failures+="the first dictation gave no text; "
check_failures "VAL-HOME-003 no-speech state" "no-speech with ENGINE_NO_SPEECH, empty transcript, clipboard still OLD (earlier text was '$text_a')" "$failures"

echo "== VAL-HOME-004 blank markers are filtered"
dictate 004 "$FIX/speech-short.wav" 3
text=$(dict .transcript)
clipboard=$(clip)
failures=""
[ "$(dict .state)" = done ] || failures+="state is $(dict .state); "
if grep -q -E '\[[^]]*\]|\([^)]*\)' <<<"$text$clipboard"; then failures+="a bracketed marker is left: $text; "; fi
[ -z "$(missing_words "$(norm <<<"$text")" quick brown fox lazy dog)" ] || failures+="words missing: $text; "
check_failures "VAL-HOME-004 blank markers" "no bracket marker after 3 s of trailing silence, transcript '$text'" "$failures"

echo "== VAL-HOME-006 language picker"
hook_action dictation-language '{"code":"auto"}' >/dev/null
$HC click home.language >/dev/null
wait_until 5 dict_is .picker.open true
sleep 0.4
$HC tree >"$RUN_OUT/006-tree.json"
shot 006-picker-open
options=$(jq '[.[] | select(.id | startswith("home.language.option."))] | length' "$RUN_OUT/006-tree.json")
failures=""
[ "$options" -ge 91 ] || failures+="the picker lists $options entries; "
for code in auto en es de ja; do
  [ "$(jq "[.[] | select(.id == \"home.language.option.$code\")] | length" "$RUN_OUT/006-tree.json")" = 1 ] || failures+="no option for $code; "
done
# Spanish is far down the list: scroll it into view with the mouse wheel, then click it.
read -r ox oy < <(settled_origin "$(app_window)")
lx=$(jq -r '.[] | select(.id == "home.language.list") | (.root_bounds.x + .root_bounds.width / 2) | floor' "$RUN_OUT/006-tree.json")
ly=$(jq -r '.[] | select(.id == "home.language.list") | (.root_bounds.y + .root_bounds.height / 2) | floor' "$RUN_OUT/006-tree.json")
es_visible() {
  $HC tree | jq -e '
    (.[] | select(.id == "home.language.list") | .root_bounds) as $l
    | .[] | select(.id == "home.language.option.es") | .root_bounds
    | .y >= $l.y and (.y + .height) <= ($l.y + $l.height)' >/dev/null
}
for _ in $(seq 60); do
  es_visible && break
  xdotool mousemove "$lx" "$ly" click 5
  sleep 0.1
done
es_visible || failures+="Spanish never scrolled into view; "
shot 006-picker-spanish
$HC click home.language.option.es >/dev/null || failures+="click on Spanish failed; "
wait_until 5 dict_is .picker.selected es || failures+="picker shows $(dict .picker.selected); "
saved=$(jq -r '.values["dictation.language"]' "$SETTINGS")
[ "$saved" = es ] || failures+="settings hold '$saved'; "
/harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle
[ "$(dict .picker.selected)" = es ] || failures+="after a restart the picker shows $(dict .picker.selected); "
[ "$(tree_text home.language)" = "Language: Spanish" ] || failures+="the picker button reads '$(tree_text home.language)'; "
shot 006-restart-spanish
# A bad value in the file turns into auto.
/harness/slot/app.sh stop
jq '.values["dictation.language"] = "xx-invalid"' "$SETTINGS" >"$SETTINGS.tmp" && mv "$SETTINGS.tmp" "$SETTINGS"
start_app
[ "$(dict .picker.selected)" = auto ] || failures+="an invalid value gives picker $(dict .picker.selected); "
hook_action capture-select '{"id":"default"}' >/dev/null
[ "$(jq -r '.values["dictation.language"]' "$SETTINGS")" = auto ] || failures+="the file holds '$(jq -r '.values["dictation.language"]' "$SETTINGS")' after a settings write; "
# Parakeet reads 25 European languages: Spanish needs no notice, Japanese gets one that names
# the model, and the picker stays on. A Whisper model reads Japanese, so it shows no notice.
hook_action model-select '{"id":"parakeet-v3-q4_k"}' >/dev/null
wait_until 30 model_active parakeet-v3-q4_k || failures+="Parakeet did not become the active model; "
hook_action dictation-language '{"code":"es"}' >/dev/null
sleep 0.4
[ -z "$(tree_text home.language-notice)" ] || failures+="Parakeet shows a notice for Spanish: '$(tree_text home.language-notice)'; "
hook_action dictation-language '{"code":"ja"}' >/dev/null
sleep 0.4
shot 006-parakeet-japanese
case "$(tree_text home.language-notice)" in *Parakeet*) ;; *) failures+="no Parakeet notice for Japanese ('$(tree_text home.language-notice)'); " ;; esac
[ "$($HC tree | jq -r '.[] | select(.id == "home.language") | .enabled')" = true ] || failures+="the picker is off with Parakeet; "
use_model base-q5_1 || failures+="base-q5_1 did not load again; "
sleep 0.4
[ -z "$(tree_text home.language-notice)" ] || failures+="base-q5_1 shows a language notice for Japanese: '$(tree_text home.language-notice)'; "
hook_action dictation-language '{"code":"auto"}' >/dev/null
check_failures "VAL-HOME-006 language picker" "$options entries, Spanish saved and kept after a restart, xx-invalid gave auto and was rewritten, Parakeet names itself for Japanese with the picker on, base-q5_1 shows no notice" "$failures"

echo "== VAL-MOD-010 the active model is used by the next dictation"
use_model base-q5_1
pid_before=$(ui_pid)
hook_action model-select '{"id":"small-q5_1"}' >/dev/null
wait_until 90 engine_ready_with small-q5_1
dictate mod010 "$FIX/speech-short.wav"
text=$(dict .transcript)
failures=""
[ "$(ui_pid)" = "$pid_before" ] || failures+="the UI pid changed; "
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="words missing: $text; "
grep -q 'model=small-q5_1' "$ENGINE_LOG" || failures+="engine.log never names small-q5_1; "
grep -E 'JOB ' "$ENGINE_LOG" | tail -1 | grep -q 'model=small-q5_1' || failures+="the last JOB line is not small-q5_1; "
[ "$(jq -r '.values["dictation.modelId"]' "$SETTINGS")" = small-q5_1 ] || failures+="settings hold $(jq -r '.values["dictation.modelId"]' "$SETTINGS"); "
/harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle
[ "$($HC state | jq -r '.models.models[] | select(.active) | .id')" = small-q5_1 ] || failures+="after a restart the active model is not small-q5_1; "
$HC click sidebar.models >/dev/null
sleep 0.4
shot mod010-restart
check_failures "VAL-MOD-010 active model in use" "small-q5_1 loaded with UI pid $pid_before, dictation '$text', settings and restart keep small-q5_1" "$failures"

echo "== VAL-HOME-007 no model: Home cannot start and points to Models"
fresh
sleep 0.5
$HC click sidebar.home >/dev/null
sleep 0.4
shot 007-no-model
$HC tree >"$RUN_OUT/007-tree.json"
failures=""
[ "$(dict .state)" = idle ] || failures+="state is $(dict .state); "
[ "$(dict .can_record)" = false ] || failures+="can_record is $(dict .can_record); "
[ "$(dict .blocker.code)" = ENGINE_NO_MODEL ] || failures+="blocker is $(dict .blocker.code); "
[ "$(jq -r '.[] | select(.id == "home.record") | .enabled' "$RUN_OUT/007-tree.json")" = false ] || failures+="the record button is enabled; "
$HC click home.record >/dev/null 2>&1 && failures+="a click on the disabled record button was accepted; "
hook_action dictation-record >/dev/null 2>&1 && failures+="the record action started a run; "
[ "$(dict .state)" = idle ] || failures+="state is $(dict .state) after the click; "
[ -z "$(ls "$SESSIONS" 2>/dev/null)" ] || failures+="a session file appeared: $(ls "$SESSIONS"); "
[ "$(jq '[.[] | select(.id == "home.notice")] | length' "$RUN_OUT/007-tree.json")" = 1 ] || failures+="no notice in the tree; "
$HC click home.notice.models >/dev/null || failures+="no way to open Models; "
wait_until 5 test "$($HC state | jq -r .view)" = models || failures+="the view is $($HC state | jq -r .view), not models; "
shot 007-models
check_failures "VAL-HOME-007 no model" "record disabled, no session file, notice with an Open Models button that shows Models" "$failures"

echo "== VAL-HOME-008 an engine crash during transcription"
fresh base-q5_1
use_model base-q5_1
dictate 008a "$FIX/speech-short.wav"
padded=$(pad "$FIX/dictation-45s.wav")
hook_action dictation-record >/dev/null
wait_until 5 dict_is .state listening
paplay --device=vmic "$padded"
hook_action dictation-record >/dev/null
wait_until 5 dict_is .state transcribing
# The state flips at the stop, before the recording is closed and the job reaches the engine. A
# kill inside that gap hits an idle engine, and the job then runs on the new one and finishes.
sleep 1
ui_before=$(ui_pid)
engine_pid=$(engine .pid)
kill -9 "$engine_pid"
wait_until 30 dict_final
shot 008-failed
# The flow bar eases to the size of its notice; this frame is the settled one.
sleep 1
shot 008-failed-settled
session=$(dict .session)
failures=""
[ "$(dict .state)" = failed ] || failures+="state is $(dict .state); "
[ -n "$(dict .notice.message)" ] || failures+="no notice; "
[ -z "$(dict .transcript)" ] || failures+="the transcript still holds '$(dict .transcript)'; "
[ -f "$session" ] || failures+="the session WAV is gone ($session); "
[ "$(ui_pid)" = "$ui_before" ] || failures+="the UI pid changed; "
wait_until 40 engine_ready_with base-q5_1 || failures+="the engine did not come back ready; "
[ "$(engine .pid)" != "$engine_pid" ] || failures+="the engine pid did not change; "
offline=$(/app/hushpen engine --smoke "$session" --model /assets/models/whisper/ggml-base-q5_1.bin 2>/dev/null | sed -n 's/^text: //p' | norm)
case " $offline " in *" lighthouse "*) ;; *) failures+="the kept WAV transcribes to '$offline'; " ;; esac
dictate 008b "$FIX/speech-short.wav"
text=$(dict .transcript)
[ -z "$(missing_words "$(norm <<<"$text")" "${SHORT_WORDS[@]}")" ] || failures+="the next dictation gave '$text'; "
check_failures "VAL-HOME-008 crash during transcription" "failed with a notice and no old text, WAV kept and transcribes offline, UI pid $ui_before, new engine pid, next dictation '$text'" "$failures"

echo "== VAL-HOME-009 an engine crash during listening"
padded=$(pad "$FIX/dictation-45s.wav")
hook_action dictation-record >/dev/null
wait_until 5 dict_is .state listening
paplay --device=vmic "$padded" &
player=$!
sleep 6
engine_pid=$(engine .pid)
kill -9 "$engine_pid"
sleep 3
state_mid=$(dict .state)
shot 009-listening
wait "$player"
hook_action dictation-record >/dev/null
wait_until 240 dict_final
text=$(dict .transcript)
failures=""
[ "$state_mid" = listening ] || failures+="state after the kill was $state_mid; "
[ "$(dict .state)" = done ] || failures+="final state is $(dict .state); "
case " $(norm <<<"$text") " in *" good morning "*) ;; *) failures+="the first sentence is missing; " ;; esac
case " $(norm <<<"$text") " in *" lighthouse "*) ;; *) failures+="'lighthouse' is missing from '$text'; " ;; esac
[ "$(engine .pid)" != "$engine_pid" ] || failures+="the engine pid did not change; "
check_failures "VAL-HOME-009 crash during listening" "stayed listening, engine restarted, transcript has the first sentence and lighthouse" "$failures"
