#!/bin/bash
# Microphone capture checks (VAL-MIC-001 to 008). Busy: it plays audio into the virtual mic and
# transcribes with tiny.en. Runs in one slot through with-env.sh; it restarts the app, empties
# /data, and loads and unloads PulseAudio modules, so the slot must not be shared.
. /harness/slot/lib.sh

HC=/app/hookctl
SPEECH=/assets/fixtures/speech-short.wav
DICTATION=/assets/fixtures/dictation-45s.wav
MODEL=/assets/models/whisper/ggml-tiny.en.bin
SESSIONS=/data/cache/sessions
REST_HEIGHT=3

cap() { $HC state | jq -r ".capture$1"; }
hook_action() { $HC action "$@"; }
now_ms() { echo $(($(date +%s%N) / 1000000)); }

# cap_is <jq path> <value>, cap_not <jq path> <value>: polled with wait_until
cap_is() { [ "$(cap "$1")" = "$2" ]; }
cap_not() { [ "$(cap "$1")" != "$2" ]; }

wait_state() { # <state> <seconds>
  wait_until "$2" cap_is .state "$1"
}

start_app() { # starts the app and waits for the hook to answer
  /harness/slot/app.sh start >/dev/null
  wait_until 10 cap_is .state idle
}

reset_app() { # empties /data and starts the app
  /harness/slot/app.sh stop
  find /data -mindepth 1 -delete
  start_app
}

load_mic_modules() {
  pactl load-module module-null-sink sink_name=vmic >/dev/null
  pactl load-module module-virtual-source source_name=vmic_src master=vmic.monitor >/dev/null
  pactl load-module module-null-sink sink_name=cues >/dev/null
  pactl set-default-source vmic_src
  pactl set-default-sink cues
}

module_ids() { # <pattern>
  pactl list short modules | awk -v p="$1" '$0 ~ p {print $1}'
}

unload_modules() { # <pattern>
  # unloading a null sink also unloads the virtual source on top of it, so a later id may be gone
  for id in $(module_ids "$1"); do pactl unload-module "$id" 2>/dev/null; done
}

# "<tallest bar> <newest bar>" of the meter in px, from the hook tree. The bars are the last 1.6 s
# of levels, so the newest one is the live reading.
bar_heights() {
  $HC tree | jq -r '[.[] | select(.id | startswith("home.mic.bar."))] as $bars |
    "\([$bars[].bounds.height] | max) \($bars[] | select(.id == "home.mic.bar.31") | .bounds.height)"'
}

# header "rate channels bits frames" of a WAV, read with python's wave module
wav_header() {
  python3 - "$1" <<'PY'
import sys, wave
with wave.open(sys.argv[1], "rb") as w:
    print(w.getframerate(), w.getnchannels(), w.getsampwidth() * 8, w.getnframes())
PY
}

# words <wav> -> lower-case transcript from tiny.en
transcript() {
  /app/hushpen engine --smoke "$1" --model "$MODEL" 2>/dev/null |
    sed -n 's/^text: //p' | tr 'A-Z' 'a-z' | tr -c 'a-z0-9 \n' ' ' | tr -s ' '
}

# missing_words <transcript> <words...>
missing_words() {
  local text=" $1 " word missing=""
  shift
  for word in "$@"; do
    case "$text" in *" $word "*) ;; *) missing+="$word " ;; esac
  done
  echo "$missing"
}

