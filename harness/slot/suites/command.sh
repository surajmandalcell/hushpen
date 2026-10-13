#!/bin/bash
# Command Mode: VAL-CMD-001 to 006. The chord is Right Alt plus Right Shift (X11 keycodes 108 and
# 62), pressed with xdotool the way a person does: Right Alt first (the app only arms the hold,
# with no cue and no bar), then Right Shift, which starts the Command Mode session or, when the
# start is refused, leaves no session, cue, or listening state at all. The instruction
# is the "make this formal" fixture played into the virtual microphone. The selection is made in
# the GTK target (target-gtk.py) with Shift+Right, so the entry owns the X11 PRIMARY selection;
# the app reads PRIMARY, pastes the edit over the selection with Ctrl+V, and puts the sentinel
# back on the clipboard. /out/gtk-live.txt holds the entry's text.
# Part one runs against the stub endpoint (slot/openai-stub.py on 127.0.0.1:11434): its default
# answer to a Command Mode request is the selection plus " [edited]", mode `edited` answers
# "EDITED TEXT", and mode `slow` answers after 30 s. Part two uses the local Qwen3.5 2B model for
# a real edit and for the kill -9 and kill -STOP cases. Needs command-formal.wav in
# HUSHPEN_TEST_FIXTURES (mounted at /fixtures), the llm folder there, and the base model in
# /assets. Start the slot with VAL_CPUS=4 VAL_MEM=4g (the local model counts as 2 busy slots). It
# empties /data, so the slot must not be shared.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

LIVE=/out/gtk-live.txt
DB=/data/history/history.db
CMD_WAV=/fixtures/command-formal.wav
STUB_LOG=$RUN_OUT/stub-requests.jsonl
STUB_PID=/tmp/stub.pid
STUB_MODE=/tmp/stub-mode
STUB_URL=http://127.0.0.1:11434/v1
MODEL_ID=qwen3.5-2b
MODEL_FILE=/data/models/llm/$MODEL_ID.gguf
FIXTURE_GGUF=/fixtures/llm/Qwen3.5-2B-Q4_K_M.gguf
SENTINEL=SENTINEL-COMMAND
WATCH=/tmp/command-watch.txt
PREFIX_TEXT="PREFIX "
SUFFIX_TEXT=" SUFFIX"
SHORT_MID="hey can you send me the file"
LONG_MID="hey so I was wondering if you could maybe send me the file for the budget review whenever you get a chance because I need to look at it before the meeting tomorrow morning"
MARKER_MID="hey can you send me the zebrafinch file"

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pmode_is() { [ "$($HC state | jq -r '.pipeline.mode // ""')" = "$1" ]; }
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
last_row() { sqlite3 "$DB" "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
row_count() { sqlite3 "$DB" 'select count(*) from transcript'; }
live() { cat "$LIVE" 2>/dev/null; }
live_is() { [ "$(live)" = "$1" ]; }
clip_is() { [ "$(clip)" = "$1" ]; }
primary() { xclip -o -selection primary 2>/dev/null; }
engine_ready() { [ "$($HC state | jq -r .engine.state)" = ready ]; }
llm_is() { [ "$(llm "$1")" = "$2" ]; }
bar() { $HC state | jq -r ".overlay$1"; }
bar_is() { [ "$(bar .state)" = "$1" ]; }
bar_failed() { case "$(bar .state)" in error | blocked) return 0 ;; esac; return 1; }
proc_dead() { [ ! -d "/proc/$1" ] || grep -q '^State:[[:space:]]*Z' "/proc/$1/status" 2>/dev/null; }
new_pid_after() { # <old pid>: a running worker with another pid
  local pid
  pid=$(llm .pid)
  [ "$pid" != null ] && [ -n "$pid" ] && [ "$pid" != "$1" ]
}
loaded_now() { [ "$(llm .loaded)" = true ]; }

