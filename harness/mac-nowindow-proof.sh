#!/bin/bash
# usage: mac-nowindow-proof.sh
# Mac proof that the GPUI component tests open no window and activate no app (VAL-FND-033).
# It runs `cargo test -p hushpen-app --lib` while it samples, read-only, every 100 ms:
#   - the frontmost app (lsappinfo),
#   - the apps registered with the window server (lsappinfo),
#   - the on-screen and off-screen windows and their owners (CGWindowListCopyWindowInfo),
# and looks for anything owned by a test binary (a process whose executable is
# .../deps/hushpen_*-<hash>). It never activates anything itself.
# It fails when a test binary registers as an app, owns a window, or becomes frontmost. A change
# of the frontmost app to another owner (the user switching apps, a game) is logged, not failed.
# It refuses to start while a game is in front.
# Evidence: $HUSHPEN_ROOT/evidence/m0-harness/mac-nowindow-proof.log
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
[ "$(uname)" = Darwin ] || die "this proof runs on the Mac"

GAME_PATTERN=${HUSHPEN_GAME_PATTERN:-League|Valorant|Riot|Steam|Fortnite|Minecraft|Overwatch|Counter-Strike|Dota}
OWN_EXE='/deps/hushpen_[a-z_]+-[0-9a-f]+$'
out=$HUSHPEN_ROOT/evidence/m0-harness
log=$out/mac-nowindow-proof.log
mkdir -p "$out"

say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$log"; }
front_info() { lsappinfo info -only name -only pid -only executablepath "$(lsappinfo front)" 2>/dev/null; }
front_name() { front_info | sed -n '1s/^"\([^"]*\)".*/\1/p'; }
front_pid() { front_info | sed -n 's/.*pid = \([0-9]*\).*/\1/p' | head -1; }
# The pids of running test binaries.
own_pids() { ps -axo pid=,comm= | awk -v re="$OWN_EXE" '$2 ~ re { print $1 }'; }
own_apps() { lsappinfo list 2>/dev/null | grep -E "executable path=.*$OWN_EXE" | sed 's/^ *//'; }
window_owners() {
  osascript -l JavaScript -e 'ObjC.import("CoreGraphics");
    ObjC.deepUnwrap(ObjC.castRefToObject($.CGWindowListCopyWindowInfo(0, 0)))
      .map(function (w) { return w.kCGWindowOwnerPID; }).join("\n")' 2>/dev/null
}

: >"$log"
start_name=$(front_name)
say "start frontmost=$start_name pid=$(front_pid) commit=$(git -C "$HUSHPEN_REPO" rev-parse --short HEAD)"
if echo "$start_name" | grep -qE "$GAME_PATTERN"; then
  say "SKIPPED: a game is in front ($start_name); retry when it leaves"
  exit 2
fi

notes=$out/.front-changes
violations=$out/.violations
: >"$notes"
: >"$violations"
(
  last=$(front_pid)
  while :; do
    now=$(front_pid)
    own=$(own_pids)
    if [ -n "$own" ]; then
      echo "$own" | grep -qxF "$now" &&
        echo "$(date -u +%T.%N) a test binary became frontmost (pid $now)" >>"$violations"
      apps=$(own_apps)
      [ -z "$apps" ] ||
        echo "$(date -u +%T.%N) a test binary registered with the window server: $apps" >>"$violations"
      owners=$(window_owners)
      for pid in $own; do
        echo "$owners" | grep -qxF "$pid" &&
          echo "$(date -u +%T.%N) a test binary owns a window (pid $pid)" >>"$violations"
      done
    fi
    if [ "$now" != "$last" ]; then
      echo "$(date -u +%T.%N) frontmost owner changed: $(front_name) (pid $now)" >>"$notes"
      last=$now
    fi
    sleep 0.1
  done
) &
poller=$!
trap 'kill $poller 2>/dev/null' EXIT

. "$HUSHPEN_ROOT/env.sh"
cd "$HUSHPEN_REPO" || exit 1
set -o pipefail
cargo test -p hushpen-app --lib --locked -j 4 2>&1 | tee -a "$log" | grep -E '^test result|FAILED|panicked'
tests_status=${PIPESTATUS[0]}
kill $poller 2>/dev/null
wait $poller 2>/dev/null

say "end frontmost=$(front_name) pid=$(front_pid)"
say "frontmost owner changes during the run (logged, not failed): $(wc -l <"$notes" | tr -d ' ')"
[ ! -s "$notes" ] || tee -a "$log" <"$notes"

fail=0
[ "$tests_status" = 0 ] || { say "FAIL: cargo test exited $tests_status"; fail=1; }
if [ -s "$violations" ]; then
  say "FAIL: a test binary showed a window, registered as an app, or became frontmost"
  sort -u "$violations" | tee -a "$log"
  fail=1
fi
[ "$fail" = 0 ] && say "PASS: the component tests opened no window and activated no app"
exit $fail
