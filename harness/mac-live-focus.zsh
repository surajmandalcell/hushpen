# Usage: zsh harness/mac-live-focus.zsh <test bundle id> <command> [args...]
# For several steps, source this with test_bundle set and call focus_step for each action.
# Call focus_check during passive waits so they do not reset the two-second window.
# A single action command has 30 seconds to return.
zmodload zsh/datetime
typeset -g focus_previous='' focus_previous_name='' focus_last_step=-3

focus_transition() {
  local app=$1 bundle=$2 name=$3 observed_at=$4
  [[ $app == "$focus_previous" ]] && return 0
  if [[ -n $focus_previous ]]; then
    if [[ $bundle == "$test_bundle" || $bundle == com.apple.TextEdit ||
          $bundle == com.apple.systempreferences || $bundle == com.apple.SystemSettings ||
          $bundle == com.apple.SecurityAgent || $bundle == com.apple.UserNotificationCenter ||
          $bundle == com.apple.CoreServicesUIAgent || $bundle == com.apple.TCC* ||
          $name == SecurityAgent || $name == UserNotificationCenter ||
          $name == CoreServicesUIAgent || $name == tccd ]] ||
       (( observed_at - focus_last_step <= 2 )); then
      print -ru2 -- "Focus guard stopped: $focus_previous_name -> $name ($bundle)"
      return 1
    fi
    print -ru2 -- "Owner switched apps: $focus_previous_name -> $name ($bundle)"
  fi
  focus_previous=$app
  focus_previous_name=$name
}

focus_check() {
  local app info name observed_at bundle=''
  app=$(lsappinfo front) || return 1
  [[ -n $app ]] || return 1
  [[ $app == "$focus_previous" ]] && return 0
  observed_at=$EPOCHREALTIME
  info=$(lsappinfo info -only name -only bundleid "$app") || return 1
  name=${info#\"}
  name=${name%%\"*}
  [[ $info == *bundleID=\"* ]] && bundle=${info#*bundleID=\"}
  bundle=${bundle%%\"*}
  focus_transition "$app" "$bundle" "$name" "$observed_at"
}

focus_step() {
  focus_check || return 1
  focus_last_step=$EPOCHREALTIME
  "$@" &
  local child=$! result=0 stop=0
  while kill -0 "$child" 2>/dev/null || (( EPOCHREALTIME - focus_last_step <= 2 )); do
    if ! focus_check; then
      stop=1
      break
    fi
    if (( EPOCHREALTIME - focus_last_step > 30 )); then
      print -ru2 -- 'Focus guard stopped: the action timed out.'
      stop=124
      break
    fi
    sleep 0.1
  done
  if (( stop != 0 )); then
    kill -KILL "$child" 2>/dev/null || true
    wait "$child" 2>/dev/null || true
    return $stop
  fi
  focus_check || return 1
  wait "$child" || result=$?
  return $result
}

if [[ $ZSH_EVAL_CONTEXT == toplevel ]]; then
  if (( $# < 2 )) || [[ -z $1 ]]; then
    print -ru2 -- "Usage: zsh $0 <test bundle id> <command> [args...]"
    exit 2
  fi
  test_bundle=$1
  shift
  focus_step "$@"
fi
