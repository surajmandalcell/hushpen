#!/bin/bash
# First-run onboarding: VAL-ONB-001 to 009 and VAL-CROSS-001. The Linux parts of VAL-ONB-008
# (a finished data folder that starts with no microphone) and VAL-ONB-003 (a macOS component test)
# are split: 003 is cargo test, 008 for Linux is here.
# Busy: it transcribes, and VAL-CROSS-001 downloads the default model once from Hugging Face (the
# slot needs network, about 150 MB). It empties /data, loads and unloads PulseAudio modules, and
# restarts the app, so the slot must not be shared. Needs speech-short.wav and silence-5s.wav in
# /assets/fixtures and ggml-base.bin in /assets/models/whisper. Runs in one slot through
# with-env.sh. Pixel baselines of the five steps are cargo test (test-pixel), not this suite.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

# Every start in this suite sees a fresh data folder as a first run.
export HARNESS_ONBOARDING=fresh
GTK_FILE=/out/gtk.txt
XTERM_FILE=/out/xterm.txt
SHORT_WAV=$FIX/speech-short.wav
SILENCE_WAV=$FIX/silence-5s.wav
DB=/data/history/history.db
DOWNLOADS=/data/cache/downloads
HOLD=108
# Pinned in assets/models.json
BASE_SHA=60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe
BASE_BYTES=147951465

ob() { $HC state | jq -r ".onboarding$1"; }
ob_is() { [ "$(ob "$1")" = "$2" ]; }
row_state() { ob ".permissions.rows[] | select(.key == \"$1\") | .state"; }
row_is() { [ "$(row_state "$1")" = "$2" ]; }
tree_has() { $HC tree | jq -e --arg id "$1" 'any(.[]; .id == $id)' >/dev/null; }
tree_get() { $HC tree | jq -r --arg id "$1" ".[] | select(.id == \$id) | $2" | head -1; }
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
onb_events() { # <detail pattern> -> how many onboarding events since T0 match
  $HC events | jq -r --argjson t0 "$T0" --arg p "$1" \
    '[.[] | select(.kind == "onboarding" and .t_ms >= $t0 and (.detail | test($p)))] | length'
}
focus() { xdotool windowfocus --sync "$1"; }
speech() { paplay --device=vmic "$(pad "$1")"; }
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
ptt() { # <wav>: hold the key for the whole clip. Waits for Listening, so a refused start returns 1.
  T0=$(now_ms)
  xdotool keydown $HOLD
  if ! wait_until 3 pstate_is listening; then
    xdotool keyup $HOLD
    return 1
  fi
  speech "$1"
  xdotool keyup $HOLD
}
ptt_ends() { wait_until 240 run_ended; }

begin() { # [model names from /assets]: empties /data, copies the models, starts the app
  /harness/slot/app.sh stop
  find /data -mindepth 1 -delete
  mkdir -p "$MODELS"
  for name in "$@"; do cp "/assets/models/whisper/ggml-$name.bin" "$MODELS/"; done
  /harness/slot/app.sh start >/dev/null
  wait_until 20 ob_is .active true
}
relaunch() {
  /harness/slot/app.sh restart >/dev/null
  wait_until 20 $HC state
}
engine_ready() { [ "$($HC state | jq -r .engine.state)" = ready ]; }
click() { $HC click "$1" >/dev/null; }
continue_enabled() { [ "$(tree_get onboarding.continue .enabled)" = true ]; }
next_step() { # <expected step after the click>
  wait_until 10 continue_enabled || return 1
  click onboarding.continue
  wait_until 5 ob_is .step "$1"
}
# screens_text: every visible text on the page, to look for a skip control
tree_ids() { $HC tree | jq -r '.[].id'; }
no_skip_control() {
  ! tree_ids | grep -Eqi 'skip|later|dismiss|close-onboarding|onboarding\.back' &&
    ! $HC tree | jq -r '.[] | select(.id | startswith("onboarding.")) | .text // ""' | grep -Eqi '^(skip|later|not now|dismiss)'
}
mic_levels_during() { # <wav>: the highest hook level while the clip plays into vmic
  speech "$1" &
  local player=$! top=0 level
  while kill -0 "$player" 2>/dev/null; do
    level=$(ob .mic.level)
    top=$(awk -v a="$top" -v b="$level" 'BEGIN {print (b + 0 > a + 0) ? b : a}')
    sleep 0.15
  done
  wait "$player" 2>/dev/null
  echo "$top"
}

