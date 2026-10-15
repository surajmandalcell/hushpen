#!/bin/bash
# The Settings sections and the About view: VAL-SET-001 to 009 and VAL-LIC-004. Sections and controls are
# clicked through their hook ids, like a user; the keyboard reachability checks use real Tabs
# from xdotool. Launch at login is read from $XDG_CONFIG_HOME/autostart, which the suite points
# at a scratch folder before the first start. The Change data folder target is queued with
# `hookctl paths`; the move restarts the app by itself and `app.sh adopt` points the pidfile at
# the new process. Needs speech-short.wav in /assets/fixtures, filler-dictation.wav in
# /fixtures, and the base model in /assets/models/whisper. Busy: it transcribes and restarts
# the app several times, so the slot must not be shared. Runs in one slot through with-env.sh.
#
# Trap: wait_until runs its command once per poll, so a `test "$(...)" = x` argument freezes the
# value at the first poll. Always pass a predicate function (view_is, setting_is, ...).
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
SHORT_WAV=$FIX/speech-short.wav
FILLER_WAV=/fixtures/filler-dictation.wav
TM="python3 /harness/slot/tray-menu.py"
# Launch at login writes into this folder, never the real one.
export XDG_CONFIG_HOME=/tmp/hp-xdg-config
AUTOSTART=$XDG_CONFIG_HOME/autostart/hushpen.desktop
SECTION_KEYS="general shortcuts audio cleanup flowbar storage updates"

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() {
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
focus() { xdotool windowfocus --sync "$1"; }
speech() { paplay --device=vmic "$(pad "$1")"; }
sget() { $HC state | jq -r ".settings[\"$1\"]"; }
sv() { $HC state | jq -r ".settings_view$1"; }
about() { $HC state | jq -r ".about$1"; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
tree_get() { $HC tree | jq -r --arg id "$1" ".[] | select(.id == \$id) | .$2" | head -1; }
# Predicate forms for wait_until (it polls the command, so the read must happen inside).
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
section_is() { [ "$(sv .section)" = "$1" ]; }
setting_is() { [ "$(sget "$1")" = "$2" ]; }
sv_is() { [ "$(sv "$1")" = "$2" ]; }
bar_visible() { [ "$($HC state | jq -r .overlay.visible)" = true ]; }
bar_hidden() { [ "$($HC state | jq -r .overlay.visible)" = false ]; }
move_state_is() { [ "$(sv .move.state)" = "$1" ]; }
dead() { ! kill -0 "$1" 2>/dev/null; }
items() { python3 /harness/slot/sni-host.py --items; }

main_wid() {
  local wid
  for wid in $(xdotool search --class hushpen 2>/dev/null); do
    [ "$(win_size "$wid")" = 940x600 ] && { echo "$wid"; return 0; }
  done
  return 1
}
map_state() { xwininfo -id "$1" 2>/dev/null | awk '/Map State/ {print $3}'; }
is_mapped() { local wid; wid=$(main_wid) && [ "$(map_state "$wid")" = IsViewable ]; }

ptt_run() { # <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  speech "$1"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}
gtk_commit() { # Enter writes the entry to /out/gtk.txt; prints it
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

open_view() { # <view key>
  $HC action open-view "{\"view\":\"$1\"}" >/dev/null
  wait_until 10 view_is "$1"
}
section() { # <section key>: the Settings view opened on the section, with its tree rendered
  open_view settings || return 1
  if ! section_is "$1"; then
    wait_until 10 tree_has "settings.section.$1" || return 1
    $HC click "settings.section.$1" >/dev/null
    wait_until 5 section_is "$1" || return 1
  fi
  wait_until 10 tree_has "settings.$1" || return 1
}
tab_to() { # <id>: Tab until that element has focus (7 tabs plus the controls of one section)
  local id=$1
  for _ in $(seq 20); do
    [ "$(tree_get "$id" focused)" = true ] && return 0
    xdotool key Tab
    sleep 0.25
  done
  [ "$(tree_get "$id" focused)" = true ]
}
first_control() { # <section key> -> the tree id of its first control
  case "$1" in
    general) echo settings.general.launchAtLogin ;;
    shortcuts) echo settings.shortcut.hold ;;
    audio) echo settings.audio.mic ;;
    cleanup) echo settings.cleanup.rules ;;
    flowbar) echo settings.flowbar.enabled ;;
    storage) echo settings.storage.changeFolder ;;
  esac
}

