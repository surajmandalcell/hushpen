#!/bin/bash
# The opt-in OpenAI-compatible endpoint for AI cleanup: VAL-EP-001 to 009. A stub server on
# 127.0.0.1:11434 (harness/slot/openai-stub.py) plays the endpoint; its mode is the first line of
# /tmp/stub-mode and every request it gets is one line in $RUN_OUT/stub-requests.jsonl. The hold
# key is Right Alt (X11 keycode 108); the words come from the virtual microphone. Rows are read
# back with sqlite3 from /data/history/history.db, the pipeline and the endpoint form from
# `hookctl state` (pipeline.ai and settings_view.ai.endpoint), and the fields are typed into with
# xdotool. The suite runs under umask 000, so a 0600 secrets.json shows that the app sets the
# mode itself. No model download and no local model: the provider is the endpoint.
# Needs filler-dictation.wav in HUSHPEN_TEST_FIXTURES (mounted at /fixtures) and the base model
# in /assets. It empties /data, so the slot must not be shared. The strace checks attach to the
# app and need ptrace in the container.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

umask 000

HOLD=108
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
FILLER_WAV=/fixtures/filler-dictation.wav
SECRETS=/data/config/secrets.json
SETTINGS_FILE=/data/config/settings.json
STUB_LOG=$RUN_OUT/stub-requests.jsonl
STUB_PID=/tmp/stub.pid
STUB_MODE=/tmp/stub-mode
KEY1=sk-test-123
KEY2=sk-test-456
OLLAMA_URL=http://localhost:11434/v1
LMSTUDIO_URL=http://localhost:1234/v1
LOOPBACK_URL=http://127.0.0.1:11434/v1
REMOTE_URL=http://10.0.0.2:11434/v1
MODEL=stub-model

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() {
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
event_t() { # <state>: ms of the first pipeline event with that detail since T0
  $HC events | jq -r --argjson t0 "$T0" --arg s "$1" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0 and .detail == $s) | .t_ms] | first // empty'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