# The pipeline, the mode, and the bar, sampled every 50 ms into $WATCH as state|mode|bar|message.
watch_start() {
  : >"$WATCH"
  (
    while :; do
      $HC state 2>/dev/null |
        jq -r '[.pipeline.state, (.pipeline.mode // ""), .overlay.state, (.overlay.message // "")] | join("|")' >>"$WATCH"
      sleep 0.05
    done
  ) &
  WATCH_PID=$!
}
watch_stop() {
  [ -n "${WATCH_PID:-}" ] || return 0
  kill "$WATCH_PID" 2>/dev/null
  wait "$WATCH_PID" 2>/dev/null
  WATCH_PID=
}
saw() { grep -qF -- "$1" "$WATCH"; }
saw_mode() { cut -d'|' -f2 "$WATCH" | grep -qx "$1"; }
saw_pstate() { cut -d'|' -f1 "$WATCH" | grep -qx "$1"; }

stub_up() { python3 -c 'import socket; socket.create_connection(("127.0.0.1", 11434), 1).close()' 2>/dev/null; }
stub_stop() {
  [ -f "$STUB_PID" ] || return 0
  kill "$(cat "$STUB_PID")" 2>/dev/null
  rm -f "$STUB_PID"
  for _ in $(seq 30); do stub_up || return 0; sleep 0.1; done
}
stub_start() { # [mode]
  stub_stop
  echo "${1:-default}" >"$STUB_MODE"
  python3 /harness/slot/openai-stub.py "$STUB_LOG" >/dev/null 2>&1 &
  echo $! >"$STUB_PID"
  wait_until 5 stub_up
}
stub_mode() { echo "$1" >"$STUB_MODE"; }
stub_posts() { # chat completion requests the stub has seen
  if [ -s "$STUB_LOG" ]; then
    jq -s '[.[] | select(.method == "POST")] | length' "$STUB_LOG"
  else
    echo 0
  fi
}
trap 'watch_stop; stub_stop' EXIT

