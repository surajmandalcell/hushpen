use super::testkit::*;
use super::{InsertSupport, KeysStatus, Phase};
use gpui_kit::TestAppContext;
use hushpen_audio::CaptureError;
use hushpen_core::dictation::{AppEvent, Mode, State};
use hushpen_core::insert::{Chord, Method, Outcome as InsertOutcome, Report, choose_x11};
use hushpen_engine::{Failure, JobOutcome};
use hushpen_platform::keys::Unavailable;
use serde_json::json;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

fn transcript(cx: &mut TestAppContext, rig: &Rig) -> String {
    rig.controller
        .read_with(cx, |controller, _| controller.transcript().to_owned())
}

fn notice_code(cx: &mut TestAppContext, rig: &Rig) -> Option<&'static str> {
    rig.controller
        .read_with(cx, |controller, _| controller.notice().map(|n| n.code))
}

/// One push-to-talk run: down, a 2 s hold, up, and the engine answers.
fn hold_run(cx: &mut TestAppContext, rig: &Rig, from: u64) {
    send_at(cx, rig, from, AppEvent::HoldDown).unwrap();
    send_at(cx, rig, from + 2_000, AppEvent::HoldUp).unwrap();
    settle(cx, rig);
}

#[gpui_kit::test]
fn the_hold_key_listens_while_down_and_transcribes_after_a_two_second_hold(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("the quick brown fox", "en"));

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.mode()),
        Mode::Hold,
        "a hold run is a hold"
    );
    assert_eq!(session_wavs(&rig).len(), 1, "capture started");

    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(transcript(cx, &rig), "the quick brown fox");
    assert_eq!(rig.specs.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn the_machine_goes_back_to_idle_by_ticks_and_the_home_result_stays_on_screen(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("kept words", "en"));
    hold_run(cx, &rig, 1_000);
    assert_eq!(machine_state(cx, &rig), State::Done);

    send_at(cx, &rig, 5_000, AppEvent::Tick).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.phase()),
        Phase::Done,
        "Home keeps showing the last result"
    );
    assert_eq!(transcript(cx, &rig), "kept words");
}

#[gpui_kit::test]
fn a_tap_under_250_ms_is_not_a_recording(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 1_100, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(rig.specs.lock().unwrap().is_empty(), "no engine job");
    assert!(session_wavs(&rig).is_empty(), "the tap left no audio");
}

#[gpui_kit::test]
fn a_tap_is_measured_by_when_the_key_moved_not_when_the_controller_got_to_it(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    // Starting the microphone is slow, so the controller reads the release long after it
    // happened.
    rig.now.set(9_000);
    rig.controller
        .update(cx, |c, cx| {
            c.dispatch_at(AppEvent::HoldDown, Some(1_000), cx)
        })
        .unwrap();
    rig.now.set(9_600);
    rig.controller
        .update(cx, |c, cx| c.dispatch_at(AppEvent::HoldUp, Some(1_100), cx))
        .unwrap();

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(rig.specs.lock().unwrap().is_empty(), "no transcription");
}

#[gpui_kit::test]
fn a_double_tap_starts_hands_free_and_the_next_press_stops_it(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("hands free words", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 1_100, AppEvent::HoldUp).unwrap();
    send_at(cx, &rig, 1_250, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 1_350, AppEvent::HoldUp).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.mode()),
        Mode::HandsFree
    );

    send_at(cx, &rig, 4_350, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 4_450, AppEvent::HoldUp).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(transcript(cx, &rig), "hands free words");
}

#[gpui_kit::test]
fn esc_in_listening_cancels_and_discards_the_audio_and_the_release_does_nothing(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    assert_eq!(session_wavs(&rig).len(), 1);

    send_at(cx, &rig, 1_800, AppEvent::Esc).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert!(session_wavs(&rig).is_empty(), "the audio is gone");

    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert!(rig.specs.lock().unwrap().is_empty(), "no transcribe job");
}

