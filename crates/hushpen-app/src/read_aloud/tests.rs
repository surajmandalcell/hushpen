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
    /// The code each chunk fails with, if any.
    fail: Mutex<Option<&'static str>>,
}

impl FakeVoice {
    fn new(voices: Result<Vec<Voice>, SpeechError>) -> Arc<Self> {
        Arc::new(Self {
            voices,
            spoken: Mutex::default(),
            fail: Mutex::default(),
        })
    }
}

impl SpeechEngine for FakeVoice {
    fn available(&self) -> bool {
        self.voices.is_ok()
    }

    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        self.voices.clone()
    }

    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError> {
        if let Some(code) = *self.fail.lock().unwrap() {
            return Err(SpeechError::new(code, "fake"));
        }
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
    turbo: Arc<FakeVoice>,
    library: Entity<VoiceLibrary>,
    work: Work,
    folder: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let folder = tempfile::tempdir().unwrap();
    let storage =
        Rc::new(storage::open(DataDir::open(folder.path().join("data")).unwrap()).unwrap());
    let system = FakeVoice::new(Ok(vec![
        voice("en-1", "en-US"),
        voice("en-2", "en-GB"),
        voice("de-1", "de-DE"),
    ]));
    let no_model = || Err(SpeechError::new(SPEECH_UNAVAILABLE, "no model"));
    let kokoro = FakeVoice::new(no_model());
    let turbo = FakeVoice::new(Ok(vec![voice(BUILT_IN_VOICE, "en")]));
    let nano = FakeVoice::new(no_model());
    let work: Work = Rc::default();
    let spawn: Spawner = {
        let work = Rc::clone(&work);
        Rc::new(move |job| {
            work.borrow_mut().push_back(job);
            Ok(())
        })
    };
    let window = cx.add_empty_window();
    let library = window.update(|window, cx| {
        let parts = crate::voices::Parts {
            storage: Rc::clone(&storage),
            mic: None,
            spawn: Rc::clone(&spawn),
            silent: true,
        };
        cx.new(|cx| VoiceLibrary::new(parts, window, cx))
    });
    let parts = Parts {
        storage: Rc::clone(&storage),
        system: Arc::clone(&system) as Arc<dyn SpeechEngine>,
        kokoro,
        turbo: Arc::clone(&turbo) as Arc<dyn SpeechEngine>,
        nano,
        library: Some(library.clone()),
        spawn,
        silent: true,
    };
    let read = window.update(|window, cx| cx.new(|cx| ReadAloud::new(parts, window, cx)));
    let fixture = Fixture {
        read,
        storage,
        system,
        turbo,
        library,
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

/// Keeps a cloned voice in the library as the sheet would, without a clip worth hearing.
fn clone_voice(cx: &mut VisualTestContext, fixture: &Fixture, name: &str) -> String {
    let dir = fixture.storage.data.voices_dir();
    let entry = hushpen_store::voices::Library::new(dir)
        .add(name, 6_000, 1, |path| std::fs::write(path, b"clip"))
        .unwrap();
    fixture.library.update(cx, |library, cx| library.reload(cx));
    entry.id
}

#[gpui_kit::test]
fn chatterbox_reads_with_the_default_voice_of_the_library_unless_one_is_chosen(
    cx: &mut TestAppContext,
) {
    let (fixture, cx) = open(cx);
    fixture
        .read
        .update(cx, |read, cx| {
            read.set_engine(EngineKind::ChatterboxTurbo, cx)
        })
        .unwrap();
    run_all(cx, &fixture);
    let built_in = state(cx, &fixture);
    assert_eq!(built_in["engine"], "chatterbox-turbo");
    assert_eq!(built_in["spoken_voice"], BUILT_IN_VOICE);
    assert_eq!(built_in["default_voice_name"], "Built-in");
    let anna = clone_voice(cx, &fixture, "Anna");
    fixture
        .library
        .update(cx, |library, cx| library.set_default(Some(&anna), cx))
        .unwrap();
    run_all(cx, &fixture);
    let cloned = state(cx, &fixture);
    assert_eq!(cloned["voice"], Value::Null, "the menu stays on Default");
    assert_eq!(cloned["spoken_voice"], json!(anna));
    assert_eq!(cloned["default_voice_name"], "Anna");
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    let spoken = fixture.turbo.spoken.lock().unwrap().clone();
    assert!(!spoken.is_empty());
    assert!(
        spoken
            .iter()
            .all(|(_, voice, _)| voice.as_deref() == Some(anna.as_str()))
    );
    fixture
        .read
        .update(cx, |read, cx| read.set_voice(Some(BUILT_IN_VOICE), cx))
        .unwrap();
    assert_eq!(state(cx, &fixture)["spoken_voice"], BUILT_IN_VOICE);
    assert_eq!(
        fixture.storage.settings.get(CHATTERBOX_VOICE_SETTING),
        Some(json!(BUILT_IN_VOICE))
    );
}

#[gpui_kit::test]
fn text_chatterbox_cannot_read_offers_the_system_voice(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    *fixture.turbo.fail.lock().unwrap() = Some(SPEECH_NOT_ENGLISH);
    fixture
        .read
        .update(cx, |read, cx| {
            read.set_engine(EngineKind::ChatterboxTurbo, cx)
        })
        .unwrap();
    run_all(cx, &fixture);
    type_text(cx, &fixture, "Guten Tag, wie geht es dir heute?");
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    let failed = state(cx, &fixture);
    assert!(failed["error"].as_str().unwrap().contains("English only"));
    assert_eq!(failed["offers_other_engine"], true);
    fixture
        .read
        .update(cx, |read, cx| read.set_engine(EngineKind::System, cx))
        .unwrap();
    let moved = state(cx, &fixture);
    assert_eq!(moved["engine"], "system");
    assert_eq!(moved["error"], Value::Null);
    assert_eq!(moved["offers_other_engine"], false);
}

#[gpui_kit::test]
fn the_engine_menu_marks_the_models_that_are_not_downloaded(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    fixture
        .read
        .update(cx, |read, cx| read.toggle_menu(Menu::Engine, cx));
    fixture.read.read_with(cx, |read, _| {
        assert!(read.missing(EngineKind::Kokoro));
        assert!(read.missing(EngineKind::ChatterboxNano));
        assert!(!read.missing(EngineKind::ChatterboxTurbo));
        assert!(!read.missing(EngineKind::System));
    });
    fixture
        .read
        .update(cx, |read, cx| {
            read.set_engine(EngineKind::ChatterboxNano, cx)
        })
        .unwrap();
    run_all(cx, &fixture);
    let state = state(cx, &fixture);
    assert_eq!(state["needs_model"], true);
    assert_eq!(
        state["unavailable"],
        "Chatterbox Nano needs its voice model. Download it in Models."
    );
    let engines: Vec<&str> = state["engines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|engine| engine["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        engines,
        ["system", "kokoro", "chatterbox-turbo", "chatterbox-nano"]
    );
}

#[gpui_kit::test]
fn manage_voices_opens_the_library_sheet(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    fixture
        .read
        .update_in(cx, |read, window, cx| read.manage_voices(window, cx));
    assert_eq!(
        fixture.library.read_with(cx, |library, _| library.page()),
        Some(VoicesPage::List)
    );
}

/// The rows of the history, newest first.
fn rows(fixture: &Fixture) -> Vec<Row> {
    history::page(&fixture.storage.database, "", None, 50)
        .unwrap()
        .rows
}

/// Counts the reads that went into the history, as the window hears of them.
fn count_saved(cx: &mut VisualTestContext, fixture: &Fixture) -> Rc<std::cell::Cell<usize>> {
    let saved = Rc::new(std::cell::Cell::new(0));
    let counter = Rc::clone(&saved);
    cx.update(|_, cx| {
        cx.subscribe(&fixture.read, move |_, request: &ReadRequest, _| {
            if *request == ReadRequest::Saved {
                counter.set(counter.get() + 1);
            }
        })
        .detach();
    });
    saved
}

#[gpui_kit::test]
fn a_read_spoken_to_the_end_goes_in_history_with_its_audio(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let saved = count_saved(cx, &fixture);
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| {
        read.set_voice(Some("en-2"), cx).unwrap();
        read.play(cx).unwrap();
    });
    run_all(cx, &fixture);

    let saved_rows = rows(&fixture);
    assert_eq!(saved_rows.len(), 1);
    let row = &saved_rows[0];
    assert_eq!(row.kind, "read");
    assert_eq!(row.status, "completed");
    assert_eq!(row.final_text.as_deref(), Some("Hello there."));
    assert_eq!(row.model_id.as_deref(), Some("system"));
    assert_eq!(row.source_name.as_deref(), Some("en-2"));
    assert_eq!(row.language_requested.as_deref(), Some("en"));
    assert_eq!(row.insert_outcome.as_deref(), Some("none"));
    assert!(row.duration_ms >= 100, "{} ms", row.duration_ms);
    assert_eq!(row.audio_removed_at, None);
    let audio = fixture
        .storage
        .data
        .root()
        .join(row.audio_path.as_deref().unwrap());
    assert_eq!(&std::fs::read(audio).unwrap()[..4], b"RIFF");
    assert_eq!(saved.get(), 1);

    // Play again of the same speech is the same read.
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    assert_eq!(rows(&fixture).len(), 1);
    assert_eq!(saved.get(), 1);
}

#[gpui_kit::test]
fn the_default_voice_of_an_engine_is_named_default_in_history(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    let row = rows(&fixture).remove(0);
    assert_eq!(row.source_name.as_deref(), Some(DEFAULT_VOICE));
    assert_eq!(row.language_requested, None);
}

#[gpui_kit::test]
fn with_audio_kept_never_a_read_keeps_its_text_but_not_its_audio(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    fixture
        .storage
        .settings
        .set(hushpen_store::retention::SETTING, json!("never"))
        .unwrap();
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    let row = rows(&fixture).remove(0);
    assert_eq!(row.final_text.as_deref(), Some("Hello there."));
    assert_eq!(row.audio_path, None);
    assert!(row.audio_removed_at.is_some());
    let kept = std::fs::read_dir(fixture.storage.data.audio_dir())
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(kept, 0);
}

#[gpui_kit::test]
fn a_stopped_or_failed_read_leaves_no_history_row(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    fixture.read.update(cx, |read, cx| {
        read.play(cx).unwrap();
        read.stop(cx);
    });
    run_all(cx, &fixture);
    *fixture.system.fail.lock().unwrap() = Some(SPEECH_VOICE_MISSING);
    fixture.read.update(cx, |read, cx| read.play(cx)).unwrap();
    run_all(cx, &fixture);
    assert!(state(cx, &fixture)["error"].is_string());
    assert!(rows(&fixture).is_empty());
}

#[gpui_kit::test]
fn an_export_of_new_speech_goes_in_history_once(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    type_text(cx, &fixture, "Hello there.");
    for name in ["first.wav", "second.wav"] {
        let path = fixture.folder.path().join(name);
        fixture.read.update(cx, |read, cx| read.export(cx)).unwrap();
        cx.run_until_parked();
        cx.simulate_new_path_selection(move |_| Some(path.clone()));
        run_all(cx, &fixture);
    }
    assert!(fixture.folder.path().join("second.wav").is_file());
    assert_eq!(
        rows(&fixture).len(),
        1,
        "the second export reused the speech"
    );
}
