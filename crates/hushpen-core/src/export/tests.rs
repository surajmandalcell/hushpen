use super::*;

fn golden(name: &str) -> String {
    let path = format!(
        "{}/../../tests/golden/export/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("golden file {path}: {error}"))
}

fn segment(start_ms: i64, end_ms: i64, text: &str) -> Segment {
    Segment {
        start_ms,
        end_ms,
        text: text.to_owned(),
    }
}

/// A row with three segments, the last one an hour in, and non-ASCII text.
fn one() -> Item {
    Item {
        id: "01HQEXPORT0000000000000001".into(),
        created_at_ms: 1_700_000_000_000,
        kind: "dictation".into(),
        status: "completed".into(),
        duration_ms: 3_601_250,
        model_id: Some("base".into()),
        language_requested: Some("auto".into()),
        language_detected: Some("en".into()),
        target_app: Some("Target-gtk.py".into()),
        raw_text: Some("hello there this is the second segment last words cafe".into()),
        final_text: Some("Hello there. This is the second segment. Last words, café ☕.".into()),
        segments: vec![
            segment(0, 1_500, " Hello there."),
            segment(1_500, 62_345, "This is the second segment. "),
            segment(3_600_000, 3_601_250, "Last words, café ☕."),
        ],
        ..Item::default()
    }
}

fn plain_row(id: &str, created_at_ms: i64, duration_ms: i64, text: &str) -> Item {
    Item {
        id: id.into(),
        created_at_ms,
        kind: "dictation".into(),
        status: "completed".into(),
        duration_ms,
        final_text: Some(text.into()),
        ..Item::default()
    }
}

/// Three rows, handed over newest first. The last one has no segments.
fn three() -> Vec<Item> {
    let mut first = plain_row("A", 1_700_000_000_000, 2_000, "First row. Still first.");
    first.segments = vec![
        segment(0, 1_000, "First row."),
        segment(1_000, 2_000, "Still first."),
    ];
    let mut second = plain_row("B", 1_700_000_060_000, 3_000, "Second row.");
    second.segments = vec![segment(0, 3_000, "Second row.")];
    let third = plain_row("C", 1_700_000_120_000, 1_000, "Third row, no segments.");
    vec![third, second, first]
}

#[test]
fn txt_of_one_row_is_its_final_text_and_a_newline_without_a_bom() {
    let text = render(Format::Txt, &[one()]);
    assert_eq!(text, golden("one.txt"));
    assert!(!text.starts_with('\u{feff}'));
}

#[test]
fn txt_of_three_rows_has_a_blank_line_between_the_texts_oldest_first() {
    assert_eq!(render(Format::Txt, &three()), golden("three.txt"));
}

#[test]
fn srt_numbers_cues_from_one_and_uses_comma_times_from_the_segments() {
    let text = render(Format::Srt, &[one()]);
    assert_eq!(text, golden("one.srt"));
    assert!(text.contains("00:00:00,000 --> 00:00:01,500"));
    assert!(text.contains("00:00:01,500 --> 00:01:02,345"));
    assert!(text.contains("01:00:00,000 --> 01:00:01,250"));
}

#[test]
fn vtt_has_the_header_a_blank_line_and_dot_times() {
    let text = render(Format::Vtt, &[one()]);
    assert_eq!(text, golden("one.vtt"));
    assert!(text.starts_with("WEBVTT\n\n"));
    assert!(text.contains("00:00:01.500 --> 00:01:02.345"));
}

#[test]
fn json_of_one_row_has_the_metadata_the_segments_and_the_exact_text() {
    let text = render(Format::Json, &[one()]);
    assert_eq!(text, golden("one.json"));
    let parsed: Value = serde_json::from_str(&text).expect("the file is JSON");
    let row = &parsed["rows"][0];
    assert_eq!(
        row["final_text"],
        "Hello there. This is the second segment. Last words, café ☕."
    );
    assert_eq!(row["segments"][2]["text"], "Last words, café ☕.");
    assert_eq!(row["segments"][2]["start_ms"], 3_600_000);
    assert_eq!(row["segments"][2]["end_ms"], 3_601_250);
}

#[test]
fn many_rows_make_one_srt_with_cues_numbered_without_gaps_and_shifted_by_earlier_durations() {
    let text = render(Format::Srt, &three());
    assert_eq!(text, golden("three.srt"));
}

#[test]
fn many_rows_make_one_vtt_on_one_clock() {
    assert_eq!(render(Format::Vtt, &three()), golden("three.vtt"));
}

#[test]
fn json_of_three_rows_lists_three_row_objects_oldest_first() {
    let parsed: Value = serde_json::from_str(&render(Format::Json, &three())).unwrap();
    let ids: Vec<&str> = parsed["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["A", "B", "C"]);
    assert_eq!(parsed["format_version"], JSON_VERSION);
}

#[test]
fn a_row_without_final_text_adds_no_text_no_cue_and_no_shift_but_json_keeps_it() {
    let mut cancelled = plain_row("B", 1_700_000_030_000, 9_000, "");
    cancelled.final_text = None;
    cancelled.status = "cancelled".into();
    let mut items = three();
    items.push(cancelled);

    assert_eq!(render(Format::Txt, &items), golden("three.txt"));
    assert_eq!(render(Format::Srt, &items), golden("three.srt"));
    assert_eq!(render(Format::Vtt, &items), golden("three.vtt"));
    let parsed: Value = serde_json::from_str(&render(Format::Json, &items)).unwrap();
    assert_eq!(parsed["rows"].as_array().unwrap().len(), 4);
    assert_eq!(parsed["rows"][1]["final_text"], Value::Null);
    assert!(has_text(&items));
    assert!(!has_text(&items[3..]));
}

#[test]
fn a_blank_line_inside_a_cue_would_end_it_so_it_is_dropped() {
    let mut item = plain_row("A", 1, 1_000, "x");
    item.segments = vec![segment(0, 1_000, "one\n\n  \ntwo")];
    assert_eq!(
        render(Format::Srt, &[item]),
        "1\n00:00:00,000 --> 00:00:01,000\none\ntwo\n"
    );
}

#[test]
fn vtt_escapes_the_characters_that_start_markup_and_the_arrow() {
    let mut item = plain_row("A", 1, 1_000, "x");
    item.segments = vec![segment(0, 1_000, "a < b & c --> d")];
    assert_eq!(
        render(Format::Vtt, &[item]),
        "WEBVTT\n\n00:00:00.000 --> 00:00:01.000\na &lt; b &amp; c --&gt; d\n"
    );
}

#[test]
fn a_row_with_no_duration_and_no_segments_still_gets_a_one_second_cue() {
    let item = plain_row("A", 1, 0, "Short.");
    assert_eq!(
        render(Format::Srt, &[item]),
        "1\n00:00:00,000 --> 00:00:01,000\nShort.\n"
    );
}

#[test]
fn hours_past_99_keep_all_their_digits() {
    assert_eq!(clock(100 * 3_600_000 + 1, ','), "100:00:00,001");
}

#[test]
fn formats_are_found_by_key_in_any_case() {
    assert_eq!(Format::from_key("SRT"), Some(Format::Srt));
    assert_eq!(Format::from_key(" json "), Some(Format::Json));
    assert_eq!(Format::from_key("docx"), None);
    assert_eq!(Format::Vtt.extension(), "vtt");
    assert_eq!(Format::Txt.label(), "TXT");
}

#[test]
fn iso_time_is_utc_with_milliseconds() {
    assert_eq!(iso_utc(1_700_000_000_000), "2023-11-14T22:13:20.000Z");
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_utc(951_782_400_123), "2000-02-29T00:00:00.123Z");
}