# set_entry <before> <selected> <after>: the entry holds the three parts with the middle one
# selected by the keyboard, so the entry owns PRIMARY. An empty middle leaves nothing selected.
set_entry() {
  P=$1 MID=$2 S=$3
  ORIG="$P$MID$S"
  focus "$GT"
  xdotool key ctrl+a BackSpace
  sleep 0.2
  wait_until 3 live_is ""
  xdotool type --delay 15 -- "$ORIG"
  wait_until 5 live_is "$ORIG" || return 1
  xdotool key Home
  [ ${#P} -gt 0 ] && xdotool key --delay 15 --repeat ${#P} Right
  if [ ${#MID} -gt 0 ]; then
    xdotool key --delay 15 --repeat ${#MID} shift+Right
  else
    xdotool key End
  fi
  sleep 0.4
  set_clip "$SENTINEL"
}
chord_down() { # Right Alt only arms the hold; Right Shift 30 ms later makes it the command keys
  T0=$(now_ms)
  xdotool keydown 108 sleep 0.03 keydown 62
}
# visible_start: since the chord went down no start cue (or any cue) played and neither the
# pipeline nor the dictation showed listening; an empty answer means nothing started
visible_start() {
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.t_ms >= $t0 and (.kind == "cue" or ((.kind == "pipeline" or .kind == "dictation") and .detail == "listening"))) | "\(.kind):\(.detail)"] | join(",")'
}
chord_up() {
  xdotool keyup 62
  xdotool keyup 108
}
# command_run <wav>: the chord, the words, release; returns when the run has ended
command_run() {
  chord_down || return 1
  wait_until 3 pmode_is command || {
    chord_up
    return 2
  }
  paplay --device=vmic "$(pad "$1")"
  chord_up
  wait_until "${RUN_WAIT:-120}" run_ended
}
# command_to_cleaning <wav>: the same run, but it returns as soon as the state is cleaning
command_to_cleaning() {
  chord_down || return 1
  wait_until 3 pmode_is command || {
    chord_up
    return 2
  }
  paplay --device=vmic "$(pad "$1")"
  chord_up
  local deadline=$((SECONDS + 90))
  while [ $SECONDS -le $deadline ]; do
    pstate_is cleaning && return 0
    case ",$(pevents)," in *,done,* | *,failed,* | *,cancelled,*) return 1 ;; esac
    sleep 0.03
  done
  return 1
}
# edited_middle: what the entry holds between the prefix and the suffix; fails when the entry no
# longer starts with the prefix and ends with the suffix
edited_middle() {
  local text
  text=$(live)
  case "$text" in "$P"*"$S") ;; *) return 1 ;; esac
  text=${text#"$P"}
  text=${text%"$S"}
  printf '%s' "$text"
}
unchanged() { # the entry text and the selection are as set_entry left them
  live_is "$ORIG" && [ "$(primary)" = "$MID" ]
}
read_row() {
  ROW_KIND=$(last_row kind)
  ROW_INSTRUCTION=$(last_row instruction)
  ROW_FINAL=$(last_row final_text)
  ROW_STATUS=$(last_row status)
}
columns_with() { # <word>: the columns of the newest row that hold the word
  local column out=""
  for column in $(sqlite3 "$DB" "select name from pragma_table_info('transcript')"); do
    if [ "$(sqlite3 "$DB" "select count(*) from (select $column as v from transcript order by created_at desc, id desc limit 1) where instr(lower(coalesce(v, '')), lower('$1')) > 0")" = 1 ]; then
      out+="$column "
    fi
  done
  echo "$out"
}
columns_equal() { # <text>: the columns of the newest row that equal the text
  local column out=""
  for column in $(sqlite3 "$DB" "select name from pragma_table_info('transcript')"); do
    if [ "$(sqlite3 "$DB" "select count(*) from (select $column as v from transcript order by created_at desc, id desc limit 1) where v = '$1'")" = 1 ]; then
      out+="$column "
    fi
  done
  echo "$out"
}
mentions_instruction() { # <text>: the spoken words "make this formal" are in the text
  case " $(norm <<<"$1") " in *" make this formal "*) return 0 ;; esac
  return 1
}
stub_last_post() { jq -s '[.[] | select(.method == "POST")] | last' "$STUB_LOG"; }

echo "== setup: base model, the paste targets, and the stub endpoint"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
for file in "$CMD_WAV" "$FIXTURE_GGUF"; do
  [ -e "$file" ] || echo "missing fixture $file"
done
rm -f "$STUB_LOG"
stub_start || echo "the stub did not start"
set_setting cleanup.llm.provider '"endpoint"'
set_setting cleanup.llm.endpointUrl "\"$STUB_URL\""
set_setting cleanup.llm.endpointModel '"stub-model"'
set_setting cleanup.llm.enabled true

echo "== VAL-CMD-001 Command Mode replaces the selected text"
failures=""
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
[ "$(primary)" = "$SHORT_MID" ] || failures+="PRIMARY holds '$(primary)' before the run, not the selection '$SHORT_MID'; "
rows=$(row_count)
posts=$(stub_posts)
watch_start
command_run "$CMD_WAV"
rc=$?
watch_stop
[ $rc = 0 ] || failures+="the command run ended with status $rc (events: $(pevents)); "
shot cmd-001-after
saw_mode command || failures+="the pipeline never showed mode command; "
new_middle=$(edited_middle) || failures+="the entry '$(live)' lost the text around the selection; "
[ -n "$new_middle" ] && [ "$new_middle" != "$SHORT_MID" ] || failures+="the middle part is '$new_middle', not a new text; "
mentions_instruction "$(live)" && failures+="the spoken instruction is in the entry: $(live); "
wait_until 5 clip_is "$SENTINEL" || failures+="the clipboard holds '$(clip)', not the sentinel; "
[ "$(stub_posts)" -gt "$posts" ] || failures+="the stub saw no request; "
read_row
[ "$ROW_KIND" = command ] || failures+="the row kind is '$ROW_KIND'; "
[ "$(row_count)" = $((rows + 1)) ] || failures+="the run added $(($(row_count) - rows)) rows; "
check_failures "VAL-CMD-001 selection replaced" "mode command, '$SHORT_MID' became '$new_middle' inside '$PREFIX_TEXT...$SUFFIX_TEXT', the instruction was not inserted, the sentinel is back" "$failures"

