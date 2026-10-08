#!/bin/bash
# usage (inside a slot, through with-env.sh): run-suite.sh <smoke|env|mic|models|home|tail|hotkey|insert|guards|paste-last|engine|cleanup|dictionary|history|audio|export|core|full>
# Runs the suite scripts in order and writes result.json. core and full are smoke plus env today;
# a feature that adds end-to-end checks adds a script to suites/ and to the lists below.
case "${1:-}" in
  smoke) scripts=(smoke) ;;
  env) scripts=(env) ;;
  mic) scripts=(mic) ;;
  models) scripts=(models) ;;
  home) scripts=(home) ;;
  tail) scripts=(tail) ;;
  hotkey) scripts=(hotkey) ;;
  insert) scripts=(insert) ;;
  guards) scripts=(guards) ;;
  paste-last) scripts=(paste-last) ;;
  engine) scripts=(engine) ;;
  cleanup) scripts=(cleanup) ;;
  dictionary) scripts=(dictionary) ;;
  history) scripts=(history) ;;
  audio) scripts=(audio) ;;
  export) scripts=(export) ;;
  core | full) scripts=(smoke env mic models home tail hotkey insert guards paste-last engine cleanup dictionary history audio export) ;;
  *) echo "run-suite: unknown suite '${1:-}'" >&2; exit 2 ;;
esac
. /harness/slot/lib.sh
: >"$CHECKS"
for script in "${scripts[@]}"; do
  # A script records its outcome with check; its own exit code carries nothing.
  bash "/harness/slot/suites/$script.sh"
done
finish "$1"
