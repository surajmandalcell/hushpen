use super::ai_tests::{Calls, ai_answers, only_row, pasted};
use super::testkit::*;
use super::tests::{hold_run, listen_for_cues};
use crate::ai_cleanup::Attempt;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{ARM_WINDOW_MS, AppEvent, Cue, Mode, State};
use hushpen_core::insert::Chord;
use hushpen_store::history;
use serde_json::json;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

const SELECTION: &str = "hey can you send me the file zebrafinch";
const INSTRUCTION: &str = "make this formal";
const EDITED: &str = "Could you please send me the zebrafinch file?";

fn setup(cx: &mut TestAppContext) -> (Rig, Arc<FakeInserter>) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("Target-gtk.py", Chord::CtrlV);
    *inserter.selected.lock().unwrap() = Some(SELECTION.into());
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text(INSTRUCTION, "en"));
    (rig, inserter)
}

fn answered(cx: &mut TestAppContext, rig: &Rig, attempt: Attempt) -> Arc<Mutex<Calls>> {
    ai_answers(cx, rig, attempt, Ok(()))
}

/// The Command Mode keys: the hold key goes down first and is only armed, then the rest of the
/// chord.
fn chord_down(cx: &mut TestAppContext, rig: &Rig, at: u64) -> Result<(), String> {
    send_at(cx, rig, at, AppEvent::HoldArm)?;
    send_at(cx, rig, at + 10, AppEvent::CommandDown)
}

fn chord_up(cx: &mut TestAppContext, rig: &Rig, at: u64) {
    send_at(cx, rig, at, AppEvent::CommandUp).unwrap();
    send_at(cx, rig, at, AppEvent::HoldUp).unwrap();
}

fn command_run(cx: &mut TestAppContext, rig: &Rig, from: u64) {
    chord_down(cx, rig, from).unwrap();
    chord_up(cx, rig, from + 2_000);
    settle(cx, rig);
}

fn flash(cx: &mut TestAppContext, rig: &Rig) -> Option<(&'static str, String)> {
    rig.controller.read_with(cx, |c, _| {
        c.flash().map(|flash| (flash.code, flash.message.clone()))
    })
}

fn rows(rig: &Rig) -> Vec<history::Row> {
    history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows
}

#[gpui_kit::test]
fn the_edited_text_is_pasted_in_place_of_the_selection(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let calls = answered(cx, &rig, Attempt::Done(EDITED.into()));
    chord_down(cx, &rig, 1_000).unwrap();
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.mode()),
        Mode::Command,
        "the hold turned into the command"
    );
    assert_eq!(
        rig.controller
            .read_with(cx, |c, _| c.pipeline_json()["mode"].clone()),
        json!("command")
    );
    chord_up(cx, &rig, 3_000);
    settle(cx, &rig);

    assert_eq!(pasted(&inserter), [EDITED]);
    assert_eq!(machine_state(cx, &rig), State::Done);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.edits.len(), 1);
    assert_eq!(calls.edits[0].0, INSTRUCTION);
    assert_eq!(calls.edits[0].1, SELECTION);
    assert!(
        calls.seen.is_empty(),
        "the instruction is never cleaned up as text"
    );
    assert_eq!(rig.specs.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn the_history_row_is_a_command_and_never_keeps_the_selection(cx: &mut TestAppContext) {
    let (rig, _inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    command_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(row.kind, "command");
    assert_eq!(row.status, "completed");
    assert_eq!(row.instruction.as_deref(), Some(INSTRUCTION));
    assert_eq!(row.final_text.as_deref(), Some(EDITED));
    assert_eq!(row.llm_outcome.as_deref(), Some("used"));
    let dump = format!("{row:?}");
    assert!(!dump.contains(SELECTION), "{dump}");
    assert_eq!(row.llm_text, None);
    assert_eq!(
        dump.matches("zebrafinch").count(),
        1,
        "only final_text: {dump}"
    );
}

#[gpui_kit::test]
fn a_command_offers_no_undo_cleanup(cx: &mut TestAppContext) {
    let (rig, _inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    command_run(cx, &rig, 1_000);
    assert!(!rig.controller.read_with(cx, |c, _| c.undo_offered()));
}

#[gpui_kit::test]
fn the_command_keys_alone_start_a_command_session(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    send_at(cx, &rig, 1_000, AppEvent::CommandDown).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Listening);
    send_at(cx, &rig, 3_000, AppEvent::CommandUp).unwrap();
    settle(cx, &rig);
    assert_eq!(pasted(&inserter), [EDITED]);
}

#[gpui_kit::test]
fn no_selection_shows_the_notice_and_starts_nothing(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    *inserter.selected.lock().unwrap() = None;
    let calls = answered(cx, &rig, Attempt::Done(EDITED.into()));

    send_at(cx, &rig, 1_000, AppEvent::CommandDown).unwrap_err();

    assert_eq!(
        flash(cx, &rig),
        Some(("COMMAND_NO_SELECTION", "Select text first".to_owned()))
    );
    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(session_wavs(&rig).is_empty(), "the microphone never opened");
    settle(cx, &rig);
    assert_eq!(calls.lock().unwrap().asked, 0, "no model job");
    assert!(rows(&rig).is_empty());
    assert!(pasted(&inserter).is_empty());
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.refused()),
        Some("COMMAND_NO_SELECTION")
    );
}

