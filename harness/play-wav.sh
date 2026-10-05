#!/bin/bash
# usage: play-wav.sh <slot> <wav> [--lead-ms N] [--device NAME]
# Pads the WAV with 1 s of lead silence (N ms with --lead-ms) and plays it into the virtual
# microphone (device vmic). Capture loses the start of the audio otherwise. Pass --device cues to
# play to the cue sink instead. The padded copy is kept under slots/<slot>/out/<run id>/padded/.
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

need_running "${1:-}"
slot=$1
shift
[ $# -gt 0 ] || die "usage: play-wav.sh <slot> <wav> [--lead-ms N] [--device NAME]"
wav=$1
shift
slot_exec "$slot" /harness/slot/play-wav.sh "$(to_slot_path "$slot" "$wav")" "$@"
