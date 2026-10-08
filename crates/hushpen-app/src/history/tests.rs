use super::*;
use crate::controller::testkit::{Rig, outcome_text, rig, say, settle};
use crate::shell::Shell;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Point, ScrollDelta, TestAppContext, WindowBounds,
    WindowOptions, point, px, size,
};
use hushpen_store::history::Segment;

struct Fixture {
    handle: AnyWindowHandle,
    history: Entity<History>,
    rig: Rig,
}

fn open(cx: &mut TestAppContext) -> Fixture {
    let rig = rig(cx, &["base"], "base");
    cx.update(gpui_kit::init);
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
    let history = {
        let storage = Rc::clone(&rig.storage);
        let controller = rig.controller.clone();
        cx.update_window(handle, move |_, window, cx| {
            cx.new(|cx| History::new(storage, controller, window, cx))
        })
        .unwrap()
    };
    // The device poll of the microphone ticks every 2 s, and its thread would break the test
    // clock, so the undo window of a test is shorter than that.
    history.update(cx, |history, _| {
        history.set_undo_window(Duration::from_millis(1_500))
    });
    shell.update(cx, |shell, cx| {
        shell.attach_history(history.clone(), cx);
        shell.select(View::History, cx);
    });
    let view = Fixture {
        handle,
        history,
        rig,
    };
    frame(cx, &view);
    view
}

