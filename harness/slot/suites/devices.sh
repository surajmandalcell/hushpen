#!/bin/bash
# Devices navigation, saved intents, authenticated pairing, and QR cancellation.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh
set +x
umask 077

HOST_DATA=$RUN_OUT/devices-hosts
HOST_PIDS=()
cleanup() {
  local pid
  for pid in "${HOST_PIDS[@]}"; do kill -TERM -- "-$pid" 2>/dev/null || true; done
  for pid in "${HOST_PIDS[@]}"; do
    for _ in $(seq 30); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
    kill -KILL -- "-$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
  done
  rm -rf "$HOST_DATA"
}
trap cleanup EXIT

setup_failed() { check "devices setup" fail "$1"; exit 0; }
device_action() { hook_action devices-action "$1" >/dev/null; }
device_state() { $HC state | jq -er ".devices | $1"; }
device_is() { [ "$(device_state "$1")" = "$2" ]; }
snapshot() { $HC state | jq '{view,devices}' >"$RUN_OUT/$1.json"; shot "$1"; }
reveal() {
  local id=$1 last= unchanged=0 x y top bottom limit_top limit_bottom bounds direction attempt tree
  for attempt in $(seq 10); do
    tree=$($HC tree)
    read -r top bottom x y limit_top limit_bottom <<<"$(jq -r --arg id "$id" \
      'map({key:.id,value:.root_bounds}) | from_entries | .[$id] as $target | .["content.scroll"] as $scroll | select($scroll != null) | "\(($target.y // -1) | floor) \((if $target == null then $scroll.y + $scroll.height + 1 else $target.y + $target.height end) | ceil) \(($scroll.x + $scroll.width / 2) | floor) \(($scroll.y + $scroll.height / 2) | floor) \($scroll.y | ceil) \(($scroll.y + $scroll.height) | floor)"' <<<"$tree")"
    [ -n "$top" ] || return 1
    [ "$top" -ge "$limit_top" ] && [ "$bottom" -le "$limit_bottom" ] && return 0
    bounds=$(jq -c '[.[] | select(.id | startswith("devices.")) | [.id,.root_bounds.y]]' <<<"$tree")
    if [ "$bounds" = "$last" ]; then unchanged=$((unchanged + 1)); else unchanged=0; fi
    [ "$unchanged" -lt 2 ] || return 1
    last=$bounds
    if [ "$bottom" -gt "$limit_bottom" ]; then direction=5; else direction=4; fi
    xdotool mousemove "$x" "$y" click --repeat 3 --delay 40 "$direction"
    sleep 0.3
    snapshot "devices-scroll-$id-$attempt"
  done
  return 1
}
real_click() {
  local x y
  case "$1" in sidebar.*) ;; *) reveal "$1" || return 1 ;; esac
  read -r x y <<<"$($HC tree | jq -r --arg id "$1" \
    '[.[] | select(.id == $id) | "\((.root_bounds.x + .root_bounds.width / 2) | floor) \((.root_bounds.y + .root_bounds.height / 2) | floor)"] | first // empty')"
  [ -n "$x" ] && [ -n "$y" ] || return 1
  xdotool mousemove "$x" "$y" click 1
  sleep 0.3
}
type_into() {
  real_click "$1" || return 1
  xdotool windowfocus --sync "$(app_window)"
  xdotool key ctrl+a
  xdotool type --delay 2 -- "$2"
  sleep 0.2
}
pair_host() {
  local number=$1 id=$2 name=$3 method=${4:-lan} port=$((47330 + $1)) data offer args pid
  data=$HOST_DATA/$number
  mkdir -p "$data"
  ss -ltn | grep -q ":$port " && return 1
  offer=$(/app/hushpen-host pair --json --data-dir "$data" --listen "127.0.0.1:$port" --host 127.0.0.1 | jq -er .offer) || return 1
  setsid timeout --kill-after=3 300 /app/hushpen-host --data-dir "$data" \
    --listen "127.0.0.1:$port" --host 127.0.0.1 --name "$name" \
    >"$data/stdout.log" 2>"$data/stderr.log" &
  pid=$!
  HOST_PIDS+=("$pid")
  wait_until 10 grep -q 'listening on' "$data/stderr.log" || return 1
  args=$(jq -nc --arg offer "$offer" --arg id "$id" --arg name "$name" --arg transport "$method" \
    '{op:"pair",offer:$offer,id:$id,name:$name,os:"linux",transport:$transport}')
  device_action "$args" || return 1
  unset offer args
  wait_until 15 device_is '.busy' false || return 1
  device_state ".devices | any(.id == \"$id\")" >/dev/null
}

[ -x /app/hushpen-host ] || setup_failed "the host program is missing"
fresh || setup_failed "the debug desktop did not start"
real_click sidebar.devices || setup_failed "the Devices sidebar control could not be clicked"
$HC wait view=devices 5000 >/dev/null || setup_failed "Devices did not open"
snapshot devices-default
failures=""
device_is '.enabled' false || failures+="remote use is not off by default; "
device_is '.noLocalModels' false || failures+="local models are disabled by default; "
real_click devices.enabled || failures+="the remote switch could not be clicked; "
snapshot devices-enabled
real_click devices.noLocal || failures+="the local-model switch could not be clicked; "
snapshot devices-no-local
/harness/slot/app.sh restart >/dev/null || setup_failed "the desktop restart failed"
wait_until 10 device_is '.enabled' true || failures+="remote use was not saved; "
device_is '.noLocalModels' true || failures+="the local-model choice was not saved; "
snapshot devices-restarted
check_failures "devices navigation and switches" "real clicks opened Devices and both switches survived a restart" "$failures"

