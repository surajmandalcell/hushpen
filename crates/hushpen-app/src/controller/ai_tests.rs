use super::CleanerSource;
use super::testkit::*;
use super::tests::hold_run;
use crate::ai_cleanup::{Attempt, Cleaner, Progress};
use crate::controller::CancelToken;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::Chord;
use hushpen_store::history::{self, Row};
use serde_json::json;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub(crate) const SPOKEN: &str =
    "um so I think we should meet on tuesday no wait wednesday at like three pm";
pub(crate) const RULE: &str =
    "So I think we should meet on tuesday no wait wednesday at like three pm.";

#[derive(Default)]
pub(crate) struct Calls {
    pub(crate) asked: usize,
    pub(crate) seen: Vec<(String, Duration)>,
}

struct Scripted {
    prepare: Result<(), String>,
    attempt: Attempt,
    calls: Arc<Mutex<Calls>>,
}

impl Cleaner for Scripted {
    fn prepare(&self, _: &CancelToken, _: &Progress) -> Result<(), String> {
        self.prepare.clone()
    }

    fn clean(&self, text: &str, timeout: Duration, _: &CancelToken) -> Attempt {
        self.calls
            .lock()
            .unwrap()
            .seen
            .push((text.to_owned(), timeout));
        self.attempt.clone()
    }
}

/// Turns AI cleanup on and answers every dictation with `attempt`.
pub(crate) fn ai_answers(
    cx: &mut TestAppContext,
    rig: &Rig,
    attempt: Attempt,
    prepare: Result<(), String>,
) -> Arc<Mutex<Calls>> {
    rig.storage
        .settings
        .set("cleanup.llm.enabled", json!(true))
        .unwrap();
    let calls: Arc<Mutex<Calls>> = Arc::default();
    let seen = Arc::clone(&calls);
    let source: CleanerSource = Rc::new(move || {
        seen.lock().unwrap().asked += 1;
        Some(Box::new(Scripted {
            prepare: prepare.clone(),
            attempt: attempt.clone(),
            calls: Arc::clone(&seen),
        }) as Box<dyn Cleaner>)
    });
    rig.controller
        .update(cx, |controller, _| controller.attach_ai(source));
    calls
}

pub(crate) fn setup(cx: &mut TestAppContext) -> (Rig, Arc<FakeInserter>) {
    let rig = rig(cx, &["base"], "base");
    rig.storage
        .settings
        .set("cleanup.rules", json!(true))
        .unwrap();
    let inserter = FakeInserter::pasted_into("Target-gtk.py", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text(SPOKEN, "en"));
    (rig, inserter)
}

pub(crate) fn only_row(rig: &Rig) -> Row {
    let rows = history::page(&rig.storage.database, "", None, 50)
        .unwrap()
        .rows;
    assert_eq!(rows.len(), 1, "exactly one row: {rows:?}");
    rows.into_iter().next().unwrap()
}

pub(crate) fn pasted(inserter: &FakeInserter) -> Vec<String> {
    inserter
        .calls
        .lock()
        .unwrap()
        .iter()
        .map(|(text, _, _)| text.clone())
        .collect()
}

fn ai_state(cx: &mut TestAppContext, rig: &Rig) -> serde_json::Value {
    rig.controller
        .read_with(cx, |controller, _| controller.pipeline_json()["ai"].clone())
}

#[gpui_kit::test]
fn ai_cleanup_is_off_by_default_and_the_rule_text_goes_in(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let calls: Arc<Mutex<Calls>> = Arc::default();
    let seen = Arc::clone(&calls);
    rig.controller.update(cx, |controller, _| {
        controller.attach_ai(Rc::new(move || {
            seen.lock().unwrap().asked += 1;
            None
        }))
    });

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    let row = only_row(&rig);
    assert_eq!(row.llm_outcome.as_deref(), Some("off"));
    assert_eq!(row.llm_text, None);
    assert_eq!(row.final_text.as_deref(), Some(RULE));
    assert_eq!(calls.lock().unwrap().asked, 0, "no worker is touched");
}