focus() { xdotool windowfocus --sync "$1"; }
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
sget() { $HC state | jq -r ".settings[\"$1\"]"; }
ai() { $HC state | jq -r ".pipeline.ai$1"; }
sv() { $HC state | jq -r ".settings_view$1"; }
ep() { $HC state | jq -r ".settings_view.ai.endpoint$1"; }
ep_is() { [ "$(ep "$1")" = "$2" ]; }
sv_is() { [ "$(sv "$1")" = "$2" ]; }
last_row() { sqlite3 "$DB" "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
row_count() { sqlite3 "$DB" 'select count(*) from transcript'; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
alive() { kill -0 "$(ui_pid)" 2>/dev/null; }
has_filler() { grep -Eiwq 'um+|uh+|er+m?' <<<"$1"; }
is_unset() { [ -z "$1" ] || [ "$1" = null ]; }
mode_of() { stat -c %a "$1" 2>/dev/null; }

stub_up() { python3 -c 'import socket; socket.create_connection(("127.0.0.1", 11434), 1).close()' 2>/dev/null; }
stub_stop() {
  [ -f "$STUB_PID" ] || return 0
  kill "$(cat "$STUB_PID")" 2>/dev/null
  rm -f "$STUB_PID"
  for _ in $(seq 30); do stub_up || return 0; sleep 0.1; done
}
stub_start() { # [mode]: a fresh stub; the log keeps growing across restarts
  stub_stop
  echo "${1:-default}" >"$STUB_MODE"
  python3 /harness/slot/openai-stub.py "$STUB_LOG" >/dev/null 2>&1 &
  echo $! >"$STUB_PID"
  wait_until 5 stub_up
}
stub_mode() { echo "$1" >"$STUB_MODE"; }
trap stub_stop EXIT
req_count() { if [ -s "$STUB_LOG" ]; then wc -l <"$STUB_LOG" | tr -d ' '; else echo 0; fi; }
chat_count() { if [ -s "$STUB_LOG" ]; then jq -s '[.[] | select(.method == "POST")] | length' "$STUB_LOG"; else echo 0; fi; }
last_chat() { jq -sc '[.[] | select(.method == "POST")] | last // {}' "$STUB_LOG"; }
last_req() { jq -sc 'last // {}' "$STUB_LOG"; }

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
read_row() {
  ROW_RAW=$(last_row raw_text)
  ROW_RULE=$(last_row rule_text)
  ROW_LLM=$(last_row llm_text)
  ROW_FINAL=$(last_row final_text)
  ROW_OUTCOME=$(last_row llm_outcome)
}
# ai_dictate <label> <wav>: one dictation into the GTK entry; sets GOT, the ROW_* fields, and
# CLEAN_MS (cleaning to inserting)
ai_dictate() {
  gtk_clear
  focus "$GT"
  local rows c i
  rows=$(row_count)
  ptt_run "$2" || return 1
  GOT=$(gtk_commit)
  [ "$(row_count)" -gt "$rows" ] || { echo "no history row was saved by $1"; return 1; }
  read_row
  CLEAN_MS=""
  c=$(event_t cleaning)
  i=$(event_t inserting)
  [ -n "$c" ] && [ -n "$i" ] && CLEAN_MS=$((i - c))
  shot "$1"
}

open_settings_cleanup() {
  focus "$(app_window)"
  $HC action open-view '{"view":"settings"}' >/dev/null
  wait_until 10 view_is settings || return 1
  wait_until 10 tree_has settings.section.cleanup || return 1
  [ "$(sv .section)" = cleanup ] || $HC click settings.section.cleanup >/dev/null
  wait_until 10 tree_has settings.cleanup.ai
}
# type_into <id> <text> [enter]: replaces what the field holds
type_into() {
  hclick "$1" || return 1
  focus "$(app_window)"
  xdotool key ctrl+a
  xdotool type --delay 40 -- "$2"
  [ -z "${3:-}" ] || xdotool key Return
  sleep 0.4
}
pick_provider() { hclick settings.cleanup.provider && sleep 0.3 && hclick "settings.cleanup.provider.option.$1"; }
pick_preset() { hclick settings.cleanup.endpointPreset && sleep 0.3 && hclick "settings.cleanup.endpointPreset.option.$1"; }
# press_test: clicks Test connection; sets TEST_MS to the time until the answer, 0 when none came
test_settled() {
  local s
  s=$(ep .test.state)
  [ "$s" != running ] && [ "$s" != idle ]
}
press_test() {
  hclick settings.cleanup.endpointTest || return 1
  local t
  t=$(now_ms)
  wait_until 12 test_settled || { TEST_MS=0; return 1; }
  TEST_MS=$(($(now_ms) - t))
}

# connects <strace file>: the distinct host:port pairs the app connected to over IP
connects() {
  python3 - "$1" <<'PY'
import re, sys
seen = set()
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r'sin_port=htons\((\d+)\), sin_addr=inet_addr\("([^"]+)"\)', line)
    if m:
        seen.add(f"{m.group(2)}:{m.group(1)}")
    m = re.search(r'sin6_port=htons\((\d+)\).*inet_pton\(AF_INET6, "([^"]+)"', line)
    if m:
        seen.add(f"[{m.group(2)}]:{m.group(1)}")
print(" ".join(sorted(seen)))
PY
}
trace_start() { # <file>: follows the app's connect calls into the file
  strace -f -qq -e trace=connect -o "$1" -p "$(ui_pid)" >/dev/null 2>&1 &
  TRACE_PID=$!
  sleep 1
  kill -0 "$TRACE_PID" 2>/dev/null
}
trace_stop() {
  sleep 0.5
  kill "$TRACE_PID" 2>/dev/null
  wait "$TRACE_PID" 2>/dev/null
}

echo "== setup: base model, the paste targets, and the stub endpoint"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
[ -e "$FILLER_WAV" ] || echo "missing fixture $FILLER_WAV"
rm -f "$STUB_LOG"
stub_start || echo "the stub did not start"

echo "== VAL-EP-001 the endpoint is opt-in and the presets fill the address"
failures=""
[ "$(sget cleanup.llm.provider)" = local ] || failures+="cleanup.llm.provider is '$(sget cleanup.llm.provider)' on a fresh folder; "
is_unset "$(sget cleanup.llm.endpointUrl)" || failures+="endpointUrl is '$(sget cleanup.llm.endpointUrl)'; "
is_unset "$(sget cleanup.llm.endpointModel)" || failures+="endpointModel is '$(sget cleanup.llm.endpointModel)'; "
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
[ "$(sv .ai.provider)" = local ] || failures+="the Cleanup section shows provider '$(sv .ai.provider)'; "
shot ep-001-defaults
$HC state | jq -c '{settings, ai: .settings_view.ai}' >"$RUN_OUT/ep-001-defaults.json"
# A saved address and AI cleanup on, with the provider left on local: no request to the stub.
set_setting cleanup.llm.endpointUrl "\"$LOOPBACK_URL\""
set_setting cleanup.llm.endpointModel "\"$MODEL\""
set_setting cleanup.llm.enabled true
ai_dictate ep-001-local-provider "$FILLER_WAV" || failures+="the dictation with the provider on local did not end; "
[ "$(req_count)" = 0 ] || failures+="the stub got $(req_count) request(s) while the provider was local; "
[ -n "$GOT" ] && [ "$GOT" = "$ROW_RULE" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
has_filler "$GOT" && failures+="the entry still has a filler: $GOT; "
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
pick_provider endpoint || failures+="the provider picker did not take the endpoint; "
wait_until 5 sv_is .ai.provider endpoint
[ "$(sget cleanup.llm.provider)" = endpoint ] || failures+="the provider setting is '$(sget cleanup.llm.provider)' after picking the endpoint; "
pick_preset ollama || failures+="the preset picker did not take Ollama; "
wait_until 5 ep_is .url "$OLLAMA_URL"
[ "$(sget cleanup.llm.endpointUrl)" = "$OLLAMA_URL" ] || failures+="Ollama set the address '$(sget cleanup.llm.endpointUrl)'; "
[ "$(ep .url)" = "$OLLAMA_URL" ] || failures+="the address field shows '$(ep .url)' for Ollama; "
[ "$(ep .preset)" = ollama ] || failures+="the preset is '$(ep .preset)'; "
shot ep-001-ollama
pick_preset lmstudio || failures+="the preset picker did not take LM Studio; "
wait_until 5 ep_is .url "$LMSTUDIO_URL"
[ "$(sget cleanup.llm.endpointUrl)" = "$LMSTUDIO_URL" ] || failures+="LM Studio set the address '$(sget cleanup.llm.endpointUrl)'; "
[ "$(ep .url)" = "$LMSTUDIO_URL" ] || failures+="the address field shows '$(ep .url)' for LM Studio; "
shot ep-001-lmstudio
pick_preset ollama || failures+="the preset picker did not take Ollama again; "
wait_until 5 ep_is .url "$OLLAMA_URL"
check_failures "VAL-EP-001 opt-in and presets" "fresh folder: provider local, address and model empty; provider local with a saved address made 0 stub requests and inserted the rule text; Ollama set $OLLAMA_URL and LM Studio $LMSTUDIO_URL" "$failures"

echo "== VAL-EP-002 Test connection reports success and failure"
failures=""
before=$(req_count)
press_test || failures+="no answer from Test connection with the stub up; "
[ "$(ep .test.state)" = connected ] || failures+="the test state is '$(ep .test.state)' with the stub up; "
[ "$(tree_text settings.cleanup.endpointStatus)" = Connected ] || failures+="the status row says '$(tree_text settings.cleanup.endpointStatus)'; "
shot ep-002-connected
[ "$(req_count)" -gt "$before" ] || failures+="the stub logged no request; "
[ "$(last_req | jq -r '.method + " " + .path')" = "GET /v1/models" ] || failures+="the stub saw '$(last_req | jq -r '.method + " " + .path')', not GET /v1/models; "
stub_stop
press_test || failures+="no answer from Test connection with the stub stopped; "
down_ms=$TEST_MS
[ "$(ep .test.state)" = failed ] || failures+="the test state is '$(ep .test.state)' with the stub stopped; "
[ "$(ep .test.code)" = ENDPOINT_FAILED ] || failures+="the test code is '$(ep .test.code)'; "
[ "$(tree_text settings.cleanup.endpointStatus)" = "The endpoint could not be reached." ] || failures+="the status row says '$(tree_text settings.cleanup.endpointStatus)', not the plain sentence; "
[ "$down_ms" -le 6000 ] || failures+="the failure took $down_ms ms; "
shot ep-002-failed
s0=$(now_ms)
$HC state >/dev/null
[ $(($(now_ms) - s0)) -lt 1500 ] || failures+="hookctl state took $(($(now_ms) - s0)) ms after the failure; "
# An endpoint that accepts and never answers: the failure comes at the 6 s timeout, and the
# window keeps answering meanwhile.
stub_start slow || failures+="the slow stub did not start; "
hclick settings.cleanup.endpointTest || failures+="the Test connection button could not be clicked; "
t=$(now_ms)
sleep 2
s0=$(now_ms)
$HC state >/dev/null
[ $(($(now_ms) - s0)) -lt 1500 ] || failures+="hookctl state took $(($(now_ms) - s0)) ms while the test waited; "
[ "$(ep .test.state)" = running ] || failures+="the test state is '$(ep .test.state)' 2 s into a silent endpoint; "
shot ep-002-running
wait_until 12 ep_is .test.state failed
slow_ms=$(($(now_ms) - t))
[ "$slow_ms" -ge 5500 ] && [ "$slow_ms" -le 7500 ] || failures+="the silent endpoint failed after $slow_ms ms, not about 6000; "
[ "$(ep .test.code)" = ENDPOINT_FAILED ] || failures+="the code after the silent endpoint is '$(ep .test.code)'; "
alive || failures+="the app died; "
stub_start || failures+="the stub did not restart; "
check_failures "VAL-EP-002 test connection" "stub up: Connected and GET /v1/models logged; stub stopped: ENDPOINT_FAILED after $down_ms ms; silent stub: ENDPOINT_FAILED after $slow_ms ms and hookctl answered meanwhile" "$failures"

echo "== VAL-EP-003 cleanup uses the endpoint"
failures=""
type_into settings.cleanup.endpointModel "$MODEL" || failures+="the model field could not be typed into; "
type_into settings.cleanup.endpointKey "$KEY1" enter || failures+="the key field could not be typed into; "
wait_until 5 ep_is .api_key_set true
[ "$(sget cleanup.llm.endpointModel)" = "$MODEL" ] || failures+="endpointModel is '$(sget cleanup.llm.endpointModel)'; "
[ "$(ep .api_key_set)" = true ] || failures+="the form says no key is saved; "
shot ep-003-form
before=$(chat_count)
ai_dictate ep-003-endpoint "$FILLER_WAV" || failures+="the dictation did not end; "
[ "$(($(chat_count) - before))" = 1 ] || failures+="the stub got $(($(chat_count) - before)) chat requests, not 1; "
chat=$(last_chat)
echo "$chat" >"$RUN_OUT/ep-003-chat.json"
[ "$(jq -r '.path' <<<"$chat")" = /v1/chat/completions ] || failures+="the request path is '$(jq -r .path <<<"$chat")'; "
[ "$(jq -r '.body.model' <<<"$chat")" = "$MODEL" ] || failures+="the request model is '$(jq -r .body.model <<<"$chat")'; "
[ "$(jq -r '.authorization' <<<"$chat")" = "Bearer $KEY1" ] || failures+="the Authorization header is '$(jq -r .authorization <<<"$chat")'; "
jq -e '.body.messages | map(select(.role == "user")) | last | .content | test("<transcript>.+</transcript>"; "s")' <<<"$chat" >/dev/null || failures+="the user message has no <transcript> block; "
case "$GOT" in *"[stub]"*) ;; *) failures+="the entry '$GOT' is not the stub reply; " ;; esac
[ "$ROW_OUTCOME" = used ] || failures+="llm_outcome is '$ROW_OUTCOME'; "
case "$ROW_LLM" in *"[stub]"*) ;; *) failures+="llm_text is '$ROW_LLM'; " ;; esac
[ "$ROW_FINAL" = "$GOT" ] || failures+="final_text '$ROW_FINAL' is not the entry '$GOT'; "
check_failures "VAL-EP-003 cleanup uses the endpoint" "one chat request: model $MODEL, Bearer key, <transcript> tags; entry '$GOT'; outcome used" "$failures"