device_action '{"op":"noLocal","value":false}' || setup_failed "local fallback could not be restored"
snapshot devices-local-restored
pair_host 1 phase2-one 'First host' || setup_failed "the first Devices pairing did not finish"
snapshot devices-paired-one
pair_host 2 phase2-two 'Second host' || setup_failed "the second Devices pairing did not finish"
snapshot devices-paired-two
failures=""
device_is '.devices | length' 2 || failures+="both authenticated hosts were not saved; "
for number in 1 2; do
  jq -e '.pairedClients | length == 1' "$HOST_DATA/$number/config/secrets.json" >/dev/null ||
    failures+="host $number did not save the desktop public pin; "
done
check_failures "devices authenticated pairing" "the Devices service paired two hosts and saved public pins on both sides" "$failures"

failures=""
before=$(device_state '.routes.cleanup | join(",")')
device_action '{"op":"move","kind":"speech","id":"phase2-one","up":true}' || failures+="speech move failed; "
snapshot devices-reordered
device_is '.routes.speech | join(",")' 'phase2-one,phase2-two,this-computer' || failures+="speech order did not change; "
device_is '.routes.cleanup | join(",")' "$before" || failures+="moving speech also moved cleanup; "
device_action '{"op":"move","kind":"speech","id":"this-computer","up":true}' || failures+="local move intent failed; "
snapshot devices-local-fixed
device_is '.routes.speech | last' this-computer || failures+="This computer moved from last; "
check_failures "devices independent routes" "speech moved independently and This computer stayed last" "$failures"

failures=""
real_click devices.device.phase2-one.info || failures+="details could not be clicked; "
snapshot devices-details
device_is '.sheet' detail || failures+="details did not open; "
device_action '{"op":"rename","id":"phase2-one","name":"Renamed host"}' || failures+="rename failed; "
snapshot devices-renamed
device_action '{"op":"unload","id":"phase2-one","rule":"afterIdle","minutes":15}' || failures+="idle rule failed; "
snapshot devices-unload
/harness/slot/app.sh restart >/dev/null || setup_failed "the details restart failed"
wait_until 10 device_is '.devices[] | select(.id == "phase2-one") | .name' 'Renamed host' || failures+="rename was not saved; "
device_is '.devices[] | select(.id == "phase2-one") | .unloadRule.minutes' 15 || failures+="idle time was not saved; "
snapshot devices-edits-restarted
device_action '{"op":"forget","id":"phase2-one"}' || failures+="forget failed; "
snapshot devices-forgotten
device_is '.devices | length' 1 || failures+="the forgotten device stayed saved; "
device_state '.routes | [.speech[], .cleanup[]] | all(. != "phase2-one")' >/dev/null || failures+="the forgotten device stayed in a route; "
check_failures "devices saved details" "rename and idle time survived a restart; forgetting removed the device from both routes" "$failures"

failures=""
device_action '{"op":"add"}' || failures+="Add device failed; "
snapshot devices-add
device_action '{"op":"offer","host":"127.0.0.1"}' || failures+="QR offer did not start; "
wait_until 10 device_is '.pairing' offered || failures+="QR offer did not become ready; "
reveal devices.qr || failures+="the QR could not scroll fully into view; "
snapshot devices-qr
if command -v zbarimg >/dev/null; then
  decoded=$(zbarimg --quiet --raw "$RUN_OUT/devices-qr.png" 2>/dev/null)
  jq -e 'type == "object" and .version == 2 and (.host | type == "string") and (.port | type == "number" and . > 0 and . <= 65535) and (.key | type == "string" and length == 44) and (.token | type == "string" and length == 44) and (.expires | type == "number" and . > now) and (has("code") | not)' <<<"$decoded" >/dev/null || failures+="the visible QR did not decode to a key-pinning payload; "
  unset decoded
fi
real_click devices.close || failures+="the QR close control could not be clicked; "
wait_until 5 device_is '.pairing' idle || failures+="closing left the pairing offer active; "
device_is '.sheet' closed || failures+="closing left Add device open; "
snapshot devices-qr-closed
check_failures "devices pairing QR" "the QR offer was visible and closing Add device cancelled it" "$failures"

failures=""
pair_host 3 tailscale-127.0.0.1-47333 'Address host' tailscale || failures+="address validation or pairing failed; "
snapshot devices-address-paired
device_is '.devices[] | select(.id == "tailscale-127.0.0.1-47333") | .address' '127.0.0.1:47333' || failures+="the typed address was not saved; "
device_is '.devices[] | select(.id == "tailscale-127.0.0.1-47333") | .transport.kind' tailscale || failures+="the transport was not saved; "
check_failures "devices address pairing" "the address from the protected bootstrap paired over Noise and saved its transport" "$failures"
