#!/bin/bash
# usage: play-wav.sh <wav> [--lead-ms N] [--device NAME]
# Pads the WAV with N ms of silence (default 1000), then plays it with paplay. The default device
# is vmic, the virtual microphone. Prints the padded file path, the lead, and the duration.
wav=${1:?usage: play-wav.sh <wav> [--lead-ms N] [--device NAME]}
shift
lead_ms=1000
device=vmic
while [ $# -gt 0 ]; do
  case "$1" in
    --lead-ms) lead_ms=$2; shift 2 ;;
    --device) device=$2; shift 2 ;;
    *) echo "play-wav: unknown argument $1" >&2; exit 2 ;;
  esac
done
mkdir -p "$RUN_OUT/padded"
padded="$RUN_OUT/padded/$(basename "$wav")"
python3 /harness/slot/pad-wav.py "$wav" "$padded" "$lead_ms" || exit 1
paplay --device="$device" "$padded"
