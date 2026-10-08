use super::testkit::*;
use super::{KeysStatus, Phase};
use gpui_kit::TestAppContext;
use hushpen_audio::CaptureError;
use hushpen_core::dictation::{AppEvent, Mode, State};
use hushpen_engine::{Failure, JobOutcome};
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
