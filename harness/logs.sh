#!/bin/bash
# usage: logs.sh <slot> [name [-f]]
# Prints a log of slot <slot> from the data drive. Without a name it lists the logs. Names: app,
# init, xvfb, openbox, sni-host, portal, xterm, ... (file names in slots/<slot>/logs without .log).
# -f follows the log. Works after the slot is gone.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_slot "${1:-}"
dir=$(slot_dir "$1")/logs
name=${2:-}
[ -d "$dir" ] || die "no logs for slot $1 ($dir)"
if [ -z "$name" ]; then
  ls "$dir"
elif [ "${3:-}" = "-f" ]; then
  exec tail -n 50 -f "$dir/$name.log"
else
  cat "$dir/$name.log"
fi
