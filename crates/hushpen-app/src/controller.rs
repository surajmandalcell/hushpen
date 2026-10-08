//! The dictation controller: one pipeline for the hold key, Home, the tray, and the flow bar.
//!
//! Every surface turns its input into an [`AppEvent`] and calls [`Controller::dispatch`], or
//! sends it down [`Controller::sender`] from another thread. The controller feeds the event to
//! the pure [`DictationMachine`] and runs the [`Effect`]s that come back: it starts and stops the
//! microphone, runs the transcription on a worker thread, and copies the words. Slow work
//! reports back as events that carry the session id, so a late answer from a cancelled run
//! changes nothing. Transcript text is never logged.

use crate::engine_host::{EngineHost, LoadProblem};
use crate::hook;
use crate::mic::{CaptureState, Mic};
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{ClipboardItem, Context, Entity};
use hushpen_core::dictation::{
    AppEvent, Config, Cue, DictationMachine, Effect, Mode, RowStatus, State,
};
use hushpen_core::error::{
    self, CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_LOAD_FAILED, ENGINE_NO_MODEL, ENGINE_NO_SPEECH,
};
use hushpen_core::language;
use hushpen_engine::{JobOutcome, TranscribeSpec};
use hushpen_platform::keys::Unavailable;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;

pub const LANGUAGE_SETTING: &str = "dictation.language";
const MAX_MINUTES_SETTING: &str = "dictation.maxMinutes";

/// How often the machine is told that time passed while a session or a result flash runs.
const TICK: Duration = Duration::from_millis(100);
/// How often the idle view looks for a failed model load, which a worker thread reports.
const PROBLEM_POLL: Duration = Duration::from_secs(1);
/// A Home start never waits for a result flash. This is added to the clock to end one.
const FLASH_END_MS: u64 = 60_000;

/// Milliseconds on a monotonic scale. Tests pass a clock they move by hand.
pub type Clock = Rc<dyn Fn() -> u64>;

/// Milliseconds since the first call in this process, readable from any thread.
pub fn monotonic_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub fn monotonic_clock() -> Clock {
    Rc::new(monotonic_ms)
}

/// Feeds the pipeline from another thread, such as the key listener. A key event is stamped
/// when it arrives: the controller may be busy (starting the microphone takes a while) when it
/// gets to the event, and a tap must be measured by when the key moved, not when it was read.
#[derive(Clone)]
pub struct EventSender(UnboundedSender<Queued>);

impl EventSender {
    pub fn send(&self, event: AppEvent) {
        let _ = self.0.unbounded_send((event, Some(monotonic_ms())));
    }
}

/// An event, and when it happened if the sender knew. Without a time the controller's clock at
/// the moment it handles the event is used.
type Queued = (AppEvent, Option<u64>);

/// Stops a running transcription. The worker thread registers what to call; a cancel that came
/// first calls it at once.
#[derive(Default)]
pub struct CancelToken {
    cancelled: AtomicBool,
    on_cancel: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let hook = self
            .on_cancel
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(hook) = hook {
            hook();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn on_cancel(&self, hook: impl FnOnce() + Send + 'static) {
        let mut slot = self
            .on_cancel
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.is_cancelled() {
            drop(slot);
            hook();
        } else {
            *slot = Some(Box::new(hook));
        }
    }
}

/// Runs one transcription and waits for it. Tests pass a fake.
pub type Runner = Arc<dyn Fn(TranscribeSpec, Arc<CancelToken>) -> JobOutcome + Send + Sync>;

/// The speech engine as the pipeline sees it.
pub struct Engine {
    run: Runner,
    problem: Rc<dyn Fn() -> Option<LoadProblem>>,
}

impl Engine {
    pub fn new(run: Runner, problem: Rc<dyn Fn() -> Option<LoadProblem>>) -> Self {
        Self { run, problem }
    }

