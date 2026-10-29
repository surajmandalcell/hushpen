use super::Speaker;
use super::testkit::*;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::Chord;
use hushpen_core::onboarding::KeyGate;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const SELECTION: &str = "the zebrafinch sings at dawn";

/// A player that keeps what it was asked to play.
#[derive(Default)]
struct FakePlayer {
    spoken: RefCell<Vec<String>>,
    playing: Cell<bool>,
    stops: Cell<usize>,
    refusal: RefCell<Option<String>>,
}

fn setup(
    cx: &mut TestAppContext,
    selected: Option<&str>,
) -> (Rig, Arc<FakeInserter>, Rc<FakePlayer>) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("Target-gtk.py", Chord::CtrlV);
    *inserter.selected.lock().unwrap() = selected.map(str::to_owned);
    attach_inserter(cx, &rig, &inserter);
    let player = Rc::new(FakePlayer::default());
    let speaker = Speaker {
        playing: Rc::new({
            let player = Rc::clone(&player);
            move |_| player.playing.get()
        }),
        stop: Rc::new({
            let player = Rc::clone(&player);
            move |_| {
                player.playing.set(false);
                player.stops.set(player.stops.get() + 1);
            }
        }),
        speak: Rc::new({
            let player = Rc::clone(&player);
            move |text, _| {
                if let Some(refusal) = player.refusal.borrow().clone() {
                    return Err(refusal);
                }
                player.spoken.borrow_mut().push(text);
                player.playing.set(true);
                Ok(())
            }
        }),
    };
    rig.controller
        .update(cx, |controller, _| controller.attach_speaker(speaker));
    (rig, inserter, player)
}

/// Presses the shortcut and runs the selection read it queued, if any.
fn press(cx: &mut TestAppContext, rig: &Rig, at: u64) -> Result<(), String> {
    let sent = send_at(cx, rig, at, AppEvent::ReadAloud);
    let job = rig.work.borrow_mut().pop_front();
    if let Some(job) = job {
        job();
    }
    cx.run_until_parked();
    sent
}

fn flash(cx: &mut TestAppContext, rig: &Rig) -> Option<(&'static str, String)> {
    rig.controller.read_with(cx, |c, _| {
        c.flash().map(|flash| (flash.code, flash.message.clone()))
    })
}

#[gpui_kit::test]
fn the_shortcut_reads_the_selection_out_loud_and_leaves_no_trace(cx: &mut TestAppContext) {
    let (rig, inserter, player) = setup(cx, Some(SELECTION));

    press(cx, &rig, 1_000).unwrap();

    assert_eq!(*player.spoken.borrow(), [SELECTION]);
    assert_eq!(inserter.selection_reads.load(Ordering::SeqCst), 1);
    assert!(
        inserter.calls.lock().unwrap().is_empty(),
        "nothing is pasted"
    );
    assert_eq!(machine_state(cx, &rig), State::Idle);
    assert_eq!(history_rows(&rig), 0);
    assert!(rig.mic_backend.starts.load(Ordering::SeqCst) == 0);
    assert_eq!(flash(cx, &rig), None);
}

#[gpui_kit::test]
fn the_shortcut_stops_a_reading_that_plays(cx: &mut TestAppContext) {
    let (rig, inserter, player) = setup(cx, Some(SELECTION));
    press(cx, &rig, 1_000).unwrap();

    press(cx, &rig, 2_000).unwrap();

    assert_eq!(player.stops.get(), 1);
    assert!(!player.playing.get());
    assert_eq!(player.spoken.borrow().len(), 1, "it does not read again");
    assert_eq!(inserter.selection_reads.load(Ordering::SeqCst), 1);
}

#[gpui_kit::test]
fn nothing_selected_says_so_on_the_flow_bar(cx: &mut TestAppContext) {
    let (rig, _, player) = setup(cx, Some("   "));

    press(cx, &rig, 1_000).unwrap();

    assert!(player.spoken.borrow().is_empty());
    assert_eq!(
        flash(cx, &rig),
        Some(("READ_NO_SELECTION", "Select text first".to_owned()))
    );
}

#[gpui_kit::test]
fn a_selection_read_that_never_answers_counts_as_no_selection(cx: &mut TestAppContext) {
    let (rig, _, player) = setup(cx, Some(SELECTION));
    rig.mic.update(cx, |mic, _| mic.hold_listing());

    send_at(cx, &rig, 1_000, AppEvent::ReadAloud).unwrap();
    // The read job runs only after the wait, as when the read hangs.
    let late = rig.work.borrow_mut().pop_front().unwrap();
    cx.executor()
        .advance_clock(super::command::SELECTION_WAIT + std::time::Duration::from_millis(100));
    cx.run_until_parked();
    late();
    cx.run_until_parked();

    assert!(player.spoken.borrow().is_empty());
    assert_eq!(flash(cx, &rig).unwrap().0, "READ_NO_SELECTION");
}

#[gpui_kit::test]
fn a_player_that_refuses_shows_its_reason(cx: &mut TestAppContext) {
    let (rig, _, player) = setup(cx, Some(SELECTION));
    *player.refusal.borrow_mut() = Some("Download the Kokoro model in Models.".into());

    press(cx, &rig, 1_000).unwrap();

    assert_eq!(
        flash(cx, &rig),
        Some((
            "SPEECH_FAILED",
            "Download the Kokoro model in Models.".to_owned()
        ))
    );
}

#[gpui_kit::test]
fn the_shortcut_reads_nothing_while_a_dictation_listens(cx: &mut TestAppContext) {
    let (rig, inserter, player) = setup(cx, Some(SELECTION));
    send_at(cx, &rig, 1_000, AppEvent::HandsFreeToggle).unwrap();

    press(cx, &rig, 2_000).unwrap();

    assert_eq!(machine_state(cx, &rig), State::Listening);
    assert_eq!(inserter.selection_reads.load(Ordering::SeqCst), 0);
    assert!(player.spoken.borrow().is_empty());
}

#[gpui_kit::test]
fn onboarding_refuses_the_shortcut(cx: &mut TestAppContext) {
    let (rig, inserter, _) = setup(cx, Some(SELECTION));
    for gate in [KeyGate::Closed, KeyGate::Practice] {
        set_gate(cx, &rig, gate);
        assert!(press(cx, &rig, 1_000).is_err(), "{gate:?}");
    }
    assert_eq!(inserter.selection_reads.load(Ordering::SeqCst), 0);
}
