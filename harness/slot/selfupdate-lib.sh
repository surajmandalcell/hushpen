#!/bin/bash
# Helpers for the self-update proofs: the installed release test builds have no test hook, so the
# state comes from hushpen.log ("updates: state ..." lines and the STARTUP line), the files on
# disk, the feed log, and screenshots; buttons are pressed with click-accent.py. Source it after
# lib.sh and dictate-lib.sh. /update-test comes from harness/update-fixtures.sh.

FEED_PORT=8765
FEED_URL=http://127.0.0.1:$FEED_PORT/latest.json
FEED_KEY=/update-test/test.pub
FEED_LOG=$RUN_OUT/feed-requests.jsonl
FEED_PID=/tmp/feed.pid
FEED_MODE=/tmp/feed-mode
FEED_SLOW=/tmp/feed-slow
SETTINGS_FILE=/data/config/settings.json
APP_LOG=/data/logs/hushpen.log
HELPER_LOG=/data/logs/update-apply.log
STUB_BIN=/tmp/stubbin
XDG_LOG=/tmp/xdg-open.log
INSTALL_DIR=/opt/hushpen-test
INSTALLED=$INSTALL_DIR/Hushpen.AppImage
STAGED=$INSTALL_DIR/.Hushpen.AppImage.update/staged.AppImage
export APPIMAGE_EXTRACT_AND_RUN=1

sha() { sha256sum "$1" | cut -d' ' -f1; }
rc1_appimage() { ls /update-test/rc1/Hushpen-*.AppImage | head -1; }
rc2_appimage() { ls /update-test/rc2/Hushpen-*.AppImage | head -1; }
version_of() { "$1" --version 2>&1 | head -1; }

feed_up() { python3 -c 'import socket; socket.create_connection(("127.0.0.1", 8765), 1).close()' 2>/dev/null; }
feed_stop() {
  [ -f "$FEED_PID" ] || return 0
  kill "$(cat "$FEED_PID")" 2>/dev/null
  rm -f "$FEED_PID"
}
feed_start() { # <mode>
  echo "$1" >"$FEED_MODE"
  python3 /harness/slot/update-feed-server.py "$FEED_LOG" "$FEED_PORT" >/dev/null 2>&1 &
  echo $! >"$FEED_PID"
  wait_until 5 feed_up
}
feed_mode() { echo "$1" >"$FEED_MODE"; }
feed_count() { if [ -s "$FEED_LOG" ]; then wc -l <"$FEED_LOG" | tr -d ' '; else echo 0; fi; }
# artifact_requests <from>: requests after the first <from> log lines for anything but the manifest
artifact_requests() {
  if [ -s "$FEED_LOG" ]; then
    tail -n +$(($1 + 1)) "$FEED_LOG" |
      jq -s '[.[] | select(.path != "/latest.json" and .path != "/latest.json.minisig")] | length'
  else echo 0; fi
}

stub_xdg_open() {
  mkdir -p "$STUB_BIN"
  printf '#!/bin/sh\nprintf "%%s\\n" "$*" >>%s\n' "$XDG_LOG" >"$STUB_BIN/xdg-open"
  chmod +x "$STUB_BIN/xdg-open"
  : >"$XDG_LOG"
}

