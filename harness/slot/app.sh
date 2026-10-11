#!/bin/bash
# usage: app.sh start|stop|restart|adopt
# Runs the debug app (built with test-automation) from /app/hushpen. The data folder is /data and
# the test hook socket is /run/hook.sock (both set by init.sh through /tmp/env.sh). Log: /logs/app.log.
# Start-time overrides for the suites that need them:
#   APP_DATA_DIR=/some/dir  - HUSHPEN_DATA_DIR points there instead of /data
#   APP_NO_DATA_DIR=1       - HUSHPEN_DATA_DIR is unset, so the app uses its default folder
#   APP_NO_WINDOW=1         - do not wait for a mapped window (start hidden)
# A Change-data-folder restart replaces the process without app.sh; `adopt` writes the new
# process id into the pidfile so the next stop finds it.
pidfile=/tmp/app.pid

stop() {
  [ -f "$pidfile" ] || return 0
  pid=$(cat "$pidfile")
  kill "$pid" 2>/dev/null
  for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  kill -9 "$pid" 2>/dev/null
  rm -f "$pidfile" /run/hook.sock
}

# The config folder of the app about to start, given the data-dir overrides.
config_dir() {
  if [ -n "${APP_DATA_DIR:-}" ]; then
    echo "$APP_DATA_DIR/config"
  elif [ "${APP_NO_DATA_DIR:-}" = 1 ]; then
    if [ -n "${XDG_DATA_HOME:-}" ]; then echo "$XDG_DATA_HOME/hushpen/config";
    else echo "$HOME/.local/share/hushpen/config"; fi
  else
    echo /data/config
  fi
}

# A fresh data folder opens onboarding, which covers the window every other suite drives. Those
# suites start with onboarding marked finished; the onboarding suite sets HARNESS_ONBOARDING=fresh.
seed_onboarding_done() {
  [ "${HARNESS_ONBOARDING:-}" = fresh ] && return 0
  local config
  config=$(config_dir)
  [ -e "$config/settings.json" ] && return 0
  mkdir -p "$config"
  printf '%s\n' '{"schemaVersion": 1, "values": {"onboarding.completed": true}}' >"$config/settings.json"
}

start() {
  stop
  [ -x /app/hushpen ] || { echo "app: /app/hushpen is missing; run harness/build-app.sh" >&2; return 1; }
  seed_onboarding_done
  local begin
  begin=$(date +%s%3N)
  if [ "${APP_NO_DATA_DIR:-}" = 1 ]; then
    env -u HUSHPEN_DATA_DIR /app/hushpen >/logs/app.log 2>&1 &
  elif [ -n "${APP_DATA_DIR:-}" ]; then
    env HUSHPEN_DATA_DIR="$APP_DATA_DIR" /app/hushpen >/logs/app.log 2>&1 &
  else
    /app/hushpen >/logs/app.log 2>&1 &
  fi
  echo $! >"$pidfile"
  if [ "${APP_NO_WINDOW:-}" = 1 ]; then
    for _ in $(seq 100); do
      [ -S /run/hook.sock ] && break
      kill -0 "$(cat "$pidfile")" 2>/dev/null || { echo "app: exited early; see /logs/app.log" >&2; return 1; }
      sleep 0.1
    done
    [ -S /run/hook.sock ] || { echo "app: no hook socket after 10 s" >&2; return 1; }
    echo "app started (no window expected)"
    return 0
  fi
  local wid=""
  for _ in $(seq 150); do
    wid=$(xdotool search --onlyvisible --class hushpen 2>/dev/null | head -1)
    [ -n "$wid" ] && break
    kill -0 "$(cat "$pidfile")" 2>/dev/null || { echo "app: exited early; see /logs/app.log" >&2; return 1; }
    sleep 0.1
  done
  [ -n "$wid" ] || { echo "app: no window after 15 s" >&2; return 1; }
  echo "app wid=$wid map_ms=$(( $(date +%s%3N) - begin ))"
}

# The app restarted itself (Change data folder): point the pidfile at the running process.
adopt() {
  local pid
  pid=$(pgrep -x hushpen | head -1)
  [ -n "$pid" ] || { echo "app: no hushpen process to adopt" >&2; return 1; }
  echo "$pid" >"$pidfile"
  echo "app adopted pid=$pid"
}

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  restart) start ;;
  adopt) adopt ;;
  *) echo "usage: app.sh start|stop|restart|adopt" >&2; exit 2 ;;
esac