#[gpui_kit::test]
fn esc_in_transcribing_cancels_the_job_keeps_the_audio_and_the_next_run_works(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("cancelled words", "en"));
    say(&rig, outcome_text("the next words", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);

    send_at(cx, &rig, 3_100, AppEvent::Esc).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert_eq!(
        session_wavs(&rig).len(),
        1,
        "a cancelled run keeps its audio"
    );

    // The engine answers late; the answer must change nothing.
    settle(cx, &rig);
    assert!(rig.tokens.lock().unwrap()[0].is_cancelled());
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert_eq!(transcript(cx, &rig), "");
    assert_eq!(clipboard(cx).as_deref(), Some("OLD"));

    send_at(cx, &rig, 3_700, AppEvent::Tick).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Idle);
    hold_run(cx, &rig, 4_000);
    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(transcript(cx, &rig), "the next words");
}

#[gpui_kit::test]
fn a_start_without_a_verified_model_is_refused_and_nothing_starts(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], "");

    let refused = send_at(cx, &rig, 1_000, AppEvent::HoldDown);
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();

    assert!(refused.unwrap_err().contains("ENGINE_NO_MODEL"));
    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(session_wavs(&rig).is_empty(), "no capture started");
    assert!(rig.specs.lock().unwrap().is_empty());
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.refused()),
        Some("ENGINE_NO_MODEL")
    );
}

#[gpui_kit::test]
fn a_microphone_that_cannot_start_fails_the_hold_run_with_its_code(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    *rig.mic_backend.start_error.lock().unwrap() = Some(CaptureError::new(
        hushpen_core::error::MIC_UNAVAILABLE,
        "no input device",
    ));

    assert!(send_at(cx, &rig, 1_000, AppEvent::HoldDown).is_err());

    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("MIC_UNAVAILABLE"));
}

#[gpui_kit::test]
fn a_run_with_no_words_ends_failed_with_no_speech_and_the_next_run_works(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("[BLANK_AUDIO]", "en"));
    say(&rig, outcome_text("real words", "en"));

    hold_run(cx, &rig, 1_000);
    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("ENGINE_NO_SPEECH"));
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.phase()),
        Phase::NoSpeech
    );
    assert!(session_wavs(&rig).is_empty(), "silence leaves no audio");

    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 7_000);
    assert_eq!(machine_state(cx, &rig), State::Done);
}

#[gpui_kit::test]
fn an_engine_failure_ends_failed_and_keeps_the_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(
        &rig,
        JobOutcome::Failed(Failure::new("ENGINE_CRASHED", "the engine died")),
    );

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("ENGINE_CRASHED"));
    assert_eq!(session_wavs(&rig).len(), 1);
}

#[gpui_kit::test]
fn home_and_the_hold_key_run_one_pipeline(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("from home", "en"));

    send_at(cx, &rig, 1_000, AppEvent::HomeToggle).unwrap();
    assert_eq!(rig.controller.read_with(cx, |c, _| c.mode()), Mode::Home);
    send_at(cx, &rig, 2_000, AppEvent::HoldDown).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(clipboard(cx).as_deref(), Some("from home"));
}

#[gpui_kit::test]
fn a_home_start_does_not_wait_for_the_result_flash_to_end(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("first", "en"));
    hold_run(cx, &rig, 1_000);
    assert_eq!(machine_state(cx, &rig), State::Done);

    send_at(cx, &rig, 4_100, AppEvent::HomeToggle).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Listening);
}

#[gpui_kit::test]
fn esc_is_grabbed_only_while_a_session_runs(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let calls = Rc::new(RefCell::new(Vec::new()));
    rig.controller.update(cx, |controller, _| {
        let calls = Rc::clone(&calls);
        controller.attach_keys(
            KeysStatus::Available,
            Rc::new(move |active| calls.borrow_mut().push(active)),
        );
    });
    assert!(calls.borrow().is_empty(), "idle takes no key");

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    assert_eq!(*calls.borrow(), vec![true]);
    send_at(cx, &rig, 1_500, AppEvent::Esc).unwrap();
    assert_eq!(*calls.borrow(), vec![true, false], "the cancel lets Esc go");

    send_at(cx, &rig, 3_000, AppEvent::Tick).unwrap();
    say(&rig, outcome_text("words", "en"));
    hold_run(cx, &rig, 4_000);
    assert_eq!(
        *calls.borrow(),
        vec![true, false, true, false],
        "one grab for the whole run, released at done"
    );
}

