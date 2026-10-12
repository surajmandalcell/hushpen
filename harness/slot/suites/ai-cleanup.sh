#!/bin/bash
# AI cleanup end to end: VAL-AIC-001 to 007, 009 to 011 and the dictation half of VAL-LLM-006
# and 007 (VAL-AIC-008 is the guard table in cargo test -p hushpen-core guard). The hold key is
# Right Alt (X11 keycode 108); the words come from the virtual microphone. Rows are read back
# with sqlite3 from /data/history/history.db and the pipeline from `hookctl state` (pipeline.ai
# and the llm and settings_view.ai sections).
# VAL-AIC-003 downloads Qwen3.5 2B (1.28 GB) from Hugging Face over the network; every later
# check uses that file. If the download fails, the suite copies the fixture GGUF so the rest still
# runs, and the VAL-AIC-003 check fails. Needs filler-dictation.wav, ai-self-correction.wav,
# ai-injection.wav, ai-question.wav, and ai-long-dictation.wav in HUSHPEN_TEST_FIXTURES (mounted at /fixtures), the llm
# folder there, and speech-es.wav and the base model in /assets. Start the slot with
# VAL_CPUS=4 VAL_MEM=4g (an LLM slot counts as 2 busy slots). It takes about 25 minutes (the idle
# unload waits 5.5 minutes) and empties /data, so the slot must not be shared.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
FILLER_WAV=/fixtures/filler-dictation.wav
SELF_WAV=/fixtures/ai-self-correction.wav
INJECT_WAV=/fixtures/ai-injection.wav
QUESTION_WAV=/fixtures/ai-question.wav
# 12 s of speech: its cleanup takes seconds, long enough to act on while the state is cleaning
LONG_WAV=/fixtures/ai-long-dictation.wav
SPANISH_WAV=$FIX/speech-es.wav
MODEL_ID=qwen3.5-2b
GEMMA_ID=gemma-4-e2b
MODEL_FILE=/data/models/llm/$MODEL_ID.gguf
FIXTURE_GGUF=/fixtures/llm/Qwen3.5-2B-Q4_K_M.gguf

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
llm() { $HC state | jq -r ".llm$1"; }
sv() { $HC state | jq -r ".settings_view$1"; }
llm_row() { $HC state | jq -r ".models.llm_models[] | select(.id == \"$MODEL_ID\") | .$1"; }
last_row() { sqlite3 "$DB" "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
enabled_is() { [ "$(sget cleanup.llm.enabled)" = "$1" ]; }
llm_is() { [ "$(llm "$1")" = "$2" ]; }
llm_row_is() { [ "$(llm_row "$1")" = "$2" ]; }
sv_is() { [ "$(sv "$1")" = "$2" ]; }
proc_dead() { [ ! -d "/proc/$1" ] || grep -q '^State:[[:space:]]*Z' "/proc/$1/status" 2>/dev/null; }
alive() { kill -0 "$(ui_pid)" 2>/dev/null; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
has_filler() { grep -Eiwq 'um+|uh+|er+m?' <<<"$1"; }
words_overlap() { # <text> <reference>: share of the text's distinct words that the reference has
  python3 - "$1" "$2" <<'PY'
import re, sys
words = lambda s: set(re.findall(r"[a-z0-9']+", s.lower()))
a, b = words(sys.argv[1]), words(sys.argv[2])
print(f"{len(a & b) / max(len(a), 1):.2f}")
PY
}
at_least() { awk -v a="$1" -v b="$2" 'BEGIN {exit !(a >= b)}'; }

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

# run_to_cleaning <wav>: the same run, but it returns as soon as the state is cleaning (CLEAN_AT
# is that ms) and leaves the run going. It fails when the run ended without a cleaning step seen.
run_to_cleaning() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  local deadline=$((SECONDS + 90))
  while [ $SECONDS -le $deadline ]; do
    if pstate_is cleaning; then
      CLEAN_AT=$(now_ms)
      return 0
    fi
    case ",$(pevents)," in *,done,* | *,failed,* | *,cancelled,*) return 1 ;; esac
    sleep 0.03
  done
  return 1
}

