//! Sessions that overlap: a start never waits for the session before it, and the older one
//! still pastes its text and saves its row, first.

use super::testkit::*;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::Chord;
use hushpen_store::history;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

/// The rows by their text, since two rows saved in one millisecond have no order.
fn rows(rig: &Rig) -> Vec<history::Row> {
    let mut rows = history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows;
    rows.sort_by(|a, b| a.final_text.cmp(&b.final_text));
    rows
}

/// Session 1 is held from 1 s to 3 s and stops; session 2 starts 40 ms later and is held to
/// 5 s. Nothing that runs on a worker has run yet.
fn two_quick_holds(cx: &mut TestAppContext, rig: &Rig) {
    send_at(cx, rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, rig, 3_000, AppEvent::HoldUp).unwrap();
    assert_eq!(machine_state(cx, rig), State::Transcribing);
    assert!(
        !rig.work.borrow().is_empty(),
        "the first recording still finishes on a worker"
    );
    send_at(cx, rig, 3_040, AppEvent::HoldDown).unwrap();
    assert_eq!(machine_state(cx, rig), State::Listening, "no wait");
    assert_eq!(rig.mic_backend.starts.load(Ordering::SeqCst), 2);
    send_at(cx, rig, 5_000, AppEvent::HoldUp).unwrap();
}

#[gpui_kit::test]
fn two_quick_dictations_both_paste_in_order_and_each_saves_its_row(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("first words", "en"));
    say(&rig, outcome_text("second words", "en"));

    two_quick_holds(cx, &rig);
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(
        *inserter.log.lock().unwrap(),
        ["insert first words", "insert second words"]
    );
    let rows = rows(&rig);
    let saved: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row.final_text.as_deref(),
                row.status.as_str(),
                row.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        saved,
        [
            (Some("first words"), "completed", 1_000),
            (Some("second words"), "completed", 1_000),
        ]
    );
    assert_eq!(
        kept_wavs(&rig).len(),
        2,
        "each row keeps its whole recording"
    );
    assert!(session_wavs(&rig).is_empty());
}

#[gpui_kit::test]
fn cancelling_the_new_dictation_leaves_the_older_one_to_finish(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("first words", "en"));

    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    send_at(cx, &rig, 3_040, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_500, AppEvent::Esc).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    settle(cx, &rig);

    assert!(!rig.tokens.lock().unwrap()[0].is_cancelled());
    assert_eq!(*inserter.log.lock().unwrap(), ["insert first words"]);
    let rows = rows(&rig);
    assert_eq!(rows.len(), 1, "the cancelled recording leaves no row");
    assert_eq!(rows[0].final_text.as_deref(), Some("first words"));
    assert_eq!(kept_wavs(&rig).len(), 1);
    assert!(session_wavs(&rig).is_empty(), "the cancelled audio is gone");
}

#[gpui_kit::test]
fn a_start_during_the_result_flash_listens_at_once(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("first", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Done);

    for start in [AppEvent::HoldDown, AppEvent::FlowBarClick] {
        send_at(cx, &rig, 3_050, AppEvent::Esc).unwrap();
        send_at(cx, &rig, 3_050, start.clone()).unwrap();
        assert_eq!(machine_state(cx, &rig), State::Listening, "{start:?}");
        send_at(cx, &rig, 3_060, AppEvent::Esc).unwrap();
    }
}

#[gpui_kit::test]
fn the_record_button_starts_again_while_the_last_words_are_read(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    say(&rig, outcome_text("first take", "en"));
    say(&rig, outcome_text("second take", "en"));
    let toggle = |cx: &mut TestAppContext| rig.dictation.update(cx, |me, cx| me.toggle(cx));

    toggle(cx).unwrap();
    toggle(cx).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Transcribing);
    assert!(cx.update(|cx| rig.dictation.read(cx).can_record(cx)));
    toggle(cx).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Listening);
    toggle(cx).unwrap();
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(clipboard(cx).as_deref(), Some("second take"));
    let finals: Vec<_> = rows(&rig).into_iter().map(|row| row.final_text).collect();
    assert_eq!(
        finals,
        [
            Some("first take".to_owned()),
            Some("second take".to_owned())
        ]
    );
}

/// Runs one worker job on its own thread. Returns whether it ended within `wait`.
fn run_on_thread(job: Box<dyn FnOnce() + Send>, wait: Duration) -> (bool, mpsc::Receiver<()>) {
    let (done, ended) = mpsc::channel();
    std::thread::spawn(move || {
        job();
        let _ = done.send(());
    });
    let finished = ended.recv_timeout(wait).is_ok();
    (finished, ended)
}

#[gpui_kit::test]
fn the_next_paste_waits_until_the_last_one_gave_the_clipboard_back(cx: &mut TestAppContext) {
    // The paste threads here are real ones, so their events arrive from other threads.
    cx.executor().allow_parking();
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("GtkTarget", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    let (release, restore) = mpsc::channel();
    *inserter.restore_wait.lock().unwrap() = Some(restore);
    say(&rig, outcome_text("first words", "en"));
    say(&rig, outcome_text("second words", "en"));
    two_quick_holds(cx, &rig);

    // Run the jobs as real threads would, until the first paste hangs in its restore and the
    // second paste is waiting for its turn.
    let mut waiting = Vec::new();
    for _ in 0..20 {
        cx.run_until_parked();
        let Some(job) = rig.work.borrow_mut().pop_front() else {
            break;
        };
        let (finished, ended) = run_on_thread(job, Duration::from_millis(300));
        if !finished {
            waiting.push(ended);
        }
        if waiting.len() == 2 {
            break;
        }
    }
    assert_eq!(waiting.len(), 2, "both pastes are under way");
    assert_eq!(
        *inserter.log.lock().unwrap(),
        ["insert first words"],
        "the second paste has not touched the clipboard"
    );

    release.send(()).unwrap();
    for ended in waiting {
        ended.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    settle(cx, &rig);
    assert_eq!(
        *inserter.log.lock().unwrap(),
        ["insert first words", "insert second words"]
    );
    assert_eq!(machine_state(cx, &rig), State::Done);
}
