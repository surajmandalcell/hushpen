#!/bin/bash
# usage (in a slot): flowbar-motion.sh setup | [REDUCE=1] take <label> [wav]
# Records the flow bar at 30 fps through one dictation, to prove that the processing wave moves
# (pixel tests only see still frames). `setup` empties /data, loads Small Q5 (slow enough on two
# CPUs that the transcribing wave runs past its 2 s turn) and the S1-mini cleanup model, so the
# bar shows the transcribing and the cleaning wave. Start the slot with VAL_CPUS=4 VAL_MEM=4g.
# `take` feeds the WAV (default: two fixtures joined, about 9 s) and writes to $RUN_OUT/<label>/:
# frames.raw (BGRX, region in region.txt), frames.raw.times, states.txt ("<unix ms> <dictation
# state> <AI phase> <bar state>" on each change), and diff.txt (see grab-region.py). REDUCE=1 turns
# on Reduce Motion for the take through the hook.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

LLM_ID=s1-mini
LLM_FILE=/data/models/llm/$LLM_ID.gguf
GRAB=/harness/slot/grab-region.py
set_setting() { hook_action set-setting "{\"key\":\"$1\",\"value\":$2}" >/dev/null; }
llm() { $HC state | jq -r ".llm$1"; }
bar() { $HC state | jq -r ".overlay$1"; }
llm_loaded() { [ "$(llm .loaded)" = true ]; }
bar_gone() { [ "$(bar .visible)" = false ]; }

setup() {
  fresh small-q5_1
  use_model small-q5_1 || { echo "Small Q5 did not load" >&2; return 1; }
  set_setting cleanup.llm.provider '"local"'
  set_setting cleanup.llm.modelId "\"$LLM_ID\""
  set_setting cleanup.llm.enabled false
  /harness/slot/app.sh stop
  mkdir -p /data/models/llm
  cp /fixtures/llm/s1-mini-q4_k_m.gguf "$LLM_FILE"
  start_app
  wait_until 600 test -e "$LLM_FILE.verified" || { echo "S1-mini was not verified" >&2; return 1; }
  use_model small-q5_1 || return 1
  set_setting cleanup.llm.enabled true
  wait_until 180 llm_loaded || { echo "S1-mini did not load" >&2; return 1; }
  echo "setup done"
}

joined_clip() {
  local out=$RUN_OUT/gen/joined.wav
  mkdir -p "$RUN_OUT/gen"
  python3 - /fixtures/filler-dictation.wav /fixtures/ai-self-correction.wav "$out" <<'PY'
import sys
import wave

frames, params = b"", None
for path in sys.argv[1:3]:
    with wave.open(path) as clip:
        params = params or clip.getparams()
        frames += clip.readframes(clip.getnframes()) + b"\0" * (params.sampwidth * params.framerate // 3)
with wave.open(sys.argv[3], "wb") as out:
    out.setparams(params)
    out.writeframes(frames)
PY
  echo "$out"
}

take() {
  local label=$1 wav=${2:-} dir padded secs bx by bw bh x y w h stop
  dir=$RUN_OUT/$label
  rm -rf "$dir"
  mkdir -p "$dir"
  [ -n "$wav" ] || wav=$(joined_clip)
  padded=$(pad "$wav" 1000)
  wait_until 90 engine_ready_with small-q5_1 || { echo "Small Q5 is not loaded" >&2; return 1; }
  hook_action set-reduce-motion "{\"on\":$([ "${REDUCE:-0}" = 1 ] && echo true || echo false)}" >/dev/null || return 1
  secs=$(python3 /harness/slot/wavstat.py "$padded" | sed -n 's/.*secs=\([0-9.]*\).*/\1/p')

  hook_action dictation-record >/dev/null || return 1
  wait_until 5 dict_is .state listening || return 1
  sleep 0.5
  read -r bx by bw bh <<<"$(bar .bounds | jq -r '"\(.x) \(.y) \(.width) \(.height)"')"
  # Wide enough for the widest processing pill (216 px) with room for the bar to grow and fade.
  w=280 h=64
  x=$(awk -v a="$bx" -v b="$bw" -v w=$w 'BEGIN {printf "%d", a + b / 2 - w / 2}')
  y=$(awk -v a="$by" -v b="$bh" -v h=$h 'BEGIN {printf "%d", a + b / 2 - h / 2}')
  echo "$x $y $w $h" >"$dir/region.txt"
  stop=$dir/stop
  python3 "$GRAB" grab "$x" "$y" "$w" "$h" 30 "$dir/frames.raw" "$stop" 120 >"$dir/grab.txt" &
  local grab_pid=$!
  (
    last=""
    while [ ! -e "$stop" ]; do
      now="$($HC state | jq -r '"\(.dictation.state) \(.pipeline.ai.phase) \(.overlay.state)"')"
      [ "$now" = "$last" ] || echo "$(now_ms) $now"
      last=$now
      sleep 0.05
    done
  ) >"$dir/states.txt" &
  local states_pid=$!

  $HC feed-wav "$padded" >"$dir/feed.json" || return 1
  sleep "$(awk -v s="$secs" 'BEGIN {printf "%.2f", s + 0.4}')"
  hook_action dictation-record >/dev/null || return 1
  wait_until 240 dict_final
  wait_until 15 bar_gone
  sleep 0.5
  touch "$stop"
  wait "$grab_pid" "$states_pid"
  python3 "$GRAB" diff "$dir/frames.raw" "$w" "$h" >"$dir/diff.txt"
  sqlite3 /data/history/history.db \
    "select raw_text, llm_text, llm_outcome from transcript order by created_at desc limit 1" >"$dir/row.txt"
  echo "frames $(cat "$dir/grab.txt") region $x $y $w $h"
  cat "$dir/states.txt"
}

case "${1:-}" in
  setup) setup ;;
  take) shift; take "$@" ;;
  *) echo "usage: flowbar-motion.sh setup | take <label> [wav]" >&2; exit 2 ;;
esac
