use hushpen_store::db::Database;
use hushpen_store::history::{self, Reprocessed, Row, Segment};
use std::path::Path;
use std::time::Instant;

fn open(path: &Path) -> Database {
    Database::open(&path.join("history").join("history.db")).unwrap()
}

fn dictation(id: &str, created_at: i64, text: &str) -> Row {
    Row {
        id: id.to_owned(),
        created_at,
        kind: "dictation".to_owned(),
        status: "completed".to_owned(),
        duration_ms: 2_000,
        model_id: Some("tiny.en".to_owned()),
        raw_text: Some(text.to_owned()),
        rule_text: Some(text.to_owned()),
        final_text: Some(text.to_owned()),
        insert_outcome: Some("pasted".to_owned()),
        target_app: Some("Target-gtk.py".to_owned()),
        audio_path: Some(format!("audio/{id}.wav")),
        ..Row::default()
    }
}

fn found(db: &Database, query: &str) -> Vec<String> {
    history::page(db, query, None, 50)
        .unwrap()
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect()
}

#[test]
fn a_saved_row_comes_back_with_every_field_and_its_segments() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let mut row = dictation("A1", 10, "the quick brown fox");
    row.prompt = Some("Zyxtrel".to_owned());
    row.language_requested = Some("auto".to_owned());
    row.language_detected = Some("en".to_owned());
    let segments = [
        Segment {
            idx: 0,
            start_ms: 0,
            end_ms: 900,
            text: "the quick".to_owned(),
        },
        Segment {
            idx: 1,
            start_ms: 900,
            end_ms: 1_800,
            text: "brown fox".to_owned(),
        },
    ];

    history::save(&db, &row, &segments).unwrap();

    assert_eq!(history::get(&db, "A1").unwrap(), Some(row));
    assert_eq!(history::segments(&db, "A1").unwrap(), segments);
    assert_eq!(history::count(&db).unwrap(), 1);
    assert_eq!(history::get(&db, "missing").unwrap(), None);
}

#[test]
fn the_list_is_newest_first_and_pages_continue_where_the_last_one_ended() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    for n in 0..120 {
        history::save(&db, &dictation(&format!("R{n:03}"), n, "words"), &[]).unwrap();
    }

    let first = history::page(&db, "", None, 50).unwrap();
    assert_eq!(first.rows.len(), 50);
    assert_eq!(first.rows[0].id, "R119");
    let second = history::page(&db, "", first.next.as_ref(), 50).unwrap();
    assert_eq!(second.rows[0].id, "R069");
    let third = history::page(&db, "", second.next.as_ref(), 50).unwrap();
    assert_eq!(third.rows.len(), 20);
    assert_eq!(third.rows[19].id, "R000");
    assert!(third.next.is_none(), "the last page has no continuation");
}

#[test]
fn rows_with_the_same_time_keep_a_stable_order_across_pages() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    for n in 0..7 {
        history::save(&db, &dictation(&format!("S{n}"), 5, "same"), &[]).unwrap();
    }
    let first = history::page(&db, "", None, 3).unwrap();
    let second = history::page(&db, "", first.next.as_ref(), 3).unwrap();
    let third = history::page(&db, "", second.next.as_ref(), 3).unwrap();
    let ids: Vec<_> = [first, second, third]
        .into_iter()
        .flat_map(|page| page.rows)
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, ["S6", "S5", "S4", "S3", "S2", "S1", "S0"]);
}

#[test]
fn search_matches_substrings_ignores_case_and_diacritics_and_handles_short_queries() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    history::save(&db, &dictation("cafe", 1, "Café meeting at noon"), &[]).unwrap();
    history::save(&db, &dictation("light", 2, "the lighthouse keeper"), &[]).unwrap();
    history::save(&db, &dictation("ok", 3, "ok then"), &[]).unwrap();

    assert_eq!(found(&db, "cafe"), ["cafe"]);
    assert_eq!(found(&db, "CAFÉ"), ["cafe"]);
    assert_eq!(found(&db, "ghthou"), ["light"]);
    assert_eq!(found(&db, "ok"), ["ok"]);
    assert_eq!(found(&db, "ok then"), ["ok"]);
    assert!(found(&db, "zzqqxx").is_empty());
    assert_eq!(found(&db, "  ").len(), 3, "a blank query lists everything");
}