echo "== setup: base model, the paste targets, a scratch autostart folder"
rm -rf /tmp/hp-xdg-config
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-SET-001 the 7 sections in order, each reachable by keyboard, Esc closes a dropdown"
failures=""
section general || failures+="Settings did not open on General; "
# The hook tree is sorted by id, so the tab order is read from the x bounds, left to right.
section_ids=$($HC tree | jq -r \
  '[.[] | select(.id | startswith("settings.section."))] | sort_by(.bounds.y, .bounds.x) | .[].id | ltrimstr("settings.section.")' \
  | paste -sd' ' -)
section_rows=$($HC tree | jq -r \
  '[.[] | select(.id | startswith("settings.section.")) | .bounds.y] | unique | length')
[ "$section_ids" = "$SECTION_KEYS" ] || failures+="the section tabs in reading order read '$section_ids'; "
[ "$section_rows" = 1 ] || failures+="the tabs wrap onto $section_rows rows; "
$HC tree >"$RUN_OUT/001-tree.json"
for key in $SECTION_KEYS; do
  section "$key" || { failures+="section $key did not open; "; continue; }
  shot "001-section-$key"
  failures+=$(chrome_problems "settings $key")
done
# Keyboard only: from the General section, Tab to each section tab and open it with Enter.
section general >/dev/null
focus "$(app_window)"
xdotool key Escape # no picker open: a no-op that settles the focus
for key in $SECTION_KEYS; do
  tab_to "settings.section.$key" || { failures+="Tab never reached the $key tab; "; continue; }
  xdotool key Return
  sleep 0.3
  section_is "$key" || failures+="Enter on the $key tab opened $(sv .section); "
  first=$(first_control "$key")
  if [ -n "$first" ]; then
    tab_to "$first" || failures+="$key: Tab never reached $first; "
  fi
done
# Esc in the open retention dropdown closes it and focuses the trigger again.
section storage >/dev/null
$HC click settings.storage.retention >/dev/null
wait_until 5 tree_has settings.storage.retention.option.never || failures+="the retention dropdown did not open; "
focus "$(app_window)"
xdotool key Escape
sleep 0.3
tree_has settings.storage.retention.option.never && failures+="Esc left the dropdown open; "
[ "$(tree_get settings.storage.retention focused)" = true ] || failures+="after Esc the trigger is not focused; "
check_failures "VAL-SET-001 sections and keyboard" "7 sections left to right: $section_ids; each reached by Tab and Enter, first controls take focus, Esc closes the retention dropdown onto its trigger" "$failures"

echo "== VAL-SET-002 launch at login writes and removes the XDG autostart file"
failures=""
section general || failures+="no General section; "
$HC click settings.general.launchAtLogin >/dev/null
wait_until 5 test -f "$AUTOSTART" || failures+="no autostart file at $AUTOSTART; "
if [ -f "$AUTOSTART" ]; then
  cat "$AUTOSTART"
  grep -q '^\[Desktop Entry\]' "$AUTOSTART" || failures+="no [Desktop Entry] header; "
  grep -q '^Type=Application' "$AUTOSTART" || failures+="no Type=Application; "
  exec_line=$(grep '^Exec=' "$AUTOSTART")
  case "$exec_line" in *" --hidden"*) ;; *) failures+="Exec lacks --hidden: $exec_line; " ;; esac
  exec_bin=$(sed -n 's/^Exec=\([^ ]*\).*/\1/p' "$AUTOSTART")
  running=$(readlink "/proc/$(ui_pid)/exe")
  [ "$exec_bin" = "$running" ] || failures+="Exec points at '$exec_bin', the app runs from '$running'; "
