use hushpen_store::Error;
use hushpen_store::data_dir::{self, Os};
use hushpen_store::data_move;
use std::fs;
use std::path::Path;

fn os(value: &str) -> Option<&std::ffi::OsStr> {
    Some(std::ffi::OsStr::new(value))
}

fn seed_data(root: &Path) {
    fs::create_dir_all(root.join("config")).unwrap();
    fs::create_dir_all(root.join("history")).unwrap();
    fs::create_dir_all(root.join("logs")).unwrap();
    fs::create_dir_all(root.join("cache/sessions")).unwrap();
    fs::create_dir_all(root.join("models/whisper/tiny")).unwrap();
    fs::write(
        root.join(".hushpen-data"),
        r#"{"product":"hushpen","created":"2026-01-02T03:04:05Z"}"#,
    )
    .unwrap();
    fs::write(
        root.join("config/settings.json"),
        r#"{"values":{"audio.mic":{"String":"default"}}}"#,
    )
    .unwrap();
    fs::write(root.join("history/history.db"), b"sqlite-bytes").unwrap();
    fs::write(root.join("history/history.db-wal"), b"wal-bytes").unwrap();
    fs::write(root.join("history/history.db-shm"), b"shm").unwrap();
    fs::write(root.join("logs/app.log"), b"log line\n").unwrap();
    fs::write(root.join("cache/sessions/one.wav"), vec![0u8; 17]).unwrap();
    fs::write(root.join("models/whisper/tiny/model.bin"), vec![1u8; 23]).unwrap();
    fs::create_dir_all(root.join("run")).unwrap();
    fs::write(root.join("run/hushpen.sock"), b"").unwrap();
}

fn tree(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            out.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn a_missing_pointer_means_the_default_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    assert_eq!(data_dir::read_location(&default).unwrap(), None);
    assert_eq!(data_dir::effective(&default).unwrap(), default);
}

#[test]
fn the_pointer_moves_the_data_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    let moved = tmp.path().join("moved");
    data_dir::write_location(&default, Some(&moved)).unwrap();
    assert_eq!(
        data_dir::read_location(&default).unwrap(),
        Some(moved.clone())
    );
    assert_eq!(data_dir::effective(&default).unwrap(), moved);
    data_dir::write_location(&default, None).unwrap();
    assert!(!data_dir::location_file(&default).exists());
}

#[test]
fn a_relative_pointer_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    fs::create_dir_all(&default).unwrap();
    fs::write(
        data_dir::location_file(&default),
        r#"{"path":"relative/folder"}"#,
    )
    .unwrap();
    assert!(data_dir::read_location(&default).is_err());
}

#[test]
fn a_malformed_pointer_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    fs::create_dir_all(&default).unwrap();
    fs::write(data_dir::location_file(&default), b"not json").unwrap();
    assert!(data_dir::read_location(&default).is_err());
}

#[test]
fn the_override_still_wins_over_a_pointer() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("xdg").join("hushpen");
    let moved = tmp.path().join("moved");
    data_dir::write_location(&default, Some(&moved)).unwrap();
    let xdg = tmp.path().join("xdg");
    let resolved =
        data_dir::resolve_effective(Os::Linux, os("/data"), Some(xdg.as_os_str()), None).unwrap();
    assert_eq!(resolved, Path::new("/data"));
    let resolved =
        data_dir::resolve_effective(Os::Linux, None, Some(xdg.as_os_str()), None).unwrap();
    assert_eq!(resolved, moved);
}