echo "== VAL-CMD-002 no selection shows the notice and changes nothing"
failures=""
set_entry "$PREFIX_TEXT" "" "$SHORT_MID$SUFFIX_TEXT" || failures+="the entry did not take the text; "
[ -z "$(primary)" ] || failures+="PRIMARY still holds '$(primary)'; "
rows=$(row_count)
posts=$(stub_posts)
watch_start
chord_down || failures+="the chord keys did not go down; "
sleep 0.3
during=$(visible_start)
[ -z "$during" ] || failures+="the chord showed a start while it was down: $during; "
paplay --device=vmic "$(pad "$CMD_WAV")"
wait_until 5 bar_failed
notice_state=$(bar .state)
notice=$(bar .message)
shot cmd-002-notice
chord_up
sleep 1
watch_stop
after=$(visible_start)
[ -z "$after" ] || failures+="the chord left a cue or a listening state: $after; "
[ "$notice_state" = blocked ] || failures+="the bar shows '$notice_state', not the blocked notice; "
case "$notice" in *"Select text first"*) ;; *) failures+="the bar says '$notice', not 'Select text first'; " ;; esac
saw_mode command && failures+="a command session started; "
saw_pstate cleaning && failures+="the run reached cleaning; "
live_is "$ORIG" || failures+="the entry changed to '$(live)'; "
[ "$(stub_posts)" = "$posts" ] || failures+="the stub saw a request; "
[ "$(row_count)" = "$rows" ] || failures+="the run added a history row; "
wait_until 5 pstate_is idle || failures+="the pipeline is '$(pstate)', not idle; "
clip_is "$SENTINEL" || failures+="the clipboard holds '$(clip)', not the sentinel; "
check_failures "VAL-CMD-002 no selection" "the bar showed '$notice' ($notice_state), the entry, the stub log, and History are unchanged" "$failures"

echo "== VAL-CMD-003 the History row is type Command and never stores the selection"
failures=""
set_entry "$PREFIX_TEXT" "$MARKER_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
command_run "$CMD_WAV" || failures+="the command run did not end; "
shot cmd-003-after
read_row
new_middle=$(edited_middle) || failures+="the entry '$(live)' lost the text around the selection; "
[ "$ROW_KIND" = command ] || failures+="the row kind is '$ROW_KIND'; "
case "$(norm <<<"$ROW_INSTRUCTION")" in *formal*) ;; *) failures+="the instruction column is '$ROW_INSTRUCTION'; " ;; esac
[ "$ROW_FINAL" = "$new_middle" ] && [ -n "$ROW_FINAL" ] || failures+="final_text '$ROW_FINAL' is not the edited text '$new_middle'; "
equal=$(columns_equal "$MARKER_MID")
[ -z "$equal" ] || failures+="the columns $equal equal the selected text; "
holders=$(columns_with zebrafinch)
[ "$holders" = "final_text " ] || failures+="zebrafinch is in '$holders', not only final_text; "
if grep -rqi zebrafinch /data/logs /logs 2>/dev/null; then failures+="zebrafinch is in the logs: $(grep -rli zebrafinch /data/logs /logs 2>/dev/null | tr '\n' ' '); "; fi
if $HC events | jq -r '.[] | tostring' | grep -qi zebrafinch; then failures+="zebrafinch is in the hook events; "; fi
sqlite3 -line "$DB" "select * from transcript order by created_at desc limit 1" >"$RUN_OUT/cmd-003-row.txt"
check_failures "VAL-CMD-003 history row" "kind command, instruction '$ROW_INSTRUCTION', final_text '$ROW_FINAL', zebrafinch only in final_text, no column equals the selection, no log line holds it" "$failures"

