//! Home dictation: record from the microphone, transcribe the session WAV with the speech
//! engine, show the words, and copy them to the clipboard.
//!
//! One run is listening, transcribing, then done, no-speech, or failed. The words of an
//! earlier run never stay on screen: starting a run clears them, and a run with no words ends
//! in no-speech with the clipboard untouched. Transcript text is never logged.

pub mod panel;

use crate::engine_host::{EngineHost, LoadProblem};
use crate::hook;
use crate::mic::{CaptureState, Mic};
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{ClipboardItem, Context, Entity, FocusHandle};
use hushpen_core::catalog::Languages;
use hushpen_core::error::{
    self, CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_LOAD_FAILED, ENGINE_NO_MODEL, ENGINE_NO_SPEECH,
};
use hushpen_core::language;
use hushpen_core::transcript::strip_blank_markers;
use hushpen_engine::{JobOutcome, TranscribeSpec};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const LANGUAGE_SETTING: &str = "dictation.language";
const RECENT_SETTING: &str = "dictation.recentLanguages";

/// How often the idle view looks for a failed model load, which a worker thread reports.
const PROBLEM_POLL: Duration = Duration::from_secs(1);

/// Runs one transcription and waits for it. Tests pass a fake.
pub type Runner = Arc<dyn Fn(TranscribeSpec) -> JobOutcome + Send + Sync>;

/// The speech engine as Home sees it.
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
            run: Arc::new(move |spec| client.transcribe(spec).wait()),
            problem: Rc::new(move || for_problem.load_problem()),
        }
    }
}

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

/// Why Home cannot record right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocker {
    pub code: &'static str,
    pub message: String,
    /// Models is where the user fixes it.
    pub models_link: bool,
}

enum Event {
    Finished {
        job: u64,
        wav: PathBuf,
        outcome: JobOutcome,
    },
}

pub struct Dictation {
    storage: Rc<Storage>,
    mic: Entity<Mic>,
    models: Entity<Models>,
    engine: Engine,
    spawn: Spawner,
    events: UnboundedSender<Event>,
    phase: Phase,
    transcript: String,
    /// The language the engine detected for the last words, as a whisper code.
    detected: Option<String>,
    notice: Option<Notice>,
    /// The last finished run: `done`, `no-speech`, or `failed`.
    last_result: Option<&'static str>,
    job: u64,
    wav: Option<PathBuf>,
    picker_open: bool,
    problem: Option<LoadProblem>,
    pub(crate) record_focus: FocusHandle,
    pub(crate) copy_focus: FocusHandle,
    pub(crate) models_focus: FocusHandle,
    pub(crate) language_focus: FocusHandle,
    pub(crate) option_focus: Vec<FocusHandle>,
}

