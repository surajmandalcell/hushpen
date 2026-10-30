//! How finished text reaches the focused app: the paste worker, the clipboard fallback,
//! and "Paste last transcript".

use super::{Controller, InsertSupport, LastInsert, Notice, failure_for, undo};
use crate::hook;
use gpui_kit::{ClipboardItem, Context};
use hushpen_core::dictation::{AppEvent, Delivery};
use hushpen_core::error::{
    INSERT_NO_PERMISSION, INSERT_NO_RECEIPT, INSERT_NO_TRANSCRIPT, INSERT_WAYLAND,
};
use hushpen_core::insert::flow::Step;
use hushpen_core::insert::{Outcome, Overrides, Report};
use hushpen_core::onboarding::KeyGate;
use hushpen_platform::insert::{Inserter, Target};
use hushpen_platform::keys::Reason;
use serde_json::Value;
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

const INSERT_COPIED: &str = "INSERT_COPIED";

impl Controller {
    /// Puts the finished text into the focused app, or copies it when it cannot. The text is
    /// kept for "Paste last transcript" before anything can fail.
    pub(super) fn insert(
        &mut self,
        session: u64,
        text: String,
        delivery: Delivery,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) {
        if self.gate == KeyGate::Practice {
            hook::record_event("onboarding", "practice-text");
            self.practice = Some(text);
            queue.push_back(AppEvent::Inserted { session });
            return;
        }
        self.last_text = Some(text.clone());
        let ready_unix_ms = unix_ms();
        if delivery == Delivery::Paste
            && let InsertSupport::Ready(inserter) = &self.insert
        {
            let inserter = Arc::clone(inserter);
            if self.paste(session, &text, inserter, ready_unix_ms, None) {
                return;
            }
        }
        let note = match (delivery, &self.insert) {
            (Delivery::Copy, _) => "home",
            (_, InsertSupport::Unavailable(_)) => "unavailable",
            _ => "no-inserter",
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        let mut report = Report::new("");
        report.outcome = Outcome::CopiedOnly;
        report.note = Some(note);
        report.restore = hushpen_core::insert::Restore::NotNeeded;
        store_report(&self.last_insert, session, ready_unix_ms, &report);
        if delivery == Delivery::Paste
            && let InsertSupport::Unavailable(why) = &self.insert
        {
            let code = if why.reason == Reason::Wayland {
                INSERT_WAYLAND
            } else {
                INSERT_COPIED
            };
            self.notice = Some(Notice {
                code,
                message: why.message.clone(),
            });
        }
        queue.push_back(AppEvent::Inserted { session });
    }

    /// Puts the last final text into the focused app again. The text is not saved as a new
    /// history row and does not change the shown transcript.
    pub(super) fn paste_last(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        let Some(text) = self.last_text.clone() else {
            let (code, message) = failure_for(INSERT_NO_TRANSCRIPT);
            self.notice = Some(Notice {
                code,
                message: message.to_owned(),
            });
            hook::record_event("insert", "paste-last none");
            return;
        };
        hook::record_event("insert", "paste-last");
        self.paste_text(text, None, cx);
    }

    /// Puts `text` into the focused app, or copies it when the system cannot paste. With
    /// `undo`, the app's undo key goes first, and the row changes once the text was taken.
    pub(super) fn paste_text(
        &mut self,
        text: String,
        undo: Option<undo::Undoing>,
        cx: &mut Context<Self>,
    ) {
        let session = self.machine.session();
        let ready_unix_ms = unix_ms();
        if let InsertSupport::Ready(inserter) = &self.insert {
            let inserter = Arc::clone(inserter);
            let undo_target = undo.as_ref().map(|undoing| undoing.target.clone());
            if self.paste(session, &text, inserter, ready_unix_ms, undo_target) {
                self.pasting_last = Some(session);
                self.pasting_text = Some(text);
                self.undoing = undo;
                return;
            }
        }
        let note = match &self.insert {
            InsertSupport::Unavailable(_) => "unavailable",
            _ => "no-inserter",
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        let mut report = Report::new("");
        report.outcome = Outcome::CopiedOnly;
        report.note = Some(note);
        report.restore = hushpen_core::insert::Restore::NotNeeded;
        store_report(&self.last_insert, session, ready_unix_ms, &report);
        if let InsertSupport::Unavailable(why) = &self.insert {
            self.notice = Some(Notice {
                code: if why.reason == Reason::Wayland {
                    INSERT_WAYLAND
                } else {
                    INSERT_COPIED
                },
                message: why.message.clone(),
            });
        }
    }

    /// Takes the answer of a "Paste last transcript" off the queue. Only a failure says
    /// anything; the machine never hears about either.
    pub(super) fn paste_last_answered(&mut self, event: &AppEvent, cx: &mut Context<Self>) -> bool {
        let (session, code) = match event {
            AppEvent::Inserted { session } => (*session, None),
            AppEvent::InsertFailed { session, code } => (*session, Some(*code)),
            _ => return false,
        };
        if self.pasting_last != Some(session) {
            return false;
        }
        self.pasting_last = None;
        let undoing = self.undoing.take();
        match (code, undoing) {
            (None, Some(undoing)) => self.commit_undo(undoing),
            (Some(_), Some(_)) => hook::record_event("undo", "failed"),
            _ => {}
        }
        if let Some(code) = code {
            let (code, message) = failure_for(code);
            self.notice = Some(Notice {
                code,
                message: message.to_owned(),
            });
            hook::record_event("insert", &format!("paste-last failed {code}"));
            if code == INSERT_NO_PERMISSION
                && let Some(text) = self.pasting_text.take().or_else(|| self.last_text.clone())
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
        }
        self.pasting_text = None;
        true
    }

    /// Hands the paste to a worker thread. The machine hears `Inserted` or `InsertFailed` once
    /// the target read the text or the wait ran out, and the clipboard comes back after that.
    /// Returns false when the thread could not start.
    pub(super) fn paste(
        &mut self,
        session: u64,
        text: &str,
        inserter: Arc<dyn Inserter>,
        ready_unix_ms: u64,
        undo_first: Option<Target>,
    ) -> bool {
        let target = inserter.target();
        self.paste_target = Some(target.clone());
        let overrides = Overrides::from_value(
            &self
                .storage
                .settings
                .get("insert.appChords")
                .unwrap_or(Value::Null),
        );
        let text = text.to_owned();
        let events = self.events.clone();
        let last = Arc::clone(&self.last_insert);
        let turn = Arc::clone(&self.paste_turn);
        let spawned = (self.spawn)(Box::new(move || {
            // The machine sends the next insert once this one settled, while this thread may
            // still have to put the user's clipboard back.
            let _turn = turn.lock().unwrap_or_else(|p| p.into_inner());
            let mut settled = false;
            let finish = |report: &Report, settled: &mut bool| {
                if std::mem::replace(settled, true) {
                    return;
                }
                let event = match (report.outcome, report.code) {
                    (Outcome::Pasted | Outcome::CopiedOnly, _) => AppEvent::Inserted { session },
                    (_, code) => AppEvent::InsertFailed {
                        session,
                        code: code.unwrap_or(INSERT_NO_RECEIPT),
                    },
                };
                let _ = events.unbounded_send((event, None));
            };
            if let Some(undo_target) = &undo_first
                && let Err(code) = undo::press_undo(inserter.as_ref(), undo_target)
            {
                let _ = events.unbounded_send((AppEvent::InsertFailed { session, code }, None));
                return;
            }
            let ran = catch_unwind(AssertUnwindSafe(|| {
                inserter.insert(&text, &target, &overrides, &mut |step| match step {
                    Step::ChordSent(report) => {
                        store_report(&last, session, ready_unix_ms, report);
                        hook::record_event("insert", &chord_detail(report));
                    }
                    Step::Settled(report) => {
                        store_report(&last, session, ready_unix_ms, report);
                        hook::record_event("insert", &format!("settled {}", report.outcome.key()));
                        finish(report, &mut settled);
                    }
                    Step::Restored(report) => {
                        store_report(&last, session, ready_unix_ms, report);
                        hook::record_event("insert", &format!("restore {}", report.restore.key()));
                    }
                })
            }));
            match ran {
                Ok(report) => {
                    store_report(&last, session, ready_unix_ms, &report);
                    finish(&report, &mut settled);
                }
                Err(_) => {
                    log::error!("{INSERT_NO_RECEIPT} the paste path panicked");
                    if !settled {
                        let _ = events.unbounded_send((
                            AppEvent::InsertFailed {
                                session,
                                code: INSERT_NO_RECEIPT,
                            },
                            None,
                        ));
                    }
                }
            }
        }));
        if let Err(error) = spawned {
            log::warn!("{INSERT_NO_RECEIPT} could not start the paste: {error}");
            return false;
        }
        true
    }

    /// `hookctl state` section `last_insert`.
    pub fn last_insert_json(&self) -> Value {
        match &*self.last_insert.lock().unwrap_or_else(|p| p.into_inner()) {
            Some((session, ready, report)) => report.to_json(*session, *ready),
            None => Value::Null,
        }
    }

    /// Whether the newest insert only reached the clipboard.
    pub fn copied_only(&self) -> bool {
        let slot = self.last_insert.lock().unwrap_or_else(|p| p.into_inner());
        matches!(&*slot, Some((_, _, report)) if report.outcome == Outcome::CopiedOnly)
    }

    /// The words for a notice after a copy that did not press a paste key.
    pub(super) fn copy_notice(&mut self, session: u64) {
        let slot = self.last_insert.lock().unwrap_or_else(|p| p.into_inner());
        let Some((owner, _, report)) = &*slot else {
            return;
        };
        if *owner != session || report.outcome != Outcome::CopiedOnly {
            return;
        }
        let message = match report.note {
            Some("no-target") => "No app had the focus, so the text is on your clipboard.",
            Some("override") => "Paste is off for this app, so the text is on your clipboard.",
            Some("own-window") => "Hushpen had the focus, so the text is on your clipboard.",
            Some("no-paste") => {
                "Hushpen could not press the paste key, so the text is on your clipboard."
            }
            _ => return,
        };
        drop(slot);
        self.notice = Some(Notice {
            code: INSERT_COPIED,
            message: message.to_owned(),
        });
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(0))
}

fn store_report(last: &LastInsert, session: u64, ready_unix_ms: u64, report: &Report) {
    *last.lock().unwrap_or_else(|p| p.into_inner()) =
        Some((session, ready_unix_ms, report.clone()));
}

fn chord_detail(report: &Report) -> String {
    format!(
        "chord {} {}",
        report.chord.map_or("none", |chord| chord.key()),
        report.target
    )
}
