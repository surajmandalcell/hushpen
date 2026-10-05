#!/bin/bash
# usage: stop.sh <slot>...      removes hushpen-val-<slot> (slots are 1 to 6); other containers are never touched
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

[ $# -gt 0 ] || die "usage: stop.sh <slot>..."
for slot in "$@"; do
  need_slot "$slot"
  if slot_exists "$slot"; then
    docker rm -f "$(slot_name "$slot")" >/dev/null && echo "$(slot_name "$slot") removed"
  fi
done
