use super::*;
use crate::storage;
use gpui_kit::{TestAppContext, VisualTestContext};
use hushpen_store::data_dir::DataDir;
use std::cell::RefCell;
use std::collections::VecDeque;

type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

struct Fixture {
    library: Entity<VoiceLibrary>,
    storage: Rc<Storage>,
    work: Work,
    folder: tempfile::TempDir,
}

fn open(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let folder = tempfile::tempdir().unwrap();
    let storage =
        Rc::new(storage::open(DataDir::open(folder.path().join("data")).unwrap()).unwrap());
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
        mic: None,
        spawn,
        silent: true,
    };
    let window = cx.add_empty_window();
    let library = window.update(|window, cx| cx.new(|cx| VoiceLibrary::new(parts, window, cx)));
    let fixture = Fixture {
        library,
        storage,
        work,
        folder,
    };
    (fixture, window)
}

/// Runs every job that waits for a thread, then applies its messages.
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
    cx.update(|_, cx| fixture.library.read(cx).state_json(cx))
}

/// A WAV of `seconds` of a 220 Hz tone after a second of quiet, the way speech sits in a take.
fn speech_file(fixture: &Fixture, name: &str, seconds: f32) -> PathBuf {
    let rate = hushpen_audio::resample::TARGET_RATE;
    let mut samples = vec![0.0; rate as usize];
    samples.extend(
        (0..(seconds * rate as f32) as usize)
            .map(|n| 0.3 * (n as f32 * 220.0 * std::f32::consts::TAU / rate as f32).sin()),
    );
    let path = fixture.folder.path().join(name);
    write_wav(&path, rate, &samples).unwrap();
    path
}

/// Adds a voice from a file of `seconds` of speech under `name`.
fn add(cx: &mut VisualTestContext, fixture: &Fixture, name: &str, seconds: f32) -> VoiceEntry {
    let path = speech_file(fixture, &format!("{name}.wav"), seconds);
    fixture.library.update(cx, |me, cx| {
        me.show(Page::Add, cx);
        me.load_file(path, cx);
    });
    run_all(cx, fixture);
    let name = name.to_owned();
    fixture.library.update(cx, |me, cx| {
        let field = me.name.clone();
        me.set_input(&field, name, cx);
    });
    cx.run_until_parked();
    fixture.library.update(cx, |me, cx| me.save(cx)).unwrap()
}

#[gpui_kit::test]
fn a_file_becomes_a_voice_with_a_trimmed_24_khz_clip(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let path = speech_file(&fixture, "anna.wav", 8.0);
    fixture.library.update(cx, |me, cx| {
        me.show(Page::Add, cx);
        me.load_file(path, cx);
    });
    assert_eq!(state(cx, &fixture)["take"]["state"], "preparing");
    run_all(cx, &fixture);
    let ready = state(cx, &fixture);
    assert_eq!(ready["sheet"], "add");
    assert_eq!(ready["name"], "Voice 1", "the field offers a free name");
    let ms = ready["clip"]["duration_ms"].as_u64().unwrap();
    assert!(ms.abs_diff(8_200) < 200, "the quiet is trimmed: {ms} ms");
    let entry = fixture.library.update(cx, |me, cx| me.save(cx)).unwrap();
    let saved = state(cx, &fixture);
    assert_eq!(saved["sheet"], "list", "saving goes back to the list");
    assert_eq!(saved["voices"][0]["name"], "Voice 1");
    let clip = PathBuf::from(saved["voices"][0]["clip"].as_str().unwrap());
    let wav = std::fs::read(&clip).unwrap();
    let channels = u16::from_le_bytes([wav[22], wav[23]]);
    let rate = u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]);
    assert_eq!((rate, channels), (SAMPLE_RATE, 1));
    assert_eq!(entry.duration_ms, ms);
}

#[gpui_kit::test]
fn a_short_file_is_refused_and_save_says_why(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let path = speech_file(&fixture, "short.wav", 3.0);
    fixture.library.update(cx, |me, cx| {
        me.show(Page::Add, cx);
        me.load_file(path, cx);
    });
    run_all(cx, &fixture);
    let refused = state(cx, &fixture);
    assert!(
        refused["notice"].as_str().unwrap().contains("at least 5 s"),
        "{}",
        refused["notice"]
    );
    assert_eq!(refused["clip"], Value::Null);
    let save = fixture.library.update(cx, |me, cx| me.save(cx));
    assert!(save.unwrap_err().contains("Record a clip or choose a file"));
    let missing = fixture.folder.path().join("gone.wav");
    fixture
        .library
        .update(cx, |me, cx| me.load_file(missing, cx));
    run_all(cx, &fixture);
    assert!(state(cx, &fixture)["notice"].as_str().is_some());
    assert_eq!(state(cx, &fixture)["voices"], json!([]));
}