#[gpui_kit::test]
fn the_pipeline_reports_listening_before_a_slow_microphone_has_opened(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let opened_before_grab = Rc::new(RefCell::new(None));
    rig.controller.update(cx, |controller, _| {
        let seen = Rc::clone(&opened_before_grab);
        let mic = Arc::clone(&rig.mic_backend);
        controller.attach_keys(
            KeysStatus::Available,
            Rc::new(move |active| {
                if active {
                    *seen.borrow_mut() = Some(mic.starts.load(Ordering::SeqCst));
                }
            }),
        );
    });

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();

    assert_eq!(
        *opened_before_grab.borrow(),
        Some(0),
        "the listening state is published before the blocking microphone start"
    );
    assert_eq!(rig.mic_backend.starts.load(Ordering::SeqCst), 1);
}

#[gpui_kit::test]
fn the_keys_status_is_reported_with_its_reason(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.controller.update(cx, |controller, _| {
        controller.attach_keys(
            KeysStatus::Unavailable(hushpen_platform::keys::Unavailable::wayland()),
            Rc::new(|_| {}),
        );
    });

    let keys = rig.controller.read_with(cx, |c, _| c.keys_json());
    assert_eq!(keys["available"], false);
    assert_eq!(keys["reason"], "wayland");
    assert!(
        keys["message"]
            .as_str()
            .unwrap()
            .contains("Not available on Wayland")
    );
}

#[gpui_kit::test]
fn the_pipeline_json_reports_state_mode_and_session(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let idle = rig.controller.read_with(cx, |c, _| c.pipeline_json());
    assert_eq!(idle["state"], "idle");
    assert!(idle["mode"].is_null());

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    let listening = rig.controller.read_with(cx, |c, _| c.pipeline_json());
    assert_eq!(listening["state"], "listening");
    assert_eq!(listening["mode"], "hold");
    assert_eq!(listening["session"], 1);
}

fn last_insert(cx: &mut TestAppContext, rig: &Rig) -> serde_json::Value {
    rig.controller
        .read_with(cx, |controller, _| controller.last_insert_json())
}

#[gpui_kit::test]
fn a_hold_run_pastes_through_the_inserter_and_leaves_the_clipboard_alone(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("the quick brown fox", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Done);
    let calls = inserter.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "the quick brown fox");
    assert_eq!(calls[0].1.label, "GtkTarget");
    assert_eq!(
        clipboard(cx).as_deref(),
        Some("OLD"),
        "the inserter owns the clipboard"
    );
    let report = last_insert(cx, &rig);
    assert_eq!(report["outcome"], "pasted");
    assert_eq!(report["chord"], "ctrl+v");
    assert_eq!(report["target"], "GtkTarget");
    assert_eq!(report["session"], 1);
    assert!(report["ready_unix_ms"].as_u64().unwrap() > 0);
    assert_eq!(report["restore"], "restored");
}

#[gpui_kit::test]
fn the_per_app_chords_setting_reaches_the_inserter(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("XTerm", Chord::ShiftInsert);
    attach_inserter(cx, &rig, &inserter);
    rig.storage
        .settings
        .set("insert.appChords", json!({"xterm": "ctrl+shift+v"}))
        .unwrap();
    say(&rig, outcome_text("words", "en"));

    hold_run(cx, &rig, 1_000);

    let calls = inserter.calls.lock().unwrap();
    let classes = vec!["xterm".to_owned()];
    assert_eq!(
        choose_x11(&classes, &calls[0].2),
        Method::Paste(Chord::CtrlShiftV)
    );
}

