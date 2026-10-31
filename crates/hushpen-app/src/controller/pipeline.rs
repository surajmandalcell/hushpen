//! The steps of one dictation: the checks before a start, the capture, the transcription,
//! the cleanup, and the history row. The run loop lives here too: the tick, the hook report of
//! a state change, and the microphone and engine watchers.

use super::runs::{Found, lock};
use super::{
    CUE_SOUNDS_SETTING, CUE_VOLUME_SETTING, CancelToken, Controller, LastAi, Notice, Phase, arm,
    command, config_from, failure_for, history,
};
use crate::ai_cleanup;
use crate::hook;
use crate::mic::{self, CaptureState};
use gpui_kit::Context;
use hushpen_core::dictation::{AppEvent, Cue, Mode, RowStatus, State};
use hushpen_core::error::{CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_NO_SPEECH, LLM_UNAVAILABLE};
use hushpen_core::llm_prompt::PromptFamily;
use hushpen_core::onboarding::KeyGate;
use hushpen_core::{cleanup, dictionary, language};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use hushpen_store::dictionary as store_dictionary;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CLEANUP_RULES_SETTING: &str = "cleanup.rules";

const SPOKEN_PUNCTUATION_SETTING: &str = "cleanup.spokenPunctuation";

const AI_ENABLED_SETTING: &str = "cleanup.llm.enabled";

const AI_PROVIDER_SETTING: &str = "cleanup.llm.provider";

