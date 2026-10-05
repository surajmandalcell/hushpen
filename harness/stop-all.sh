#!/bin/bash
# usage: stop-all.sh      removes hushpen-val-1 to hushpen-val-6 by exact name, then lists what is left
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

for slot in $(seq "$SLOT_MIN" "$SLOT_MAX"); do
  slot_exists "$slot" && docker rm -f "$(slot_name "$slot")" >/dev/null && echo "$(slot_name "$slot") removed"
done
left=$(docker ps -a --filter name=hushpen-val- --format '{{.Names}}')
[ -z "$left" ] || die "still present: $left"
exit 0