stop_apps() {
  pkill -f 'hushpen update-apply' 2>/dev/null
  pkill -x hushpen 2>/dev/null
  for _ in $(seq 50); do pgrep -x hushpen >/dev/null || break; sleep 0.1; done
  pkill -9 -x hushpen 2>/dev/null
  return 0
}
reset_data() {
  stop_apps
  find /data -mindepth 1 -delete
  mkdir -p /data/config
}
# seed <settings json object>: a finished onboarding plus these values
seed() {
  mkdir -p /data/config
  jq -n --argjson v "$1" '{schemaVersion: 1, values: ({"onboarding.completed": true} + $v)}' >"$SETTINGS_FILE"
}
# install_rc1: a fresh rc.1 AppImage at $INSTALLED, with nothing else in its folder
install_rc1() {
  find "$INSTALL_DIR" -mindepth 1 -delete 2>/dev/null
  mkdir -p "$INSTALL_DIR"
  cp "$(rc1_appimage)" "$INSTALLED"
  chmod +x "$INSTALLED"
}
# launch <binary> [VAR=value...]: the build with the test feed, the test key, and the stub xdg-open
launch() {
  local bin=$1
  shift
  env "$@" HUSHPEN_UPDATE_FEED="$FEED_URL" HUSHPEN_UPDATE_PUBKEY="$FEED_KEY" PATH="$STUB_BIN:$PATH" \
    "$bin" >>/logs/app-installed.log 2>&1 &
  wait_until 60 main_window_id
}
starts_logged() { grep -c 'STARTUP version=' "$APP_LOG" 2>/dev/null || true; }
version_logged() { grep -h 'STARTUP version=' "$APP_LOG" 2>/dev/null | tail -1 | sed -E 's/.*STARTUP version=([^ ]*).*/\1/'; }
# state_logged <state text>: the log has "updates: state <text>" (a prefix match)
state_logged() { grep -q "updates: state $1" "$APP_LOG" 2>/dev/null; }
state_lines() { grep -h 'updates: ' "$APP_LOG" 2>/dev/null | sed -E 's/^([^ ]+) [A-Z]+ [^ ]+ /\1 /'; }
click_banner() { python3 /harness/slot/click-accent.py "$(main_window_id)"; }

# notice_check <label> <install kind> <binary to run> <file whose sha256 must not change>
# VAL-AUP-005 for one install: the check and the automatic install are on, the feed offers a newer
# version with a real artifact, and the app shows the notice only. Records one check.
notice_check() {
  local label=$1 kind=$2 bin=$3 hashed=$4 failures="" before after from lines open
  reset_data
  seed '{"updates.check": true, "updates.autoInstall": true}'
  : >"$XDG_LOG"
  before=$(sha "$hashed")
  feed_mode good
  from=$(feed_count)
  launch "$bin" || failures+="the app did not open a window; "
  wait_until 40 state_logged available || failures+="no 'state available' in the log; "
  lines=$(state_lines)
  printf '%s\n' "$lines" >"$RUN_OUT/$label-states.txt"
  echo "$lines" | grep -q "state available install_kind=$kind delivery=notice" ||
    failures+="the offer is not 'install_kind=$kind delivery=notice' (log: $(echo "$lines" | tr '\n' '|')); "
  sleep 3
  shot "$label-notice"
  state_logged downloading && failures+="the state went to downloading; "
  click_banner >"$RUN_OUT/$label-click.txt" 2>&1 || failures+="no button in the banner: $(cat "$RUN_OUT/$label-click.txt"); "
  wait_until 10 test -s "$XDG_LOG"
  open=$(head -1 "$XDG_LOG")
  case "$open" in
    https://github.com/surajmandalcell/asrpro/releases/tag/*) ;;
    *) failures+="the banner button opened '$open', not the release page; " ;;
  esac
  sleep 5
  jq -c '[.t_ms, .path]' "$FEED_LOG" >"$RUN_OUT/$label-feed.txt" 2>/dev/null
  [ "$(artifact_requests "$from")" = 0 ] || failures+="the feed log has $(artifact_requests "$from") artifact request(s); "
  after=$(sha "$hashed")
  [ "$before" = "$after" ] || failures+="$(basename "$hashed") changed: $before to $after; "
  echo "$before" >"$RUN_OUT/$label-sha-before.txt"
  echo "$after" >"$RUN_OUT/$label-sha-after.txt"
  stop_apps
  check_failures "VAL-AUP-005 $label notice only" "$label install (install-kind $kind): the offer is logged as delivery=notice, the banner button opened $open, 0 artifact requests, $(basename "$hashed") keeps sha256 ${after:0:16}; screenshot $label-notice.png" "$failures"
}
starts_are() { [ "$(starts_logged)" = "$1" ]; }
artifacts_since() { [ "$(artifact_requests "$1")" -ge 1 ]; }
