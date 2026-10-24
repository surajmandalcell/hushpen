#!/bin/bash
# usage: compositor.sh start|stop|status   runs picom on the slot's X screen
# picom owns _NET_WM_CM_S0 while it runs, so the app turns see-through within a second of a
# start and back to solid within a second of a stop. The xrender backend needs no GL.
. /tmp/env.sh
case "${1:-}" in
  start)
    pgrep -x picom >/dev/null && exit 0
    picom --backend xrender --no-fading-openclose --shadow-exclude 'class_g = "flowbar"' \
      >/logs/picom.log 2>&1 &
    for _ in $(seq 50); do
      python3 - <<'EOF' && exit 0
import sys
from Xlib import display
x = display.Display()
owner = x.get_selection_owner(x.intern_atom("_NET_WM_CM_S0"))
sys.exit(0 if getattr(owner, "id", owner) else 1)
EOF
      sleep 0.1
    done
    echo "picom did not take _NET_WM_CM_S0" >&2
    exit 1
    ;;
  stop)
    pkill -x picom
    for _ in $(seq 50); do pgrep -x picom >/dev/null || exit 0; sleep 0.1; done
    echo "picom did not stop" >&2
    exit 1
    ;;
  status)
    pgrep -x picom >/dev/null && echo running || echo stopped
    ;;
  *)
    echo "usage: compositor.sh start|stop|status" >&2
    exit 2
    ;;
esac
