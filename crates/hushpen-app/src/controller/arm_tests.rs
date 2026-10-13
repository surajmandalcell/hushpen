//! The hold key while the Command Mode chord can still follow: armed with no cue, no flow bar,
//! and no session, and started as a plain hold when the chord does not come.

use super::testkit::*;
use super::tests::listen_for_cues;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{ARM_WINDOW_MS, AppEvent, Cue, Mode, State};
use hushpen_core::onboarding::KeyGate;
use std::sync::atomic::Ordering;
use std::time::Duration;

fn mic_starts(rig: &Rig) -> usize {
    rig.mic_backend.starts.load(Ordering::SeqCst)
}

fn past_the_window(cx: &mut TestAppContext) {
    cx.executor()
        .advance_clock(Duration::from_millis(ARM_WINDOW_MS + 30));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn an_armed_hold_shows_nothing_and_plays_nothing_but_keeps_the_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let played = listen_for_cues(cx, &rig);

    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.phase().key()),
        "idle",
        "the flow bar has nothing to show"
    );
    assert!(played.lock().unwrap().is_empty());
    assert_eq!(
        mic_starts(&rig),
        1,
        "the microphone is already filling memory"
    );
}

#[gpui_kit::test]
fn an_armed_hold_starts_with_its_cue_when_the_window_ends(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let played = listen_for_cues(cx, &rig);
    say(&rig, outcome_text("plain words", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();

    past_the_window(cx);

    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(rig.controller.read_with(cx, |c, _| c.mode()), Mode::Hold);
    assert_eq!(*played.lock().unwrap(), vec![(Cue::Start, 0.5)]);
    assert_eq!(mic_starts(&rig), 1, "the start adopted the armed capture");
    assert_eq!(session_wavs(&rig).len(), 1);

    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(rig.specs.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn a_hold_released_after_the_window_counts_from_the_key_press(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("plain words", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();
    // No timer ran: the release itself starts the hold, dated at the key press, so the audio
    // since then is kept and the hold is long enough to count.
    send_at(cx, &rig, 1_300, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(rig.specs.lock().unwrap().len(), 1);
    assert_eq!(mic_starts(&rig), 1);
}

#[gpui_kit::test]
fn a_tap_inside_the_window_is_a_tap_and_leaves_no_audio(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();
    send_at(cx, &rig, 1_050, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert!(rig.specs.lock().unwrap().is_empty());
    assert!(session_wavs(&rig).is_empty());
    assert!(kept_wavs(&rig).is_empty());
}

#[gpui_kit::test]
fn the_window_timer_does_nothing_after_the_chord_took_the_hold(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let played = listen_for_cues(cx, &rig);
    send_at(cx, &rig, 1_000, AppEvent::HoldArm).unwrap();
    send_at(cx, &rig, 1_010, AppEvent::HoldUp).unwrap();

    past_the_window(cx);

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(*played.lock().unwrap(), vec![(Cue::Start, 0.5)], "one cue");
}

#[gpui_kit::test]
fn a_closed_gate_arms_nothing_and_opens_no_microphone(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    rig.controller
        .update(cx, |c, cx| c.set_gate(KeyGate::Closed, cx));

    assert!(send_at(cx, &rig, 1_000, AppEvent::HoldArm).is_err());
    assert_eq!(mic_starts(&rig), 0);
}

#[gpui_kit::test]
fn a_hold_with_no_model_is_refused_at_once_like_before(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], "base");

    assert!(send_at(cx, &rig, 1_000, AppEvent::HoldArm).is_err());
    assert_eq!(mic_starts(&rig), 0);
    assert_eq!(
        rig.controller.read_with(cx, |c, _| c.refused()),
        Some("ENGINE_NO_MODEL")
    );
}

#[gpui_kit::test]
fn a_hold_while_a_dictation_runs_is_not_armed(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    send_at(cx, &rig, 1_000, AppEvent::HandsFreeToggle).unwrap();
    assert_eq!(mic_starts(&rig), 1);

    send_at(cx, &rig, 4_000, AppEvent::HoldArm).unwrap();

    assert_eq!(
        machine_state(cx, &rig),
        State::Transcribing,
        "the hold key stops a hands-free run at once"
    );
}
