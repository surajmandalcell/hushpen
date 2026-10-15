#!/bin/sh
# Starts an AppImage under Xvfb with Mesa lavapipe and checks that a Hushpen
# window maps at 940x600 within 30 s and that the process stays up for 10 s.
#
# usage: appimage-xvfb.sh <AppImage> [screenshot.png]
# Needs xvfb-run, x11-utils (xwininfo), and mesa-vulkan-drivers. It runs the
# app with a throwaway HOME, so it leaves no data folder behind.
set -eu

if [ -z "${HUSHPEN_SMOKE_INNER:-}" ]; then
  # Same script again, inside the virtual display (and a session bus when there is one).
  export HUSHPEN_SMOKE_INNER=1
  if command -v dbus-run-session >/dev/null 2>&1; then
    exec xvfb-run -a -s "-screen 0 1280x800x24 -nolisten tcp" dbus-run-session -- "$0" "$@"
  fi
  exec xvfb-run -a -s "-screen 0 1280x800x24 -nolisten tcp" "$0" "$@"
fi

appimage=$1
shot=${2:-}

[ -x "$appimage" ] || chmod +x "$appimage"
home=$(mktemp -d)
trap 'rm -rf "$home"' EXIT

VK_ICD_FILENAMES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -n 1)
export VK_ICD_FILENAMES
export APPIMAGE_EXTRACT_AND_RUN=1
export HOME="$home" XDG_RUNTIME_DIR="$home/run" XDG_DATA_HOME="$home/data" XDG_CONFIG_HOME="$home/config"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

"$appimage" > "$home/app.log" 2>&1 &
pid=$!

i=0
while [ "$i" -lt 60 ]; do
  if xwininfo -root -tree | grep -E ' 940x600\+'; then
    break
  fi
  kill -0 "$pid" 2>/dev/null || { echo "the app exited early"; cat "$home/app.log"; exit 1; }
  i=$((i + 1))
  sleep 0.5
done
[ "$i" -lt 60 ] || { echo "no 940x600 window within 30 s"; cat "$home/app.log"; exit 1; }
echo "window mapped after about $((i / 2)) s"

sleep 10
kill -0 "$pid" || { echo "the app did not stay up for 10 s"; cat "$home/app.log"; exit 1; }
if [ -n "$shot" ] && command -v import >/dev/null 2>&1; then
  import -window root "$shot"
fi
kill "$pid"
echo "--- app log (last 30 lines)"
tail -n 30 "$home/app.log"
echo "appimage ok"
