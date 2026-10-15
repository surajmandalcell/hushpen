#!/bin/bash
# The opt-in update check: VAL-UPD-001 to 007 and 009, and the Linux open_path/open_url half of
# VAL-LIC-004 (xdg-open is called). VAL-UPD-008 is `cargo test -p hushpen-app net::update`.
# A local feed (harness/slot/update-feed-server.py) on 127.0.0.1:8765 answers with the signed test
# manifests in harness/slot/update-feed; the first line of /tmp/feed-mode picks the folder and every
# request is one line in $RUN_OUT/feed-requests.jsonl. The debug app is built with update-test-feed,
# so HUSHPEN_UPDATE_FEED and HUSHPEN_UPDATE_PUBKEY (test.pub) point it at that feed. A stub
# xdg-open on the app's PATH logs what the About rows and the release notes link open. The strace
# checks start the app under strace (APP_STRACE) so no connect can slip in before the audit.
# VAL-UPD-009 runs the release build from /app/hushpen-release (HUSHPEN_BUILD_RELEASE=1 harness/build-app.sh);
# without that file the check fails, because a skipped proof is not a pass. The onboarding half of
# VAL-UPD-001 (the update step shows the choice off) is in the onboarding suite.
# Needs no model and no fixtures. It empties /data, so the slot must not be shared.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

FEED_PORT=8765
FEED_URL=http://127.0.0.1:$FEED_PORT/latest.json
FEED_KEY=/harness/slot/update-feed/test.pub
FEED_LOG=$RUN_OUT/feed-requests.jsonl
FEED_PID=/tmp/feed.pid
FEED_MODE=/tmp/feed-mode
SETTINGS_FILE=/data/config/settings.json
NOTES_NEWER=https://github.com/surajmandalcell/asrpro/releases/tag/v2.1.0
STUB_BIN=/tmp/stubbin
XDG_LOG=/tmp/xdg-open.log

focus() { xdotool windowfocus --sync "$1"; }
upd() { $HC state | jq -r ".updates$1"; }
upd_is() { [ "$(upd "$1")" = "$2" ]; }
sv() { $HC state | jq -r ".settings_view$1"; }
view_is() { [ "$($HC state | jq -r .view)" = "$1" ]; }
tree_has() { $HC tree | jq -e --arg id "$1" '.[] | select(.id == $id)' >/dev/null; }
tree_get() { $HC tree | jq -r --arg id "$1" ".[] | select(.id == \$id) | .$2" | head -1; }
hook_up() { $HC state >/dev/null 2>&1; }
update_events() { $HC events | jq -r '[.[] | select(.kind == "updates") | .detail] | join(",")'; }

feed_up() { python3 -c 'import socket; socket.create_connection(("127.0.0.1", 8765), 1).close()' 2>/dev/null; }
feed_stop() {
  [ -f "$FEED_PID" ] || return 0
  kill "$(cat "$FEED_PID")" 2>/dev/null
  rm -f "$FEED_PID"
}
feed_start() {
  echo "$1" >"$FEED_MODE"
  python3 /harness/slot/update-feed-server.py "$FEED_LOG" "$FEED_PORT" >/dev/null 2>&1 &
  echo $! >"$FEED_PID"
  wait_until 5 feed_up
}
feed_mode() { echo "$1" >"$FEED_MODE"; }
trap feed_stop EXIT
# feed_lines [from]: the feed log lines after the first <from> lines
feed_count() { if [ -s "$FEED_LOG" ]; then wc -l <"$FEED_LOG" | tr -d ' '; else echo 0; fi; }
feed_after() { if [ -s "$FEED_LOG" ]; then tail -n +$(($1 + 1)) "$FEED_LOG"; fi; }
manifest_count() { # <from>: manifest requests after the first <from> log lines
  feed_after "$1" | jq -s '[.[] | select(.path == "/latest.json")] | length'
}
feed_paths() { feed_after "$1" | jq -rs '[.[].path] | unique | join(",")'; }

