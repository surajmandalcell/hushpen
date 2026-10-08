#!/bin/bash
# The tray icon and its menu: VAL-TRAY-001 to 008, VAL-CROSS-009, and the History Re-paste hide
# (architecture 5.5). The StatusNotifier item is read and clicked over D-Bus like a panel does it
# (tray-menu.py: the watcher stub's item list, the dbusmenu layout, and the `clicked` event). The
# main window is the 780x520 window of class hushpen; its map state comes from xwininfo and its
# minimized state from xprop WM_STATE. The words come from the virtual microphone. Needs
# speech-short.wav in /assets/fixtures and the base and tiny.en models in /assets/models/whisper.
# Busy: it transcribes. It empties /data and restarts the app, and the last check stops the
# watcher stub, so the slot must not be shared. Runs in one slot through with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
DB=/data/history/history.db
SHORT_WAV=$FIX/speech-short.wav
TM="python3 /harness/slot/tray-menu.py"
LOG=/data/logs/hushpen.log

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() { # the pipeline states since T0, as "a,b,c"
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
sessions() { # how many listening states since T0
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0 and .detail == "listening")] | length'
}
is() { [ "$(eval "$1")" "$2" "$3" ]; }
focus() { xdotool windowfocus --sync "$1"; }
fid() { xdotool getwindowfocus; }
speech() { paplay --device=vmic "$(pad "$1")"; }
count() { sqlite3 "$DB" "select count(*) from transcript"; }
q() { sqlite3 "$DB" "$1"; }
gtk_commit() { # Enter in the GTK entry writes what it holds to /out/gtk.txt; prints it
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

# The main window is the one that is 780x520; the flow bar is another window of the same class.
main_wid() {
  local wid
  for wid in $(xdotool search --class hushpen 2>/dev/null); do
    [ "$(win_size "$wid")" = 780x520 ] && { echo "$wid"; return 0; }
  done
  return 1
}
map_state() { xwininfo -id "$1" 2>/dev/null | awk '/Map State/ {print $3}'; }
is_mapped() { [ "$(map_state "$(main_wid)")" = IsViewable ]; }
is_unmapped() { [ "$(map_state "$(main_wid)")" = IsUnMapped ]; }
wm_state() { xprop -id "$(main_wid)" WM_STATE 2>/dev/null | sed -n 's/.*window state: *//p'; }
is_iconic() { [ "$(wm_state)" = Iconic ]; }
win_away() { $HC state | jq -r '.window.away'; }
view() { $HC state | jq -r .view; }

labels() { $TM labels | tr '\n' '|' | sed 's/|$//'; }
first_label() { $TM labels | head -1; }
items() { python3 /harness/slot/sni-host.py --items; }
tray_click() { $TM click "$1" >>"$RUN_OUT/tray-clicks.log" 2>&1; }
ui_alive() { kill -0 "$(ui_pid)" 2>/dev/null; }
engine_alive() { local pid; pid=$(engine .pid); [ -n "$pid" ] && [ "$pid" != null ] && kill -0 "$pid" 2>/dev/null; }
hushpen_procs() { pgrep -ax hushpen | grep -vc ' engine$'; }
hushpen_any() { pgrep -x hushpen | wc -l; }

close_click() { # a real pointer click on the close traffic light
  local wid ox oy bx by
  wid=$(main_wid)
  read -r ox oy < <(settled_origin "$wid")
  read -r bx by < <($HC tree | jq -r '.[] | select(.id == "window.close") | .bounds | "\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"')
  [ -n "$bx" ] || return 1
  xdotool mousemove $((ox + bx)) $((oy + by))
  sleep 0.4
  xdotool click 1
  sleep 0.2
}
park() { xdotool mousemove 5 5; sleep 0.2; }

hold_start() {
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
}
hold_stop() { xdotool keyup $HOLD; }
ptt_run() { # <wav>
  hold_start || return 1
  speech "$1"
  hold_stop
  wait_until 240 run_ended
}
words_ok() { [ -z "$(missing_words "$(norm <<<"$1")" "${SHORT_WORDS[@]}")" ]; }

echo "== setup: base and tiny.en models, the paste targets"
fresh base tiny.en
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)
wait_until 10 main_wid
wait_until 20 is 'items | wc -l' = 1
park

