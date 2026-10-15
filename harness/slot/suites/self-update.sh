#!/bin/bash
# Self-update in a slot: VAL-AUP-001 to 004, the deb and tar.gz halves of VAL-AUP-005, and
# VAL-CROSS-008. The apps are release builds made with `xtask package --test-feed` (rc.1 and rc.2
# AppImages, the rc.1 deb and tar.gz) from /update-test, which harness/update-fixtures.sh wrote
# and HUSHPEN_UPDATE_TEST mounts. A release build has no test hook, so the proof is hushpen.log,
# the files on disk, the feed log, xdotool, sqlite3, and screenshots; selfupdate-lib.sh has the
# helpers. The feed (update-feed-server.py) offers rc.2 with the real AppImage ("good") or with a
# wrong sha256 ("badsha"). Needs the tiny.en model in /assets; empties /data and /opt/hushpen-test,
# so the slot must not be shared. The rpm half of VAL-AUP-005 runs in fedora:40 through
# packaging/smoke/update-notice-in-container.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh
. /harness/slot/selfupdate-lib.sh

DB=/data/history/history.db
HOLD=108
trap 'stop_apps; feed_stop' EXIT

if [ ! -d /update-test/rc1 ] || [ ! -d /update-test/feed ]; then
  check "self-update fixtures" fail "/update-test is missing: run harness/update-fixtures.sh and set HUSHPEN_UPDATE_TEST to its output folder"
  finish self-update
  exit 0
fi

/harness/slot/app.sh stop
stub_xdg_open
rm -f "$FEED_LOG" /logs/app-installed.log "$FEED_SLOW"
feed_start good || echo "the feed did not start"
RC1_SHA=$(sha "$(rc1_appimage)")
RC2_SHA=$(sha "$(rc2_appimage)")
RC1_VERSION=$(version_of "$(rc1_appimage)")
RC2_VERSION=$(version_of "$(rc2_appimage)")
echo "rc.1: $RC1_VERSION $RC1_SHA"
echo "rc.2: $RC2_VERSION $RC2_SHA"

echo "== VAL-AUP-001 and VAL-CROSS-008: rc.1 updates itself to rc.2 and keeps the data"
failures=""
cross=""
reset_data
install_rc1
seed '{"updates.check": false, "audio.cueVolume": 0.25, "dictation.modelId": "tiny.en"}'
mkdir -p "$MODELS"
cp /assets/models/whisper/ggml-tiny.en.bin "$MODELS/"
# The first start makes the History database; it runs with the check off, then data is added.
launch "$INSTALLED" || failures+="rc.1 did not open a window; "
wait_until 30 test -s "$DB"
stop_apps
/harness/slot/seed-history.sh 3 >/dev/null
sqlite3 "$DB" "insert into dictionary_entry (phrase, heard_as, created_at) values ('Hushpen', 'hush pen', 1760000000000)"
rows_before=$(sqlite3 "$DB" 'select count(*) from transcript')
dict_before=$(sqlite3 "$DB" 'select count(*) from dictionary_entry')
jq '.values["updates.check"] = true | .values["updates.autoInstall"] = true' "$SETTINGS_FILE" >/tmp/settings.new
mv /tmp/settings.new "$SETTINGS_FILE"
cp "$SETTINGS_FILE" "$RUN_OUT/settings-before.json"
: >/logs/app-installed.log
rm -f "$HELPER_LOG" "$APP_LOG"
before_sha=$(sha "$INSTALLED")
before_version=$(version_of "$INSTALLED")
launch "$INSTALLED" || failures+="rc.1 did not open a window with the check on; "
wait_until 90 state_logged ready || failures+="the log never shows 'state ready'; "
state_lines >"$RUN_OUT/001-states.txt"
cat "$RUN_OUT/001-states.txt"
for want in checking available downloading ready; do
  grep -q "state $want" "$RUN_OUT/001-states.txt" || failures+="state $want is not in the log; "
