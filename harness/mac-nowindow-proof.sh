#!/bin/bash
# usage: mac-nowindow-proof.sh
# Mac proof that the GPUI component tests open no window and activate no app (VAL-FND-033).
# It runs `cargo test -p hushpen-app --lib` while it reads the frontmost app every 100 ms
# (read-only lsappinfo) and compares the list of registered apps before and after. It never
# activates anything itself. It refuses to start while a game is in front and stops at the first
# change of the frontmost app.
# Evidence: $HUSHPEN_ROOT/evidence/m0-harness/mac-nowindow-proof.log
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
[ "$(uname)" = Darwin ] || die "this proof runs on the Mac"

GAME_PATTERN=${HUSHPEN_GAME_PATTERN:-League|Valorant|Riot|Steam|Fortnite|Minecraft|Overwatch|Counter-Strike|Dota}
out=$HUSHPEN_ROOT/evidence/m0-harness
log=$out/mac-nowindow-proof.log
mkdir -p "$out"

front_name() { lsappinfo info -only name "$(lsappinfo front)" 2>/dev/null | sed -n '1s/^"\([^"]*\)".*/\1/p'; }
front_asn() { lsappinfo front 2>/dev/null; }
apps() { lsappinfo list 2>/dev/null | grep -o 'ASN:[0-9a-fx-]*' | sort -u; }
say() { echo "$(date -u +%FT%TZ) $*" | tee -a "$log"; }

: >"$log"
start_name=$(front_name)
start_asn=$(front_asn)
say "start frontmost=$start_name asn=$start_asn commit=$(git -C "$HUSHPEN_REPO" rev-parse --short HEAD)"
if echo "$start_name" | grep -qE "$GAME_PATTERN"; then
  say "SKIPPED: a game is in front ($start_name); retry when it leaves"
  exit 2
fi

apps >"$out/.apps-before"
changes=$out/.front-changes
: >"$changes"
(
  while :; do
    now=$(front_asn)
    [ "$now" = "$start_asn" ] || echo "$(date -u +%T.%N) frontmost became $now" >>"$changes"
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

apps >"$out/.apps-after"
new_apps=$(comm -13 "$out/.apps-before" "$out/.apps-after")
end_asn=$(front_asn)
say "end frontmost=$(front_name) asn=$end_asn"
say "front changes during the run: $(wc -l <"$changes" | tr -d ' ')"
[ -s "$changes" ] && cat "$changes" | tee -a "$log"
say "apps registered during the run: ${new_apps:-none}"

fail=0
[ "$tests_status" = 0 ] || { say "FAIL: cargo test exited $tests_status"; fail=1; }
[ ! -s "$changes" ] || { say "FAIL: the frontmost app changed"; fail=1; }
[ -z "$new_apps" ] || { say "FAIL: a new app registered with the window server"; fail=1; }
[ "$end_asn" = "$start_asn" ] || { say "FAIL: the frontmost app differs at the end"; fail=1; }
[ "$fail" = 0 ] && say "PASS: the component tests opened no window and activated no app"
exit $fail
