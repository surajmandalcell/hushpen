#!/bin/bash
# Paired speech on a headless host, refusal without local models, and local fallback.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh
set +x

HOST_DATA=$RUN_OUT/host-data
HOST_PORT=47329
HOST_PID=
GTK_FILE=/out/gtk.txt

stop_host() {
  [ -n "$HOST_PID" ] || return 0
  kill -TERM -- "-$HOST_PID" 2>/dev/null || true
  for _ in $(seq 30); do kill -0 "$HOST_PID" 2>/dev/null || break; sleep 0.1; done
  kill -KILL -- "-$HOST_PID" 2>/dev/null || true
  wait "$HOST_PID" 2>/dev/null || true
  HOST_PID=
}

cleanup() {
  stop_host
  [ ! -f "$HOST_DATA/logs/engine.log" ] || cp "$HOST_DATA/logs/engine.log" "$RUN_OUT/remote-engine.log"
  [ ! -f "$HOST_DATA/logs/host.log" ] || cp "$HOST_DATA/logs/host.log" "$RUN_OUT/remote-host.log"
  rm -rf "$HOST_DATA" "$RUN_OUT/padded"
}
trap cleanup EXIT

setup_failed() { check "remote setup" fail "$1"; exit 0; }
st_is() { [ "$($HC state | jq -r "$1")" = "$2" ]; }
pair_finished() { case "$($HC state | jq -r .remote.state)" in paired | failed) return 0 ;; esac; return 1; }
host_ready() { kill -0 "$HOST_PID" 2>/dev/null && grep -q 'listening on' "$RUN_OUT/host-stderr.log"; }

gtk_clear() {
  xdotool windowfocus --sync "$GT"
  xdotool key ctrl+a BackSpace
}

gtk_commit() {
  rm -f "$GTK_FILE"
  xdotool windowfocus --sync "$GT"
  xdotool key Return
  wait_until 5 test -e "$GTK_FILE" || return 1
  cat "$GTK_FILE"
}

dictate_feed() {
  gtk_clear
  hold_wav "$FIX/speech-short.wav"
}

[ -x /app/hushpen-host ] || setup_failed "/app/hushpen-host is missing"
[ -f "$FIX/speech-short.wav" ] || setup_failed "the speech WAV fixture is missing"
fresh base-q5_1 || setup_failed "the debug desktop did not start"
use_model base-q5_1 || setup_failed "the speech model did not pass verification"
hook_action set-setting '{"key":"cleanup.llm.enabled","value":false}' >/dev/null || setup_failed "AI cleanup could not be disabled"

mkdir -p "$HOST_DATA/models/whisper"
model=$(model_file base-q5_1)
cp -p "$MODELS/$model" "$MODELS/$model.verified" "$HOST_DATA/models/whisper/" || setup_failed "the verified host model could not be copied"
if ss -ltn | grep -q ":$HOST_PORT "; then setup_failed "the isolated host port is already in use"; fi

offer=$(/app/hushpen-host pair --json --data-dir "$HOST_DATA" --listen "127.0.0.1:$HOST_PORT" --host 127.0.0.1 | jq -er .offer) || setup_failed "the host offer could not be created"
setsid timeout --kill-after=3 300 /app/hushpen-host --data-dir "$HOST_DATA" --listen "127.0.0.1:$HOST_PORT" --host 127.0.0.1 --name 'Test host' >"$RUN_OUT/host-stdout.log" 2>"$RUN_OUT/host-stderr.log" &
HOST_PID=$!
wait_until 10 host_ready || setup_failed "the host did not listen"
args=$(jq -nc --arg offer "$offer" '{offer:$offer,id:"test-host",name:"Test host",noLocalModels:true}')
hook_action remote-pair "$args" >/dev/null || setup_failed "the debug pairing action did not start"
unset offer args
wait_until 15 pair_finished || setup_failed "pairing did not finish"
st_is .remote.state paired || setup_failed "the encrypted pairing failed"
wait_until 10 engine_is .model null || setup_failed "the local model stayed loaded with noLocalModels on"
$HC state | jq '{remote,settings:(.settings|with_entries(select(.key|startswith("remote."))))}' >"$RUN_OUT/remote-paired.json"
check "remote pairing" pass "the desktop and host saved public pins; no local model is loaded"

ids=$(/harness/slot/targets.sh) || setup_failed "the GTK paste target did not open"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
[ -n "$GT" ] || setup_failed "the GTK paste target has no window"

failures=""
dictate_feed || failures+="the remote dictation did not finish; "
text=$(dict .transcript)
missing=$(word_recall_ok "$(norm <<<"$text")" "${SHORT_WORDS[@]}") || failures+="the transcript misses words: $missing; "
[ "$(dict .state)" = done ] || failures+="the dictation ended $(dict .state); "
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="the GTK entry did not receive the transcript; "
engine_is .model null || failures+="a local speech model loaded during the remote job; "
grep -q ' JOB ' "$HOST_DATA/logs/engine.log" || failures+="the host logged no speech job; "
$HC state | jq '{dictation,pipeline,engine,last_insert}' >"$RUN_OUT/remote-result.json"
shot remote-result
check_failures "remote speech" "the paired host returned speech and the desktop inserted it with no local model" "$failures"

stop_host
failures=""
dictate_feed || failures+="the refusal did not finish; "
[ "$(dict .state)" = failed ] || failures+="the unavailable host did not refuse the job; "
engine_is .model null || failures+="refusal loaded a local model; "
got=$(gtk_commit)
[ -z "$got" ] || failures+="refusal inserted text; "
$HC state | jq '{dictation,engine}' >"$RUN_OUT/remote-refused.json"
shot remote-refused
check_failures "remote unavailable without local models" "the job refused and kept local speech unloaded" "$failures"

hook_action set-setting '{"key":"remote.noLocalModels","value":false}' >/dev/null || setup_failed "local fallback could not be enabled"
wait_until 90 engine_ready_with base-q5_1 || setup_failed "the fallback model did not load"
failures=""
dictate_feed || failures+="the fallback dictation did not finish; "
text=$(dict .transcript)
missing=$(word_recall_ok "$(norm <<<"$text")" "${SHORT_WORDS[@]}") || failures+="the fallback transcript misses words: $missing; "
[ "$(dict .state)" = done ] || failures+="the fallback ended $(dict .state); "
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="the GTK entry did not receive fallback text; "
notice=$($HC state | jq -r .pipeline.remoteNotice)
[ "$notice" = 'Using this computer. Test host did not answer.' ] || failures+="the fallback notice was missing or wrong; "
$HC state | jq '{dictation,pipeline,engine,last_insert}' >"$RUN_OUT/remote-fallback.json"
shot remote-fallback
check_failures "remote host loss falls back" "local speech returned text after host loss and showed the core fallback notice" "$failures"