done
sleep 5
shot 001-ready
[ "$(sha "$INSTALLED")" = "$before_sha" ] || failures+="the AppImage changed before Restart to update was pressed; "
[ "$(version_logged)" = "2.0.0-rc.1" ] || failures+="the running app reports '$(version_logged)' before the restart; "
[ -f "$STAGED" ] && [ "$(sha "$STAGED")" = "$RC2_SHA" ] || failures+="no staged rc.2 file with the right sha256 at $STAGED; "
ls -la "$INSTALL_DIR" "$INSTALL_DIR/.Hushpen.AppImage.update" >"$RUN_OUT/001-folder-before-restart.txt" 2>&1
[ "$(starts_logged)" = 1 ] || failures+="the app started $(starts_logged) times before the restart; "
echo "pressing Restart to update"
click_banner >"$RUN_OUT/001-click.txt" 2>&1 || failures+="no Restart to update button: $(cat "$RUN_OUT/001-click.txt"); "
wait_until 60 starts_are 2 || failures+="the app did not start again after the restart; "
wait_until 30 main_window_id || failures+="no window after the relaunch; "
sleep 5
shot 001-after-relaunch
state_lines >"$RUN_OUT/001-states-after.txt"
after_sha=$(sha "$INSTALLED")
after_version=$(version_of "$INSTALLED")
[ "$after_sha" = "$RC2_SHA" ] || failures+="the AppImage sha256 after is $after_sha, not rc.2's $RC2_SHA; "
[ "$after_version" = "$RC2_VERSION" ] || failures+="--version after prints '$after_version', not '$RC2_VERSION'; "
[ "$(version_logged)" = "2.0.0-rc.2" ] || failures+="the relaunched app reports '$(version_logged)'; "
[ -e "$INSTALL_DIR/.Hushpen.AppImage.update" ] && failures+="the staging folder is still there; "
[ -e "$INSTALLED.old" ] && failures+="$INSTALLED.old is still there; "
ls -la "$INSTALL_DIR" >"$RUN_OUT/001-folder-after.txt" 2>&1
cp "$HELPER_LOG" "$RUN_OUT/001-update-apply.log" 2>/dev/null
grep -q UPDATE_SWAP_DONE "$HELPER_LOG" || failures+="update-apply did not log UPDATE_SWAP_DONE; "
printf 'before: %s  %s\nafter:  %s  %s\n' "$before_version" "$before_sha" "$after_version" "$after_sha" >"$RUN_OUT/001-versions.txt"
check_failures "VAL-AUP-001 AppImage self-update" "check and automatic install on, no clicks: the log shows checking, available, downloading, ready; Restart to update was pressed, and only then did the AppImage change from '$before_version' ($before_sha) to '$after_version' ($after_sha); the app relaunched as 2.0.0-rc.2; screenshots 001-ready.png and 001-after-relaunch.png" "$failures"

echo "== VAL-CROSS-008 the data and the core loop after the update"
size=$(win_size "$(main_window_id)" 2>/dev/null)
rows_after=$(sqlite3 "$DB" 'select count(*) from transcript')
dict_after=$(sqlite3 "$DB" 'select count(*) from dictionary_entry')
[ "$rows_before" = 3 ] && [ "$rows_after" = 3 ] || cross+="History rows $rows_before before, $rows_after after (want 3 and 3); "
[ "$dict_before" = 1 ] && [ "$dict_after" = 1 ] || cross+="dictionary entries $dict_before before, $dict_after after (want 1 and 1); "
cp "$SETTINGS_FILE" "$RUN_OUT/settings-after.json"
# updates.lastCheckAt is the app's own record of the check, not a setting the person changed.
diff <(jq -S 'del(.values["updates.lastCheckAt"])' "$RUN_OUT/settings-before.json") \
  <(jq -S 'del(.values["updates.lastCheckAt"])' "$RUN_OUT/settings-after.json") >"$RUN_OUT/settings.diff" ||
  cross+="settings.json changed across the update: $(tr '\n' ' ' <"$RUN_OUT/settings.diff"); "
