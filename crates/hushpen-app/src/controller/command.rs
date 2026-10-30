//! Command Mode: the user holds the command keys with text selected, says what to do with it,
//! and the model's edit takes the selection's place. The selection lives in memory for the one
//! session; no log line, hook event, or history column gets it.

use super::arm::Armed;
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
use std::time::Duration;

/// How long a Command Mode start waits for the selection read before it counts as no
/// selection.
pub(super) const SELECTION_WAIT: Duration = Duration::from_secs(2);

/// Whether preflight lets an event reach the machine now.
pub(super) enum Pass {
    Go,
    /// Not yet: the machine hears it once the selection is read.
    Later,
}

/// A Command Mode start that waits for the selection read on a worker thread.
pub(super) struct Selecting {
    seq: u64,
    /// When the command keys went down, on the controller's clock.
    at: u64,
    /// The hold session that becomes the command, or `None` for a start from idle.
    hold: Option<u64>,
    /// The open microphone that the command takes over, so no speech is lost while it waits.
    armed: Option<Armed>,
    /// Key releases that came while it waited, with their times.
    held: Vec<(AppEvent, u64)>,
}

/// The selection a worker thread read, for the start that the number names. It stays in
/// memory like the selection itself.
pub(super) type SelectionRead = (u64, Option<String>);