#[test]
fn a_query_with_quotes_and_wildcards_is_plain_text() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    history::save(&db, &dictation("q", 1, "she said \"stop\" now"), &[]).unwrap();
    history::save(&db, &dictation("p", 2, "100% sure"), &[]).unwrap();
    history::save(&db, &dictation("u", 3, "an under_score"), &[]).unwrap();

    assert_eq!(found(&db, "\"stop\""), ["q"]);
    assert_eq!(found(&db, "%"), ["p"]);
    assert_eq!(found(&db, "_"), ["u"]);
    assert!(found(&db, "NEAR(").is_empty());
}

#[test]
fn search_pages_follow_the_same_cursor() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    for n in 0..8 {
        history::save(&db, &dictation(&format!("M{n}"), n, "lighthouse"), &[]).unwrap();
    }
    history::save(&db, &dictation("X", 100, "other"), &[]).unwrap();

    let first = history::page(&db, "lighthouse", None, 5).unwrap();
    let second = history::page(&db, "lighthouse", first.next.as_ref(), 5).unwrap();
    assert_eq!(first.rows.len(), 5);
    assert_eq!(first.rows[0].id, "M7");
    assert_eq!(second.rows.len(), 3);
    assert_eq!(second.rows[2].id, "M0");
    assert!(second.next.is_none());
}

#[test]
fn deleting_a_row_removes_it_from_search_and_a_restore_brings_back_row_and_segments() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let segments = [Segment {
        idx: 0,
        start_ms: 0,
        end_ms: 500,
        text: "lighthouse".to_owned(),
    }];
    let row = dictation("D1", 1, "the lighthouse keeper");
    history::save(&db, &row, &segments).unwrap();

    let removed = history::delete(&db, "D1").unwrap().unwrap();
    assert!(found(&db, "lighthouse").is_empty());
    assert_eq!(history::count(&db).unwrap(), 0);
    assert!(history::segments(&db, "D1").unwrap().is_empty());
    assert_eq!(history::delete(&db, "D1").unwrap(), None);

    history::restore(&db, &removed).unwrap();
    assert_eq!(history::get(&db, "D1").unwrap(), Some(row));
    assert_eq!(history::segments(&db, "D1").unwrap(), segments);
    assert_eq!(found(&db, "lighthouse"), ["D1"]);
}

#[test]
fn a_reprocessed_row_keeps_its_id_and_search_follows_the_new_text() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let mut failed = dictation("F1", 1, "ignored");
    failed.status = "failed".to_owned();
    failed.error_code = Some("ENGINE_CRASHED".to_owned());
    failed.raw_text = None;
    failed.rule_text = None;
    failed.final_text = None;
    failed.model_id = Some("tiny.en".to_owned());
    history::save(&db, &failed, &[]).unwrap();
    history::save(
        &db,
        &dictation("K1", 2, "an old sentence about oranges"),
        &[],
    )
    .unwrap();

    history::reprocess(
        &db,
        "F1",
        &Reprocessed {
            model_id: Some("base.en".to_owned()),
            language_detected: Some("en".to_owned()),
            prompt: Some("Zyxtrel".to_owned()),
            raw_text: "the lighthouse".to_owned(),
            rule_text: "The lighthouse.".to_owned(),
            final_text: "The lighthouse.".to_owned(),
            segments: vec![Segment {
                idx: 0,
                start_ms: 0,
                end_ms: 700,
                text: "The lighthouse.".to_owned(),
            }],
        },
    )
    .unwrap();
    history::reprocess(
        &db,
        "K1",
        &Reprocessed {
            model_id: Some("base.en".to_owned()),
            language_detected: None,
            prompt: None,
            raw_text: "a new sentence about pears".to_owned(),
            rule_text: "A new sentence about pears.".to_owned(),
            final_text: "A new sentence about pears.".to_owned(),
            segments: Vec::new(),
        },
    )
    .unwrap();

    let recovered = history::get(&db, "F1").unwrap().unwrap();
    assert_eq!(recovered.status, "completed");
    assert_eq!(recovered.error_code, None);
    assert_eq!(recovered.model_id.as_deref(), Some("base.en"));
    assert_eq!(recovered.prompt.as_deref(), Some("Zyxtrel"));
    assert_eq!(recovered.final_text.as_deref(), Some("The lighthouse."));
    assert_eq!(history::count(&db).unwrap(), 2);
    assert_eq!(history::segments(&db, "F1").unwrap().len(), 1);
    assert_eq!(found(&db, "lighthouse"), ["F1"]);
    assert_eq!(found(&db, "pears"), ["K1"]);
    assert!(found(&db, "oranges").is_empty(), "the old text is gone");
}

