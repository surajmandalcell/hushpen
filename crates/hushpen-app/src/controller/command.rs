//! Command Mode: the user holds the command keys with text selected, says what to do with it,
//! and the model's edit takes the selection's place. The selection lives in memory for the one
//! session; no log line, hook event, or history column gets it.

use super::{Controller, InsertSupport, Queued, failure_for};
use crate::ai_cleanup;
use crate::controller::CancelToken;
use crate::hook;
use gpui_kit::Context;
use hushpen_core::cleanup::outcome::Outcome;
use hushpen_core::dictation::{AppEvent, Mode, State};
use hushpen_core::error::{
    COMMAND_NEEDS_AI, COMMAND_NO_SELECTION, ENDPOINT_FAILED, LLM_CRASHED, LLM_GENERATE_FAILED,
    LLM_GUARD_REJECTED, LLM_LOAD_FAILED, LLM_NO_MODEL, LLM_PROMPT_TOO_LONG, LLM_PROTOCOL,
    LLM_TIMEOUT, LLM_UNAVAILABLE,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// A short notice for a Command Mode start that was refused. The flow bar shows each one once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flash {
    pub seq: u64,
    pub code: &'static str,
    pub message: String,
}

/// The code a failed edit shows: the failure of the worker or the endpoint, or a code of its
/// own for a timeout and a rejected answer.
pub(super) fn edit_failure_code(code: &str) -> &'static str {
    [
        LLM_TIMEOUT,
        LLM_GUARD_REJECTED,
        LLM_UNAVAILABLE,
        LLM_LOAD_FAILED,
        LLM_NO_MODEL,
        LLM_PROTOCOL,
        LLM_PROMPT_TOO_LONG,
        LLM_GENERATE_FAILED,
        ENDPOINT_FAILED,
    ]
    .into_iter()
    .find(|known| *known == code)
    .unwrap_or(LLM_CRASHED)
}

impl Controller {
    /// The newest refused Command Mode start, for the flow bar.
    pub fn flash(&self) -> Option<&Flash> {
        self.flash.as_ref()
    }

    /// The checks of the command keys, before the machine hears them: AI cleanup must be on,
    /// and the focused app must have a selection. A refused start leaves no session and no
    /// history row; a hold that already began is cancelled.
    pub(super) fn command_preflight(
        &mut self,
        now: u64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let starting = match self.machine.state() {
            State::Idle => true,
            State::Listening if self.machine.mode() == Mode::Hold => false,
            _ => return Ok(()),
        };
        if !self.ai_wanted() {
            return self.refuse_command(COMMAND_NEEDS_AI, now, cx);
        }
        if starting && let Some(blocker) = self.blocker(cx) {
            self.refused = Some(blocker.code);
            hook::record_event("dictation", &format!("blocked {}", blocker.code));
            return Err(format!("{}: {}", blocker.code, blocker.message));
        }
        let selection = match &self.insert {
            InsertSupport::Ready(inserter) => inserter.selection(&inserter.target()),
            _ => None,
        }
        .filter(|text| !text.trim().is_empty());
        let Some(selection) = selection else {
            return self.refuse_command(COMMAND_NO_SELECTION, now, cx);
        };
        self.selection = Some(selection);
        self.refused = None;
        if starting {
            self.machine.set_config(super::config_from(&self.storage));
        }
        Ok(())
    }

    fn refuse_command(
        &mut self,
        code: &'static str,
        now: u64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let message = failure_for(code).1;
        self.refused = Some(code);
        self.selection = None;
        self.flash_seq += 1;
        self.flash = Some(Flash {
            seq: self.flash_seq,
            code,
            message: message.to_owned(),
        });
        hook::record_event("dictation", &format!("blocked {code}"));
        if self.machine.state() == State::Listening {
            let before = self.machine.state();
            let effects = self.machine.handle(AppEvent::Esc, now);
            let mut queue = VecDeque::new();
            for effect in effects {
                let _ = self.apply(effect, &mut queue, cx);
            }
            self.record(before);
        }
        Err(format!("{code}: {message}"))
    }

    /// Hands the instruction and the selection to the model on a worker thread. The machine
    /// hears `Cleaned` with the edited text, or `EditFailed`, and nothing at all when the user
    /// cancelled.
    pub(super) fn edit(
        &mut self,
        session: u64,
        instruction: String,
        queue: &mut VecDeque<AppEvent>,
    ) {
        self.pending.instruction = Some(instruction.clone());
        let Some(selection) = self.selection.clone() else {
            queue.push_back(AppEvent::EditFailed {
                session,
                code: COMMAND_NO_SELECTION,
            });
            return;
        };
        let cleaner = self.ai.as_ref().and_then(|source| source());
        let timeout = self.ai_timeout();
        let token = Arc::new(CancelToken::default());
        self.ai_cancel = Some(Arc::clone(&token));
        let progress = Arc::clone(&self.ai_progress);
        let last = Arc::clone(&self.last_ai);
        let events = self.events.clone();
        let spawned = (self.spawn)(Box::new(move || {
            let Some(record) = ai_cleanup::run_edit(
                cleaner.as_deref(),
                &instruction,
                &selection,
                timeout,
                &token,
                &progress,
            ) else {
                return;
            };
            log::info!(
                "AI_EDIT session={session} outcome={} code={} ms={}",
                record.outcome.key(),
                record.code,
                record.ms
            );
            hook::record_event(
                "cleanup",
                &format!(
                    "edit {} {} {}ms",
                    record.outcome.key(),
                    record.code,
                    record.ms
                ),
            );
            let event = match (record.outcome, record.text.clone()) {
                (Outcome::Used, Some(text)) => AppEvent::Cleaned { session, text },
                _ => AppEvent::EditFailed {
                    session,
                    code: edit_failure_code(&record.code),
                },
            };
            {
                let mut slot = last.lock().unwrap_or_else(|p| p.into_inner());
                if slot.as_ref().is_none_or(|(owner, _)| *owner <= session) {
                    *slot = Some((session, record));
                }
            }
            let queued: Queued = (event, None);
            let _ = events.unbounded_send(queued);
        }));
        if let Err(error) = spawned {
            log::warn!("{LLM_UNAVAILABLE} could not start the AI edit: {error}");
            self.ai_cancel = None;
            queue.push_back(AppEvent::EditFailed {
                session,
                code: LLM_UNAVAILABLE,
            });
        }
    }

    /// Whether the running or the last session was a Command Mode one, for the history row.
    pub(super) fn is_command(&self) -> bool {
        self.machine.mode() == Mode::Command
    }
}
