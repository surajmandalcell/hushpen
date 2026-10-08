#!/bin/bash
# Capture tail checks: a clip that ends on speech, with no trailing silence, is played into the
# virtual microphone and the recording is stopped the moment the player returns. The last word
# must be in the transcript. Busy: it transcribes with base. It empties /data and restarts the
# app, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

trimmed() { # <fixture> -> path of the copy that ends on its last loud sample
  mkdir -p "$RUN_OUT/trimmed"
  python3 /harness/slot/trim-tail-wav.py "$FIX/$1" "$RUN_OUT/trimmed/$1" >>"$RUN_OUT/trimmed/log.txt"
  echo "$RUN_OUT/trimmed/$1"
}

echo "== the last word of a 45 s clip that ends on speech"
fresh base
use_model base
text=""
failures=""
for run in 1 2 3; do
  dictate "tail-45s-$run" "$(trimmed dictation-45s.wav)"
  text=$(dict .transcript)
  [ "$(dict .state)" = done ] || failures+="run $run: state is $(dict .state); "
  case " $(norm <<<"$text") " in
    *" the final word of this dictation is lighthouse "*) ;;
    *) failures+="run $run: the last sentence is missing: $text; " ;;
  esac
done
check_failures "tail 45 s clip" "3 of 3 stops right after the last word kept 'the final word of this dictation is lighthouse'" "$failures"

echo "== the last word of a short clip that ends on speech"
failures=""
for run in 1 2 3; do
  dictate "tail-short-$run" "$(trimmed speech-short.wav)"
  text=$(dict .transcript)
  [ "$(dict .state)" = done ] || failures+="run $run: state is $(dict .state); "
  case "$(norm <<<"$text" | sed 's/ *$//')" in
    *" 5" | *" five") ;;
    *) failures+="run $run: the text does not end with 'five': $text; " ;;
  esac
done
check_failures "tail short clip" "3 of 3 stops right after the last word kept 'five'" "$failures"
