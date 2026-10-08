//! The insertion sequence: save the clipboard, write the text, send the paste chord, wait for the
//! target to read the text, and put the clipboard back.
//!
//! The platform supplies a [`Backend`] and a [`Clock`]; this module owns the order and the
//! timing, so the restore rules are tested without a display or a pasteboard.

use super::method::{Chord, Method, Selection};
use super::report::{Outcome, Report, Restore};
use crate::error::{INSERT_NO_PERMISSION, INSERT_NO_RECEIPT};

/// How long to wait for the first read after the chord. Without one the paste failed.
pub const RECEIPT_WAIT_MS: u64 = 2_000;
/// After the last read, how long the clipboard must stay unread before it is put back.
pub const QUIET_MS: u64 = 200;
/// The longest the text stays on the clipboard, from the write.
pub const RESTORE_CAP_MS: u64 = 8_000;
const POLL_MS: u64 = 5;

pub trait Clock {
    fn now_ms(&self) -> u64;
    fn sleep_ms(&self, ms: u64);
}

/// When the target read the text, on the clock's scale. `None` until the first read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Receipts {
    pub first: Option<u64>,
    pub last: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordError {
    /// The system does not let Hushpen send keys.
    NoPermission,
    /// No way to send keys here (Wayland, no XTest). The text stays on the clipboard.
    Unavailable,
}

pub trait Backend {
    /// What was on the selection before: every format, or nothing.
    type Snapshot;

    fn snapshot(&mut self, selection: Selection) -> Self::Snapshot;
    /// Makes Hushpen the owner of the selection with the text, and clears the receipts.
    fn write(&mut self, selection: Selection, text: &str) -> Result<(), String>;
    /// Presses the chord, then lifts every modifier it pressed.
    fn send_chord(&mut self, chord: Chord) -> Result<(), ChordError>;
    fn receipts(&mut self) -> Receipts;
    /// Hushpen still owns the selection: nobody copied since the write.
    fn still_ours(&mut self, selection: Selection) -> bool;
    fn restore(&mut self, selection: Selection, snapshot: Self::Snapshot);
}

pub struct Request<'a> {
    pub text: &'a str,
    /// The app name for the report.
    pub target: &'a str,
    pub method: Method,
    /// Why the method is copy-only, for the report.
    pub copy_note: Option<&'static str>,
}

pub enum Step<'a> {
    /// The chord was sent.
    ChordSent(&'a Report),
    /// The target read the text, or the wait for a read ran out.
    Settled(&'a Report),
    /// The restore is over.
    Restored(&'a Report),
}

/// Times that decide when the clipboard comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timeline {
    pub written_at: u64,
    pub chord_at: u64,
    pub receipts: Receipts,
}

/// Whether the wait is over and the restore may run.
pub fn restore_due(timeline: &Timeline, now: u64) -> bool {
    match timeline.receipts.last {
        Some(last) => {
            now.saturating_sub(last) >= QUIET_MS
                || now.saturating_sub(timeline.written_at) >= RESTORE_CAP_MS
        }
        None => now.saturating_sub(timeline.chord_at) >= RECEIPT_WAIT_MS,
    }
}

/// Puts the snapshot back when it is dropped, unless the flow finished it. A panic in the paste
/// path unwinds through here.
struct Guard<'a, B: Backend> {
    backend: &'a mut B,
    selection: Selection,
    snapshot: Option<B::Snapshot>,
}

impl<B: Backend> Guard<'_, B> {
    fn finish(&mut self) -> Restore {
        match self.snapshot.take() {
            Some(snapshot) if self.backend.still_ours(self.selection) => {
                self.backend.restore(self.selection, snapshot);
                Restore::Restored
            }
            Some(_) => Restore::SkippedNewerCopy,
            None => Restore::NotNeeded,
        }
    }

    /// The text stays on the clipboard on purpose.
    fn keep_text(&mut self) {
        self.snapshot = None;
    }
}

impl<B: Backend> Drop for Guard<'_, B> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn since(start: u64, at: u64) -> u64 {
    at.saturating_sub(start)
}

