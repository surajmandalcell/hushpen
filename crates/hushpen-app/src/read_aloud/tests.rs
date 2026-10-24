use super::*;
use crate::storage;
use gpui_kit::{TestAppContext, VisualTestContext};
use hushpen_core::error::SPEECH_UNAVAILABLE;
use hushpen_speech::{Pcm, VoiceQuality};
use hushpen_store::data_dir::DataDir;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A tenth of a second of sound for each chunk.
const CHUNK_SAMPLES: usize = SAMPLE_RATE as usize / 10;

struct FakeVoice {
    voices: Result<Vec<Voice>, SpeechError>,
    spoken: Mutex<Vec<(String, Option<String>, f32)>>,
}

impl SpeechEngine for FakeVoice {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        self.voices.clone()
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        self.spoken
            .lock()
            .unwrap()
            .push((text.to_owned(), voice.map(str::to_owned), speed));
        Ok(Pcm {
            rate: SAMPLE_RATE,
            samples: vec![0.1; CHUNK_SAMPLES],
        })
    }
}

fn voice(id: &str, language: &str) -> Voice {
    Voice {
        id: id.into(),
        name: id.into(),
        language: language.into(),
        quality: VoiceQuality::Default,
    }
}

type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

struct Fixture {
    read: Entity<ReadAloud>,
    storage: Rc<Storage>,
    system: Arc<FakeVoice>,
    work: Work,
    folder: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let folder = tempfile::tempdir().unwrap();
    let storage =
        Rc::new(storage::open(DataDir::open(folder.path().join("data")).unwrap()).unwrap());
    let system = Arc::new(FakeVoice {
        voices: Ok(vec![
            voice("en-1", "en-US"),
            voice("en-2", "en-GB"),
            voice("de-1", "de-DE"),
        ]),
        spoken: Mutex::default(),
    });
    let kokoro = Arc::new(FakeVoice {
        voices: Err(SpeechError::new(SPEECH_UNAVAILABLE, "no model")),
        spoken: Mutex::default(),
    });
    let work: Work = Rc::default();
    let spawn: Spawner = {
        let work = Rc::clone(&work);
        Rc::new(move |job| {
            work.borrow_mut().push_back(job);
            Ok(())
        })
    };
    let parts = Parts {
        storage: Rc::clone(&storage),
        system: Arc::clone(&system) as Arc<dyn SpeechEngine>,
        kokoro,
        spawn,
        silent: true,
    };
    let window = cx.add_empty_window();
    let read = window.update(|window, cx| cx.new(|cx| ReadAloud::new(parts, window, cx)));
    let fixture = Fixture {
        read,
        storage,
        system,
        work,
        folder,
    };
    run_all(window, &fixture);
    (fixture, window)
}

/// Runs every job that waits for a thread, then applies its events.
fn run_all(cx: &mut VisualTestContext, fixture: &Fixture) {
    cx.run_until_parked();
    loop {
        let job = fixture.work.borrow_mut().pop_front();
        let Some(job) = job else { break };
        job();
        cx.run_until_parked();
    }
}

fn state(cx: &mut VisualTestContext, fixture: &Fixture) -> Value {
    cx.update(|_, cx| fixture.read.read(cx).state_json(cx))
}

fn type_text(cx: &mut VisualTestContext, fixture: &Fixture, text: &str) {
    fixture
        .read
        .update(cx, |read, cx| read.set_text(text, TextFormat::Plain, cx));
}

#[gpui_kit::test]
fn the_voices_of_the_system_list_by_language_with_english_first(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let state = state(cx, &fixture);
    assert_eq!(state["engine"], "system");
    assert_eq!(state["language"], "en");
    assert_eq!(state["languages"], json!(["en", "de"]));
    let ids: Vec<String> = fixture
        .read
        .read_with(cx, |read, _| read.language_voices())
        .into_iter()
        .map(|voice| voice.id)
        .collect();
    assert_eq!(ids, ["en-1", "en-2"]);
    fixture
        .read
        .update(cx, |read, cx| read.set_language("de", cx))
        .unwrap();
    assert_eq!(
        fixture.read.read_with(cx, |read, _| read.language_voices())[0].id,
        "de-1"
    );
}