fn frame(cx: &mut TestAppContext, view: &Fixture) {
    cx.run_until_parked();
    cx.update_window(view.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, view: &Fixture, id: &str) {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(view.handle, move |_, window, cx| window.click(id, cx))
        .unwrap();
    frame(cx, view);
}

fn type_text(cx: &mut TestAppContext, view: &Fixture, text: &str) {
    cx.update_window(view.handle, |_, window, cx| window.input(text, cx))
        .unwrap();
    frame(cx, view);
}

fn present(cx: &mut TestAppContext, view: &Fixture, id: &str) -> bool {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(view.handle, move |_, window, _| {
        window.try_find(id).is_some()
    })
    .unwrap()
}

fn row_count(cx: &mut TestAppContext, view: &Fixture) -> usize {
    (0..200)
        .take_while(|index| present(cx, view, &format!("history.row.{index}")))
        .count()
}

fn names(cx: &mut TestAppContext, view: &Fixture) -> Vec<String> {
    let state = cx.update(|cx| view.history.read(cx).state_json(cx));
    state["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["title"].as_str().unwrap().to_owned())
        .collect()
}

fn row_of(id: &str, created_at: i64, text: &str) -> Row {
    Row {
        id: id.to_owned(),
        created_at,
        kind: "dictation".into(),
        status: "completed".into(),
        duration_ms: 2_000,
        model_id: Some("base".into()),
        language_detected: Some("en".into()),
        raw_text: Some(format!("raw {text}")),
        rule_text: Some(format!("rule {text}")),
        final_text: Some(text.to_owned()),
        insert_outcome: Some("pasted".into()),
        target_app: Some("GtkTarget".into()),
        ..Row::default()
    }
}

fn seed(view: &Fixture, id: &str, created_at: i64, text: &str) -> Row {
    let row = row_of(id, created_at, text);
    history::save(&view.rig.storage.database, &row, &[]).unwrap();
    row
}

fn seed_with_audio(view: &Fixture, id: &str, created_at: i64, text: &str) -> (Row, PathBuf) {
    let mut row = row_of(id, created_at, text);
    let relative = format!("audio/{id}.wav");
    let path = view.rig.storage.data.root().join(&relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"wav").unwrap();
    row.audio_path = Some(relative);
    history::save(
        &view.rig.storage.database,
        &row,
        &[Segment {
            idx: 0,
            start_ms: 0,
            end_ms: 900,
            text: text.to_owned(),
        }],
    )
    .unwrap();
    (row, path)
}

fn reload(cx: &mut TestAppContext, view: &Fixture) {
    view.history.update(cx, |history, cx| history.reload(cx));
    frame(cx, view);
}

fn state(cx: &mut TestAppContext, view: &Fixture) -> Value {
    cx.update(|cx| view.history.read(cx).state_json(cx))
}

#[gpui_kit::test]
fn an_empty_history_shows_the_empty_state_and_no_rows(cx: &mut TestAppContext) {
    let view = open(cx);

    assert!(present(cx, &view, "history.empty"));
    assert!(present(cx, &view, "history.search"));
    assert!(!present(cx, &view, "history.row.0"));
    assert!(!present(cx, &view, "history.no-results"));
}

#[gpui_kit::test]
fn the_list_is_newest_first_and_paged_by_show_more(cx: &mut TestAppContext) {
    let view = open(cx);
    for index in 0..120 {
        seed(
            &view,
            &format!("R{index:03}"),
            1_000 + index,
            &format!("note {index}"),
        );
    }
    reload(cx, &view);

    assert_eq!(names(cx, &view).len(), PAGE);
    assert_eq!(names(cx, &view)[0], "note 119");
    assert!(present(cx, &view, "history.more"));
    let drawn = row_count(cx, &view);
    assert!(
        0 < drawn && drawn < PAGE / 2,
        "only the rows near the screen are drawn, not {drawn}"
    );
    assert!(present(cx, &view, "history.row.0"));
    assert!(!present(cx, &view, &format!("history.row.{PAGE}")));

    view.history.update(cx, |history, cx| history.load_more(cx));
    frame(cx, &view);
    assert_eq!(names(cx, &view).len(), 2 * PAGE);
    let titles = names(cx, &view);
    assert_eq!(titles[PAGE], "note 69");
    view.history.update(cx, |history, cx| history.load_more(cx));
    frame(cx, &view);
    assert_eq!(names(cx, &view).len(), 120);
    assert!(!present(cx, &view, "history.more"));
    let titles = names(cx, &view);
    assert_eq!(titles.len(), 120);
    assert_eq!(titles[119], "note 0");
}

#[gpui_kit::test]
fn scrolling_to_the_end_of_the_page_loads_the_next_page(cx: &mut TestAppContext) {
    let view = open(cx);
    for index in 0..120 {
        seed(
            &view,
            &format!("R{index:03}"),
            1_000 + index,
            &format!("note {index}"),
        );
    }
    reload(cx, &view);
    assert_eq!(names(cx, &view).len(), PAGE);

    for _ in 0..4 {
        cx.update_window(view.handle, |_, window, cx| {
            window.scroll(
                "content.scroll",
                ScrollDelta::Pixels(point(px(0.0), px(-20_000.0))),
                cx,
            )
        })
        .unwrap();
        frame(cx, &view);
        frame(cx, &view);
    }

    assert_eq!(names(cx, &view).len(), 120);
    assert_eq!(names(cx, &view)[PAGE], "note 69");
    assert!(present(cx, &view, "history.row.119"), "the end is drawn");
    assert!(!present(cx, &view, "history.row.0"), "the top is not");
}

#[gpui_kit::test]
fn the_arrow_keys_move_the_focus_to_a_row_that_is_not_drawn_yet(cx: &mut TestAppContext) {
    let view = open(cx);
    for index in 0..30 {
        seed(
            &view,
            &format!("R{index:03}"),
            1_000 + index,
            &format!("note {index}"),
        );
    }
    reload(cx, &view);
    let drawn_end = state(cx, &view)["drawn"][1].as_u64().unwrap() as usize;
    assert!(drawn_end < 30, "the last rows are not drawn yet");
    let last = view
        .history
        .read_with(cx, |history, _| history.row_focus[drawn_end - 1].clone());
    cx.update_window(view.handle, |_, window, cx| window.focus(&last, cx))
        .unwrap();
    frame(cx, &view);

    for _ in 0..3 {
        cx.update_window(view.handle, |_, window, cx| window.press("down", cx))
            .unwrap();
        frame(cx, &view);
    }

    let target = view
        .history
        .read_with(cx, |history, _| history.row_focus[drawn_end + 2].clone());
    cx.update_window(view.handle, move |_, window, _| {
        assert!(
            target.is_focused(window),
            "the focus moved three rows down, past the rows that were drawn"
        );
    })
    .unwrap();
    let range = state(cx, &view)["drawn"].clone();
    assert!(
        range[1].as_u64().unwrap() as usize > drawn_end + 2,
        "the list drew the rows that the focus reached: {range}"
    );
}

#[gpui_kit::test]
fn search_finds_substrings_ignores_case_and_accents_and_shows_no_results(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "A", 1_000, "Caf\u{e9} meeting at noon");
    seed(&view, "B", 2_000, "the lighthouse keeper");
    seed(&view, "C", 3_000, "ok then");
    reload(cx, &view);
    assert_eq!(row_count(cx, &view), 3);

    click(cx, &view, "history.search");
    for (query, expected) in [
        ("cafe", "Caf\u{e9} meeting at noon"),
        ("CAF\u{c9}", "Caf\u{e9} meeting at noon"),
        ("ghthou", "the lighthouse keeper"),
        ("ok", "ok then"),
    ] {
        view.history.update(cx, |history, cx| {
            history.set_query(query.to_owned(), cx);
        });
        frame(cx, &view);
        assert_eq!(names(cx, &view), [expected], "{query}");
    }

    view.history
        .update(cx, |history, cx| history.set_query("zzqqxx".into(), cx));
    frame(cx, &view);
    assert!(present(cx, &view, "history.no-results"));
    assert_eq!(row_count(cx, &view), 0);
    assert!(!present(cx, &view, "history.empty"));

    view.history
        .update(cx, |history, cx| history.set_query(String::new(), cx));
    frame(cx, &view);
    assert_eq!(row_count(cx, &view), 3);
}

#[gpui_kit::test]
fn typing_in_the_search_box_filters_the_list(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "B", 2_000, "the lighthouse keeper");
    seed(&view, "C", 3_000, "ok then");
    reload(cx, &view);

    click(cx, &view, "history.search");
    type_text(cx, &view, "light");

    assert_eq!(names(cx, &view), ["the lighthouse keeper"]);
    assert_eq!(state(cx, &view)["query"], "light");
}