echo "== VAL-EP-006 endpoint down or slow falls back to rule text"
failures=""
stub_stop
ai_dictate ep-006-down "$FILLER_WAV" || failures+="the dictation with the stub down did not end; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = error ] || failures+="llm_outcome is '$ROW_OUTCOME' with the stub down; "
[ "$(ai .last.code)" = ENDPOINT_FAILED ] || failures+="the hook shows code '$(ai .last.code)', not ENDPOINT_FAILED; "
down_clean_ms=$CLEAN_MS
open_settings_cleanup >/dev/null
press_test >/dev/null
[ "$(ep .test.state)" = failed ] || failures+="Test connection with a key saved and the stub down is '$(ep .test.state)'; "
stub_start slow || failures+="the slow stub did not start; "
ai_dictate ep-006-slow "$FILLER_WAV" || failures+="the dictation with the slow stub did not end; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the slow entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = timeout ] || failures+="llm_outcome is '$ROW_OUTCOME' with the slow stub; "
slow_clean_ms=$CLEAN_MS
[ -n "$slow_clean_ms" ] && [ "$slow_clean_ms" -le 7000 ] || failures+="the rule text came ${slow_clean_ms:-?} ms after cleaning started, over 7000; "
[ -n "$slow_clean_ms" ] && [ "$slow_clean_ms" -ge 5500 ] || failures+="the slow stub gave up after ${slow_clean_ms:-?} ms, under the 6 s timeout; "
alive || failures+="the app died; "
stub_start || failures+="the stub did not restart; "
check_failures "VAL-EP-006 down or slow" "stub down: rule text, outcome error, code ENDPOINT_FAILED (${down_clean_ms} ms); 30 s stub: rule text ${slow_clean_ms} ms after cleaning started, outcome timeout" "$failures"