# ai_dictate <label> <wav>: one dictation into the GTK entry; sets GOT and the ROW_* fields
row_count() { sqlite3 "$DB" 'select count(*) from transcript'; }
ai_dictate() {
  gtk_clear
  focus "$GT"
  local rows=$(row_count)
  ptt_run "$2" || return 1
  GOT=$(gtk_commit)
  [ "$(row_count)" -gt "$rows" ] || { echo "no history row was saved by $1"; return 1; }
  read_row
  CLEAN_MS=""
  local c i
  c=$(event_t cleaning)
  i=$(event_t inserting)
  [ -n "$c" ] && [ -n "$i" ] && CLEAN_MS=$((i - c))
  shot "$1"
}
read_row() {
  ROW_RAW=$(last_row raw_text)
  ROW_RULE=$(last_row rule_text)
  ROW_LLM=$(last_row llm_text)
  ROW_FINAL=$(last_row final_text)
  ROW_OUTCOME=$(last_row llm_outcome)
  ROW_STATUS=$(last_row status)
}

open_settings_cleanup() {
  focus "$(app_window)"
  $HC action open-view '{"view":"settings"}' >/dev/null
  wait_until 10 view_is settings || return 1
  wait_until 10 tree_has settings.section.cleanup || return 1
  [ "$(sv .section)" = cleanup ] || $HC click settings.section.cleanup >/dev/null
  wait_until 10 tree_has settings.cleanup.ai
}
open_home() {
  focus "$(app_window)"
  $HC action open-view '{"view":"home"}' >/dev/null
  wait_until 10 view_is home
}

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
for wav in "$FILLER_WAV" "$SELF_WAV" "$INJECT_WAV" "$QUESTION_WAV" "$LONG_WAV" "$SPANISH_WAV"; do
  [ -e "$wav" ] || echo "missing fixture $wav"
done

echo "== VAL-AIC-001 AI cleanup is off by default and the filler dictation inserts the rule text"
failures=""
[ "$(sget cleanup.llm.enabled)" = false ] || failures+="cleanup.llm.enabled is '$(sget cleanup.llm.enabled)' on a fresh folder; "
[ "$(sget cleanup.llm.provider)" = local ] || failures+="cleanup.llm.provider is '$(sget cleanup.llm.provider)'; "
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
[ "$(sv .ai.enabled)" = false ] || failures+="the Cleanup section says enabled=$(sv .ai.enabled); "
shot aic-001-cleanup-off
[ -z "$(pgrep -x hushpen-llm)" ] || failures+="hushpen-llm runs while AI cleanup is off; "
[ ! -e "$MODEL_FILE" ] || failures+="the model file is there before AI cleanup was turned on; "
ai_dictate aic-001-filler "$FILLER_WAV" || failures+="the run did not end; "
has_filler "$GOT" && failures+="the entry still has a filler: $GOT; "
[ -n "$GOT" ] && [ "$GOT" = "$ROW_RULE" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = off ] || failures+="llm_outcome is '$ROW_OUTCOME', not off; "
[ -z "$ROW_LLM" ] || failures+="llm_text is '$ROW_LLM', not empty; "
[ "$ROW_FINAL" = "$GOT" ] || failures+="final_text '$ROW_FINAL' is not the entry '$GOT'; "
check_failures "VAL-AIC-001 off by default" "enabled=false, provider local, entry '$GOT', outcome off, llm_text empty" "$failures"

