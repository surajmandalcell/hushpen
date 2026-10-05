#!/bin/bash
# usage: shot.sh [name]   screenshot of the whole X screen into $RUN_OUT/<name>.png
# The path printed is the path inside the slot; the host script maps it to the data drive.
name=${1:-shot-$(date +%H%M%S)}
import -window root "$RUN_OUT/$name.png" && echo "$RUN_OUT/$name.png"