echo "== VAL-EP-007 the output guards also apply to the endpoint"
failures=""
for kind in poem refuse; do
  stub_mode "$kind"
  ai_dictate "ep-007-$kind" "$FILLER_WAV" || failures+="the $kind dictation did not end; "
  [ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the $kind entry '$GOT' is not the rule text '$ROW_RULE'; "
  [ "$ROW_OUTCOME" = guard_rejected ] || failures+="llm_outcome for $kind is '$ROW_OUTCOME'; "
done
stub_mode default
check_failures "VAL-EP-007 guards" "a 400 character poem and 'I cannot help with that.' both inserted the rule text with outcome guard_rejected" "$failures"

echo "== VAL-EP-008 the network audit lists the endpoint as user-configured"
failures=""
type_into settings.cleanup.endpointUrl "$LOOPBACK_URL" || failures+="the address field could not be typed into; "
wait_until 5 ep_is .url "$LOOPBACK_URL"
trace_start "$RUN_OUT/ep-008-connect.strace" || failures+="strace could not attach to the app; "
net_from=$($HC net | jq 'length')
ai_dictate ep-008-up "$FILLER_WAV" || failures+="the dictation with the stub up did not end; "
case "$GOT" in *"[stub]"*) ;; *) failures+="the entry '$GOT' is not the stub reply; " ;; esac
[ "$ROW_OUTCOME" = used ] || failures+="llm_outcome with the stub up is '$ROW_OUTCOME'; "
stub_stop
ai_dictate ep-008-down "$FILLER_WAV" || failures+="the dictation with the stub down did not end; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = error ] || failures+="llm_outcome with the stub down is '$ROW_OUTCOME'; "
trace_stop
seen=$(connects "$RUN_OUT/ep-008-connect.strace")
[ "$seen" = "127.0.0.1:11434" ] || failures+="the app connected to '$seen', not only 127.0.0.1:11434; "
$HC net | jq --argjson n "$net_from" '.[$n:]' >"$RUN_OUT/ep-008-net.json"
[ "$(jq 'length' "$RUN_OUT/ep-008-net.json")" -ge 2 ] || failures+="hookctl net lists fewer than 2 requests for the session; "
[ "$(jq '[.[] | select(.purpose != "endpoint")] | length' "$RUN_OUT/ep-008-net.json")" = 0 ] || failures+="hookctl net lists a request that is not an endpoint request; "
[ "$(jq -c '[.[].host] | unique' "$RUN_OUT/ep-008-net.json")" = '["127.0.0.1"]' ] || failures+="the endpoint requests name hosts $(jq -c '[.[].host] | unique' "$RUN_OUT/ep-008-net.json"); "
check_failures "VAL-EP-008 audit session" "strace connects: $seen; stub up: reply inserted; stub stopped: rule text, outcome error; hookctl net lists only endpoint requests to 127.0.0.1 (the user-configured label in cargo xtask net-audit is checked by its unit test)" "$failures"
stub_start