#[test]
fn clearing_returns_every_audio_path_and_empties_the_tables() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    history::save(&db, &dictation("C1", 1, "one"), &[]).unwrap();
    let mut no_audio = dictation("C2", 2, "two");
    no_audio.audio_path = None;
    history::save(&db, &no_audio, &[]).unwrap();

    let paths = history::clear(&db).unwrap();

    assert_eq!(paths, ["audio/C1.wav"]);
    assert_eq!(history::count(&db).unwrap(), 0);
    assert!(found(&db, "one").is_empty());
}

#[test]
fn a_read_only_history_file_refuses_a_write_with_an_error() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    let file = tmp.path().join("history").join("history.db");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();

    let result = history::save(&db, &dictation("W1", 1, "lost?"), &[]);

    assert!(result.is_err());
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(history::count(&db).unwrap(), 0);
}

#[test]
fn new_ids_are_26_characters_and_sort_by_time() {
    let first = history::new_id(1_700_000_000_000);
    let later = history::new_id(1_700_000_000_001);
    assert_eq!(first.len(), 26);
    assert!(first < later);
    assert_ne!(history::new_id(5), history::new_id(5));
}

/// The 10,000-row search benchmark (VAL-HIST-007). Prints the slowest query.
#[test]
fn search_on_ten_thousand_rows_returns_its_first_page_in_under_200_ms() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, 10_000);

    let queries = [
        "lig",
        "light",
        "lighthouse",
        "keeper of",
        "the quick",
        "brown fox",
        "fox jumps",
        "meeting",
        "noon",
        "café",
        "CAFE",
        "dictation",
        "number 77",
        "sentence",
        "tide",
        "weather",
        "ok",
        "zq",
        "zzqqxx no match",
        "harbor",
    ];
    assert_eq!(queries.len(), 20);
    let mut slowest = std::time::Duration::ZERO;
    for query in queries {
        let started = Instant::now();
        let page = history::page(&db, query, None, 50).unwrap();
        let took = started.elapsed();
        slowest = slowest.max(took);
        println!("search {query:?}: {} rows in {took:?}", page.rows.len());
    }
    println!("slowest of 20 queries: {slowest:?} on 10,000 rows");
    assert!(slowest.as_millis() < 200, "slowest {slowest:?}");

    let started = Instant::now();
    let first = history::page(&db, "", None, 50).unwrap();
    assert_eq!(first.rows.len(), 50);
    assert!(started.elapsed().as_millis() < 200);
}

/// Distinct created_at values, and text that gives each query some hits.
fn seed(db: &Database, rows: i64) {
    const WORDS: [&str; 12] = [
        "lighthouse keeper of the harbor",
        "the quick brown fox jumps",
        "Café meeting at noon",
        "weather turns before the tide",
        "ok then",
        "dictation sentence number",
        "a long sentence about the sea",
        "numbers one two three",
        "hold the key and speak",
        "the report is ready",
        "zebra crossing ahead",
        "light rain in the evening",
    ];
    let connection = db.connection();
    connection.execute_batch("BEGIN").unwrap();
    for n in 0..rows {
        let text = format!("{} number {n}", WORDS[(n % 12) as usize]);
        history::save(db, &dictation(&format!("B{n:06}"), 1_000 + n, &text), &[]).unwrap();
    }
    connection.execute_batch("COMMIT").unwrap();
}
