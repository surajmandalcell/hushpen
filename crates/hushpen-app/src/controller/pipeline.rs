//! The steps of one dictation: the checks before a start, the capture, the transcription,
//! the cleanup, and the history row. The run loop lives here too: the tick, the hook report of
//! a state change, and the microphone and engine watchers.

use super::{
    CUE_SOUNDS_SETTING, CUE_VOLUME_SETTING, CancelToken, Controller, LastAi, Notice, Phase, arm,
    command, config_from, failure_for, history,
};
use crate::ai_cleanup;
use crate::hook;
use crate::mic::CaptureState;
use gpui_kit::Context;
use hushpen_core::dictation::{AppEvent, Cue, RowStatus, State};
use hushpen_core::error::{CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_NO_SPEECH, LLM_UNAVAILABLE};
use hushpen_core::onboarding::KeyGate;
use hushpen_core::{cleanup, dictionary, language};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use hushpen_store::dictionary as store_dictionary;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

/// A Home start never waits for a result flash. This is added to the clock to end one.
const FLASH_END_MS: u64 = 60_000;

const CLEANUP_RULES_SETTING: &str = "cleanup.rules";

const SPOKEN_PUNCTUATION_SETTING: &str = "cleanup.spokenPunctuation";

const AI_ENABLED_SETTING: &str = "cleanup.llm.enabled";

const AI_PROVIDER_SETTING: &str = "cleanup.llm.provider";

const AI_TIMEOUT_SETTING: &str = "cleanup.llm.timeoutMs";

const AI_ENDPOINT_TIMEOUT_SETTING: &str = "cleanup.llm.endpointTimeoutMs";