reset_data() {
  /harness/slot/app.sh stop
  find /data -mindepth 1 -delete
}
seed() { # <json object of settings>: a finished onboarding plus these values
  mkdir -p /data/config
  jq -n --argjson v "$1" '{schemaVersion: 1, values: ({"onboarding.completed": true} + $v)}' >"$SETTINGS_FILE"
}
# start_feed_app [VAR=value...]: the debug app pointed at the local feed, with the stub xdg-open first on PATH
start_feed_app() {
  env "$@" HUSHPEN_UPDATE_FEED="$FEED_URL" HUSHPEN_UPDATE_PUBKEY="$FEED_KEY" PATH="$STUB_BIN:$PATH" \
    /harness/slot/app.sh start >/dev/null || return 1
  wait_until 20 hook_up
}
open_updates() {
  focus "$(app_window)"
  $HC action open-view '{"view":"settings"}' >/dev/null
  wait_until 10 view_is settings || return 1
  wait_until 10 tree_has settings.section.updates || return 1
  [ "$(sv .section)" = updates ] || $HC click settings.section.updates >/dev/null
  wait_until 10 tree_has settings.updates.checkNow
}
check_now() { hclick settings.updates.checkNow; }
settled() { # the state is neither idle nor checking
  case "$(upd .state)" in idle | checking) return 1 ;; esac
}
setting_true() { [ "$(jq -r --arg k "$1" '.values[$k] // false' "$SETTINGS_FILE")" = true ]; }

connects() { # <strace file>: the distinct host:port pairs the app connected to over IP
  python3 - "$1" <<'PY'
import re, sys
seen = set()
for line in open(sys.argv[1], errors="replace"):
    m = re.search(r'sin_port=htons\((\d+)\), sin_addr=inet_addr\("([^"]+)"\)', line)
    if m:
        seen.add(f"{m.group(2)}:{m.group(1)}")
    m = re.search(r'sin6_port=htons\((\d+)\).*inet_pton\(AF_INET6, "([^"]+)"', line)
    if m:
        seen.add(f"[{m.group(2)}]:{m.group(1)}")
print(" ".join(sorted(seen)))
PY
}
inet_connects() { grep -cE 'AF_INET6?' "$1" 2>/dev/null || true; }

echo "== setup: the local signed feed and a stub xdg-open"
mkdir -p "$STUB_BIN"
printf '#!/bin/sh\nprintf "%%s\\n" "$*" >>%s\n' "$XDG_LOG" >"$STUB_BIN/xdg-open"
chmod +x "$STUB_BIN/xdg-open"
rm -f "$FEED_LOG" "$XDG_LOG"
feed_start newer || echo "the feed did not start"
printf '%s\n' "$FEED_URL" >"$RUN_OUT/feed-url.txt"

echo "== VAL-UPD-001 the check and the automatic install are off by default"
failures=""
reset_data
start_feed_app || failures+="the app did not start; "
open_updates || failures+="Settings > Updates did not open; "
[ "$(upd .check)" = false ] || failures+="the update check is '$(upd .check)' on a fresh folder; "
[ "$(upd .auto_install)" = false ] || failures+="auto install is '$(upd .auto_install)' on a fresh folder; "
setting_true updates.check && failures+="settings.json has updates.check true; "
setting_true updates.autoInstall && failures+="settings.json has updates.autoInstall true; "
tree_has settings.updates.check || failures+="no 'Check for updates' switch; "
tree_has settings.updates.autoInstall || failures+="no 'Install updates automatically' switch; "
[ "$(tree_get settings.updates.checkNow enabled)" = false ] || failures+="Check now is enabled with the check off; "
[ "$(tree_get settings.updates.status text)" = "Update checks are off" ] || failures+="the status row says '$(tree_get settings.updates.status text)'; "
$HC tree | jq -r '.[] | select(.id | startswith("settings.updates")) | "\(.id)\t\(.text)\t\(.enabled)"' >"$RUN_OUT/001-tree.tsv"
$HC state | jq -c '{updates, settings}' >"$RUN_OUT/001-state.json"
cp "$SETTINGS_FILE" "$RUN_OUT/001-settings.json" 2>/dev/null
shot 001-updates-defaults
check_failures "VAL-UPD-001 off by default" "fresh folder: updates.check false and updates.autoInstall false in the hook state, neither true in settings.json, status 'Update checks are off', Check now disabled; both switches are in the view" "$failures"

