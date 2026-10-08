#!/bin/bash
# The personal dictionary end to end: VAL-DICT-001 to 004, the hook half of VAL-DICT-006, and
# VAL-INS-014. Entries are added with the keyboard only, survive a restart, change the inserted
# text, reach the whisper prompt, and a non-ASCII replacement reaches the GTK entry byte for byte
# while the clipboard comes back byte for byte.
# The hold key is Right Alt (X11 keycode 108); the words come from the virtual microphone.
# The history row columns (prompt, rule_text, final_text) are read with sqlite3 for VAL-DICT-003 and
# VAL-DICT-006. Needs speech-short.wav in /assets/fixtures. Busy: it transcribes with base. It
# empties /data and restarts the app, so the slot must not be shared. Runs in one slot through
# with-env.sh.
. /harness/slot/lib.sh
. /harness/slot/dictate-lib.sh

HOLD=108
GTK_FILE=/out/gtk.txt
SHORT_WAV=$FIX/speech-short.wav
DB=/data/history/history.db

pstate() { $HC state | jq -r .pipeline.state; }
pstate_is() { [ "$(pstate)" = "$1" ]; }
pevents() {
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "pipeline" and .t_ms >= $t0) | .detail] | join(",")'
}
run_ended() {
  case ",$(pevents)," in
    *,done,* | *,failed,* | *,cancelled,*) pstate_is idle ;;
    *) return 1 ;;
  esac
}
focus() { xdotool windowfocus --sync "$1"; }

gtk_commit() { # Enter in the GTK entry writes what it holds to /out/gtk.txt; prints it
  rm -f "$GTK_FILE"
  focus "$GT"
  xdotool key Return
  wait_until 5 test -e "$GTK_FILE"
  cat "$GTK_FILE"
}
gtk_clear() {
  focus "$GT"
  xdotool key ctrl+a BackSpace
  sleep 0.2
}

ptt_run() { # <wav>: hold the key for the whole clip; waits until the run has ended
  T0=$(now_ms)
  xdotool keydown $HOLD
  wait_until 5 pstate_is listening || { xdotool keyup $HOLD; return 1; }
  paplay --device=vmic "$(pad "$1")"
  xdotool keyup $HOLD
  wait_until 240 run_ended
}

last_row() { sqlite3 "$DB" "select coalesce($1, '') from transcript order by created_at desc, id desc limit 1"; }
rows() { sqlite3 "$DB" "select count(*) from dictionary_entry"; }
rows_is() { [ "$(rows)" = "$1" ]; }
row_of() { sqlite3 "$DB" "select phrase || '|' || coalesce(heard_as, 'NULL') from dictionary_entry order by id" | paste -sd, -; }
dstate() { $HC state | jq -r ".dictionary$1"; }
message() { dstate .message; }
event_prompt() { # the prompt of the newest transcribe event since T0
  $HC events | jq -r --argjson t0 "$T0" \
    '[.[] | select(.kind == "transcribe" and .t_ms >= $t0) | .detail] | last // ""'
}
tab() { xdotool key Tab; sleep 0.25; }
rendered() { $HC tree | jq -e "any(.[]; .id == \"$1\")" >/dev/null; }

# From the focused Dictionary sidebar item, five Tabs reach the Write as field: the four items
# after it, then the field.
open_dictionary() {
  focus "$(app_window)"
  $HC click sidebar.dictionary >/dev/null
  wait_until 5 rendered dictionary.form
  sleep 0.4
}
to_write_as() { for _ in 1 2 3 4 5; do tab; done; }

echo "== setup: base model and the paste targets"
fresh base
use_model base
ids=$(/harness/slot/targets.sh)
echo "$ids"
GT=$(grep -o 'gtk=[0-9]*' <<<"$ids" | cut -d= -f2)

echo "== VAL-DICT-001 an empty Dictionary view, then keyboard-only entries that survive a restart"
failures=""
open_dictionary
rendered dictionary.empty || failures+="no empty state on a fresh data folder; "
rendered dictionary.submit || failures+="no add control; "
[ "$(rows)" = 0 ] || failures+="the table is not empty at the start; "
shot dict-001-empty
to_write_as
xdotool type --delay 40 Zyxtrel
sleep 0.2
xdotool key Return
wait_until 5 rows_is 1 || failures+="the word was not saved; "
xdotool type --delay 40 Foxtrel
tab
xdotool type --delay 40 fox
sleep 0.2
xdotool key Return
wait_until 5 rows_is 2 || failures+="the replacement was not saved; "
shot dict-001-added
$HC tree >"$RUN_OUT/001-tree.json"
[ "$(row_of)" = "Zyxtrel|NULL,Foxtrel|fox" ] || failures+="the table holds '$(row_of)'; "
[ "$(dstate .count)" = 2 ] || failures+="the view lists $(dstate .count) entries; "
rendered dictionary.row.0 && rendered dictionary.row.1 || failures+="the rows are not on screen; "
/harness/slot/app.sh stop
start_app
open_dictionary
sleep 0.5
shot dict-001-restarted
[ "$(dstate .count)" = 2 ] || failures+="after a restart the view lists $(dstate .count) entries; "
[ "$(row_of)" = "Zyxtrel|NULL,Foxtrel|fox" ] || failures+="after a restart the table holds '$(row_of)'; "
check_failures "VAL-DICT-001 keyboard-only add survives a restart" "word and replacement in the list and the table, still there after a restart" "$failures"