#[gpui_kit::test]
fn the_default_voice_is_kept_in_settings_and_a_deleted_voice_lets_go_of_it(
    cx: &mut TestAppContext,
) {
    let (fixture, cx) = open(cx);
    let anna = add(cx, &fixture, "Anna", 6.0);
    let ben = add(cx, &fixture, "Ben", 6.0);
    let settings = &fixture.storage.settings;
    assert_eq!(state(cx, &fixture)["default_name"], BUILT_IN_NAME);
    fixture
        .library
        .update(cx, |me, cx| me.set_default(Some(&ben.id), cx))
        .unwrap();
    assert_eq!(settings.get(DEFAULT_SETTING), Some(json!(ben.id)));
    let set = state(cx, &fixture);
    assert_eq!(
        (set["default"].clone(), set["default_name"].clone()),
        (json!(ben.id), json!("Ben"))
    );
    assert_eq!(set["voices"][1]["default"], true);
    assert!(
        fixture
            .library
            .update(cx, |me, cx| me.set_default(Some("v404"), cx))
            .is_err()
    );
    settings
        .set(crate::read_aloud::CHATTERBOX_VOICE_SETTING, json!(ben.id))
        .unwrap();
    fixture
        .library
        .update(cx, |me, cx| me.delete(&ben.id, cx))
        .unwrap();
    assert_eq!(settings.get(DEFAULT_SETTING), Some(json!("")));
    assert_eq!(
        settings.get(crate::read_aloud::CHATTERBOX_VOICE_SETTING),
        Some(json!(""))
    );
    let left = state(cx, &fixture);
    assert_eq!(left["voices"].as_array().unwrap().len(), 1);
    assert_eq!(left["voices"][0]["id"], json!(anna.id));
    assert_eq!(left["default"], Value::Null);
    fixture
        .library
        .update(cx, |me, cx| me.set_default(Some(&anna.id), cx))
        .unwrap();
    fixture
        .library
        .update(cx, |me, cx| me.set_default(None, cx))
        .unwrap();
    assert_eq!(state(cx, &fixture)["default_name"], BUILT_IN_NAME);
}

#[gpui_kit::test]
fn a_rename_keeps_names_unique_and_not_empty(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let anna = add(cx, &fixture, "Anna", 6.0);
    add(cx, &fixture, "Ben", 6.0);
    let taken = fixture
        .library
        .update(cx, |me, cx| me.rename_voice(&anna.id, "ben", cx));
    assert!(taken.unwrap_err().contains("Another voice"));
    let empty = fixture
        .library
        .update(cx, |me, cx| me.rename_voice(&anna.id, "  ", cx));
    assert!(empty.unwrap_err().contains("Give the voice a name"));
    fixture
        .library
        .update(cx, |me, cx| me.start_rename(&anna.id, cx))
        .unwrap();
    assert_eq!(state(cx, &fixture)["renaming"], json!(anna.id));
    fixture.library.update(cx, |me, cx| {
        let field = me.rename.clone();
        me.set_input(&field, "Anna at home".into(), cx);
    });
    cx.run_until_parked();
    fixture
        .library
        .update(cx, |me, cx| me.save_rename(None, cx))
        .unwrap();
    let renamed = state(cx, &fixture);
    assert_eq!(renamed["renaming"], Value::Null);
    let names: Vec<&str> = renamed["voices"]
        .as_array()
        .unwrap()
        .iter()
        .map(|voice| voice["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Anna at home", "Ben"]);
}

#[gpui_kit::test]
fn the_sheet_opens_on_a_page_and_a_dismiss_drops_the_new_clip(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    assert_eq!(state(cx, &fixture)["sheet"], Value::Null);
    fixture.library.update(cx, |me, cx| me.show(Page::List, cx));
    assert_eq!(state(cx, &fixture)["sheet"], "list");
    let path = speech_file(&fixture, "clip.wav", 6.0);
    fixture.library.update(cx, |me, cx| {
        me.show(Page::Add, cx);
        me.load_file(path, cx);
    });
    run_all(cx, &fixture);
    fixture
        .library
        .update(cx, |me, cx| me.toggle_play(Playing::NewClip, cx));
    assert_eq!(state(cx, &fixture)["playing"], "new");
    fixture.library.update(cx, |me, cx| me.dismiss(cx));
    let closed = state(cx, &fixture);
    assert_eq!(closed["sheet"], Value::Null);
    assert_eq!(closed["playing"], Value::Null);
    fixture.library.update(cx, |me, cx| me.show(Page::Add, cx));
    assert_eq!(
        state(cx, &fixture)["clip"],
        Value::Null,
        "a new add starts clean"
    );
    assert!(!state(cx, &fixture)["can_record"].as_bool().unwrap());
}

#[test]
fn the_take_line_counts_to_the_shortest_clip_then_down_to_the_longest() {
    assert_eq!(
        take_detail(Take::Recording { seconds: 2 }, None),
        "Speak now. Keep going for 3 s"
    );
    assert_eq!(
        take_detail(Take::Recording { seconds: 12 }, None),
        "Speak now. 8 s left, or press Stop"
    );
    assert_eq!(
        take_detail(Take::Idle, None),
        "Read the lines above, 5 to 20 s"
    );
    let clip = Clip {
        samples: vec![0.0; SAMPLE_RATE as usize * 7],
        cut: false,
    };
    assert_eq!(
        take_detail(Take::Idle, Some(&clip)),
        "7 s of speech, ready to save"
    );
}
