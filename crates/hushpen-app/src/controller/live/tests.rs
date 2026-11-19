use super::{Draft, LiveError, LiveSpan, Target, Writer};
use crate::controller::testkit::*;
use gpui_kit::TestAppContext;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::Chord;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

const BEFORE: &str = "Before: ";

#[derive(Default)]
struct Field {
    text: String,
    edits: Vec<(usize, String)>,
    lose_next_receipt: bool,
    moved_caret: Option<mpsc::Sender<()>>,
}

struct Span {
    field: Arc<Mutex<Field>>,
    expected: String,
}

impl LiveSpan for Span {
    fn unchanged(&self) -> bool {
        let field = self.field.lock().unwrap();
        field.moved_caret.is_none() && field.text == self.expected
    }

    fn at_caret(&self) -> bool {
        let field = self.field.lock().unwrap();
        if let Some(observed) = &field.moved_caret {
            observed.send(()).unwrap();
            return false;
        }
        field.text == self.expected
    }

    fn replace(&mut self, keep: usize, suffix: &str) -> Result<(), LiveError> {
        let mut field = self.field.lock().unwrap();
        if field.moved_caret.is_some() || field.text != self.expected {
            return Err(LiveError::Changed);
        }
        field.text.truncate(BEFORE.len() + keep);
        field.text.push_str(suffix);
        field.edits.push((keep, suffix.into()));
        if std::mem::take(&mut field.lose_next_receipt) {
            return Err(LiveError::Uncertain);
        }
        self.expected.clone_from(&field.text);
        Ok(())
    }

    fn splice(&mut self, start: usize, end: usize, with: &str) -> Result<(), LiveError> {
        let mut field = self.field.lock().unwrap();
        if field.moved_caret.is_some() || field.text != self.expected {
            return Err(LiveError::Changed);
        }
        field.text.replace_range(BEFORE.len() + start..BEFORE.len() + end, with);
        field.edits.push((start, with.into()));
        self.expected.clone_from(&field.text);
        Ok(())
    }
}

fn draft() -> (Draft, Arc<Mutex<Field>>) {
    let field = Arc::new(Mutex::new(Field {
        text: BEFORE.into(),
        ..Default::default()
    }));
    let span = Span {
        field: field.clone(),
        expected: BEFORE.into(),
    };
    (
        Draft {
            target: Target {
                window: Some(7),
                ..Default::default()
            },
            span: Ok(Box::new(span)),
            stable: Default::default(),
            typed: String::new(),
        },
        field,
    )
}

#[test]
fn stable_partial_then_minimal_final_edit_then_one_cleanup_replacement() {
    let (mut draft, field) = draft();
    assert_eq!(draft.partial("rain"), "rain");
    assert_eq!(draft.partial("rain cats"), "cats");
    assert_eq!(draft.partial("rain cats now"), "now");
    draft.finish_raw("rain cars now");
    assert_eq!(draft.finish("rain cars now", "Rain cars now."), Some(""));
    let field = field.lock().unwrap();
    assert_eq!(field.text, "Before: Rain cars now.");
    assert_eq!(
        field.edits,
        [
            (0, "rain".into()),
            (4, " cats".into()),
            (7, "rs now".into()),
            (10, "now.".into()),
            (0, "Rain ".into()),
        ]
    );
}

#[test]
fn cleanup_fixes_only_the_changed_words_in_a_long_draft() {
    let (mut draft, field) = draft();
    let raw = "we read the plan and uh we liked it, you know, a lot and then we left";
    draft.partial(raw);
    draft.partial(raw);
    draft.finish_raw(raw);
    let cleaned = "We read the plan and we liked it a lot and then we left.";
    assert_eq!(draft.finish(raw, cleaned), Some(""));
    let field = field.lock().unwrap();
    assert_eq!(field.text, format!("{BEFORE}{cleaned}"));
    assert!(
        field.edits.iter().all(|(_, with)| with != cleaned),
        "{:?}",
        field.edits
    );
}

#[test]
fn user_typing_interrupts_further_live_edits_and_only_the_remainder_is_pasted() {
    let (mut draft, field) = draft();
    draft.partial("red red");
    draft.partial("red red boat");
    field.lock().unwrap().text.push_str(" [my note]");
    assert_eq!(draft.partial("red red boat sailing"), "boat sailing");
    draft.finish_raw("red red boat sailing");
    let remainder = draft
        .finish("red red boat sailing", "Red red boat sailing.")
        .unwrap();
    let mut field = field.lock().unwrap();
    field.text.push_str(remainder);
    assert_eq!(field.text, "Before: red red [my note] boat sailing");
}

#[test]
fn late_cleanup_cannot_overwrite_the_users_edit_to_the_final_transcript() {
    let (mut draft, field) = draft();
    draft.partial("meet");
    draft.partial("meet tomorrow");
    draft.finish_raw("meet tomorrow");
    field.lock().unwrap().text.push('?');
    assert_eq!(draft.finish("meet tomorrow", "Meet tomorrow."), Some(""));
    draft.cancel();
    assert_eq!(field.lock().unwrap().text, "Before: meet tomorrow?");
}