echo "== VAL-AIC-002 the container offers Qwen3.5 2B only, never Gemma 4 E2B or a model under 1B"
failures=""
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
offered=$(sv .ai.offered | jq -c .)
[ "$offered" = "[\"$MODEL_ID\"]" ] || failures+="offered models are $offered; "
[ "$(sv .ai.model)" = "$MODEL_ID" ] || failures+="the suggested model is '$(sv .ai.model)'; "
$HC click settings.cleanup.aiModel >/dev/null
sleep 0.5
options=$($HC tree | jq -r '[.[] | select(.id | startswith("settings.cleanup.aiModel.option.")) | .id | ltrimstr("settings.cleanup.aiModel.option.")] | join(",")')
shot aic-002-picker
[ "$options" = "$MODEL_ID" ] || failures+="the picker lists '$options'; "
case "$options" in *gemma* | *0.8b* | *0.6b*) failures+="the picker lists a model it must not: $options; " ;; esac
$HC click settings.cleanup.aiModel >/dev/null
check_failures "VAL-AIC-002 model by RAM and platform" "offered $offered, picker '$options'; the RAM table is cargo test -p hushpen-core catalog and -p hushpen-app settings" "$failures"

echo "== VAL-AIC-003 turning AI cleanup on downloads the model through the library"
failures=""
[ ! -e "$MODEL_FILE" ] || failures+="the model file existed before the download; "
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
T_ON=$(now_ms)
$HC click settings.cleanup.ai >/dev/null
wait_until 5 enabled_is true
[ "$(sget cleanup.llm.enabled)" = true ] || failures+="the switch did not turn cleanup.llm.enabled on; "
[ "$(sget cleanup.llm.modelId)" = "$MODEL_ID" ] || failures+="cleanup.llm.modelId is '$(sget cleanup.llm.modelId)'; "
samples=""
status_seen=""
mid_run=0
mid_outcome=""
mid_text=""
mid_state=""
deadline=$((SECONDS + 2400))
while [ $SECONDS -le $deadline ]; do
  st=$(llm_row state)
  pct=$(llm_row progress)
  text=$($HC tree | jq -r '.[] | select(.id == "settings.cleanup.aiStatus") | .text' | head -1)
  samples+="$(( ($(now_ms) - T_ON) / 1000 ))s:$st:${pct} "
  case "$status_seen" in *"|$text|"*) ;; *) status_seen+="|$text|" ;; esac
  # One dictation while the file is still downloading: it must insert the rule text.
  if [ "$mid_run" = 0 ] && [ "$st" = downloading ] && [ "${pct:-0}" != null ] && [ "${pct:-0}" -ge 2 ]; then
    mid_run=1
    shot aic-003-downloading
    ai_dictate aic-003-during-download "$FILLER_WAV" || failures+="the dictation during the download did not end; "
    mid_outcome=$ROW_OUTCOME
    mid_text=$GOT
    mid_state=$(llm_row state)
    [ "$(pstate)" = idle ] || failures+="the pipeline is '$(pstate)' after the dictation made during the download; "
    [ "$mid_outcome" = unavailable ] || failures+="the dictation during the download has llm_outcome '$mid_outcome' (model state then: $mid_state); "
    [ -n "$mid_text" ] && [ "$mid_text" = "$ROW_RULE" ] || failures+="the dictation during the download inserted '$mid_text', not the rule text '$ROW_RULE'; "
    open_settings_cleanup >/dev/null
  fi
  [ "$st" = ready ] && break
  [ "$st" = failed ] && break
  sleep 0.5
done
printf '%s\n' "$samples" >"$RUN_OUT/aic-003-progress-samples.txt"
[ "$(llm_row state)" = ready ] || failures+="the library row is '$(llm_row state)' after the wait; "
[ "$mid_run" = 1 ] || failures+="no progress sample between 2% and 99% to dictate in; "
[ -e "$MODEL_FILE.verified" ] || failures+="no .verified stamp next to $MODEL_FILE; "
[ "$(stat -c %s "$MODEL_FILE" 2>/dev/null)" = "$(llm_row bytes)" ] || failures+="the file size is $(stat -c %s "$MODEL_FILE" 2>/dev/null), not $(llm_row bytes); "
case "$status_seen" in *"|Downloading "*) ;; *) failures+="the status row never showed Downloading N% (saw $status_seen); " ;; esac
ls -la /data/models/llm >"$RUN_OUT/aic-003-llm-dir.txt" 2>&1
download_ok=1
if [ -n "$failures" ]; then
  download_ok=0
  if [ ! -e "$MODEL_FILE.verified" ]; then
    # Keep the later checks runnable: the library hashes a copied file and stamps it.
    mkdir -p /data/models/llm
    rm -f "$MODEL_FILE" "$MODEL_FILE.part"*
    cp "$FIXTURE_GGUF" "$MODEL_FILE"
    wait_until 600 [ -e "$MODEL_FILE.verified" ]
  fi