echo "== VAL-TRAY-001 the StatusNotifier item is on D-Bus with an icon"
failures=""
items >"$RUN_OUT/001-items.txt"
[ "$(wc -l <"$RUN_OUT/001-items.txt")" = 1 ] || failures+="the watcher lists $(wc -l <"$RUN_OUT/001-items.txt") items; "
$TM props >"$RUN_OUT/001-props.json" 2>&1 || failures+="reading the item failed: $(cat "$RUN_OUT/001-props.json"); "
title=$(jq -r .title "$RUN_OUT/001-props.json" 2>/dev/null)
item_id=$(jq -r .id "$RUN_OUT/001-props.json" 2>/dev/null)
pixmap=$(jq -r '.pixmaps[0] | "\(.[0])x\(.[1]) \(.[2]) bytes"' "$RUN_OUT/001-props.json" 2>/dev/null)
icon_name=$(jq -r .icon_name "$RUN_OUT/001-props.json" 2>/dev/null)
case "$title $item_id" in *Hushpen* | *hushpen*) ;; *) failures+="title '$title' and id '$item_id' do not name Hushpen; " ;; esac
[ "$(jq -r '.pixmaps | length' "$RUN_OUT/001-props.json" 2>/dev/null)" -gt 0 ] || [ -n "$icon_name" ] || failures+="the item has no icon; "
read -r bus_name _ <<<"$($TM item)"
gdbus introspect --session --dest "$bus_name" --object-path /StatusNotifierItem >"$RUN_OUT/001-introspect.txt" 2>&1
check_failures "VAL-TRAY-001 item" "one item $(head -1 "$RUN_OUT/001-items.txt"), title '$title', id '$item_id', icon pixmap $pixmap" "$failures"

echo "== VAL-TRAY-002 the menu and its live label"
failures=""
want="Start dictation|Paste last transcript|Show Hushpen|Settings|Quit"
idle_labels=$(labels)
[ "$idle_labels" = "$want" ] || failures+="idle menu is '$idle_labels'; "
$HC state | jq -c .tray_menu >"$RUN_OUT/002-hook-menu.json"
[ "$(jq -r '[.items[].label] | join("|")' "$RUN_OUT/002-hook-menu.json")" = "$want" ] || failures+="the hook menu is $(cat "$RUN_OUT/002-hook-menu.json"); "
hold_start || failures+="the hold did not reach listening; "
wait_until 3 is first_label = "Stop dictation" || failures+="while listening the first entry reads '$(first_label)'; "
listening_labels=$(labels)
speech "$SHORT_WAV"
hold_stop
wait_until 240 run_ended || failures+="the hold run did not end; "
ended=$(now_ms)
wait_until 1 is first_label = "Start dictation" || failures+="after the run the first entry reads '$(first_label)'; "
back_ms=$(($(now_ms) - ended))
got=$(gtk_commit)
gtk_clear
check_failures "VAL-TRAY-002 menu" "idle '$idle_labels'; listening '$listening_labels'; Start dictation back $back_ms ms after the run ended; the hold run typed '$got'" "$failures"