#[test]
fn final_punctuation_appends_without_erasing_the_draft() {
    let (mut draft, field) = draft();
    draft.partial("Meet me");
    draft.partial("Meet me there");
    draft.finish_raw("Meet me there");
    assert_eq!(draft.finish("Meet me there", "Meet me there."), Some(""));
    let field = field.lock().unwrap();
    assert_eq!(field.text, "Before: Meet me there.");
    assert_eq!(
        field.edits.last(),
        Some(&("Meet me there".len(), ".".into()))
    );
}

#[test]
fn writer_stops_on_a_caret_move_during_silence_even_if_the_caret_returns() {
    let (draft, field) = draft();
    let (send_preview, preview) = mpsc::channel();
    let (writer, partial) = Writer::start(
        draft,
        Arc::new(move |tail| send_preview.send(tail).unwrap()),
    )
    .unwrap();
    let deadline = Duration::from_secs(3);
    partial("go".into());
    assert_eq!(preview.recv_timeout(deadline).unwrap(), "go");
    partial("go north".into());
    assert_eq!(preview.recv_timeout(deadline).unwrap(), "north");
    assert_eq!(field.lock().unwrap().text, "Before: go");

    let (observed, probe) = mpsc::channel();
    field.lock().unwrap().moved_caret = Some(observed);
    probe
        .recv_timeout(deadline)
        .expect("a caret check during silence");
    field.lock().unwrap().moved_caret = None;
    partial("go north now".into());
    assert_eq!(preview.recv_timeout(deadline).unwrap(), "north now");

    let mut draft = writer.stop().unwrap();
    draft.finish_raw("go north now.");
    assert_eq!(
        draft.finish("go north now.", "Go north now."),
        Some(" north now.")
    );
    assert_eq!(field.lock().unwrap().text, "Before: go");
}

#[test]
fn a_lost_receipt_after_a_real_write_prevents_automatic_retry() {
    let (mut draft, field) = draft();
    field.lock().unwrap().lose_next_receipt = true;
    draft.partial("go");
    draft.partial("go now");
    draft.finish_raw("go now");
    assert_eq!(draft.finish("go now", "Go now."), None);
    draft.cancel();
    let field = field.lock().unwrap();
    assert_eq!(field.text, "Before: go");
    assert_eq!(field.edits.len(), 1);
}

#[gpui_kit::test]
fn a_refused_fallback_paste_copies_only_the_missing_words(cx: &mut TestAppContext) {
    let rig = rig(cx, &["base"], "base");
    let inserter = crate::controller::tests::blocked_inserter(
        hushpen_core::insert::Outcome::NoPermission,
        hushpen_core::error::INSERT_NO_PERMISSION,
    );
    attach_inserter(cx, &rig, &inserter);
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    let (mut draft, field) = draft();
    draft.partial("blue");
    draft.partial("blue boat");
    field.lock().unwrap().text.push_str(" [my note]");
    rig.controller.update(cx, |controller, _| {
        controller.run_of(controller.machine.session()).draft = Some(draft);
    });
    say(&rig, outcome_text("blue boat sails", "en"));
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Failed);
    assert_eq!(inserter.calls.lock().unwrap()[0].0, " boat sails");
    assert_eq!(clipboard(cx).as_deref(), Some(" boat sails"));
    assert_eq!(field.lock().unwrap().text, "Before: blue [my note]");
}

#[gpui_kit::test]
fn escape_removes_the_draft_recover_inserts_it_once_and_one_undo_removes_it(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx, &["base"], "base");
    let inserter = FakeInserter::pasted_into("Target", Chord::CtrlV);
    attach_inserter(cx, &rig, &inserter);
    say(&rig, outcome_text("red red boat", "en"));
    send_at(cx, &rig, 1_000, AppEvent::HoldDown).unwrap();
    let (mut draft, field) = draft();
    draft.partial("red red");
    draft.partial("red red boat");
    rig.controller.update(cx, |controller, _| {
        let session = controller.machine.session();
        controller.run_of(session).draft = Some(draft);
    });
    assert_eq!(field.lock().unwrap().text, "Before: red red");
    send_at(cx, &rig, 2_800, AppEvent::Esc).unwrap();
    send_at(cx, &rig, 3_000, AppEvent::HoldUp).unwrap();
    settle(cx, &rig);
    assert_eq!(field.lock().unwrap().text, BEFORE);

    rig.controller
        .update(cx, |controller, cx| controller.recover(cx))
        .unwrap();
    settle(cx, &rig);
    assert_eq!(machine_state(cx, &rig), State::Done);
    assert_eq!(field.lock().unwrap().text, "Before: red red boat");
    rig.controller
        .update(cx, |controller, cx| controller.undo_cleanup(cx))
        .unwrap();
    assert_eq!(field.lock().unwrap().text, BEFORE);
    assert!(inserter.log.lock().unwrap().is_empty());
}
