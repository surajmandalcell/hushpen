#!/bin/bash
# Runs inside a stock distro container (started by packaging/smoke/update-notice-in-container.sh):
# installs the rc.1 deb or rpm from /update-test with the tools of a slot, then runs the notice
# check of VAL-AUP-005 under Xvfb. KIND (deb or rpm) and LABEL come from the environment. The
# harness folder is /harness, the fixtures /update-test, and the results go to /out/<label>.
set -eu

if [ -z "${NOTICE_INNER:-}" ]; then
  pkg=$(ls /update-test/rc1/Hushpen-*."$KIND" | head -n 1)
  cp "$pkg" "/tmp/hushpen.$KIND"
  chmod 644 "/tmp/hushpen.$KIND"
  if [ "$KIND" = deb ]; then
    apt-get update -qq
    DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb xauth x11-utils xdotool imagemagick \
      python3 jq procps psmisc mesa-vulkan-drivers dbus dbus-x11 "/tmp/hushpen.$KIND" >/tmp/install.log 2>&1
  else
    dnf install -y -q xorg-x11-server-Xvfb xorg-x11-xauth xwininfo xdotool ImageMagick python3 jq \
      procps-ng psmisc mesa-vulkan-drivers dbus-daemon dbus-x11 "/tmp/hushpen.$KIND" >/tmp/install.log 2>&1
  fi || { tail -20 /tmp/install.log; exit 1; }
  [ "$(cat /usr/lib/hushpen/install-kind)" = "$KIND" ] || { echo "install-kind is not $KIND"; exit 1; }
  export NOTICE_INNER=1
  exec xvfb-run -a -s "-screen 0 1280x800x24 -nolisten tcp" dbus-run-session -- "$0" "$@"
fi

set +e
mkdir -p /data /logs "/out/$LABEL"
export RUN_OUT=/out/$LABEL HUSHPEN_DATA_DIR=/data XDG_RUNTIME_DIR=/tmp/xdg SLOT="$LABEL"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
VK_ICD_FILENAMES=$(ls /usr/share/vulkan/icd.d/lvp_icd*.json | head -n 1)
export VK_ICD_FILENAMES
. /harness/slot/lib.sh
. /harness/slot/selfupdate-lib.sh
: >"$CHECKS"
cat /etc/os-release | head -2 >"$RUN_OUT/os-release.txt"
stub_xdg_open
rm -f "$FEED_LOG"
feed_start good || echo "the feed did not start"
notice_check "$LABEL" "$KIND" /usr/bin/hushpen /usr/lib/hushpen/hushpen
feed_stop
finish "notice-$LABEL"