echo "== VAL-UPD-002 no update request while the check is off"
failures=""
reset_data
start_feed_app APP_STRACE="$RUN_OUT/upd-002.strace" || failures+="the app did not start under strace; "
n0=$(feed_count)
open_updates || failures+="Settings > Updates did not open; "
shot 002-off-updates
echo "idle for 60 s with the check off"
sleep 60
shot 002-off-after-idle
inet=$(inet_connects "$RUN_OUT/upd-002.strace")
seen=$(connects "$RUN_OUT/upd-002.strace")
[ "$inet" = 0 ] || failures+="strace logged $inet IP connect call(s): $seen; "
[ "$(feed_count)" = "$n0" ] || failures+="the feed log grew from $n0 to $(feed_count) lines; "
$HC net >"$RUN_OUT/002-net.json"
[ "$(jq length "$RUN_OUT/002-net.json")" = 0 ] || failures+="hookctl net lists $(jq length "$RUN_OUT/002-net.json") request(s); "
[ "$(upd .state)" = idle ] || failures+="the update state is '$(upd .state)'; "
[ -s "$RUN_OUT/upd-002.strace" ] || echo "(the strace file is empty: no connect call of any kind)"
check_failures "VAL-UPD-002 no request while off" "60 s idle with Settings > Updates open and the check off, under strace from the first instruction: 0 IP connect calls, 0 feed requests, hookctl net empty" "$failures"

echo "== VAL-UPD-003 a newer signed version shows available, the notes link and the banner"
failures=""
reset_data
seed "{\"updates.check\": true, \"updates.lastCheckAt\": $(now_ms)}"
feed_mode newer
n0=$(feed_count)
start_feed_app || failures+="the app did not start; "
open_updates || failures+="Settings > Updates did not open; "
sleep 1
[ "$(feed_count)" = "$n0" ] || failures+="a check ran at start although one ran a moment ago; "
[ "$(tree_get settings.updates.status text)" = "Not checked yet" ] || failures+="the status before the check says '$(tree_get settings.updates.status text)'; "
# The window origin is read here so the release check below can click the same places.
read -r ox oy <<<"$(settled_origin "$(app_window)")"
rel() { $HC tree | jq -r --arg id "$1" --argjson ox "$ox" --argjson oy "$oy" \
  '.[] | select(.id == $id) | "\((.root_bounds.x + .root_bounds.width / 2 - $ox) | floor) \((.root_bounds.y + .root_bounds.height / 2 - $oy) | floor)"' | head -1; }
CHECK_NOW_REL=$(rel settings.updates.checkNow)
SIDEBAR_SETTINGS_REL=$(rel sidebar.settings)
SECTION_UPDATES_REL=$(rel settings.section.updates)
shot 003-before-check
check_now || failures+="Check now could not be clicked; "
wait_until 20 upd_is .state available || failures+="the state is '$(upd .state)', not available; "
update_events | tee "$RUN_OUT/003-events.txt" | grep -q 'state checking,state available' || failures+="the update events are '$(update_events)', not checking then available; "
[ "$(upd .version)" = 2.1.0 ] || failures+="the offered version is '$(upd .version)'; "
[ "$(upd .notes_url)" = "$NOTES_NEWER" ] || failures+="notes_url is '$(upd .notes_url)'; "
[ "$(tree_get settings.updates.status text)" = "Version 2.1.0 is available" ] || failures+="the status row says '$(tree_get settings.updates.status text)'; "
tree_has settings.updates.notes || failures+="no release notes link; "
[ "$(tree_get update.banner.install text)" = "Download and install" ] || failures+="the banner button says '$(tree_get update.banner.install text)'; "
[ "$(upd .banner)" = true ] || failures+="the hook says the banner is hidden; "
shot 003-available
hclick settings.updates.notes || failures+="the release notes link could not be clicked; "
wait_until 5 upd_is .last_open.ok true
[ "$(upd .last_open.url)" = "$NOTES_NEWER" ] || failures+="the last open target is '$(upd .last_open.url)', not the manifest notes_url; "
sleep 0.5
grep -qxF "$NOTES_NEWER" "$XDG_LOG" 2>/dev/null || failures+="the stub xdg-open was not called with the notes URL (log: $(cat "$XDG_LOG" 2>/dev/null | tr '\n' ' ')); "
$HC state | jq -c .updates >"$RUN_OUT/003-state.json"
check_failures "VAL-UPD-003 available" "Check now: events '$(update_events)'; version 2.1.0; notes_url $NOTES_NEWER opened through xdg-open; the banner offers 'Download and install'" "$failures"