#[gpui_kit::test]
fn a_blank_selection_counts_as_none(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    *inserter.selected.lock().unwrap() = Some("  \n ".into());
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    send_at(cx, &rig, 1_000, AppEvent::CommandDown).unwrap_err();
    assert_eq!(flash(cx, &rig).unwrap().0, "COMMAND_NO_SELECTION");
}

#[gpui_kit::test]
fn a_hold_that_ran_before_the_missing_selection_was_found_is_cancelled(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    *inserter.selected.lock().unwrap() = None;
    answered(cx, &rig, Attempt::Done(EDITED.into()));

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 1_010, AppEvent::CommandDown).unwrap_err();
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    send_at(cx, &rig, 1_020, AppEvent::CommandUp).unwrap();
    send_at(cx, &rig, 1_020, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);

    assert!(
        rig.specs.lock().unwrap().is_empty(),
        "nothing was transcribed"
    );
    assert!(rows(&rig).is_empty());
    assert!(pasted(&inserter).is_empty());
    assert_eq!(flash(cx, &rig).unwrap().0, "COMMAND_NO_SELECTION");
}

#[gpui_kit::test]
fn a_chord_without_a_selection_starts_no_session_and_plays_no_cue(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    *inserter.selected.lock().unwrap() = None;
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    let played = listen_for_cues(cx, &rig);

    chord_down(cx, &rig, 1_000).unwrap_err();
    chord_up(cx, &rig, 1_500);
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(played.lock().unwrap().is_empty(), "no start or cancel cue");
    assert!(session_wavs(&rig).is_empty(), "the armed audio was dropped");
    assert!(rows(&rig).is_empty());
    assert_eq!(flash(cx, &rig).unwrap().0, "COMMAND_NO_SELECTION");
}