#[gpui_kit::test]
fn a_cancelled_row_is_labelled_not_transcribed_and_a_failed_row_shows_as_failed(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    let base = Row {
        kind: "dictation".into(),
        duration_ms: 1_000,
        ..Row::default()
    };
    history::save(
        &view.rig.storage.database,
        &Row {
            id: "CANCELLED".into(),
            created_at: 1_000,
            status: "cancelled".into(),
            ..base.clone()
        },
        &[],
    )
    .unwrap();
    history::save(
        &view.rig.storage.database,
        &Row {
            id: "FAILED".into(),
            created_at: 2_000,
            status: "failed".into(),
            error_code: Some("ENGINE_NO_SPEECH".into()),
            ..base
        },
        &[],
    )
    .unwrap();
    reload(cx, &view);

    let titles = names(cx, &view);
    assert!(titles[0].starts_with("Failed"), "{titles:?}");
    assert_eq!(titles[1], "Not transcribed");
}

#[gpui_kit::test]
fn the_detail_view_shows_the_fields_of_the_row_and_back_returns_to_the_list(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    seed(&view, "A", 1_000, "hello there");
    reload(cx, &view);

    click(cx, &view, "history.row.0");

    assert!(present(cx, &view, "history.detail"));
    assert!(!present(cx, &view, "history.row.0"));
    let detail = state(cx, &view)["detail"].clone();
    assert_eq!(detail["raw_text"], "raw hello there");
    assert_eq!(detail["rule_text"], "rule hello there");
    assert_eq!(detail["final_text"], "hello there");
    assert_eq!(detail["target_app"], "GtkTarget");
    assert_eq!(detail["duration_ms"], 2_000);
    assert_eq!(detail["model_id"], "base");
    assert_eq!(detail["language"], "en");
    for field in [
        "raw", "rule", "final", "app", "duration", "model", "language",
    ] {
        assert!(
            present(cx, &view, &format!("history.detail.{field}")),
            "{field}"
        );
    }
    assert!(!present(cx, &view, "history.detail.llm"));

    click(cx, &view, "history.back");
    assert!(present(cx, &view, "history.row.0"));
}

