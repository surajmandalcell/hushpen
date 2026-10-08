#!/bin/bash
# Helpers for the suites that dictate through the real microphone path (home, tail). Source it
# after lib.sh. fresh, use_model, dictate and the dict_* readers drive the app through hookctl.

# shellcheck disable=SC2034
HC=/app/hookctl
FIX=/assets/fixtures
MODELS=/data/models/whisper
SESSIONS=/data/cache/sessions
SETTINGS=/data/config/settings.json
ENGINE_LOG=/data/logs/engine.log
SHORT_WORDS=(the quick brown fox jumps over lazy dog)

hook_action() { $HC action "$@"; }
now_ms() { echo $(($(date +%s%N) / 1000000)); }
dict() { $HC state | jq -r ".dictation$1"; }
dict_is() { [ "$(dict "$1")" = "$2" ]; }
dict_final() { case "$(dict .state)" in done | no-speech | failed) return 0 ;; esac; return 1; }
engine() { $HC state | jq -r ".engine$1"; }
engine_is() { [ "$(engine "$1")" = "$2" ]; }
engine_ready_with() { [ "$(engine .state)" = ready ] && [ "$(engine .model)" = "$1" ]; }
m_state() { $HC state | jq -r ".models.models[] | select(.id == \"$1\") | .state"; }
model_ready() { [ "$(m_state "$1")" = ready ]; }
clip() { xclip -o -selection clipboard 2>/dev/null; }
set_clip() { printf '%s' "$1" | xclip -selection clipboard -i; }
norm() { tr 'A-Z' 'a-z' | tr -c 'a-z0-9 \n' ' ' | tr -s ' '; }
ui_pid() { cat /tmp/app.pid; }

missing_words() { # <text> <words...>
  local text=" $1 " word missing=""
  shift
  for word in "$@"; do
    case "$text" in *" $word "*) ;; *) missing+="$word " ;; esac
  done
  echo "$missing"
}

start_app() {
  /harness/slot/app.sh start >/dev/null
  wait_until 20 dict_is .state idle
}

# fresh [models...]: empties /data, copies the models from /assets, starts the app
fresh() {
  /harness/slot/app.sh stop
  find /data -mindepth 1 -delete
  mkdir -p "$MODELS"
  for name in "$@"; do cp "/assets/models/whisper/ggml-$name.bin" "$MODELS/"; done
  start_app
}

# use_model <id>: waits for the hash check, chooses the model, waits until the engine holds it
use_model() {
  wait_until 60 model_ready "$1" || return 1
  hook_action model-select "{\"id\":\"$1\"}" >/dev/null
  wait_until 90 engine_ready_with "$1"
}

pad() { # <wav> [lead_ms] -> path of the padded copy
  mkdir -p "$RUN_OUT/padded"
  python3 /harness/slot/pad-wav.py "$1" "$RUN_OUT/padded/$(basename "$1")" "${2:-1000}" >/dev/null
  echo "$RUN_OUT/padded/$(basename "$1")"
}

# dictate <label> <wav>: one Home dictation through the real microphone path. Sets
# RUN_T0 (ms) and returns when the run has ended in done, no-speech, or failed.
dictate() {
  local label=$1 wav=$2 padded
  padded=$(pad "$wav")
  RUN_T0=$(now_ms)
  hook_action dictation-record >/dev/null || return 1
  wait_until 5 dict_is .state listening || return 1
  shot "$label-listening"
  paplay --device=vmic "$padded"
  hook_action dictation-record >/dev/null || return 1
  shot "$label-after-stop"
  wait_until 240 dict_final || return 1
  sleep 0.3
  shot "$label-final"
}

# run_events: the dictation hook events since RUN_T0, as "a,b,c"
run_events() {
  $HC events | jq -r --argjson t0 "$RUN_T0" \
    '[.[] | select(.kind == "dictation" and .t_ms >= $t0) | .detail] | join(",")'
}

tree_text() { $HC tree | jq -r ".[] | select(.id == \"$1\") | .text"; }
