#!/bin/sh
# usage: with-env.sh <command> [args...]
# Runs a command inside a slot with the desktop environment that init.sh wrote. HUSHPEN_RUN_ID
# (set by the host scripts through docker exec -e) names the output folder, /out/<run-id>.
. /tmp/env.sh
RUN_OUT=/out/${HUSHPEN_RUN_ID:-adhoc}
export RUN_OUT
mkdir -p "$RUN_OUT"
exec "$@"
