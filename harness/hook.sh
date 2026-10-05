#!/bin/bash
# usage: hook.sh <slot> <hookctl arguments...>
# Examples: hook.sh 1 tree | hook.sh 1 state | hook.sh 1 click sidebar.settings |
#           hook.sh 1 wait pipeline.state=idle 5000 | hook.sh 1 action open-view '{"view":"history"}'
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot=$1
shift
[ $# -gt 0 ] || die "usage: hook.sh <slot> <hookctl arguments...>"
slot_exec "$slot" /app/hookctl "$@"
