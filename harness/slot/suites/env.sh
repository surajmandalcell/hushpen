#!/bin/bash
# Slot environment checks (UI only, no app needed): the tools run, lavapipe renders, the cue sink
# is separate from the virtual mic, a StatusNotifier host and the portal answer, play-wav pads
# with 1 s of silence, the paste targets receive text, and XI2 sees keycode 108 as press and
# release. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh

FIXTURE=${FIXTURE:-/assets/fixtures/speech-short.wav}

echo "== tools"
failures=""
for tool in "xdotool --version" "xclip -version" "import -version" "xinput list" "strace -V" \
  "ss -V" "jq --version" "xdpyinfo -display $DISPLAY" "pactl info" "paplay --version" \
  "vulkaninfo --summary" "wmctrl -m"; do
  $tool >/dev/null 2>&1 || failures+="'$tool' failed; "
done
check_failures "tools" "xdotool, xclip, import, xinput, strace, ss, jq, xdpyinfo, pactl, paplay, vulkaninfo, wmctrl run" "$failures"

echo "== lavapipe"
if vulkaninfo --summary 2>/dev/null | grep -qi llvmpipe; then
  check lavapipe pass "vulkaninfo names llvmpipe"
else
  check lavapipe fail "vulkaninfo does not name llvmpipe"
fi

echo "== audio routing"
failures=""
[ "$(pactl get-default-source)" = vmic_src ] || failures+="default source is $(pactl get-default-source); "
[ "$(pactl get-default-sink)" = cues ] || failures+="default sink is $(pactl get-default-sink); "
sinks=$(pactl list short sinks | awk '{print $2}' | tr '\n' ' ')
case " $sinks" in *" vmic "*) ;; *) failures+="no vmic sink ($sinks); " ;; esac
case " $sinks" in *" cues "*) ;; *) failures+="no cues sink ($sinks); " ;; esac
record() { # <source> <seconds> <wav>
  timeout -s INT "$2" parecord --device="$1" --rate=16000 --channels=1 --format=s16le \
    --file-format=wav "$3" 2>/dev/null
}
stat_of() { python3 /harness/slot/wavstat.py "$1"; }
silent() { stat_of "$1" | grep -q 'silent=true'; }

# A sound played to the default output must reach cues.monitor and leave the microphone silent.
record cues.monitor 8 "$RUN_OUT/cue-monitor.wav" &
cue_pid=$!
record vmic_src 8 "$RUN_OUT/cue-mic.wav" &
mic_pid=$!
sleep 0.7
paplay "$FIXTURE"
wait "$cue_pid" "$mic_pid"
echo "cue sound, cues.monitor: $(stat_of "$RUN_OUT/cue-monitor.wav")"
echo "cue sound, vmic_src:     $(stat_of "$RUN_OUT/cue-mic.wav")"
silent "$RUN_OUT/cue-monitor.wav" && failures+="a sound to the default output did not reach cues.monitor; "
silent "$RUN_OUT/cue-mic.wav" || failures+="a sound to the default output reached the virtual mic; "

# Audio played to vmic must reach the microphone and stay out of the cue sink.
record vmic_src 8 "$RUN_OUT/mic-capture.wav" &
mic_pid=$!
record cues.monitor 8 "$RUN_OUT/mic-cues.wav" &
cue_pid=$!
sleep 0.7
play_out=$(/harness/slot/play-wav.sh "$FIXTURE")
wait "$cue_pid" "$mic_pid"
echo "$play_out"
echo "play-wav, vmic_src:     $(stat_of "$RUN_OUT/mic-capture.wav")"
echo "play-wav, cues.monitor: $(stat_of "$RUN_OUT/mic-cues.wav")"
silent "$RUN_OUT/mic-capture.wav" && failures+="play-wav did not reach vmic_src; "
silent "$RUN_OUT/mic-cues.wav" || failures+="play-wav audio leaked into cues.monitor; "
check_failures "audio routing" "default source vmic_src, default sink cues (separate from vmic); a default-output sound reaches cues.monitor and leaves the mic silent; play-wav reaches the mic only" "$failures"

echo "== play-wav padding"
padded=$(sed -n 's/^padded=\([^ ]*\) .*/\1/p' <<<"$play_out")
lead=$(python3 /harness/slot/wavstat.py "$padded" --lead-ms 1000)
echo "$lead"
if grep -q 'lead_silent=true' <<<"$lead" && ! grep -q 'body_peak_dbfs=-inf' <<<"$lead"; then
  check "play-wav padding" pass "first 1000 ms at or below -60 dBFS, then the fixture audio: $(grep -o 'lead_peak_dbfs=[^ ]* body_peak_dbfs=[^ ]*' <<<"$lead")"
