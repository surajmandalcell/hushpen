//! What the onboarding gate lets through: no start while closed, the words of a practice
//! dictation go to onboarding and nowhere else, and an open gate is the normal pipeline.

use super::testkit::*;
use super::tests::hold_run;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::onboarding::KeyGate;
use hushpen_store::history;

fn set_gate(cx: &mut TestAppContext, rig: &Rig, gate: KeyGate) {
    rig.controller
        .update(cx, |controller, cx| controller.set_gate(gate, cx));
}

fn history_rows(rig: &Rig) -> usize {
    history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows
        .len()
}

#[gpui_kit::test]
fn a_closed_gate_refuses_every_start_and_opens_no_microphone(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_gate(cx, &rig, KeyGate::Closed);

    for event in [
        AppEvent::HoldDown,
        AppEvent::HandsFreeToggle,
        AppEvent::HomeToggle,
        AppEvent::FlowBarClick,
        AppEvent::PasteLast,
    ] {
        assert!(
            send_at(cx, &rig, 1_000, event.clone()).is_err(),
            "{event:?}"
        );
    }

    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(
        rig.mic_backend
            .starts
            .load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the microphone never opened"
    );
    assert!(rig.specs.lock().unwrap().is_empty());
}

#[gpui_kit::test]
fn a_practice_dictation_goes_to_onboarding_and_leaves_no_trace(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_gate(cx, &rig, KeyGate::Practice);
    put_on_clipboard(cx, "USER-CLIP");
    say(&rig, outcome_text("the quick brown fox", "en"));

    hold_run(cx, &rig, 1_000);

    let words = rig
        .controller
        .update(cx, |controller, _| controller.take_practice());
    assert_eq!(words.as_deref(), Some("the quick brown fox"));
    assert_eq!(
        rig.controller.update(cx, |c, _| c.take_practice()),
        None,
        "the words are handed over once"
    );
    assert_eq!(clipboard(cx).as_deref(), Some("USER-CLIP"), "no copy");
    assert_eq!(history_rows(&rig), 0, "a practice run saves no row");
    assert!(session_wavs(&rig).is_empty(), "and keeps no audio");
    assert!(kept_wavs(&rig).is_empty());
    assert!(
        send_at(cx, &rig, 9_000, AppEvent::PasteLast).is_err(),
        "paste last has nothing to repeat"
    );
}

#[gpui_kit::test]
fn the_button_and_the_key_both_practice(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_gate(cx, &rig, KeyGate::Practice);
    say(&rig, outcome_text("from the button", "en"));

    send_at(cx, &rig, 1_000, AppEvent::HomeToggle).unwrap();
    send_at(cx, &rig, 4_000, AppEvent::HomeToggle).unwrap();
    settle(cx, &rig);

    let words = rig.controller.update(cx, |c, _| c.take_practice());
    assert_eq!(words.as_deref(), Some("from the button"));
}

#[gpui_kit::test]
fn an_open_gate_is_the_normal_pipeline(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    set_gate(cx, &rig, KeyGate::Closed);
    set_gate(cx, &rig, KeyGate::Open);
    say(&rig, outcome_text("normal words", "en"));

    hold_run(cx, &rig, 1_000);

    assert_eq!(rig.controller.update(cx, |c, _| c.take_practice()), None);
    assert_eq!(history_rows(&rig), 1);
    assert_eq!(clipboard(cx).as_deref(), Some("normal words"));
}
