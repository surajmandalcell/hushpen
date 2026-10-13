#!/bin/bash
# Undo cleanup: VAL-UNDO-001 to 007 and VAL-CROSS-002. Part one runs against the stub endpoint
# (harness/slot/openai-stub.py on 127.0.0.1:11434): its default answer is the transcript with
# " [stub]" added, so the AI text always differs from the rule text and no model is needed. Part
# two (VAL-CROSS-002) uses the local Qwen3.5 2B model from the fixture folder, a dictionary
# replacement, and the self-correction fixture. The words come from the virtual microphone and
# the hold key's path goes through the hook (pipeline-event), so a keyboard grab cannot stop it.
# The GTK target (target-gtk.py) steps back on Ctrl+Z like an editor's entry, keeps the live text
# in /out/gtk-live.txt and one line for every Ctrl+Z in /out/gtk-undo.log. The xterm target is
# stock xterm running `cat > /out/xterm.txt`; a Ctrl+Z that reached its terminal would stop cat.
# Needs filler-dictation.wav and ai-self-correction.wav in HUSHPEN_TEST_FIXTURES (mounted at
# /fixtures), the llm folder there, and the base model in /assets. Start the slot with
# VAL_CPUS=4 VAL_MEM=4g (the local model counts as 2 busy slots). It empties /data, so the slot
# must not be shared.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