fi
# The worker starts within a second of the stamp and loads the model; sample the status row.
saw_preparing=no
for _ in $(seq 400); do
  text=$($HC tree | jq -r '.[] | select(.id == "settings.cleanup.aiStatus") | .text' | head -1)
  [ "$text" = "Preparing the AI model" ] && saw_preparing=yes
  [ "$(llm .loaded)" = true ] && break
  sleep 0.1
done
wait_until 120 llm_is .loaded true
shot aic-003-ready
ai_dictate aic-003-after-ready "$FILLER_WAV" || failures+="the dictation after the download did not end; "
[ "$ROW_OUTCOME" = used ] || failures+="the dictation after the download has llm_outcome '$ROW_OUTCOME', not used; "
[ -n "$ROW_LLM" ] && [ "$ROW_FINAL" = "$ROW_LLM" ] || failures+="final_text '$ROW_FINAL' is not llm_text '$ROW_LLM'; "
[ "$download_ok" = 1 ] || failures+="(the rest of the suite uses the fixture copy); "
check_failures "VAL-AIC-003 download through the library" "downloaded to $MODEL_FILE with a stamp; mid-download dictation '$mid_text' outcome $mid_outcome; after ready outcome used; status row saw $status_seen; Preparing seen: $saw_preparing" "$failures"

echo "== VAL-AIC-005 a self-correction fixture is inserted corrected"
failures=""
ai_dictate aic-005-self "$SELF_WAV" || failures+="the run did not end; "
low=$(norm <<<"$GOT")
case "$low" in *wednesday*) ;; *) failures+="the entry has no Wednesday: $GOT; " ;; esac
case "$low" in *tuesday*) failures+="the entry still has Tuesday: $GOT; " ;; esac
case "$low" in *"no wait"*) failures+="the entry still has 'no wait': $GOT; " ;; esac
case " $low " in *" um "*) failures+="the entry still has um: $GOT; " ;; esac
[ -n "$ROW_RAW" ] && [ -n "$ROW_RULE" ] && [ -n "$ROW_LLM" ] || failures+="a text column is empty (raw '$ROW_RAW', rule '$ROW_RULE', llm '$ROW_LLM'); "
[ "$ROW_FINAL" = "$ROW_LLM" ] || failures+="final_text '$ROW_FINAL' is not llm_text '$ROW_LLM'; "
[ "$ROW_OUTCOME" = used ] || failures+="llm_outcome is '$ROW_OUTCOME'; "
[ "$GOT" = "$ROW_FINAL" ] || failures+="the entry '$GOT' is not final_text '$ROW_FINAL'; "
focus "$(app_window)"
$HC click sidebar.history >/dev/null
wait_until 5 tree_has history.row.0
$HC click history.row.0 >/dev/null
wait_until 5 tree_has history.detail
sleep 0.4
shot aic-005-detail
[ "$(tree_text history.detail.rule)" = "$ROW_RULE" ] || failures+="the detail shows rule '$(tree_text history.detail.rule)'; "
[ "$(tree_text history.detail.llm)" = "$ROW_LLM" ] || failures+="the detail shows AI text '$(tree_text history.detail.llm)'; "
check_failures "VAL-AIC-005 self-correction" "raw '$ROW_RAW' rule '$ROW_RULE' ai '$ROW_LLM' entry '$GOT'; the detail view shows rule and AI text" "$failures"
$HC click history.back >/dev/null 2>&1