[ "$(jq -r '.values["audio.cueVolume"]' "$SETTINGS_FILE")" = 0.25 ] || cross+="audio.cueVolume is $(jq -r '.values["audio.cueVolume"]' "$SETTINGS_FILE"); "
[ "$(jq -r '.values["onboarding.completed"]' "$SETTINGS_FILE")" = true ] || cross+="onboarding.completed is not true; "
[ "$size" = 940x600 ] || cross+="the window after the relaunch is '$size'; "
sqlite3 "$DB" "select 'transcript', count(*) from transcript union all select 'dictionary_entry', count(*) from dictionary_entry" >"$RUN_OUT/cross-008-counts.txt"
echo "push-to-talk into the GTK entry"
/harness/slot/targets.sh >"$RUN_OUT/targets.txt"
GT=$(grep -o 'gtk=[0-9]*' "$RUN_OUT/targets.txt" | cut -d= -f2)
set_clip OLD
xdotool windowfocus --sync "$GT"
for _ in $(seq 100); do
  grep -q 'tiny.en' "$ENGINE_LOG" 2>/dev/null && break
  sleep 0.3
done
sleep 5
xdotool keydown $HOLD
sleep 1.5
paplay --device=vmic "$(pad "$FIX/speech-short.wav")"
xdotool keyup $HOLD
for _ in $(seq 240); do
  [ "$(sqlite3 "$DB" "select count(*) from transcript where status = 'completed'" 2>/dev/null)" = 4 ] && break
  sleep 0.5
done
sleep 1
xdotool windowfocus --sync "$GT"
rm -f /out/gtk.txt
xdotool key Return
wait_until 5 test -e /out/gtk.txt
cp /out/gtk.txt "$RUN_OUT/gtk.txt" 2>/dev/null
text=$(norm <"$RUN_OUT/gtk.txt" 2>/dev/null)
echo "gtk.txt: $text"
missed=$(word_recall_ok "$text" "${SHORT_WORDS[@]}") || cross+="the GTK entry misses: $missed; "
shot cross-008-after
check_failures "VAL-CROSS-008 data and core loop" "rc.1 to rc.2 with 3 History rows, 1 dictionary entry, audio.cueVolume 0.25: after the relaunch 3 rows (4 after the dictation), 1 entry, settings.json identical, onboarding done, window 940x600, and push-to-talk put the speech into the GTK entry (gtk.txt: $text)" "$cross"
pkill -x xterm 2>/dev/null
pkill -f target-gtk.py 2>/dev/null

echo "== VAL-AUP-002 a sha256 mismatch keeps the old AppImage"
failures=""
reset_data
install_rc1
seed '{"updates.check": true, "updates.autoInstall": true}'
feed_mode badsha
rm -f "$HELPER_LOG"
before_sha=$(sha "$INSTALLED")
launch "$INSTALLED" || failures+="rc.1 did not open a window; "
wait_until 90 state_logged error || failures+="the log never shows an error state; "
sleep 3
state_lines >"$RUN_OUT/002-states.txt"
cat "$RUN_OUT/002-states.txt"
grep -q 'state error UPDATE_HASH_MISMATCH' "$RUN_OUT/002-states.txt" || failures+="the error is not UPDATE_HASH_MISMATCH; "
grep -q 'state ready' "$RUN_OUT/002-states.txt" && failures+="the state reached ready; "
shot 002-after-mismatch
ls -la "$INSTALL_DIR" "$INSTALL_DIR/.Hushpen.AppImage.update" >"$RUN_OUT/002-folder.txt" 2>&1
[ "$(sha "$INSTALLED")" = "$before_sha" ] && [ "$before_sha" = "$RC1_SHA" ] || failures+="the AppImage sha256 is $(sha "$INSTALLED"), not rc.1's $RC1_SHA; "
[ "$(version_of "$INSTALLED")" = "$RC1_VERSION" ] || failures+="--version prints '$(version_of "$INSTALLED")'; "
[ "$(starts_logged)" = 1 ] && [ "$(version_logged)" = "2.0.0-rc.1" ] || failures+="the app did not keep running as rc.1; "
[ -e "$STAGED" ] && failures+="a staged file is left at $STAGED; "
stop_apps
check_failures "VAL-AUP-002 sha256 mismatch" "the feed lists the rc.2 AppImage with a sha256 it does not have: the log shows 'state error UPDATE_HASH_MISMATCH', the AppImage keeps sha256 $RC1_SHA and prints '$RC1_VERSION', and no staged file is left" "$failures"
feed_mode good