#[gpui_kit::test]
fn copy_puts_the_final_text_on_the_clipboard(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "A", 1_000, "copy me");
    reload(cx, &view);
    crate::controller::testkit::put_on_clipboard(cx, "OLD");

    click(cx, &view, "history.row.0");
    click(cx, &view, "history.copy");

    assert_eq!(
        crate::controller::testkit::clipboard(cx).as_deref(),
        Some("copy me")
    );
}

#[gpui_kit::test]
fn delete_hides_the_row_at_once_and_undo_brings_back_the_row_segments_and_audio(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    let (row, wav) = seed_with_audio(&view, "A", 1_000, "to be undone");
    reload(cx, &view);

    click(cx, &view, "history.row.0");
    click(cx, &view, "history.delete");

    assert!(present(cx, &view, "history.undo"));
    assert!(present(cx, &view, "history.empty"));
    assert_eq!(history::get(&view.rig.storage.database, "A").unwrap(), None);

    cx.executor().advance_clock(Duration::from_secs(1));
    frame(cx, &view);
    click(cx, &view, "history.undo");

    assert_eq!(
        history::get(&view.rig.storage.database, "A").unwrap(),
        Some(row)
    );
    assert_eq!(
        history::segments(&view.rig.storage.database, "A")
            .unwrap()
            .len(),
        1
    );
    assert!(wav.is_file(), "the audio is back where the row expects it");
    assert!(!present(cx, &view, "history.undo"));
    assert_eq!(row_count(cx, &view), 1);
}