#[gpui_kit::test]
fn the_endpoint_provider_runs_with_its_own_six_second_timeout(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let corrected = "I think we should meet on Wednesday at 3 PM.";
    let calls = ai_answers(cx, &rig, Attempt::Done(corrected.into()), Ok(()));
    rig.storage
        .settings
        .set("cleanup.llm.provider", json!("endpoint"))
        .unwrap();
    rig.storage
        .settings
        .set("cleanup.llm.timeoutMs", json!(1_234))
        .unwrap();

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [corrected]);
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("used"));
    let calls = calls.lock().unwrap();
    assert_eq!(calls.asked, 1);
    assert_eq!(calls.seen[0].1, Duration::from_secs(6));
}

#[gpui_kit::test]
fn the_endpoint_timeout_setting_is_the_one_that_counts(cx: &mut TestAppContext) {
    let (rig, _inserter) = setup(cx);
    let calls = ai_answers(cx, &rig, Attempt::TimedOut, Ok(()));
    rig.storage
        .settings
        .set("cleanup.llm.provider", json!("endpoint"))
        .unwrap();
    rig.storage
        .settings
        .set("cleanup.llm.endpointTimeoutMs", json!(2_500))
        .unwrap();

    hold_run(cx, &rig, 1_000);

    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("timeout"));
    assert_eq!(
        calls.lock().unwrap().seen[0].1,
        Duration::from_millis(2_500)
    );
}

#[gpui_kit::test]
fn a_self_correction_is_inserted_corrected_and_the_row_keeps_every_text(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let corrected = "I think we should meet on Wednesday at 3 PM.";
    let calls = ai_answers(cx, &rig, Attempt::Done(corrected.into()), Ok(()));

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [corrected]);
    let row = only_row(&rig);
    assert_eq!(row.status, "completed");
    assert_eq!(row.raw_text.as_deref(), Some(SPOKEN));
    assert_eq!(row.rule_text.as_deref(), Some(RULE));
    assert_eq!(row.llm_text.as_deref(), Some(corrected));
    assert_eq!(row.final_text, row.llm_text);
    assert_eq!(row.llm_outcome.as_deref(), Some("used"));
    assert_eq!(
        calls.lock().unwrap().seen[0].0,
        RULE,
        "the model gets the rule text"
    );
    let ai = ai_state(cx, &rig);
    assert_eq!(ai["last"]["outcome"], "used");
    assert_eq!(ai["phase"], serde_json::Value::Null);
}

#[gpui_kit::test]
fn a_timeout_inserts_the_rule_text(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    ai_answers(cx, &rig, Attempt::TimedOut, Ok(()));

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    let row = only_row(&rig);
    assert_eq!(row.llm_outcome.as_deref(), Some("timeout"));
    assert_eq!(row.llm_text, None);
    assert_eq!(row.final_text.as_deref(), Some(RULE));
    assert_eq!(row.status, "completed");
    assert_eq!(ai_state(cx, &rig)["last"]["code"], "LLM_TIMEOUT");
}

#[gpui_kit::test]
fn the_model_gets_the_timeout_setting_and_the_platform_default(cx: &mut TestAppContext) {
    let (rig, _) = setup(cx);
    let calls = ai_answers(cx, &rig, Attempt::TimedOut, Ok(()));
    hold_run(cx, &rig, 1_000);
    let default = if cfg!(target_os = "linux") {
        8_000
    } else {
        3_000
    };
    assert_eq!(
        calls.lock().unwrap().seen[0].1,
        Duration::from_millis(default)
    );

    rig.storage
        .settings
        .set("cleanup.llm.timeoutMs", json!(1_500))
        .unwrap();
    say(&rig, outcome_text(SPOKEN, "en"));
    send_at(cx, &rig, 60_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 70_000);
    assert_eq!(
        calls.lock().unwrap().seen[1].1,
        Duration::from_millis(1_500)
    );
}