echo "== VAL-CMD-004 Command Mode does not start when AI is off"
failures=""
set_setting cleanup.llm.enabled false
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
rows=$(row_count)
posts=$(stub_posts)
watch_start
chord_down || failures+="the chord keys did not go down; "
sleep 0.3
during=$(visible_start)
[ -z "$during" ] || failures+="the chord showed a start while it was down: $during; "
paplay --device=vmic "$(pad "$CMD_WAV")"
wait_until 5 bar_failed
notice_state=$(bar .state)
notice=$(bar .message)
shot cmd-004-notice
chord_up
sleep 1
watch_stop
after=$(visible_start)
[ -z "$after" ] || failures+="the chord left a cue or a listening state: $after; "
[ "$notice_state" = blocked ] || failures+="the bar shows '$notice_state', not the blocked notice; "
case "$notice" in *"Command Mode needs AI cleanup or the endpoint"*) ;; *) failures+="the bar says '$notice'; " ;; esac
saw_mode command && failures+="a command session started; "
saw_pstate cleaning && failures+="the run reached cleaning; "
unchanged || failures+="the entry is '$(live)' with the selection '$(primary)'; "
[ "$(stub_posts)" = "$posts" ] || failures+="the stub saw a request; "
[ -z "$(pgrep -x hushpen-llm)" ] || failures+="a model worker runs; "
[ "$(row_count)" = "$rows" ] || failures+="the run added a history row; "
wait_until 5 pstate_is idle || failures+="the pipeline is '$(pstate)', not idle; "
clip_is "$SENTINEL" || failures+="the clipboard holds '$(clip)', not the sentinel; "
set_setting cleanup.llm.enabled true
check_failures "VAL-CMD-004 AI off" "the bar showed '$notice', no command session, no request, the entry and the selection unchanged" "$failures"

echo "== VAL-CMD-006 Command Mode works through the endpoint"
failures=""
stub_mode edited
rm -f "$STUB_LOG"
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
command_run "$CMD_WAV" || failures+="the command run did not end; "
shot cmd-006-after
[ "$(edited_middle)" = "EDITED TEXT" ] || failures+="the entry is '$(live)', not '$PREFIX_TEXT""EDITED TEXT""$SUFFIX_TEXT'; "
sent=$(stub_last_post | jq -r '[.body.messages[].content] | join("\n")')
case "$sent" in *"$SHORT_MID"*) ;; *) failures+="the request did not carry the selection; " ;; esac
case "$(norm <<<"$sent")" in *formal*) ;; *) failures+="the request did not carry the instruction; " ;; esac
[ "$(stub_last_post | jq -r .path)" = /v1/chat/completions ] || failures+="the request went to '$(stub_last_post | jq -r .path)'; "
stub_last_post >"$RUN_OUT/cmd-006-request.json"
wait_until 5 clip_is "$SENTINEL" || failures+="the clipboard holds '$(clip)', not the sentinel; "
check_failures "VAL-CMD-006 endpoint" "the stub got the instruction and the selection '$SHORT_MID' and the entry became '$(live)'" "$failures"
stub_mode default

echo "== VAL-CMD-005 cancel, failure, and timeout leave the selection unchanged (part one: Esc)"
failures=""
rows=$(row_count)
# (1) Esc while listening
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
posts=$(stub_posts)
chord_down || failures+="the chord keys did not go down; "
wait_until 3 pmode_is command || failures+="no command session; "
sleep 0.6
xdotool key Escape
wait_until 5 pstate_is idle || failures+="Esc while listening left the pipeline '$(pstate)'; "
shot cmd-005-esc-listening
chord_up
sleep 1
unchanged || failures+="after Esc while listening the entry is '$(live)' with the selection '$(primary)'; "
clip_is "$SENTINEL" || failures+="after Esc while listening the clipboard holds '$(clip)'; "
[ "$(stub_posts)" = "$posts" ] || failures+="Esc while listening still sent a request; "
# (2) Esc while cleaning (the stub answers after 30 s)
stub_mode slow
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
if command_to_cleaning "$CMD_WAV"; then
  xdotool key Escape
  wait_until 10 run_ended || failures+="Esc while cleaning did not end the run; "
  case ",$(pevents)," in *,cancelled,*) ;; *) failures+="the run did not end cancelled (events: $(pevents)); " ;; esac
  shot cmd-005-esc-cleaning