#[gpui_kit::test]
fn a_delete_that_is_not_undone_removes_the_row_segments_and_audio_when_the_window_ends(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    let (_, wav) = seed_with_audio(&view, "A", 1_000, "lighthouse");
    reload(cx, &view);

    view.history
        .update(cx, |history, cx| history.delete(Some("A"), cx))
        .unwrap();
    assert!(!wav.exists(), "the audio is set aside at once");
    cx.executor().advance_clock(Duration::from_millis(1_600));
    frame(cx, &view);

    assert!(!present(cx, &view, "history.undo"));
    assert!(
        history::segments(&view.rig.storage.database, "A")
            .unwrap()
            .is_empty()
    );
    assert!(
        view.rig
            .storage
            .data
            .trash_dir()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
    assert!(!wav.exists());
    let refused = view.history.update(cx, |history, cx| history.undo(cx));
    assert!(refused.is_err());
}

#[gpui_kit::test]
fn quitting_inside_the_undo_window_removes_the_audio_and_the_next_start_sweeps_leftovers(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    let (_, wav) = seed_with_audio(&view, "A", 1_000, "going away");
    view.history
        .update(cx, |history, cx| history.delete(Some("A"), cx))
        .unwrap();
    view.history.update(cx, |history, _| history.finish_undo());
    assert!(!wav.exists());
    assert!(
        view.rig
            .storage
            .data
            .trash_dir()
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );

    let leftover = view.rig.storage.data.trash_dir().join("LEFT.wav");
    fs::write(&leftover, b"wav").unwrap();
    sweep_trash(&view.rig.storage);
    assert!(!leftover.exists());
}

#[gpui_kit::test]
fn a_second_delete_ends_the_window_of_the_first(cx: &mut TestAppContext) {
    let view = open(cx);
    let (_, first) = seed_with_audio(&view, "A", 1_000, "first");
    let (_, second) = seed_with_audio(&view, "B", 2_000, "second");
    reload(cx, &view);

    view.history
        .update(cx, |history, cx| history.delete(Some("A"), cx))
        .unwrap();
    view.history
        .update(cx, |history, cx| history.delete(Some("B"), cx))
        .unwrap();
    view.history
        .update(cx, |history, cx| history.undo(cx))
        .unwrap();

    assert!(!first.exists());
    assert!(second.is_file());
    assert!(
        history::get(&view.rig.storage.database, "A")
            .unwrap()
            .is_none()
    );
    assert!(
        history::get(&view.rig.storage.database, "B")
            .unwrap()
            .is_some()
    );
}

#[gpui_kit::test]
fn clear_all_asks_first_declining_keeps_everything_and_confirming_removes_rows_and_audio(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    let (_, wav) = seed_with_audio(&view, "A", 1_000, "one");
    seed(&view, "B", 2_000, "two");
    reload(cx, &view);

    click(cx, &view, "history.clear");
    assert!(present(cx, &view, "history.confirm"));
    assert_eq!(history::count(&view.rig.storage.database).unwrap(), 2);
    click(cx, &view, "history.clear-cancel");
    assert!(!present(cx, &view, "history.confirm"));
    assert_eq!(history::count(&view.rig.storage.database).unwrap(), 2);
    assert!(wav.is_file());

    let early = view
        .history
        .update(cx, |history, cx| history.confirm_clear(cx));
    assert!(early.is_err(), "no confirmation, no clearing");
    assert_eq!(history::count(&view.rig.storage.database).unwrap(), 2);

    click(cx, &view, "history.clear");
    click(cx, &view, "history.clear-confirm");
    assert_eq!(history::count(&view.rig.storage.database).unwrap(), 0);
    assert!(!wav.exists());
    assert!(present(cx, &view, "history.empty"));
}

#[gpui_kit::test]
fn a_new_dictation_shows_at_the_top_and_reprocess_changes_the_row_text(cx: &mut TestAppContext) {
    let view = open(cx);
    say(&view.rig, outcome_text("fresh words", "en"));
    crate::controller::testkit::send_at(
        cx,
        &view.rig,
        1_000,
        hushpen_core::dictation::AppEvent::HoldDown,
    )
    .unwrap();
    crate::controller::testkit::send_at(
        cx,
        &view.rig,
        3_000,
        hushpen_core::dictation::AppEvent::HoldUp,
    )
    .unwrap();
    settle(cx, &view.rig);
    frame(cx, &view);
    assert_eq!(names(cx, &view), ["fresh words"]);
    let id = state(cx, &view)["rows"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    say(&view.rig, outcome_text("better words", "en"));
    click(cx, &view, "history.row.0");
    click(cx, &view, "history.reprocess");
    settle(cx, &view.rig);
    frame(cx, &view);

    assert_eq!(state(cx, &view)["detail"]["final_text"], "better words");
    assert_eq!(state(cx, &view)["detail"]["id"], id.as_str());
    assert_eq!(history::count(&view.rig.storage.database).unwrap(), 1);
    let search = history::page(&view.rig.storage.database, "fresh", None, 50).unwrap();
    assert!(search.rows.is_empty(), "the old text is no longer found");
}

#[gpui_kit::test]
fn reprocess_of_a_row_without_audio_is_refused_with_a_reason(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "A", 1_000, "no audio");
    reload(cx, &view);
    click(cx, &view, "history.row.0");

    assert!(present(cx, &view, "history.audio-missing"));
    let refused = view
        .history
        .update(cx, |history, cx| history.reprocess(None, cx));
    assert!(refused.unwrap_err().contains("audio"));
    assert!(present(cx, &view, "history.message"));
}

#[gpui_kit::test]
fn the_panel_and_its_controls_fit_the_content_width(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "A", 1_000, &"a long line of words ".repeat(30));
    reload(cx, &view);

    let (list, clear, row) = cx
        .update_window(view.handle, |_, window, _| {
            (
                window.find("history.list").bounds(),
                window.find("history.clear").bounds(),
                window.find("history.row.0").bounds(),
            )
        })
        .unwrap();

    assert!(f32::from(list.size.width) <= space::CONTENT_MAX_WIDTH);
    assert!(clear.right() <= list.right());
    assert!(row.right() <= list.right());
    assert!(
        f32::from(row.size.height) < 80.0,
        "a long text stays on one line"
    );
}
