//! "Undo cleanup": for five seconds after an AI-cleaned text went into an app, the user can
//! take the cleanup back. The app's own undo key removes the pasted text, then the rule text
//! goes in through the same paste path, and the history row follows.
//!
//! The offer is made only when the target read the text and is no terminal, and it ends the
//! moment another window has the focus, because the undo key goes to whatever has it.

use super::{Controller, InsertSupport};
use crate::hook;
use gpui_kit::Context;
use hushpen_core::insert::Report;
use hushpen_core::insert::undo::{Candidate, Declined, Offer, Standing, decide};
use hushpen_platform::insert::{Inserter, Target, UndoError};
use hushpen_store::history;
use serde_json::{Value, json};
use std::time::Duration;

/// How long the app gets to take the undo key before the rule text is pasted over it.
pub(super) const UNDO_SETTLE: Duration = Duration::from_millis(120);

const NOTHING: &str = "There is nothing to undo.";
const EXPIRED: &str = "The cleanup can no longer be undone.";

/// An offer and the app it belongs to.
pub(super) struct Standby {
    offer: Offer,
    target: Target,
}

/// An undo whose paste is under way: the row changes once the app took the rule text.
pub(super) struct Undoing {
    pub row_id: String,
    pub target: Target,
}

impl Controller {
    fn focus_window(&self) -> Option<u64> {
        match &self.insert {
            InsertSupport::Ready(inserter) => inserter.target().window,
            _ => None,
        }
    }

    fn undo_standing(&self) -> Option<Standing> {
        let standby = self.undo.as_ref()?;
        Some(standby.offer.standing((self.clock)(), self.focus_window()))
    }

    /// A new dictation is news: the offer is over once the machine leaves Idle and Done.
    fn dictating(&self) -> bool {
        !matches!(
            self.machine.state(),
            hushpen_core::dictation::State::Idle | hushpen_core::dictation::State::Done
        )
    }

    /// Whether the bar may offer "Undo cleanup" now. A read: it ends nothing.
    pub fn undo_offered(&self) -> bool {
        self.undo_standing() == Some(Standing::Open) && !self.dictating()
    }

    /// Makes the offer for the session that just saved its row, when the insertion earned one.
    pub(super) fn offer_undo(&mut self, session: u64, row_id: String, ai_text: Option<&str>) {
        self.undo = None;
        let Some(ai_text) = ai_text else { return };
        let report: Option<Report> = {
            let slot = self.last_insert.lock().unwrap_or_else(|p| p.into_inner());
            slot.as_ref()
                .filter(|(owner, _, _)| *owner == session)
                .map(|(_, _, report)| report.clone())
        };
        let (Some(report), Some(target)) = (report, self.paste_target.clone()) else {
            return;
        };
        let candidate = Candidate {
            ai_used: true,
            ai_text,
            rule_text: &self.pending.rule,
            report: &report,
            classes: &target.classes,
            window: target.window,
            own: target.own,
        };
        match decide(&candidate) {
            Ok(window) => {
                let offer = Offer::new(row_id, self.pending.rule.clone(), window, (self.clock)());
                hook::record_event("undo", "offered");
                self.undo = Some(Standby { offer, target });
            }
            Err(Declined::Terminal) => hook::record_event("undo", "declined terminal"),
            Err(why) => hook::record_event("undo", &format!("declined {why:?}")),
        }
    }

    /// Ends an offer that ran out, lost the focus, or that a new dictation made stale.
    pub(super) fn watch_undo(&mut self, cx: &mut Context<Self>) {
        let Some(standing) = self.undo_standing() else {
            return;
        };
        if standing != Standing::Open || self.dictating() {
            self.undo = None;
            hook::record_event(
                "undo",
                match standing {
                    Standing::Expired => "closed expired",
                    Standing::FocusChanged => "closed focus",
                    Standing::Open => "closed new dictation",
                },
            );
            cx.notify();
        }
    }

    /// The flow bar's "Undo cleanup": presses the app's undo key, then pastes the rule text.
    pub fn undo_cleanup(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let standing = self.undo_standing();
        let Some(standby) = self.undo.take() else {
            hook::record_event("undo", "rejected none");
            return Err(NOTHING.to_owned());
        };
        if standing != Some(Standing::Open) || self.dictating() {
            hook::record_event("undo", "rejected closed");
            cx.notify();
            return Err(EXPIRED.to_owned());
        }
        let Standby { offer, target } = standby;
        hook::record_event("undo", "started");
        self.notice = None;
        self.paste_text(
            offer.rule_text,
            Some(Undoing {
                row_id: offer.row_id,
                target,
            }),
            cx,
        );
        cx.notify();
        Ok(())
    }

    /// The row of a finished undo paste: the rule text is final now.
    pub(super) fn commit_undo(&mut self, undoing: Undoing) {
        match history::undo_cleanup(&self.storage.database, &undoing.row_id) {
            Ok(Some(text)) => self.cleanup_taken_back(&undoing.row_id, text),
            Ok(None) => {}
            Err(error) => log::error!(
                "{} the undone row was not saved: {error}",
                hushpen_core::error::HISTORY_WRITE_FAILED
            ),
        }
    }

    /// History switched the final text of a row to its rule text. Nothing is pasted. The offer
    /// for that row goes, and "Paste last transcript" gives the new text.
    pub fn cleanup_undone(&mut self, row_id: &str, text: String, cx: &mut Context<Self>) {
        self.cleanup_taken_back(row_id, text);
        cx.notify();
    }

    fn cleanup_taken_back(&mut self, row_id: &str, text: String) {
        if self
            .undo
            .as_ref()
            .is_some_and(|standby| standby.offer.row_id == row_id)
        {
            self.undo = None;
        }
        if self.last_row.as_deref() == Some(row_id) {
            self.last_text = Some(text.clone());
            self.transcript = text;
        }
        self.history_revision += 1;
        hook::record_event("history", &format!("cleanup-undone {row_id}"));
    }

    /// `hookctl state` `pipeline.undo`.
    pub(super) fn undo_json(&self) -> Value {
        let standby = self.undo.as_ref();
        json!({
            "offered": self.undo_offered(),
            "row_id": standby.map(|standby| standby.offer.row_id.as_str()),
            "remaining_ms": standby.map(|standby| {
                standby.offer.expires_at.saturating_sub((self.clock)())
            }),
        })
    }
}

/// What the paste thread does before the paste of an undo: presses the app's undo key and gives
/// the app a moment to take it. A refusal becomes the failure code of the whole undo.
pub(super) fn press_undo(inserter: &dyn Inserter, target: &Target) -> Result<(), &'static str> {
    use hushpen_core::error::{INSERT_NO_PERMISSION, INSERT_NO_RECEIPT};
    match inserter.undo(target) {
        Ok(()) => {
            hook::record_event("undo", &format!("key sent {}", target.label));
            std::thread::sleep(UNDO_SETTLE);
            Ok(())
        }
        Err(error) => {
            hook::record_event("undo", &format!("key refused {error:?}"));
            Err(match error {
                UndoError::NoPermission => INSERT_NO_PERMISSION,
                _ => INSERT_NO_RECEIPT,
            })
        }
    }
}
