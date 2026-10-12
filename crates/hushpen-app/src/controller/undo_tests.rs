use super::ai_tests::{RULE, SPOKEN, ai_answers, only_row, pasted, setup};
use super::testkit::*;
use super::tests::hold_run;
use crate::ai_cleanup::Attempt;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::AppEvent;
use hushpen_core::insert::{Chord, Outcome};
use hushpen_platform::insert::UndoError;
use serde_json::json;

const CORRECTED: &str = "I think we should meet on Wednesday at 3 PM.";

fn offered(cx: &mut TestAppContext, rig: &Rig) -> bool {
    rig.controller
        .read_with(cx, |controller, _| controller.undo_offered())
}

fn undo(cx: &mut TestAppContext, rig: &Rig) -> Result<(), String> {
    let result = rig
        .controller
        .update(cx, |controller, cx| controller.undo_cleanup(cx));
    settle(cx, rig);
    result
}

fn cleaned_run(cx: &mut TestAppContext) -> (Rig, std::sync::Arc<FakeInserter>) {
    let (rig, inserter) = setup(cx);
    ai_answers(cx, &rig, Attempt::Done(CORRECTED.into()), Ok(()));
    hold_run(cx, &rig, 1_000);
    (rig, inserter)
}

#[gpui_kit::test]
fn an_ai_text_that_a_normal_app_took_is_offered_for_undo(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);

    assert_eq!(pasted(&inserter), [CORRECTED]);
    assert!(offered(cx, &rig));
    let state = rig
        .controller
        .read_with(cx, |controller, _| controller.pipeline_json());
    assert_eq!(state["undo"]["offered"], true);
    assert_eq!(state["undo"]["row_id"], json!(only_row(&rig).id));
}

#[gpui_kit::test]
fn undo_presses_undo_then_pastes_the_rule_text_and_the_row_follows(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    let row_id = only_row(&rig).id;
    let revision = rig
        .controller
        .read_with(cx, |controller, _| controller.history_revision());

    undo(cx, &rig).unwrap();

    assert_eq!(
        *inserter.log.lock().unwrap(),
        [
            format!("insert {CORRECTED}"),
            "undo".to_owned(),
            format!("insert {RULE}")
        ],
        "the undo key goes first, then the rule text"
    );
    let row = only_row(&rig);
    assert_eq!(row.id, row_id, "no new row");
    assert_eq!(row.final_text.as_deref(), Some(RULE));
    assert_eq!(row.llm_outcome.as_deref(), Some("undone"));
    assert_eq!(row.llm_text.as_deref(), Some(CORRECTED));
    assert_eq!(row.raw_text.as_deref(), Some(SPOKEN));
    assert!(!offered(cx, &rig), "one undo is all there is");
    rig.controller.read_with(cx, |controller, _| {
        assert!(controller.history_revision() > revision);
        assert_eq!(
            controller.pipeline_json()["last_text_chars"],
            RULE.chars().count()
        );
    });
}

#[gpui_kit::test]
fn paste_last_after_an_undo_inserts_the_rule_text(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    undo(cx, &rig).unwrap();

    send_at(cx, &rig, 20_000, AppEvent::PasteLast).unwrap();
    settle(cx, &rig);

    assert_eq!(pasted(&inserter).last().map(String::as_str), Some(RULE));
    assert_eq!(pasted(&inserter).len(), 3);
}

#[gpui_kit::test]
fn the_offer_ends_five_seconds_after_the_insertion(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    rig.now.set(3_000 + 4_900);
    assert!(offered(cx, &rig), "still on offer just before five seconds");
    rig.now.set(3_000 + 5_100);
    assert!(!offered(cx, &rig));

    let refused = undo(cx, &rig);

    assert!(refused.is_err());
    assert!(inserter.undos.lock().unwrap().is_empty());
    assert_eq!(pasted(&inserter), [CORRECTED], "nothing was pasted again");
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("used"));
}

#[gpui_kit::test]
fn a_change_of_focus_withdraws_the_offer_for_good(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    inserter.target.lock().unwrap().window = Some(8);
    assert!(!offered(cx, &rig));
    rig.controller
        .update(cx, |controller, cx| controller.tick(cx));
    inserter.target.lock().unwrap().window = Some(7);

    assert!(!offered(cx, &rig), "coming back does not revive it");
    assert!(undo(cx, &rig).is_err());
    assert!(inserter.undos.lock().unwrap().is_empty());
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("used"));
}