echo "== setup: the paste targets and a fresh data folder"
begin
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
XT=$(grep -o 'xterm=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-ONB-001 an empty data folder starts onboarding at the permissions step"
failures=""
sleep 1
shot 001-permissions
$HC tree >"$RUN_OUT/001-tree.json"
[ "$(ob .active)" = true ] || failures+="onboarding is not active; "
[ "$(ob .step)" = permissions ] || failures+="step is $(ob .step); "
[ "$(ob .completed)" = false ] || failures+="completed is $(ob .completed); "
[ "$(ob .gate)" = closed ] || failures+="gate is $(ob .gate); "
tree_has onboarding.page && tree_has onboarding.continue || failures+="the page or Continue is not in the tree; "
tree_has sidebar.home && failures+="the sidebar is on screen during onboarding; "
gtk_clear
focus "$GT"
T0=$(now_ms)
xdotool keydown $HOLD
sleep 1.5
xdotool keyup $HOLD
sleep 1
[ -z "$(pevents)" ] && pstate_is idle || failures+="the hold key started a dictation: $(pevents); "
[ "$(onb_events 'key-blocked')" -ge 1 ] || failures+="no key-blocked event; "
[ -z "$(gtk_commit)" ] || failures+="the entry got text; "
check_failures "VAL-ONB-001 first run" "onboarding at permissions, completed false, gate closed, hold key with the GTK entry focused started nothing (events '$(pevents)')" "$failures"

echo "== VAL-ONB-002 the Linux permissions page names the X11 needs, live"
failures=""
for key in x11 keys paste microphone; do
  tree_has "onboarding.permissions.row.$key" || failures+="no $key row in the tree; "
  row_is "$key" ready || failures+="$key row is $(row_state "$key"); "
done
shot 002-ready
$HC state | jq .onboarding.permissions >"$RUN_OUT/002-ready.json"
pactl unload-module module-virtual-source
t0=$(now_ms)
wait_until 3 row_is microphone missing || failures+="the microphone row is $(row_state microphone) 3 s after the source went; "
gone_ms=$(($(now_ms) - t0))
sleep 0.3
shot 002-mic-missing
$HC state | jq .onboarding.permissions >"$RUN_OUT/002-missing.json"
$HC tree >"$RUN_OUT/002-missing-tree.json"
ob .can_continue | grep -q false || failures+="Continue is open without a microphone; "
pactl load-module module-virtual-source source_name=vmic_src master=vmic.monitor >/dev/null
pactl set-default-source vmic_src
t0=$(now_ms)
wait_until 3 row_is microphone ready || failures+="the microphone row is $(row_state microphone) 3 s after the source came back; "
back_ms=$(($(now_ms) - t0))
shot 002-mic-back
/harness/slot/app.sh stop
XDG_SESSION_TYPE=wayland /harness/slot/app.sh start >/dev/null
wait_until 20 ob_is .active true
sleep 1.5
shot 002-wayland
$HC state | jq .onboarding.permissions >"$RUN_OUT/002-wayland.json"
for key in keys paste; do
  row_is "$key" unavailable || failures+="wayland: the $key row is $(row_state "$key"); "
  detail=$(ob ".permissions.rows[] | select(.key == \"$key\") | .detail")
  case "$detail" in *Wayland*) ;; *) failures+="wayland: the $key row says '$detail'; " ;; esac
done
/harness/slot/app.sh restart >/dev/null
wait_until 20 ob_is .active true
check_failures "VAL-ONB-002 live permissions" "4 rows ready; microphone missing after ${gone_ms} ms and ready after ${back_ms} ms; Wayland: keys and paste not available" "$failures"