pub fn run<B: Backend>(
    backend: &mut B,
    clock: &dyn Clock,
    request: &Request<'_>,
    observe: &mut dyn FnMut(Step<'_>),
) -> Report {
    let started = clock.now_ms();
    let mut report = Report::new(request.target);
    let chord = match request.method {
        Method::Paste(chord) => chord,
        Method::CopyOnly => {
            report.note = request.copy_note.or(Some("copy-only"));
            return copy_only(backend, request.text, report, observe);
        }
    };
    let selection = chord.selection();
    report.chord = Some(chord);
    report.selection = Some(selection);

    let snapshot = backend.snapshot(selection);
    let mut guard = Guard {
        backend,
        selection,
        snapshot: Some(snapshot),
    };
    if guard.backend.write(selection, request.text).is_err() {
        guard.keep_text();
        report.outcome = Outcome::Failed;
        report.code = Some(INSERT_NO_RECEIPT);
        report.restore = Restore::NotNeeded;
        observe(Step::Settled(&report));
        return report;
    }
    let written_at = clock.now_ms();
    match guard.backend.send_chord(chord) {
        Ok(()) => {}
        Err(error) => {
            guard.keep_text();
            report.restore = Restore::NotNeeded;
            report.chord = None;
            report.selection = None;
            match error {
                ChordError::NoPermission => {
                    report.outcome = Outcome::NoPermission;
                    report.code = Some(INSERT_NO_PERMISSION);
                }
                ChordError::Unavailable => {
                    report.outcome = Outcome::CopiedOnly;
                    report.note = Some("no-paste");
                }
            }
            observe(Step::Settled(&report));
            return report;
        }
    }
    let chord_at = clock.now_ms();
    report.chord_sent_ms = Some(since(started, chord_at));
    observe(Step::ChordSent(&report));

    let mut settled = false;
    loop {
        let receipts = guard.backend.receipts();
        let now = clock.now_ms();
        report.first_receipt_ms = receipts.first.map(|at| since(started, at));
        report.last_receipt_ms = receipts.last.map(|at| since(started, at));
        if !settled && (receipts.first.is_some() || since(chord_at, now) >= RECEIPT_WAIT_MS) {
            settled = true;
            if receipts.first.is_some() {
                report.outcome = Outcome::Pasted;
            } else {
                report.outcome = Outcome::Failed;
                report.code = Some(INSERT_NO_RECEIPT);
            }
            observe(Step::Settled(&report));
        }
        let timeline = Timeline {
            written_at,
            chord_at,
            receipts,
        };
        if restore_due(&timeline, now) {
            break;
        }
        clock.sleep_ms(POLL_MS);
    }
    report.restore = guard.finish();
    report.restored_ms = Some(since(started, clock.now_ms()));
    observe(Step::Restored(&report));
    report
}

/// The text is the copy, so there is nothing to put back and nothing to wait for.
fn copy_only<B: Backend>(
    backend: &mut B,
    text: &str,
    mut report: Report,
    observe: &mut dyn FnMut(Step<'_>),
) -> Report {
    report.restore = Restore::NotNeeded;
    if backend.write(Selection::Clipboard, text).is_ok() {
        report.outcome = Outcome::CopiedOnly;
    } else {
        report.outcome = Outcome::Failed;
        report.code = Some(INSERT_NO_RECEIPT);
    }
    observe(Step::Settled(&report));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insert::method::Chord;
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;

    struct FakeClock(Rc<Cell<u64>>);

    impl Clock for FakeClock {
        fn now_ms(&self) -> u64 {
            self.0.get()
        }
        fn sleep_ms(&self, ms: u64) {
            self.0.set(self.0.get() + ms);
        }
    }

    /// A clipboard with scripted target behavior.
    struct FakeBoard {
        now: Rc<Cell<u64>>,
        log: Rc<RefCell<Vec<String>>>,
        /// The text on the selection before the run.
        before: Option<&'static str>,
        /// The target reads this many ms after the chord, and again after each gap.
        reads_after: Vec<u64>,
        /// Someone else copies this many ms after the chord.
        copy_after: Option<u64>,
        chord_error: Option<ChordError>,
        panic_on_chord: bool,
        write_error: bool,
        chord_at: Option<u64>,
    }

    impl FakeBoard {
        fn new(now: &Rc<Cell<u64>>) -> Self {
            Self {
                now: Rc::clone(now),
                log: Rc::default(),
                before: Some("OLD"),
                reads_after: Vec::new(),
                copy_after: None,
                chord_error: None,
                panic_on_chord: false,
                write_error: false,
                chord_at: None,
            }
        }

        fn note(&self, text: impl Into<String>) {
            self.log.borrow_mut().push(text.into());
        }
    }

    impl Backend for FakeBoard {
        type Snapshot = Option<&'static str>;

        fn snapshot(&mut self, selection: Selection) -> Self::Snapshot {
            self.note(format!("snapshot {}", selection.key()));
            self.before
        }

        fn write(&mut self, selection: Selection, text: &str) -> Result<(), String> {
            if self.write_error {
                return Err("no owner".into());
            }
            self.note(format!("write {} {text}", selection.key()));
            Ok(())
        }

        fn send_chord(&mut self, chord: Chord) -> Result<(), ChordError> {
            if self.panic_on_chord {
                panic!("the paste path broke");
            }
            if let Some(error) = self.chord_error {
                return Err(error);
            }
            self.chord_at = Some(self.now.get());
            self.note(format!("chord {}", chord.key()));
            Ok(())
        }

        fn receipts(&mut self) -> Receipts {
            let Some(chord_at) = self.chord_at else {
                return Receipts::default();
            };
            let now = self.now.get();
            let seen: Vec<u64> = self
                .reads_after
                .iter()
                .map(|after| chord_at + after)
                .filter(|at| *at <= now)
                .collect();
            Receipts {
                first: seen.first().copied(),
                last: seen.last().copied(),
            }
        }

        fn still_ours(&mut self, _selection: Selection) -> bool {
            match (self.copy_after, self.chord_at) {
                (Some(after), Some(chord_at)) => self.now.get() < chord_at + after,
                _ => true,
            }
        }

        fn restore(&mut self, selection: Selection, snapshot: Self::Snapshot) {
            self.note(format!(
                "restore {} {}",
                selection.key(),
                snapshot.unwrap_or("<empty>")
            ));
        }
    }

    struct Rig {
        now: Rc<Cell<u64>>,
        board: FakeBoard,
    }

    fn rig() -> Rig {
        let now = Rc::new(Cell::new(10_000));
        Rig {
            board: FakeBoard::new(&now),
            now,
        }
    }

    fn paste(rig: &mut Rig, chord: Chord) -> (Report, Vec<String>) {
        let request = Request {
            text: "hello",
            target: "Gtk-target",
            method: Method::Paste(chord),
            copy_note: None,
        };
        let mut steps = Vec::new();
        let report = run(
            &mut rig.board,
            &FakeClock(Rc::clone(&rig.now)),
            &request,
            &mut |step| {
                steps.push(
                    match step {
                        Step::ChordSent(_) => "chord",
                        Step::Settled(_) => "settled",
                        Step::Restored(_) => "restored",
                    }
                    .to_owned(),
                );
            },
        );
        (report, steps)
    }

    #[test]
    fn a_read_then_200_ms_of_quiet_puts_the_clipboard_back() {
        let mut rig = rig();
        rig.board.reads_after = vec![40];

        let (report, steps) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.outcome, Outcome::Pasted);
        assert_eq!(report.restore, Restore::Restored);
        assert_eq!(report.first_receipt_ms, Some(40));
        let restored = report.restored_ms.unwrap();
        assert!(
            (240..=250).contains(&restored),
            "restore at 40 + 200 ms, got {restored}"
        );
        assert_eq!(
            *rig.board.log.borrow(),
            [
                "snapshot clipboard",
                "write clipboard hello",
                "chord ctrl+v",
                "restore clipboard OLD"
            ]
        );
        assert_eq!(steps, ["chord", "settled", "restored"]);
    }

    #[test]
    fn a_second_read_inside_the_quiet_time_pushes_the_restore_back() {
        let mut rig = rig();
        rig.board.reads_after = vec![40, 180];

        let (report, _) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.last_receipt_ms, Some(180));
        assert!(report.restored_ms.unwrap() >= 380);
        assert_eq!(report.restore, Restore::Restored);
    }

    #[test]
    fn with_no_read_the_clipboard_comes_back_at_the_2_s_cap_and_the_paste_failed() {
        let mut rig = rig();

        let (report, steps) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.outcome, Outcome::Failed);
        assert_eq!(report.code, Some("INSERT_NO_RECEIPT"));
        assert_eq!(report.restore, Restore::Restored);
        let restored = report.restored_ms.unwrap();
        assert!(
            (2_000..=2_010).contains(&restored),
            "restore at the 2 s cap, got {restored}"
        );
        assert_eq!(steps, ["chord", "settled", "restored"]);
    }

    #[test]
    fn a_newer_user_copy_is_never_overwritten() {
        let mut rig = rig();
        rig.board.copy_after = Some(500);

        let (report, _) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.restore, Restore::SkippedNewerCopy);
        assert!(
            !rig.board
                .log
                .borrow()
                .iter()
                .any(|line| line.starts_with("restore")),
            "the new copy stays"
        );
    }

    #[test]
    fn a_newer_copy_during_a_read_also_wins() {
        let mut rig = rig();
        rig.board.reads_after = vec![40];
        rig.board.copy_after = Some(100);

        let (report, _) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.outcome, Outcome::Pasted);
        assert_eq!(report.restore, Restore::SkippedNewerCopy);
    }

    #[test]
    fn a_read_that_never_goes_quiet_still_ends_at_the_8_s_cap() {
        let mut rig = rig();
        rig.board.reads_after = (0..90).map(|n| 40 + n * 100).collect();

        let (report, _) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.restore, Restore::Restored);
        let restored = report.restored_ms.unwrap();
        assert!(
            (8_000..=8_010).contains(&restored),
            "the cap is 8 s from the write, got {restored}"
        );
    }

    #[test]
    fn an_empty_clipboard_before_the_run_is_restored_empty() {
        let mut rig = rig();
        rig.board.before = None;
        rig.board.reads_after = vec![10];

        let _ = paste(&mut rig, Chord::CtrlV);

        assert!(
            rig.board
                .log
                .borrow()
                .contains(&"restore clipboard <empty>".to_owned())
        );
    }

    #[test]
    fn shift_insert_works_on_the_primary_selection_only() {
        let mut rig = rig();
        rig.board.reads_after = vec![30];

        let (report, _) = paste(&mut rig, Chord::ShiftInsert);

        assert_eq!(report.selection, Some(Selection::Primary));
        assert_eq!(
            *rig.board.log.borrow(),
            [
                "snapshot primary",
                "write primary hello",
                "chord shift+insert",
                "restore primary OLD"
            ]
        );
    }

    #[test]
    fn a_panic_in_the_paste_path_still_restores_the_clipboard() {
        let mut rig = rig();
        rig.board.panic_on_chord = true;
        let log = Rc::clone(&rig.board.log);

        let result = catch_unwind(AssertUnwindSafe(|| paste(&mut rig, Chord::CtrlV)));

        assert!(result.is_err());
        assert_eq!(
            log.borrow().last().map(String::as_str),
            Some("restore clipboard OLD")
        );
    }

    #[test]
    fn a_chord_that_cannot_be_sent_leaves_the_text_copied() {
        let mut rig = rig();
        rig.board.chord_error = Some(ChordError::Unavailable);

        let (report, steps) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.outcome, Outcome::CopiedOnly);
        assert_eq!(report.restore, Restore::NotNeeded);
        assert!(
            !rig.board
                .log
                .borrow()
                .iter()
                .any(|line| line.starts_with("restore")),
            "the copy is the result"
        );
        assert_eq!(steps, ["settled"]);
    }

    #[test]
    fn missing_permission_reports_its_code_and_keeps_the_copy() {
        let mut rig = rig();
        rig.board.chord_error = Some(ChordError::NoPermission);

        let (report, _) = paste(&mut rig, Chord::CmdV);

        assert_eq!(report.outcome, Outcome::NoPermission);
        assert_eq!(report.code, Some("INSERT_NO_PERMISSION"));
        assert_eq!(report.restore, Restore::NotNeeded);
    }

    #[test]
    fn copy_only_writes_the_clipboard_and_sends_nothing() {
        let mut rig = rig();
        let request = Request {
            text: "hello",
            target: "",
            method: Method::CopyOnly,
            copy_note: Some("no-target"),
        };

        let report = run(
            &mut rig.board,
            &FakeClock(Rc::clone(&rig.now)),
            &request,
            &mut |_| {},
        );

        assert_eq!(report.outcome, Outcome::CopiedOnly);
        assert_eq!(report.note, Some("no-target"));
        assert_eq!(*rig.board.log.borrow(), ["write clipboard hello"]);
    }

    #[test]
    fn a_write_that_fails_pastes_nothing() {
        let mut rig = rig();
        rig.board.write_error = true;

        let (report, _) = paste(&mut rig, Chord::CtrlV);

        assert_eq!(report.outcome, Outcome::Failed);
        assert!(
            !rig.board
                .log
                .borrow()
                .iter()
                .any(|line| line.starts_with("chord")),
        );
    }

    #[test]
    fn restore_is_due_after_quiet_the_cap_or_the_receipt_wait() {
        let at = |first: Option<u64>, last: Option<u64>| Timeline {
            written_at: 1_000,
            chord_at: 1_010,
            receipts: Receipts { first, last },
        };
        assert!(!restore_due(&at(None, None), 3_009));
        assert!(restore_due(&at(None, None), 3_010));
        assert!(!restore_due(&at(Some(1_050), Some(1_050)), 1_249));
        assert!(restore_due(&at(Some(1_050), Some(1_050)), 1_250));
        assert!(restore_due(&at(Some(1_050), Some(8_900)), 9_000));
    }
}
