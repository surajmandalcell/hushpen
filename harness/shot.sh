#!/bin/bash
# usage: shot.sh <slot> [name]      screenshot of the slot's screen; prints the PNG path on the data drive
# Lands in slots/<slot>/out/${HUSHPEN_RUN_ID:-adhoc}/<name>.png
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot=$1
name=${2:-shot-$(date +%H%M%S)}
slot_exec "$slot" /harness/slot/shot.sh "$name" >/dev/null || die "screenshot failed"
echo "$(slot_run_dir "$slot")/$name.png"
