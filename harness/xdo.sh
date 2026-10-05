#!/bin/bash
# usage: xdo.sh <slot> <xdotool arguments...>
# Hold keys use the keycode form: `xdo.sh 1 keydown 108` / `keyup 108` for Right Alt (Right Ctrl
# is 105, Right Super 134). The keysym form (`keydown Alt_R`) also presses the left key, so a
# detector would see a chord.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot=$1
shift
[ $# -gt 0 ] || die "usage: xdo.sh <slot> <xdotool arguments...>"
slot_exec "$slot" xdotool "$@"