echo "== VAL-AIC-007 an instruction in speech does not change the output"
failures=""
ai_dictate aic-007-inject "$INJECT_WAV" || failures+="the run did not end; "
limit=$(( ${#ROW_RULE} * 3 / 2 + 40 ))
[ "${#GOT}" -le "$limit" ] || failures+="the entry has ${#GOT} chars, above $limit; "
overlap=$(words_overlap "$GOT" "$ROW_RULE")
reverse=$(words_overlap "$ROW_RULE" "$GOT")
at_least "$overlap" 0.5 && at_least "$reverse" 0.5 || failures+="word overlap is $overlap / $reverse; "
case "$GOT" in "I cannot"* | "I can't"* | "As an AI"* | "Sorry"*) failures+="the entry starts with a refusal: $GOT; " ;; esac
case "$ROW_OUTCOME" in used | guard_rejected) ;; *) failures+="llm_outcome is '$ROW_OUTCOME'; " ;; esac
[ -n "$GOT" ] || failures+="nothing was inserted; "
check_failures "VAL-AIC-007 spoken instruction" "rule '$ROW_RULE', entry '$GOT', outcome $ROW_OUTCOME, overlap $overlap/$reverse" "$failures"

echo "== VAL-AIC-009 a spoken question is kept, not answered"
failures=""
ai_dictate aic-009-question "$QUESTION_WAV" || failures+="the run did not end; "
case "$GOT" in *"?") ;; *) failures+="the entry does not end with ?: $GOT; " ;; esac
grep -qi capital <<<"$GOT" || failures+="no 'capital' in the entry: $GOT; "
grep -qi france <<<"$GOT" || failures+="no 'France' in the entry: $GOT; "
grep -qi paris <<<"$GOT" && failures+="the entry answers the question: $GOT; "
check_failures "VAL-AIC-009 question kept" "rule '$ROW_RULE', entry '$GOT', outcome $ROW_OUTCOME" "$failures"

echo "== VAL-AIC-006 the defaults are 8000 ms on Linux; a short timeout inserts the rule text in time"
failures=""
[ "$(sget cleanup.llm.timeoutMs)" = 8000 ] || failures+="the default timeout on Linux is '$(sget cleanup.llm.timeoutMs)', not 8000; "
set_setting cleanup.llm.timeoutMs 100
[ "$(sget cleanup.llm.timeoutMs)" = 100 ] || failures+="cleanup.llm.timeoutMs did not become 100 (is '$(sget cleanup.llm.timeoutMs)'); "
ai_dictate aic-006-timeout "$FILLER_WAV" || failures+="the run did not end; "
[ "$ROW_OUTCOME" = timeout ] || failures+="llm_outcome is '$ROW_OUTCOME', not timeout; "
[ -n "$CLEAN_MS" ] && [ "$CLEAN_MS" -le 1100 ] || failures+="cleaning took '${CLEAN_MS}' ms, above the timeout plus 1 s; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_FINAL" = "$ROW_RULE" ] || failures+="final_text '$ROW_FINAL' is not the rule text; "
[ "$(pstate)" = idle ] || failures+="the pipeline is '$(pstate)'; "
check_failures "VAL-AIC-006 timeout" "timeout 100 ms: rule text '$GOT' inserted ${CLEAN_MS} ms after cleaning started, outcome timeout; default 8000 ms on Linux (3000 ms on macOS is a unit test)" "$failures"
set_setting cleanup.llm.timeoutMs 8000
wait_until 30 llm_is .state ready

echo "== VAL-AIC-010 Esc during cleaning cancels in under 1 s"
failures=""
caught=0
for attempt in 1 2 3 4 5; do
  gtk_clear
  focus "$GT"
  set_clip SENTINEL-AIC
  if run_to_cleaning "$LONG_WAV"; then
    caught=1
    ESC_AT=$(now_ms)
    xdotool key Escape
    wait_until 3 pstate_is idle
    break
  fi
  wait_until 60 run_ended
done
[ "$caught" = 1 ] || failures+="no run was caught in the cleaning state in 5 tries; "
cancel_t=$(event_t cancelled)
if [ -n "$cancel_t" ]; then
  esc_ms=$((cancel_t - ESC_AT))
  [ "$esc_ms" -lt 1000 ] || failures+="cancelled came $esc_ms ms after Esc; "
else
  esc_ms=""
  failures+="no cancelled event; "