const AI_MODEL_SETTING: &str = "cleanup.llm.modelId";

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

    /// Whether the AI source can edit by an instruction: the endpoint, or a local model that is
    /// not cleanup-only.
    pub(super) fn ai_edits(&self) -> bool {
        if self.ai_uses_endpoint() {
            return true;
        }
        let stored = self
            .storage
            .settings
            .get(AI_MODEL_SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        hushpen_core::catalog::embedded()
            .chosen_llm(&stored)
            .and_then(|entry| PromptFamily::from_model_name(&format!("{}.gguf", entry.id)))
            .is_none_or(PromptFamily::follows_instructions)
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
        let found = {
            let run = self.run_of(session);
            run.pending.raw = raw;
            run.pending.rule = rule.clone();
            Arc::clone(&run.found)
        };
        let store = |last: &LastAi, record: ai_cleanup::Record| {
            lock(&found).ai = Some(record.clone());
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
        self.run_of(session).ai_cancel = Some(Arc::clone(&token));
        let progress = Arc::clone(&self.ai_progress);
        let last = Arc::clone(&self.last_ai);
        let mine = Arc::clone(&found);
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
            lock(&mine).ai = Some(record.clone());
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
            self.run_of(session).ai_cancel = None;
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
        let settings = &self.storage.settings;
        let on = settings
            .get(CUE_SOUNDS_SETTING)
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let volume = settings
            .get(CUE_VOLUME_SETTING)
            .and_then(|value| value.as_f64())
            .map_or(0.25, |volume| volume as f32);
        if let Some(audio) = &self.dictation_audio {
            let mute = settings
                .get("audio.muteWhileDictating")
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            audio(Some(cue), on.then_some(volume), mute);
        }
        if on && let Some(cues) = &self.cues {
            cues(cue, volume);
        }
    }

    /// The checks before a start: a start with no usable model or engine is refused before the
    /// microphone opens. A start never waits for the session before it.
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
        let repeats = matches!(event, AppEvent::PasteLast | AppEvent::ReadAloud);
        let refused = match self.gate {
            KeyGate::Open => false,
            KeyGate::Closed => starts || command || repeats,
            KeyGate::Practice => command || repeats,
        };
        if refused {
            hook::record_event("onboarding", "key-blocked");
            return Err("onboarding is not finished: no dictation can start yet".into());
        }
        if command {
            return self.command_preflight(now, adopted, cx);
        }
        // While a session listens, these keys stop it or do nothing; they never start one.
        if !starts || self.machine.state() == State::Listening {
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
        session: u64,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.transcript.clear();
        self.language = None;
        self.notice = None;
        self.result = None;
        self.warned = false;
        self.runs.insert(session, super::runs::Run::default());
        match self.mic.update(cx, |mic, cx| {
            mic.start(crate::mic::CaptureUse::Dictation, cx)
        }) {
            Ok(path) => {
                self.run_of(session).wav = Some(path);
                Ok(())
            }
            Err(error) => {
                let message = self
                    .mic
                    .read(cx)
                    .notice()
                    .map_or_else(|| error.to_string(), |notice| notice.message.clone());
                self.run_of(session).detail = Some(message);
                self.start_failed = true;
                queue.push_back(AppEvent::CaptureError { code: error.code });
                Err(error.to_string())
            }
        }
    }

    /// Ends the recording of `session`. The microphone is free at once for the next start; the
    /// old WAV is finished on a worker thread, by the transcription when one follows.
    pub(super) fn stop_capture(
        &mut self,
        session: u64,
        keep: bool,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) {
        // Also restore on a discarded tap or a capture failure, neither needs a finish cue.
        if let Some(audio) = &self.dictation_audio {
            audio(None, None, false);
        }
        let detached = self.mic.update(cx, |mic, cx| mic.detach(keep, cx));
        let command = self.machine.session() == session && self.machine.mode() == Mode::Command;
        let selection = self.selection.take();
        let run = self.run_of(session);
        if !keep {
            run.wav = None;
            run.ended = true;
            if let Some(detached) = detached {
                let spawned = (self.spawn)(Box::new(move || {
                    let _ = detached.finish();
                }));
                if let Err(error) = spawned {
                    log::warn!("could not finish a discarded recording: {error}");
                }
            }
            return;
        }
        run.command = command;
        run.selection = selection;
        match detached {
            Some(detached) => run.stopping = Some(detached),
            None => {
                run.detail = Some("The recording ended before it could be transcribed.".into());
                queue.push_back(AppEvent::TranscribeFailed {
                    session,
                    code: CAPTURE_FAILED,
                });
            }
        }
    }

    pub(super) fn transcribe(&mut self, session: u64, model: &str, queue: &mut VecDeque<AppEvent>) {
        let language = self.language();
        let prompt = dictionary::prompt_for_model(model, &self.dictionary());
        let token = Arc::new(CancelToken::default());
        let run = self.run_of(session);
        let Some(wav) = run.wav.clone() else {
            return;
        };
        run.pending.language_requested = Some(language.clone());
        run.prompt = prompt.clone();
        run.cancel = Some(Arc::clone(&token));
        let stopping = run.stopping.take();
        let found = Arc::clone(&run.found);
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
        let engine = Arc::clone(&self.engine.run);
        let events = self.events.clone();
        let spawned = (self.spawn)(Box::new(move || {
            if let Some(stopping) = stopping
                && let Err(error) = finish_recording(stopping, &found)
            {
                let _ = events.unbounded_send((
                    AppEvent::TranscribeFailed {
                        session,
                        code: error,
                    },
                    None,
                ));
                return;
            }
            let event = match engine(spec, token) {
                JobOutcome::Done(done) => {
                    {
                        let mut found = lock(&found);
                        found.segments = history::segments_of(&done);
                        if done.language != "und" {
                            found.language = Some(done.language);
                        }
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
            self.run_of(session).detail =
                Some("Transcription could not start. The recording was kept.".into());
            queue.push_back(AppEvent::TranscribeFailed {
                session,
                code: ENGINE_CRASHED,
            });
        }
    }

    pub(super) fn save_row(
        &mut self,
        session: u64,
        status: RowStatus,
        text: Option<String>,
        code: Option<&'static str>,
    ) {
        let run = self.run_of(session);
        run.ended = true;
        // Only a session that failed while it listened can still hold its recording here.
        if let Some(stopping) = run.stopping.take() {
            let _ = finish_recording(stopping, &run.found);
        }
        if let Some(ms) = lock(&run.found).duration_ms {
            run.pending.duration_ms = ms;
        }
        let command = run.command;
        if self.gate == KeyGate::Practice {
            // A practice run is not the user's dictation: no row, and no audio kept.
            if let Some(wav) = self.run_of(session).wav.take() {
                let _ = std::fs::remove_file(wav);
            }
        } else {
            let saved = self.history_revision;
            self.record_history(session, status, &text, code);
            if status == RowStatus::Done
                && !command
                && self.history_revision != saved
                && let Some(row_id) = self.last_row.clone()
            {
                let ai_text = self.ai_text_of(session);
                self.offer_undo(session, row_id, ai_text.as_deref());
            }
        }
        match status {
            RowStatus::Done => {
                self.transcript = text.unwrap_or_default();
                let language = lock(&self.run_of(session).found).language.clone();
                self.language = language;
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

    pub(super) fn notify(&mut self, session: u64, code: &'static str) {
        let message = self
            .take_detail(session)
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
            let detail = self.mic.read(cx).notice().map_or_else(
                || "Recording stopped because of an error.".to_owned(),
                |notice| notice.message.clone(),
            );
            let session = self.machine.session();
            self.run_of(session).detail = Some(detail);
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

/// Finishes the WAV of a stopped recording and notes its length. A failure says why on the
/// notice and gives the code for the session.
fn finish_recording(
    stopping: crate::mic::Detached,
    found: &Mutex<Found>,
) -> Result<(), &'static str> {
    match stopping.finish() {
        Ok(finished) => {
            lock(found).duration_ms = Some(i64::try_from(finished.duration_ms).unwrap_or(0));
            Ok(())
        }
        Err(error) => {
            log::warn!("{error}");
            lock(found).detail = Some(mic::stop_message(&error));
            Err(error.code)
        }
    }
}
