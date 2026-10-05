#!/bin/bash
# usage: app.sh start|stop|restart
# Runs the debug app (built with test-automation) from /app/hushpen. The data folder is /data and
# the test hook socket is /run/hook.sock (both set by init.sh through /tmp/env.sh). Log: /logs/app.log.
pidfile=/tmp/app.pid

stop() {
  [ -f "$pidfile" ] || return 0
  pid=$(cat "$pidfile")
  kill "$pid" 2>/dev/null
  for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
  kill -9 "$pid" 2>/dev/null
  rm -f "$pidfile" /run/hook.sock
}

start() {
  stop
  [ -x /app/hushpen ] || { echo "app: /app/hushpen is missing; run harness/build-app.sh" >&2; return 1; }
  local begin
  begin=$(date +%s%3N)
  /app/hushpen >/logs/app.log 2>&1 &
  echo $! >"$pidfile"
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

case "${1:-}" in
  start) start ;;
  stop) stop ;;
  restart) start ;;
  *) echo "usage: app.sh start|stop|restart" >&2; exit 2 ;;
esac