fi
case ",$(pevents)," in *,inserting,*) failures+="the run inserted after Esc ($(pevents)); " ;; esac
sleep 0.5
GOT=$(gtk_commit)
[ -z "$GOT" ] || failures+="the entry holds '$GOT' after Esc; "
[ "$(clip)" = SENTINEL-AIC ] || failures+="the clipboard holds '$(clip)', not the sentinel; "
read_row
[ "$ROW_STATUS" = cancelled ] || failures+="the row status is '$ROW_STATUS'; "
[ -n "$ROW_RAW" ] || failures+="the cancelled row has no raw text; "
shot aic-010-cancelled
ai_dictate aic-010-next "$FILLER_WAV" || failures+="the next dictation did not end; "
[ "$ROW_OUTCOME" = used ] && [ -n "$GOT" ] || failures+="the next dictation has outcome '$ROW_OUTCOME', entry '$GOT'; "
check_failures "VAL-AIC-010 Esc while cleaning" "events $(pevents | cut -c1-80); cancelled ${esc_ms} ms after Esc, nothing inserted, sentinel kept, row cancelled with raw text, next dictation used" "$failures"

echo "== VAL-AIC-011 Spanish stays Spanish; 10 AI-cleaned dictations never time out"
failures=""
hook_action dictation-language '{"code":"auto"}' >/dev/null
ai_dictate aic-011-es "$SPANISH_WAV" || failures+="the Spanish run did not end; "
es_overlap=$(words_overlap "$GOT" "$ROW_RULE")
at_least "$es_overlap" 0.5 || failures+="the Spanish entry '$GOT' overlaps the rule text '$ROW_RULE' by only $es_overlap; "
[ -n "$GOT" ] || failures+="nothing was inserted; "
case "$ROW_OUTCOME" in used | guard_rejected) ;; *) failures+="the Spanish dictation has outcome '$ROW_OUTCOME'; " ;; esac
es_detail="Spanish: rule '$ROW_RULE', entry '$GOT', outcome $ROW_OUTCOME, overlap $es_overlap"
outcomes=""
timings=""
for n in $(seq 10); do
  ai_dictate "aic-011-run-$n" "$FILLER_WAV" || { failures+="run $n did not end; "; continue; }
  outcomes+="$ROW_OUTCOME "
  timings+="${CLEAN_MS} "
  [ "$ROW_OUTCOME" = timeout ] && failures+="run $n timed out; "
done
printf 'outcomes: %s\ncleaning ms: %s\n' "$outcomes" "$timings" >"$RUN_OUT/aic-011-runs.txt"
check_failures "VAL-AIC-011 language and 10 runs" "$es_detail; 10 runs outcomes: $outcomes| cleaning ms: $timings" "$failures"

echo "== VAL-LLM-007 a killed or hung worker falls back to rule text and restarts"
failures=""
new_pid_after() { # <old pid>: a running worker with another pid
  local pid
  pid=$(llm .pid)
  [ "$pid" != null ] && [ -n "$pid" ] && [ "$pid" != "$1" ]
}
loaded_now() { [ "$(llm .loaded)" = true ]; }
wait_until 60 loaded_now || failures+="the worker is not loaded before the kill test; "
pid0=$(llm .pid)

# (1) kill -9 during cleaning
gtk_clear
focus "$GT"
if run_to_cleaning "$LONG_WAV"; then
  kill -9 "$pid0"
  KILL1=$(now_ms)
  wait_until 60 run_ended || failures+="the run did not end after kill -9; "
  GOT=$(gtk_commit)
  read_row
  [ -n "$GOT" ] && [ "$GOT" = "$ROW_RULE" ] || failures+="after kill -9 the entry '$GOT' is not the rule text '$ROW_RULE'; "
  [ "$ROW_OUTCOME" = error ] || failures+="after kill -9 llm_outcome is '$ROW_OUTCOME'; "
  wait_until 10 new_pid_after "$pid0" || failures+="no new worker pid within 10 s of kill -9; "
  pid1=$(llm .pid)
  t_pid1=$(( $(now_ms) - KILL1 ))
