#!/bin/bash
# Runs inside a harness slot (through with-env.sh). Installs a release deb, starts it with no test
# hook, and checks the core loop: window 940x600, no hook socket, hookctl refused, a push-to-talk
# with keycode 108 puts the words into the override xterm, and the History database has one
# completed row. The slot's /tmp/env.sh still sets HUSHPEN_TESTHOOK_SOCKET, which proves that a
# release build ignores it.
#
# usage: release-deb-in-slot.sh <deb in /out/inputs>
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

deb=$1
HOLD=108
out=$RUN_OUT

echo "== install"
dpkg -i "$deb" >"$out/dpkg.log" 2>&1 || { cat "$out/dpkg.log"; exit 1; }
tail -3 "$out/dpkg.log"
[ "$(cat /usr/lib/hushpen/install-kind)" = deb ] || { echo "install-kind is not deb"; exit 1; }

echo "== data folder with tiny.en"
/harness/slot/app.sh stop
find /data -mindepth 1 -delete
mkdir -p /data/models/whisper /data/config
cp /assets/models/whisper/ggml-tiny.en.bin /data/models/whisper/
# The override xterm takes Ctrl+Shift+V; Return flushes the line cat is waiting for.
printf '%s\n' '{"schemaVersion": 1, "values": {"onboarding.completed": true, "dictation.modelId": "tiny.en", "insert.appChords": {"xterm": "ctrl+shift+v"}}}' \
  >/data/config/settings.json

echo "== start /usr/bin/hushpen (HUSHPEN_TESTHOOK_SOCKET=${HUSHPEN_TESTHOOK_SOCKET:-unset})"
/usr/bin/hushpen >/logs/app.log 2>&1 &
app_pid=$!
echo "$app_pid" >/tmp/app.pid
wid=""
for _ in $(seq 300); do
  wid=$(xdotool search --onlyvisible --class hushpen 2>/dev/null | head -1)
  [ -n "$wid" ] && break
  kill -0 "$app_pid" 2>/dev/null || { cat /logs/app.log; echo "the app exited early"; exit 1; }
  sleep 0.1
done
size=$(win_size "$wid")
xwininfo -id "$wid" | grep -E 'Width|Height'
[ "$size" = 940x600 ] || failures+="window is $size; "
sleep 3
shot release-deb-window

echo "== no hook socket"
find /data /run /tmp /root -xdev \( -type s -o -name 'hook*' -o -name '*testhook*' \) 2>/dev/null |
  grep -v -e '/\.X11-unix' -e '/dbus' -e '/pulse' -e '/\.ICE-unix' -e '/tmp/dbus' >"$out/sockets.txt"
cat "$out/sockets.txt"
[ ! -e /run/hook.sock ] || failures+="/run/hook.sock exists; "
grep -q hook "$out/sockets.txt" && failures+="a hook path exists; "
if "$HC" state >"$out/hookctl.txt" 2>&1; then failures+="hookctl connected; "; fi
echo "hookctl: $(head -c 200 "$out/hookctl.txt")"

echo "== push-to-talk into the xterm"
/harness/slot/targets.sh >"$out/targets.txt"
cat "$out/targets.txt"
xt=$(grep -o 'xterm=[0-9]*' "$out/targets.txt" | cut -d= -f2)
set_clip OLD
xdotool windowfocus --sync "$xt"
for _ in $(seq 100); do
  grep -q "tiny.en" /data/logs/engine.log 2>/dev/null && break
  sleep 0.3
done
sleep 5
xdotool keydown $HOLD
sleep 1.5
shot release-deb-listening
paplay --device=vmic "$(pad "$FIX/speech-short.wav")"
xdotool keyup $HOLD
db=/data/history/history.db
for _ in $(seq 240); do
  [ "$(sqlite3 "$db" "select count(*) from transcript where status = 'completed'" 2>/dev/null)" = 1 ] && break
  sleep 0.5
done
sleep 1
xdotool key Return
for _ in $(seq 20); do
  [ -s /out/xterm.txt ] && break
  sleep 0.5
done
shot release-deb-final
text=$(norm <"/out/xterm.txt")
echo "xterm.txt: $text"
cp /out/xterm.txt "$out/xterm.txt"
missed=$(word_recall_ok "$text" "${SHORT_WORDS[@]}") || failures+="xterm text misses: $missed; "

echo "== History row"
rows=$(sqlite3 "$db" "select count(*) from transcript where status = 'completed'" 2>&1)
echo "completed rows: $rows"
sqlite3 -header "$db" 'select id, status, final_text from transcript' | tee "$out/sqlite.txt"
[ "$rows" = 1 ] || failures+="History has $rows completed rows; "

/harness/slot/app.sh stop
[ -z "${failures:-}" ] && { echo "release deb ok"; exit 0; }
echo "FAILED: $failures"
exit 1
