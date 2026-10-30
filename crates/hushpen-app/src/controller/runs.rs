//! What the controller keeps for each session until its row is saved. Sessions overlap: a new
//! one listens while older ones still transcribe, clean, or insert, so each keeps its own audio,
//! cancel tokens, and what its steps learned.

use super::{CancelToken, Controller, history};
use crate::ai_cleanup;
use crate::mic::Detached;
use hushpen_store::history::Segment;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Run {
    /// The session audio: the live file while listening, the finished file after.
    pub wav: Option<PathBuf>,
    /// The microphone session that still has to finish the WAV. The transcription does that
    /// on its worker thread before it reads the file.
    pub stopping: Option<Detached>,
    pub cancel: Option<Arc<CancelToken>>,
    pub ai_cancel: Option<Arc<CancelToken>>,
    /// What the history row needs from the steps before the save.
    pub pending: history::Pending,
    /// Text for the notice of this session, when the failing part knows more than the code.
    pub detail: Option<String>,
    /// The text Command Mode edits, from the moment the recording stops.
    pub selection: Option<String>,
    /// The whisper prompt the transcription used.
    pub prompt: Option<String>,
    /// The session ran in Command Mode.
    pub command: bool,
    /// What the worker threads of this session learned.
    pub found: Arc<Mutex<Found>>,
    /// The row is saved, or the session left none. The entry goes once no newer session needs
    /// it on screen.
    pub ended: bool,
}

/// Written by the worker threads of one session.
#[derive(Default)]
pub(super) struct Found {
    pub duration_ms: Option<i64>,
    /// The language the engine detected.
    pub language: Option<String>,
    /// The timed pieces of the words.
    pub segments: Vec<Segment>,
    pub ai: Option<ai_cleanup::Record>,
    /// Why the recording or the transcription failed, for the notice.
    pub detail: Option<String>,
}

pub(super) fn lock(found: &Mutex<Found>) -> std::sync::MutexGuard<'_, Found> {
    found
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Controller {
    pub(super) fn run_of(&mut self, session: u64) -> &mut Run {
        self.runs.entry(session).or_default()
    }

    /// Drops the entries of ended sessions, except the newest one, whose audio and words are
    /// still on screen.
    pub(super) fn prune_runs(&mut self) {
        let front = self.machine.session();
        self.runs
            .retain(|session, run| !run.ended || *session == front);
    }

    /// The message for the notice of `session`: what its steps said, or the words for the code.
    pub(super) fn take_detail(&mut self, session: u64) -> Option<String> {
        let run = self.runs.get_mut(&session)?;
        run.detail.take().or_else(|| lock(&run.found).detail.take())
    }
}
