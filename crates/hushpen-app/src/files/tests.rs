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
                    turbo: Arc::new(Mute),
                    nano: Arc::new(Mute),
                    library: None,
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
    let (fixture, handle, _) = read_aloud_shell(cx);
    (fixture, handle)
}

/// The same, with the shell of the window.
fn read_aloud_shell(
    cx: &mut TestAppContext,
) -> (
    Fixture,
    gpui_kit::AnyWindowHandle,
    Entity<crate::shell::Shell>,
) {
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
    (fixture, handle, shell)
}

#[gpui_kit::test]
fn the_mode_control_sits_in_the_title_bar_centered_on_the_pane(cx: &mut TestAppContext) {
    let (_fixture, handle) = read_aloud_window(cx);
    let (control, pane, title, page) = cx
        .update_window(handle, |_, window, _| {
            (
                window.find("files.mode").bounds(),
                window.find("toolbar.drag").bounds(),
                window.find("view.title").bounds(),
                window.find("files.view").bounds(),
            )
        })
        .unwrap();
    let center_x = f32::from(control.origin.x) + f32::from(control.size.width) / 2.0;
    let pane_x = f32::from(pane.origin.x) + f32::from(pane.size.width) / 2.0;
    assert!((center_x - pane_x).abs() < 0.5, "{center_x} vs {pane_x}");
    let center_y = f32::from(control.origin.y) + f32::from(control.size.height) / 2.0;
    assert!(
        (center_y - crate::theme::size::CHROME_CENTER).abs() < 0.5,
        "on the chrome line: {center_y}"
    );
    assert!(control.left() > title.right(), "right of the view title");
    assert!(
        page.top() >= control.bottom(),
        "the page starts under the title bar band"
    );
}

#[gpui_kit::test]
fn left_and_right_in_the_title_bar_control_switch_the_mode(cx: &mut TestAppContext) {
    let (fixture, handle) = read_aloud_window(cx);
    cx.update_window(handle, |_, window, cx| window.click("files.mode", cx))
        .unwrap();
    for (key, expected) in [("left", Mode::Transcribe), ("right", Mode::ReadAloud)] {
        cx.update_window(handle, |_, window, cx| window.press(key, cx))
            .unwrap();
        frame(cx, handle);
        assert_eq!(
            fixture.files.read_with(cx, |files, _| files.mode()),
            expected,
            "{key}"
        );
    }
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

#[gpui_kit::test]
fn read_aloud_again_in_history_reads_the_text_in_read_aloud(cx: &mut TestAppContext) {
    use crate::history::History;
    use crate::read_aloud::ReadRequest;
    use crate::views::View;
    use hushpen_store::history::{self, Row};

    let (fixture, handle, shell) = read_aloud_shell(cx);
    fixture
        .files
        .update(cx, |files, cx| files.set_mode(Mode::Transcribe, cx))
        .unwrap();
    let read_row = |id: &str, text: &str| Row {
        id: id.to_owned(),
        created_at: 1_000,
        kind: "read".to_owned(),
        status: "completed".to_owned(),
        duration_ms: 2_000,
        model_id: Some("system".to_owned()),
        final_text: Some(text.to_owned()),
        insert_outcome: Some("none".to_owned()),
        source_name: Some("Default".to_owned()),
        ..Row::default()
    };
    history::save(
        &fixture.storage.database,
        &read_row("R", "Read me again."),
        &[],
    )
    .unwrap();
    let history = {
        let storage = Rc::clone(&fixture.storage);
        let controller = fixture._rig.controller.clone();
        cx.update_window(handle, move |_, window, cx| {
            cx.new(|cx| History::new(storage, controller, window, cx))
        })
        .unwrap()
    };
    shell.update(cx, |shell, cx| {
        shell.attach_history(history.clone(), cx);
        shell.select(View::History, cx);
    });
    history
        .update(cx, |history, cx| history.open("R", cx))
        .unwrap();
    frame(cx, handle);

    let button = gpui_kit::SharedString::from("history.read-again");
    cx.update_window(handle, move |_, window, cx| window.click(button, cx))
        .unwrap();
    frame(cx, handle);
    assert_eq!(shell.read_with(cx, |shell, _| shell.active()), View::Manual);
    assert_eq!(
        fixture.files.read_with(cx, |files, _| files.mode()),
        Mode::ReadAloud
    );
    let read = cx.update(|cx| fixture.read.read(cx).state_json(cx));
    assert_eq!(read["text_chars"], 14);
    assert_eq!(read["playing"], true, "the text plays at once");

    // A read that Read aloud saved shows in History without a dictation in between.
    history::save(
        &fixture.storage.database,
        &read_row("S", "Saved next."),
        &[],
    )
    .unwrap();
    fixture.read.update(cx, |_, cx| cx.emit(ReadRequest::Saved));
    frame(cx, handle);
    let listed = history.read_with(cx, |history, _| history.rows().len());
    assert_eq!(listed, 2);
}