#[test]
fn a_move_copies_verifies_points_and_clears_the_old_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    let target = tmp.path().join("moved");
    fs::create_dir_all(&target).unwrap();

    let report = data_move::move_data(&default, &target, &default).unwrap();
    assert!(report.files >= 5, "{report:?}");

    // The target holds the data set, minus runtime and journal files.
    let names = tree(&target);
    for wanted in [
        ".hushpen-data",
        "config",
        "config/settings.json",
        "history",
        "history/history.db",
        "logs",
        "logs/app.log",
        "cache/sessions/one.wav",
        "models/whisper/tiny/model.bin",
    ] {
        assert!(names.contains(&wanted.to_owned()), "missing {wanted}");
    }
    assert!(!names.contains(&"history/history.db-wal".to_owned()));
    assert!(!names.contains(&"run".to_owned()));
    assert_eq!(
        fs::read(target.join("config/settings.json")).unwrap(),
        br#"{"values":{"audio.mic":{"String":"default"}}}"#
    );
    assert_eq!(
        fs::read(target.join("history/history.db")).unwrap(),
        b"sqlite-bytes"
    );

    // The pointer names the absolute target.
    assert_eq!(data_dir::effective(&default).unwrap(), target);

    // The old folder holds only the pointer.
    assert_eq!(tree(&default), vec!["location.json".to_owned()]);
}

#[test]
fn a_move_back_to_the_default_removes_the_pointer() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    let away = tmp.path().join("away");
    seed_data(&away);
    data_dir::write_location(&default, Some(&away)).unwrap();
    fs::create_dir_all(&default).unwrap();

    data_move::move_data(&away, &default, &default).unwrap();

    assert!(!data_dir::location_file(&default).exists());
    assert_eq!(data_dir::effective(&default).unwrap(), default);
    assert!(default.join("history/history.db").is_file());
    assert!(!away.join("history/history.db").exists());
}

#[test]
fn a_folder_with_our_marker_accepts_the_move() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    let target = tmp.path().join("moved");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join(".hushpen-data"),
        r#"{"product":"hushpen","created":"2020-01-02T03:04:05Z"}"#,
    )
    .unwrap();

    data_move::move_data(&default, &target, &default).unwrap();
    assert!(target.join("history/history.db").is_file());
    // The marker the folder already had stays.
    let marker = fs::read_to_string(target.join(".hushpen-data")).unwrap();
    assert!(marker.contains("2020-01-02T03:04:05Z"));
}

#[test]
fn a_foreign_folder_is_refused_and_nothing_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    let target = tmp.path().join("foreign");
    fs::create_dir_all(&target).unwrap();
    fs::write(
        target.join(".hushpen-data"),
        r#"{"product":"other","created":"2020-01-02T03:04:05Z"}"#,
    )
    .unwrap();

    let err = data_move::move_data(&default, &target, &default).unwrap_err();
    assert!(matches!(err, Error::ForeignDataFolder(_)));
    assert!(default.join("history/history.db").is_file());
    assert!(!data_dir::location_file(&default).exists());
    assert!(!target.join("history/history.db").exists());
}

#[test]
fn a_folder_that_is_not_empty_is_refused_and_nothing_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    let target = tmp.path().join("occupied");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("notes.txt"), b"mine").unwrap();

    let err = data_move::move_data(&default, &target, &default).unwrap_err();
    assert!(matches!(err, Error::MoveFailed(_)));
    assert!(default.join("history/history.db").is_file());
    assert!(!data_dir::location_file(&default).exists());
    assert_eq!(tree(&target), vec!["notes.txt".to_owned()]);
}

#[test]
fn a_move_into_the_current_folder_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    let target = default.join("inside");
    assert!(data_move::move_data(&default, &target, &default).is_err());
    assert!(data_move::move_data(&default, &default, &default).is_err());
}

#[cfg(unix)]
#[test]
fn a_failed_copy_cleans_the_target_and_keeps_the_pointer_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let default = tmp.path().join("default");
    seed_data(&default);
    // A dangling link breaks the copy partway through.
    std::os::unix::fs::symlink("/nonexistent", default.join("cache/sessions/broken.wav")).unwrap();
    let target = tmp.path().join("moved");
    fs::create_dir_all(&target).unwrap();

    let err = data_move::move_data(&default, &target, &default).unwrap_err();
    assert!(matches!(err, Error::MoveFailed(_)));
    assert!(default.join("history/history.db").is_file());
    assert!(!data_dir::location_file(&default).exists());
    // The partial copy is gone; the target is empty again.
    assert!(tree(&target).is_empty(), "{:?}", tree(&target));
}