GTK_FILE=/out/gtk.txt
LIVE=/out/gtk-live.txt
UNDO_LOG=/out/gtk-undo.log
XTERM_FILE=/out/xterm.txt
DB=/data/history/history.db
FILLER_WAV=/fixtures/filler-dictation.wav
SELF_WAV=/fixtures/ai-self-correction.wav
STUB_LOG=$RUN_OUT/stub-requests.jsonl
STUB_PID=/tmp/stub.pid
STUB_MODE=/tmp/stub-mode
STUB_URL=http://127.0.0.1:11434/v1
MODEL_ID=qwen3.5-2b
MODEL_FILE=/data/models/llm/$MODEL_ID.gguf
FIXTURE_GGUF=/fixtures/llm/Qwen3.5-2B-Q4_K_M.gguf
SENTINEL=SENTINEL-UNDO

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
fid() { xdotool getwindowfocus; }
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
sget() { $HC state | jq -r ".settings[\"$1\"]"; }
llm() { $HC state | jq -r ".llm$1"; }
last_row() { sqlite3 "$DB" "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
row_count() { sqlite3 "$DB" 'select count(*) from transcript'; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
live() { cat "$LIVE" 2>/dev/null; }
live_is() { [ "$(live)" = "$1" ]; }
clip_is() { [ "$(clip)" = "$1" ]; }
outcome_is() { [ "$(last_row llm_outcome)" = "$1" ]; }
llm_is() { [ "$(llm "$1")" = "$2" ]; }
engine_ready() { [ "$($HC state | jq -r .engine.state)" = ready ]; }
bar_not() { [ "$(bar .state)" != "$1" ]; }
undo_keys() { if [ -s "$UNDO_LOG" ]; then wc -l <"$UNDO_LOG" | tr -d ' '; else echo 0; fi; }

bar() { $HC state | jq -r ".overlay$1"; }
bar_is() { [ "$(bar .state)" = "$1" ]; }
offered() { [ "$($HC state | jq -r .pipeline.undo.offered)" = true ]; }
not_offered() { ! offered; }
el_center() { # <tree id> -> "x y": the middle of the element on the screen
  $HC tree | jq -r ".[] | select(.id == \"$1\") | .root_bounds | \"\\((.x + .width / 2) | floor) \\((.y + .height / 2) | floor)\""
}
click_el() { # a real pointer click on a tree element; the pointer rests there for 0.4 s first
  local x y
  read -r x y < <(el_center "$1")
  [ -n "$x" ] || return 1
  xdotool mousemove "$x" "$y"
  sleep 0.4
  xdotool click 1
  sleep 0.2
}
park() { xdotool mousemove 5 5; sleep 0.2; }
overlay_since() { # the bar's state changes since T0, as "a@ms b@ms"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "overlay" and .t_ms >= $t0) | "\(.detail)@\(.t_ms - $t0)"] | join(" ")'
}
undo_events() { # the undo events since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "undo" and .t_ms >= $t0) | .detail] | join(",")'
}
undo_shown_ms() { # how long the bar showed undo, since T0 (hook timestamps); empty when it never did
  $HC events | jq -r --argjson t0 "$T0" '
    [.[] | select(.kind == "overlay" and .t_ms >= $t0)] as $e
    | ($e | map(select(.detail == "undo")) | first | .t_ms) as $u
    | ($e | map(select(.detail != "undo" and .t_ms > $u)) | first | .t_ms) as $n
    | if $u == null or $n == null then empty else $n - $u end'
}
saw_undo_bar() { overlay_since | grep -q 'undo@'; }
crop_bar() { # <name>: the bar's crop of the last screenshot
  local w h x y
  read -r w h x y < <($HC state | jq -r '.overlay.bounds | "\(.width | floor) \(.height | floor) \(.x | floor) \(.y | floor)"')
  convert "$RUN_OUT/$1.png" -crop "${w}x${h}+${x}+${y}" +repage "$RUN_OUT/$1-bar.png"
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
  wait_until 3 live_is ""
}
read_row() {
  ROW_ID=$(last_row id)
  ROW_RAW=$(last_row raw_text)
  ROW_RULE=$(last_row rule_text)
  ROW_LLM=$(last_row llm_text)
  ROW_FINAL=$(last_row final_text)
  ROW_OUTCOME=$(last_row llm_outcome)
}
# hold_run <wav>: hold-down, the words, hold-up; returns as soon as the key is up
hold_run() {
  T0=$(now_ms)
  hook_action pipeline-event '{"event":"hold-down"}' >/dev/null
  wait_until 5 pstate_is listening || {
    hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
    return 1
  }
  paplay --device=vmic "$(pad "$1")"
  hook_action pipeline-event '{"event":"hold-up"}' >/dev/null
}
# ai_run <wav>: one dictation into the GTK entry (clean, focused, sentinel on the clipboard)
# that returns the moment the bar offers Undo cleanup. Sets ROW_*, AI_AT (ms of the offer).
ai_run() {
  gtk_clear
  set_clip "$SENTINEL"
  focus "$GT"
  hold_run "$1" || return 1
  wait_until "${OFFER_WAIT:-90}" offered || return 1
  AI_AT=$(now_ms)
  read_row
}
# quiet_run <wav>: a dictation after which no offer may come; waits until the run ended
quiet_run() {
  gtk_clear
  set_clip "$SENTINEL"
  focus "$GT"
  hold_run "$1" || return 1
  wait_until 90 run_ended || return 1
  read_row
}
wait_expired() { wait_until 10 not_offered; sleep 0.5; }
undo_rejected() { # the hook undo action must refuse; prints its message
  local out
  if out=$(hook_action overlay-undo-cleanup 2>&1); then
    echo "accepted"
    return 1
  fi
  echo "$out"
}

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
trap stub_stop EXIT

open_history_detail() { # opens the detail of the newest row
  focus "$(app_window)"
  $HC action open-view '{"view":"history"}' >/dev/null
  wait_until 10 view_is history || return 1
  $HC click history.back >/dev/null 2>&1
  wait_until 10 tree_has history.row.0 || return 1
  $HC click history.row.0 >/dev/null
  wait_until 10 tree_has history.detail
}
history_count() { $HC state | jq -r '.history.count'; }
history_search() { hook_action history-search "{\"query\":\"$1\"}" >/dev/null; sleep 0.6; }

echo "== setup: base model, the paste targets, and the stub endpoint"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
XT=$(grep -o 'xterm=[0-9]*' <<<"$ids" | cut -d= -f2)
for wav in "$FILLER_WAV" "$SELF_WAV" "$FIXTURE_GGUF"; do
  [ -e "$wav" ] || echo "missing fixture $wav"
done
rm -f "$STUB_LOG"
stub_start || echo "the stub did not start"
set_setting cleanup.llm.provider '"endpoint"'
set_setting cleanup.llm.endpointUrl "\"$STUB_URL\""
set_setting cleanup.llm.endpointModel '"stub-model"'
set_setting cleanup.llm.enabled true

echo "== VAL-UNDO-001 and 002 Undo cleanup in the flow bar replaces the AI text and updates History"
failures=""
ai_run "$FILLER_WAV" || failures+="no offer after the dictation; "
AI_TEXT=$(live)
RULE_TEXT=$ROW_RULE
shot undo-001-offered
crop_bar undo-001-offered
[ -n "$RULE_TEXT" ] && [ "$AI_TEXT" = "$ROW_LLM" ] && [ "$AI_TEXT" != "$RULE_TEXT" ] || failures+="the entry '$AI_TEXT' is not the AI text '$ROW_LLM' that differs from the rule text '$RULE_TEXT'; "
[ "$ROW_OUTCOME" = used ] || failures+="llm_outcome is '$ROW_OUTCOME' before the undo; "
bar_is undo || failures+="the bar shows '$(bar .state)', not undo; "
tree_has flowbar.undo || failures+="no flowbar.undo in the tree; "
keys_before=$(undo_keys)
click_el flowbar.undo || failures+="no Undo cleanup button to click; "
park
wait_until 10 live_is "$RULE_TEXT" || failures+="the entry is '$(live)' after the undo, not the rule text '$RULE_TEXT'; "
shot undo-001-after
focus_after=$(fid)
[ "$focus_after" = "$GT" ] || failures+="the focus moved to $focus_after from the entry $GT; "
case "$(live)" in *"[stub]"*) failures+="the AI text is still in the entry: $(live); " ;; esac
wait_until 5 clip_is "$SENTINEL" || failures+="the clipboard holds '$(clip)', not the sentinel; "
[ "$(undo_keys)" -gt "$keys_before" ] || failures+="the entry never received Ctrl+Z; "
wait_until 5 bar_not undo || failures+="the bar still shows undo after the click; "
wait_until 10 outcome_is undone
read_row
[ "$ROW_FINAL" = "$ROW_RULE" ] && [ -n "$ROW_FINAL" ] || failures+="final_text '$ROW_FINAL' is not rule_text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = undone ] || failures+="llm_outcome is '$ROW_OUTCOME', not undone; "
[ "$ROW_LLM" = "$AI_TEXT" ] || failures+="llm_text changed to '$ROW_LLM'; "
[ "$(row_count)" = 1 ] || failures+="the undo made $(row_count) rows; "
# Paste last transcript now gives the rule text.
gtk_clear
focus "$GT"
xdotool key ctrl+alt+v
wait_until 10 live_is "$RULE_TEXT" || failures+="Paste last transcript put '$(live)' in the entry, not the rule text '$RULE_TEXT'; "
shot undo-002-paste-last
check_failures "VAL-UNDO-001 and 002 undo in the bar" "AI text '$AI_TEXT' became rule text '$RULE_TEXT' in the entry, focus $focus_after, sentinel restored, row final=rule outcome undone, Paste last gives the rule text" "$failures"

echo "== VAL-UNDO-003 the offer lasts 5 s and then the undo is rejected"
failures=""
ai_run "$FILLER_WAV" || failures+="no offer after the dictation; "
AI_TEXT=$(live)
keys_before=$(undo_keys)
sleep 6
shot undo-003-after-6s
shown=$(undo_shown_ms)
[ -n "$shown" ] && [ "$shown" -ge 4500 ] && [ "$shown" -le 5500 ] || failures+="the bar showed Undo cleanup for '${shown}' ms (events: $(overlay_since)); "
bar_is undo && failures+="the bar still shows undo after 6 s; "
tree_has flowbar.undo && failures+="flowbar.undo is still in the tree; "
rejected=$(undo_rejected) || failures+="the hook undo action was accepted after 6 s; "
sleep 0.5
live_is "$AI_TEXT" || failures+="the entry changed to '$(live)'; "
[ "$(undo_keys)" = "$keys_before" ] || failures+="the entry got Ctrl+Z after the offer ended; "
[ "$(last_row llm_outcome)" = used ] || failures+="llm_outcome is '$(last_row llm_outcome)', not used; "
check_failures "VAL-UNDO-003 expiry" "Undo cleanup showed ${shown} ms, then the hook undo was rejected ('$rejected'), entry and row unchanged" "$failures"

echo "== VAL-UNDO-004 a change of focus withdraws the offer"
failures=""
ai_run "$FILLER_WAV" || failures+="no offer after the dictation; "
AI_TEXT=$(live)
keys_before=$(undo_keys)
focus "$XT"
wait_until 2 not_offered || failures+="the offer was still on 2 s after the xterm took the focus; "
sleep 0.3
shot undo-004-xterm-focused
bar_is undo && failures+="the bar still shows undo; "
tree_has flowbar.undo && failures+="flowbar.undo is still in the tree; "
rejected=$(undo_rejected) || failures+="the hook undo action was accepted after the focus changed; "
focus "$GT"
sleep 0.3
offered && failures+="the offer came back when the entry got the focus again; "
live_is "$AI_TEXT" || failures+="the entry changed to '$(live)'; "
[ "$(undo_keys)" = "$keys_before" ] || failures+="the entry got Ctrl+Z; "
[ ! -s "$XTERM_FILE" ] || failures+="the xterm output holds '$(od -An -c "$XTERM_FILE" | head -2)'; "
check_failures "VAL-UNDO-004 focus changed" "xterm focused: no undo on the bar, hook undo rejected ('$rejected'), entry '$AI_TEXT' and xterm output unchanged" "$failures"

echo "== VAL-UNDO-005 no offer without a confirmed AI insertion"
failures=""
# (1) a keyboard grab blocks the paste: no receipt
python3 /harness/slot/grab-keyboard.py >"$RUN_OUT/grab-helper.log" 2>&1 &
GRAB_PID=$!
wait_until 10 grep -q 'grab status 0' "$RUN_OUT/grab-helper.log" || failures+="the grab helper did not get the grab; "
quiet_run "$FILLER_WAV" || failures+="the run under the grab did not end; "
sleep 1
[ "$($HC state | jq -r .last_insert.outcome)" = blocked_grab ] || failures+="the receipt is '$($HC state | jq -c .last_insert)', not blocked_grab; "
offered && failures+="Undo cleanup was offered after a blocked insertion; "
saw_undo_bar && failures+="the bar showed undo after a blocked insertion ($(overlay_since)); "
shot undo-005-grab
kill "$GRAB_PID" 2>/dev/null
wait "$GRAB_PID" 2>/dev/null
sleep 0.5
# (2) the endpoint is too slow: the rule text goes in
stub_mode slow
set_setting cleanup.llm.endpointTimeoutMs 1000
quiet_run "$FILLER_WAV" || failures+="the run with the slow endpoint did not end; "
[ "$ROW_OUTCOME" = timeout ] || failures+="the slow endpoint gave llm_outcome '$ROW_OUTCOME', not timeout; "
sleep 1
live_is "$ROW_RULE" || failures+="the entry '$(live)' is not the rule text '$ROW_RULE'; "
offered && failures+="Undo cleanup was offered after a timeout; "
saw_undo_bar && failures+="the bar showed undo after a timeout ($(overlay_since)); "
shot undo-005-timeout
stub_mode default
set_setting cleanup.llm.endpointTimeoutMs 6000
# (3) AI cleanup is off
set_setting cleanup.llm.enabled false
quiet_run "$FILLER_WAV" || failures+="the run with AI cleanup off did not end; "
sleep 1
[ "$ROW_OUTCOME" = off ] || failures+="llm_outcome is '$ROW_OUTCOME' with AI cleanup off; "
offered && failures+="Undo cleanup was offered with AI cleanup off; "
saw_undo_bar && failures+="the bar showed undo with AI cleanup off ($(overlay_since)); "
shot undo-005-off
set_setting cleanup.llm.enabled true
check_failures "VAL-UNDO-005 no confirmed AI insertion" "blocked grab, timeout, and AI cleanup off each left the bar without Undo cleanup" "$failures"

echo "== VAL-UNDO-006 Undo cleanup in the History detail"
failures=""
ai_run "$FILLER_WAV" || failures+="no offer after the dictation; "
AI_TEXT=$(live)
RULE_TEXT=$ROW_RULE
keys_before=$(undo_keys)
wait_expired
open_history_detail || failures+="the detail of the newest row did not open; "
tree_has history.undo-cleanup || failures+="no Undo cleanup button in the detail; "
$HC click history.back >/dev/null 2>&1
wait_until 10 tree_has history.row.0
history_search stub
before_hits=$(history_count)
history_search ""
open_history_detail || failures+="the detail did not open again; "
shot undo-006-before
$HC click history.undo-cleanup >/dev/null || failures+="the Undo cleanup button could not be clicked; "
wait_until 10 outcome_is undone || failures+="llm_outcome is '$(last_row llm_outcome)', not undone; "
sleep 0.5
shot undo-006-after
detail_final=$($HC state | jq -r .history.detail.final_text)
[ "$detail_final" = "$RULE_TEXT" ] || failures+="the detail shows final '$detail_final', not the rule text '$RULE_TEXT'; "
[ "$(last_row final_text)" = "$RULE_TEXT" ] || failures+="final_text is '$(last_row final_text)'; "
tree_has history.undo-cleanup && failures+="the button is still there after the undo; "
$HC click history.back >/dev/null 2>&1
wait_until 10 tree_has history.row.0
word=$(awk '{print $1}' <<<"$RULE_TEXT" | tr -dc 'A-Za-z')
history_search stub
after_hits=$(history_count)
history_search "$word"
rule_hits=$(history_count)
history_search ""
[ $((before_hits - after_hits)) = 1 ] || failures+="a search for the AI-only word 'stub' found $before_hits row(s) before and $after_hits after the undo, not one fewer; "
[ "${rule_hits:-0}" -ge 1 ] || failures+="a search for the rule word '$word' found $rule_hits row(s) after the undo; "
[ "$(undo_keys)" = "$keys_before" ] || failures+="the entry got Ctrl+Z; "
live_is "$AI_TEXT" || failures+="the entry changed to '$(live)'; "
[ ! -s "$XTERM_FILE" ] || failures+="the xterm got text; "
check_failures "VAL-UNDO-006 undo in History" "detail final '$RULE_TEXT', outcome undone, search 'stub' $before_hits -> $after_hits, search '$word' $rule_hits, the entry '$AI_TEXT' untouched with no key sent" "$failures"

echo "== VAL-UNDO-007 a terminal target hides Undo cleanup and never gets Ctrl+Z"
failures=""
# Fresh targets: cat holds /out/xterm.txt open, so deleting the file would not empty it.
ids=$(/harness/slot/targets.sh)
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
XT=$(grep -o 'xterm=[0-9]*' <<<"$ids" | cut -d= -f2)
set_clip "$SENTINEL"
focus "$XT"
hold_run "$FILLER_WAV" || failures+="the run did not start; "
wait_until 90 run_ended || failures+="the run did not end; "
read_row
cat_pid=$(pgrep -x cat | head -1)
# Watch the whole offer window of the other targets.
shown_undo=no
for _ in $(seq 60); do
  bar_is undo && shown_undo=yes
  offered && shown_undo=yes
  tree_has flowbar.undo && shown_undo=yes
  sleep 0.1
done
shot undo-007-terminal
crop_bar undo-007-terminal
[ "$shown_undo" = no ] || failures+="the bar offered Undo cleanup in the terminal; "
saw_undo_bar && failures+="overlay events show undo ($(overlay_since)); "
[ "$ROW_OUTCOME" = used ] && [ "$ROW_LLM" != "$ROW_RULE" ] || failures+="the run was not AI-cleaned with a different text (outcome '$ROW_OUTCOME'); "
rejected=$(undo_rejected) || failures+="the hook undo action was accepted in a terminal; "
events=$(undo_events)
case ",$events," in *,key\ sent*) failures+="an undo key was sent ($events); " ;; esac
case "$events" in *"declined terminal"*) ;; *) failures+="no 'declined terminal' event ($events); " ;; esac
focus "$XT"
xdotool key Return
wait_until 5 test -s "$XTERM_FILE" || failures+="/out/xterm.txt stayed empty; "
sleep 0.5
if [ "$(tr -dc '\032' <"$XTERM_FILE" | wc -c | tr -d ' ')" != 0 ]; then failures+="/out/xterm.txt holds a 0x1a byte; "; fi
[ "$(tr -d '\n' <"$XTERM_FILE")" = "$ROW_LLM" ] || failures+="the xterm holds '$(cat "$XTERM_FILE")', not the AI text '$ROW_LLM'; "
cat_state=$(ps -o stat= -p "${cat_pid:-0}" 2>/dev/null | tr -d ' ')
case "$cat_state" in "" ) failures+="the cat process is gone; " ;; *T*) failures+="the cat process is stopped (state $cat_state); " ;; esac
[ "$(undo_keys)" = 0 ] || failures+="the GTK entry got a Ctrl+Z; "
open_history_detail || failures+="the detail of the terminal row did not open; "
sleep 0.4
shot undo-007-history
tree_has history.copy-original || failures+="the History detail has no Copy original; "
set_clip OLD
$HC click history.copy-original >/dev/null 2>&1
wait_until 5 clip_is "$ROW_RULE" || failures+="Copy original put '$(clip)' on the clipboard, not the rule text '$ROW_RULE'; "
check_failures "VAL-UNDO-007 terminal" "no undo on the bar for 6 s, hook undo rejected ('$rejected'), events '$events', xterm.txt has no 0x1a, cat state '$cat_state', History offers Copy original" "$failures"