else
  check "play-wav padding" fail "$lead"
fi

echo "== StatusNotifier host and portal"
watcher=$(gdbus call --session --dest org.kde.StatusNotifierWatcher --object-path /StatusNotifierWatcher \
  --method org.freedesktop.DBus.Properties.Get org.kde.StatusNotifierWatcher IsStatusNotifierHostRegistered 2>&1)
echo "watcher: $watcher"
if grep -q 'true' <<<"$watcher"; then
  check "status notifier host" pass "org.kde.StatusNotifierWatcher answers with a registered host"
else
  check "status notifier host" fail "$watcher"
fi
portal=$(timeout 20 gdbus call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop --method org.freedesktop.DBus.Peer.Ping 2>&1)
if [ $? -eq 0 ]; then
  check "portal" pass "org.freedesktop.portal.Desktop answers"
else
  check "portal" fail "$portal"
fi

echo "== XI2: keycode 108"
xinput test-xi2 --root >"$RUN_OUT/xi2.log" 2>&1 &
xi_pid=$!
sleep 1
xdotool keydown 108
sleep 0.3
xdotool keyup 108
sleep 0.6
kill "$xi_pid" 2>/dev/null
wait "$xi_pid" 2>/dev/null
# One line per raw key event: "RawKeyPress 108".
events=$(awk '
  /EVENT type/ { kind = ($0 ~ /RawKeyPress/) ? "RawKeyPress" : (($0 ~ /RawKeyRelease/) ? "RawKeyRelease" : "") }
  /detail:/ && kind != "" { print kind, $2; kind = "" }' "$RUN_OUT/xi2.log")
echo "$events"
if [ "$events" = "$(printf 'RawKeyPress 108\nRawKeyRelease 108')" ]; then
  check "xi2 keycode 108" pass "one RawKeyPress and one RawKeyRelease for keycode 108, in order, none for 64"
else
  check "xi2 keycode 108" fail "events: $(tr '\n' ',' <<<"$events")"
fi
down=$(xinput query-state 'Virtual core XTEST keyboard' 2>/dev/null | grep -c '=down')
[ "$down" = 0 ] && check "keys released" pass "no key is still down on the XTEST keyboard" ||
  check "keys released" fail "$down keys still down"

echo "== paste targets"
failures=""
targets=$(/harness/slot/targets.sh --stock)
echo "$targets"
XT=$(sed -n 's/.*xterm=\([0-9]*\).*/\1/p' <<<"$targets")
GT=$(sed -n 's/.*gtk=\([0-9]*\).*/\1/p' <<<"$targets")
ST=$(sed -n 's/.*xterm_stock=\([0-9]*\).*/\1/p' <<<"$targets")
focus() { xdotool windowfocus "$1"; sleep 0.4; }
TEXT='hp-é-ß'

printf '%s' "$TEXT" | xclip -selection clipboard -i
focus "$GT"
xdotool key --clearmodifiers ctrl+v
sleep 0.3
xdotool key Return
sleep 0.5
[ "$(cat /out/gtk.txt 2>/dev/null)" = "$TEXT" ] || failures+="GTK Ctrl+V got [$(cat /out/gtk.txt 2>/dev/null)]; "

printf '%s' "$TEXT" | xclip -selection clipboard -i
focus "$XT"
xdotool key --clearmodifiers ctrl+shift+v
sleep 0.3
xdotool key Return
sleep 0.5
grep -qxF "$TEXT" /out/xterm.txt 2>/dev/null || failures+="xterm Ctrl+Shift+V (override) got [$(cat /out/xterm.txt 2>/dev/null)]; "

printf '%s' "$TEXT-primary" | xclip -selection primary -i
focus "$ST"
xdotool key --clearmodifiers shift+Insert
sleep 0.3
xdotool key Return
sleep 0.5
grep -qxF "$TEXT-primary" /out/xterm-stock.txt 2>/dev/null || failures+="stock xterm Shift+Insert got [$(cat /out/xterm-stock.txt 2>/dev/null)]; "
shot paste-after
down=$(xinput query-state 'Virtual core XTEST keyboard' 2>/dev/null | grep -c '=down')
[ "$down" = 0 ] || failures+="$down keys still down after the pastes; "
check_failures "paste targets" "$TEXT reaches the GTK entry (Ctrl+V) and the xterm override (Ctrl+Shift+V); stock xterm takes Shift+Insert from PRIMARY" "$failures"
pkill -x xterm 2>/dev/null
pkill -f target-gtk.py 2>/dev/null