echo "== VAL-EP-004 the API key lives only in a 0600 secrets file"
failures=""
[ "$(mode_of "$SETTINGS_FILE")" != 600 ] || failures+="settings.json is 0600, so the suite's umask 000 did not reach the app; "
[ "$(mode_of "$SECRETS")" = 600 ] || failures+="secrets.json has mode '$(mode_of "$SECRETS")'; "
grep -q "$KEY1" "$SECRETS" 2>/dev/null || failures+="secrets.json does not hold the key; "
grep -q 'sk-test' "$SETTINGS_FILE" && failures+="settings.json holds the key; "
leaks=$(grep -rl 'sk-test' /data 2>/dev/null | tr '\n' ' ')
[ "$leaks" = "$SECRETS " ] || failures+="files under /data that hold the key: '$leaks'; "
cp "$SECRETS" "$RUN_OUT/ep-004-secrets.json"
ls -l "$SECRETS" "$SETTINGS_FILE" >"$RUN_OUT/ep-004-modes.txt"
open_settings_cleanup >/dev/null
type_into settings.cleanup.endpointKey "$KEY2" enter || failures+="the key field could not be typed into; "
wait_until 5 grep -q "$KEY2" "$SECRETS"
[ "$(mode_of "$SECRETS")" = 600 ] || failures+="secrets.json has mode '$(mode_of "$SECRETS")' after the key changed; "
grep -q "$KEY2" "$SECRETS" || failures+="secrets.json does not hold the new key; "
grep -q "$KEY1" "$SECRETS" && failures+="secrets.json still holds the old key; "
ai_dictate ep-004-changed "$FILLER_WAV" || failures+="the dictation with the changed key did not end; "
[ "$(last_chat | jq -r .authorization)" = "Bearer $KEY2" ] || failures+="the Authorization header after the change is '$(last_chat | jq -r .authorization)'; "
hclick settings.cleanup.endpointKeyClear || failures+="the Remove key button could not be clicked; "
wait_until 5 ep_is .api_key_set false
[ "$(ep .api_key_set)" = false ] || failures+="the form still says a key is saved; "
grep -q 'sk-test' "$SECRETS" 2>/dev/null && failures+="secrets.json still holds a key after Remove key; "
shot ep-004-cleared
before=$(chat_count)
ai_dictate ep-004-no-key "$FILLER_WAV" || failures+="the dictation with no key did not end; "
[ "$(($(chat_count) - before))" = 1 ] || failures+="the stub got $(($(chat_count) - before)) chat requests after Remove key; "
[ "$(last_chat | jq -r '.authorization')" = null ] || failures+="the request after Remove key has Authorization '$(last_chat | jq -r .authorization)'; "
check_failures "VAL-EP-004 0600 secrets file" "secrets.json 0600 under umask 000 (settings.json $(mode_of "$SETTINGS_FILE")) and only it held the key; still 0600 after a change; Remove key emptied it and the next request had no Authorization header" "$failures"