echo "== VAL-ONB-001 and 007 the step list and the model-less mic test, with the practice gate shut"
# The mic test comes before the model step: with no model the level alone passes it.
failures=""
next_step mic || failures+="Continue did not open the mic test; "
sleep 0.5
shot 004-mic-nomodel
click onboarding.mic.test
wait_until 5 ob_is .mic.phase listening || failures+="the mic test did not start; "
top=$(mic_levels_during "$SHORT_WAV")
click onboarding.mic.test
wait_until 10 ob_is .mic.passed true || failures+="the level check did not pass ($(ob .mic.verdict)); "
shot 004-mic-nomodel-passed
awk -v t="$top" 'BEGIN {exit !(t > 0.05)}' || failures+="the level stayed at $top; "
check_failures "VAL-CROSS-001 mic step" "level reached $top while speech played; passed without a model (verdict $(ob .mic.verdict.result))" "$failures"

echo "== VAL-ONB-006 the keys stay shut until the practice step"
failures=""
next_step model || failures+="Continue did not open the model step; "
shot 005-model-start
$HC state | jq .onboarding >"$RUN_OUT/005-model-state.json"
[ "$(ob .model.id)" = base ] || failures+="the step names model '$(ob .model.id)', not the default base; "
[ "$(ob .model.state)" = not_downloaded ] || failures+="model state is $(ob .model.state); "
continue_enabled && failures+="Continue is open with no model; "
gtk_clear
focus "$GT"
ptt "$SHORT_WAV" && failures+="a push to talk started on the model step; "
sleep 1
[ -z "$(gtk_commit)" ] || failures+="the entry got text before the practice step; "
check_failures "VAL-ONB-006 keys closed before practice" "on the model step the hold key started nothing and the entry stayed empty" "$failures"

echo "== VAL-ONB-005 the model step downloads the default model through the library"
failures=""
click onboarding.model.download
samples=()
for _ in $(seq 6000); do
  p=$(ob .model.progress)
  [ "$p" != null ] && samples+=("$p")
  ob_is .model.state ready && break
  ob_is .model.state failed && break
  sleep 0.2
done
wait_until 30 ob_is .model.state ready || failures+="the model is $(ob .model.state) at the end; "
shot 005-model-ready
prev=-1 dropped=0
for p in "${samples[@]}"; do
  [ "$p" -lt "$prev" ] && dropped=$((dropped + 1))
  prev=$p
done
[ "${#samples[@]}" -ge 5 ] || failures+="only ${#samples[@]} progress samples; "
[ "$dropped" = 0 ] || failures+="progress fell $dropped times; "
[ "${samples[0]:--1}" -lt 20 ] || failures+="the first progress was ${samples[0]:--1}; "
size=$(stat -c %s "$MODELS/ggml-base.bin" 2>/dev/null)
sum=$(sha256sum "$MODELS/ggml-base.bin" 2>/dev/null | awk '{print $1}')
stamp="$MODELS/ggml-base.bin.verified"
[ "$size" = "$BASE_BYTES" ] || failures+="size is $size; "
[ "$sum" = "$BASE_SHA" ] || failures+="sha256 is $sum; "
[ "$(jq -r '"\(.algo) \(.hash) \(.size)"' "$stamp" 2>/dev/null)" = "sha256 $BASE_SHA $BASE_BYTES" ] || failures+="the stamp is wrong: $(cat "$stamp" 2>/dev/null); "
[ -z "$(ls "$DOWNLOADS" 2>/dev/null)" ] || failures+="left in downloads: $(ls "$DOWNLOADS" | tr '\n' ' '); "
wait_until 5 continue_enabled || failures+="Continue is closed with a verified model; "
$HC net >"$RUN_OUT/005-net.json"
[ "$(jq '[.[] | select(.purpose == "model_download" and .host == "huggingface.co")] | length' "$RUN_OUT/005-net.json")" -ge 1 ] || failures+="hookctl net lists no huggingface.co request; "
[ "$(jq '[.[] | select(.purpose != "model_download" or (.host != "huggingface.co" and (.host | endswith(".hf.co") | not)))] | length' "$RUN_OUT/005-net.json")" = 0 ] || failures+="hookctl net lists a request outside the allow-list; "
check_failures "VAL-ONB-005 download" "${#samples[@]} samples rose from ${samples[0]:--1}% to ${samples[${#samples[@]} - 1]:--1}%, then ready; size, sha256 and stamp match; no partial file; net: $(jq -r '[.[] | .host + " " + .result] | join(", ")' "$RUN_OUT/005-net.json")" "$failures"

