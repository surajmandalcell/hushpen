use super::*;
use std::cell::RefCell;

const SPOKEN: &str = "Read this aloud for me.";

fn seed_read(view: &Fixture, id: &str, seconds: u32) -> Row {
    let mut row = Row {
        id: id.to_owned(),
        created_at: 5_000,
        kind: "read".into(),
        status: "completed".into(),
        duration_ms: i64::from(seconds) * 1000,
        model_id: Some("chatterbox-turbo".into()),
        language_requested: Some("en".into()),
        final_text: Some(SPOKEN.into()),
        insert_outcome: Some("none".into()),
        source_name: Some("Narrator".into()),
        ..Row::default()
    };
    if seconds > 0 {
        let relative = format!("audio/{id}.wav");
        let path = view.rig.storage.data.root().join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, silence_wav(seconds)).unwrap();
        row.audio_path = Some(relative);
    } else {
        row.duration_ms = 3_000;
        row.audio_removed_at = Some(5_000);
    }
    history::save(&view.rig.storage.database, &row, &[]).unwrap();
    row
}

#[gpui_kit::test]
fn a_read_row_names_read_aloud_its_engine_and_its_voice(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "D", 1_000, "dictated");
    seed_read(&view, "R", 3);
    reload(cx, &view);

    let listed = state(cx, &view);
    let read = listed["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "R")
        .unwrap()
        .clone();
    assert_eq!(read["kind"], "read");
    assert_eq!(read["title"], SPOKEN);
    let meta = read["meta"].as_str().unwrap();
    assert!(
        meta.ends_with("Read aloud \u{b7} Chatterbox Turbo \u{b7} Narrator \u{b7} 3.0 s"),
        "{meta}"
    );

    view.history
        .update(cx, |history, cx| history.set_query("narrator".into(), cx));
    frame(cx, &view);
    let found = state(cx, &view);
    assert_eq!(found["count"], 1, "the voice name is searchable");
    assert_eq!(found["rows"][0]["id"], "R");
}

#[gpui_kit::test]
fn a_read_detail_plays_its_audio_and_reads_again_in_place_of_reprocess(cx: &mut TestAppContext) {
    let view = open(cx);
    seed_read(&view, "R", 3);
    reload(cx, &view);
    click(cx, &view, "history.row.0");

    assert!(present(cx, &view, "history.play"));
    assert!(present(cx, &view, "history.read-again"));
    assert!(!present(cx, &view, "history.reprocess"));
    assert_eq!(
        label(cx, &view, "history.read-again").as_deref(),
        Some("Read aloud again")
    );
    assert_eq!(
        label(cx, &view, "history.detail.engine").as_deref(),
        Some("Chatterbox Turbo")
    );
    assert_eq!(
        label(cx, &view, "history.detail.voice").as_deref(),
        Some("Narrator")
    );
    assert_eq!(
        label(cx, &view, "history.detail.language").as_deref(),
        Some("English")
    );
    for gone in ["raw", "rule", "app", "model", "status"] {
        assert!(
            !present(cx, &view, &format!("history.detail.{gone}")),
            "{gone}"
        );
    }
    let refused = view
        .history
        .update(cx, |history, cx| history.reprocess(None, cx));
    assert!(refused.unwrap_err().contains("read aloud"));
    assert!(state(cx, &view)["reprocess"].is_null());
}

#[gpui_kit::test]
fn read_aloud_again_hands_the_text_to_the_window(cx: &mut TestAppContext) {
    let view = open(cx);
    seed(&view, "D", 1_000, "dictated");
    seed_read(&view, "R", 0);
    reload(cx, &view);
    let asked: Rc<RefCell<Vec<HistoryRequest>>> = Rc::default();
    {
        let asked = Rc::clone(&asked);
        cx.update(|cx| {
            cx.subscribe(&view.history, move |_, request: &HistoryRequest, _| {
                asked.borrow_mut().push(request.clone());
            })
            .detach();
        });
    }
    click(cx, &view, "history.row.0");

    let detail = state(cx, &view)["detail"].clone();
    assert_eq!(detail["audio"], false);
    let note = detail["audio_note"].as_str().unwrap();
    assert!(note.starts_with("Audio removed."), "{note}");
    assert!(note.contains("Read aloud again"), "{note}");

    click(cx, &view, "history.read-again");
    assert_eq!(
        *asked.borrow(),
        [HistoryRequest::ReadAloud(SPOKEN.to_owned())]
    );
    let refused = view
        .history
        .update(cx, |history, cx| history.read_again(Some("D"), cx));
    assert!(refused.is_err(), "a dictation is not read again");
    assert_eq!(asked.borrow().len(), 1);
}