    pub fn host(host: &Rc<EngineHost>) -> Self {
        let client = host.client().clone();
        let for_problem = Rc::clone(host);
        Self {
            run: Arc::new(move |spec, token| {
                let job = client.transcribe(spec);
                let id = job.id();
                let cancel = client.clone();
                token.on_cancel(move || cancel.cancel(id));
                job.wait()
            }),
            problem: Rc::new(move || for_problem.load_problem()),
        }
    }
}

/// What Home shows. The pipeline has more states; Home folds the steps after listening into
/// transcribing and keeps the last result on screen after the pipeline is idle again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Listening,
    Transcribing,
    Done,
    NoSpeech,
    Failed,
}

impl Phase {
    pub fn key(self) -> &'static str {
        match self {
            Phase::Idle => "idle",
            Phase::Listening => "listening",
            Phase::Transcribing => "transcribing",
            Phase::Done => "done",
            Phase::NoSpeech => "no-speech",
            Phase::Failed => "failed",
        }
    }

    pub fn busy(self) -> bool {
        matches!(self, Phase::Listening | Phase::Transcribing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: &'static str,
    pub message: String,
}

/// Why no dictation can start right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocker {
    pub code: &'static str,
    pub message: String,
    /// Models is where the user fixes it.
    pub models_link: bool,
}

/// Whether the global keys run, as `main` found out when it started them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeysStatus {
    Available,
    Unavailable(Unavailable),
}

pub struct Controller {
    storage: Rc<Storage>,
    mic: Entity<Mic>,
    models: Entity<Models>,
    engine: Engine,
    spawn: Spawner,
    clock: Clock,
    machine: DictationMachine,
    events: UnboundedSender<Queued>,
    /// The session audio: the live file while listening, the finished file after.
    wav: Option<PathBuf>,
    cancel: Option<Arc<CancelToken>>,
    /// The language the engine detected, with the session that it belongs to.
    detected: Arc<Mutex<Option<(u64, String)>>>,
    transcript: String,
    language: Option<String>,
    notice: Option<Notice>,
    /// Text for the next notice, when the failing part knows more than the code.
    detail: Option<String>,
    result: Option<Phase>,
    last_result: Option<&'static str>,
    refused: Option<&'static str>,
    last_text: Option<String>,
    warned: bool,
    start_failed: bool,
    problem: Option<LoadProblem>,
    keys: Option<KeysStatus>,
    session_active: Option<Rc<dyn Fn(bool)>>,
    esc_taken: bool,
    last_phase: Phase,
}

