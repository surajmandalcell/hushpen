//! The dictation controller: one pipeline for the hold key, Home, the tray, and the flow bar.
//!
//! Every surface turns its input into an [`AppEvent`] and calls [`Controller::dispatch`], or
//! sends it down [`Controller::sender`] from another thread. The controller feeds the event to
//! the pure [`DictationMachine`] and runs the [`Effect`]s that come back: it starts and stops the
//! microphone, runs the transcription on a worker thread, and copies the words. Slow work
//! reports back as events that carry the session id, so a late answer from a cancelled run
//! changes nothing. Transcript text is never logged.

use crate::ai_cleanup::{self, Cleaner};
use crate::engine_host::{EngineHost, LoadProblem};
use crate::hook;
use crate::mic::Mic;
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{ClipboardItem, Context, Entity};
pub use history::ReprocessState;
pub(crate) use history::{retention_of, segments_of};
use hushpen_core::dictation::{
    ARM_WINDOW_MS, AppEvent, Config, Cue, DictationMachine, Effect, Mode, State,
};
use hushpen_core::error::{ENGINE_LOAD_FAILED, ENGINE_NO_MODEL, INSERT_NO_PERMISSION};
use hushpen_core::insert::Report;
use hushpen_core::language;
use hushpen_core::onboarding::{Availability, KeyGate};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use hushpen_platform::insert::{Inserter, Target};
use hushpen_platform::keys::{Reason, Unavailable};
use permissions::PermissionWatch;
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) mod ai_tests;
mod arm;
mod command;
pub use command::Flash;
#[cfg(test)]
mod arm_tests;
#[cfg(test)]
mod command_tests;
mod failure;
pub(crate) use failure::failure_for;
#[cfg(test)]
mod gate_tests;
mod history;
#[cfg(test)]
mod history_tests;
mod insert;
mod keys;
#[cfg(test)]
mod keys_tests;
#[cfg(test)]
mod overlap_tests;
pub use keys::{KeysRestart, RETRIES as KEYS_RETRIES};
mod permissions;
mod pipeline;
mod runs;
mod speak;
pub use speak::Speaker;
#[cfg(test)]
mod speak_tests;
#[cfg(test)]
pub(crate) mod testkit;
#[cfg(test)]
mod tests;
mod undo;
#[cfg(test)]
mod undo_tests;

pub const LANGUAGE_SETTING: &str = "dictation.language";

const MAX_MINUTES_SETTING: &str = "dictation.maxMinutes";

/// How often the machine is told that time passed while a session or a result flash runs.
const TICK: Duration = Duration::from_millis(100);

/// How often the idle view looks for a failed model load, which a worker thread reports.
const PROBLEM_POLL: Duration = Duration::from_secs(1);

/// How often the permissions are read again. Reading never prompts.
const PERMISSION_POLL: Duration = Duration::from_secs(5);

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

pub const CUE_SOUNDS_SETTING: &str = "audio.cueSounds";

pub const CUE_VOLUME_SETTING: &str = "audio.cueVolume";

/// Plays one cue at a volume from 0.0 to 1.0 and returns at once.
pub type CueSink = Arc<dyn Fn(Cue, f32) + Send + Sync>;

/// Cue and output control on one audio thread. `None` restores without playing a sound.
pub type DictationAudioSink = Arc<dyn Fn(Option<Cue>, Option<f32>, bool) + Send + Sync>;

/// Gives the model worker for one dictation, or `None` while AI cleanup has no running worker.
pub type CleanerSource = Rc<dyn Fn() -> Option<Box<dyn Cleaner>>>;

/// The AI step of the newest session, written by its worker thread.
type LastAi = Arc<Mutex<Option<(u64, ai_cleanup::Record)>>>;

