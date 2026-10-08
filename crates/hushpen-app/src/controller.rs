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
pub use history::ReprocessState;
use hushpen_core::cleanup;
use hushpen_core::dictation::{
    AppEvent, Config, Cue, Delivery, DictationMachine, Effect, Mode, RowStatus, State,
};
use hushpen_core::dictionary;
use hushpen_core::error::{
    self, CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_LOAD_FAILED, ENGINE_NO_MODEL, ENGINE_NO_SPEECH,
    INSERT_KEYBOARD_GRABBED, INSERT_NO_PERMISSION, INSERT_NO_RECEIPT, INSERT_NO_TRANSCRIPT,
    INSERT_SECURE_FIELD, INSERT_WAYLAND,
};
use hushpen_core::insert::flow::Step;
use hushpen_core::insert::{Outcome, Overrides, Report};
use hushpen_core::language;
use hushpen_core::onboarding::{Availability, KeyGate};
use hushpen_core::permission::{Permissions, Preflight};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use hushpen_platform::insert::Inserter;
use hushpen_platform::keys::{Reason, Unavailable};
use hushpen_store::dictionary as store_dictionary;
use permissions::PermissionWatch;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod gate_tests;
mod history;
#[cfg(test)]
mod history_tests;
mod permissions;
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
/// How often the permissions are read again. Reading never prompts.
const PERMISSION_POLL: Duration = Duration::from_secs(5);
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

/// How finished text reaches the focused app.
#[derive(Clone)]
pub enum InsertSupport {
    /// Nothing was set up: the text is copied and no key is pressed.
    Detached,
    Ready(Arc<dyn Inserter>),
    /// The system cannot paste (Wayland, no display): the text is copied and Home says why.
    Unavailable(Unavailable),
}

/// The newest insertion: its session, the wall clock time at which the text was ready, and what
/// happened. A worker thread writes it as the steps go by.
type LastInsert = Arc<Mutex<Option<(u64, u64, Report)>>>;
type TimedSegments = (u64, Vec<hushpen_store::history::Segment>);

const INSERT_COPIED: &str = "INSERT_COPIED";

const CLEANUP_RULES_SETTING: &str = "cleanup.rules";
const SPOKEN_PUNCTUATION_SETTING: &str = "cleanup.spokenPunctuation";
pub const CUE_SOUNDS_SETTING: &str = "audio.cueSounds";
pub const CUE_VOLUME_SETTING: &str = "audio.cueVolume";