/// A short notice for a refused Command Mode start or read-aloud shortcut. The flow bar shows
/// each one once.
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
    /// The newest refused Command Mode start or read-aloud shortcut, for the flow bar.
    pub fn flash(&self) -> Option<&Flash> {
        self.flash.as_ref()
    }

    /// The checks of the command keys, before the machine hears them: AI cleanup must be on,
    /// and the focused app must have a selection. A refused start leaves no session and no
    /// history row; a hold that already began is cancelled.
    ///
    /// The selection is read on a worker thread, because on macOS and X11 the read can wait
    /// for a copy. The machine hears the keys only once [`Self::selection_read`] has the
    /// answer, so this returns [`Pass::Later`] for a read that started.
    pub(super) fn command_preflight(
        &mut self,
        now: u64,
        adopted: &mut Option<Armed>,
        cx: &mut Context<Self>,
    ) -> Result<Pass, String> {
        let starting = match (self.machine.state(), self.machine.mode()) {
            (State::Listening, Mode::Hold) => false,
            (State::Listening, _) => return Ok(Pass::Go),
            _ => true,
        };
        if let Some(selection) = self.selection_ready.take() {
            self.selection = Some(selection);
            self.refused = None;
            if starting {
                self.machine.set_config(super::config_from(&self.storage));
            }
            return Ok(Pass::Go);
        }
        if self.selecting.is_some() {
            return Ok(Pass::Later);
        }
        if !self.ai_wanted() {
            return Err(self.refuse_command(COMMAND_NEEDS_AI, now, cx));
        }
        if starting && let Some(blocker) = self.blocker(cx) {
            self.refused = Some(blocker.code);
            hook::record_event("dictation", &format!("blocked {}", blocker.code));
            return Err(format!("{}: {}", blocker.code, blocker.message));
        }
        let InsertSupport::Ready(inserter) = &self.insert else {
            return Err(self.refuse_command(COMMAND_NO_SELECTION, now, cx));
        };
        let inserter = Arc::clone(inserter);
        let target = inserter.target();
        self.selection_seq += 1;
        let seq = self.selection_seq;
        let answers = self.selections.clone();
        let spawned = (self.spawn)(Box::new(move || {
            let text = inserter.selection(&target);
            let _ = answers.unbounded_send((seq, text));
        }));
        if let Err(error) = spawned {
            log::warn!("{COMMAND_NO_SELECTION} could not start the selection read: {error}");
            return Err(self.refuse_command(COMMAND_NO_SELECTION, now, cx));
        }
        // A read that hangs or panics sends nothing, and the key releases would wait for Esc.
        let deadline = self.selections.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(SELECTION_WAIT).await;
            let _ = deadline.unbounded_send((seq, None));
        })
        .detach();
        let armed = match adopted.take() {
            Some(armed) => Some(armed),
            None if starting => Some(self.open_early(now, cx)),
            None => None,
        };
        self.selecting = Some(Selecting {
            seq,
            at: now,
            hold: (!starting).then(|| self.machine.session()),
            armed,
            held: Vec::new(),
        });
        Ok(Pass::Later)
    }

    /// Keeps the key releases that come while the selection is read, so they reach the
    /// machine after the command started, and drops the waiting start on Esc. Returns
    /// whether the event was kept back.
    pub(super) fn while_selecting(
        &mut self,
        event: &AppEvent,
        now: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(selecting) = &mut self.selecting else {
            return false;
        };
        match event {
            AppEvent::CommandUp | AppEvent::HoldUp => {
                selecting.held.push((event.clone(), now));
                true
            }
            AppEvent::Esc => {
                if let Some(armed) = self.selecting.take().and_then(|selecting| selecting.armed) {
                    self.release_unused_capture(armed, cx);
                }
                false
            }
            _ => false,
        }
    }

    /// The answer of the selection read, or `None` once [`SELECTION_WAIT`] passed. The waiting
    /// command starts, dated at its key press; with no selection it is refused. The key
    /// releases kept since then follow either way. An answer for a start that was cancelled or
    /// already answered changes nothing.
    pub(super) fn selection_read(&mut self, (seq, text): SelectionRead, cx: &mut Context<Self>) {
        if self
            .selecting
            .as_ref()
            .is_none_or(|selecting| selecting.seq != seq)
        {
            return;
        }
        let Some(selecting) = self.selecting.take() else {
            return;
        };
        let still = self.armed.is_none()
            && match selecting.hold {
                None => self.machine.state() != State::Listening,
                Some(session) => {
                    self.machine.state() == State::Listening
                        && self.machine.mode() == Mode::Hold
                        && self.machine.session() == session
                }
            };
        match text.filter(|text| !text.trim().is_empty()) {
            Some(text) if still => {
                self.selection_ready = Some(text);
                self.armed = selecting.armed;
                let _ = self.dispatch_at(AppEvent::CommandDown, Some(selecting.at), cx);
                self.selection_ready = None;
                for (event, at) in selecting.held {
                    let _ = self.dispatch_at(event, Some(at), cx);
                }
            }
            _ => {
                if still {
                    let now = (self.clock)();
                    self.refuse_command(COMMAND_NO_SELECTION, now, cx);
                }
                if let Some(armed) = selecting.armed {
                    self.release_unused_capture(armed, cx);
                }
                // A hold that began during the read still ends on its own key release.
                for (event, at) in selecting.held {
                    let _ = self.dispatch_at(event, Some(at), cx);
                }
                cx.notify();
            }
        }
    }

    /// Shows a short notice on the flow bar once.
    pub(super) fn show_flash(&mut self, code: &'static str, message: String) {
        self.flash_seq += 1;
        self.flash = Some(Flash {
            seq: self.flash_seq,
            code,
            message,
        });
    }

    /// Records a refused start, cancels a hold that already began, and returns the reason.
    fn refuse_command(&mut self, code: &'static str, now: u64, cx: &mut Context<Self>) -> String {
        let message = failure_for(code).1;
        self.refused = Some(code);
        self.selection = None;
        self.show_flash(code, message.to_owned());
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
        format!("{code}: {message}")
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
        let run = self.run_of(session);
        run.pending.instruction = Some(instruction.clone());
        let found = Arc::clone(&run.found);
        let Some(selection) = run.selection.clone() else {
            queue.push_back(AppEvent::EditFailed {
                session,
                code: COMMAND_NO_SELECTION,
            });
            return;
        };
        let cleaner = self.ai.as_ref().and_then(|source| source());
        let timeout = self.ai_timeout();
        let token = Arc::new(CancelToken::default());
        self.run_of(session).ai_cancel = Some(Arc::clone(&token));
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
            super::runs::lock(&found).ai = Some(record.clone());
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
            self.run_of(session).ai_cancel = None;
            queue.push_back(AppEvent::EditFailed {
                session,
                code: LLM_UNAVAILABLE,
            });
        }
    }

    /// Whether the newest session was a Command Mode one, for the views.
    pub fn ran_command(&self) -> bool {
        self.machine.mode() == Mode::Command
    }
}