pub struct Controller {
    storage: Rc<Storage>,
    mic: Entity<Mic>,
    models: Entity<Models>,
    engine: Engine,
    spawn: Spawner,
    clock: Clock,
    machine: DictationMachine,
    events: UnboundedSender<Queued>,
    /// Each session the controller still keeps, by id.
    runs: BTreeMap<u64, runs::Run>,
    transcript: String,
    language: Option<String>,
    notice: Option<Notice>,
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
    keys_restarter: keys::Restarter,
    session_active: Option<Rc<dyn Fn(bool)>>,
    esc_taken: bool,
    last_phase: Phase,
    insert: InsertSupport,
    last_insert: LastInsert,
    /// Held by a paste thread from its clipboard snapshot to the restore, so two pastes never
    /// share the clipboard.
    paste_turn: Arc<Mutex<()>>,
    permissions: Option<PermissionWatch>,
    cues: Option<CueSink>,
    dictation_audio: Option<DictationAudioSink>,
    ai: Option<CleanerSource>,
    /// Whether the local cleanup model is loading right now.
    ai_loading: Option<Rc<dyn Fn() -> bool>>,
    ai_progress: Arc<ai_cleanup::Progress>,
    /// The AI phase the views last drew.
    shown_ai: Option<ai_cleanup::Phase>,
    last_ai: LastAi,
    /// The session whose text a "Paste last transcript" is putting in right now. The machine
    /// has no state for it, so the controller keeps the answer.
    pasting_last: Option<u64>,
    /// The text of the paste that `pasting_last` waits for.
    pasting_text: Option<String>,
    /// The app that got the newest paste, as read when the text was ready.
    paste_target: Option<Target>,
    /// The "Undo cleanup" on offer.
    undo: Option<undo::Standby>,
    /// An undo whose paste `pasting_last` waits for.
    undoing: Option<undo::Undoing>,
    /// The text Command Mode edits, while its session listens: in memory only, never logged
    /// or saved.
    selection: Option<String>,
    /// The Command Mode start that waits for its selection read.
    selecting: Option<command::Selecting>,
    selection_seq: u64,
    /// The selection read for the start that is reaching the machine now.
    selection_ready: Option<String>,
    selections: UnboundedSender<command::SelectionRead>,
    /// The player of the read-aloud shortcut.
    speaker: Option<Speaker>,
    /// The selection read of the read-aloud shortcut that is waiting.
    speaking: Option<u64>,
    spoken: UnboundedSender<command::SelectionRead>,
    /// The newest refused Command Mode start.
    flash: Option<Flash>,
    flash_seq: u64,
    /// The hold key is down and the command chord may still follow.
    armed: Option<arm::Armed>,
    arm_seq: u64,
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
        let (selections, answers) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut answers = answers;
            while let Some(read) = answers.next().await {
                if this
                    .update(cx, |me, cx| me.selection_read(read, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let (spoken, read) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut read = read;
            while let Some(answer) = read.next().await {
                if this
                    .update(cx, |me, cx| me.spoken_selection(answer, cx))
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
            runs: BTreeMap::new(),
            transcript: String::new(),
            language: None,
            notice: None,
            result: None,
            last_result: None,
            refused: None,
            last_text: None,
            last_prompt: None,
            warned: false,
            start_failed: false,
            problem: None,
            keys: None,
            keys_restarter: keys::Restarter::default(),
            session_active: None,
            esc_taken: false,
            last_phase: Phase::Idle,
            insert: InsertSupport::Detached,
            last_insert: Arc::default(),
            paste_turn: Arc::default(),
            permissions: None,
            cues: None,
            dictation_audio: None,
            ai: None,
            ai_loading: None,
            ai_progress: Arc::default(),
            shown_ai: None,
            last_ai: Arc::default(),
            pasting_last: None,
            pasting_text: None,
            paste_target: None,
            undo: None,
            undoing: None,
            selection: None,
            selecting: None,
            selection_seq: 0,
            selection_ready: None,
            selections,
            speaker: None,
            speaking: None,
            spoken,
            flash: None,
            flash_seq: 0,
            armed: None,
            arm_seq: 0,
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

    pub fn attach_dictation_audio(&mut self, audio: DictationAudioSink) {
        self.dictation_audio = Some(audio);
    }

    /// Tells the pipeline where the cleanup model worker comes from. Without it, AI cleanup
    /// counts as unavailable and the rule text goes in.
    pub fn attach_ai(&mut self, source: CleanerSource) {
        self.ai = Some(source);
    }

    /// Tells the pipeline how to see that the local model is still loading, so a dictation in
    /// that time says so while the rule text goes in.
    pub fn attach_ai_loading(&mut self, loading: Rc<dyn Fn() -> bool>) {
        self.ai_loading = Some(loading);
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

    /// True from the moment the microphone stops until the row of the run is saved: the one
    /// stretch in which a data folder move would lose the result. An older session that
    /// finishes behind a new one counts too.
    pub fn in_flight(&self) -> bool {
        matches!(
            self.machine.state(),
            State::Transcribing | State::Cleaning | State::Inserting
        ) || self.machine.behind() > 0
            || matches!(self.reprocess, Some((_, ReprocessState::Running)))
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

    /// The audio of the newest session.
    pub fn session(&self) -> Option<&Path> {
        self.runs
            .get(&self.machine.session())
            .and_then(|run| run.wav.as_deref())
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

    /// A start never waits for the session before it, so only a blocker stops one.
    pub fn can_record(&self, cx: &gpui_kit::App) -> bool {
        self.phase() == Phase::Listening || self.blocker(cx).is_none()
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
        if self.while_selecting(&event, now, cx) {
            return Ok(());
        }
        if event == AppEvent::HoldArm {
            let _ = self.promote_armed(cx);
            return self.arm_hold(now, cx);
        }
        // The chord key inside the window takes over the armed hold; anything else means the
        // hold goes on as a plain dictation first.
        let mut adopted = None;
        if let Some(armed) = self.armed {
            if event == AppEvent::CommandDown && now.saturating_sub(armed.at) < ARM_WINDOW_MS {
                adopted = self.armed.take();
            } else {
                let _ = self.promote_armed(cx);
            }
        }
        let mut queue = VecDeque::from([event]);
        let mut outcome = Ok(());
        let mut changed = false;
        while let Some(event) = queue.pop_front() {
            if self.paste_last_answered(&event, cx) {
                changed = true;
                continue;
            }
            match self.preflight(&event, now, &mut adopted, cx) {
                Ok(command::Pass::Go) => {}
                Ok(command::Pass::Later) => {
                    changed = true;
                    continue;
                }
                Err(reason) => {
                    outcome = Err(reason);
                    changed = true;
                    continue;
                }
            }
            if event == AppEvent::ReadAloud {
                self.read_aloud_pressed(cx);
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
        if let Some(armed) = adopted {
            self.release_unused_capture(armed, cx);
        }
        self.prune_runs();
        if changed {
            cx.notify();
        }
        outcome
    }

    fn apply(
        &mut self,
        effect: Effect,
        queue: &mut VecDeque<AppEvent>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match effect {
            Effect::StartCapture { session } => return self.start_capture(session, queue, cx),
            Effect::StopCapture { session, keep } => self.stop_capture(session, keep, queue, cx),
            Effect::Cue(cue) => {
                let failed_start = cue == Cue::Start && std::mem::take(&mut self.start_failed);
                if !failed_start {
                    hook::record_event("cue", cue_key(cue));
                    self.play_cue(cue);
                }
            }
            Effect::PasteLast => self.paste_last(cx),
            Effect::Transcribe { session } => {
                let model = self.models.read(cx).effective();
                self.run_of(session).pending.model_id = Some(model.clone());
                self.transcribe(session, &model, queue);
            }
            Effect::CancelTranscribe { session } => {
                if let Some(token) = self.run_of(session).cancel.take() {
                    token.cancel();
                }
            }
            Effect::Clean { session, raw } => self.clean(session, raw, queue),
            Effect::Edit {
                session,
                instruction,
            } => self.edit(session, instruction, queue),
            Effect::StopLlm { session } => {
                if let Some(token) = self.run_of(session).ai_cancel.take() {
                    token.cancel();
                }
            }
            Effect::Insert {
                session,
                text,
                delivery,
            } => self.insert(session, text, delivery, queue, cx),
            Effect::SaveRow {
                session,
                status,
                text,
                code,
            } => self.save_row(session, status, text, code),
            Effect::UpdatePasteLast { text } => {
                if self.gate != KeyGate::Practice {
                    self.last_text = Some(text);
                }
            }
            Effect::MaxDurationWarning { seconds_left } => {
                self.warned = true;
                hook::record_event("pipeline", &format!("warning {seconds_left}s left"));
            }
            Effect::Notify { session, code } => {
                self.notify(session, code);
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

    /// `hookctl state` section `pipeline`.
    pub fn pipeline_json(&self) -> Value {
        let state = self.machine.state();
        json!({
            "state": state.key(),
            "mode": (state != State::Idle).then(|| self.machine.mode().key()),
            "session": self.machine.session(),
            "behind": self.machine.behind(),
            "phase": self.phase().key(),
            "refused": self.refused,
            "warning": self.warned,
            "last_text_chars": self.last_text.as_ref().map(|text| text.chars().count()),
            "prompt": self.last_prompt,
            "ai": self.ai_json(),
            "undo": self.undo_json(),
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

fn cue_key(cue: Cue) -> &'static str {
    match cue {
        Cue::Start => "start",
        Cue::Stop => "stop",
        Cue::Cancel => "cancel",
    }
}

fn config_from(storage: &Storage) -> Config {
    use hushpen_core::shortcut::{DEFAULT_HANDS_FREE, ENABLED_SETTING, Slot};
    let minutes = storage
        .settings
        .get(MAX_MINUTES_SETTING)
        .and_then(|value| value.as_u64())
        .filter(|minutes| *minutes > 0);
    let enabled = storage.settings.get(ENABLED_SETTING);
    let built_in = storage
        .settings
        .get(Slot::HandsFree.setting_key())
        .and_then(|value| value.as_str().map(str::to_owned))
        .is_none_or(|text| text.trim().eq_ignore_ascii_case(DEFAULT_HANDS_FREE));
    Config {
        hold_to_talk: Slot::Hold.is_on(enabled.as_ref()),
        double_tap: built_in && Slot::HandsFree.is_on(enabled.as_ref()),
        ..minutes.map_or_else(Config::default, Config::with_max_minutes)
    }
}