echo "== VAL-EP-009 plain http is refused for hosts other than loopback"
failures=""
open_settings_cleanup >/dev/null
type_into settings.cleanup.endpointUrl "$REMOTE_URL" || failures+="the address field could not be typed into; "
wait_until 5 ep_is .url "$REMOTE_URL"
[ "$(ep .preset)" = custom ] || failures+="the preset is '$(ep .preset)' for a typed address; "
trace_start "$RUN_OUT/ep-009-connect.strace" || failures+="strace could not attach to the app; "
before_net=$($HC net | jq 'length')
before_chat=$(chat_count)
press_test || failures+="no answer from Test connection for the http address; "
[ "$(ep .test.state)" = failed ] || failures+="the test state is '$(ep .test.state)'; "
[ "$(ep .test.code)" = ENDPOINT_FAILED ] || failures+="the test code is '$(ep .test.code)'; "
case "$(ep .test.reason)" in *https*) ;; *) failures+="the reason '$(ep .test.reason)' does not say https is needed; " ;; esac
[ "$(tree_text settings.cleanup.endpointStatus)" = "Use https. Plain http works only for this computer." ] || failures+="the status row says '$(tree_text settings.cleanup.endpointStatus)', not the plain sentence; "
shot ep-009-refused
refused_ms=$TEST_MS
ai_dictate ep-009-dictation "$FILLER_WAV" || failures+="the dictation did not end; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = error ] || failures+="llm_outcome is '$ROW_OUTCOME'; "
trace_stop
seen=$(connects "$RUN_OUT/ep-009-connect.strace")
case " $seen " in *" 10.0.0.2:"*) failures+="the app connected to 10.0.0.2 ($seen); " ;; esac
grep -q '10\.0\.0\.2' "$RUN_OUT/ep-009-connect.strace" && failures+="the strace log names 10.0.0.2; "
[ "$(chat_count)" = "$before_chat" ] || failures+="the stub got a chat request for the refused address; "
$HC net >"$RUN_OUT/ep-009-net.json"
[ "$(jq 'length' "$RUN_OUT/ep-009-net.json")" -gt "$before_net" ] || failures+="hookctl net lists no entry for the refused requests; "
jq -e '[.[] | select(.purpose == "endpoint" and (.result | startswith("blocked")))] | length > 0' "$RUN_OUT/ep-009-net.json" >/dev/null || failures+="hookctl net has no blocked endpoint entry; "
check_failures "VAL-EP-009 http refused" "Test connection refused in ${refused_ms} ms with ENDPOINT_FAILED and a reason naming https; the dictation inserted the rule text; no connect to 10.0.0.2 (connects: ${seen:-none}); the refusals are in hookctl net as blocked" "$failures"

