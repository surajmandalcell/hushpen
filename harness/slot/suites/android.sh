#!/bin/bash
# Run remote, then stop the paired Android host externally and run fallback in the same slot.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh
set +x
umask 077

phase=${HUSHPEN_ANDROID_PHASE:-remote}
case "$phase" in remote | fallback) ;; *) check "Android setup" fail "HUSHPEN_ANDROID_PHASE must be remote or fallback"; exit 0 ;; esac
trap 'rm -rf "$RUN_OUT/padded"' EXIT
setup_failed() { check "Android $phase setup" fail "$1"; exit 0; }
device_action() { hook_action devices-action "$1" >/dev/null; }
setting() { hook_action set-setting "$1" >/dev/null; }
state_is() { [ "$($HC state | jq -r "$1")" = "$2" ]; }

gtk_clear() {
  xdotool windowfocus --sync "$GT"
  xdotool key ctrl+a BackSpace
}

gtk_commit() {
  rm -f /out/gtk.txt
  xdotool windowfocus --sync "$GT"
  xdotool key Return
  wait_until 5 test -e /out/gtk.txt || return 1
  cat /out/gtk.txt
}

[ -f "$FIX/speech-short.wav" ] || setup_failed "the speech WAV fixture is missing"
state=$($HC state) || setup_failed "the existing debug desktop did not answer"
phone=$(jq -cer --arg id "${HUSHPEN_ANDROID_DEVICE_ID:-}" '
  [.devices.devices[] | select(.os == "android" and ($id == "" or .id == $id))]
  | select(length == 1) | first' <<<"$state") || setup_failed "pair exactly one Android phone or set HUSHPEN_ANDROID_DEVICE_ID"
id=$(jq -r .id <<<"$phone")
name=$(jq -r .name <<<"$phone")
wait_until 60 model_ready base-q5_1 || setup_failed "stage the verified Base Q5 model in the desktop data before pairing"
setting '{"key":"cleanup.llm.enabled","value":false}' || setup_failed "AI cleanup could not be disabled"

if [ "$phase" = remote ]; then
  device_action '{"op":"enabled","value":true}' || setup_failed "remote use could not be enabled"
  device_action '{"op":"noLocal","value":true}' || setup_failed "local models could not be disabled"
  hook_action model-select '{"id":"base-q5_1"}' >/dev/null || setup_failed "Base Q5 could not be selected"
  routes=$(jq -c '.devices.routes' <<<"$state")
  args=$(jq -nc --arg id "$id" --argjson routes "$routes" \
    '{key:"remote.routes",value:{speech:[$id],cleanup:$routes.cleanup}}')
  setting "$args" || setup_failed "the Android speech route could not be saved"
  wait_until 10 engine_is .model null || setup_failed "the local speech model stayed loaded"
  route=$(jq -nc --arg id "$id" '[$id]')
  wait_until 5 state_is '.devices.routes.speech | tojson' "$route" || setup_failed "the speech route did not select only the Android phone"
  check "Android speech route" pass "the paired Android phone is the only speech route and local speech is unloaded"
else
  state_is '.devices.enabled' true || setup_failed "remote use is not enabled from the remote stage"
  state_is '.devices.noLocalModels' true || setup_failed "the remote stage did not leave local models disabled"
  device_action '{"op":"noLocal","value":false}' || setup_failed "local fallback could not be enabled"
  wait_until 90 engine_ready_with base-q5_1 || setup_failed "the local fallback model did not load"
fi

ids=$(/harness/slot/targets.sh) || setup_failed "the GTK paste target did not open"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
[ -n "$GT" ] || setup_failed "the GTK paste target has no window"
gtk_clear
failures=""
hold_wav "$FIX/speech-short.wav" 450 || failures+="dictation did not finish; "
text=$(dict .transcript)
missing=$(word_recall_ok "$(norm <<<"$text")" "${SHORT_WORDS[@]}") || failures+="the transcript misses words: $missing; "
[ "$(dict .state)" = done ] || failures+="dictation ended $(dict .state); "
got=$(gtk_commit)
[ -n "$got" ] && [ "$got" = "$text" ] || failures+="the GTK entry did not receive the transcript; "

if [ "$phase" = remote ]; then
  engine_is .model null || failures+="a local speech model loaded during the Android job; "
  state_is '.pipeline.remoteNotice' null || failures+="remote speech used a fallback notice; "
  detail="the paired Android emulator returned speech over Noise and the desktop inserted it without a local speech model"
else
  engine_ready_with base-q5_1 || failures+="the local fallback model is not ready; "
  state_is '.pipeline.remoteNotice' "Using this computer. $name did not answer." || failures+="the core fallback notice was missing or wrong; "
  detail="local speech returned text after Android host loss and showed the core fallback notice"
fi
$HC state | jq '{devices,dictation,pipeline,engine,last_insert}' >"$RUN_OUT/android-$phase.json"
shot "android-$phase"
check_failures "Android $phase speech" "$detail" "$failures"