echo "== VAL-AUP-003 an interrupted swap leaves the old AppImage runnable"
failures=""
reset_data
install_rc1
seed '{"updates.check": true, "updates.autoInstall": true}'
rm -f "$HELPER_LOG"
launch "$INSTALLED" HUSHPEN_UPDATE_SWAP_PAUSE_MS=120000 || failures+="rc.1 did not open a window; "
wait_until 90 state_logged ready || failures+="the log never shows 'state ready'; "
sleep 2
click_banner >/dev/null 2>&1 || failures+="no Restart to update button; "
wait_until 30 test -e "$INSTALLED.old" || failures+="the helper never reached the swap step; "
pid=$(pgrep -f 'hushpen update-apply' | head -1)
{
  echo "helper before the kill:"
  ps -o pid,args -p "${pid:-0}" 2>&1
  echo "kill -9 $pid"
  kill -9 "$pid" 2>&1
  sleep 1
  echo "helper after the kill:"
  ps -o pid,args -p "${pid:-0}" 2>&1 || echo "gone"
} >"$RUN_OUT/003-kill.log" 2>&1
cat "$RUN_OUT/003-kill.log"
[ -n "$pid" ] || failures+="no update-apply process to kill; "
ls -la "$INSTALL_DIR" "$INSTALL_DIR/.Hushpen.AppImage.update" >"$RUN_OUT/003-folder.txt" 2>&1
cat "$RUN_OUT/003-folder.txt"
[ "$(sha "$INSTALLED")" = "$RC1_SHA" ] || failures+="after the kill the AppImage path holds sha256 $(sha "$INSTALLED"), not rc.1's; "
[ "$(version_of "$INSTALLED")" = "$RC1_VERSION" ] || failures+="--version after the kill prints '$(version_of "$INSTALLED")'; "
partial=$(find "$INSTALL_DIR" -maxdepth 1 -type f -newer "$STAGED" ! -name Hushpen.AppImage ! -name Hushpen.AppImage.old 2>/dev/null)
[ -z "$partial" ] || failures+="a stray file is in the folder: $partial; "
pgrep -x hushpen >/dev/null && failures+="an app process is still running; "
: >/logs/app-installed.log
rm -f "$APP_LOG"
launch "$INSTALLED" || failures+="the AppImage after the kill did not open a window; "
sleep 3
wid=$(main_window_id)
xwininfo -id "$wid" 2>&1 | grep -E 'Width|Height' >"$RUN_OUT/003-xwininfo.txt"
cat "$RUN_OUT/003-xwininfo.txt"
[ "$(win_size "$wid")" = 940x600 ] || failures+="the window after the kill is '$(win_size "$wid")'; "
[ "$(version_logged)" = "2.0.0-rc.1" ] || failures+="the app after the kill reports '$(version_logged)'; "
shot 003-after-kill
stop_apps
check_failures "VAL-AUP-003 SIGKILL during the swap" "update-apply was killed with SIGKILL at the swap step (pid $pid): the AppImage path still holds rc.1 (sha256 $RC1_SHA, --version '$RC1_VERSION'), no stray file, and the next start maps a 940x600 window as 2.0.0-rc.1" "$failures"

