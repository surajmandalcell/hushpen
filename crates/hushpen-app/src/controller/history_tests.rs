use super::testkit::*;
use super::tests::{blocked_inserter, hold_run};
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::{Chord, Outcome as InsertOutcome};
use hushpen_engine::{Failure, JobOutcome};
use hushpen_store::dictionary as store_dictionary;
use hushpen_store::history::{self, Row};
use serde_json::json;

fn rows(rig: &Rig) -> Vec<Row> {
    history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows
}

fn only_row(rig: &Rig) -> Row {
    let rows = rows(rig);
    assert_eq!(rows.len(), 1, "exactly one row: {rows:?}");
    rows.into_iter().next().unwrap()
}

fn audio_of(rig: &Rig, row: &Row) -> std::path::PathBuf {
    rig.storage
        .data
        .root()
        .join(row.audio_path.as_ref().unwrap())
}

#[gpui_kit::test]
fn a_pasted_dictation_makes_one_complete_row_with_its_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("Target-gtk.py", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    store_dictionary::add(&rig.storage.database, "Zyxtrel", None).unwrap();
    say(&rig, outcome_text("the quick brown fox", "en"));

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(row.kind, "dictation");
    assert_eq!(row.status, "completed");
    assert_eq!(row.error_code, None);
    assert_eq!(row.insert_outcome.as_deref(), Some("pasted"));
    assert_eq!(row.target_app.as_deref(), Some("Target-gtk.py"));
    assert_eq!(row.duration_ms, 1_000);
    assert_eq!(row.model_id.as_deref(), Some("base"));
    assert_eq!(row.language_requested.as_deref(), Some("auto"));
    assert_eq!(row.language_detected.as_deref(), Some("en"));
    assert_eq!(row.raw_text.as_deref(), Some("the quick brown fox"));
    assert_eq!(row.rule_text.as_deref(), Some("the quick brown fox"));
    assert_eq!(row.final_text.as_deref(), Some("the quick brown fox"));
    assert!(row.prompt.as_deref().unwrap().contains("Zyxtrel"));
    assert_eq!(paste_texts(&inserter), ["the quick brown fox"]);

    assert_eq!(row.audio_path, Some(format!("audio/{}.wav", row.id)));
    assert!(audio_of(&rig, &row).is_file());
    assert!(session_wavs(&rig).is_empty(), "the audio moved to audio/");
}

fn paste_texts(inserter: &FakeInserter) -> Vec<String> {
    inserter
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|(text, _, _)| text.clone())
        .collect()
}

#[gpui_kit::test]
fn the_row_keeps_the_raw_text_and_the_cleaned_rule_text_apart(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set("cleanup.rules", json!(true))
        .unwrap();
    say(
        &rig,
        outcome_text("Um, so I think, uh, we should go to the store.", "en"),
    );

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(
        row.raw_text.as_deref(),
        Some("Um, so I think, uh, we should go to the store.")
    );
    let rule = row.rule_text.clone().unwrap();
    assert!(
        !rule.to_lowercase().contains("um,") && !rule.contains(" uh"),
        "{rule}"
    );
    assert_eq!(row.final_text, Some(rule));
}

#[gpui_kit::test]
fn a_run_without_a_paste_path_is_copied_only_with_no_target_app(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("copied words", "en"));

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(row.status, "completed");
    assert_eq!(row.insert_outcome.as_deref(), Some("copied_only"));
    assert_eq!(row.target_app, None);
    assert_eq!(row.final_text.as_deref(), Some("copied words"));
}

#[gpui_kit::test]
fn a_blocked_paste_saves_a_failed_row_that_still_holds_the_text(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = blocked_inserter(InsertOutcome::BlockedGrab, "INSERT_KEYBOARD_GRABBED");
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("grabbed words", "en"));

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(row.status, "failed");
    assert_eq!(row.error_code.as_deref(), Some("INSERT_KEYBOARD_GRABBED"));
    assert_eq!(row.insert_outcome.as_deref(), Some("blocked_grab"));
    assert_eq!(row.final_text.as_deref(), Some("grabbed words"));
    assert_eq!(row.target_app.as_deref(), Some("GtkTarget"));
}