else
  failures+="the run was not caught in cleaning for Esc; "
  xdotool key Escape
fi
sleep 1
unchanged || failures+="after Esc while cleaning the entry is '$(live)' with the selection '$(primary)'; "
clip_is "$SENTINEL" || failures+="after Esc while cleaning the clipboard holds '$(clip)'; "
mentions_instruction "$(live)" && failures+="the instruction was pasted after Esc while cleaning; "
stub_mode default
# the app is ready for the next command
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
command_run "$CMD_WAV" || failures+="the next command did not end; "
new_middle=$(edited_middle) || failures+="after Esc the next command left '$(live)'; "
[ -n "$new_middle" ] && [ "$new_middle" != "$SHORT_MID" ] || failures+="the next command after Esc gave '$new_middle'; "
[ "$(last_row kind)" = command ] || failures+="the next command's row kind is '$(last_row kind)'; "
ESC_DETAIL="Esc while listening and while cleaning left the entry, the selection, and the sentinel as they were and sent nothing pasted; the next command gave '$new_middle'"
ESC_FAILURES=$failures

echo "== VAL-CMD-005 part two: the local Qwen3.5 2B model"
failures=""
stub_stop
set_setting cleanup.llm.provider '"local"'
set_setting cleanup.llm.modelId "\"$MODEL_ID\""
set_setting cleanup.llm.enabled false
/harness/slot/app.sh stop
mkdir -p /data/models/llm
cp "$FIXTURE_GGUF" "$MODEL_FILE"
# Only the start of the app hashes a copied model file and stamps it.
/harness/slot/app.sh start >/dev/null || failures+="the app did not restart; "
wait_until 600 test -e "$MODEL_FILE.verified" || failures+="the model file was never verified; "
wait_until 60 engine_ready || failures+="the speech engine was not ready after the restart; "
set_setting cleanup.llm.enabled true
wait_until 180 llm_is .loaded true || failures+="the Qwen3.5 2B worker never loaded the model; "
set_setting cleanup.llm.timeoutMs 20000

echo "== VAL-CMD-001 again with the local model"
local_failures=""
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || local_failures+="the entry did not take the text; "
command_run "$CMD_WAV" || local_failures+="the command run did not end; "
shot cmd-001-local
new_middle=$(edited_middle) || local_failures+="the entry '$(live)' lost the text around the selection; "
[ -n "$new_middle" ] && [ "$new_middle" != "$SHORT_MID" ] || local_failures+="the local model left the middle part as '$new_middle'; "
mentions_instruction "$(live)" && local_failures+="the spoken instruction is in the entry: $(live); "
wait_until 5 clip_is "$SENTINEL" || local_failures+="the clipboard holds '$(clip)'; "
[ "$(last_row kind)" = command ] || local_failures+="the row kind is '$(last_row kind)'; "
check_failures "VAL-CMD-001 local model" "'$SHORT_MID' became '$new_middle' with the local Qwen3.5 2B model" "$local_failures"
set_setting cleanup.llm.timeoutMs 8000

# (3) kill -9 while cleaning
wait_until 60 loaded_now || failures+="the worker is not loaded before the kill test; "
pid0=$(llm .pid)
set_entry "$PREFIX_TEXT" "$LONG_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
watch_start
if command_to_cleaning "$CMD_WAV"; then
  kill -9 "$pid0"
  wait_until 60 run_ended || failures+="the run did not end after kill -9; "
  wait_until 5 bar_is error
  kill_state=$(bar .state)
  kill_message=$(bar .message)
  shot cmd-005-kill9
