//! Shared fixtures for the controller and Home tests: a fake microphone, a fake engine that
//! answers from a queue, a models folder on disk, and a clock the test moves by hand.

use super::{CancelToken, Clock, Controller, Engine, InsertSupport};
use crate::dictation::Dictation;
use crate::engine_host::LoadProblem;
use crate::mic::{Mic, MicBackend, MicSession};
use crate::models::{Models, Spawner};
use crate::net::download::{Outcome, Progress, Spec};
use crate::storage;
use gpui_kit::{AppContext, ClipboardItem, Entity, TestAppContext};
use hushpen_audio::{CaptureError, EventSink, FeedInfo, Finished, InputDevice};
use hushpen_core::catalog::Catalog;
use hushpen_core::dictation::{AppEvent, State};
use hushpen_core::insert::flow::Step;
use hushpen_core::insert::{Chord, Outcome as InsertOutcome, Overrides, Report, Restore};
use hushpen_engine::{JobOutcome, TranscribeSpec, Transcription};
use hushpen_platform::insert::{Inserter, Target};
use hushpen_store::data_dir::DataDir;
use hushpen_store::model_files;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

pub struct FakeMic {
    pub start_error: Mutex<Option<CaptureError>>,
    pub starts: std::sync::atomic::AtomicUsize,
}

struct FakeSession {
    path: PathBuf,
}

impl MicBackend for FakeMic {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        Ok(Vec::new())
    }

    fn start(
        &self,
        _device: &str,
        path: PathBuf,
        _sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        self.starts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if let Some(error) = self.start_error.lock().unwrap().clone() {
            return Err(error);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"wav").unwrap();
        Ok(Box::new(FakeSession { path }))
    }
}

impl MicSession for FakeSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError> {
        Ok(Finished {
            path: self.path,
            samples: 16_000,
            duration_ms: 1000,
        })
    }

    fn feed_wav(&self, _path: &Path) -> Result<FeedInfo, CaptureError> {
        Ok(FeedInfo {
            duration_ms: 1000,
            sample_rate: 16_000,
            channels: 1,
        })
    }
}

pub const BASE: &[u8] = b"base model bytes";
pub const TINY_EN: &[u8] = b"tiny english model";

fn catalog() -> &'static Catalog {
    let entry = |id: &str, body: &[u8], languages: &str, default: bool| {
        json!({
            "id": id,
            "name": id,
            "file": format!("ggml-{id}.bin"),
            "bytes": body.len(),
            "sha256": model_files::hex(&Sha256::digest(body)),
            "url": format!("https://huggingface.co/test/{id}.bin"),
            "license": "MIT",
            "languages": languages,
            "default": default,
        })
    };
    let text = json!({"whisper": [
        entry("base", BASE, "multilingual", true),
        entry("tiny.en", TINY_EN, "english", false),
    ]})
    .to_string();
    Box::leak(Box::new(Catalog::parse(&text).unwrap()))
}

pub fn outcome_text(text: &str, language: &str) -> JobOutcome {
    JobOutcome::Done(Transcription {
        text: text.into(),
        language: language.into(),
        segments: Vec::new(),
        audio_ms: 1000,
        decode_ms: 10,
    })
}

pub struct Rig {
    pub controller: Entity<Controller>,
    pub dictation: Entity<Dictation>,
    pub models: Entity<Models>,
    pub storage: Rc<storage::Storage>,
    pub work: Work,
    pub outcomes: Arc<Mutex<VecDeque<JobOutcome>>>,
    pub specs: Arc<Mutex<Vec<TranscribeSpec>>>,
    /// The cancel token of each job the fake engine was given, oldest first.
    pub tokens: Arc<Mutex<Vec<Arc<CancelToken>>>>,
    pub problem: Rc<RefCell<Option<LoadProblem>>>,
    pub mic_backend: Arc<FakeMic>,
    /// The controller's clock, in milliseconds.
    pub now: Rc<Cell<u64>>,
    _tmp: tempfile::TempDir,
}

/// `files` are the models on disk (`base`, `tiny.en`); `chosen` is `dictation.modelId`.
pub fn rig(cx: &mut TestAppContext, files: &[&str], chosen: &str) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    std::fs::create_dir_all(storage.data.whisper_models_dir()).unwrap();
    for id in files {
        let body = if *id == "base" { BASE } else { TINY_EN };
        let path = storage
            .data
            .whisper_models_dir()
            .join(format!("ggml-{id}.bin"));
        std::fs::write(&path, body).unwrap();
        model_files::write_stamp(&path, &model_files::hex(&Sha256::digest(body))).unwrap();
    }
    storage
        .settings
        .set("dictation.modelId", json!(chosen))
        .unwrap();
    // The pipeline tests compare the engine's words with the inserted words; the cleanup tests
    // turn the rules on.
    storage.settings.set("cleanup.rules", json!(false)).unwrap();
    let work: Work = Rc::default();
    let queue = Rc::clone(&work);
    let spawn: Spawner = Rc::new(move |job| {
        queue.borrow_mut().push_back(job);
        Ok(())
    });
    let mic_backend = Arc::new(FakeMic {
        start_error: Mutex::new(None),
        starts: std::sync::atomic::AtomicUsize::new(0),
    });
    let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), mic_backend.clone(), cx));
    let models = cx.new(|cx| {
        Models::new(
            Rc::clone(&storage),
            catalog(),
            Arc::new(
                |_: &Spec, _: &std::sync::atomic::AtomicBool, _: &mut dyn FnMut(Progress)| {
                    Ok(Outcome::Cancelled)
                },
            ),
            Rc::clone(&spawn),
            None,
            cx,
        )
    });
    let outcomes: Arc<Mutex<VecDeque<JobOutcome>>> = Arc::default();
    let specs: Arc<Mutex<Vec<TranscribeSpec>>> = Arc::default();
    let tokens: Arc<Mutex<Vec<Arc<CancelToken>>>> = Arc::default();
    let problem: Rc<RefCell<Option<LoadProblem>>> = Rc::default();
    let engine = Engine::new(
        {
            let outcomes = Arc::clone(&outcomes);
            let specs = Arc::clone(&specs);
            let tokens = Arc::clone(&tokens);
            Arc::new(move |spec, token| {
                specs.lock().unwrap().push(spec);
                tokens.lock().unwrap().push(token);
                outcomes
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("a queued outcome")
            })
        },
        {
            let problem = Rc::clone(&problem);
            Rc::new(move || problem.borrow().clone())
        },
    );
    let now = Rc::new(Cell::new(1_000));
    let clock: Clock = {
        let now = Rc::clone(&now);
        Rc::new(move || now.get())
    };
    let controller = cx.new(|cx| {
        Controller::new(
            Rc::clone(&storage),
            mic,
            models.clone(),
            engine,
            Rc::clone(&spawn),
            clock,
            cx,
        )
    });
    let dictation =
        cx.new(|cx| Dictation::new(Rc::clone(&storage), models.clone(), controller.clone(), cx));
    Rig {
        controller,
        dictation,
        models,
        storage,
        work,
        outcomes,
        specs,
        tokens,
        problem,
        mic_backend,
        now,
        _tmp: tmp,
    }
}