#[gpui_kit::test]
fn a_chord_key_that_comes_after_the_window_turns_the_running_hold_into_the_command(
    cx: &mut TestAppContext,
) {
    let (rig, _inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    let played = listen_for_cues(cx, &rig);

    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();
    send_at(cx, &rig, 1_000 + ARM_WINDOW_MS + 5, AppEvent::CommandDown).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(rig.controller.read_with(cx, |c, _| c.mode()), Mode::Command);
    assert_eq!(*played.lock().unwrap(), vec![(Cue::Start, 0.5)], "one cue");
    assert_eq!(rig.mic_backend.starts.load(Ordering::SeqCst), 1);
}

#[gpui_kit::test]
fn a_chord_that_is_accepted_plays_one_start_cue_and_keeps_the_armed_audio(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    let played = listen_for_cues(cx, &rig);

    chord_down(cx, &rig, 1_000).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(rig.controller.read_with(cx, |c, _| c.mode()), Mode::Command);
    chord_up(cx, &rig, 3_000);
    settle(cx, &rig);

    assert_eq!(
        *played.lock().unwrap(),
        vec![(Cue::Start, 0.5), (Cue::Stop, 0.5)]
    );
    assert_eq!(
        rig.mic_backend.starts.load(Ordering::SeqCst),
        1,
        "the microphone opened when the key went down, not when the command began"
    );
    assert_eq!(pasted(&inserter), [EDITED]);
}

#[gpui_kit::test]
fn with_ai_off_the_chord_starts_no_session_and_reads_no_selection(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let calls = answered(cx, &rig, Attempt::Done(EDITED.into()));
    rig.storage
        .settings
        .set("cleanup.llm.enabled", json!(false))
        .unwrap();
    let played = listen_for_cues(cx, &rig);

    chord_down(cx, &rig, 1_000).unwrap_err();
    assert!(played.lock().unwrap().is_empty(), "no start or cancel cue");

    let (code, message) = flash(cx, &rig).unwrap();
    assert_eq!(code, "COMMAND_NEEDS_AI");
    assert!(
        message.contains("AI cleanup") && message.contains("endpoint"),
        "{message}"
    );
    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_ne!(rig.controller.read_with(cx, |c, _| c.mode()), Mode::Command);
    assert!(session_wavs(&rig).is_empty(), "the armed audio was dropped");
    chord_up(cx, &rig, 3_000);
    settle(cx, &rig);
    assert_eq!(
        inserter
            .selection_reads
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(calls.lock().unwrap().asked, 0);
    assert!(rows(&rig).is_empty());
    assert!(pasted(&inserter).is_empty());
}

#[gpui_kit::test]
fn a_failed_edit_pastes_nothing_and_shows_the_error(cx: &mut TestAppContext) {
    for (attempt, code) in [
        (Attempt::TimedOut, "LLM_TIMEOUT"),
        (Attempt::Failed("LLM_CRASHED".into()), "LLM_CRASHED"),
        (Attempt::Failed("ENDPOINT_FAILED".into()), "ENDPOINT_FAILED"),
        (Attempt::Done(String::new()), "LLM_GUARD_REJECTED"),
    ] {
        let (rig, inserter) = setup(cx);
        answered(cx, &rig, attempt);
        command_run(cx, &rig, 1_000);

        assert_eq!(machine_state(cx, &rig), State::Failed, "{code}");
        assert!(
            pasted(&inserter).is_empty(),
            "{code}: the instruction is never pasted"
        );
        let failure = rig.controller.read_with(cx, |c, _| c.failure_code());
        assert_eq!(failure, Some(code));
        let row = only_row(&rig);
        assert_eq!(row.kind, "command");
        assert_eq!(row.status, "failed");
        assert_eq!(row.error_code.as_deref(), Some(code));
        assert_eq!(row.instruction.as_deref(), Some(INSTRUCTION));
        assert_eq!(row.final_text, None);
    }
}

#[gpui_kit::test]
fn esc_while_the_model_edits_leaves_the_selection_alone(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let calls = answered(cx, &rig, Attempt::Done(EDITED.into()));
    chord_down(cx, &rig, 1_000).unwrap();
    chord_up(cx, &rig, 3_000);
    let transcribe = rig.work.borrow_mut().pop_front().unwrap();
    transcribe();
    cx.run_until_parked();
    assert_eq!(machine_state(cx, &rig), State::Cleaning);

    send_at(cx, &rig, 3_100, AppEvent::Esc).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert!(pasted(&inserter).is_empty());
    assert!(calls.lock().unwrap().edits.is_empty());
}

#[gpui_kit::test]
fn esc_while_listening_leaves_the_selection_alone(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    chord_down(cx, &rig, 1_000).unwrap();
    send_at(cx, &rig, 1_500, AppEvent::Esc).unwrap();
    chord_up(cx, &rig, 3_000);
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert!(pasted(&inserter).is_empty());
    assert!(rows(&rig).is_empty());
}

#[gpui_kit::test]
fn a_hands_free_session_ignores_the_command_keys(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    send_at(cx, &rig, 1_000, AppEvent::HandsFreeToggle).unwrap();
    send_at(cx, &rig, 1_100, AppEvent::CommandDown).unwrap();
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.mode()),
        Mode::HandsFree
    );
    assert_eq!(
        inserter
            .selection_reads
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[gpui_kit::test]
fn a_plain_dictation_after_a_command_is_a_dictation_again(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    answered(cx, &rig, Attempt::Done(EDITED.into()));
    command_run(cx, &rig, 1_000);
    send_at(cx, &rig, 60_000, AppEvent::Tick).unwrap();
    say(&rig, outcome_text("hello there", "en"));
    hold_run(cx, &rig, 70_000);

    assert_eq!(pasted(&inserter).len(), 2);
    let kinds: Vec<String> = rows(&rig).into_iter().map(|row| row.kind).collect();
    assert_eq!(kinds.iter().filter(|kind| *kind == "command").count(), 1);
    assert_eq!(kinds.iter().filter(|kind| *kind == "dictation").count(), 1);
}