#[gpui_kit::test]
fn a_paste_that_nobody_read_fails_the_run_and_keeps_the_text_for_paste_last(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("Helper", Chord::CtrlV);
    {
        let mut script = inserter.script.lock().unwrap();
        script.outcome = InsertOutcome::Failed;
        script.code = Some("INSERT_NO_RECEIPT");
        script.first_receipt_ms = None;
        script.last_receipt_ms = None;
    }
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("lost words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("INSERT_NO_RECEIPT"));
    let message = rig
        .controller
        .read_with(cx, |c, _| c.notice().unwrap().message.clone());
    assert!(message.contains("Paste last transcript"), "{message}");
    assert_eq!(last_insert(cx, &rig)["outcome"], "failed");
    let chars = rig
        .controller
        .read_with(cx, |c, _| c.pipeline_json()["last_text_chars"].clone());
    assert_eq!(chars, 10, "the text can still be pasted later");
}

#[gpui_kit::test]
fn a_copy_only_result_ends_done_with_a_notice_that_says_why(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("", Chord::CtrlV);
    {
        let mut script = inserter.script.lock().unwrap();
        *script = Report::new("");
        script.outcome = InsertOutcome::CopiedOnly;
        script.note = Some("no-target");
    }
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("copied words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(notice_code(cx, &rig), Some("INSERT_COPIED"));
    assert_eq!(last_insert(cx, &rig)["outcome"], "copied_only");
    assert_eq!(last_insert(cx, &rig)["note"], "no-target");
}

#[gpui_kit::test]
fn a_panic_in_the_paste_path_fails_the_run_and_the_next_run_works(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    inserter
        .panic
        .store(true, std::sync::atomic::Ordering::SeqCst);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("first", "en"));
    say(&rig, outcome_text("second", "en"));

    hold_run(cx, &rig, 1_000);
    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("INSERT_NO_RECEIPT"));

    inserter
        .panic
        .store(false, std::sync::atomic::Ordering::SeqCst);
    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 7_000);
    assert_eq!(machine_state(cx, &rig), State::Done);
}

#[gpui_kit::test]
fn a_home_run_copies_and_never_presses_a_key(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("from home", "en"));

    send_at(cx, &rig, 1_000, AppEvent::HomeToggle).unwrap();
    send_at(cx, &rig, 2_000, AppEvent::HomeToggle).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert!(inserter.calls.lock().unwrap().is_empty());
    assert_eq!(clipboard(cx).as_deref(), Some("from home"));
}