#[gpui_kit::test]
fn esc_while_transcribing_saves_a_cancelled_row_with_audio_and_no_text(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("never used", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();

    send_at(cx, &rig, 3_100, AppEvent::Esc).unwrap();
    settle(cx, &rig);

    let row = only_row(&rig);
    assert_eq!(row.status, "cancelled");
    assert_eq!(row.raw_text, None);
    assert_eq!(row.final_text, None);
    assert_eq!(row.insert_outcome.as_deref(), Some("none"));
    assert!(audio_of(&rig, &row).is_file());
}

#[gpui_kit::test]
fn esc_while_listening_and_a_tap_save_no_row(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 1_800, AppEvent::Esc).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    send_at(cx, &rig, 3_700, AppEvent::Tick).unwrap();
    send_at(cx, &rig, 5_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 5_100, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);

    assert!(rows(&rig).is_empty());
    assert!(kept_wavs(&rig).is_empty());
}

#[gpui_kit::test]
fn an_engine_failure_and_a_silent_run_each_save_a_failed_row_with_a_code_and_audio(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    say(
        &rig,
        JobOutcome::Failed(Failure::new("ENGINE_CRASHED", "the engine died")),
    );
    say(&rig, outcome_text("[BLANK_AUDIO]", "en"));

    hold_run(cx, &rig, 1_000);
    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 7_000);

    let rows = rows(&rig);
    assert_eq!(rows.len(), 2);
    let mut codes: Vec<_> = rows.iter().map(|row| row.error_code.clone()).collect();
    codes.sort();
    assert_eq!(
        codes,
        [
            Some("ENGINE_CRASHED".to_owned()),
            Some("ENGINE_NO_SPEECH".to_owned())
        ]
    );
    for row in &rows {
        assert_eq!(row.status, "failed");
        assert_eq!(row.final_text, None);
        assert!(audio_of(&rig, row).is_file());
    }
}

#[gpui_kit::test]
fn a_history_that_cannot_be_written_never_loses_the_text(cx: &mut TestAppContext) {
    use std::os::unix::fs::PermissionsExt;
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    let file = rig.storage.data.database_path();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
    say(&rig, outcome_text("kept in memory", "en"));

    hold_run(cx, &rig, 1_000);
    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();
    send_at(cx, &rig, 7_000, AppEvent::PasteLast).unwrap();
    settle(cx, &rig);

    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(paste_texts(&inserter), ["kept in memory", "kept in memory"]);
    assert!(rows(&rig).is_empty());
}

fn failed_row_with_audio(cx: &mut TestAppContext, rig: &Rig) -> Row {
    say(
        rig,
        JobOutcome::Failed(Failure::new("ENGINE_CRASHED", "the engine died")),
    );
    hold_run(cx, rig, 1_000);
    send_at(cx, rig, 6_000, AppEvent::Tick).unwrap();
    only_row(rig)
}

fn reprocess(cx: &mut TestAppContext, rig: &Rig, id: &str) -> Result<(), String> {
    let result = rig
        .controller
        .update(cx, |controller, cx| controller.reprocess(id, cx));
    settle(cx, rig);
    result
}

#[gpui_kit::test]
fn reprocess_recovers_a_failed_row_in_place_with_the_current_model(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let failed = failed_row_with_audio(cx, &rig);
    say(&rig, outcome_text("the lighthouse keeper", "en"));
    let before = rig.specs.lock().unwrap().len();

    reprocess(cx, &rig, &failed.id).unwrap();

    let row = only_row(&rig);
    assert_eq!(row.id, failed.id);
    assert_eq!(row.status, "completed");
    assert_eq!(row.error_code, None);
    assert_eq!(row.model_id.as_deref(), Some("base"));
    assert_eq!(row.raw_text.as_deref(), Some("the lighthouse keeper"));
    assert_eq!(row.final_text.as_deref(), Some("the lighthouse keeper"));
    assert_eq!(row.created_at, failed.created_at);
    assert_eq!(row.audio_path, failed.audio_path);
    let specs = rig.specs.lock().unwrap();
    assert_eq!(specs.len(), before + 1);
    assert_eq!(specs.last().unwrap().wav_path, audio_of(&rig, &failed));
    assert_eq!(
        history::page(&rig.storage.database, "lighthouse", None, 50)
            .unwrap()
            .rows
            .len(),
        1
    );
}

#[gpui_kit::test]
fn reprocess_applies_the_dictionary_as_it_is_now(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("the fox runs", "en"));
    hold_run(cx, &rig, 1_000);
    let before = only_row(&rig);
    assert_eq!(before.rule_text.as_deref(), Some("the fox runs"));
    store_dictionary::add(&rig.storage.database, "Foxtrel", Some("fox")).unwrap();
    say(&rig, outcome_text("the fox runs", "en"));

    reprocess(cx, &rig, &before.id).unwrap();

    let after = only_row(&rig);
    assert_eq!(after.rule_text.as_deref(), Some("the Foxtrel runs"));
    assert_eq!(after.final_text.as_deref(), Some("the Foxtrel runs"));
    assert_eq!(after.raw_text.as_deref(), Some("the fox runs"));
}