/// Runs the queued worker jobs on this thread and applies their events.
pub fn settle(cx: &mut TestAppContext, rig: &Rig) {
    loop {
        cx.run_until_parked();
        let Some(job) = rig.work.borrow_mut().pop_front() else {
            return;
        };
        job();
    }
}

pub fn say(rig: &Rig, outcome: JobOutcome) {
    rig.outcomes.lock().unwrap().push_back(outcome);
}

pub fn send(cx: &mut TestAppContext, rig: &Rig, event: AppEvent) -> Result<(), String> {
    rig.controller
        .update(cx, |controller, cx| controller.dispatch(event, cx))
}

/// Moves the clock to `ms` and sends the event.
pub fn send_at(cx: &mut TestAppContext, rig: &Rig, ms: u64, event: AppEvent) -> Result<(), String> {
    rig.now.set(ms);
    send(cx, rig, event)
}

pub fn machine_state(cx: &mut TestAppContext, rig: &Rig) -> State {
    rig.controller
        .read_with(cx, |controller, _| controller.state())
}

pub fn clipboard(cx: &mut TestAppContext) -> Option<String> {
    cx.update(|cx| cx.read_from_clipboard().and_then(|item| item.text()))
}

pub fn put_on_clipboard(cx: &mut TestAppContext, text: &str) {
    cx.update(|cx| cx.write_to_clipboard(ClipboardItem::new_string(text.into())));
}

/// The WAV files in the session folder.
pub fn session_wavs(rig: &Rig) -> Vec<PathBuf> {
    std::fs::read_dir(rig.storage.data.sessions_dir())
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_default()
}

/// The WAV files that history rows keep.
pub fn kept_wavs(rig: &Rig) -> Vec<PathBuf> {
    std::fs::read_dir(rig.storage.data.audio_dir())
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.extension().is_some_and(|ext| ext == "wav"))
                .collect()
        })
        .unwrap_or_default()
}

/// An inserter that records what it was asked and answers with a scripted report.
pub struct FakeInserter {
    pub target: Mutex<Target>,
    pub script: Mutex<Report>,
    pub calls: Mutex<Vec<(String, Target, Overrides)>>,
    pub panic: std::sync::atomic::AtomicBool,
}

impl FakeInserter {
    pub fn pasted_into(label: &str, chord: Chord) -> Arc<Self> {
        let mut report = Report::new(label);
        report.outcome = InsertOutcome::Pasted;
        report.chord = Some(chord);
        report.selection = Some(chord.selection());
        report.chord_sent_ms = Some(40);
        report.first_receipt_ms = Some(90);
        report.last_receipt_ms = Some(95);
        report.restore = Restore::Restored;
        Arc::new(Self {
            target: Mutex::new(Target {
                window: Some(7),
                label: label.into(),
                classes: vec![label.into()],
                own: false,
            }),
            script: Mutex::new(report),
            calls: Mutex::default(),
            panic: std::sync::atomic::AtomicBool::new(false),
        })
    }
}

impl Inserter for FakeInserter {
    fn target(&self) -> Target {
        self.target.lock().unwrap().clone()
    }

    fn insert(
        &self,
        text: &str,
        target: &Target,
        overrides: &Overrides,
        observe: &mut dyn FnMut(Step<'_>),
    ) -> Report {
        self.calls
            .lock()
            .unwrap()
            .push((text.to_owned(), target.clone(), overrides.clone()));
        if self.panic.load(std::sync::atomic::Ordering::SeqCst) {
            panic!("the paste path broke");
        }
        let report = self.script.lock().unwrap().clone();
        if report.chord_sent_ms.is_some() {
            observe(Step::ChordSent(&report));
        }
        observe(Step::Settled(&report));
        observe(Step::Restored(&report));
        report
    }
}

/// Gives the controller of `rig` this inserter, the way `main` does with the system one.
pub fn attach_inserter(cx: &mut TestAppContext, rig: &Rig, inserter: &Arc<FakeInserter>) {
    let inserter: Arc<dyn Inserter> = inserter.clone();
    rig.controller.update(cx, |controller, _| {
        controller.attach_insert(InsertSupport::Ready(inserter));
    });
}
