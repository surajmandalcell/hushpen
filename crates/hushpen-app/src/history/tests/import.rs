use super::*;
use std::path::Path;

fn seed_import(view: &Fixture, id: &str, name: &str, language: &str) {
    let row = Row {
        id: id.to_owned(),
        created_at: 5_000,
        kind: "import".into(),
        status: "completed".into(),
        duration_ms: 7_000,
        model_id: Some("base".into()),
        language_detected: Some(language.into()),
        final_text: Some("first part second part".into()),
        raw_text: Some("first part second part".into()),
        source_name: Some(name.to_owned()),
        ..Row::default()
    };
    let segments = [(0, 3_400, "first part"), (3_400, 7_000, "second part")]
        .into_iter()
        .enumerate()
        .map(|(idx, (start_ms, end_ms, text))| Segment {
            idx: idx as i64,
            start_ms,
            end_ms,
            text: text.to_owned(),
        })
        .collect::<Vec<_>>();
    history::save(&view.rig.storage.database, &row, &segments).unwrap();
}

fn pick(cx: &mut TestAppContext, view: &Fixture, path: &Path) {
    let path = path.to_path_buf();
    cx.simulate_new_path_selection(move |_| Some(path.clone()));
    frame(cx, view);
}

#[gpui_kit::test]
fn the_list_row_says_import_and_names_the_file(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "D", 1_000, "dictated");
    seed_import(&view, "I", "interview.mp3", "en");
    reload(cx, &view);

    let rows = state(cx, &view)["rows"].clone();
    let import = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "I")
        .unwrap();
    let meta = import["meta"].as_str().unwrap();
    assert!(meta.contains("Import"), "{meta}");
    assert!(meta.contains("interview.mp3"), "{meta}");
    let dictation = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "D")
        .unwrap();
    assert!(!dictation["meta"].as_str().unwrap().contains("Import"));
    assert_eq!(import["kind"], "import");
    assert_eq!(import["source_name"], "interview.mp3");
}

#[gpui_kit::test]
fn the_search_finds_an_import_by_its_file_name(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "D", 1_000, "dictated");
    seed_import(&view, "I", "interview.mp3", "en");
    reload(cx, &view);

    view.history
        .update(cx, |history, cx| history.set_query("interview".into(), cx));
    frame(cx, &view);

    let state = state(cx, &view);
    assert_eq!(state["count"], 1);
    assert_eq!(state["rows"][0]["id"], "I");
}

#[gpui_kit::test]
fn the_detail_lists_the_segments_with_their_times(cx: &mut TestAppContext) {
    let view = open(cx);
    seed_import(&view, "I", "interview.mp3", "es");
    reload(cx, &view);

    click(cx, &view, "history.row.0");

    let detail = state(cx, &view)["detail"].clone();
    assert_eq!(detail["kind"], "import");
    assert_eq!(detail["source_name"], "interview.mp3");
    assert_eq!(detail["language"], "es");
    assert_eq!(
        detail["segments"],
        json!([
            {"start_ms": 0, "end_ms": 3_400, "text": "first part"},
            {"start_ms": 3_400, "end_ms": 7_000, "text": "second part"},
        ])
    );
    assert!(present(cx, &view, "history.detail.segment.0"));
    assert!(present(cx, &view, "history.detail.segment.1"));
    assert!(!present(cx, &view, "history.detail.segment.2"));
    assert!(present(cx, &view, "history.detail.file"));
    assert!(!present(cx, &view, "history.detail.app"));
    assert!(!present(cx, &view, "history.detail.rule"));
}

#[gpui_kit::test]
fn a_dictation_detail_has_no_segment_rows_and_no_file_row(cx: &mut TestAppContext) {
    let view = open(cx);
    seed_with_audio(&view, "D", 1_000, "dictated");
    reload(cx, &view);

    click(cx, &view, "history.row.0");

    assert!(!present(cx, &view, "history.detail.segment.0"));
    assert!(!present(cx, &view, "history.detail.file"));
    assert!(present(cx, &view, "history.detail.app"));
}

#[gpui_kit::test]
fn an_import_cannot_be_reprocessed_and_the_note_says_why(cx: &mut TestAppContext) {
    let view = open(cx);
    seed_import(&view, "I", "interview.mp3", "en");
    reload(cx, &view);

    click(cx, &view, "history.row.0");
    assert!(present(cx, &view, "history.audio-missing"));
    let detail = state(cx, &view)["detail"].clone();
    assert_eq!(detail["audio"], false);
    assert!(
        detail["audio_note"]
            .as_str()
            .unwrap()
            .contains("imported file")
    );

    let _ = view
        .history
        .update(cx, |history, cx| history.reprocess(None, cx));
    assert!(state(cx, &view)["reprocess"].is_null());
}

#[gpui_kit::test]
fn an_import_exports_as_srt_and_json_with_its_language(cx: &mut TestAppContext) {
    let view = open(cx);
    seed_import(&view, "I", "entrevista.wav", "es");
    reload(cx, &view);
    let out = tempfile::tempdir().unwrap();

    click(cx, &view, "history.row.0");
    click(cx, &view, "history.export");
    click(cx, &view, "history.export-srt");
    pick(cx, &view, &out.path().join("talk.srt"));
    let srt = fs::read_to_string(out.path().join("talk.srt")).unwrap();
    assert!(srt.starts_with("1\n00:00:00,000 --> 00:00:03,400\nfirst part\n"));
    assert!(srt.contains("2\n00:00:03,400 --> 00:00:07,000\nsecond part"));

    if !present(cx, &view, "history.export-bar") {
        click(cx, &view, "history.export");
    }
    click(cx, &view, "history.export-json");
    pick(cx, &view, &out.path().join("talk.json"));
    let json: Value =
        serde_json::from_str(&fs::read_to_string(out.path().join("talk.json")).unwrap()).unwrap();
    assert!(json.to_string().contains("\"language_detected\":\"es\""));
}