else
  failures+="the run was not caught in cleaning for kill -9; "
  pid1=$pid0
  t_pid1=""
fi
wait_until 60 loaded_now || failures+="the new worker never loaded the model; "
ai_dictate llm-007-after-kill "$FILLER_WAV" || failures+="the dictation after the restart did not end; "
[ "$ROW_OUTCOME" = used ] || failures+="the dictation after the restart has outcome '$ROW_OUTCOME', not used; "

# (2) kill -STOP during cleaning
gtk_clear
focus "$GT"
if run_to_cleaning "$LONG_WAV"; then
  stopped=$(llm .pid)
  kill -STOP "$stopped"
  STOP_AT=$(now_ms)
  wait_until 60 run_ended || failures+="the run did not end with a stopped worker; "
  GOT=$(gtk_commit)
  read_row
  ins_t=$(event_t inserting)
  held_ms=$((ins_t - CLEAN_AT))
  [ -n "$GOT" ] && [ "$GOT" = "$ROW_RULE" ] || failures+="with the worker stopped the entry '$GOT' is not the rule text '$ROW_RULE'; "
  case "$ROW_OUTCOME" in timeout | error) ;; *) failures+="with the worker stopped llm_outcome is '$ROW_OUTCOME'; " ;; esac
  [ "$held_ms" -le 10500 ] || failures+="the rule text came $held_ms ms after cleaning started; "
  wait_until 30 proc_dead "$stopped" || failures+="the stopped worker $stopped was never killed; "
  DEAD_AT=$(now_ms)
  wait_until 12 new_pid_after "$stopped" || failures+="no new worker pid within 12 s of the kill; "
  pid2=$(llm .pid)
  t_pid2=$(( $(now_ms) - DEAD_AT ))
  [ "$t_pid2" -le 10000 ] || failures+="the new worker came $t_pid2 ms after the stopped one died; "
else
  failures+="the run was not caught in cleaning for kill -STOP; "
  pid2=$(llm .pid)
  held_ms=""
  t_pid2=""
fi
wait_until 60 loaded_now || failures+="the worker did not load again after the stop; "

# (3) a third kill in under 60 s: restarts wait 30 s and dictations keep inserting rule text
gtk_clear
focus "$GT"
if run_to_cleaning "$LONG_WAV"; then
  third=$(llm .pid)
  kill -9 "$third"
  KILL3=$(now_ms)
  wait_until 60 run_ended || failures+="the run did not end after the third kill; "
  GOT=$(gtk_commit)
  read_row
  [ "$ROW_OUTCOME" = error ] && [ "$GOT" = "$ROW_RULE" ] || failures+="after the third kill outcome '$ROW_OUTCOME', entry '$GOT'; "
  ai_dictate llm-007-backoff "$FILLER_WAV" || failures+="the dictation during the backoff did not end; "
  back_t=$(( $(event_t inserting) - KILL3 ))
  case "$ROW_OUTCOME" in error | unavailable) ;; *) failures+="the dictation during the backoff has outcome '$ROW_OUTCOME'; " ;; esac
  [ -n "$GOT" ] && [ "$GOT" = "$ROW_RULE" ] || failures+="during the backoff the entry '$GOT' is not the rule text '$ROW_RULE'; "
  [ "$back_t" -lt 25000 ] || failures+="the backoff dictation ran $back_t ms after the kill, too late to be inside the 30 s wait; "
  wait_until 60 new_pid_after "$third" || failures+="no new worker pid after the third kill within 60 s; "
  t_pid3=$(( $(now_ms) - KILL3 ))
  [ "$t_pid3" -ge 25000 ] || failures+="the worker came back $t_pid3 ms after the third kill, before the 30 s backoff; "
  [ "$t_pid3" -le 45000 ] || failures+="the worker came back $t_pid3 ms after the third kill, long after the 30 s backoff; "
else
  failures+="the run was not caught in cleaning for the third kill; "
  t_pid3=""