#[gpui_kit::test]
fn a_system_that_cannot_paste_copies_the_text_and_says_why(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.controller.update(cx, |controller, _| {
        controller.attach_insert(InsertSupport::Unavailable(Unavailable::wayland()));
    });
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("wayland words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(clipboard(cx).as_deref(), Some("wayland words"));
    assert_eq!(notice_code(cx, &rig), Some("INSERT_WAYLAND"));
}

#[gpui_kit::test]
fn runs_that_fail_before_the_text_exists_insert_nothing_and_leave_the_clipboard(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(
        &rig,
        JobOutcome::Failed(Failure::new("ENGINE_CRASHED", "the engine died")),
    );
    say(&rig, outcome_text("[BLANK_AUDIO]", "en"));
    say(&rig, outcome_text("works again", "en"));

    hold_run(cx, &rig, 1_000);
    assert_eq!(machine_state(cx, &rig), State::Failed);
    send_at(cx, &rig, 6_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 7_000);
    assert_eq!(notice_code(cx, &rig), Some("ENGINE_NO_SPEECH"));
    assert!(
        inserter.calls.lock().unwrap().is_empty(),
        "nothing was pasted"
    );
    assert_eq!(clipboard(cx).as_deref(), Some("OLD"));

    send_at(cx, &rig, 12_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 13_000);
    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(inserter.calls.lock().unwrap().len(), 1);
}

fn blocked_inserter(outcome: InsertOutcome, code: &'static str) -> Arc<FakeInserter> {
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    {
        let mut script = inserter.script.lock().unwrap();
        *script = Report::new("GtkTarget");
        script.outcome = outcome;
        script.code = Some(code);
        script.restore = hushpen_core::insert::Restore::NotNeeded;
    }
    inserter
}

#[gpui_kit::test]
fn a_keyboard_grab_fails_the_run_with_its_code_and_keeps_clipboard_and_text(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = blocked_inserter(InsertOutcome::BlockedGrab, "INSERT_KEYBOARD_GRABBED");
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("grabbed words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("INSERT_KEYBOARD_GRABBED"));
    let message = rig
        .controller
        .read_with(cx, |c, _| c.notice().unwrap().message.clone());
    assert!(
        message.contains("Another app holds the keyboard"),
        "{message}"
    );
    assert_eq!(clipboard(cx).as_deref(), Some("OLD"));
    assert_eq!(last_insert(cx, &rig)["outcome"], "blocked_grab");
    assert_eq!(last_insert(cx, &rig)["code"], "INSERT_KEYBOARD_GRABBED");
    let chars = rig
        .controller
        .read_with(cx, |c, _| c.pipeline_json()["last_text_chars"].clone());
    assert_eq!(chars, 13, "Paste last transcript still has the text");
}

#[gpui_kit::test]
fn a_secure_field_fails_the_run_with_its_code_and_keeps_clipboard_and_text(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = blocked_inserter(InsertOutcome::BlockedSecure, "INSERT_SECURE_FIELD");
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("secret words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(notice_code(cx, &rig), Some("INSERT_SECURE_FIELD"));
    let message = rig
        .controller
        .read_with(cx, |c, _| c.notice().unwrap().message.clone());
    assert!(message.contains("Secure input is on"), "{message}");
    assert_eq!(clipboard(cx).as_deref(), Some("OLD"));
    assert_eq!(last_insert(cx, &rig)["outcome"], "blocked_secure");
}

#[gpui_kit::test]
fn missing_key_permission_copies_the_text_after_the_guard_wrote_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = blocked_inserter(InsertOutcome::NoPermission, "INSERT_NO_PERMISSION");
    attach_inserter(cx, &rig, &inserter);
    put_on_clipboard(cx, "OLD");
    say(&rig, outcome_text("allowed later", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(notice_code(cx, &rig), Some("INSERT_NO_PERMISSION"));
    assert_eq!(last_insert(cx, &rig)["outcome"], "no_permission");
    assert_eq!(clipboard(cx).as_deref(), Some("allowed later"));
}

struct Scripted(hushpen_core::permission::Permissions);

impl hushpen_core::permission::Preflight for Scripted {
    fn microphone(&self) -> hushpen_core::permission::Access {
        self.0.microphone
    }

    fn post_event(&self) -> hushpen_core::permission::Access {
        self.0.accessibility
    }

    fn listen_event(&self) -> hushpen_core::permission::Access {
        self.0.input_monitoring
    }
}

#[gpui_kit::test]
fn the_permissions_state_lists_every_key_and_the_grants_lost_since_the_last_start(
    cx: &mut TestAppContext,
) {
    use hushpen_core::permission::{Access, Permissions};
    let rig = rig(cx, &["base"], "base");
    let unattached = rig.controller.read_with(cx, |c, _| c.permissions_json());
    assert_eq!(unattached["microphone"], "notApplicable");
    assert_eq!(unattached["lost"], json!([]));

    rig.storage
        .settings
        .set_internal(
            "permissions.lastGranted",
            json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
        )
        .unwrap();
    let preflight = Arc::new(Scripted(Permissions {
        microphone: Access::Granted,
        accessibility: Access::Denied,
        input_monitoring: Access::Granted,
    }));
    rig.controller
        .update(cx, |controller, _| controller.attach_permissions(preflight));

    let state = rig.controller.read_with(cx, |c, _| c.permissions_json());
    assert_eq!(state["microphone"], "granted");
    assert_eq!(state["accessibility"], "denied");
    assert_eq!(state["inputMonitoring"], "granted");
    assert_eq!(state["lost"], json!(["accessibility"]));
    let lost = rig
        .controller
        .read_with(cx, |c, _| c.lost_permissions().len());
    assert_eq!(lost, 1);
}
