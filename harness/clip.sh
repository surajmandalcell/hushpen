#!/bin/bash
# usage: clip.sh <slot> set <text> | get | clear | set-primary <text> | get-primary
# Reads and writes the X clipboard (CLIPBOARD) or the PRIMARY selection in the slot with xclip.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot=$1
action=${2:-}
text=${3:-}
case "$action" in
  set) slot_exec "$slot" sh -c 'printf "%s" "$1" | xclip -selection clipboard -i' _ "$text" ;;
  set-primary) slot_exec "$slot" sh -c 'printf "%s" "$1" | xclip -selection primary -i' _ "$text" ;;
  get) slot_exec "$slot" xclip -selection clipboard -o ;;
  get-primary) slot_exec "$slot" xclip -selection primary -o ;;
  clear) slot_exec "$slot" sh -c 'printf "" | xclip -selection clipboard -i' ;;
  *) die "usage: clip.sh <slot> set <text> | get | clear | set-primary <text> | get-primary" ;;
esac
