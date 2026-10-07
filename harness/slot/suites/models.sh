#!/bin/bash
# Model library checks (VAL-MOD-002, 003, 004, 005, 008). Busy: it downloads base.en and small.en
# from Hugging Face (the slot needs network), hashes model files, and runs the engine. It empties
# /data and restarts the app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh

HC=/app/hookctl
SPEECH=/assets/fixtures/speech-short.wav
MODELS=/data/models/whisper
DOWNLOADS=/data/cache/downloads
ENGINE_LOG=/data/logs/engine.log

# Pinned in assets/models.json
BASE_EN_SHA=a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002
BASE_EN_BYTES=147964211
SMALL_EN_SHA=c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d

hook_action() { $HC action "$@"; }
# m <id> <jq expression on that model's state>
m() { $HC state | jq -r ".models.models[] | select(.id == \"$1\") | $2"; }
m_is() { [ "$(m "$1" "$2")" = "$3" ]; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
now_ms() { echo $(($(date +%s%N) / 1000000)); }
index_of() { $HC state | jq -r ".models.models | map(.id) | index(\"$1\")"; }
# click_row <button> <id> clicks models.<button>.<index of id>
click_row() { $HC click "models.$1.$(index_of "$2")" >/dev/null; }
progress_at_least() { [ "$(m "$1" '.progress // 0')" -ge "$2" ]; }
downloads_empty() { [ -z "$(ls "$DOWNLOADS" 2>/dev/null)" ]; }
download_files() { ls "$DOWNLOADS" 2>/dev/null; }
engine_has() { $HC state | jq -r .engine.model | grep -q "$1"; }
job_done() { [ "$($HC state | jq -r .engine.last_job.state)" = done ]; }
engine_up() { [ "$($HC state | jq -r .engine.model)" != null ]; }

start_app() {
  /harness/slot/app.sh start >/dev/null
  wait_until 15 $HC state
}

# fresh [model names from /assets]: empties /data, copies the models, starts the app
fresh() {
  /harness/slot/app.sh stop
  find /data -mindepth 1 -delete
  mkdir -p "$MODELS"
  for name in "$@"; do cp "/assets/models/whisper/ggml-$name.bin" "$MODELS/"; done
  start_app
}

# select_model <id>: waits until the model is verified, then chooses it like the Use button
select_model() {
  wait_until 30 m_is "$1" .state ready || return 1
  hook_action model-select "{\"id\":\"$1\"}" >/dev/null
}

models_rendered() { $HC tree | jq -e 'any(.[]; .id | startswith("models.row."))' >/dev/null; }

open_models() {
  $HC click sidebar.models >/dev/null
  wait_until 5 view_is models
  # The tree follows the next frame: wait until the rows exist before a click targets one.
  wait_until 5 models_rendered
}

echo "== VAL-MOD-002 the view lists the catalog with state"
fresh tiny.en
select_model tiny.en
open_models
sleep 0.5
shot 002-models
$HC state | jq .models >"$RUN_OUT/002-state.json"
$HC tree >"$RUN_OUT/002-tree.json"
failures=""
rows=$(jq '[.[] | select(.id | test("^models\\.row\\.[0-9]+$"))] | length' "$RUN_OUT/002-tree.json")
buttons=$(jq '[.[] | select(.id | test("^models\\.download\\.[0-9]+$"))] | length' "$RUN_OUT/002-tree.json")
listed=$(jq -r '[.models[].id] | join(",")' "$RUN_OUT/002-state.json")
[ "$rows" = 8 ] || failures+="$rows rows in the tree, want 8; "
[ "$listed" = "tiny,tiny.en,base,base.en,small,small.en,medium,large-v3-turbo" ] || failures+="catalog order is $listed; "
[ "$buttons" = 7 ] || failures+="$buttons Download buttons, want 7; "
[ "$(jq -r '.models[] | select(.id == "tiny.en") | "\(.state) \(.active)"' "$RUN_OUT/002-state.json")" = "ready true" ] || failures+="tiny.en is not ready and active; "
[ "$(jq '[.models[] | select(.state == "not_downloaded")] | length' "$RUN_OUT/002-state.json")" = 7 ] || failures+="not 7 models are not_downloaded; "
[ "$(jq '[.models[] | select(.bytes > 0)] | length' "$RUN_OUT/002-state.json")" = 8 ] || failures+="a model has no size; "
[ "$(jq '[.[] | select(.id == "models.download.1")] | length' "$RUN_OUT/002-tree.json")" = 0 ] || failures+="tiny.en has a Download button; "
check_failures "VAL-MOD-002 catalog view" "8 rows in catalog order, tiny.en ready and active, 7 not downloaded with a Download button" "$failures"

echo "== VAL-MOD-003 download with progress, verified stamp, responsive UI"
fresh tiny.en
select_model tiny.en
open_models
click_row download base.en
samples=() part_seen=0 responsive="" h_ms=-1 m_ms=-1
started=$(now_ms)
while [ $(($(now_ms) - started)) -lt 120000 ]; do
  state=$(m base.en .state)
  [ "$state" = ready ] && break
  [ "$state" = downloading ] && samples+=("$(m base.en '.progress // 0')")
  download_files | grep -q '\.download$' && part_seen=1
  if [ -z "$responsive" ] && [ "${#samples[@]}" -ge 5 ]; then
    shot 003-during
    t0=$(now_ms)
    $HC click sidebar.history >/dev/null
    wait_until 1 view_is history && h_ms=$(($(now_ms) - t0)) || h_ms=-1
    t0=$(now_ms)
    $HC click sidebar.models >/dev/null
    wait_until 1 view_is models && m_ms=$(($(now_ms) - t0)) || m_ms=-1
    responsive="History ${h_ms} ms, Models ${m_ms} ms"
  fi
  sleep 0.2
done
wait_until 10 m_is base.en .state ready
shot 003-done
failures=""
prev=-1 dropped=0
for p in "${samples[@]}"; do
  [ "$p" -lt "$prev" ] && dropped=$((dropped + 1))
  prev=$p
done
first=${samples[0]:--1}
last=${samples[${#samples[@]} - 1]:--1}
[ "${#samples[@]}" -ge 5 ] || failures+="only ${#samples[@]} progress samples; "
[ "$dropped" = 0 ] || failures+="progress fell $dropped times; "
[ "$first" -ge 0 ] && [ "$first" -lt 10 ] || failures+="first progress was $first, want below 10; "
[ "$last" -ge 70 ] || failures+="last progress before the end was $last; "
[ "$part_seen" = 1 ] || failures+="no *.download file while it ran; "
[ -n "$responsive" ] || failures+="the UI was not tested during the download; "
{ [ "$h_ms" -ge 0 ] && [ "$h_ms" -le 1000 ] && [ "$m_ms" -ge 0 ] && [ "$m_ms" -le 1000 ]; } || failures+="a view change took over 1 s ($responsive); "
size=$(stat -c %s "$MODELS/ggml-base.en.bin" 2>/dev/null)
sum=$(sha256sum "$MODELS/ggml-base.en.bin" 2>/dev/null | awk '{print $1}')
stamp="$MODELS/ggml-base.en.bin.verified"
[ "$size" = "$BASE_EN_BYTES" ] || failures+="size is $size; "
[ "$sum" = "$BASE_EN_SHA" ] || failures+="sha256 is $sum; "
[ "$(jq -r '"\(.algo) \(.hash) \(.size)"' "$stamp" 2>/dev/null)" = "sha256 $BASE_EN_SHA $BASE_EN_BYTES" ] || failures+="stamp is wrong: $(cat "$stamp" 2>/dev/null); "
[ "$(jq -r '.mtime_ms | type' "$stamp" 2>/dev/null)" = number ] || failures+="stamp has no mtime; "
downloads_empty || failures+="left in downloads: $(download_files | tr '\n' ' '); "
$HC net >"$RUN_OUT/003-net.json"
[ "$(jq '[.[] | select(.purpose == "model_download" and .host == "huggingface.co")] | length' "$RUN_OUT/003-net.json")" -ge 1 ] || failures+="hookctl net lists no huggingface.co request; "
[ "$(jq '[.[] | select(.purpose != "model_download" or (.host != "huggingface.co" and (.host | endswith(".hf.co") | not)))] | length' "$RUN_OUT/003-net.json")" = 0 ] || failures+="hookctl net lists a request outside the allow-list; "
check_failures "VAL-MOD-003 download" "${#samples[@]} samples rose from $first% to $last% then ready; size, sha256, and stamp match; no *.download left; $responsive; net: $(jq -r '[.[] | .host + " " + .result] | join(", ")' "$RUN_OUT/003-net.json")" "$failures"

echo "== VAL-MOD-004 cancel and kill -9"
fresh tiny.en
select_model tiny.en
open_models
click_row download small.en
wait_until 60 progress_at_least small.en 10
at_cancel=$(m small.en '.progress // 0')
shot 004-before-cancel
t0=$(now_ms)
click_row cancel small.en
wait_until 2 m_is small.en .state not_downloaded
cancel_ms=$(($(now_ms) - t0))
sleep 1
shot 004-after-cancel
failures=""
[ "$(m small.en .state)" = not_downloaded ] || failures+="state after cancel is $(m small.en .state); "
[ "$cancel_ms" -le 2000 ] || failures+="cancel took $cancel_ms ms; "
[ "$(m small.en .progress)" = null ] || failures+="progress is still $(m small.en .progress); "
[ ! -e "$MODELS/ggml-small.en.bin" ] || failures+="a model file exists after cancel; "
wait_until 2 downloads_empty || failures+="left in downloads: $(download_files | tr '\n' ' '); "
check_failures "VAL-MOD-004 cancel" "cancelled at ${at_cancel}%, not_downloaded after $cancel_ms ms, no file in downloads or models, progress null" "$failures"

click_row download small.en
wait_until 60 progress_at_least small.en 15
at_kill=$(m small.en '.progress // 0')
partial_kb=$(du -k "$DOWNLOADS"/*.download 2>/dev/null | awk '{s += $1} END {print s + 0}')
kill -9 "$(cat /tmp/app.pid)"
sleep 1
rm -f /run/hook.sock /tmp/app.pid
start_app
open_models
shot 004-after-kill
failures=""
[ "$(m small.en .state)" = not_downloaded ] || failures+="state after the kill is $(m small.en .state); "
[ ! -e "$MODELS/ggml-small.en.bin" ] || failures+="a model file exists after the kill; "
left=$(download_files | tr '\n' ' ')
click_row download small.en
wait_until 180 m_is small.en .state ready
shot 004-redownloaded
sum=$(sha256sum "$MODELS/ggml-small.en.bin" 2>/dev/null | awk '{print $1}')
[ "$(m small.en .state)" = ready ] || failures+="the new download ended as $(m small.en .state); "
[ "$sum" = "$SMALL_EN_SHA" ] || failures+="sha256 after the new download is $sum; "
[ -f "$MODELS/ggml-small.en.bin.verified" ] || failures+="no stamp after the new download; "
downloads_empty || failures+="left in downloads: $(download_files | tr '\n' ' '); "
check_failures "VAL-MOD-004 kill -9" "killed at ${at_kill}% with ${partial_kb} KB partial (after restart: ${left:-nothing kept}); not_downloaded after restart; the new download matched sha256 and has a stamp" "$failures"

echo "== VAL-MOD-005 a changed file fails the check"
fresh tiny.en base.en
select_model tiny.en
wait_until 30 m_is base.en .state ready
/harness/slot/app.sh stop
# One byte flipped in the middle: same size, new mtime.
middle=$((BASE_EN_BYTES / 2))
orig=$(dd if="$MODELS/ggml-base.en.bin" bs=1 skip=$middle count=1 2>/dev/null | od -An -tx1 | tr -d ' ')
flip=$(printf '%02x' $(((0x$orig + 1) % 256)))
printf "\\x$flip" | dd of="$MODELS/ggml-base.en.bin" bs=1 seek=$middle conv=notrunc 2>/dev/null
start_app
open_models
wait_until 60 m_is base.en .state failed
shot 005-failed
click_row select base.en
sleep 0.5
shot 005-select-refused
$HC state >"$RUN_OUT/005-state.json"
hook_action engine-transcribe "{\"wav\":\"$SPEECH\",\"language\":\"en\"}" >/dev/null
wait_until 30 job_done
cp "$ENGINE_LOG" "$RUN_OUT/005-engine.log"
failures=""
[ "$(m base.en .state)" = failed ] || failures+="base.en is $(m base.en .state), want failed; "
code=$(jq -r .models.notice.code "$RUN_OUT/005-state.json")
[ "$code" = MODEL_HASH_MISMATCH ] || failures+="notice code is $code; "
[ "$(jq -r .models.active "$RUN_OUT/005-state.json")" = tiny.en ] || failures+="active is $(jq -r .models.active "$RUN_OUT/005-state.json"); "
[ "$(jq -r '.values["dictation.modelId"]' /data/config/settings.json)" = tiny.en ] || failures+="settings.json does not name tiny.en; "
loads=$(grep -c "LOADED.*base.en" "$RUN_OUT/005-engine.log")
[ "$loads" = 0 ] || failures+="engine.log shows $loads loads of base.en; "
job=$(grep " JOB " "$RUN_OUT/005-engine.log" | tail -1)
case "$job" in *tiny.en*) ;; *) failures+="the job did not use tiny.en: $job; " ;; esac
check_failures "VAL-MOD-005 changed file" "base.en failed, select refused with $code, tiny.en stays active, no engine load of base.en; job: $job" "$failures"

echo "== VAL-MOD-008 delete, and the active model cannot be deleted"
fresh tiny.en base.en
select_model tiny.en
wait_until 30 m_is base.en .state ready
open_models
shot 008-before
click_row delete base.en
wait_until 5 m_is base.en .state not_downloaded
shot 008-after-delete
failures=""
[ ! -e "$MODELS/ggml-base.en.bin" ] || failures+="base.en .bin is still there; "
[ ! -e "$MODELS/ggml-base.en.bin.verified" ] || failures+="base.en stamp is still there; "
[ "$(m base.en .state)" = not_downloaded ] || failures+="base.en is $(m base.en .state); "
click_row delete tiny.en
sleep 0.5
shot 008-in-use
code=$($HC state | jq -r .models.notice.code)
[ "$code" = MODEL_IN_USE ] || failures+="notice code is $code; "
{ [ -f "$MODELS/ggml-tiny.en.bin" ] && [ -f "$MODELS/ggml-tiny.en.bin.verified" ]; } || failures+="tiny.en file or stamp is gone; "
[ "$(m tiny.en .state)" = ready ] || failures+="tiny.en is $(m tiny.en .state); "
check_failures "VAL-MOD-008 delete" "base.en and its stamp deleted; deleting active tiny.en gave $code and kept its file and stamp" "$failures"

echo "== select loads the model in the running engine"
fresh tiny.en base.en
select_model tiny.en
wait_until 20 engine_up
pid=$($HC state | jq -r .engine.pid)
wait_until 30 m_is base.en .state ready
hook_action model-select '{"id":"base.en"}' >/dev/null
wait_until 20 engine_has base.en
failures=""
engine_has base.en || failures+="engine model is $($HC state | jq -r .engine.model); "
[ "$($HC state | jq -r .engine.pid)" = "$pid" ] || failures+="engine pid changed; "
[ "$(jq -r '.values["dictation.modelId"]' /data/config/settings.json)" = base.en ] || failures+="settings.json does not name base.en; "
check_failures "model switch" "base.en loaded with engine pid $pid unchanged; settings.json names base.en" "$failures"