echo "== VAL-TRAY-003 and VAL-CROSS-009 Start and Stop from the tray, Paste last transcript"
failures=""
gtk_clear
set_clip SENTINEL
focus "$GT"
F0=$(fid)
c0=$(count)
T0=$(now_ms)
tray_click "Start dictation" || failures+="no Start dictation entry; "
wait_until 5 pstate_is listening || failures+="the tray did not start a dictation; "
wait_until 3 is first_label = "Stop dictation" || failures+="the entry did not flip to Stop dictation; "
speech "$SHORT_WAV" &
player=$!
sleep 1.5
wait "$player"
bar_x=$($HC tree | jq -r '.[] | select(.id == "flowbar.bar") | .root_bounds | "\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"')
read -r bx by <<<"$bar_x"
[ -n "$bx" ] || failures+="no flowbar.bar in the tree; "
xdotool mousemove "$bx" "$by"
sleep 0.4
xdotool click 1
wait_until 240 run_ended || failures+="the run did not end after the flow bar click; "
park
sessions_n=$(sessions)
[ "$sessions_n" = 1 ] || failures+="$sessions_n sessions started; "
got=$(gtk_commit)
words_ok "$got" || failures+="the entry holds '$got'; "
[ "$(fid)" = "$F0" ] || failures+="focus moved from $F0 to $(fid); "
wait_until 3 test "$(clip)" = SENTINEL || failures+="the clipboard reads '$(clip)' instead of SENTINEL; "
[ "$(count)" = $((c0 + 1)) ] || failures+="the run made $(($(count) - c0)) rows; "
[ "$(q "select status from transcript order by created_at desc, id desc limit 1")" = completed ] || failures+="the newest row is not completed; "
first_text=$got
# Stop dictation from the tray
gtk_clear
focus "$GT"
c1=$(count)
T0=$(now_ms)
tray_click "Start dictation"
wait_until 5 pstate_is listening || failures+="the second tray start did not listen; "
speech "$SHORT_WAV"
tray_click "Stop dictation" || failures+="no Stop dictation entry; "
wait_until 240 run_ended || failures+="the tray stop did not end the run; "
got2=$(gtk_commit)
words_ok "$got2" || failures+="after the tray stop the entry holds '$got2'; "
[ "$(fid)" = "$F0" ] || failures+="focus moved to $(fid) after the tray stop; "
wait_until 3 test "$(clip)" = SENTINEL || failures+="the clipboard reads '$(clip)' after the tray stop; "
# Paste last transcript
gtk_clear
focus "$GT"
c2=$(count)
tray_click "Paste last transcript" || failures+="no Paste last transcript entry; "
sleep 1.5
got3=$(gtk_commit)
[ "$got3" = "$got2" ] || failures+="Paste last transcript typed '$got3', not '$got2'; "
[ "$(count)" = "$c2" ] || failures+="Paste last transcript changed the row count; "
wait_until 3 test "$(clip)" = SENTINEL || failures+="the clipboard reads '$(clip)' after Paste last; "
shot 003-after
# The hold key while the tray's session listens starts no second session: the pipeline has one
# session, and a press in a hands-free session stops it, as for a flow bar session.
T0=$(now_ms)
tray_click "Start dictation"
wait_until 5 pstate_is listening || failures+="the third tray start did not listen; "
xdotool keydown $HOLD
sleep 0.3
xdotool keyup $HOLD
wait_until 60 run_ended || failures+="the session did not end after the hold key; "
sleep 0.5
hold_sessions=$(sessions)
[ "$hold_sessions" = 1 ] || failures+="the hold key during a tray session left $hold_sessions sessions; "
park
check_failures "VAL-TRAY-003 and VAL-CROSS-009 tray start and stop" "tray start + flow bar click typed '$first_text' (1 session, 1 completed row, focus $F0); tray start + stop typed '$got2'; Paste last typed '$got3'; sentinel restored; the hold key during a tray session made $hold_sessions session" "$failures"

echo "== VAL-TRAY-007 Start dictation respects the preflight"
failures=""
pactl unload-module module-virtual-source
pactl unload-module module-null-sink
sleep 1
sources=$(pactl list short sources | wc -l)
pid0=$(ui_pid)
tray_click "Start dictation"
sleep 1.5
state_now=$(pstate)
code=$($HC state | jq -r '.capture.notice.code')
bar_state=$($HC state | jq -r '.overlay.state')
bar_msg=$($HC state | jq -r '.overlay.message')
label_now=$(first_label)
shot 007-no-mic
wait_until 10 pstate_is idle || failures+="the pipeline did not return to idle (state $(pstate)); "
[ "$sources" = 0 ] || failures+="pactl lists $sources sources; "
[ "$state_now" != listening ] || failures+="the pipeline is still listening; "
[ "$code" = MIC_UNAVAILABLE ] || failures+="the notice code is '$code'; "
[ "$bar_state" = error ] || failures+="the bar shows $bar_state, not error; "
[ "$label_now" = "Start dictation" ] || failures+="the first entry reads '$label_now'; "
[ "$(ui_pid)" = "$pid0" ] && ui_alive || failures+="the app is gone; "
pactl load-module module-null-sink sink_name=vmic >/dev/null
pactl load-module module-virtual-source source_name=vmic_src master=vmic.monitor >/dev/null
pactl load-module module-null-sink sink_name=cues >/dev/null
pactl set-default-source vmic_src
pactl set-default-sink cues
sleep 2.5
check_failures "VAL-TRAY-007 preflight" "no sources: state $state_now, notice $code, bar $bar_state ('$bar_msg'), first entry '$label_now', app pid $pid0 still running" "$failures"