echo "== VAL-ONB-005 a cancelled second attempt leaves no partial file"
failures=""
# Delete the verified file the way the library does, then start over and cancel.
$HC action model-delete '{"id":"base"}' >"$RUN_OUT/005-delete.txt" 2>&1 || failures+="the model could not be deleted: $(cat "$RUN_OUT/005-delete.txt"); "
wait_until 10 ob_is .model.state not_downloaded || failures+="the model is $(ob .model.state) after the delete; "
click onboarding.model.download
for _ in $(seq 300); do
  p=$(ob .model.progress)
  [ "$p" != null ] && [ "$p" -ge 3 ] && break
  sleep 0.1
done
shot 005-cancel-running
click onboarding.model.cancel
t0=$(now_ms)
wait_until 10 ob_is .model.state not_downloaded || failures+="the model is $(ob .model.state) after Cancel; "
settled=$(($(now_ms) - t0))
sleep 1
shot 005-cancelled
[ -z "$(ls "$DOWNLOADS" 2>/dev/null)" ] || failures+="left in downloads: $(ls "$DOWNLOADS" | tr '\n' ' '); "
[ ! -e "$MODELS/ggml-base.bin" ] || failures+="a model file is there after the cancel; "
continue_enabled && failures+="Continue is open after the cancel; "
[ "$settled" -le 2000 ] || failures+="the cancel took $settled ms; "
check_failures "VAL-ONB-005 cancel" "cancel settled in $settled ms, state not_downloaded, downloads empty, step not passed" "$failures"

echo "== VAL-ONB-005 and 007 a present verified model shows ready with no network; a restart resumes the step"
failures=""
/harness/slot/app.sh stop
cp "/assets/models/whisper/ggml-base.bin" "$MODELS/"
relaunch
wait_until 20 ob_is .active true
wait_until 60 ob_is .model.state ready || failures+="the present model is $(ob .model.state); "
sleep 1
shot 007-resume-model
[ "$(ob .step)" = model ] || failures+="the restart resumed at '$(ob .step)', not the model step; "
$HC net >"$RUN_OUT/005-net-present.json"
[ "$(jq 'length' "$RUN_OUT/005-net-present.json")" = 0 ] || failures+="the present model made network requests: $(jq -c . "$RUN_OUT/005-net-present.json"); "
wait_until 5 continue_enabled || failures+="Continue is closed with the model present; "
check_failures "VAL-ONB-005 present model" "the model step shows ready after a restart, hookctl net is empty" "$failures"

echo "== VAL-ONB-006 the practice field receives a dictation before the global keys turn on"
failures=""
wait_until 90 engine_ready || failures+="the engine did not load; "
next_step practice || failures+="Continue did not open the practice step; "
sleep 0.5
shot 006-practice
[ "$(ob .gate)" = practice ] || failures+="gate is $(ob .gate); "
tree_has onboarding.practice.field || failures+="no practice field in the tree; "
continue_enabled && failures+="Continue is open before a dictation; "
gtk_clear
click onboarding.practice.field
ptt "$SHORT_WAV" || failures+="the practice push to talk did not start; "
wait_until 240 ob_is .practice.passed true || failures+="the practice field never passed; "
sleep 0.5
shot 006-practice-passed
text=$(ob .practice.text)
words_ok "$text" || failures+="the practice field holds '$text'; "
[ "$(ob .gate)" = open ] || failures+="gate is $(ob .gate) after the pass; "
[ -z "$(gtk_commit)" ] || failures+="the practice run also typed into the entry; "
wait_until 5 pstate_is idle
gtk_clear
focus "$GT"
ptt "$SHORT_WAV" || failures+="the hold key did not start after the practice pass; "
ptt_ends || failures+="the run after the practice did not end; "
got=$(gtk_commit)
words_ok "$got" || failures+="the entry got '$got' after the pass; "
check_failures "VAL-ONB-006 practice" "practice field read '$text', then the hold key typed '$got' into the entry" "$failures"

