# Run with zsh. No apps are opened or controlled.
set -eu
test_bundle=app.hushpen.test.live-typing
source "${0:A:h}/mac-live-focus.zsh"

focus_transition finder com.apple.finder Finder 100
focus_transition orca com.stablyai.orca Orca 101
[[ $focus_previous == orca ]]

focus_last_step=110
focus_transition orca com.stablyai.orca Orca 110
if focus_transition chrome com.google.Chrome Chrome 112; then exit 1; fi
[[ $focus_previous == orca ]]
focus_transition chrome com.google.Chrome Chrome 112.001
[[ $focus_previous == chrome ]]

for bundle in "$test_bundle" com.apple.TextEdit com.apple.systempreferences com.apple.UserNotificationCenter; do
  if focus_transition test "$bundle" Test 120; then exit 1; fi
done
if focus_transition prompt '' SecurityAgent 120; then exit 1; fi
print 'Focus guard checks passed.'