echo "== VAL-CROSS-002 dictionary, rule cleanup, AI cleanup, and undo leave every version in History"
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
# An ordinary word outside the self-correction: the model undoes an odd word it takes for the
# corrected one (it turned Wednesbury back into Wednesday).
REPLACEMENT=huddle
hook_action dictionary-add "{\"phrase\":\"$REPLACEMENT\",\"heard_as\":\"meet\"}" >/dev/null || failures+="the dictionary entry was not saved; "
set_setting cleanup.rules true
ai_run "$SELF_WAV" || failures+="no offer after the dictation; "
AI_TEXT=$(live)
RULE_TEXT=$ROW_RULE
shot cross-002-before
crop_bar cross-002-before
cp "$LIVE" "$RUN_OUT/gtk-before-undo.txt"
case "$AI_TEXT" in *"$REPLACEMENT"*) ;; *) failures+="the entry '$AI_TEXT' has no dictionary replacement $REPLACEMENT; " ;; esac
[ "$AI_TEXT" = "$ROW_LLM" ] && [ "$AI_TEXT" != "$RULE_TEXT" ] || failures+="the entry '$AI_TEXT' is not the AI text '$ROW_LLM' that differs from the rule text '$RULE_TEXT'; "
case " $(norm <<<"$RULE_TEXT") " in *" um "* | *" uh "*) failures+="the rule text still has a filler: $RULE_TEXT; " ;; esac
case "$RULE_TEXT" in *"$REPLACEMENT"*) ;; *) failures+="the rule text '$RULE_TEXT' has no replacement; " ;; esac
click_el flowbar.undo || failures+="no Undo cleanup button to click; "
park
wait_until 10 live_is "$RULE_TEXT" || failures+="the entry is '$(live)' after the undo, not the rule text '$RULE_TEXT'; "
cp "$LIVE" "$RUN_OUT/gtk-after-undo.txt"
wait_until 10 outcome_is undone
read_row
[ "$ROW_OUTCOME" = undone ] && [ "$ROW_FINAL" = "$ROW_RULE" ] || failures+="the row has outcome '$ROW_OUTCOME' and final '$ROW_FINAL'; "
open_history_detail || failures+="the detail did not open; "
sleep 0.5
shot cross-002-detail
detail() { $HC state | jq -r ".history.detail.$1"; }
for field in history.detail.raw history.detail.rule history.detail.llm history.detail.final; do
  tree_has "$field" || failures+="no $field in the detail; "
done
[ "$(detail raw_text)" = "$ROW_RAW" ] || failures+="the detail shows raw '$(detail raw_text)'; "
[ "$(detail rule_text)" = "$RULE_TEXT" ] || failures+="the detail shows rule '$(detail rule_text)'; "
[ "$(detail llm_text)" = "$AI_TEXT" ] || failures+="the detail shows AI text '$(detail llm_text)'; "
[ "$(detail final_text)" = "$RULE_TEXT" ] || failures+="the detail shows final '$(detail final_text)'; "
sqlite3 -line "$DB" "select raw_text, rule_text, llm_text, final_text, llm_outcome from transcript order by created_at desc limit 1" >"$RUN_OUT/cross-002-row.txt"
check_failures "VAL-CROSS-002 every version in History" "raw '$ROW_RAW', rule '$RULE_TEXT', AI '$AI_TEXT', final = rule, outcome undone; the entry went from the AI text to the rule text" "$failures"

finish undo
