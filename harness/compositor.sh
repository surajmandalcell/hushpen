#!/bin/bash
# usage: compositor.sh <slot> start|stop|status   starts or stops picom in the slot
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot_exec "$1" /harness/slot/compositor.sh "${2:-status}"
