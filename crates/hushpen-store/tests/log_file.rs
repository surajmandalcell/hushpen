use hushpen_store::log_file::{LOG_FILES, LOG_MAX_BYTES, RotatingFile};
use std::fs;

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn eight_megabytes_leave_exactly_three_files_of_at_most_2_1_mb() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("hushpen.log");
    let mut file = RotatingFile::open(&path, LOG_MAX_BYTES, LOG_FILES).unwrap();
    let line = format!("{}\n", "x".repeat(99));
    for _ in 0..(8_000_000 / line.len()) {
        file.write_line(&line).unwrap();
    }
    drop(file);

    assert_eq!(
        names(tmp.path()),
        ["hushpen.log", "hushpen.log.1", "hushpen.log.2"]
    );
    for name in names(tmp.path()) {
        let size = fs::metadata(tmp.path().join(name)).unwrap().len();
        assert!(size <= 2_100_000, "{size}");
    }
}

#[test]
fn an_oversized_file_from_a_previous_run_rotates_on_open() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("hushpen.log");
    let seed = "seed line\n".repeat(210_000);
    fs::write(&path, &seed).unwrap();

    let mut file = RotatingFile::open(&path, LOG_MAX_BYTES, LOG_FILES).unwrap();
    file.write_line("fresh\n").unwrap();
    drop(file);

    assert_eq!(
        fs::read_to_string(tmp.path().join("hushpen.log.1")).unwrap(),
        seed
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "fresh\n");
}

#[test]
fn a_small_file_is_appended_to() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("hushpen.log");
    for text in ["one\n", "two\n"] {
        let mut file = RotatingFile::open(&path, LOG_MAX_BYTES, LOG_FILES).unwrap();
        file.write_line(text).unwrap();
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    assert_eq!(names(tmp.path()), ["hushpen.log"]);
}