echo "== VAL-TRAY-004 close hides to the tray; Show Hushpen and Settings bring the window back"
failures=""
engine_pid=$(engine .pid)
ui0=$(ui_pid)
wait_until 10 is_mapped || failures+="the window is not mapped before the close; "
wid=$(main_wid)
close_click || failures+="no close control to click; "
wait_until 5 is_unmapped || failures+="the window is $(map_state "$wid") after the close click; "
shot 004-hidden
sleep 0.5
ui_alive && [ "$(ui_pid)" = "$ui0" ] || failures+="the UI process ended; "
engine_alive || failures+="the engine child is gone; "
[ "$(items | wc -l)" = 1 ] || failures+="the tray item is not registered ($(items | wc -l)); "
[ "$(win_away)" = hidden ] || failures+="the hook says away=$(win_away); "
gtk_clear
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the hold run did not end with the window hidden; "
hidden_got=$(gtk_commit)
words_ok "$hidden_got" || failures+="with the window hidden the entry holds '$hidden_got'; "
tray_click "Show Hushpen"
wait_until 5 is_mapped || failures+="Show Hushpen did not map the window; "
sleep 0.5
size=$(win_size "$(main_wid)")
[ "$size" = 780x520 ] || failures+="the window is $size after Show; "
[ "$(win_away)" = null ] || failures+="the hook says away=$(win_away) while shown; "
shot 004-shown
close_click
wait_until 5 is_unmapped || failures+="the second close did not hide the window; "
tray_click Settings
wait_until 5 is_mapped || failures+="Settings did not map the window; "
wait_until 5 is view = settings || failures+="the view is $(view), not settings; "
sleep 0.4
shot 004-settings
[ "$(win_size "$(main_wid)")" = 780x520 ] || failures+="the window is $(win_size "$(main_wid)") after Settings; "
check_failures "VAL-TRAY-004 close hides" "close click unmapped window $wid; UI $ui0 and engine $engine_pid kept running; item still registered; hold dictation typed '$hidden_got' while hidden; Show Hushpen mapped 780x520; Settings opened the settings view" "$failures"

echo "== History Re-paste (architecture 5.5) hides the window and pastes into the app behind it"
failures=""
tray_click "Show Hushpen"
wait_until 5 is_mapped
$HC action open-view '{"view":"history"}' >/dev/null
wait_until 10 is view = history || failures+="no History view; "
sleep 0.5
hist_text=$(q "select final_text from transcript order by created_at desc, id desc limit 1")
$HC click history.row.0 >/dev/null
has_repaste() { [ -n "$($HC tree | jq -r '.[] | select(.id == "history.repaste") | .id')" ]; }
wait_until 5 has_repaste || failures+="no Re-paste control; "
gtk_clear
focus "$GT"
focus "$(main_wid)"
sleep 0.4
[ "$(fid)" = "$(main_wid)" ] || failures+="Hushpen does not hold the focus before Re-paste; "
read -r ox oy < <(settled_origin "$(main_wid)")
read -r rx ry < <($HC tree | jq -r '.[] | select(.id == "history.repaste") | .bounds | "\(.x + .width / 2 | floor) \(.y + .height / 2 | floor)"')
xdotool mousemove $((ox + rx)) $((oy + ry))
sleep 0.4
xdotool click 1
wait_until 5 is_unmapped || failures+="the window did not hide after Re-paste; "
sleep 2
repaste_got=$(gtk_commit)
[ "$repaste_got" = "$hist_text" ] || failures+="the entry holds '$repaste_got', not '$hist_text'; "
is_unmapped || failures+="the window is $(map_state "$(main_wid)") after the paste; "
shot repaste-after
tray_click "Show Hushpen"
wait_until 5 is_mapped || failures+="Show Hushpen did not bring the window back after Re-paste; "
check_failures "History Re-paste" "Re-paste with Hushpen focused hid the window; '$repaste_got' landed in the GTK entry; Show Hushpen reopened it" "$failures"

echo "== VAL-TRAY-005 a second start while hidden shows the window"
failures=""
close_click
wait_until 5 is_unmapped || failures+="the window did not hide; "
procs_before=$(hushpen_procs)
begin=$(now_ms)
/app/hushpen >"$RUN_OUT/005-second.log" 2>&1 &
second=$!
wait_until 5 bash -c "! kill -0 $second 2>/dev/null"
second_ms=$(($(now_ms) - begin))
wait "$second"
second_rc=$?
wait_until 5 is_mapped || failures+="the first process did not map its window; "
sleep 0.3
shot 005-shown
procs_after=$(hushpen_procs)
[ "$second_ms" -le 2000 ] || failures+="the second start ran $second_ms ms; "
[ "$procs_before" = 1 ] && [ "$procs_after" = 1 ] || failures+="hushpen processes: $procs_before before, $procs_after after; "
[ "$(win_size "$(main_wid)")" = 780x520 ] || failures+="the window is $(win_size "$(main_wid)"); "
check_failures "VAL-TRAY-005 second start" "second process exited ($second_rc) after $second_ms ms; $procs_before then $procs_after hushpen process; the window mapped at 780x520" "$failures"

