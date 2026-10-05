#!/bin/bash
# Container entry point of one validation slot. Brings up the desktop the app runs in:
#   Xvfb :99 -> D-Bus session -> openbox -> PulseAudio -> StatusNotifier host -> portal.
# Writes /tmp/env.sh for later `docker exec` calls, then /tmp/slot-ready, then sleeps.
# Logs go to /logs (the slot's logs folder on the data drive).
exec >/logs/init.log 2>&1
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/xdg
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

wait_for() { # <tries> <command...>   10 tries per second
  local tries=$1
  shift
  for _ in $(seq "$tries"); do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; done
  return 1
}

Xvfb :99 -screen 0 1280x800x24 -nolisten tcp +extension RANDR +extension RENDER +extension GLX \
  >/logs/xvfb.log 2>&1 &
wait_for 100 xdpyinfo || { echo "Xvfb did not start"; exit 1; }

eval "$(dbus-launch --sh-syntax)"
export DBUS_SESSION_BUS_ADDRESS

openbox --sm-disable >/logs/openbox.log 2>&1 &
wait_for 100 wmctrl -m || { echo "openbox did not start"; exit 1; }

# vmic is the virtual microphone: only audio played to it reaches vmic_src. cues is the default
# sink, so cue sounds the app plays never reach the microphone.
pulseaudio --daemonize=yes --exit-idle-time=-1 --disallow-exit -n \
  --load="module-native-protocol-unix" \
  --load="module-null-sink sink_name=vmic" \
  --load="module-virtual-source source_name=vmic_src master=vmic.monitor" \
  --load="module-null-sink sink_name=cues"
wait_for 100 pactl info || { echo "PulseAudio did not start"; exit 1; }
pactl set-default-source vmic_src
pactl set-default-sink cues

python3 /harness/slot/sni-host.py >/logs/sni-host.log 2>&1 &
wait_for 100 python3 /harness/slot/sni-host.py --query || echo "StatusNotifier host did not start"

# The portal is started by D-Bus on first use; ask for it once so a missing portal shows up here.
timeout 20 gdbus call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.DBus.Peer.Ping >/logs/portal.log 2>&1 || echo "portal did not answer"

cat >/tmp/env.sh <<EOF
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/xdg DBUS_SESSION_BUS_ADDRESS='$DBUS_SESSION_BUS_ADDRESS'
export LANG=C.UTF-8 RUST_LOG=info,zbus=warn,tracing=warn
export HUSHPEN_DATA_DIR=/data HUSHPEN_TESTHOOK_SOCKET=/run/hook.sock
EOF
echo "slot ready: $(wmctrl -m | head -1); sources: $(pactl list short sources | awk '{print $2}' | tr '\n' ' ')"
touch /tmp/slot-ready
exec sleep infinity