newest_session() { ls -t "$SESSIONS"/*.wav 2>/dev/null | head -1; }

# speech_session <label> pa|feed [device]: starts a capture, plays padded speech-short.wav, samples
# the meter every 200 ms with a screenshot each, stops, and sets SESSION_* variables.
speech_session() {
  local label=$1 mode=$2 device=${3:-vmic}
  local padded=$RUN_OUT/padded/$(basename "$SPEECH")
  mkdir -p "$RUN_OUT/$label" "$RUN_OUT/padded"
  python3 /harness/slot/pad-wav.py "$SPEECH" "$padded" 1000 >/dev/null
  local started
  started=$(now_ms)
  hook_action capture-start >/dev/null || return 1
  wait_state listening 5 || return 1
  local play_at
  play_at=$(now_ms)
  if [ "$mode" = pa ]; then
    paplay --device="$device" "$padded" &
  else
    (cd "$RUN_OUT/padded" && $HC feed-wav "$(basename "$SPEECH")" >/dev/null)
  fi
  local player=$!
  SAMPLES_LEAD=() SAMPLES_SPEECH=() SHOT_HASHES=()
  local i=0 t tallest newest
  while true; do
    t=$(($(now_ms) - play_at))
    if [ "$mode" = pa ]; then
      kill -0 "$player" 2>/dev/null || break
    else
      [ "$t" -lt 6100 ] || break
    fi
    import -window root "$RUN_OUT/$label/shot-$(printf %02d "$i").png"
    read -r tallest newest < <(bar_heights)
    if [ "$t" -lt 800 ]; then SAMPLES_LEAD+=("$tallest"); fi
    if [ "$t" -ge 1500 ] && [ "$t" -le 5800 ]; then SAMPLES_SPEECH+=("$newest"); fi
    echo "$t $tallest $newest $(cap .level)" >>"$RUN_OUT/$label/meter.log"
    i=$((i + 1))
    sleep 0.05
  done
  [ "$mode" = pa ] && wait "$player" 2>/dev/null
  local stopped
  SESSION_STOP=$(hook_action capture-stop '{"keep":true}')
  stopped=$(now_ms)
  SESSION_WALL_MS=$((stopped - started))
  SHOT_HASHES=$(md5sum "$RUN_OUT/$label"/shot-*.png | awk '{print $1}' | sort -u | wc -l)
  SESSION_WAV=$(jq -r .path <<<"$SESSION_STOP")
  SESSION_MS=$(jq -r .duration_ms <<<"$SESSION_STOP")
}

echo "== VAL-MIC-001 picker"
reset_app
read -r ox oy < <(settled_origin "$(app_window)")
saved=$(jq -r '.values["audio.inputDeviceId"]' /data/config/settings.json)
$HC state | jq .capture >"$RUN_OUT/001-capture-state.json"
$HC tree >"$RUN_OUT/001-tree.json"
shot 001-picker
listed=$(jq -r '[.devices[] | select(.id | test("vmic_src"))] | length' "$RUN_OUT/001-capture-state.json")
has_default=$(jq -r '[.[] | select(.id == "home.mic.device.default")] | length' "$RUN_OUT/001-tree.json")
failures=""
[ "$saved" = default ] || failures+="settings audio.inputDeviceId is $saved; "
[ "$listed" = 1 ] || failures+="the picker lists $listed entries for vmic_src; "
[ "$has_default" = 1 ] || failures+="no Default row in the tree; "
[ "$(cap .active_device)" = default ] || failures+="active device is $(cap .active_device); "
check_failures "VAL-MIC-001 picker" "settings default, picker lists Default and $(jq -r '.devices[0].id' "$RUN_OUT/001-capture-state.json"), Default active" "$failures"

echo "== VAL-MIC-002 and 003 level meter and session WAV (play to vmic)"
speech_session pa-vmic pa vmic
lead_bad=0
for h in "${SAMPLES_LEAD[@]}"; do [ "$h" = "$REST_HEIGHT.0" ] || [ "$h" = "$REST_HEIGHT" ] || lead_bad=$((lead_bad + 1)); done
distinct=$(printf '%s\n' "${SAMPLES_SPEECH[@]}" | awk -v r="$REST_HEIGHT" '$1 > r' | sort -u | wc -l)
shots=$(ls "$RUN_OUT/pa-vmic"/shot-*.png | wc -l)
failures=""
[ "$lead_bad" = 0 ] || failures+="$lead_bad of ${#SAMPLES_LEAD[@]} lead-silence samples left the rest height; "
[ "$distinct" -ge 3 ] || failures+="only $distinct distinct heights above rest during speech; "
[ "$shots" -ge 5 ] || failures+="only $shots screenshots; "
[ "$SHOT_HASHES" -ge 5 ] || failures+="only $SHOT_HASHES different screenshots; "
check_failures "VAL-MIC-002 level meter" "${#SAMPLES_LEAD[@]} lead samples at rest, $distinct heights above rest in ${#SAMPLES_SPEECH[@]} speech samples, $shots screenshots ($SHOT_HASHES different)" "$failures"

read -r rate ch bits frames < <(wav_header "$SESSION_WAV")
wav_secs=$(awk -v f="$frames" 'BEGIN {printf "%.2f", f / 16000}')
diff_ms=$((SESSION_WALL_MS - SESSION_MS))
[ "$diff_ms" -lt 0 ] && diff_ms=$((-diff_ms))
text=$(transcript "$SESSION_WAV")
missing=$(missing_words "$text" the quick brown fox jumps over lazy dog)
failures=""
[ "$rate $ch $bits" = "16000 1 16" ] || failures+="header is $rate Hz $ch ch $bits bit; "
[ "$diff_ms" -le 300 ] || failures+="WAV is ${SESSION_MS} ms but start to stop took ${SESSION_WALL_MS} ms; "
[ -z "$missing" ] || failures+="transcript misses: $missing ($text); "
check_failures "VAL-MIC-003 session WAV" "16000 Hz mono 16-bit, $wav_secs s vs $SESSION_WALL_MS ms wall (diff $diff_ms ms), transcript: $text" "$failures"

# A 2 s recording with nothing playing: the virtual mic sends no audio at all, so the first
# callback comes late, and the file must still follow the wall clock.
started=$(now_ms)
hook_action capture-start >/dev/null
wait_state listening 5
sleep 2
quiet_stop=$(hook_action capture-stop '{"keep":true}')
quiet_wall_ms=$(($(now_ms) - started))
quiet_ms=$(jq -r .duration_ms <<<"$quiet_stop")
quiet_diff=$((quiet_wall_ms - quiet_ms))
[ "$quiet_diff" -lt 0 ] && quiet_diff=$((-quiet_diff))
failures=""
[ "$quiet_diff" -le 300 ] || failures+="the 2 s WAV is ${quiet_ms} ms but start to stop took ${quiet_wall_ms} ms; "
check_failures "VAL-MIC-003 short session WAV" "$quiet_ms ms vs $quiet_wall_ms ms wall (diff $quiet_diff ms), no audio played" "$failures"

echo "== VAL-MIC-008 hook WAV feed"
rm -f "$SESSIONS"/*.wav
speech_session feed feed
lead_bad=0
for h in "${SAMPLES_LEAD[@]}"; do [ "$h" = "$REST_HEIGHT.0" ] || [ "$h" = "$REST_HEIGHT" ] || lead_bad=$((lead_bad + 1)); done
distinct=$(printf '%s\n' "${SAMPLES_SPEECH[@]}" | awk -v r="$REST_HEIGHT" '$1 > r' | sort -u | wc -l)
read -r rate ch bits frames < <(wav_header "$SESSION_WAV")
text=$(transcript "$SESSION_WAV")
missing=$(missing_words "$text" the quick brown fox jumps over lazy dog)
failures=""
[ "$distinct" -ge 3 ] || failures+="only $distinct distinct heights above rest; "
[ "$lead_bad" = 0 ] || failures+="$lead_bad lead samples moved; "
[ "$rate $ch $bits" = "16000 1 16" ] || failures+="header is $rate Hz $ch ch $bits bit; "
[ -z "$missing" ] || failures+="transcript misses: $missing ($text); "
check_failures "VAL-MIC-008 hook feed" "meter moved ($distinct heights), 16000 Hz mono, transcript: $text" "$failures"

echo "== VAL-MIC-004 killed recording is recovered"
reset_app
python3 /harness/slot/pad-wav.py "$DICTATION" "$RUN_OUT/padded/dictation-45s.wav" 1000 >/dev/null
hook_action capture-start >/dev/null
wait_state listening 5
paplay --device=vmic "$RUN_OUT/padded/dictation-45s.wav" &
player=$!
sleep 11
pid=$(cat /tmp/app.pid)
kill -9 "$pid"
kill "$player" 2>/dev/null
wait "$player" 2>/dev/null
rm -f /run/hook.sock
left=$(newest_session)
ls -l "$SESSIONS" >"$RUN_OUT/004-before-restart.txt"
read -r rate ch bits frames < <(wav_header "$left" 2>&1)
before_secs=$(awk -v f="${frames:-0}" 'BEGIN {printf "%.2f", f / 16000}')
python3 /harness/slot/wavstat.py "$left" >>"$RUN_OUT/004-before-restart.txt" 2>&1
# a header-only file and a file 25 h old, for the sweep
python3 - "$SESSIONS" <<'PY'
import os, sys, time, wave
d = sys.argv[1]
for name, frames, age_h in (("header-only.wav", 0, 0), ("old.wav", 16000 * 3, 25)):
    path = os.path.join(d, name)
    with wave.open(path, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(16000)
        w.writeframes(b"\x01\x00" * frames)
    if age_h:
        t = time.time() - age_h * 3600
        os.utime(path, (t, t))
PY
start_app
ls -l "$SESSIONS" >"$RUN_OUT/004-after-restart.txt"
shot 004-after-restart
failures=""
[ "$(awk -v s="$before_secs" 'BEGIN {print (s >= 8)}')" = 1 ] || failures+="WAV left behind has $before_secs s in its header; "
[ -f "$left" ] || failures+="the killed recording is gone after the restart; "
[ ! -e "$SESSIONS/header-only.wav" ] || failures+="the header-only file is still there; "
[ ! -e "$SESSIONS/old.wav" ] || failures+="the 25 h old file is still there; "
read -r rate ch bits frames < <(wav_header "$left" 2>&1)
real_bytes=$(($(stat -c %s "$left") - 44))
header_bytes=$((frames * 2))
[ "$real_bytes" = "$header_bytes" ] || failures+="header says $header_bytes data bytes, file has $real_bytes; "
text=$(transcript "$left")
missing=$(missing_words "$text" good morning this long dictation test speech recognition application)
[ -z "$missing" ] || failures+="first sentence misses: $missing ($text); "
[ "$(cap '.recovered | length')" -ge 1 ] || failures+="state.capture.recovered is empty; "
check_failures "VAL-MIC-004 recovery" "killed after $before_secs s in the header; $((header_bytes / 32000)) s recovered with matching sizes; seeded header-only and 25 h files removed; transcript starts: ${text:0:90}" "$failures"

echo "== VAL-MIC-005 selected mic survives a restart"
/harness/slot/app.sh stop
find /data -mindepth 1 -delete
start_app
/harness/slot/app.sh stop
tmp=$(mktemp)
jq '.values["future.key"] = 42' /data/config/settings.json >"$tmp" && cat "$tmp" >/data/config/settings.json
rm -f "$tmp"
pactl load-module module-null-sink sink_name=vmic2 >/dev/null
pactl load-module module-virtual-source source_name=vmic2_src master=vmic2.monitor >/dev/null
start_app
wait_until 10 cap_is '.devices | map(select(.id | test("vmic2_src"))) | length' 1
index=$(cap '.devices | map(.id | test("vmic2_src")) | index(true) + 1')
id2=$(cap '.devices[] | select(.id | test("vmic2_src")) | .id')
# The device list sits below the dictation panel: scroll the page until the row is in the window.
read -r ox oy < <(settled_origin "$(app_window)")
xdotool mousemove $((ox + 220)) $((oy + 300))
for _ in $(seq 8); do xdotool click 5; sleep 0.1; done
$HC click "home.mic.device.$index" >/dev/null
wait_until 5 cap_is .saved_device "$id2"
/harness/slot/app.sh stop
cp /data/config/settings.json "$RUN_OUT/005-settings-after-select.json"
start_app
shot 005-picker-after-restart
saved=$(jq -r '.values["audio.inputDeviceId"]' /data/config/settings.json)
# the store keeps keys it does not know under values._unknown
future=$(jq -r '.values._unknown["future.key"] // .values["future.key"]' /data/config/settings.json)
rm -f "$SESSIONS"/*.wav
speech_session pa-vmic2 pa vmic2
silent_stat=$(python3 /harness/slot/wavstat.py "$SESSION_WAV")
failures=""
[ "$(cap .active_device)" = "$id2" ] || failures+="active device is $(cap .active_device), want $id2; "
[ "$saved" = "$id2" ] || failures+="settings audio.inputDeviceId is $saved; "
[ "$future" = 42 ] || failures+="future.key is $future; "
grep -q 'silent=false' <<<"$silent_stat" || failures+="session from vmic2_src is silent ($silent_stat); "
check_failures "VAL-MIC-005 selected mic" "$id2 selected after restart and saved, future.key=42 kept, vmic2 audio recorded ($silent_stat)" "$failures"

echo "== VAL-MIC-006 a missing selected mic falls back"
/harness/slot/app.sh stop
unload_modules vmic2
start_app
sleep 1
shot 006-notice
$HC tree >"$RUN_OUT/006-tree.json"
$HC state | jq .capture >"$RUN_OUT/006-capture-state.json"
notice=$(cap '.notice.message')
rm -f "$SESSIONS"/*.wav
speech_session pa-fallback pa vmic
stat=$(python3 /harness/slot/wavstat.py "$SESSION_WAV")
failures=""
[ "$(cap .active_device)" = default ] || failures+="active device is $(cap .active_device); "
case "$notice" in *"not available"* | *"isn't available"*) ;; *) failures+="notice is '$notice'; " ;; esac
grep -q 'silent=false' <<<"$stat" || failures+="session from the default mic is silent ($stat); "
check_failures "VAL-MIC-006 fallback" "notice '$notice', Default active, session recorded ($stat)" "$failures"

echo "== VAL-MIC-007 no mic and a mic removed while recording"
/harness/slot/app.sh stop
unload_modules "module-virtual-source"
unload_modules "module-null-sink"
sources=$(pactl list short sources | wc -l)
start_app
hook_action capture-start >"$RUN_OUT/007-start.txt" 2>&1
sleep 1
nomic_state=$(cap .state)
nomic_code=$(cap '.notice.code')
shot 007-no-mic
failures=""
[ "$sources" = 0 ] || failures+="pactl lists $sources sources; "
[ "$nomic_code" = MIC_UNAVAILABLE ] || failures+="no-mic notice code is $nomic_code; "
[ "$nomic_state" != listening ] || failures+="state is listening without a mic; "
load_mic_modules
sleep 2.5
leave_onboarding_repair || failures+="onboarding repair did not close after the mic came back; "
pid_before=$(cat /tmp/app.pid)
python3 /harness/slot/pad-wav.py "$SPEECH" "$RUN_OUT/padded/speech-short.wav" 1000 >/dev/null
hook_action capture-start >/dev/null
wait_state listening 5 || failures+="capture did not start after the mic came back; "
paplay --device=vmic "$RUN_OUT/padded/speech-short.wav" &
player=$!
sleep 3
removed_at=$(now_ms)
unload_modules "module-virtual-source"
wait_until 6 cap_not .state listening
stopped_in=$(($(now_ms) - removed_at))
kill "$player" 2>/dev/null
wait "$player" 2>/dev/null
state_after=$(cap .state)
code_after=$(cap '.notice.code')
shot 007-removed
pid_after=$(cat /tmp/app.pid)
kill -0 "$pid_after" 2>/dev/null || failures+="the app process is gone; "
[ "$pid_before" = "$pid_after" ] || failures+="pid changed $pid_before -> $pid_after; "
[ -n "$(app_window)" ] || failures+="the window is not mapped; "
[ "$stopped_in" -le 3000 ] || failures+="stopped after $stopped_in ms; "
[ "$state_after" != listening ] || failures+="still listening; "
[ "$code_after" = MIC_UNAVAILABLE ] || failures+="notice code after removal is $code_after; "
check_failures "VAL-MIC-007 no mic" "no sources: notice $nomic_code, state $nomic_state; removal: stopped in $stopped_in ms, state $state_after, notice $code_after, pid $pid_after kept, window mapped" "$failures"

# leave the slot as init.sh made it
unload_modules "module-virtual-source"
unload_modules "module-null-sink"
load_mic_modules
