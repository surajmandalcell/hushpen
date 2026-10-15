#!/bin/bash
# The whole-session network audits: VAL-CROSS-005 (update checks off: zero connects) and
# VAL-CROSS-006 (checks on against the local signed feed with nothing newer: connects only to the
# feed address, only manifest requests). One scripted session runs twice under strace from the
# first instruction (APP_STRACE): start, a push-to-talk run into a stock xterm, a dictation with
# local AI cleanup (Qwen3.5 2B), a History search, Reprocess, a TXT export, an import of
# speech-short.flac, a visit to every Settings section, About, and Quit through the tray menu.
# Both runs set HUSHPEN_UPDATE_FEED and HUSHPEN_UPDATE_PUBKEY, so the off run also proves that
# the override alone starts nothing. The feed is harness/slot/update-feed-server.py in its `same`
# mode (the version that runs), see updates.sh.
# Needs speech-short.wav and speech-short.flac in /assets/fixtures, the base model in /assets, and
# filler-dictation.wav and llm/Qwen3.5-2B-Q4_K_M.gguf in /fixtures. Start the slot with
# VAL_CPUS=4 VAL_MEM=4g (an LLM slot counts as 2 busy slots). It empties /data, so the slot must
# not be shared.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
FEED_PORT=8765
FEED_URL=http://127.0.0.1:$FEED_PORT/latest.json
FEED_KEY=/harness/slot/update-feed/test.pub
FEED_LOG=$RUN_OUT/feed-requests.jsonl
FEED_PID=/tmp/feed.pid
FEED_MODE=/tmp/feed-mode
DB=/data/history/history.db
SETTINGS_FILE=/data/config/settings.json
STOCK_FILE=/out/xterm-stock.txt
GTK_FILE=/out/gtk.txt
EXPORT_FILE=/out/session-export.txt
FILLER_WAV=/fixtures/filler-dictation.wav
SHORT_WAV=$FIX/speech-short.wav
FLAC=$FIX/speech-short.flac
MODEL_ID=qwen3.5-2b
MODEL_FILE=/data/models/llm/$MODEL_ID.gguf
FIXTURE_GGUF=/fixtures/llm/Qwen3.5-2B-Q4_K_M.gguf
SECTION_KEYS="general shortcuts audio cleanup flowbar storage updates"

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
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
llm() { $HC state | jq -r ".llm$1"; }
llm_is() { [ "$(llm "$1")" = "$2" ]; }
hs() { $HC state | jq -r ".history$1"; }
imp() { $HC state | jq -r ".import$1"; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
q() { sqlite3 "$DB" "$1"; }
engine_ready() { [ "$(engine .state)" = ready ]; }
hook_up() { $HC state >/dev/null 2>&1; }
item() { $HC state | jq -r --arg name "$1" "[.import.items[] | select(.name == \$name)] | last | $2"; }
item_final() { case "$(item "$1" .state)" in done | failed | cancelled) return 0 ;; esac; return 1; }
reprocess_idle() { [ "$(hs .reprocess)" = null ]; }
ptt_run() { # <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}
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

feed_up() { python3 -c 'import socket; socket.create_connection(("127.0.0.1", 8765), 1).close()' 2>/dev/null; }
feed_stop() {
  [ -f "$FEED_PID" ] || return 0
  kill "$(cat "$FEED_PID")" 2>/dev/null
  rm -f "$FEED_PID"
}
trap feed_stop EXIT
feed_count() { if [ -s "$FEED_LOG" ]; then wc -l <"$FEED_LOG" | tr -d ' '; else echo 0; fi; }
feed_after() { if [ -s "$FEED_LOG" ]; then tail -n +$(($1 + 1)) "$FEED_LOG"; fi; }

connects() { # <strace file>: the distinct host:port pairs the app connected to over IP
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
inet_connects() { grep -cE 'AF_INET6?' "$1" 2>/dev/null || true; }

# session <label> <updates on|off>: the scripted session; sets SESSION_FAILURES and SESSION_STEPS
session() {
  local label=$1 updates=$2 strace_file=$RUN_OUT/$1.strace ids rows id words
  SESSION_FAILURES=""
  SESSION_STEPS=""
  step() { SESSION_STEPS+="$1; "; echo "step: $1"; }
  fail() { SESSION_FAILURES+="$label: $1; "; }

  /harness/slot/app.sh stop
  # The models stay between the two runs: the library hashes a copied model file only once.
  find /data -mindepth 1 -maxdepth 1 ! -name models -exec rm -rf {} +
  mkdir -p /data/models/whisper /data/models/llm /data/config
  [ -e /data/models/whisper/ggml-base.bin ] || cp /assets/models/whisper/ggml-base.bin /data/models/whisper/
  [ -e "$MODEL_FILE" ] || cp "$FIXTURE_GGUF" "$MODEL_FILE"
  local check=false
  [ "$updates" = on ] && check=true
  jq -n --argjson check "$check" \
    '{schemaVersion: 1, values: {"onboarding.completed": true, "updates.check": $check}}' >"$SETTINGS_FILE"

  env APP_STRACE="$strace_file" HUSHPEN_UPDATE_FEED="$FEED_URL" HUSHPEN_UPDATE_PUBKEY="$FEED_KEY" \
    /harness/slot/app.sh start >/dev/null || fail "the app did not start under strace"
  wait_until 20 hook_up || fail "no hook after the start"
  wait_until 600 test -e "$MODEL_FILE.verified" || fail "the LLM file was never verified"
  use_model base || fail "the base model did not load"
  step "start, models verified"

  ids=$(/harness/slot/targets.sh --stock)
  GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
  ST=$(grep -o 'xterm_stock=[0-9]*' <<<"$ids" | cut -d= -f2)

  # A push-to-talk run into a stock xterm (Shift+Insert with PRIMARY).
  local size
  size=$(stat -c %s "$STOCK_FILE" 2>/dev/null || echo 0)
  focus "$ST"
  ptt_run "$SHORT_WAV" || fail "the xterm run did not end"
  sleep 0.3
  xdotool key Return
  sleep 0.5
  words=$(tail -c +$((size + 1)) "$STOCK_FILE" 2>/dev/null | tr -d '\n')
  [ -z "$(missing_words "$(norm <<<"$words")" "${SHORT_WORDS[@]}")" ] || fail "xterm got '$words'"
  step "push-to-talk into xterm"

  # A dictation with local AI cleanup.
  set_setting cleanup.llm.provider '"local"'
  set_setting cleanup.llm.modelId "\"$MODEL_ID\""
  set_setting cleanup.llm.enabled true
  wait_until 240 llm_is .loaded true || fail "the AI cleanup model did not load"
  gtk_clear
  focus "$GT"
  ptt_run "$FILLER_WAV" || fail "the AI cleanup run did not end"
  words=$(gtk_commit)
  [ "$(q "select llm_outcome from transcript order by created_at desc, id desc limit 1")" = used ] ||
    fail "the AI cleanup outcome is '$(q "select llm_outcome from transcript order by created_at desc, id desc limit 1")'"
  step "dictation with local AI cleanup"

  # History search, Reprocess, and a TXT export.
  focus "$(app_window)"
  $HC click sidebar.history >/dev/null
  wait_until 10 view_is history || fail "History did not open"
  hook_action history-search '{"query":"quick"}' >/dev/null
  sleep 0.6
  step "History search"
  id=$(q "select id from transcript where llm_outcome != 'used' order by created_at limit 1")
  [ -n "$id" ] || id=$(q "select id from transcript order by created_at limit 1")
  hook_action history-reprocess "{\"id\":\"$id\"}" >/dev/null || fail "Reprocess was refused"
  sleep 1
  wait_until 240 reprocess_idle || fail "Reprocess did not end"
  [ "$(q "select status from transcript where id = '$id'")" = completed ] || fail "the reprocessed row is not completed"
  step "Reprocess"
  rm -f "$EXPORT_FILE"
  $HC paths "$EXPORT_FILE" >/dev/null
  hook_action history-export "{\"format\":\"txt\",\"ids\":[\"$id\"]}" >/dev/null
  wait_until 20 test -s "$EXPORT_FILE" || fail "the TXT export wrote no file"
  step "TXT export"
  hook_action history-search '{"query":""}' >/dev/null

  # An import of speech-short.flac.
  $HC click sidebar.import >/dev/null
  wait_until 10 view_is import || fail "Import did not open"
  hook_action import-add "{\"paths\":[\"$FLAC\"]}" >/dev/null
  wait_until 300 item_final speech-short.flac || fail "the import did not finish"
  [ "$(item speech-short.flac .state)" = done ] || fail "the import ended as '$(item speech-short.flac .state)'"
  step "import of speech-short.flac"

  # Every Settings section, then About. On a run with checks on the Updates section also gets a Check now.
  $HC click sidebar.settings >/dev/null
  wait_until 10 view_is settings || fail "Settings did not open"
  for key in $SECTION_KEYS; do
    wait_until 10 tree_has "settings.section.$key" || fail "no tab $key"
    $HC click "settings.section.$key" >/dev/null
    sleep 0.4
  done
  if [ "$updates" = on ]; then
    hclick settings.updates.checkNow || fail "Check now could not be clicked"
    sleep 3
  fi
  step "every Settings section"
  $HC click sidebar.about >/dev/null
  wait_until 10 view_is about || fail "About did not open"
  step "About"
  $HC events >"$RUN_OUT/$label-events.json"
  $HC net >"$RUN_OUT/$label-net.json"
  shot "$label-before-quit"

  # Quit through the tray menu; the strace file is complete when the app has gone.
  local pid
  pid=$(ui_pid)
  python3 /harness/slot/tray-menu.py click Quit >>"$RUN_OUT/tray-clicks.log" 2>&1 || fail "no Quit entry in the tray menu"
  wait_until 15 bash -c "! kill -0 $pid 2>/dev/null" || fail "the app did not quit"
  rm -f /tmp/app.pid
  sleep 1
  step "Quit"
}

echo "== setup: the local feed (no newer version) and the paste targets"
rm -f "$FEED_LOG"
echo same >"$FEED_MODE"
python3 /harness/slot/update-feed-server.py "$FEED_LOG" "$FEED_PORT" >/dev/null 2>&1 &
echo $! >"$FEED_PID"
wait_until 5 feed_up || echo "the feed did not start"
for wav in "$SHORT_WAV" "$FLAC" "$FILLER_WAV" "$FIXTURE_GGUF"; do
  [ -e "$wav" ] || echo "missing fixture $wav"
done

echo "== VAL-CROSS-005 a full session with update checks off makes zero network connects"
n0=$(feed_count)
session off-session off
failures=$SESSION_FAILURES
steps=$SESSION_STEPS
inet=$(inet_connects "$RUN_OUT/off-session.strace")
seen=$(connects "$RUN_OUT/off-session.strace")
[ -s "$RUN_OUT/off-session.strace" ] || failures+="the strace file is empty, so the audit saw nothing; "
grep -q 'AF_UNIX' "$RUN_OUT/off-session.strace" || failures+="the strace file has no AF_UNIX connect, so it did not follow the app; "
[ "$inet" = 0 ] || failures+="strace logged $inet IP connect call(s): $seen; "
[ "$(feed_count)" = "$n0" ] || failures+="the feed log grew by $(($(feed_count) - n0)) request(s); "
[ "$(jq length "$RUN_OUT/off-session-net.json")" = 0 ] || failures+="hookctl net lists $(jq length "$RUN_OUT/off-session-net.json") request(s); "
check_failures "VAL-CROSS-005 zero connects with checks off" "steps: ${steps}0 IP connect calls over the whole session (strace -f from the first instruction), 0 feed requests, hookctl net empty" "$failures"

echo "== VAL-CROSS-006 with update checks on, only the feed is contacted"
n1=$(feed_count)
session on-session on
failures=$SESSION_FAILURES
steps=$SESSION_STEPS
seen=$(connects "$RUN_OUT/on-session.strace")
[ "$seen" = "127.0.0.1:$FEED_PORT" ] || failures+="the app connected to '$seen', not only 127.0.0.1:$FEED_PORT; "
paths=$(feed_after "$n1" | jq -rs '[.[].path] | unique | join(",")')
[ "$paths" = "/latest.json,/latest.json.minisig" ] || failures+="the feed saw '$paths'; "
requests=$(feed_after "$n1" | jq -s '[.[] | select(.path == "/latest.json")] | length')
[ "$requests" -ge 1 ] || failures+="the feed saw no manifest request, so the check never ran; "
jq -e '[.[] | select(.purpose != "update_check")] | length == 0' "$RUN_OUT/on-session-net.json" >/dev/null || failures+="hookctl net lists a request that is not an update check; "
feed_after "$n1" >"$RUN_OUT/on-session-feed.jsonl"
check_failures "VAL-CROSS-006 only the feed host" "steps: ${steps}connects: $seen; the feed saw $requests manifest request(s) and only $paths; hookctl net lists only update_check" "$failures"

finish update-session