fi
setting_is startup.launchAtLogin true || failures+="startup.launchAtLogin is $(sget startup.launchAtLogin); "
$HC click settings.general.launchAtLogin >/dev/null
wait_until 5 test ! -e "$AUTOSTART" || failures+="the file is still there after the toggle off; "
setting_is startup.launchAtLogin false || failures+="startup.launchAtLogin is $(sget startup.launchAtLogin) after off; "
# On again, the file removed by hand, Settings reopened: the toggle reads off.
$HC click settings.general.launchAtLogin >/dev/null
wait_until 5 test -f "$AUTOSTART" || failures+="no autostart file after the second on; "
rm -f "$AUTOSTART"
open_view home >/dev/null
open_view settings >/dev/null
wait_until 5 sv_is .launch_at_login false || failures+="after the file was removed by hand the toggle still reads on; "
setting_is startup.launchAtLogin false || failures+="the key is $(sget startup.launchAtLogin) after the hand removal; "
$HC click settings.general.launchAtLogin >/dev/null
wait_until 5 test -f "$AUTOSTART" || failures+="no autostart file after the restore; "
check_failures "VAL-SET-002 launch at login" "on wrote $AUTOSTART (Exec $(sed -n 's/^Exec=//p' "$AUTOSTART" 2>/dev/null)), off removed it, a hand removal resynced the toggle to off" "$failures"

echo "== VAL-SET-003 start hidden starts with the tray only"
failures=""
section general >/dev/null
setting_is startup.startHidden false || failures+="startHidden was already on; "
$HC click settings.general.startHidden >/dev/null
wait_until 5 setting_is startup.startHidden true || failures+="startHidden did not turn on; "
APP_NO_WINDOW=1 /harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle || failures+="the app did not come back on the hook; "
sleep 1
is_mapped && failures+="the main window is mapped after a hidden start; "
shot 003-hidden
[ "$(items | wc -l)" = 1 ] || failures+="the watcher lists $(items | wc -l) items; "
wait_until 10 bar_visible || failures+="the idle bar does not show; "
gtk_clear
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the push-to-talk run did not end: $(pevents); "
got=$(gtk_commit)
case " $(norm <<<"$got") " in *" quick "*) ;; *) failures+="the entry holds '$got'; " ;; esac
tray_click() { $TM click "$1" >>"$RUN_OUT/tray-clicks.log" 2>&1; }
tray_click "Show Hushpen" || failures+="no Show Hushpen entry; "
wait_until 10 is_mapped || failures+="Show Hushpen did not map the window; "
shot 003-shown
# Back to a normal start for the rest of the suite.
section general >/dev/null
$HC click settings.general.startHidden >/dev/null
wait_until 5 setting_is startup.startHidden false || failures+="startHidden did not turn off; "
/harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle
wait_until 10 is_mapped || failures+="the window is not mapped after the normal restart; "
check_failures "VAL-SET-003 start hidden" "no window mapped, tray item on D-Bus, idle bar shown, push-to-talk typed '$got', Show Hushpen mapped the window" "$failures"