impl Dictation {
    pub fn new(
        storage: Rc<Storage>,
        mic: Entity<Mic>,
        models: Entity<Models>,
        engine: Engine,
        spawn: Spawner,
        cx: &mut Context<Self>,
    ) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(event) = queue.next().await {
                if this.update(cx, |me, cx| me.handle(event, cx)).is_err() {
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
        // One handle for Auto and one for each language.
        let option_focus = (0..=language::LANGUAGES.len())
            .map(|_| cx.focus_handle().tab_stop(true))
            .collect();
        Self {
            storage,
            mic,
            models,
            engine,
            spawn,
            events,
            phase: Phase::Idle,
            transcript: String::new(),
            detected: None,
            notice: None,
            last_result: None,
            job: 0,
            wav: None,
            picker_open: false,
            problem: None,
            record_focus: cx.focus_handle().tab_stop(true),
            copy_focus: cx.focus_handle().tab_stop(true),
            models_focus: cx.focus_handle().tab_stop(true),
            language_focus: cx.focus_handle().tab_stop(true),
            option_focus,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn transcript(&self) -> &str {
        &self.transcript
    }

    pub fn detected(&self) -> Option<&str> {
        self.detected.as_deref()
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn picker_open(&self) -> bool {
        self.picker_open
    }

    pub fn session(&self) -> Option<&std::path::Path> {
        self.wav.as_deref()
    }

    /// The `dictation.language` setting: `auto` or a whisper code.
    pub fn language(&self) -> String {
        self.storage
            .settings
            .get(LANGUAGE_SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| language::AUTO.to_owned())
    }

    fn recent(&self) -> Vec<String> {
        self.storage
            .settings
            .get(RECENT_SETTING)
            .and_then(|value| {
                value.as_array().map(|list| {
                    list.iter()
                        .filter_map(|code| code.as_str().map(str::to_owned))
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// `auto`, then the recent languages, then English and the rest by name.
    pub fn picker_codes(&self) -> Vec<&'static str> {
        let mut codes = vec![language::AUTO];
        codes.extend(language::picker_order(&self.recent()));
        codes
    }

    /// Why the picker is off, or `None` while it works.
    pub fn picker_off_reason(&self, cx: &gpui_kit::App) -> Option<&'static str> {
        let models = self.models.read(cx);
        let id = models.effective();
        let english_only = models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .is_some_and(|row| row.entry.languages == Languages::English);
        english_only.then_some("This model understands English only.")
    }

    /// Why Home cannot start a recording, or `None` while it can.
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
        let models = self.models.read(cx);
        if models.usable().is_some() {
            return None;
        }
        Some(Blocker {
            code: ENGINE_NO_MODEL,
            message: "A speech model is needed before you can dictate. Get one in Models.".into(),
            models_link: true,
        })
    }

    pub fn can_record(&self, cx: &gpui_kit::App) -> bool {
        match self.phase {
            Phase::Listening => true,
            Phase::Transcribing => false,
            _ => self.blocker(cx).is_none(),
        }
    }

    /// What the record button does: stop while listening, otherwise start.
    pub fn toggle(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        match self.phase {
            Phase::Listening => self.stop(cx),
            Phase::Transcribing => Err("a transcription is still running".into()),
            _ => self.start(cx),
        }
    }

    pub fn start(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.phase.busy() {
            return Err("a dictation is already running".into());
        }
        if let Some(blocker) = self.blocker(cx) {
            hook::record_event("dictation", &format!("blocked {}", blocker.code));
            return Err(format!("{}: {}", blocker.code, blocker.message));
        }
        self.clear_run();
        match self.mic.update(cx, |mic, cx| mic.start(cx)) {
            Ok(path) => {
                self.wav = Some(path);
                self.phase = Phase::Listening;
                hook::record_event("dictation", "listening");
                cx.notify();
                Ok(())
            }
            Err(error) => {
                let message = self
                    .mic
                    .read(cx)
                    .notice()
                    .map_or_else(|| error.to_string(), |notice| notice.message.clone());
                self.fail(error.code, message, cx);
                Err(error.to_string())
            }
        }
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.phase != Phase::Listening {
            return Err("no dictation is listening".into());
        }
        match self.mic.update(cx, |mic, cx| mic.stop(true, cx)) {
            Ok(Some(finished)) => {
                self.begin(finished.path, cx);
                Ok(())
            }
            Ok(None) => {
                self.fail(
                    CAPTURE_FAILED,
                    "The recording ended before it could be transcribed.".into(),
                    cx,
                );
                Err("the capture session was already gone".into())
            }
            Err(error) => {
                let message = self
                    .mic
                    .read(cx)
                    .notice()
                    .map_or_else(|| error.to_string(), |notice| notice.message.clone());
                self.fail(error.code, message, cx);
                Err(error.to_string())
            }
        }
    }

    fn begin(&mut self, wav: PathBuf, cx: &mut Context<Self>) {
        self.job += 1;
        let job = self.job;
        let language = self.language();
        let spec = TranscribeSpec {
            wav_path: wav.clone(),
            language: (language != language::AUTO).then_some(language),
            prompt: None,
        };
        let run = Arc::clone(&self.engine.run);
        let events = self.events.clone();
        let sent_wav = wav.clone();
        let spawned = (self.spawn)(Box::new(move || {
            let outcome = run(spec);
            let _ = events.unbounded_send(Event::Finished {
                job,
                wav: sent_wav,
                outcome,
            });
        }));
        self.wav = Some(wav);
        if let Err(error) = spawned {
            log::warn!("{ENGINE_CRASHED} could not start the transcription: {error}");
            self.fail(
                ENGINE_CRASHED,
                "Transcription could not start. The recording was kept.".into(),
                cx,
            );
            return;
        }
        self.phase = Phase::Transcribing;
        hook::record_event("dictation", "transcribing");
        cx.notify();
    }

    fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        let Event::Finished { job, wav, outcome } = event;
        if job != self.job || self.phase != Phase::Transcribing {
            return;
        }
        match outcome {
            JobOutcome::Done(done) => {
                let text = strip_blank_markers(&done.text);
                if text.is_empty() {
                    self.no_speech(&wav, cx);
                } else {
                    log::info!("DICTATION_DONE job={job} chars={}", text.chars().count());
                    self.detected = (done.language != "und").then_some(done.language);
                    self.transcript = text;
                    self.phase = Phase::Done;
                    self.last_result = Some("done");
                    cx.write_to_clipboard(ClipboardItem::new_string(self.transcript.clone()));
                    discard_wav(&wav);
                    hook::record_event("dictation", "done");
                    cx.notify();
                }
            }
            JobOutcome::Failed(failure) if failure.code == ENGINE_NO_SPEECH => {
                self.no_speech(&wav, cx);
            }
            JobOutcome::Failed(failure) => {
                log::warn!("{} dictation job {job} failed", failure.code);
                let (code, message) = failure_for(&failure.code);
                self.fail(code, message.into(), cx);
            }
            JobOutcome::Cancelled => {
                self.phase = Phase::Idle;
                hook::record_event("dictation", "idle");
                cx.notify();
            }
        }
    }

    fn no_speech(&mut self, wav: &std::path::Path, cx: &mut Context<Self>) {
        self.phase = Phase::NoSpeech;
        self.last_result = Some("no-speech");
        self.notice = Some(Notice {
            code: ENGINE_NO_SPEECH,
            message: "No speech was heard. Try again and speak a little closer to the microphone."
                .into(),
        });
        discard_wav(wav);
        hook::record_event("dictation", "no-speech");
        cx.notify();
    }

    fn fail(&mut self, code: &'static str, message: String, cx: &mut Context<Self>) {
        self.phase = Phase::Failed;
        self.last_result = Some("failed");
        self.notice = Some(Notice { code, message });
        hook::record_event("dictation", &format!("failed {code}"));
        cx.notify();
    }

    /// Forgets the last run before a new one starts, so its words can never show again.
    fn clear_run(&mut self) {
        self.transcript.clear();
        self.detected = None;
        self.notice = None;
        self.wav = None;
    }

    /// The microphone failed under a running dictation. The audio so far stays on disk.
    fn mic_changed(&mut self, cx: &mut Context<Self>) {
        if self.phase == Phase::Listening && self.mic.read(cx).state() != CaptureState::Listening {
            let message = self.mic.read(cx).notice().map_or_else(
                || "Recording stopped because of an error.".to_owned(),
                |notice| notice.message.clone(),
            );
            self.fail(CAPTURE_FAILED, message, cx);
        }
        cx.notify();
    }

    fn poll_problem(&mut self, cx: &mut Context<Self>) {
        let current = (self.engine.problem)();
        if current != self.problem {
            self.problem = current;
            cx.notify();
        }
    }

    pub fn toggle_picker(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(reason) = self.picker_off_reason(cx) {
            self.picker_open = false;
            return Err(reason.to_owned());
        }
        self.picker_open = !self.picker_open;
        cx.notify();
        Ok(())
    }

    pub fn close_picker(&mut self, cx: &mut Context<Self>) {
        if self.picker_open {
            self.picker_open = false;
            cx.notify();
        }
    }

    /// Saves the language of the next dictation: `auto` or a whisper code.
    pub fn set_language(&mut self, code: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if !language::is_setting_value(code) {
            return Err(format!("'{code}' is not a language whisper knows"));
        }
        if let Some(reason) = self.picker_off_reason(cx) {
            return Err(reason.to_owned());
        }
        self.storage
            .settings
            .set(LANGUAGE_SETTING, json!(code))
            .map_err(|error| error.to_string())?;
        let recent = language::push_recent(&self.recent(), code);
        self.storage
            .settings
            .set(RECENT_SETTING, json!(recent))
            .map_err(|error| error.to_string())?;
        self.picker_open = false;
        hook::record_event("dictation", &format!("language {code}"));
        cx.notify();
        Ok(())
    }

    /// Copies the shown transcript again.
    pub fn copy(&self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.transcript.is_empty() {
            return Err("there is no transcript to copy".into());
        }
        cx.write_to_clipboard(ClipboardItem::new_string(self.transcript.clone()));
        hook::record_event("dictation", "copied");
        Ok(())
    }

    pub fn state_json(&self, cx: &gpui_kit::App) -> Value {
        let models = self.models.read(cx);
        let blocker = self.blocker(cx);
        let off = self.picker_off_reason(cx);
        let language = self.language();
        json!({
            "state": self.phase.key(),
            "transcript": self.transcript,
            "language": self.detected,
            "language_name": self.detected.as_deref().and_then(language::name),
            "notice": self.notice.as_ref().map(|notice| json!({
                "code": notice.code,
                "message": notice.message,
            })),
            "last_result": self.last_result,
            "blocker": blocker.as_ref().map(|blocker| json!({
                "code": blocker.code,
                "message": blocker.message,
                "models_link": blocker.models_link,
            })),
            "can_record": self.can_record(cx),
            "model": models.effective(),
            "picker": {
                "selected": language,
                "label": language::label(&language),
                "open": self.picker_open,
                "enabled": off.is_none(),
                "reason": off,
                "options": self.picker_codes().len(),
            },
            "session": self.wav.as_ref().map(|path| path.to_string_lossy().into_owned()),
        })
    }
}

/// The audio of a finished run is not kept; the test hook keeps it for inspection.
fn discard_wav(path: &std::path::Path) {
    if !cfg!(feature = "test-automation") {
        let _ = std::fs::remove_file(path);
    }
}

/// The code and the message of a failed job. Every failure keeps the recording.
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
        _ => (
            ENGINE_CRASHED,
            "The speech engine stopped while transcribing. The recording was kept.",
        ),
    }
}

#[cfg(test)]
mod tests;