echo "== VAL-TRAY-006 Quit exits the app and its children"
failures=""
/harness/slot/app.sh stop
/app/hushpen >"$RUN_OUT/006-app.log" 2>&1 &
pid=$!
echo $pid >/tmp/app.pid
wait_until 20 dict_is .state idle
wait_until 20 is 'items | wc -l' = 1
engine_pid=$(engine .pid)
tray_click Quit || failures+="no Quit entry; "
begin=$(now_ms)
wait_until 3 bash -c "! kill -0 $pid 2>/dev/null" || failures+="the app was still running 3 s after Quit; "
quit_ms=$(($(now_ms) - begin))
wait "$pid"
rc=$?
rm -f /tmp/app.pid
[ "$rc" = 0 ] || failures+="the exit code is $rc; "
wait_until 3 bash -c "! kill -0 $engine_pid 2>/dev/null" || failures+="the engine child $engine_pid still runs; "
wait_until 3 test "$(items | wc -l)" = 0 || failures+="the watcher still lists $(items | wc -l) items; "
[ "$(hushpen_any)" = 0 ] || failures+="a hushpen process is left; "
pgrep -f hushpen-llm >/dev/null && failures+="hushpen-llm is left; "
items >"$RUN_OUT/006-items.txt" 2>&1
check_failures "VAL-TRAY-006 Quit" "exit code $rc about $quit_ms ms after the click; engine $engine_pid gone; the item left the watcher; no hushpen or hushpen-llm process" "$failures"

echo "== VAL-TRAY-008 with no tray host the close button minimizes the window"
failures=""
pkill -f 'slot/sni-host.py$'
wait_until 5 bash -c "! python3 /harness/slot/sni-host.py --query"
owner=$(gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner org.kde.StatusNotifierWatcher)
echo "watcher owned: $owner" >"$RUN_OUT/008-watcher.txt"
[ "$owner" = "(false,)" ] || failures+="the watcher name is still owned ($owner); "
: >"$LOG"
start_app
sleep 2
wid=$(main_wid)
[ -n "$wid" ] || failures+="no main window; "
items >>"$RUN_OUT/008-watcher.txt" 2>&1 && failures+="the watcher answered an items query; "
lines=$(grep -c 'no tray host' "$LOG")
[ "$lines" = 1 ] || failures+="the log has $lines lines about the tray host; "
$HC state | jq -c .tray_menu >"$RUN_OUT/008-hook-menu.json"
ui0=$(ui_pid)
close_click || failures+="no close control to click; "
wait_until 5 is_iconic || failures+="WM_STATE is '$(wm_state)', not Iconic; "
shot 008-minimized
xprop -id "$wid" WM_STATE _NET_WM_STATE >"$RUN_OUT/008-xprop.txt" 2>&1
ui_alive && [ "$(ui_pid)" = "$ui0" ] || failures+="the UI process ended; "
engine_alive || failures+="the engine child is gone; "
[ "$(map_state "$wid")" != IsUnMapped ] || [ "$(wm_state)" = Iconic ] || failures+="the window was withdrawn; "
gtk_clear
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the hold run did not end while minimized; "
mini_got=$(gtk_commit)
words_ok "$mini_got" || failures+="while minimized the entry holds '$mini_got'; "
procs_before=$(hushpen_procs)
/app/hushpen >"$RUN_OUT/008-second.log" 2>&1 &
second=$!
wait_until 5 bash -c "! kill -0 $second 2>/dev/null"
wait "$second"
wait_until 5 is_mapped || failures+="the second start did not map the window (WM_STATE '$(wm_state)'); "
sleep 0.4
[ "$(win_size "$(main_wid)")" = 780x520 ] || failures+="the window is $(win_size "$(main_wid)"); "
[ "$(hushpen_procs)" = "$procs_before" ] || failures+="the second start left a process; "
shot 008-restored
python3 /harness/slot/sni-host.py >/logs/sni-host.log 2>&1 &
check_failures "VAL-TRAY-008 no tray host" "no watcher: one log line, close left UI $ui0 and the engine running with WM_STATE Iconic; the hold run typed '$mini_got'; a second start mapped the window at 780x520" "$failures"