#[gpui_kit::test]
fn a_poem_is_rejected_and_the_rule_text_goes_in(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    ai_answers(
        cx,
        &rig,
        Attempt::Done("Soft paws on moonlit halls, cats dream of silver rain.".into()),
        Ok(()),
    );

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    let row = only_row(&rig);
    assert_eq!(row.llm_outcome.as_deref(), Some("guard_rejected"));
    assert_eq!(row.llm_text, None);
}

#[gpui_kit::test]
fn a_worker_failure_inserts_the_rule_text_and_the_next_dictation_still_cleans(
    cx: &mut TestAppContext,
) {
    let (rig, inserter) = setup(cx);
    ai_answers(cx, &rig, Attempt::Failed("LLM_CRASHED".into()), Ok(()));
    hold_run(cx, &rig, 1_000);
    assert_eq!(pasted(&inserter), [RULE]);
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("error"));
    assert_eq!(ai_state(cx, &rig)["last"]["code"], "LLM_CRASHED");

    let corrected = "I think we should meet on Wednesday at 3 PM.";
    ai_answers(cx, &rig, Attempt::Done(corrected.into()), Ok(()));
    say(&rig, outcome_text(SPOKEN, "en"));
    send_at(cx, &rig, 60_000, AppEvent::Tick).unwrap();
    hold_run(cx, &rig, 70_000);
    assert_eq!(pasted(&inserter), [RULE, corrected]);
}

#[gpui_kit::test]
fn a_missing_worker_is_unavailable_and_the_rule_text_goes_in(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    rig.storage
        .settings
        .set("cleanup.llm.enabled", json!(true))
        .unwrap();
    rig.controller
        .update(cx, |controller, _| controller.attach_ai(Rc::new(|| None)));

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    let row = only_row(&rig);
    assert_eq!(row.llm_outcome.as_deref(), Some("unavailable"));
    assert_eq!(row.status, "completed");
    assert_eq!(ai_state(cx, &rig)["last"]["code"], "LLM_UNAVAILABLE");
}

#[gpui_kit::test]
fn a_dictation_with_no_attached_worker_source_is_unavailable(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    rig.storage
        .settings
        .set("cleanup.llm.enabled", json!(true))
        .unwrap();

    hold_run(cx, &rig, 1_000);

    assert_eq!(pasted(&inserter), [RULE]);
    assert_eq!(only_row(&rig).llm_outcome.as_deref(), Some("unavailable"));
}

#[gpui_kit::test]
fn esc_while_cleaning_cancels_and_a_late_answer_changes_nothing(cx: &mut TestAppContext) {
    let (rig, inserter) = setup(cx);
    let calls = ai_answers(
        cx,
        &rig,
        Attempt::Done("I think we should meet on Wednesday at 3 PM.".into()),
        Ok(()),
    );
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    // The speech job runs; the AI job stays queued.
    let transcribe = rig.work.borrow_mut().pop_front().unwrap();
    transcribe();
    cx.run_until_parked();
    assert_eq!(machine_state(cx, &rig), State::Cleaning);

    send_at(cx, &rig, 3_100, AppEvent::Esc).unwrap();
    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    settle(cx, &rig);

    assert_eq!(machine_state(cx, &rig), State::Cancelled);
    assert!(pasted(&inserter).is_empty(), "nothing was inserted");
    assert!(
        calls.lock().unwrap().seen.is_empty(),
        "a cancelled step never reaches the model"
    );
    let row = only_row(&rig);
    assert_eq!(row.status, "cancelled");
    assert_eq!(row.raw_text.as_deref(), Some(SPOKEN));
    assert_eq!(row.final_text, None);
}

#[gpui_kit::test]
fn the_phase_reads_cleaning_text_state_while_the_ai_job_is_queued(cx: &mut TestAppContext) {
    let (rig, _) = setup(cx);
    ai_answers(cx, &rig, Attempt::TimedOut, Ok(()));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    let transcribe = rig.work.borrow_mut().pop_front().unwrap();
    transcribe();
    cx.run_until_parked();
    assert_eq!(machine_state(cx, &rig), State::Cleaning);
    let state = rig
        .controller
        .read_with(cx, |controller, _| controller.pipeline_json());
    assert_eq!(state["state"], "cleaning");
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Done);
}