echo "== VAL-UPD-001 (switch) the banner follows Install updates automatically"
failures=""
[ "$(upd .banner)" = true ] || failures+="no banner with auto install off; "
hclick settings.updates.autoInstall || failures+="the auto install switch could not be clicked; "
wait_until 5 upd_is .auto_install true || failures+="the switch did not turn auto install on; "
setting_true updates.autoInstall || failures+="settings.json did not get updates.autoInstall true; "
[ "$(upd .banner)" = false ] || failures+="the banner stays with auto install on; "
tree_has update.banner.install && failures+="the banner button is still in the tree with auto install on; "
shot 003-auto-on
hclick settings.updates.autoInstall
wait_until 5 upd_is .auto_install false || failures+="the switch did not turn auto install off; "
[ "$(upd .banner)" = true ] || failures+="the banner did not come back with auto install off; "
tree_has update.banner.install || failures+="the banner button is missing with auto install off; "
shot 003-auto-off
check_failures "VAL-UPD-001 install switch and banner" "with the switch off the banner offers Download and install; on, it is gone and settings.json has updates.autoInstall true; off again, it returns" "$failures"

echo "== VAL-UPD-004 the same version shows up to date"
failures=""
feed_mode same
n0=$(feed_count)
current=$(upd .current)
[ "$(jq -r .version /harness/slot/update-feed/same/latest.json)" = "$current" ] || failures+="the same-version manifest is $(jq -r .version /harness/slot/update-feed/same/latest.json), the app runs $current; "
check_now || failures+="Check now could not be clicked; "
wait_until 20 upd_is .state up_to_date || failures+="the state is '$(upd .state)', not up_to_date; "
[ "$(tree_get settings.updates.status text)" = "Hushpen is up to date" ] || failures+="the status row says '$(tree_get settings.updates.status text)'; "
[ "$(upd .banner)" = false ] || failures+="the banner stays for the same version; "
[ "$(feed_paths "$n0")" = "/latest.json,/latest.json.minisig" ] || failures+="the check asked for '$(feed_paths "$n0")'; "
shot 004-up-to-date
$HC state | jq -c .updates >"$RUN_OUT/004-state.json"
feed_after "$n0" >"$RUN_OUT/004-feed.jsonl"
check_failures "VAL-UPD-004 up to date" "the manifest for $current ends in up_to_date, no banner; the feed saw only /latest.json and /latest.json.minisig" "$failures"

echo "== VAL-UPD-005 a tampered or wrongly signed manifest is rejected"
failures=""
for mode in tampered wrongkey; do
  feed_mode "$mode"
  n0=$(feed_count)
  check_now || failures+="$mode: Check now could not be clicked; "
  wait_until 20 upd_is .state error || failures+="$mode: the state is '$(upd .state)', not error; "
  [ "$(upd .error.code)" = UPDATE_SIGNATURE_INVALID ] || failures+="$mode: the code is '$(upd .error.code)'; "
  [ "$(upd .version)" = null ] || failures+="$mode: the view offers version '$(upd .version)'; "
  [ "$(upd .banner)" = false ] || failures+="$mode: a banner shows; "
  tree_has settings.updates.notes && failures+="$mode: a release notes link shows; "
  [ "$(tree_get settings.updates.status text)" = "The update could not be verified" ] || failures+="$mode: the status row says '$(tree_get settings.updates.status text)'; "
  case "$(tree_get settings.updates.status text)" in *UPDATE_*) failures+="$mode: the view shows the raw code; " ;; esac
  [ "$(feed_paths "$n0")" = "/latest.json,/latest.json.minisig" ] || failures+="$mode: the check asked for '$(feed_paths "$n0")'; "
  shot "005-$mode"
  $HC state | jq -c .updates >"$RUN_OUT/005-$mode-state.json"