echo "== VAL-AUP-003 a failed swap without a kill"
failures=""
reset_data
install_rc1
seed '{"updates.check": true, "updates.autoInstall": true}'
rm -f "$HELPER_LOG"
launch "$INSTALLED" HUSHPEN_UPDATE_SWAP_PAUSE_MS=10000 || failures+="rc.1 did not open a window; "
wait_until 90 state_logged ready || failures+="the log never shows 'state ready'; "
sleep 2
click_banner >/dev/null 2>&1 || failures+="no Restart to update button; "
wait_until 30 test -e "$INSTALLED.old" || failures+="the helper never reached the swap step; "
rm -f "$STAGED"
echo "the staged file is gone while the swap is held open"
wait_until 40 grep -q UPDATE_SWAP_FAILED "$HELPER_LOG" || failures+="update-apply never logged UPDATE_SWAP_FAILED; "
wait_until 30 starts_are 2 || failures+="the old version was not started again; "
cp "$HELPER_LOG" "$RUN_OUT/003-update-apply.log" 2>/dev/null
cat "$HELPER_LOG"
ls -la "$INSTALL_DIR" >"$RUN_OUT/003-failed-folder.txt" 2>&1
[ -e "$INSTALLED.old" ] && failures+="$INSTALLED.old is still there; "
[ "$(sha "$INSTALLED")" = "$RC1_SHA" ] || failures+="the AppImage sha256 is $(sha "$INSTALLED") after the failed swap; "
[ "$(version_logged)" = "2.0.0-rc.1" ] || failures+="the app started again as '$(version_logged)'; "
shot 003-after-failed-swap
stop_apps
check_failures "VAL-AUP-003 failed swap" "the staged file vanished while the swap was held open: update-apply logged UPDATE_SWAP_FAILED, no .old is left, the AppImage keeps rc.1 (sha256 $RC1_SHA), and rc.1 started again" "$failures"

echo "== VAL-AUP-004 with Install updates automatically off nothing downloads until asked"
failures=""
reset_data
install_rc1
seed '{"updates.check": true, "updates.autoInstall": false}'
touch "$FEED_SLOW"
from=$(feed_count)
launch "$INSTALLED" || failures+="rc.1 did not open a window; "
wait_until 60 state_logged available || failures+="the log never shows 'state available'; "
sleep 2
shot 004-banner
echo "waiting 60 s with the offer on screen"
sleep 60
artifacts=$(artifact_requests "$from")
[ "$artifacts" = 0 ] || failures+="$artifacts artifact request(s) in 60 s before the button; "
state_logged downloading && failures+="the state went to downloading by itself; "
[ -e "$INSTALL_DIR/.Hushpen.AppImage.update" ] && failures+="a staging folder exists before the button; "
shot 004-banner-after-60s
mark=$(feed_count)
echo "pressing Download and install"
click_banner >"$RUN_OUT/004-click.txt" 2>&1 || failures+="no Download and install button: $(cat "$RUN_OUT/004-click.txt"); "
wait_until 20 artifacts_since "$mark" || failures+="no artifact request after the button; "
wait_until 20 state_logged downloading || failures+="the log never shows 'state downloading'; "
sleep 1
shot 004-downloading
wait_until 90 state_logged ready || failures+="the log never shows 'state ready'; "
sleep 1
shot 004-ready
tail -n +$((from + 1)) "$FEED_LOG" | jq -c '[.t_ms, .path]' >"$RUN_OUT/004-feed.txt"
state_lines >"$RUN_OUT/004-states.txt"
cat "$RUN_OUT/004-states.txt" "$RUN_OUT/004-feed.txt"
[ "$(sha "$INSTALLED")" = "$RC1_SHA" ] || failures+="the AppImage changed before the restart; "
rm -f "$FEED_SLOW"
stop_apps
check_failures "VAL-AUP-004 manual download" "autoInstall false: the offer showed Download and install, the feed log has no artifact request in 60 s, the button started the artifact request, and the log shows downloading then ready (feed log 004-feed.txt)" "$failures"

echo "== VAL-AUP-005 deb and tar.gz installs show a notice only"
dpkg -i "$(ls /update-test/rc1/Hushpen-*.deb | head -1)" >"$RUN_OUT/dpkg.log" 2>&1 || { tail -5 "$RUN_OUT/dpkg.log"; echo "dpkg failed"; }
notice_check deb deb /usr/bin/hushpen /usr/lib/hushpen/hushpen
tar_dir=/opt/hushpen-tar
find "$tar_dir" -mindepth 1 -delete 2>/dev/null
mkdir -p "$tar_dir"
tar -xzf "$(ls /update-test/rc1/Hushpen-*.tar.gz | head -1)" -C "$tar_dir"
tar_bin=$(ls "$tar_dir"/*/hushpen | head -1)
notice_check tar tar "$tar_bin" "$tar_bin"

finish self-update