echo "== VAL-SET-004 the five controls write their keys and take effect"
failures=""
section cleanup >/dev/null && $HC click settings.cleanup.rules >/dev/null
section audio >/dev/null && $HC click settings.audio.sounds >/dev/null
section flowbar >/dev/null && { $HC click settings.flowbar.enabled >/dev/null; $HC click settings.flowbar.idle >/dev/null; }
section storage >/dev/null && {
  $HC click settings.storage.retention >/dev/null
  wait_until 5 tree_has settings.storage.retention.option.never
  $HC click settings.storage.retention.option.never >/dev/null
}
for pair in "cleanup.rules:false" "audio.cueSounds:false" "overlay.enabled:false" "overlay.idleVisible:false" "history.audioRetention:never"; do
  key=${pair%%:*}
  want=${pair##*:}
  setting_is "$key" "$want" || failures+="$key is $(sget "$key"), want $want; "
done
shot 004-controls
/harness/slot/app.sh restart >/dev/null
wait_until 20 dict_is .state idle
for pair in "cleanup.rules:false" "audio.cueSounds:false" "overlay.enabled:false" "overlay.idleVisible:false" "history.audioRetention:never"; do
  key=${pair%%:*}
  want=${pair##*:}
  setting_is "$key" "$want" || failures+="after the restart $key is $(sget "$key"), want $want; "
done
# Rule cleanup off: the filler dictation reaches the entry raw.
gtk_clear
ptt_run "$FILLER_WAV" || failures+="the filler run did not end; "
got=$(gtk_commit)
echo "  filler run: $got"
grep -Eiwq 'um+|uh+|er+m?' <<<"$got" || failures+="no filler words in '$got' with cleanup off; "
# The flow bar off: no bar during a full dictation.
gtk_clear
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="no listening state with the bar off; "
bar_visible && failures+="the bar shows with overlay.enabled off; "
sleep 1
bar_visible && failures+="the bar shows while listening with overlay.enabled off; "
speech "$SHORT_WAV"
xdotool keyup $HOLD
wait_until 240 run_ended || failures+="the bar-off run did not end; "
bar_visible && failures+="the bar shows after the run with overlay.enabled off; "
gtk_commit >/dev/null
# Only "show when idle" off: the bar maps during listening and unmaps after the result.
section flowbar >/dev/null && $HC click settings.flowbar.enabled >/dev/null
wait_until 5 setting_is overlay.enabled true || failures+="overlay.enabled did not turn back on; "
gtk_clear
T0=$(now_ms)
xdotool keydown $HOLD
wait_until 5 pstate_is listening || failures+="no listening state for the idle-off run; "
wait_until 5 bar_visible || failures+="the bar does not show while listening; "
speech "$SHORT_WAV"
xdotool keyup $HOLD
wait_until 240 run_ended || failures+="the idle-off run did not end; "
wait_until 10 bar_hidden || failures+="the bar stays mapped after the result with idleVisible off; "
gtk_commit >/dev/null
check_failures "VAL-SET-004 controls take effect" "the 5 keys written and kept across a restart; the filler dictation inserted raw ('$got'); no bar with overlay.enabled off; bar only while listening with idleVisible off" "$failures"

echo "== VAL-SET-005 the data folder shows as ~/... under home and in full outside it"
failures=""
/harness/slot/app.sh stop >/dev/null
rm -rf "$HOME/hp-data"
APP_DATA_DIR=$HOME/hp-data /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle || failures+="the app did not start on ~/hp-data; "
section storage >/dev/null
shown=$(tree_get settings.storage.dataFolder text)
[ "$shown" = "~/hp-data" ] || failures+="Storage shows '$shown'; "
open_view about >/dev/null
wait_until 10 tree_has about.dataFolder || failures+="the About view did not render; "
about_shown=$(tree_get about.dataFolder text)
[ "$about_shown" = "~/hp-data" ] || failures+="About shows '$about_shown'; "
shot 005-home-relative
/harness/slot/app.sh stop >/dev/null
APP_DATA_DIR=/data /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
section storage >/dev/null
shown=$(tree_get settings.storage.dataFolder text)
[ "$shown" = "/data" ] || failures+="with /data the Storage row shows '$shown'; "
open_view about >/dev/null
wait_until 10 tree_has about.dataFolder || failures+="the About view did not render for /data; "
about_shown=$(tree_get about.dataFolder text)
[ "$about_shown" = "/data" ] || failures+="with /data the About row shows '$about_shown'; "
check_failures "VAL-SET-005 ~/... display" "home-relative shows ~/hp-data, outside home shows /data, on both Storage and About" "$failures"

echo "== VAL-SET-006 Change data folder moves the data and the app uses it after the restart"
failures=""
DEF=${XDG_DATA_HOME:-$HOME/.local/share}/hushpen
NEW=$HOME/hp-moved
/harness/slot/app.sh stop >/dev/null
rm -rf "$DEF" "$NEW"
# The default folder with the base model in place before the first start, so the model scan
# finds it (a model copied in later is not picked up again).
mkdir -p "$DEF/models/whisper"
cp /assets/models/whisper/ggml-base.bin "$DEF/models/whisper/"
APP_NO_DATA_DIR=1 /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle || failures+="the app did not start on the default folder; "
use_model base || failures+="the base model never became ready; "
[ -f "$DEF/models/whisper/ggml-base.bin.verified" ] || failures+="no .verified stamp for base; "
# 3 history rows with audio, 1 dictionary entry, a changed cue volume.
/harness/slot/app.sh stop >/dev/null
/harness/slot/seed-history.sh 3 "$DEF/history/history.db" >/dev/null
sqlite3 "$DEF/history/history.db" \
  "update transcript set audio_path = 'audio/' || id || '.wav'"
for wav in $(sqlite3 "$DEF/history/history.db" "select audio_path from transcript"); do
  mkdir -p "$DEF/$(dirname "$wav")"
  cp "$SHORT_WAV" "$DEF/$wav"
done
sqlite3 "$DEF/history/history.db" \
  "insert into dictionary_entry (phrase, heard_as, created_at) values ('Hushpen', 'hush pen', 1)"
find "$DEF" -type f | sort >"$RUN_OUT/006-before.txt"
APP_NO_DATA_DIR=1 /harness/slot/app.sh start >/dev/null
wait_until 20 dict_is .state idle
$HC action set-setting '{"key":"audio.cueVolume","value":0.25}' >/dev/null
setting_is audio.cueVolume 0.25 || failures+="audio.cueVolume is $(sget audio.cueVolume); "
section storage >/dev/null
mkdir -p "$NEW"
$HC paths "$NEW" >/dev/null
old_pid=$(ui_pid)
$HC click settings.storage.changeFolder >/dev/null
# The move runs, the old process quits, and a fresh copy binds the hook socket.
wait_until 60 dead "$old_pid" || failures+="the old process did not quit after the move; "
wait_until 60 $HC state >/dev/null || failures+="the app never came back on the hook; "
/harness/slot/app.sh adopt >/dev/null || failures+="no restarted app process to adopt; "
wait_until 60 dict_is .state idle || failures+="the restarted app is not idle; "
[ -f "$NEW/.hushpen-data" ] || failures+="no marker in the new folder; "
[ "$(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript' 2>/dev/null)" = 3 ] || failures+="the new history.db does not hold 3 rows; "
[ "$(find "$NEW/audio" -name '*.wav' 2>/dev/null | wc -l)" = 3 ] || failures+="the new audio folder holds $(find "$NEW/audio" -name '*.wav' 2>/dev/null | wc -l) WAVs; "
[ "$(sqlite3 "$NEW/history/history.db" 'select phrase from dictionary_entry' 2>/dev/null)" = Hushpen ] || failures+="the dictionary entry did not move; "
[ -f "$NEW/models/whisper/ggml-base.bin" ] || failures+="the model did not move; "
[ -f "$NEW/models/whisper/ggml-base.bin.verified" ] || failures+="the .verified stamp did not move; "
[ "$(jq -r '.values["audio.cueVolume"]' "$NEW/config/settings.json")" = 0.25 ] || failures+="cueVolume in the new settings.json is $(jq -r '.values["audio.cueVolume"]' "$NEW/config/settings.json"); "
[ ! -e "$DEF/history/history.db" ] || failures+="the old folder still holds history.db; "
[ ! -d "$DEF/audio" ] || failures+="the old folder still holds audio/; "
[ ! -d "$DEF/models" ] || failures+="the old folder still holds models/; "
[ "$(jq -r .path "$DEF/location.json" 2>/dev/null)" = "$NEW" ] || failures+="location.json reads $(cat "$DEF/location.json" 2>/dev/null); "
find "$NEW" -type f | sort >"$RUN_OUT/006-after.txt"
cat "$DEF/location.json" >"$RUN_OUT/006-location.json" 2>/dev/null
# After the restart both rows read the new folder as ~/..., the history lists the 3 rows, and a
# new dictation lands in the new database only.
section storage >/dev/null
shown=$(tree_get settings.storage.dataFolder text)
[ "$shown" = "~/hp-moved" ] || failures+="after the move Storage shows '$shown'; "
open_view about >/dev/null
wait_until 10 tree_has about.dataFolder || failures+="the About view did not render after the move; "
about_shown=$(tree_get about.dataFolder text)
[ "$about_shown" = "~/hp-moved" ] || failures+="after the move About shows '$about_shown'; "
open_view history >/dev/null
wait_until 10 tree_has history.search || failures+="the History view did not render; "
sleep 0.5
rows=$($HC tree | jq -r '[.[] | select(.id | test("^history.row"))] | length')
[ "$rows" = 3 ] || failures+="History lists $rows rows; "
shot 006-moved
wait_until 90 engine_ready_with base || failures+="the engine is not ready with base after the move; "
gtk_clear
ptt_run "$SHORT_WAV" || failures+="the post-move dictation did not end; "
gtk_commit >/dev/null
[ "$(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript')" = 4 ] || failures+="the new history.db holds $(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript') rows; "
[ ! -e "$DEF/history/history.db" ] || failures+="a history.db reappeared in the default folder; "
check_failures "VAL-SET-006 move to an empty folder" "marker, 3 rows, 3 WAVs, the dictionary entry, the model with its stamp, and cueVolume 0.25 in $NEW; the old folder cleared; location.json points at $NEW; the 4th row landed in the new database" "$failures"

echo "== VAL-SET-007 a move to a read-only folder fails safely"
failures=""
section storage >/dev/null
# The suite runs as root, so chmod 0555 is still writable; a read-only filesystem is not.
READONLY_DIR=/proc/hushpen-read-only-move
rows_before=$(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript')
$HC paths "$READONLY_DIR" >/dev/null
$HC click settings.storage.changeFolder >/dev/null
wait_until 10 move_state_is failed || failures+="the move did not fail (state $(sv .move.state)); "
[ "$(sv .move.error)" = DATA_FOLDER_MOVE_FAILED ] || failures+="the move error is $(sv .move.error); "
shot 007-readonly
# The app still runs on the current folder; the pointer still names it.
[ "$(jq -r .path "$DEF/location.json")" = "$NEW" ] || failures+="location.json changed to $(cat "$DEF/location.json"); "
[ "$(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript')" = "$rows_before" ] || failures+="the row count changed; "
[ ! -e "$READONLY_DIR" ] || failures+="something appeared under the read-only target; "
# A dictation right after still lands in the current folder.
gtk_clear
ptt_run "$SHORT_WAV" || failures+="the post-failure dictation did not end; "
gtk_commit >/dev/null
[ "$(sqlite3 "$NEW/history/history.db" 'select count(*) from transcript')" = $((rows_before + 1)) ] || failures+="the new row is missing from the current folder; "
check_failures "VAL-SET-007 read-only target" "DATA_FOLDER_MOVE_FAILED shown, location.json untouched, the current folder unchanged ($rows_before rows before), the next dictation saved its row there" "$failures"

echo "== VAL-SET-008 About shows the version and the data folder"
failures=""
open_view about >/dev/null
wait_until 10 tree_has about.version || failures+="the About view did not render; "
version=$(tree_get about.version text)
cli_version=$(/app/hushpen --version 2>/dev/null | awk '{print $2}')
[ "$version" = "$cli_version" ] || failures+="the version row reads '$version', --version says '$cli_version'; "
case "$version" in 2.0.0*) ;; *) failures+="the version '$version' does not start with 2.0.0; " ;; esac
folder=$(tree_get about.dataFolder text)
[ "$folder" = "~/hp-moved" ] || failures+="the data folder row reads '$folder'; "
all_text=$($HC tree | jq -r '.[].text' | grep -v '^$')
for banned in Electron React GPUI Rust whisper.cpp llama.cpp; do
  grep -q "$banned" <<<"$all_text" && failures+="the About text contains '$banned'; "
done
$HC tree >"$RUN_OUT/008-tree.json"
shot 008-about
check_failures "VAL-SET-008 about content" "version $version (== hushpen --version), data folder $folder, no stack names in the text" "$failures"

echo "== VAL-SET-009 the About rows open the right targets"
failures=""
rm -rf "$NEW/logs"
open_view about >/dev/null
for target in data-folder log-folder github issues; do
  $HC click "about.open.$target" >/dev/null || failures+="no row for $target; "
  sleep 0.3
done
$HC state | jq -c '.about["open-requests"]' >"$RUN_OUT/009-requests.json"
requests=$(cat "$RUN_OUT/009-requests.json")
echo "$requests"
[ "$(jq length <<<"$requests")" = 4 ] || failures+="4 clicks logged $(jq length <<<"$requests") requests; "
check_req() { # <index> <target> <url substring>
  local entry
  entry=$(jq -c ".[$1]" <<<"$requests")
  [ "$(jq -r .target <<<"$entry")" = "$2" ] || failures+="request $1 target is $(jq -r .target <<<"$entry"); "
  case "$(jq -r .url <<<"$entry")" in *"$3"*) ;; *) failures+="request $1 url is $(jq -r .url <<<"$entry"), want *$3*; " ;; esac
  [ "$(jq -r .ok <<<"$entry")" = true ] || failures+="request $1 failed; "
}
check_req 0 data-folder "$NEW"
check_req 1 log-folder "$NEW/logs"
check_req 2 github github.com
check_req 3 issues issues
[ -d "$NEW/logs" ] || failures+="logs/ did not come back in the data folder; "
check_failures "VAL-SET-009 about opens" "one request per row: data folder $NEW, logs $NEW/logs (recreated), the repository URL, the issues URL" "$failures"