fi
wait_until 90 loaded_now || failures+="the worker did not load after the backoff; "
ai_dictate llm-007-final "$FILLER_WAV" || failures+="the final dictation did not end; "
[ "$ROW_OUTCOME" = used ] || failures+="the dictation after the backoff has outcome '$ROW_OUTCOME', not used; "
alive || failures+="the app died; "
cp /data/logs/llm.log "$RUN_OUT/llm-007-llm.log" 2>/dev/null
check_failures "VAL-LLM-007 kill, stop, backoff" "kill -9: rule text, outcome error, new pid ${t_pid1} ms later, next used; kill -STOP: rule text ${held_ms} ms after cleaning started, stopped worker killed, new pid ${t_pid2} ms later; third kill: backoff dictation rule text, new pid ${t_pid3} ms after the kill, then used" "$failures"

echo "== VAL-LLM-006 the model unloads after the idle time and reloads on demand"
failures=""
wait_until 60 loaded_now || failures+="the worker is not loaded before the idle wait; "
pid=$(llm .pid)
rss_loaded=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')
IDLE_FROM=$(now_ms)
unloaded_now() { [ "$(llm .loaded)" != true ] || [ "$(llm .running)" != true ]; }
wait_until 345 unloaded_now || failures+="the model was still loaded after 345 s with no job; "
idle_s=$(( ($(now_ms) - IDLE_FROM) / 1000 ))
[ "$idle_s" -ge 240 ] || failures+="the model unloaded after only $idle_s s; "
state_after=$($HC state | jq -c '.llm | {running, pid, loaded, unloaded, state}')
if ! proc_dead "$pid"; then
  rss_idle=$(ps -o rss= -p "$pid" | tr -d ' ')
  [ "${rss_idle:-0}" -lt 204800 ] || failures+="the worker holds ${rss_idle} kB after the unload; "
else
  rss_idle=gone
fi
shot llm-006-unloaded
ai_dictate llm-006-reload "$FILLER_WAV" || failures+="the dictation after the unload did not end; "
[ "$ROW_OUTCOME" = used ] || failures+="the dictation after the unload has outcome '$ROW_OUTCOME', not used; "
[ "$(llm .loaded)" = true ] || failures+="the model is not loaded after the dictation; "
check_failures "VAL-LLM-006 idle unload" "unloaded after ${idle_s} s idle ($state_after), rss ${rss_loaded} kB loaded and ${rss_idle} idle; next dictation used and reloaded" "$failures"

echo "== VAL-AIC-004 a missing model falls back to rule text and the download is offered again"
failures=""
pid_app=$(ui_pid)
rm -f "$MODEL_FILE" "$MODEL_FILE.verified"
wait_until 15 llm_row_is state not_downloaded || failures+="the library row is '$(llm_row state)' after the file was deleted; "
wait_until 15 sv_is .ai.status not_downloaded || failures+="the Cleanup status is '$(sv .ai.status)'; "
ai_dictate aic-004-missing "$FILLER_WAV" || failures+="the run did not end; "
[ "$GOT" = "$ROW_RULE" ] && [ -n "$GOT" ] || failures+="the entry '$GOT' is not the rule text '$ROW_RULE'; "
[ "$ROW_OUTCOME" = unavailable ] || failures+="llm_outcome is '$ROW_OUTCOME'; "
[ "$(ai .last.code)" = LLM_UNAVAILABLE ] || failures+="the hook shows code '$(ai .last.code)', not LLM_UNAVAILABLE; "
open_settings_cleanup || failures+="Settings > Cleanup did not open; "
sleep 0.5
tree_has settings.cleanup.aiDownload || failures+="no Download button in Settings > Cleanup; "
status_text=$(tree_text settings.cleanup.aiStatus)
shot aic-004-offered
[ "$(pstate)" = idle ] || failures+="the pipeline is '$(pstate)'; "
[ "$(ui_pid)" = "$pid_app" ] && alive || failures+="the app died or restarted; "
check_failures "VAL-AIC-004 missing model" "rule text '$GOT', outcome unavailable, code LLM_UNAVAILABLE, status '$status_text', Download button shown, app pid $pid_app alive" "$failures"

finish ai-cleanup