#[gpui_kit::test]
fn the_engine_voice_and_speed_are_kept_in_settings(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    fixture.read.update(cx, |read, cx| {
        read.set_voice(Some("en-2"), cx).unwrap();
        read.set_speed(1.23, cx).unwrap();
        assert!(read.set_voice(Some("fr-9"), cx).is_err());
    });
    let settings = &fixture.storage.settings;
    assert_eq!(settings.get("readAloud.voice.system"), Some(json!("en-2")));
    assert_eq!(settings.get(SPEED_SETTING), Some(json!(1.25)));

    fixture
        .read
        .update(cx, |read, cx| read.set_engine(EngineKind::Kokoro, cx))
        .unwrap();
    run_all(cx, &fixture);
    assert_eq!(settings.get(ENGINE_SETTING), Some(json!("kokoro")));
    // Each engine keeps its own voice.
    assert_eq!(state(cx, &fixture)["voice"], Value::Null);
}

#[gpui_kit::test]
fn kokoro_without_its_model_says_to_download_it_and_play_is_refused(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    fixture
        .read
        .update(cx, |read, cx| read.set_engine(EngineKind::Kokoro, cx))
        .unwrap();
    run_all(cx, &fixture);
    let state = state(cx, &fixture);
    assert_eq!(state["needs_model"], true);
    assert!(state["unavailable"].as_str().unwrap().contains("Models"));
    assert!(fixture.read.update(cx, |read, cx| read.play(cx)).is_err());
    assert!(fixture.work.borrow().is_empty(), "nothing was spoken");
}

#[gpui_kit::test]
fn play_speaks_the_text_with_the_chosen_voice_and_speed(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "One sentence. Another sentence.");
    fixture.read.update(cx, |read, cx| {
        read.set_voice(Some("en-2"), cx).unwrap();
        read.set_speed(1.5, cx).unwrap();
        read.play(cx).unwrap();
    });
    assert!(fixture.read.read_with(cx, |read, _| read.playing()));
    run_all(cx, &fixture);
    let spoken = fixture.system.spoken.lock().unwrap().clone();
    assert!(!spoken.is_empty());
    assert!(
        spoken
            .iter()
            .all(|(_, voice, speed)| voice.as_deref() == Some("en-2") && *speed == 1.5)
    );
    let state = state(cx, &fixture);
    assert_eq!(state["complete"], true);
    assert!(state["audio_ms"].as_u64().unwrap() >= 100);
}

#[gpui_kit::test]
fn play_with_no_text_says_what_to_do(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let refused = fixture.read.update(cx, |read, cx| read.play(cx));
    assert!(refused.unwrap_err().contains("no text"));
    assert!(fixture.read.read_with(cx, |read, _| read.error().is_some()));
}

#[gpui_kit::test]
fn stop_drops_speech_that_is_still_being_made(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| {
        read.play(cx).unwrap();
        read.stop(cx);
    });
    run_all(cx, &fixture);
    let state = state(cx, &fixture);
    assert_eq!(state["playing"], false);
    assert_eq!(state["audio_ms"], Value::Null);
}

#[gpui_kit::test]
fn export_writes_a_wav_of_the_speech(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    let path = fixture.folder.path().join("speech.wav");
    fixture.read.update(cx, |read, cx| read.export(cx)).unwrap();
    cx.run_until_parked();
    let chosen = path.clone();
    cx.simulate_new_path_selection(move |_| Some(chosen.clone()));
    run_all(cx, &fixture);
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert!(bytes.len() > CHUNK_SAMPLES * 2, "{} bytes", bytes.len());
    assert!(fixture.read.read_with(cx, |read, _| read.info().is_some()));
}

#[gpui_kit::test]
fn a_text_file_opens_in_the_text_area(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let path = fixture.folder.path().join("notes.md");
    std::fs::write(&path, "# Title\n\nSome words.").unwrap();
    fixture
        .read
        .update(cx, |read, cx| read.open_text_file(path, cx));
    let chars = state(cx, &fixture)["text_chars"].as_u64().unwrap();
    assert!(chars > 0);
    let missing = fixture.folder.path().join("gone.txt");
    fixture
        .read
        .update(cx, |read, cx| read.open_text_file(missing, cx));
    assert!(fixture.read.read_with(cx, |read, _| read.error().is_some()));
}
