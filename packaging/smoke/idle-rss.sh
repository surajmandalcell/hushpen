#!/bin/bash
# Runs inside a harness slot (through with-env.sh). Starts a release AppImage with a finished
# onboarding and no model, lets it sit idle on Home, and prints the memory of the UI process and
# of its children: VmRSS (what `ps` shows), RssAnon, and PSS. The slot renders with Mesa lavapipe
# on the CPU, so the number includes the software renderer's buffers and is an upper bound for a
# machine with a GPU.
#
# usage: idle-rss.sh <AppImage> [idle seconds]       env: RSS_TARGET_MB (default 150)
. /harness/slot/lib.sh

appimage=$1
idle=${2:-30}
target=${RSS_TARGET_MB:-150}
out=$RUN_OUT

/harness/slot/app.sh stop
find /data -mindepth 1 -delete
mkdir -p /data/config
printf '%s\n' '{"schemaVersion": 1, "values": {"onboarding.completed": true}}' >/data/config/settings.json
chmod +x "$appimage"

APPIMAGE_EXTRACT_AND_RUN=1 "$appimage" >/logs/app.log 2>&1 &
launcher=$!
trap 'kill "$launcher" 2>/dev/null; pkill -P "$launcher" 2>/dev/null; true' EXIT

wid=""
for _ in $(seq 300); do
  wid=$(xdotool search --onlyvisible --class hushpen 2>/dev/null | head -1)
  [ -n "$wid" ] && break
  kill -0 "$launcher" 2>/dev/null || { cat /logs/app.log; check "idle RSS" fail "the app exited early"; finish idle-rss; exit 0; }
  sleep 0.1
done
[ -n "$wid" ] || { check "idle RSS" fail "no window within 30 s"; finish idle-rss; exit 0; }
echo "window $(win_size "$wid"); idle for ${idle} s with no model"
sleep "$idle"
shot idle-rss-home

# The UI process is the hushpen whose parent is not another hushpen.
ui=""
for pid in $(pgrep -x hushpen); do
  parent=$(ps -o comm= -p "$(ps -o ppid= -p "$pid" | tr -d ' ')")
  [ "$parent" = hushpen ] || ui=$pid
done
[ -n "$ui" ] || { check "idle RSS" fail "no hushpen UI process"; finish idle-rss; exit 0; }

kb() { awk -v k="$2" '$1 == k":" {print $2}' "/proc/$1/status"; }
pss() { awk '/^Pss:/ {print $2}' "/proc/$1/smaps_rollup"; }
ps -eo pid,ppid,rss,args | grep -E 'hushpen|Hushpen' | grep -v grep | tee "$out/idle-rss-ps.txt"
ui_rss=$(kb "$ui" VmRSS)
ui_anon=$(kb "$ui" RssAnon)
ui_pss=$(pss "$ui")
echo "ui process $ui: VmRSS ${ui_rss} kB, RssAnon ${ui_anon} kB, PSS ${ui_pss} kB"
echo "largest resident mappings of the UI process (kB):"
awk '/^[0-9a-f]+-[0-9a-f]+ / {name = ($6 == "" ? "[anonymous]" : $6)} /^Rss:/ {rss[name] += $2}
  END {for (n in rss) print rss[n], n}' "/proc/$ui/smaps" | sort -rn | head -8 | tee "$out/idle-rss-maps.txt"
children_rss=0
for child in $(pgrep -P "$ui"); do
  r=$(kb "$child" VmRSS)
  echo "child $child ($(cat "/proc/$child/comm")): VmRSS ${r} kB"
  children_rss=$((children_rss + r))
done
mb=$((ui_rss / 1024))
verdict=$([ "$mb" -le "$target" ] && echo "within the $target MB target" || echo "over the $target MB target")
{
  echo "appimage_sha256=$(sha256sum "$appimage" | cut -d' ' -f1)"
  echo "ui_vmrss_kb=$ui_rss ui_rssanon_kb=$ui_anon ui_pss_kb=$ui_pss children_vmrss_kb=$children_rss idle_s=$idle"
} >"$out/idle-rss.txt"
check "idle RSS" pass "UI process VmRSS ${mb} MB (RssAnon $((ui_anon / 1024)) MB, PSS $((ui_pss / 1024)) MB), children $((children_rss / 1024)) MB, no model, ${idle} s idle: $verdict"
finish idle-rss
