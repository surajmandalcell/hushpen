use super::testkit::*;
use super::tests::hold_run;
use gpui_kit::TestAppContext;
use hushpen_audio::CaptureError;
use hushpen_core::dictation::{AppEvent, RECOVER_MS, State};
use hushpen_core::error::MIC_UNAVAILABLE;
use hushpen_core::insert::Chord;
use hushpen_store::history::{self, Row};
use std::sync::Arc;

fn rows(rig: &Rig) -> Vec<Row> {
    history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows
}

fn offered(cx: &mut TestAppContext, rig: &Rig) -> bool {
    rig.controller
        .read_with(cx, |controller, _| controller.recover_offered())
}

fn recover_at(cx: &mut TestAppContext, rig: &Rig, ms: u64) -> Result<(), String> {
    rig.now.set(ms);
    rig.controller
        .update(cx, |controller, cx| controller.recover(cx))
}

fn pasting(cx: &mut TestAppContext, rig: &Rig) -> Arc<FakeInserter> {
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, rig, &inserter);
    inserter
}

/// Down at 1 s, Esc while it listens at 2.8 s, and the key comes up after.
fn cancel_while_listening(cx: &mut TestAppContext, rig: &Rig) {
    send_at(cx, rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, rig, 2_800, AppEvent::Esc).unwrap();
    send_at(cx, rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, rig);
    assert_eq!(machine_state(cx, rig), State::Cancelled);
}

#[gpui_kit::test]
fn recover_in_time_transcribes_the_cancelled_dictation_and_pastes_it(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = pasting(cx, &rig);
    say(&rig, outcome_text("brought back", "en"));
    cancel_while_listening(cx, &rig);
    assert!(offered(cx, &rig));
    assert!(rig.specs.lock().unwrap().is_empty());

    recover_at(cx, &rig, 2_800 + RECOVER_MS - 1).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);
    settle(cx, &rig);

    assert_eq!(*inserter.log.lock().unwrap(), ["insert brought back"]);
    let rows = rows(&rig);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "completed");
    assert_eq!(rows[0].final_text.as_deref(), Some("brought back"));
    assert_eq!(kept_wavs(&rig).len(), 1);
    assert!(session_wavs(&rig).is_empty());
    assert!(!offered(cx, &rig), "once only");
}

#[gpui_kit::test]
fn recover_too_late_does_nothing_and_the_audio_goes(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = pasting(cx, &rig);
    cancel_while_listening(cx, &rig);

    rig.now.set(2_800 + RECOVER_MS);
    assert!(!offered(cx, &rig));
    assert!(recover_at(cx, &rig, 2_800 + RECOVER_MS).is_err());
    send_at(cx, &rig, 2_800 + RECOVER_MS, AppEvent::Tick).unwrap();
    settle(cx, &rig);

    assert!(rig.specs.lock().unwrap().is_empty(), "no transcribe job");
    assert!(inserter.log.lock().unwrap().is_empty());
    assert!(rows(&rig).is_empty());
    assert!(session_wavs(&rig).is_empty(), "the audio is gone");
}

#[gpui_kit::test]
fn a_new_dictation_ends_the_recover_time_and_drops_the_cancelled_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = pasting(cx, &rig);
    say(&rig, outcome_text("the new words", "en"));
    cancel_while_listening(cx, &rig);

    hold_run(cx, &rig, 3_500);
    assert!(!offered(cx, &rig));
    assert!(recover_at(cx, &rig, 6_000).is_err());
    settle(cx, &rig);

    assert_eq!(*inserter.log.lock().unwrap(), ["insert the new words"]);
    assert_eq!(rig.specs.lock().unwrap().len(), 1, "only the new dictation");
    assert_eq!(rows(&rig).len(), 1);
    assert_eq!(kept_wavs(&rig).len(), 1);
    assert!(session_wavs(&rig).is_empty(), "the cancelled audio is gone");
}

#[gpui_kit::test]
fn a_dictation_that_never_recorded_has_nothing_to_recover(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    *rig.mic_backend.start_error.lock().unwrap() =
        Some(CaptureError::new(MIC_UNAVAILABLE, "no input device"));
    assert!(send_at(cx, &rig, 1_000, AppEvent::HoldDown).is_err());
    send_at(cx, &rig, 1_200, AppEvent::Esc).unwrap();

    assert!(!offered(cx, &rig));
    assert!(recover_at(cx, &rig, 1_300).is_err());
    assert_eq!(machine_state(cx, &rig), State::Failed);
}

#[gpui_kit::test]
fn recover_after_a_cancel_while_transcribing_starts_again_from_the_kept_audio(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = pasting(cx, &rig);
    say(&rig, outcome_text("never used", "en"));
    say(&rig, outcome_text("second try", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    send_at(cx, &rig, 3_100, AppEvent::Esc).unwrap();
    // The cancelled job answers late; nothing goes in.
    settle(cx, &rig);
    assert!(inserter.log.lock().unwrap().is_empty());
    assert!(
        rows(&rig).is_empty(),
        "no row while Recover can bring it back"
    );

    recover_at(cx, &rig, 4_000).unwrap();
    settle(cx, &rig);

    let specs = rig.specs.lock().unwrap();
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[1].wav_path, specs[0].wav_path, "the same recording");
    drop(specs);
    assert_eq!(*inserter.log.lock().unwrap(), ["insert second try"]);
    let rows = rows(&rig);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, "completed");
    assert_eq!(kept_wavs(&rig).len(), 1);
}