else
  failures+="the run was not caught in cleaning for kill -9; "
  kill_state="" kill_message=""
fi
watch_stop
[ "$kill_state" = error ] || failures+="after kill -9 the bar shows '$kill_state', not an error; "
case "$kill_message" in *"Text unchanged"*) ;; *) failures+="after kill -9 the bar says '$kill_message'; " ;; esac
sleep 1
unchanged || failures+="after kill -9 the entry is '$(live)' with the selection '$(primary)'; "
clip_is "$SENTINEL" || failures+="after kill -9 the clipboard holds '$(clip)'; "
mentions_instruction "$(live)" && failures+="the instruction was pasted after kill -9; "
[ "$(last_row status)" = failed ] || failures+="after kill -9 the row status is '$(last_row status)'; "
wait_until 15 new_pid_after "$pid0" || failures+="no new worker pid within 15 s of kill -9; "
wait_until 90 loaded_now || failures+="the new worker never loaded the model; "
# the next command works
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
command_run "$CMD_WAV" || failures+="the command after kill -9 did not end; "
new_middle=$(edited_middle) || failures+="the command after kill -9 left '$(live)'; "
[ -n "$new_middle" ] && [ "$new_middle" != "$SHORT_MID" ] || failures+="the command after kill -9 gave '$new_middle'; "
KILL_DETAIL="kill -9: the bar showed '$kill_message', the entry and the selection were unchanged, a new worker came and the next command gave '$new_middle'"

# (4) kill -STOP while cleaning (timeout)
stopped=$(llm .pid)
set_entry "$PREFIX_TEXT" "$LONG_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
watch_start
if command_to_cleaning "$CMD_WAV"; then
  kill -STOP "$(llm .pid)"
  stopped=$(llm .pid)
  CLEAN_AT=$(now_ms)
  wait_until 60 run_ended || failures+="the run did not end with a stopped worker; "
  HELD_MS=$(($(now_ms) - CLEAN_AT))
  wait_until 5 bar_is error
  stop_state=$(bar .state)
  stop_message=$(bar .message)
  shot cmd-005-stop
else
  failures+="the run was not caught in cleaning for kill -STOP; "
  stop_state="" stop_message="" HELD_MS=""
fi
watch_stop
[ "$stop_state" = error ] || failures+="after kill -STOP the bar shows '$stop_state', not an error; "
case "$stop_message" in *"Text unchanged"*) ;; *) failures+="after kill -STOP the bar says '$stop_message'; " ;; esac
sleep 1
unchanged || failures+="after kill -STOP the entry is '$(live)' with the selection '$(primary)'; "
clip_is "$SENTINEL" || failures+="after kill -STOP the clipboard holds '$(clip)'; "
mentions_instruction "$(live)" && failures+="the instruction was pasted after kill -STOP; "
wait_until 30 proc_dead "$stopped" || failures+="the stopped worker $stopped was never killed; "
wait_until 30 new_pid_after "$stopped" || failures+="no new worker pid after the stop; "
wait_until 90 loaded_now || failures+="the worker did not load again after the stop; "
set_entry "$PREFIX_TEXT" "$SHORT_MID" "$SUFFIX_TEXT" || failures+="the entry did not take the text; "
command_run "$CMD_WAV" || failures+="the command after kill -STOP did not end; "
new_middle=$(edited_middle) || failures+="the command after kill -STOP left '$(live)'; "
[ -n "$new_middle" ] && [ "$new_middle" != "$SHORT_MID" ] || failures+="the command after kill -STOP gave '$new_middle'; "
STOP_DETAIL="kill -STOP: the bar showed '$stop_message' ${HELD_MS} ms after cleaning began, the entry and the selection were unchanged, the stopped worker was replaced and the next command gave '$new_middle'"
check_failures "VAL-CMD-005 cancel, failure, timeout" "$ESC_DETAIL; $KILL_DETAIL; $STOP_DETAIL" "$ESC_FAILURES$failures"

finish command
