use super::*;
use crate::controller::testkit::{Rig, rig};
use crate::import::{Import, Parts as ImportParts};
use crate::read_aloud::{Parts as ReadParts, ReadAloud};
use gpui_kit::{AppContext as _, TestAppContext, VisualTestContext};
use hushpen_speech::{Pcm, SpeechEngine, SpeechError, Voice};
use std::sync::Arc;

struct Mute;

impl SpeechEngine for Mute {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        Ok(Vec::new())
    }

    fn speak(&self, _: &str, _: Option<&str>, _: f32) -> Result<Pcm, SpeechError> {
        Err(SpeechError::failed("mute"))
    }
}

struct Fixture {
    files: Entity<Files>,
    import: Entity<Import>,
    read: Entity<ReadAloud>,
    storage: Rc<Storage>,
    folder: tempfile::TempDir,
    /// Holds the data folder.
    _rig: Rig,
}

fn open(cx: &mut TestAppContext) -> (Fixture, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
    let rig = rig(cx, &["base"], "base");
    let storage = Rc::clone(&rig.storage);
    let idle: crate::models::Spawner = Rc::new(|_| Ok(()));
    let import = cx.new(|cx| {
        Import::new(
            ImportParts {
                storage: Rc::clone(&storage),
                models: rig.models.clone(),
                controller: rig.controller.clone(),
                run: Arc::new(|_, _, _| unreachable!("no job runs")),
                spawn: Rc::clone(&idle),
                clock: Rc::new(|| 0),
            },
            cx,
        )
    });
    let window = cx.add_empty_window();
    let read = window.update(|window, cx| {
        cx.new(|cx| {
            ReadAloud::new(
                ReadParts {
                    storage: Rc::clone(&storage),
                    system: Arc::new(Mute),
                    kokoro: Arc::new(Mute),
                    spawn: idle,
                    silent: true,
                },
                window,
                cx,
            )
        })
    });
    let files = window.update(|_, cx| {
        cx.new(|cx| Files::new(Rc::clone(&storage), import.clone(), Some(read.clone()), cx))
    });
    let fixture = Fixture {
        files,
        import,
        read,
        storage,
        folder: tempfile::tempdir().unwrap(),
        _rig: rig,
    };
    (fixture, window)
}

fn file(fixture: &Fixture, name: &str, body: &str) -> PathBuf {
    let path = fixture.folder.path().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

fn mode(cx: &mut VisualTestContext, fixture: &Fixture) -> Mode {
    fixture.files.read_with(cx, |files, _| files.mode())
}

#[gpui_kit::test]
fn the_mode_starts_on_transcribe_and_the_pick_is_kept(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    assert_eq!(mode(cx, &fixture), Mode::Transcribe);
    fixture
        .files
        .update(cx, |files, cx| files.set_mode(Mode::ReadAloud, cx))
        .unwrap();
    assert_eq!(mode(cx, &fixture), Mode::ReadAloud);
    assert_eq!(
        fixture.storage.settings.get(MODE_SETTING),
        Some(json!("readAloud"))
    );
}

#[gpui_kit::test]
fn dropped_audio_goes_in_the_queue_and_shows_transcribe(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    fixture
        .files
        .update(cx, |files, cx| files.set_mode(Mode::ReadAloud, cx))
        .unwrap();
    let audio = file(&fixture, "talk.mp3", "audio");
    fixture
        .files
        .update(cx, |files, cx| files.drop_paths(vec![audio], cx));
    assert_eq!(mode(cx, &fixture), Mode::Transcribe);
    assert_eq!(
        fixture
            .import
            .read_with(cx, |import, _| import.items().len()),
        1
    );
}

#[gpui_kit::test]
fn a_dropped_text_file_opens_in_read_aloud(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let text = file(&fixture, "notes.TXT", "Some words to read.");
    fixture
        .files
        .update(cx, |files, cx| files.drop_paths(vec![text], cx));
    assert_eq!(mode(cx, &fixture), Mode::ReadAloud);
    let chars = cx.update(|_, cx| fixture.read.read(cx).state_json(cx))["text_chars"].clone();
    assert_eq!(chars, 19);
    assert!(
        fixture
            .import
            .read_with(cx, |import, _| import.items().is_empty())
    );
}

#[gpui_kit::test]
fn text_and_audio_dropped_together_all_go_in_the_queue(cx: &mut TestAppContext) {
    let (fixture, cx) = open(cx);
    let paths = vec![
        file(&fixture, "notes.md", "words"),
        file(&fixture, "talk.wav", "audio"),
    ];
    fixture
        .files
        .update(cx, |files, cx| files.drop_paths(paths, cx));
    assert_eq!(mode(cx, &fixture), Mode::Transcribe);
}