echo "== VAL-ONB-007 a restart at the practice step resumes it; the update choice is off; Finish ends onboarding"
failures=""
/harness/slot/app.sh stop
/harness/slot/app.sh start >/dev/null
wait_until 20 ob_is .active true
sleep 1
shot 007-resume-practice
[ "$(ob .step)" = practice ] || failures+="the restart resumed at '$(ob .step)'; "
[ "$(ob .model.state)" = ready ] || failures+="the model step shows $(ob .model.state) after the restart; "
$HC net >"$RUN_OUT/007-net.json"
[ "$(jq '[.[] | select(.purpose == "model_download")] | length' "$RUN_OUT/007-net.json")" = 0 ] || failures+="a second download started; "
[ "$(ob .gate)" = practice ] || failures+="gate is $(ob .gate) after the restart; "
gtk_clear
click onboarding.practice.field
ptt "$SHORT_WAV" || failures+="the practice run after the restart did not start; "
wait_until 240 ob_is .practice.passed true || failures+="the practice did not pass again; "
next_step updates || failures+="Continue did not open the update step; "
sleep 0.5
shot 007-updates
[ "$(ob .updates.check)" = false ] || failures+="the update choice is $(ob .updates.check) by default; "
tree_has onboarding.updates.toggle || failures+="no update toggle in the tree; "
$HC tree >"$RUN_OUT/007-updates-tree.json"
click onboarding.continue
wait_until 10 ob_is .active false || failures+="Finish did not close onboarding; "
sleep 0.5
shot 007-finished
[ "$(jq -r '.values["updates.check"]' "$SETTINGS")" = false ] || failures+="updates.check is $(jq -r '.values["updates.check"]' "$SETTINGS"); "
[ "$(jq -r '.values["onboarding.completed"]' "$SETTINGS")" = true ] || failures+="onboarding.completed is $(jq -r '.values["onboarding.completed"]' "$SETTINGS"); "
[ "$(jq -r '.values["onboarding.step"]' "$SETTINGS")" = "" ] || failures+="onboarding.step is '$(jq -r '.values["onboarding.step"]' "$SETTINGS")'; "
cp "$SETTINGS" "$RUN_OUT/007-settings-finished.json"
/harness/slot/app.sh restart >/dev/null
wait_until 20 $HC state
sleep 1.5
shot 007-restart-home
[ "$(ob .active)" = false ] || failures+="onboarding opened again after Finish; "
[ "$($HC state | jq -r .view)" = home ] || failures+="the view after the restart is $($HC state | jq -r .view); "
tree_has sidebar.home || failures+="no sidebar after the restart; "
check_failures "VAL-ONB-007 resume and finish" "restart resumed at practice with the model ready and no second download; update choice false by default; Finish saved completed=true; the next start opens Home" "$failures"

echo "== VAL-CROSS-001 the first push to talk after onboarding lands in xterm and History"
failures=""
rows_before=$(sqlite3 "$DB" "select count(*) from transcript")
wait_until 90 engine_ready || failures+="the engine did not load; "
focus "$XT"
# cat keeps its file open, so the text is what the file gained, not what a new file holds
size=$(stat -c %s "$XTERM_FILE" 2>/dev/null || echo 0)
ptt "$SHORT_WAV" || failures+="the hold key did not start a dictation; "
ptt_ends || failures+="the run did not end; "
sleep 0.4
xdotool key Return
sleep 0.5
shot cross-001-xterm
got=$(tail -c +$((size + 1)) "$XTERM_FILE" 2>/dev/null | tr -d '\n')
words_ok "$got" || failures+="xterm got '$got'; "
rows_after=$(sqlite3 "$DB" "select count(*) from transcript")
[ "$rows_after" = $((rows_before + 1)) ] || failures+="history rows went $rows_before -> $rows_after; "
row=$(sqlite3 "$DB" "select coalesce(insert_outcome,'') || '|' || coalesce(target_app,'') from transcript order by created_at desc, id desc limit 1")
case "$row" in pasted\|[xX][tT]erm) ;; *) failures+="the newest row is '$row'; " ;; esac
check_failures "VAL-CROSS-001 first dictation" "first run: network model, mic level moved, practice passed, Finish; then xterm got '$got', History row '$row'" "$failures"

