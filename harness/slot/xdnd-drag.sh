#!/bin/bash
# usage: xdnd-drag.sh <x> <y> <file>...      inside a slot, through with-env.sh
# A real XDND drag, as from a file manager: starts drag-source.py with the files, presses the
# pointer on its window, moves it in steps to the absolute point x,y, holds it there, and lets go.
# DRAG_SHOT=<name> saves a screenshot while the pointer hovers. Exits 2 when the drag source cannot
# run (python3 GTK missing or no window), so a suite can skip the check instead of failing it.
. /harness/slot/lib.sh

[ $# -ge 3 ] || { echo "usage: xdnd-drag.sh <x> <y> <file>..." >&2; exit 64; }
x=$1 y=$2
shift 2
python3 -c 'import gi; gi.require_version("Gtk", "3.0"); from gi.repository import Gtk' 2>/dev/null ||
  { echo "the drag source needs python3 with GTK 3 (python3-gi, gir1.2-gtk-3.0)"; exit 2; }
python3 /harness/slot/drag-source.py "$@" >>"$RUN_OUT/drag-source.log" 2>&1 &
source_pid=$!
trap 'kill "$source_pid" 2>/dev/null; wait "$source_pid" 2>/dev/null' EXIT
wid=""
for _ in $(seq 50); do
  wid=$(xdotool search --onlyvisible --name '^drag-source$' 2>/dev/null | head -1)
  [ -n "$wid" ] && break
  sleep 0.1
done
[ -n "$wid" ] || { echo "the drag source showed no window; see $RUN_OUT/drag-source.log"; exit 2; }
read -r sx sy <<<"$(settled_origin "$wid")"
size=$(win_size "$wid")
sx=$((sx + ${size%x*} / 2))
sy=$((sy + ${size#*x} / 2))

xdotool mousemove "$sx" "$sy"
sleep 0.3
xdotool mousedown 1
sleep 0.3
# GTK starts the drag only after the pointer passes its threshold, and the target answers each
# XdndPosition, so the move goes in small steps with a pause between them.
for i in $(seq 20); do
  xdotool mousemove $((sx + (x - sx) * i / 20)) $((sy + (y - sy) * i / 20))
  sleep 0.05
done
xdotool mousemove $((x + 2)) $((y + 2))
sleep 0.8
[ -z "${DRAG_SHOT:-}" ] || shot "$DRAG_SHOT"
xdotool mouseup 1
sleep 0.5
