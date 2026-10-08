//! What one insertion did, as the test hook and the history row show it.

use super::method::{Chord, Selection};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pasted,
    /// The text is on the clipboard and no key was sent.
    CopiedOnly,
    /// macOS has not allowed Hushpen to post keys. The text is on the clipboard.
    NoPermission,
    Failed,
}

impl Outcome {
    pub fn key(self) -> &'static str {
        match self {
            Outcome::Pasted => "pasted",
            Outcome::CopiedOnly => "copied_only",
            Outcome::NoPermission => "no_permission",
            Outcome::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// The wait for the restore is not over.
    Pending,
    /// The earlier clipboard is back.
    Restored,
    /// The clipboard changed after Hushpen wrote the text, so the new copy stays.
    SkippedNewerCopy,
    /// Nothing was saved, so nothing comes back: the text stays on the clipboard.
    NotNeeded,
}

impl Restore {
    pub fn key(self) -> &'static str {
        match self {
            Restore::Pending => "pending",
            Restore::Restored => "restored",
            Restore::SkippedNewerCopy => "skipped_newer_copy",
            Restore::NotNeeded => "not_needed",
        }
    }
}

/// Times are milliseconds after the text was ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub outcome: Outcome,
    pub code: Option<&'static str>,
    /// Why no key was sent, when `outcome` is `CopiedOnly`.
    pub note: Option<&'static str>,
    /// The app that got the text: its `WM_CLASS` class or bundle id. Empty when none did.
    pub target: String,
    pub chord: Option<Chord>,
    pub selection: Option<Selection>,
    pub chord_sent_ms: Option<u64>,
    pub first_receipt_ms: Option<u64>,
    pub last_receipt_ms: Option<u64>,
    pub restore: Restore,
    pub restored_ms: Option<u64>,
}

impl Report {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            outcome: Outcome::Failed,
            code: None,
            note: None,
            target: target.into(),
            chord: None,
            selection: None,
            chord_sent_ms: None,
            first_receipt_ms: None,
            last_receipt_ms: None,
            restore: Restore::Pending,
            restored_ms: None,
        }
    }

    /// The `last_insert` section of `hookctl state`. `ready_unix_ms` is the wall clock time at
    /// which the text was ready; every other time is an offset from it.
    pub fn to_json(&self, session: u64, ready_unix_ms: u64) -> Value {
        json!({
            "session": session,
            "outcome": self.outcome.key(),
            "code": self.code,
            "note": self.note,
            "target": self.target,
            "chord": self.chord.map(Chord::key),
            "selection": self.selection.map(Selection::key),
            "ready_unix_ms": ready_unix_ms,
            "chord_sent_ms": self.chord_sent_ms,
            "first_receipt_ms": self.first_receipt_ms,
            "last_receipt_ms": self.last_receipt_ms,
            "restore": self.restore.key(),
            "restored_ms": self.restored_ms,
        })
    }
}