echo "== VAL-EP-005 the key never appears in logs or the hook state"
failures=""
# Put the key back and run one more request, a failed test and a cleanup error, so the files
# hold a full session with the key in use.
open_settings_cleanup >/dev/null
type_into settings.cleanup.endpointUrl "$LOOPBACK_URL" || failures+="the address field could not be typed into; "
type_into settings.cleanup.endpointKey "$KEY1" enter || failures+="the key field could not be typed into; "
wait_until 5 ep_is .api_key_set true
ai_dictate ep-005-session "$FILLER_WAV" || failures+="the dictation did not end; "
stub_stop
press_test >/dev/null
ai_dictate ep-005-error "$FILLER_WAV" || failures+="the dictation with the stub down did not end; "
[ "$ROW_OUTCOME" = error ] || failures+="llm_outcome with the stub down is '$ROW_OUTCOME'; "
for what in state events net tree; do $HC "$what" >"$RUN_OUT/ep-005-hook-$what.json"; done
hits=$(grep -l "$KEY1" "$RUN_OUT"/ep-005-hook-*.json 2>/dev/null | tr '\n' ' ')
[ -z "$hits" ] || failures+="the hook output holds the key: $hits; "
log_hits=$(grep -rl "$KEY1" /data/logs /logs 2>/dev/null | tr '\n' ' ')
[ -z "$log_hits" ] || failures+="log files hold the key: $log_hits; "
ls -l /data/logs >"$RUN_OUT/ep-005-logs.txt" 2>&1
[ "$(find /data/logs -type f | wc -l)" -ge 1 ] || failures+="there is no log file to search; "
grep -q "$KEY1" "$SECRETS" || failures+="the key is not in secrets.json after the session, so the check proves nothing; "
check_failures "VAL-EP-005 no key in logs or state" "after endpoint cleanups, a failed test connection and an endpoint error: grep -r $KEY1 over /data/logs, /logs and the hookctl state, events, net and tree dumps finds nothing" "$failures"

finish endpoint