done
check_failures "VAL-UPD-005 tampered and wrong key" "tampered manifest and a manifest signed by another key both end in error with UPDATE_SIGNATURE_INVALID, no version, no banner, no notes link, no artifact request" "$failures"

echo "== VAL-UPD-006 automatic checks run at most once per 24 h"
failures=""
reset_data
seed '{"updates.check": true}'
feed_mode newer
n0=$(feed_count)
start_feed_app || failures+="start 1 failed; "
wait_until 20 settled || failures+="start 1: no check ended; "
after1=$(manifest_count "$n0")
[ "$after1" = 1 ] || failures+="start 1 made $after1 manifest request(s); "
first_ms=$(feed_after "$n0" | jq -rs '[.[] | select(.path == "/latest.json")] | first | .t_ms')
for start in 2 3; do
  start_feed_app || failures+="start $start failed; "
  sleep 4
  [ "$(upd .state)" = idle ] || failures+="start $start: the state is '$(upd .state)'; "
done
[ "$(manifest_count "$n0")" = 1 ] || failures+="3 starts made $(manifest_count "$n0") manifest requests in total; "
open_updates || failures+="Settings > Updates did not open; "
shot 006-after-three-starts
check_now || failures+="Check now could not be clicked; "
wait_until 20 settled || failures+="Check now: no check ended; "
[ "$(manifest_count "$n0")" = 2 ] || failures+="Check now left $(manifest_count "$n0") manifest requests, want 2; "
second_ms=$(feed_after "$n0" | jq -rs '[.[] | select(.path == "/latest.json")] | last | .t_ms')
feed_after "$n0" | jq -c '{t_ms, path}' >"$RUN_OUT/006-feed-times.jsonl"
check_failures "VAL-UPD-006 once per 24 h" "3 starts made 1 manifest request (at $first_ms ms); Check now made one more at $second_ms ms" "$failures"

echo "== VAL-UPD-007 the request carries no install id or telemetry"
failures=""
machine_id=$(cat /etc/machine-id 2>/dev/null)
needles=$(jq -nc --arg host "$(hostname)" --arg mid "$machine_id" --arg user "$(id -un)" \
  --argjson stored "$(jq -c '[.values | .. | strings]' "$SETTINGS_FILE")" \
  '["/data", "/home", $host, $mid, $user] + $stored | map(select(length >= 4)) | unique')
echo "$needles" >"$RUN_OUT/007-needles.json"
jq -c . "$FEED_LOG" >"$RUN_OUT/007-feed-full.jsonl"
requests=$(jq -s 'map(select(.path | startswith("/latest.json"))) | length' "$FEED_LOG")
[ "$requests" -ge 4 ] || failures+="only $requests feed request(s) to look at; "
bad=$(jq -s '[.[] | select(.method != "GET" or (.path | contains("?")) or (.path | test("^/latest\\.json(\\.minisig)?$") | not))] | length' "$FEED_LOG")
[ "$bad" = 0 ] || failures+="$bad request(s) are not a plain GET of the manifest or its signature; "
cookies=$(jq -s '[.[] | select(.headers | with_entries(.key |= ascii_downcase) | has("cookie") or has("authorization"))] | length' "$FEED_LOG")
[ "$cookies" = 0 ] || failures+="$cookies request(s) carry a cookie or authorization header; "
leaks=$(jq -s --argjson needles "$needles" \
  '[.[] | .headers | to_entries[] | .value as $v | select($needles | any(. as $n | $v | contains($n)))] | length' "$FEED_LOG")
[ "$leaks" = 0 ] || failures+="$leaks header value(s) hold the data folder, the host, the user, the machine id or a stored setting; "
jq -s '[.[].headers | keys[]] | unique' "$FEED_LOG" >"$RUN_OUT/007-header-names.json"
check_failures "VAL-UPD-007 no identifiers" "$requests requests: all GET /latest.json or /latest.json.minisig, no query, no cookie or authorization, header names $(jq -c . "$RUN_OUT/007-header-names.json"), no header value contains $(jq -c . "$RUN_OUT/007-needles.json")" "$failures"