#[gpui_kit::test]
fn reprocess_of_a_row_without_audio_is_refused_and_a_silent_result_changes_nothing(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("kept words", "en"));
    hold_run(cx, &rig, 1_000);
    let row = only_row(&rig);

    say(&rig, outcome_text("[BLANK_AUDIO]", "en"));
    reprocess(cx, &rig, &row.id).unwrap();
    assert_eq!(only_row(&rig), row, "no words leave the row as it was");
    let state = rig
        .controller
        .read_with(cx, |c, _| c.reprocess_state().cloned());
    assert!(matches!(state, Some((_, super::ReprocessState::Failed(_)))));

    std::fs::remove_file(audio_of(&rig, &row)).unwrap();
    let refused = reprocess(cx, &rig, &row.id).unwrap_err();
    assert!(refused.contains("audio"), "{refused}");
    assert!(reprocess(cx, &rig, "missing").is_err());
}

#[gpui_kit::test]
fn re_paste_puts_the_text_in_the_focused_app_and_adds_no_row(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("first words", "en"));
    hold_run(cx, &rig, 1_000);
    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();

    rig.controller.update(cx, |controller, cx| {
        controller.repaste("older words".into(), cx)
    });
    settle(cx, &rig);

    assert_eq!(paste_texts(&inserter), ["first words", "older words"]);
    assert_eq!(rows(&rig).len(), 1);
    assert_eq!(machine_state(cx, &rig), State::Idle);
}

#[gpui_kit::test]
fn the_row_keeps_the_timed_segments_of_the_transcript(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(
        &rig,
        JobOutcome::Done(hushpen_engine::Transcription {
            text: "two parts".into(),
            language: "en".into(),
            segments: vec![
                hushpen_core::protocol::Segment {
                    start_ms: 0,
                    end_ms: 400,
                    text: "two".into(),
                },
                hushpen_core::protocol::Segment {
                    start_ms: 400,
                    end_ms: 900,
                    text: "parts".into(),
                },
            ],
            audio_ms: 1_000,
            decode_ms: 10,
        }),
    );

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    let segments = history::segments(&rig.storage.database, &row.id).unwrap();
    assert_eq!(
        segments
            .iter()
            .map(|s| (s.idx, s.start_ms, s.end_ms, s.text.as_str()))
            .collect::<Vec<_>>(),
        [(0, 0, 400, "two"), (1, 400, 900, "parts")]
    );
}

fn set_retention(rig: &Rig, value: &str) {
    rig.storage
        .settings
        .set("history.audioRetention", json!(value))
        .unwrap();
}

#[gpui_kit::test]
fn the_default_keeps_audio_for_a_finished_run(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("kept words", "en"));

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert!(audio_of(&rig, &row).is_file());
    assert_eq!(row.audio_removed_at, None);
}

#[gpui_kit::test]
fn never_saves_the_text_of_a_finished_run_and_no_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_retention(&rig, "never");
    say(&rig, outcome_text("words without audio", "en"));

    hold_run(cx, &rig, 1_000);

    let row = only_row(&rig);
    assert_eq!(row.status, "completed");
    assert_eq!(row.final_text.as_deref(), Some("words without audio"));
    assert_eq!(row.audio_path, None);
    assert!(row.audio_removed_at.is_some());
    assert!(kept_wavs(&rig).is_empty());
    assert!(
        session_wavs(&rig).is_empty(),
        "the session file is gone too"
    );
}

#[gpui_kit::test]
fn never_keeps_the_audio_of_a_failed_run_so_reprocess_can_recover_it(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_retention(&rig, "never");
    let failed = failed_row_with_audio(cx, &rig);
    assert!(audio_of(&rig, &failed).is_file());
    assert_eq!(failed.audio_removed_at, None);
    say(&rig, outcome_text("the lighthouse keeper", "en"));

    reprocess(cx, &rig, &failed.id).unwrap();

    let row = only_row(&rig);
    assert_eq!(row.status, "completed");
    assert_eq!(row.final_text.as_deref(), Some("the lighthouse keeper"));
    assert_eq!(
        row.audio_path, None,
        "the recovered row no longer needs audio"
    );
    assert!(row.audio_removed_at.is_some());
    assert!(kept_wavs(&rig).is_empty());
}