impl Controller {
    /// Whether the settings ask for AI cleanup. The cleaner source picks the provider.
    pub(super) fn ai_wanted(&self) -> bool {
        self.storage
            .settings
            .get(AI_ENABLED_SETTING)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    pub(super) fn ai_uses_endpoint(&self) -> bool {
        self.storage
            .settings
            .get(AI_PROVIDER_SETTING)
            .is_some_and(|provider| provider.as_str() == Some("endpoint"))
    }

    /// How long the model step may take: the endpoint has its own limit, because a network
    /// round trip is not a local generation.
    pub(super) fn ai_timeout(&self) -> Duration {
        let key = if self.ai_uses_endpoint() {
            AI_ENDPOINT_TIMEOUT_SETTING
        } else {
            AI_TIMEOUT_SETTING
        };
        self.storage.settings.millis(key)
    }

    /// The rule text, then the model's rewrite when AI cleanup is on. Every way the model step
    /// can fail leaves the rule text as the answer; the pipeline hears `Cleaned` either way.
    pub(super) fn clean(&mut self, session: u64, raw: String, queue: &mut VecDeque<AppEvent>) {
        let rule = self.rule_cleanup(&raw);
        self.pending.raw = raw;
        self.pending.rule = rule.clone();
        let store = |last: &LastAi, record: ai_cleanup::Record| {
            *last.lock().unwrap_or_else(|p| p.into_inner()) = Some((session, record));
        };
        if !self.ai_wanted() || rule.trim().is_empty() {
            store(&self.last_ai, ai_cleanup::Record::off());
            queue.push_back(AppEvent::Cleaned {
                session,
                text: rule,
            });
            return;
        }
        let cleaner = self.ai.as_ref().and_then(|source| source());
        let timeout = self.ai_timeout();
        let token = Arc::new(CancelToken::default());
        self.ai_cancel = Some(Arc::clone(&token));
        let progress = Arc::clone(&self.ai_progress);
        let last = Arc::clone(&self.last_ai);
        let events = self.events.clone();
        let for_thread = rule.clone();
        let keep = dictionary::phrases_in(&rule, &self.dictionary());
        let spawned = (self.spawn)(Box::new(move || {
            let rule = for_thread;
            let Some(record) =
                ai_cleanup::run(cleaner.as_deref(), &rule, &keep, timeout, &token, &progress)
            else {
                return;
            };
            let text = record.text.clone().unwrap_or_else(|| rule.clone());
            log::info!(
                "AI_CLEANUP session={session} outcome={} code={} ms={}",
                record.outcome.key(),
                record.code,
                record.ms
            );
            hook::record_event(
                "cleanup",
                &format!(
                    "ai {} {} {}ms",
                    record.outcome.key(),
                    record.code,
                    record.ms
                ),
            );
            {
                // A step that finished after its session ended must not hide a newer one.
                let mut slot = last.lock().unwrap_or_else(|p| p.into_inner());
                if slot.as_ref().is_none_or(|(owner, _)| *owner <= session) {
                    *slot = Some((session, record));
                }
            }
            let _ = events.unbounded_send((AppEvent::Cleaned { session, text }, None));
        }));
        if let Err(error) = spawned {
            log::warn!("{LLM_UNAVAILABLE} could not start the AI cleanup: {error}");
            self.ai_cancel = None;
            store(
                &self.last_ai,
                ai_cleanup::Record {
                    outcome: hushpen_core::cleanup::outcome::Outcome::Unavailable,
                    code: LLM_UNAVAILABLE.to_owned(),
                    text: None,
                    ms: 0,
                },
            );
            queue.push_back(AppEvent::Cleaned {
                session,
                text: rule,
            });
        }
    }

    /// What the AI step is doing now, for Home and the hook. A dictation that runs while the
    /// local model loads reads `Preparing` too, although its AI step does not wait for the load.
    pub fn ai_phase(&self) -> Option<ai_cleanup::Phase> {
        self.ai_progress.get().or_else(|| {
            let busy = matches!(self.machine.state(), State::Transcribing | State::Cleaning);
            let loading = self.ai_loading.as_ref().is_some_and(|loading| loading());
            (busy && loading && self.ai_wanted() && !self.ai_uses_endpoint())
                .then_some(ai_cleanup::Phase::Preparing)
        })
    }

    #[cfg(test)]
    pub(crate) fn show_ai_phase(&self, phase: Option<ai_cleanup::Phase>) {
        self.ai_progress.set(phase);
    }

    /// `hookctl state` `pipeline.ai`: the running phase and the newest step's outcome.
    pub(super) fn ai_json(&self) -> Value {
        let last = self.last_ai.lock().unwrap_or_else(|p| p.into_inner());
        json!({
            "phase": self.ai_phase().map(ai_cleanup::Phase::key),
            "last": last.as_ref().map(|(session, record)| json!({
                "session": session,
                "outcome": record.outcome.key(),
                "code": record.code,
                "ms": record.ms,
            })),
        })
    }

    /// The rule cleanup, unless `cleanup.rules` is off, and then the dictionary replacements,
    /// which run either way. The settings and the dictionary are read each time, so a change
    /// takes effect on the next dictation.
    pub(super) fn rule_cleanup(&self, raw: &str) -> String {
        let flag = |key: &str| {
            self.storage
                .settings
                .get(key)
                .and_then(|value| value.as_bool())
                .unwrap_or(true)
        };
        let cleaned = if flag(CLEANUP_RULES_SETTING) {
            let options = cleanup::Options {
                spoken_punctuation: flag(SPOKEN_PUNCTUATION_SETTING),
            };
            cleanup::clean(raw, &options)
        } else {
            raw.to_owned()
        };
        dictionary::apply(&cleaned, &self.dictionary())
    }

    /// The personal dictionary. A database that cannot be read counts as an empty one: a
    /// dictation must not fail because of it.
    pub(super) fn dictionary(&self) -> Vec<dictionary::Entry> {
        store_dictionary::list(&self.storage.database).unwrap_or_else(|error| {
            log::warn!("the dictionary could not be read: {error}");
            Vec::new()
        })
    }

    /// Plays the cue unless the user turned the sounds off. The settings are read each time, so
    /// a change takes effect on the next cue.
    pub(super) fn play_cue(&self, cue: Cue) {
        let Some(cues) = &self.cues else {
            return;
        };
        let settings = &self.storage.settings;
        let on = settings
            .get(CUE_SOUNDS_SETTING)
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if !on {
            return;
        }
        let volume = settings
            .get(CUE_VOLUME_SETTING)
            .and_then(|value| value.as_f64())
            .map_or(0.5, |volume| volume as f32);
        cues(cue, volume);
    }

    /// The checks before a start: Home never waits for a result flash, and a start with no
    /// usable model or engine is refused before the microphone opens.
    pub(super) fn preflight(
        &mut self,
        event: &AppEvent,
        now: u64,
        adopted: &mut Option<arm::Armed>,
        cx: &mut Context<Self>,
    ) -> Result<command::Pass, String> {
        let starts = matches!(
            event,
            AppEvent::HoldDown
                | AppEvent::HandsFreeToggle
                | AppEvent::HomeToggle
                | AppEvent::FlowBarClick
        );
        let command = matches!(event, AppEvent::CommandDown);
        let refused = match self.gate {
            KeyGate::Open => false,
            KeyGate::Closed => starts || command || matches!(event, AppEvent::PasteLast),
            KeyGate::Practice => command || matches!(event, AppEvent::PasteLast),
        };
        if refused {
            hook::record_event("onboarding", "key-blocked");
            return Err("onboarding is not finished: no dictation can start yet".into());
        }
        if command {
            return self.command_preflight(now, adopted, cx);
        }
        if !starts {
            return Ok(command::Pass::Go);
        }
        if matches!(event, AppEvent::HomeToggle)
            && matches!(
                self.machine.state(),
                State::Done | State::Cancelled | State::Failed
            )
        {
            let before = self.machine.state();
            self.machine
                .handle(AppEvent::Tick, now.saturating_add(FLASH_END_MS));
            self.record(before);
        }
        if self.machine.state() != State::Idle {
            return Ok(command::Pass::Go);
        }
        if let Some(blocker) = self.blocker(cx) {
            self.refused = Some(blocker.code);
            hook::record_event("dictation", &format!("blocked {}", blocker.code));
            return Err(format!("{}: {}", blocker.code, blocker.message));
        }
        self.refused = None;
        self.selection = None;
        self.machine.set_config(config_from(&self.storage));
        Ok(command::Pass::Go)
    }

    pub(super) fn start_capture(
        &mut self,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.transcript.clear();
        self.language = None;
        self.notice = None;
        self.result = None;
        self.warned = false;
        self.wav = None;
        self.pending = history::Pending::default();
        *self.detected.lock().unwrap_or_else(|p| p.into_inner()) = None;
        *self.segments.lock().unwrap_or_else(|p| p.into_inner()) = None;
        match self.mic.update(cx, |mic, cx| {
            mic.start(crate::mic::CaptureUse::Dictation, cx)
        }) {
            Ok(path) => {
                self.wav = Some(path);
                Ok(())
            }
            Err(error) => {
                let message = self
                    .mic
                    .read(cx)
                    .notice()
                    .map_or_else(|| error.to_string(), |notice| notice.message.clone());
                self.detail = Some(message);
                self.start_failed = true;
                queue.push_back(AppEvent::CaptureError { code: error.code });
                Err(error.to_string())
            }
        }
    }

    pub(super) fn stop_capture(
        &mut self,
        keep: bool,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) {
        let session = self.machine.session();
        let stopped = self.mic.update(cx, |mic, cx| mic.stop(keep, cx));
        if !keep {
            self.wav = None;
            return;
        }
        match stopped {
            Ok(Some(finished)) => {
                self.pending.duration_ms = i64::try_from(finished.duration_ms).unwrap_or(0);
                self.wav = Some(finished.path);
            }
            Ok(None) => {
                self.detail = Some("The recording ended before it could be transcribed.".into());
                queue.push_back(AppEvent::TranscribeFailed {
                    session,
                    code: CAPTURE_FAILED,
                });
            }
            Err(error) => {
                self.detail = Some(
                    self.mic
                        .read(cx)
                        .notice()
                        .map_or_else(|| error.to_string(), |notice| notice.message.clone()),
                );
                queue.push_back(AppEvent::TranscribeFailed {
                    session,
                    code: error.code,
                });
            }
        }
    }

    pub(super) fn transcribe(&mut self, session: u64, queue: &mut VecDeque<AppEvent>) {
        let Some(wav) = self.wav.clone() else {
            return;
        };
        let language = self.language();
        self.pending.language_requested = Some(language.clone());
        let prompt = Some(dictionary::build_prompt(&self.dictionary())).filter(|p| !p.is_empty());
        hook::record_event(
            "transcribe",
            &format!("prompt {}", prompt.as_deref().unwrap_or("")),
        );
        self.last_prompt = prompt.clone();
        let spec = TranscribeSpec {
            wav_path: wav,
            language: (language != language::AUTO).then_some(language),
            prompt,
        };
        let token = Arc::new(CancelToken::default());
        self.cancel = Some(Arc::clone(&token));
        let run = Arc::clone(&self.engine.run);
        let events = self.events.clone();
        let detected = Arc::clone(&self.detected);
        let segments = Arc::clone(&self.segments);
        let spawned = (self.spawn)(Box::new(move || {
            let event = match run(spec, token) {
                JobOutcome::Done(done) => {
                    *segments.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some((session, history::segments_of(&done)));
                    if done.language != "und" {
                        *detected.lock().unwrap_or_else(|p| p.into_inner()) =
                            Some((session, done.language));
                    }
                    AppEvent::Transcribed {
                        session,
                        text: done.text,
                    }
                }
                JobOutcome::Failed(failure) if failure.code == ENGINE_NO_SPEECH => {
                    AppEvent::Transcribed {
                        session,
                        text: String::new(),
                    }
                }
                JobOutcome::Failed(failure) => {
                    log::warn!(
                        "{} dictation job for session {session} failed",
                        failure.code
                    );
                    AppEvent::TranscribeFailed {
                        session,
                        code: failure_for(&failure.code).0,
                    }
                }
                JobOutcome::Cancelled => return,
            };
            let _ = events.unbounded_send((event, None));
        }));
        if let Err(error) = spawned {
            log::warn!("{ENGINE_CRASHED} could not start the transcription: {error}");
            self.detail = Some("Transcription could not start. The recording was kept.".into());
            queue.push_back(AppEvent::TranscribeFailed {
                session,
                code: ENGINE_CRASHED,
            });
        }
    }

    pub(super) fn save_row(
        &mut self,
        status: RowStatus,
        text: Option<String>,
        code: Option<&'static str>,
    ) {
        if self.gate == KeyGate::Practice {
            // A practice run is not the user's dictation: no row, and no audio kept.
            if let Some(wav) = self.wav.take() {
                let _ = std::fs::remove_file(wav);
            }
        } else {
            let saved = self.history_revision;
            self.record_history(status, &text, code);
            if status == RowStatus::Done
                && !self.is_command()
                && self.history_revision != saved
                && let Some(row_id) = self.last_row.clone()
            {
                let ai_text = self.ai_text_of(self.machine.session());
                self.offer_undo(self.machine.session(), row_id, ai_text.as_deref());
            }
        }
        match status {
            RowStatus::Done => {
                self.transcript = text.unwrap_or_default();
                let session = self.machine.session();
                let detected = self
                    .detected
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .take();
                self.language = detected
                    .filter(|(owner, _)| *owner == session)
                    .map(|(_, code)| code);
                self.result = Some(Phase::Done);
                self.last_result = Some("done");
                self.copy_notice(session);
                log::info!(
                    "DICTATION_DONE session={session} chars={}",
                    self.transcript.chars().count()
                );
            }
            RowStatus::Cancelled | RowStatus::Failed => {}
        }
    }

    pub(super) fn notify(&mut self, code: &'static str) {
        let message = self
            .detail
            .take()
            .unwrap_or_else(|| failure_for(code).1.to_owned());
        self.notice = Some(Notice { code, message });
        let result = if code == ENGINE_NO_SPEECH {
            Phase::NoSpeech
        } else {
            Phase::Failed
        };
        self.result = Some(result);
        self.last_result = Some(result.key());
    }

    pub(super) fn tick(&mut self, cx: &mut Context<Self>) {
        // The worker thread moves the AI phase without telling the views.
        let ai = self.ai_phase();
        if ai != self.shown_ai {
            self.shown_ai = ai;
            cx.notify();
        }
        self.watch_undo(cx);
        if self.machine.state() != State::Idle {
            let _ = self.dispatch(AppEvent::Tick, cx);
        }
    }

    /// Hook events for the state change, and the Esc grab that follows the session.
    pub(super) fn record(&mut self, before: State) {
        let state = self.machine.state();
        if state != before {
            hook::record_event("pipeline", state.key());
        }
        let phase = self.phase();
        if phase != self.last_phase {
            self.last_phase = phase;
            let detail = match (phase, &self.notice) {
                (Phase::Failed, Some(notice)) => format!("failed {}", notice.code),
                _ => phase.key().to_owned(),
            };
            hook::record_event("dictation", &detail);
        }
        if matches!(
            state,
            State::Idle | State::Done | State::Failed | State::Cancelled
        ) {
            self.selection = None;
        }
        let take_escape = state.takes_escape();
        if take_escape != self.esc_taken {
            self.esc_taken = take_escape;
            if let Some(session_active) = &self.session_active {
                session_active(take_escape);
            }
        }
    }

    /// The microphone failed under a running dictation. The audio so far stays on disk.
    pub(super) fn mic_changed(&mut self, cx: &mut Context<Self>) {
        if self.machine.state() == State::Listening
            && self.mic.read(cx).state() != CaptureState::Listening
        {
            self.detail = Some(self.mic.read(cx).notice().map_or_else(
                || "Recording stopped because of an error.".to_owned(),
                |notice| notice.message.clone(),
            ));
            let _ = self.dispatch(
                AppEvent::CaptureError {
                    code: CAPTURE_FAILED,
                },
                cx,
            );
        }
        cx.notify();
    }

    pub(crate) fn poll_problem(&mut self, cx: &mut Context<Self>) {
        let current = (self.engine.problem)();
        if current != self.problem {
            self.problem = current;
            cx.notify();
        }
    }
}
