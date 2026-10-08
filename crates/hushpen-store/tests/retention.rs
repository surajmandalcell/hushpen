use hushpen_store::db::Database;
use hushpen_store::history::{self, Row};
use hushpen_store::retention::{DAY_MS, Retention, SweepReport, remove_audio, sweep};
use std::fs;
use std::path::Path;

const NOW: i64 = 1_800_000_000_000;

fn open(root: &Path) -> Database {
    Database::open(&root.join("history").join("history.db")).unwrap()
}

/// Saves a row made `age_days` ago, with a WAV file on disk.
fn seed(db: &Database, root: &Path, id: &str, age_days: i64, status: &str) {
    let relative = format!("audio/{id}.wav");
    fs::create_dir_all(root.join("audio")).unwrap();
    fs::write(root.join(&relative), b"wav").unwrap();
    let row = Row {
        id: id.to_owned(),
        created_at: NOW - age_days * DAY_MS,
        kind: "dictation".into(),
        status: status.to_owned(),
        duration_ms: 2_000,
        final_text: Some(format!("text of {id}")),
        audio_path: Some(relative),
        ..Row::default()
    };
    history::save(db, &row, &[]).unwrap();
}

fn exists(root: &Path, id: &str) -> bool {
    root.join(format!("audio/{id}.wav")).exists()
}

#[test]
fn thirty_days_removes_the_older_audio_and_keeps_the_text() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, tmp.path(), "OLD", 31, "completed");
    seed(&db, tmp.path(), "NEW", 29, "completed");

    let report = sweep(&db, tmp.path(), Retention::Days(30), NOW).unwrap();

    assert_eq!(
        report,
        SweepReport {
            removed: 1,
            failed: 0
        }
    );
    assert!(!exists(tmp.path(), "OLD"));
    assert!(exists(tmp.path(), "NEW"));
    let old = history::get(&db, "OLD").unwrap().unwrap();
    assert_eq!(old.audio_path, None);
    assert_eq!(old.audio_removed_at, Some(NOW));
    assert_eq!(old.final_text.as_deref(), Some("text of OLD"));
    assert_eq!(
        history::page(&db, "text of OLD", None, 10)
            .unwrap()
            .rows
            .len(),
        1
    );
    let new = history::get(&db, "NEW").unwrap().unwrap();
    assert_eq!(new.audio_path.as_deref(), Some("audio/NEW.wav"));
    assert_eq!(new.audio_removed_at, None);
}

#[test]
fn keep_forever_removes_nothing_even_after_400_days() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, tmp.path(), "ANCIENT", 400, "completed");

    let report = sweep(&db, tmp.path(), Retention::Forever, NOW).unwrap();

    assert_eq!(report, SweepReport::default());
    assert!(exists(tmp.path(), "ANCIENT"));
    assert!(
        history::get(&db, "ANCIENT")
            .unwrap()
            .unwrap()
            .audio_path
            .is_some()
    );
}

#[test]
fn never_removes_the_audio_of_finished_rows_and_keeps_young_unfinished_ones() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, tmp.path(), "DONE", 0, "completed");
    seed(&db, tmp.path(), "FAILED", 1, "failed");
    seed(&db, tmp.path(), "CANCELLED", 2, "cancelled");
    seed(&db, tmp.path(), "STALE", 31, "failed");

    let report = sweep(&db, tmp.path(), Retention::Never, NOW).unwrap();

    assert_eq!(report.removed, 2);
    assert!(!exists(tmp.path(), "DONE"));
    assert!(!exists(tmp.path(), "STALE"));
    assert!(exists(tmp.path(), "FAILED"));
    assert!(exists(tmp.path(), "CANCELLED"));
}

#[test]
fn a_file_that_is_already_gone_still_marks_the_row() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, tmp.path(), "GONE", 40, "completed");
    fs::remove_file(tmp.path().join("audio/GONE.wav")).unwrap();

    let report = sweep(&db, tmp.path(), Retention::Days(30), NOW).unwrap();

    assert_eq!(report.removed, 1);
    assert_eq!(
        history::get(&db, "GONE").unwrap().unwrap().audio_removed_at,
        Some(NOW)
    );
}

#[test]
fn a_second_sweep_has_nothing_left_to_do() {
    let tmp = tempfile::tempdir().unwrap();
    let db = open(tmp.path());
    seed(&db, tmp.path(), "OLD", 31, "completed");
    sweep(&db, tmp.path(), Retention::Days(30), NOW).unwrap();

    let again = sweep(&db, tmp.path(), Retention::Days(30), NOW + DAY_MS).unwrap();

    assert_eq!(again, SweepReport::default());
    assert_eq!(
        history::get(&db, "OLD").unwrap().unwrap().audio_removed_at,
        Some(NOW)
    );
}

#[test]
fn a_path_that_leaves_the_data_folder_is_refused_and_nothing_is_deleted() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    fs::create_dir_all(&root).unwrap();
    let db = open(&root);
    let outside = tmp.path().join("outside.wav");
    fs::write(&outside, b"keep me").unwrap();
    let row = Row {
        id: "EVIL".into(),
        created_at: NOW - 90 * DAY_MS,
        kind: "dictation".into(),
        status: "completed".into(),
        audio_path: Some("../outside.wav".into()),
        ..Row::default()
    };
    history::save(&db, &row, &[]).unwrap();

    let report = sweep(&db, &root, Retention::Days(30), NOW).unwrap();

    assert_eq!(
        report,
        SweepReport {
            removed: 0,
            failed: 1
        }
    );
    assert!(outside.exists());
    assert!(remove_audio(&db, &root, "EVIL", "/etc/passwd", NOW).is_err());
}