echo "== VAL-DICT-002 empty and duplicate phrases are refused"
failures=""
to_write_as
xdotool key Return
sleep 0.5
[ "$(rows)" = 2 ] || failures+="an empty phrase changed the table; "
[ -n "$(message)" ] && [ "$(message)" != null ] || failures+="an empty phrase shows no message; "
shot dict-002-empty
empty_message=$(message)
xdotool type --delay 40 ZYXTREL
xdotool key Return
sleep 0.5
[ "$(rows)" = 2 ] || failures+="a repeated word changed the table; "
[ -n "$(message)" ] && [ "$(message)" != null ] || failures+="a repeated word shows no message; "
shot dict-002-duplicate
check_failures "VAL-DICT-002 empty and duplicate refused" "rows stayed 2; messages '$empty_message' and '$(message)'" "$failures"
xdotool key ctrl+a BackSpace

echo "== VAL-DICT-003 a replacement changes the inserted text"
failures=""
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
shot dict-003
got=$(gtk_commit)
case " $got " in *Foxtrel*) ;; *) failures+="the entry has no Foxtrel: '$got'; " ;; esac
grep -Eiwq 'fox' <<<"$got" && failures+="the entry still has the word fox: '$got'; "
row_rule=$(last_row rule_text)
row_final=$(last_row final_text)
case "$row_rule" in *Foxtrel*) ;; *) failures+="the row rule_text has no Foxtrel: '$row_rule'; " ;; esac
case "$row_final" in *Foxtrel*) ;; *) failures+="the row final_text has no Foxtrel: '$row_final'; " ;; esac
[ "$row_final" = "$got" ] || failures+="the row final_text '$row_final' is not the inserted '$got'; "
check_failures "VAL-DICT-003 replacement inserted" "entry '$got', row rule_text '$row_rule'" "$failures"

echo "== VAL-DICT-006 (hook half) a stored word reaches the whisper prompt, and leaves it when deleted"
failures=""
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
state_prompt=$($HC state | jq -r '.pipeline.prompt // ""')
case "$(event_prompt)" in *Zyxtrel*) ;; *) failures+="the transcribe event prompt has no Zyxtrel: '$(event_prompt)'; " ;; esac
case "$state_prompt" in *Zyxtrel*) ;; *) failures+="pipeline.prompt has no Zyxtrel: '$state_prompt'; " ;; esac
row_prompt=$(last_row prompt)
case "$row_prompt" in *Zyxtrel*) ;; *) failures+="the row prompt column has no Zyxtrel: '$row_prompt'; " ;; esac
zyx_id=$(sqlite3 "$DB" "select id from dictionary_entry where phrase = 'Zyxtrel'")
$HC action dictionary-delete "{\"id\":$zyx_id}" >/dev/null
[ "$(rows)" = 1 ] || failures+="the word was not deleted; "
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the second run did not end; "
case "$(event_prompt)" in *Zyxtrel*) failures+="the deleted word is still in the prompt: '$(event_prompt)'; " ;; esac
case "$(last_row prompt)" in *Zyxtrel*) failures+="the next row prompt column still has Zyxtrel: '$(last_row prompt)'; " ;; esac
check_failures "VAL-DICT-006 prompt follows the dictionary" "prompt '$state_prompt', then without the word: '$(event_prompt)'" "$failures"

echo "== VAL-DICT-004 an edited entry and a deleted entry change the next dictation"
failures=""
fox_id=$(sqlite3 "$DB" "select id from dictionary_entry where phrase = 'Foxtrel'")
$HC action dictionary-edit "{\"id\":$fox_id,\"phrase\":\"Vixen\",\"heard_as\":\"fox\"}" >/dev/null
[ "$(row_of)" = "Vixen|fox" ] || failures+="the edit did not reach the table: '$(row_of)'; "
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run after the edit did not end; "
edited=$(gtk_commit)
case " $edited " in *Vixen*) ;; *) failures+="no Vixen after the edit: '$edited'; " ;; esac
case " $edited " in *Foxtrel*) failures+="Foxtrel is still inserted after the edit: '$edited'; " ;; esac
$HC action dictionary-delete "{\"id\":$fox_id}" >/dev/null
[ "$(rows)" = 0 ] || failures+="the entry was not deleted; "
gtk_clear
set_clip OLD
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run after the delete did not end; "
deleted=$(gtk_commit)
grep -Eiwq 'fox' <<<"$deleted" || failures+="the word fox is missing after the delete: '$deleted'; "
case " $deleted " in *Vixen* | *Foxtrel*) failures+="a replacement is still applied after the delete: '$deleted'; " ;; esac
check_failures "VAL-DICT-004 edit and delete apply to the next dictation" "edited '$edited', deleted '$deleted'" "$failures"

echo "== VAL-INS-014 non-ASCII text is inserted and the clipboard is restored byte for byte"
failures=""
want=$(printf 'caf\303\251-\303\237-\360\237\216\244')
sentinel=$(printf 'hushpen-\303\251-\303\237-\342\234\223')
$HC action dictionary-add "{\"phrase\":\"$want\",\"heard_as\":\"dog\"}" >/dev/null
[ "$(rows)" = 1 ] || failures+="the dog entry was not saved; "
gtk_clear
set_clip "$sentinel"
focus "$GT"
ptt_run "$SHORT_WAV" || failures+="the run did not end; "
shot ins-014
got=$(gtk_commit)
printf '%s' "$got" | hexdump -C >"$RUN_OUT/ins-014-gtk.hex"
case "$got" in *"$want"*) ;; *) failures+="the entry does not hold the replacement bytes: '$got'; " ;; esac
clip_back=$(clip)
printf '%s' "$clip_back" | hexdump -C >"$RUN_OUT/ins-014-clip.hex"
[ "$clip_back" = "$sentinel" ] || failures+="the clipboard holds '$clip_back', not the sentinel; "
check_failures "VAL-INS-014 non-ASCII bytes in, clipboard back" "entry '$got', clipboard '$clip_back'" "$failures"