echo "== VAL-LIC-004 (Linux) the About rows call xdg-open"
failures=""
: >"$XDG_LOG"
focus "$(app_window)"
$HC action open-view '{"view":"about"}' >/dev/null
wait_until 10 view_is about || failures+="the About view did not open; "
wait_until 10 tree_has about.open.licenses || failures+="no licenses row; "
for target in data-folder log-folder github issues licenses; do
  hclick "about.open.$target" || failures+="no row for $target; "
  sleep 0.4
done
sleep 0.5
cp "$XDG_LOG" "$RUN_OUT/xdg-open-calls.txt" 2>/dev/null
[ "$(wc -l <"$XDG_LOG" | tr -d ' ')" = 5 ] || failures+="xdg-open was called $(wc -l <"$XDG_LOG" | tr -d ' ') times for 5 rows: $(tr '\n' ' ' <"$XDG_LOG"); "
grep -qxF /data "$XDG_LOG" || failures+="no xdg-open call for the data folder; "
grep -qxF /data/logs "$XDG_LOG" || failures+="no xdg-open call for the log folder; "
grep -q 'github.com/.*asrpro$' "$XDG_LOG" || failures+="no xdg-open call for the repository; "
grep -q '/issues$' "$XDG_LOG" || failures+="no xdg-open call for the issues page; "
grep -qxF /app/THIRD-PARTY-NOTICES.html "$XDG_LOG" || failures+="no xdg-open call for the licenses file; "
alive() { kill -0 "$(ui_pid)" 2>/dev/null; }
alive || failures+="the app died; "
s0=$(now_ms)
$HC state >/dev/null
[ $(($(now_ms) - s0)) -lt 1500 ] || failures+="the app was slow to answer after the clicks; "
check_failures "VAL-LIC-004 xdg-open" "5 rows, 5 xdg-open calls: $(tr '\n' ' ' <"$XDG_LOG")" "$failures"

echo "== VAL-UPD-009 a release build ignores the feed override"
failures=""
if [ ! -x /app/hushpen-release ]; then
  failures+="/app/hushpen-release is missing; build it with HUSHPEN_BUILD_RELEASE=1 harness/build-app.sh; "
else
  reset_data
  seed '{"updates.check": true}'
  n0=$(feed_count)
  feed_mode newer
  # The release build has no hook socket; the window is the only way in.
  env APP_BIN=/app/hushpen-release APP_STRACE="$RUN_OUT/upd-009.strace" \
    HUSHPEN_UPDATE_FEED="$FEED_URL" HUSHPEN_UPDATE_PUBKEY="$FEED_KEY" PATH="$STUB_BIN:$PATH" \
    /harness/slot/app.sh start >/dev/null || failures+="the release app did not start; "
  wait_until 20 app_window || failures+="the release app has no window; "
  ls /run/hook.sock >/dev/null 2>&1 && failures+="the release app opened a hook socket; "
  sleep 3
  wid=$(app_window)
  if [ -n "$wid" ]; then
    focus "$wid"
    read -r wx wy <<<"$(settled_origin "$wid")"
    click_rel() { read -r cx cy <<<"$1"; xdotool mousemove $((wx + cx)) $((wy + cy)) click 1; sleep 0.8; }
    click_rel "$SIDEBAR_SETTINGS_REL"
    click_rel "$SECTION_UPDATES_REL"
    shot 009-release-updates
    click_rel "$CHECK_NOW_REL"
    sleep 6
    shot 009-release-after-check-now
  fi
  [ "$(feed_count)" = "$n0" ] || failures+="the feed log grew from $n0 to $(feed_count) lines with the release build; "
  seen=$(connects "$RUN_OUT/upd-009.strace")
  case " $seen " in *" 127.0.0.1:"*) failures+="the release app connected to the local feed: $seen; " ;; esac
  echo "release app connects (the real GitHub feed is expected): ${seen:-none}"
  kill -0 "$(ui_pid)" 2>/dev/null || failures+="the release app died; "
  /harness/slot/app.sh stop
fi
check_failures "VAL-UPD-009 release ignores the feed" "release build, check on, HUSHPEN_UPDATE_FEED set to the local feed: window opened, no hook socket, Check now pressed, 0 feed requests, no connect to 127.0.0.1 (connects: ${seen:-none})" "$failures"

finish updates