impl Controller {
    pub fn new(
        storage: Rc<Storage>,
        mic: Entity<Mic>,
        models: Entity<Models>,
        engine: Engine,
        spawn: Spawner,
        clock: Clock,
        cx: &mut Context<Self>,
    ) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some((event, at)) = queue.next().await {
                if this
                    .update(cx, |me, cx| {
                        // A refusal shows in the notice; nothing else to tell a worker.
                        let _ = me.dispatch_at(event, at, cx);
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |me, cx| me.tick(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PROBLEM_POLL).await;
                if this.update(cx, |me, cx| me.poll_problem(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.observe(&mic, |me, _, cx| me.mic_changed(cx)).detach();
        cx.observe(&models, |_, _, cx| cx.notify()).detach();
        let config = config_from(&storage);
        Self {
            storage,
            mic,
            models,
            engine,
            spawn,
            clock,
            machine: DictationMachine::new(config),
            events,
            wav: None,
            cancel: None,
            detected: Arc::default(),
            transcript: String::new(),
            language: None,
            notice: None,
            detail: None,
            result: None,
            last_result: None,
            refused: None,
            last_text: None,
            warned: false,
            start_failed: false,
            problem: None,
            keys: None,
            session_active: None,
            esc_taken: false,
            last_phase: Phase::Idle,
        }
    }

    /// For threads that feed the pipeline, such as the key listener.
    pub fn sender(&self) -> EventSender {
        EventSender(self.events.clone())
    }

    /// Records whether the global keys run, and how to tell them that a session started or
    /// ended (the Esc grab on X11).
    pub fn attach_keys(&mut self, status: KeysStatus, session_active: Rc<dyn Fn(bool)>) {
        self.keys = Some(status);
        self.session_active = Some(session_active);
    }

    pub fn state(&self) -> State {
        self.machine.state()
    }

    pub fn mode(&self) -> Mode {
        self.machine.mode()
    }

    pub fn phase(&self) -> Phase {
        match self.machine.state() {
            State::Listening => Phase::Listening,
            State::Transcribing | State::Cleaning | State::Inserting => Phase::Transcribing,
            State::Cancelled => Phase::Idle,
            State::Done | State::Failed | State::Idle => self.result.unwrap_or(Phase::Idle),
        }
    }

    pub fn transcript(&self) -> &str {
        &self.transcript
    }

    /// The language the engine detected for the shown words, as a whisper code.
    pub fn detected(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn session(&self) -> Option<&Path> {
        self.wav.as_deref()
    }

    pub fn last_result(&self) -> Option<&'static str> {
        self.last_result
    }

    /// The code of the last start that preflight refused, until a start is accepted.
    pub fn refused(&self) -> Option<&'static str> {
        self.refused
    }

    /// The `dictation.language` setting: `auto` or a whisper code.
    pub fn language(&self) -> String {
        self.storage
            .settings
            .get(LANGUAGE_SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| language::AUTO.to_owned())
    }

    /// Why no dictation can start, or `None` while one can.
    pub fn blocker(&self, cx: &gpui_kit::App) -> Option<Blocker> {
        if let Some(problem) = &self.problem {
            return Some(if problem.unsupported_cpu {
                Blocker {
                    code: ENGINE_LOAD_FAILED,
                    message: "This computer's processor is not supported. Hushpen needs a processor with AVX2, FMA, F16C, and BMI2."
                        .into(),
                    models_link: false,
                }
            } else {
                Blocker {
                    code: ENGINE_LOAD_FAILED,
                    message: "The speech model could not be loaded. Download it again in Models."
                        .into(),
                    models_link: true,
                }
            });
        }
        if self.models.read(cx).usable().is_some() {
            return None;
        }
        Some(Blocker {
            code: ENGINE_NO_MODEL,
            message: "A speech model is needed before you can dictate. Get one in Models.".into(),
            models_link: true,
        })
    }

    pub fn can_record(&self, cx: &gpui_kit::App) -> bool {
        match self.phase() {
            Phase::Listening => true,
            Phase::Transcribing => false,
            _ => self.blocker(cx).is_none(),
        }
    }

    /// Runs one event through the pipeline at the controller's clock. Returns the reason when a
    /// start was refused (preflight, or the microphone would not start).
    pub fn dispatch(&mut self, event: AppEvent, cx: &mut Context<Self>) -> Result<(), String> {
        self.dispatch_at(event, None, cx)
    }

    /// Like [`Controller::dispatch`], for an event that happened at `at` on the controller's
    /// clock.
    pub fn dispatch_at(
        &mut self,
        event: AppEvent,
        at: Option<u64>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let now = at.unwrap_or_else(|| (self.clock)());
        let mut queue = VecDeque::from([event]);
        let mut outcome = Ok(());
        let mut changed = false;
        while let Some(event) = queue.pop_front() {
            if let Err(reason) = self.preflight(&event, now, cx) {
                outcome = Err(reason);
                changed = true;
                continue;
            }
            let mut before = self.machine.state();
            let effects = self.machine.handle(event, now);
            changed |= before != self.machine.state() || !effects.is_empty();
            // Opening the microphone can block for a second, and the key is already held.
            if effects
                .iter()
                .any(|effect| matches!(effect, Effect::StartCapture { .. }))
            {
                self.record(before);
                before = self.machine.state();
            }
            for effect in effects {
                if let Err(reason) = self.apply(effect, &mut queue, cx) {
                    outcome = Err(reason);
                }
            }
            self.record(before);
        }
        if changed {
            cx.notify();
        }
        outcome
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.machine.state() != State::Idle {
            let _ = self.dispatch(AppEvent::Tick, cx);
        }
    }

    /// The checks before a start: Home never waits for a result flash, and a start with no
    /// usable model or engine is refused before the microphone opens.
    fn preflight(
        &mut self,
        event: &AppEvent,
        now: u64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let starts = matches!(
            event,
            AppEvent::HoldDown
                | AppEvent::HandsFreeToggle
                | AppEvent::HomeToggle
                | AppEvent::FlowBarClick
        );
        if !starts {
            return Ok(());
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
            return Ok(());
        }
        if let Some(blocker) = self.blocker(cx) {
            self.refused = Some(blocker.code);
            hook::record_event("dictation", &format!("blocked {}", blocker.code));
            return Err(format!("{}: {}", blocker.code, blocker.message));
        }
        self.refused = None;
        self.machine.set_config(config_from(&self.storage));
        Ok(())
    }

    fn apply(
        &mut self,
        effect: Effect,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match effect {
            Effect::StartCapture { .. } => return self.start_capture(queue, cx),
            Effect::StopCapture { keep } => self.stop_capture(keep, queue, cx),
            Effect::Cue(cue) => {
                let failed_start = cue == Cue::Start && std::mem::take(&mut self.start_failed);
                if !failed_start {
                    hook::record_event("cue", cue_key(cue));
                }
            }
            Effect::DiscardAudio => {
                if let Some(wav) = &self.wav {
                    discard_wav(wav);
                }
            }
            Effect::Transcribe { session } => self.transcribe(session, queue),
            Effect::CancelTranscribe => {
                if let Some(token) = self.cancel.take() {
                    token.cancel();
                }
            }
            Effect::Clean { session, raw } => {
                queue.push_back(AppEvent::Cleaned { session, text: raw });
            }
            Effect::StopLlm => {}
            Effect::Insert { session, text, .. } => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                queue.push_back(AppEvent::Inserted { session });
            }
            Effect::SaveRow { status, text, .. } => self.save_row(status, text),
            Effect::UpdatePasteLast { text } => self.last_text = Some(text),
            Effect::MaxDurationWarning { seconds_left } => {
                self.warned = true;
                hook::record_event("pipeline", &format!("warning {seconds_left}s left"));
            }
            Effect::Notify { code } => self.notify(code),
        }
        Ok(())
    }

    fn start_capture(
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
        *self.detected.lock().unwrap_or_else(|p| p.into_inner()) = None;
        match self.mic.update(cx, |mic, cx| mic.start(cx)) {
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

    fn stop_capture(&mut self, keep: bool, queue: &mut VecDeque<AppEvent>, cx: &mut Context<Self>) {
        let session = self.machine.session();
        let stopped = self.mic.update(cx, |mic, cx| mic.stop(keep, cx));
        if !keep {
            self.wav = None;
            return;
        }
        match stopped {
            Ok(Some(finished)) => self.wav = Some(finished.path),
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

    fn transcribe(&mut self, session: u64, queue: &mut VecDeque<AppEvent>) {
        let Some(wav) = self.wav.clone() else {
            return;
        };
        let language = self.language();
        let spec = TranscribeSpec {
            wav_path: wav,
            language: (language != language::AUTO).then_some(language),
            prompt: None,
        };
        let token = Arc::new(CancelToken::default());
        self.cancel = Some(Arc::clone(&token));
        let run = Arc::clone(&self.engine.run);
        let events = self.events.clone();
        let detected = Arc::clone(&self.detected);
        let spawned = (self.spawn)(Box::new(move || {
            let event = match run(spec, token) {
                JobOutcome::Done(done) => {
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

    fn save_row(&mut self, status: RowStatus, text: Option<String>) {
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
                log::info!(
                    "DICTATION_DONE session={session} chars={}",
                    self.transcript.chars().count()
                );
                if let Some(wav) = &self.wav {
                    discard_wav(wav);
                }
            }
            RowStatus::Cancelled | RowStatus::Failed => {}
        }
    }

    fn notify(&mut self, code: &'static str) {
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

    /// Hook events for the state change, and the Esc grab that follows the session.
    fn record(&mut self, before: State) {
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
        let take_escape = state.takes_escape();
        if take_escape != self.esc_taken {
            self.esc_taken = take_escape;
            if let Some(session_active) = &self.session_active {
                session_active(take_escape);
            }
        }
    }

    /// The microphone failed under a running dictation. The audio so far stays on disk.
    fn mic_changed(&mut self, cx: &mut Context<Self>) {
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

    /// `hookctl state` section `global_keys`.
    pub fn keys_json(&self) -> Value {
        match &self.keys {
            Some(KeysStatus::Available) => json!({"available": true, "reason": null}),
            Some(KeysStatus::Unavailable(why)) => json!({
                "available": false,
                "reason": why.reason.key(),
                "message": why.message,
            }),
            None => json!({"available": false, "reason": "not-started"}),
        }
    }

    /// The words Home shows in its notice row when the global keys are off, if they are.
    pub fn keys_notice(&self) -> Option<&str> {
        match &self.keys {
            Some(KeysStatus::Unavailable(why)) => Some(&why.message),
            _ => None,
        }
    }

    /// `hookctl state` section `pipeline`.
    pub fn pipeline_json(&self) -> Value {
        let state = self.machine.state();
        json!({
            "state": state.key(),
            "mode": (state != State::Idle).then(|| self.machine.mode().key()),
            "session": self.machine.session(),
            "phase": self.phase().key(),
            "refused": self.refused,
            "warning": self.warned,
            "last_text_chars": self.last_text.as_ref().map(|text| text.chars().count()),
        })
    }
}

fn cue_key(cue: Cue) -> &'static str {
    match cue {
        Cue::Start => "start",
        Cue::Stop => "stop",
        Cue::Cancel => "cancel",
    }
}

fn config_from(storage: &Storage) -> Config {
    let minutes = storage
        .settings
        .get(MAX_MINUTES_SETTING)
        .and_then(|value| value.as_u64())
        .filter(|minutes| *minutes > 0);
    minutes.map_or_else(Config::default, Config::with_max_minutes)
}

/// The audio of a finished run is not kept; the test hook keeps it for inspection.
fn discard_wav(path: &Path) {
    if !cfg!(feature = "test-automation") {
        let _ = std::fs::remove_file(path);
    }
}

/// The code and the message of a failed run. Every failure after the recording keeps it.
fn failure_for(code: &str) -> (&'static str, &'static str) {
    match code {
        error::ENGINE_UNAVAILABLE => (
            error::ENGINE_UNAVAILABLE,
            "The speech engine is not available. Try again in a moment. The recording was kept.",
        ),
        error::ENGINE_NO_MODEL => (
            error::ENGINE_NO_MODEL,
            "The speech model is not loaded yet. Try again in a moment. The recording was kept.",
        ),
        error::ENGINE_BAD_AUDIO => (
            error::ENGINE_BAD_AUDIO,
            "The recording could not be read. It was kept.",
        ),
        error::ENGINE_BAD_LANGUAGE => (
            error::ENGINE_BAD_LANGUAGE,
            "That language is not supported. Choose another one.",
        ),
        error::ENGINE_NO_SPEECH => (
            ENGINE_NO_SPEECH,
            "No speech was heard. Try again and speak a little closer to the microphone.",
        ),
        error::CAPTURE_FAILED => (CAPTURE_FAILED, "Recording stopped because of an error."),
        _ => (
            ENGINE_CRASHED,
            "The speech engine stopped while transcribing. The recording was kept.",
        ),
    }
}