echo "== VAL-ONB-008 a finished data folder with no microphone reopens the permissions step"
failures=""
/harness/slot/app.sh stop
cp "$SETTINGS" "$RUN_OUT/008-settings-before.json"
ls -l "$MODELS" >"$RUN_OUT/008-models-before.txt"
jq -S '.values' "$SETTINGS" >"$RUN_OUT/008-values-before.json"
pactl unload-module module-virtual-source
pactl unload-module module-null-sink
sources=$(pactl list short sources | wc -l)
/harness/slot/app.sh start >/dev/null
wait_until 20 ob_is .mode repair || failures+="onboarding mode is $(ob .mode), not repair; "
sleep 1
shot 008-repair
$HC tree >"$RUN_OUT/008-repair-tree.json"
[ "$sources" = 0 ] || failures+="pactl lists $sources sources; "
[ "$(ob .step)" = permissions ] || failures+="the repair is at '$(ob .step)'; "
row_is microphone missing || failures+="the microphone row is $(row_state microphone); "
[ "$(ob .completed)" = true ] || failures+="completed is $(ob .completed); "
[ "$(ob .gate)" = open ] || failures+="the gate is $(ob .gate) in repair; "
continue_enabled && failures+="Continue is open with no microphone; "
no_skip_control || failures+="a skip control is there; "
jq -S '.values' "$SETTINGS" >"$RUN_OUT/008-values-during.json"
cmp -s "$RUN_OUT/008-values-before.json" "$RUN_OUT/008-values-during.json" || failures+="settings changed while the page was open; "
pactl load-module module-null-sink sink_name=vmic >/dev/null
pactl load-module module-virtual-source source_name=vmic_src master=vmic.monitor >/dev/null
pactl load-module module-null-sink sink_name=cues >/dev/null
pactl set-default-source vmic_src
pactl set-default-sink cues
wait_until 5 row_is microphone ready || failures+="the microphone row is $(row_state microphone) after the source came back; "
shot 008-mic-back
wait_until 5 continue_enabled || failures+="Continue is closed after the microphone came back; "
click onboarding.continue
wait_until 5 ob_is .active false || failures+="Continue did not return to the main window; "
sleep 0.5
shot 008-main
[ "$($HC state | jq -r .view)" = home ] || failures+="the view is $($HC state | jq -r .view); "
[ "$(ob .step)" = permissions ] || [ "$(ob .mode)" = hidden ] || failures+="another step opened; "
jq -S '.values' "$SETTINGS" >"$RUN_OUT/008-values-after.json"
cmp -s "$RUN_OUT/008-values-before.json" "$RUN_OUT/008-values-after.json" || failures+="settings changed after the repair; "
ls -l "$MODELS" >"$RUN_OUT/008-models-after.txt"
cmp -s "$RUN_OUT/008-models-before.txt" "$RUN_OUT/008-models-after.txt" || failures+="the models folder changed; "
check_failures "VAL-ONB-008 lost microphone" "no source: repair at permissions, settings and models unchanged, completed stays true; microphone back: Continue returned to Home" "$failures"

echo "== VAL-ONB-004 the mic test shows the level and a test transcript"
failures=""
begin base
wait_until 90 engine_ready || failures+="the engine did not load; "
next_step mic || failures+="Continue did not open the mic test; "
sleep 0.5
shot 004-mic-start
click onboarding.mic.test
wait_until 5 ob_is .mic.phase listening || failures+="the silence test did not start; "
top_silence=$(mic_levels_during "$SILENCE_WAV")
click onboarding.mic.test
wait_until 60 ob_is .mic.phase idle || failures+="the silence test did not end; "
sleep 0.3
shot 004-silence
verdict=$(ob .mic.verdict.result)
hint=$(ob .mic.verdict.hint)
[ "$(ob .mic.passed)" = false ] || failures+="silence passed the test; "
[ "$verdict" = no-sound ] || [ "$verdict" = no-speech ] || failures+="silence verdict is $verdict; "
[ -n "$hint" ] && [ "$hint" != null ] || failures+="silence shows no hint; "
tree_has onboarding.mic.hint || failures+="the hint is not on screen; "
continue_enabled && failures+="Continue is open after silence; "
click onboarding.mic.test
wait_until 5 ob_is .mic.phase listening || failures+="the speech test did not start; "
top=$(mic_levels_during "$SHORT_WAV")
click onboarding.mic.test
wait_until 120 ob_is .mic.passed true || failures+="speech did not pass ($(ob .mic.verdict)); "
sleep 0.3
shot 004-speech
transcript=$(ob .mic.verdict.transcript)
words_ok "$transcript" || failures+="recall below 0.8: '$transcript' misses $(missing_words "$(norm <<<"$transcript")" "${SHORT_WORDS[@]}"); "
missing_n=$(wc -w <<<"$(missing_words "$(norm <<<"$transcript")" "${SHORT_WORDS[@]}")")
awk -v t="$top" 'BEGIN {exit !(t > 0.05)}' || failures+="the level stayed at $top during speech; "
tree_has onboarding.mic.transcript || failures+="the transcript is not on screen; "
wait_until 5 continue_enabled || failures+="Continue is closed after the pass; "
check_failures "VAL-ONB-004 mic test" "silence: level $top_silence, $verdict, hint shown, not passed; speech: level $top, transcript '$transcript' (${missing_n} of ${#SHORT_WORDS[@]} words missing), passed" "$failures"

