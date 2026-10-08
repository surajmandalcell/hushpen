use super::*;
use std::path::Path;

fn seed_timed(view: &Fixture, id: &str, created_at: i64, text: &str, pieces: &[(i64, i64, &str)]) {
    let segments: Vec<Segment> = pieces
        .iter()
        .enumerate()
        .map(|(idx, (start_ms, end_ms, text))| Segment {
            idx: idx as i64,
            start_ms: *start_ms,
            end_ms: *end_ms,
            text: (*text).to_owned(),
        })
        .collect();
    history::save(
        &view.rig.storage.database,
        &row_of(id, created_at, text),
        &segments,
    )
    .unwrap();
}

/// Three rows: the oldest has two segments, the others one each.
fn three_rows(cx: &mut TestAppContext, view: &Fixture) {
    seed_timed(
        view,
        "A",
        1_000,
        "alpha text",
        &[(0, 900, "alpha"), (900, 2_000, "text")],
    );
    seed_timed(view, "B", 2_000, "beta text", &[(0, 2_000, "beta text")]);
    seed_timed(view, "C", 3_000, "gamma text", &[(0, 1_500, "gamma text")]);
    reload(cx, view);
}

fn pick(cx: &mut TestAppContext, view: &Fixture, path: Option<PathBuf>) -> PathBuf {
    let mut asked_in = PathBuf::new();
    cx.simulate_new_path_selection(|directory| {
        asked_in = directory.to_path_buf();
        path.clone()
    });
    frame(cx, view);
    asked_in
}

fn files_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[gpui_kit::test]
fn three_selected_rows_export_to_one_file_in_each_format(cx: &mut TestAppContext) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();

    click(cx, &view, "history.select");
    for index in 0..3 {
        click(cx, &view, &format!("history.row.{index}"));
    }
    let chosen = state(cx, &view)["selection"].clone();
    assert_eq!(chosen.as_array().unwrap().len(), 3);
    assert!(
        !present(cx, &view, "history.detail"),
        "a click chooses, it does not open"
    );

    for (done, format) in ["txt", "srt", "vtt", "json"].into_iter().enumerate() {
        click(cx, &view, &format!("history.export-{format}"));
        assert_eq!(state(cx, &view)["export_busy"], true);
        let target = out.path().join(format!("all.{format}"));
        pick(cx, &view, Some(target.clone()));

        assert_eq!(
            files_in(out.path()).len(),
            done + 1,
            "one file for each export"
        );
        let text = fs::read_to_string(&target).unwrap();
        let now = state(cx, &view);
        assert_eq!(now["export_busy"], false);
        assert_eq!(now["export"]["format"], format);
        assert_eq!(now["export"]["rows"], 3);
        assert_eq!(now["message"], Value::Null);
        assert!(
            now["notice"]
                .as_str()
                .unwrap()
                .starts_with("Exported 3 transcripts to ")
        );
        match format {
            "txt" => assert_eq!(text, "alpha text\n\nbeta text\n\ngamma text\n"),
            "srt" => {
                let numbers: Vec<&str> = text
                    .split("\n\n")
                    .filter_map(|cue| cue.lines().next())
                    .collect();
                assert_eq!(numbers, ["1", "2", "3", "4"]);
                assert!(text.contains("00:00:00,900 --> 00:00:02,000\ntext"));
                assert!(text.contains("00:00:02,000 --> 00:00:04,000\nbeta text"));
                assert!(text.contains("00:00:04,000 --> 00:00:05,500\ngamma text"));
            }
            "vtt" => {
                assert!(text.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:00.900\nalpha\n"));
                assert!(text.contains("00:00:04.000 --> 00:00:05.500\ngamma text"));
            }
            _ => {
                let parsed: Value = serde_json::from_str(&text).unwrap();
                let rows = parsed["rows"].as_array().unwrap();
                let texts: Vec<&str> = rows
                    .iter()
                    .map(|row| row["final_text"].as_str().unwrap())
                    .collect();
                assert_eq!(texts, ["alpha text", "beta text", "gamma text"]);
                assert_eq!(rows[0]["segments"].as_array().unwrap().len(), 2);
            }
        }
    }
}

#[gpui_kit::test]
fn the_next_dialog_starts_in_the_folder_of_the_last_export_and_names_the_file(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();
    click(cx, &view, "history.select");
    click(cx, &view, "history.row.0");

    click(cx, &view, "history.export-srt");
    let first = pick(cx, &view, Some(out.path().join("one.srt")));
    click(cx, &view, "history.export-srt");
    let second = pick(cx, &view, None);

    assert_ne!(
        first,
        out.path(),
        "the first dialog starts in the home folder"
    );
    assert_eq!(second, out.path());
}

#[gpui_kit::test]
fn a_cancelled_dialog_writes_nothing_and_shows_no_error(cx: &mut TestAppContext) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();
    click(cx, &view, "history.select");
    click(cx, &view, "history.row.0");

    click(cx, &view, "history.export-txt");
    pick(cx, &view, None);

    assert!(files_in(out.path()).is_empty());
    let now = state(cx, &view);
    assert_eq!(now["export_busy"], false);
    assert_eq!(now["message"], Value::Null);
    assert_eq!(now["notice"], Value::Null);
    assert_eq!(now["export"], Value::Null);
    assert!(!present(cx, &view, "history.message"));
}

