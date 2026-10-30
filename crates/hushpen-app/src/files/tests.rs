use super::*;
use crate::controller::testkit::{Rig, rig};
use crate::import::{Import, Parts as ImportParts};
use crate::read_aloud::{Parts as ReadParts, ReadAloud};
use gpui_kit::test::TestWindowExt as _;
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

fn frame(cx: &mut TestAppContext, handle: gpui_kit::AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

/// The Manual view in Read aloud, in a window of the app's size.
fn read_aloud_window(cx: &mut TestAppContext) -> (Fixture, gpui_kit::AnyWindowHandle) {
    use crate::shell::Shell;
    use crate::theme::space;
    use crate::views::View;
    use gpui_kit::{Bounds, Point, WindowBounds, WindowOptions, px, size};

    let (fixture, _) = open(cx);
    fixture
        .files
        .update(cx, |files, cx| files.set_mode(Mode::ReadAloud, cx))
        .unwrap();
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("open test window")
    });
    shell.update(cx, |shell, cx| {
        shell.attach_files(fixture.files.clone(), cx);
        shell.select(View::Manual, cx);
    });
    frame(cx, handle);
    (fixture, handle)
}

#[gpui_kit::test]
fn a_read_aloud_menu_floats_over_the_page_and_esc_closes_it(cx: &mut TestAppContext) {
    use crate::theme::space;
    use gpui_kit::px;

    let (_fixture, handle) = read_aloud_window(cx);
    cx.update_window(handle, |_, window, cx| window.click("read.format", cx))
        .unwrap();
    frame(cx, handle);
    let (row, option) = cx
        .update_window(handle, |_, window, _| {
            (
                window.find("read.format.row").bounds(),
                window.find("read.format.option.0"),
            )
        })
        .unwrap();
    assert!(option.visible());
    assert_eq!(option.selected(), Some(true));
    assert!(
        option.bounds().bottom() > row.top()
            && option.bounds().bottom() <= px(space::WINDOW_HEIGHT - space::SM),
        "the last row has no room under it, so the menu moves up into the window"
    );

    cx.update_window(handle, |_, window, cx| window.press("escape", cx))
        .unwrap();
    frame(cx, handle);
    let (open, focused) = cx
        .update_window(handle, |_, window, _| {
            (
                window.try_find("read.format.list").is_some(),
                window.find("read.format").focused(),
            )
        })
        .unwrap();
    assert!(!open);
    assert_eq!(focused, Some(true));
}

/// The speed reads left of its track, and the slider gives the same words to the accessibility
/// tree, at rest, after a press, and after an arrow key.
#[gpui_kit::test]
fn the_speed_slider_reads_its_value_beside_the_track(cx: &mut TestAppContext) {
    let (_fixture, handle) = read_aloud_window(cx);
    let reading = |cx: &mut TestAppContext| {
        cx.update_window(handle, |_, window, _| {
            let shown = window.find("read.speed.value").label().map(str::to_owned);
            let value = window.find("read.speed").value().map(str::to_owned);
            assert_eq!(
                shown, value,
                "the reading and the accessibility value agree"
            );
            shown
        })
        .unwrap()
    };
    assert_eq!(reading(cx).as_deref(), Some("1.00\u{d7}"));
    cx.update_window(handle, |_, window, cx| window.click("read.speed", cx))
        .unwrap();
    frame(cx, handle);
    assert_eq!(
        reading(cx).as_deref(),
        Some("1.25\u{d7}"),
        "a press in the middle"
    );
    cx.update_window(handle, |_, window, cx| window.press("right", cx))
        .unwrap();
    frame(cx, handle);
    assert_eq!(
        reading(cx).as_deref(),
        Some("1.35\u{d7}"),
        "one step to the right"
    );
}