/// Plays one cue at a volume from 0.0 to 1.0 and returns at once.
pub type CueSink = Arc<dyn Fn(Cue, f32) + Send + Sync>;

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
    /// The timed pieces of the shown words, with the session that they belong to.
    segments: Arc<Mutex<Option<TimedSegments>>>,
    transcript: String,
    language: Option<String>,
    notice: Option<Notice>,
    /// Text for the next notice, when the failing part knows more than the code.
    detail: Option<String>,
    result: Option<Phase>,
    last_result: Option<&'static str>,
    refused: Option<&'static str>,
    last_text: Option<String>,
    /// The whisper prompt of the newest transcription, for the hook.
    last_prompt: Option<String>,
    warned: bool,
    start_failed: bool,
    problem: Option<LoadProblem>,
    keys: Option<KeysStatus>,
    session_active: Option<Rc<dyn Fn(bool)>>,
    esc_taken: bool,
    last_phase: Phase,
    insert: InsertSupport,
    last_insert: LastInsert,
    permissions: Option<PermissionWatch>,
    cues: Option<CueSink>,
    /// The session whose text a "Paste last transcript" is putting in right now. The machine
    /// has no state for it, so the controller keeps the answer.
    pasting_last: Option<u64>,
    /// The text of the paste that `pasting_last` waits for.
    pasting_text: Option<String>,
    /// What the history row of the running session needs from the steps before the save.
    pending: history::Pending,
    /// Counts the saved and changed history rows, so the History view reloads only when needed.
    history_revision: u64,
    /// The id of the newest row a run saved.
    last_row: Option<String>,
    reprocess: Option<(String, ReprocessState)>,
    /// Whether onboarding lets a dictation start, and where its words go.
    gate: KeyGate,
    /// Words that a practice dictation delivered, until onboarding takes them.
    practice: Option<String>,
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
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PERMISSION_POLL).await;
                if this
                    .update(cx, |me, cx| me.refresh_permissions(cx))
                    .is_err()
                {
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
            segments: Arc::default(),
            transcript: String::new(),
            language: None,
            notice: None,
            detail: None,
            result: None,
            last_result: None,
            refused: None,
            last_text: None,
            last_prompt: None,
            warned: false,
            start_failed: false,
            problem: None,
            keys: None,
            session_active: None,
            esc_taken: false,
            last_phase: Phase::Idle,
            insert: InsertSupport::Detached,
            last_insert: Arc::default(),
            permissions: None,
            cues: None,
            pasting_last: None,
            pasting_text: None,
            pending: history::Pending::default(),
            history_revision: 0,
            last_row: None,
            reprocess: None,
            gate: KeyGate::Open,
            practice: None,
        }
    }

    pub fn attach_insert(&mut self, support: InsertSupport) {
        self.insert = support;
    }

    pub fn attach_cues(&mut self, cues: CueSink) {
        self.cues = Some(cues);
    }

    /// The rule cleanup, unless `cleanup.rules` is off, and then the dictionary replacements,
    /// which run either way. The settings and the dictionary are read each time, so a change
    /// takes effect on the next dictation.
    fn rule_cleanup(&self, raw: &str) -> String {
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
    fn dictionary(&self) -> Vec<dictionary::Entry> {
        store_dictionary::list(&self.storage.database).unwrap_or_else(|error| {
            log::warn!("the dictionary could not be read: {error}");
            Vec::new()
        })
    }

    /// Plays the cue unless the user turned the sounds off. The settings are read each time, so
    /// a change takes effect on the next cue.
    fn play_cue(&self, cue: Cue) {
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

    /// Reads the permissions now and again every few seconds, and records which grants are
    /// lost since the last start.
    pub fn attach_permissions(&mut self, preflight: Arc<dyn Preflight>) {
        self.permissions = Some(PermissionWatch::start(preflight, &self.storage.settings));
    }

    pub fn refresh_permissions(&mut self, cx: &mut Context<Self>) {
        if let Some(watch) = &mut self.permissions
            && watch.refresh(&self.storage.settings)
        {
            cx.notify();
        }
    }

    /// The permissions that were granted before and are missing now, for onboarding.
    pub fn lost_permissions(&self) -> &[hushpen_core::permission::Permission] {
        self.permissions.as_ref().map_or(&[], PermissionWatch::lost)
    }

    /// What the system lets Hushpen do now, as of the last read.
    pub fn access(&self) -> Permissions {
        self.permissions.as_ref().map_or_else(
            || Permissions::read(&hushpen_core::permission::NotApplicable),
            |watch| *watch.current(),
        )
    }

    /// Whether the global keys run, for the onboarding permissions page.
    pub fn keys_availability(&self) -> Availability {
        match &self.keys {
            Some(KeysStatus::Available) => Availability::Ready,
            Some(KeysStatus::Unavailable(why)) => unavailable(why),
            None => Availability::Off("Global keys are not running.".into()),
        }
    }

    /// Whether finished text can be pasted, for the onboarding permissions page.
    pub fn paste_availability(&self) -> Availability {
        match &self.insert {
            InsertSupport::Ready(_) => Availability::Ready,
            InsertSupport::Unavailable(why) => unavailable(why),
            InsertSupport::Detached => Availability::Off("Paste is not set up.".into()),
        }
    }

    /// The engine call, for work that is not a dictation, such as the microphone test.
    pub fn runner(&self) -> Runner {
        Arc::clone(&self.engine.run)
    }

    pub fn spawner(&self) -> Spawner {
        Rc::clone(&self.spawn)
    }

    /// Sets what onboarding allows. `Closed` refuses every start, `Practice` lets dictations
    /// start but delivers their words to [`Controller::take_practice`], and `Open` is normal.
    pub fn set_gate(&mut self, gate: KeyGate, cx: &mut Context<Self>) {
        if self.gate != gate {
            self.gate = gate;
            hook::record_event("onboarding", &format!("gate {gate:?}"));
            cx.notify();
        }
    }

    pub fn gate(&self) -> KeyGate {
        self.gate
    }

    /// The words of the newest practice dictation, once.
    pub fn take_practice(&mut self) -> Option<String> {
        self.practice.take()
    }

    /// `hookctl state` section `permissions`.
    pub fn permissions_json(&self) -> Value {
        self.permissions
            .as_ref()
            .map_or_else(permissions::unattached_json, PermissionWatch::to_json)
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

    /// The code of the failure that ended the run, while the pipeline shows `Failed`.
    pub fn failure_code(&self) -> Option<&'static str> {
        (self.machine.state() == State::Failed)
            .then(|| self.notice.as_ref().map(|notice| notice.code))
            .flatten()
    }

    /// The id of the newest history row a dictation saved, for "Open history".
    pub fn last_row_id(&self) -> Option<&str> {
        self.last_row.as_deref()
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
            if self.paste_last_answered(&event, cx) {
                changed = true;
                continue;
            }
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
        let refused = match self.gate {
            KeyGate::Open => false,
            KeyGate::Closed => starts || matches!(event, AppEvent::PasteLast),
            KeyGate::Practice => matches!(event, AppEvent::PasteLast),
        };
        if refused {
            hook::record_event("onboarding", "key-blocked");
            return Err("onboarding is not finished: no dictation can start yet".into());
        }
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
                    self.play_cue(cue);
                }
            }
            Effect::PasteLast => self.paste_last(cx),
            Effect::Transcribe { session } => {
                self.pending.model_id = Some(self.models.read(cx).effective());
                self.transcribe(session, queue);
            }
            Effect::CancelTranscribe => {
                if let Some(token) = self.cancel.take() {
                    token.cancel();
                }
            }
            Effect::Clean { session, raw } => {
                let text = self.rule_cleanup(&raw);
                self.pending.raw = raw;
                self.pending.rule = text.clone();
                queue.push_back(AppEvent::Cleaned { session, text });
            }
            Effect::StopLlm => {}
            Effect::Insert {
                session,
                text,
                delivery,
            } => self.insert(session, text, delivery, queue, cx),
            Effect::SaveRow { status, text, code } => self.save_row(status, text, code),
            Effect::UpdatePasteLast { text } => {
                if self.gate != KeyGate::Practice {
                    self.last_text = Some(text);
                }
            }
            Effect::MaxDurationWarning { seconds_left } => {
                self.warned = true;
                hook::record_event("pipeline", &format!("warning {seconds_left}s left"));
            }
            Effect::Notify { code } => {
                self.notify(code);
                if code == INSERT_NO_PERMISSION
                    && let Some(text) = self.last_text.clone()
                {
                    // The guard writes nothing; the copy is for the user's own paste.
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                }
            }
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

    fn stop_capture(&mut self, keep: bool, queue: &mut VecDeque<AppEvent>, cx: &mut Context<Self>) {
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

    fn transcribe(&mut self, session: u64, queue: &mut VecDeque<AppEvent>) {
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

    /// Puts the finished text into the focused app, or copies it when it cannot. The text is
    /// kept for "Paste last transcript" before anything can fail.
    fn insert(
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
            if self.paste(session, &text, inserter, ready_unix_ms) {
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
    fn paste_last(&mut self, cx: &mut Context<Self>) {
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
        self.paste_text(text, cx);
    }

    /// Puts `text` into the focused app, or copies it when the system cannot paste.
    fn paste_text(&mut self, text: String, cx: &mut Context<Self>) {
        let session = self.machine.session();
        let ready_unix_ms = unix_ms();
        if let InsertSupport::Ready(inserter) = &self.insert {
            let inserter = Arc::clone(inserter);
            if self.paste(session, &text, inserter, ready_unix_ms) {
                self.pasting_last = Some(session);
                self.pasting_text = Some(text);
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
    fn paste_last_answered(&mut self, event: &AppEvent, cx: &mut Context<Self>) -> bool {
        let (session, code) = match event {
            AppEvent::Inserted { session } => (*session, None),
            AppEvent::InsertFailed { session, code } => (*session, Some(*code)),
            _ => return false,
        };
        if self.pasting_last != Some(session) {
            return false;
        }
        self.pasting_last = None;
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
    fn paste(
        &mut self,
        session: u64,
        text: &str,
        inserter: Arc<dyn Inserter>,
        ready_unix_ms: u64,
    ) -> bool {
        let target = inserter.target();
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
        let spawned = (self.spawn)(Box::new(move || {
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
    fn copy_notice(&mut self, session: u64) {
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

    fn save_row(&mut self, status: RowStatus, text: Option<String>, code: Option<&'static str>) {
        if self.gate == KeyGate::Practice {
            // A practice run is not the user's dictation: no row, and no audio kept.
            if let Some(wav) = self.wav.take() {
                let _ = std::fs::remove_file(wav);
            }
        } else {
            self.record_history(status, &text, code);
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
            "prompt": self.last_prompt,
        })
    }
}

fn unavailable(why: &Unavailable) -> Availability {
    if why.reason == Reason::Wayland {
        Availability::NotOnWayland
    } else {
        Availability::Off(why.message.clone())
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

/// The code and the message of a failed run. Every failure after the recording keeps it.
pub(crate) fn failure_for(code: &str) -> (&'static str, &'static str) {
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
        INSERT_NO_RECEIPT => (
            INSERT_NO_RECEIPT,
            "The app did not take the text. Use Paste last transcript to try again.",
        ),
        INSERT_NO_PERMISSION => (
            INSERT_NO_PERMISSION,
            "Hushpen is not allowed to press keys, so it could not paste. The text is on your clipboard. Use Paste last transcript once you allow it.",
        ),
        INSERT_SECURE_FIELD => (
            INSERT_SECURE_FIELD,
            "Secure input is on, so Hushpen did not paste. Your clipboard is unchanged. Use Paste last transcript in another field.",
        ),
        INSERT_KEYBOARD_GRABBED => (
            INSERT_KEYBOARD_GRABBED,
            "Another app holds the keyboard, so Hushpen did not paste. Your clipboard is unchanged. Use Paste last transcript when the keyboard is free.",
        ),
        INSERT_NO_TRANSCRIPT => (
            INSERT_NO_TRANSCRIPT,
            "No transcript is available yet. Dictate something first.",
        ),
        _ => (
            ENGINE_CRASHED,
            "The speech engine stopped while transcribing. The recording was kept.",
        ),
    }
}