echo "== VAL-LIC-004 the About licenses row opens the notices file"
failures=""
open_view about >/dev/null
wait_until 10 tree_has about.open.licenses || failures+="no licenses row; "
[ -f /app/THIRD-PARTY-NOTICES.html ] || failures+="/app has no THIRD-PARTY-NOTICES.html; "
last_licenses_url() {
  about '["open-requests"]' | jq -r '[.[] | select(.target == "licenses")] | last | .url // ""'
}
licenses_count() {
  about '["open-requests"]' | jq '[.[] | select(.target == "licenses")] | length'
}
$HC click about.open.licenses >/dev/null
sleep 0.3
[ "$(licenses_count)" = 1 ] || failures+="the click logged $(licenses_count) licenses requests; "
[ "$(last_licenses_url)" = /app/THIRD-PARTY-NOTICES.html ] || failures+="the click opened '$(last_licenses_url)'; "
shot 010-licenses-click
# The click above left the focus on the licenses row; move it away so the Tabs prove the way.
licenses_unfocused() { [ "$(tree_get about.open.licenses focused)" != true ]; }
$HC click sidebar.about >/dev/null
wait_until 5 licenses_unfocused || failures+="the focus stayed on the licenses row; "
focus "$(app_window)"
reached=0
for _ in $(seq 1 25); do
  [ "$(tree_get about.open.licenses focused)" = true ] && { reached=1; break; }
  xdotool key Tab
  sleep 0.25
done
[ $reached = 1 ] || failures+="Tab never reached the licenses row; "
xdotool key Return
licenses_requested_twice() { [ "$(licenses_count)" = 2 ]; }
wait_until 5 licenses_requested_twice
[ "$(licenses_count)" = 2 ] || failures+="Enter left $(licenses_count) licenses requests, want 2; "
[ "$(last_licenses_url)" = /app/THIRD-PARTY-NOTICES.html ] || failures+="Enter opened '$(last_licenses_url)'; "
shot 011-licenses-keyboard
check_failures "VAL-LIC-004 about licenses" "click and Tab then Enter each logged target licenses with $(last_licenses_url)" "$failures"