#[gpui_kit::test]
fn an_undo_for_another_focus_is_refused_before_any_key(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    inserter.target.lock().unwrap().window = Some(8);

    assert!(undo(cx, &rig).is_err());

    assert!(inserter.undos.lock().unwrap().is_empty());
    assert_eq!(pasted(&inserter), [CORRECTED]);
}

#[gpui_kit::test]
fn a_timeout_with_the_rule_text_offers_no_undo(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    ai_answers(cx, &rig, Attempt::TimedOut, Ok(()));
    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    assert!(!offered(cx, &rig));
    assert!(undo(cx, &rig).is_err());
}

#[gpui_kit::test]
fn with_ai_cleanup_off_there_is_no_undo(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    assert!(!offered(cx, &rig));
}

#[gpui_kit::test]
fn a_blocked_insertion_has_no_receipt_and_no_undo(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    {
        let mut script = inserter.script.lock().unwrap();
        script.outcome = Outcome::BlockedGrab;
        script.code = Some("INSERT_KEYBOARD_GRABBED");
        script.first_receipt_ms = None;
        script.last_receipt_ms = None;
        script.chord_sent_ms = None;
    }
    ai_answers(cx, &rig, Attempt::Done(CORRECTED.into()), Ok(()));
    hold_run(cx, &rig, 1_000);

    assert!(!offered(cx, &rig));
    assert!(inserter.undos.lock().unwrap().is_empty());
}

#[gpui_kit::test]
fn a_terminal_gets_no_offer_and_no_undo_key(cx: &mut TestAppContext) {
    let (rig, _) = setup(cx);
    let terminal = FakeInserter::pasted_into("XTerm", Chord::ShiftInsert);
    attach_inserter(cx, &rig, &terminal);
    ai_answers(cx, &rig, Attempt::Done(CORRECTED.into()), Ok(()));
    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&terminal), [CORRECTED]);
    assert!(!offered(cx, &rig));
    assert!(undo(cx, &rig).is_err());
    assert!(terminal.undos.lock().unwrap().is_empty());
    assert_eq!(
        *terminal.log.lock().unwrap(),
        [format!("insert {CORRECTED}")]
    );
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("used"));
}

#[gpui_kit::test]
fn a_new_dictation_ends_the_offer(cx: &mut TestAppContext) {
    let (rig, _) = cleaned_run(cx);
    say(&rig, outcome_text(SPOKEN, "en"));

    send_at(cx, &rig, 4_700, AppEvent::Tick).unwrap();
    send_at(cx, &rig, 4_800, AppEvent::HoldDown).unwrap();

    assert_eq!(
        machine_state(cx, &rig),
        hushpen_core::dictation::State::Listening
    );
    assert!(!offered(cx, &rig));
}

#[gpui_kit::test]
fn a_refused_undo_key_leaves_the_row_and_the_text_alone(cx: &mut TestAppContext) {
    let (rig, inserter) = cleaned_run(cx);
    *inserter.undo_error.lock().unwrap() = Some(UndoError::NoPermission);

    assert!(undo(cx, &rig).is_ok(), "the paste step reports the failure");

    assert_eq!(
        pasted(&inserter),
        [CORRECTED],
        "the rule text was not pasted"
    );
    let row = only_row(&rig);
    assert_eq!(row.llm_outcome.as_deref(), Some("used"));
    assert_eq!(row.final_text.as_deref(), Some(CORRECTED));
}

#[gpui_kit::test]
fn undoing_in_history_takes_the_offer_away_and_changes_the_text_for_paste_last(
    cx: &mut TestAppContext,
) {
    let (rig, inserter) = cleaned_run(cx);
    let id = only_row(&rig).id;

    rig.controller.update(cx, |controller, cx| {
        controller.cleanup_undone(&id, RULE.to_owned(), cx)
    });

    assert!(!offered(cx, &rig));
    send_at(cx, &rig, 20_000, AppEvent::PasteLast).unwrap();
    settle(cx, &rig);
    assert_eq!(pasted(&inserter).last().map(String::as_str), Some(RULE));
    assert!(inserter.undos.lock().unwrap().is_empty(), "no key goes out");
}