echo "== VAL-ONB-009 the keyboard alone completes the five steps; no step has a skip control"
failures=""
begin base
wait_until 90 engine_ready || failures+="the engine did not load; "
focus "$(app_window)"
tab_to() { # <id>: Tab until that element has focus
  local id=$1
  for _ in $(seq 12); do
    [ "$(tree_get "$id" .focused)" = true ] && return 0
    xdotool key Tab
    sleep 0.25
  done
  [ "$(tree_get "$id" .focused)" = true ]
}
key_step() { # <step key> <shot name>
  sleep 0.5
  $HC tree >"$RUN_OUT/009-$2-tree.json"
  shot "009-$2"
  no_skip_control || failures+="$1: a skip control is in the tree; "
  [ "$(ob .step)" = "$1" ] || failures+="expected step $1, on $(ob .step); "
}
key_step permissions 1-permissions
wait_until 10 continue_enabled || failures+="permissions: Continue stayed closed; "
tab_to onboarding.continue || failures+="permissions: Tab never reached Continue; "
shot 009-1-permissions-focus
xdotool key Return
wait_until 5 ob_is .step mic || failures+="permissions: Return did not open the mic test; "

key_step mic 2-mic
tab_to onboarding.mic.test || failures+="mic: Tab never reached Test; "
shot 009-2-mic-focus
xdotool key space
wait_until 5 ob_is .mic.phase listening || failures+="mic: Space did not start the test; "
speech "$SHORT_WAV"
xdotool key space
wait_until 120 ob_is .mic.passed true || failures+="mic: the test did not pass ($(ob .mic.verdict)); "
tab_to onboarding.continue || failures+="mic: Tab never reached Continue; "
xdotool key Return
wait_until 5 ob_is .step model || failures+="mic: Return did not open the model step; "

key_step model 3-model
wait_until 60 ob_is .model.state ready || failures+="model: the present model is $(ob .model.state); "
tab_to onboarding.continue || failures+="model: Tab never reached Continue; "
shot 009-3-model-focus
xdotool key Return
wait_until 5 ob_is .step practice || failures+="model: Return did not open the practice step; "

key_step practice 4-practice
tab_to onboarding.practice.field || failures+="practice: Tab never reached the practice field; "
shot 009-4-practice-focus
ptt "$SHORT_WAV" || failures+="practice: the hold key did not start; "
wait_until 240 ob_is .practice.passed true || failures+="practice: the field never passed; "
tab_to onboarding.continue || failures+="practice: Tab never reached Continue; "
xdotool key Return
wait_until 5 ob_is .step updates || failures+="practice: Return did not open the update step; "

key_step updates 5-updates
tab_to onboarding.updates.toggle || failures+="updates: Tab never reached the toggle; "
shot 009-5-updates-focus
xdotool key space
wait_until 3 ob_is .updates.check true || failures+="updates: Space did not turn the choice on; "
xdotool key space
wait_until 3 ob_is .updates.check false || failures+="updates: Space did not turn the choice off; "
tab_to onboarding.continue || failures+="updates: Tab never reached Finish; "
xdotool key Return
wait_until 10 ob_is .active false || failures+="updates: Return did not finish onboarding; "
[ "$(jq -r '.values["onboarding.completed"]' "$SETTINGS")" = true ] || failures+="onboarding.completed is not true; "
check_failures "VAL-ONB-009 keyboard only" "all five steps completed with Tab, Space, and Return; no skip control in any step; focus shown in the 009-*-focus screenshots" "$failures"

# leave the slot as init.sh made it
/harness/slot/app.sh stop