#[gpui_kit::test]
fn the_detail_view_exports_its_row_and_a_name_without_an_extension_gets_one(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();

    click(cx, &view, "history.row.2");
    assert!(!present(cx, &view, "history.export-bar"));
    click(cx, &view, "history.export");
    assert!(present(cx, &view, "history.export-bar"));
    click(cx, &view, "history.export-vtt");
    pick(cx, &view, Some(out.path().join("note")));

    assert_eq!(files_in(out.path()), ["note.vtt"]);
    let text = fs::read_to_string(out.path().join("note.vtt")).unwrap();
    assert!(text.starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:00.900\nalpha\n"));
    assert!(!text.contains("beta"));
    assert!(present(cx, &view, "history.notice"));
}

#[gpui_kit::test]
fn a_row_with_no_text_is_refused_for_text_formats_and_kept_in_json(cx: &mut TestAppContext) {
    let view = open(cx);
    let mut cancelled = row_of("X", 1_000, "");
    cancelled.status = "cancelled".into();
    cancelled.final_text = None;
    cancelled.rule_text = None;
    cancelled.raw_text = None;
    history::save(&view.rig.storage.database, &cancelled, &[]).unwrap();
    reload(cx, &view);
    let out = tempfile::tempdir().unwrap();

    click(cx, &view, "history.select");
    click(cx, &view, "history.row.0");
    click(cx, &view, "history.export-srt");

    let now = state(cx, &view);
    assert_eq!(now["export_busy"], false, "no dialog opened");
    assert_eq!(now["message"], "This transcript has no text to export.");

    click(cx, &view, "history.export-json");
    pick(cx, &view, Some(out.path().join("x.json")));
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(out.path().join("x.json")).unwrap()).unwrap();
    assert_eq!(parsed["rows"][0]["status"], "cancelled");
    assert_eq!(parsed["rows"][0]["final_text"], Value::Null);
}

#[gpui_kit::test]
fn an_unwritable_path_shows_an_error_and_leaves_the_selection(cx: &mut TestAppContext) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();
    click(cx, &view, "history.select");
    click(cx, &view, "history.row.1");

    click(cx, &view, "history.export-txt");
    pick(
        cx,
        &view,
        Some(out.path().join("missing-folder").join("a.txt")),
    );

    let now = state(cx, &view);
    assert!(
        now["message"]
            .as_str()
            .unwrap()
            .starts_with("The file could not be written to "),
        "{now}"
    );
    assert_eq!(now["selection"].as_array().unwrap().len(), 1);
    assert!(present(cx, &view, "history.message"));
}

#[gpui_kit::test]
fn exporting_with_nothing_chosen_is_refused_and_leaving_select_mode_forgets_the_choice(
    cx: &mut TestAppContext,
) {
    let view = open(cx);
    three_rows(cx, &view);

    click(cx, &view, "history.select");
    click(cx, &view, "history.row.0");
    click(cx, &view, "history.row.0");
    assert_eq!(state(cx, &view)["selection"], json!([]));
    click(cx, &view, "history.export-txt");
    assert_eq!(state(cx, &view)["export_busy"], false);

    click(cx, &view, "history.select-all");
    assert_eq!(state(cx, &view)["selection"].as_array().unwrap().len(), 3);
    click(cx, &view, "history.select");
    let now = state(cx, &view);
    assert_eq!(now["selecting"], false);
    assert_eq!(now["selection"], json!([]));
    assert!(!present(cx, &view, "history.select-bar"));
}

#[gpui_kit::test]
fn deleting_a_selected_row_takes_it_out_of_the_selection(cx: &mut TestAppContext) {
    let view = open(cx);
    three_rows(cx, &view);
    view.history.update(cx, |history, cx| {
        history.select_only(vec!["A".into(), "B".into()], cx);
        history.delete(Some("A"), cx).unwrap();
    });
    frame(cx, &view);

    assert_eq!(state(cx, &view)["selection"], json!(["B"]));
}

#[cfg(feature = "test-automation")]
#[gpui_kit::test]
fn a_path_from_the_test_hook_is_used_with_no_dialog(cx: &mut TestAppContext) {
    let view = open(cx);
    three_rows(cx, &view);
    let out = tempfile::tempdir().unwrap();
    hushpen_testhook::dialogs::answer_next_dialog(vec![out.path().join("inj.vtt")]);

    view.history.update(cx, |history, cx| {
        history.select_only(vec!["A".into(), "B".into(), "C".into()], cx);
        history
            .export(hushpen_core::export::Format::Vtt, None, cx)
            .unwrap();
    });
    frame(cx, &view);

    assert_eq!(files_in(out.path()), ["inj.vtt"]);
    assert_eq!(state(cx, &view)["export"]["rows"], 3);
}
