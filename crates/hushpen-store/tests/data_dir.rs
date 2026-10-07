use hushpen_store::data_dir::{DataDir, Os, resolve};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

fn os(value: &str) -> Option<&OsStr> {
    Some(OsStr::new(value))
}

#[test]
fn the_override_wins_over_every_default() {
    let home = Path::new("/home/u");
    for target in [Os::Linux, Os::MacOs] {
        let dir = resolve(target, os("/data"), os("/xdg"), Some(home));
        assert_eq!(dir, PathBuf::from("/data"));
    }
}

#[test]
fn an_empty_override_is_ignored() {
    let dir = resolve(Os::Linux, os(""), None, Some(Path::new("/home/u")));
    assert_eq!(dir, PathBuf::from("/home/u/.local/share/hushpen"));
}

#[test]
fn linux_uses_xdg_data_home() {
    let dir = resolve(Os::Linux, None, os("/xdg"), Some(Path::new("/home/u")));
    assert_eq!(dir, PathBuf::from("/xdg/hushpen"));
}

#[test]
fn linux_falls_back_to_local_share_when_xdg_is_unset_or_empty() {
    let home = Some(Path::new("/home/u"));
    let expected = PathBuf::from("/home/u/.local/share/hushpen");
    assert_eq!(resolve(Os::Linux, None, None, home), expected);
    assert_eq!(resolve(Os::Linux, None, os(""), home), expected);
}

#[test]
fn linux_ignores_a_relative_xdg_data_home() {
    let dir = resolve(Os::Linux, None, os("relative"), Some(Path::new("/home/u")));
    assert_eq!(dir, PathBuf::from("/home/u/.local/share/hushpen"));
}

#[test]
fn macos_uses_application_support() {
    let dir = resolve(Os::MacOs, None, os("/xdg"), Some(Path::new("/Users/u")));
    assert_eq!(
        dir,
        PathBuf::from("/Users/u/Library/Application Support/Hushpen")
    );
}

#[test]
fn a_new_folder_gets_a_marker_and_its_subfolders() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    let data = DataDir::open(&root).unwrap();

    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join(".hushpen-data")).unwrap()).unwrap();
    assert_eq!(marker["product"], "hushpen");
    let created = marker["created"].as_str().unwrap();
    assert_eq!(created.len(), "2026-10-05T12:00:00Z".len(), "{created}");
    assert!(created.ends_with('Z') && created.as_bytes()[10] == b'T');

    assert!(data.config_dir().is_dir());
    assert!(data.history_dir().is_dir());
    assert!(data.logs_dir().is_dir());
}

#[test]
fn session_recordings_live_under_the_cache_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    let data = DataDir::open(&root).unwrap();

    assert_eq!(data.sessions_dir(), root.join("cache").join("sessions"));
}

#[test]
fn a_second_open_keeps_the_created_time() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    DataDir::open(&root).unwrap();
    let marker_path = root.join(".hushpen-data");
    std::fs::write(
        &marker_path,
        r#"{"product":"hushpen","created":"2020-01-02T03:04:05Z"}"#,
    )
    .unwrap();

    let data = DataDir::open(&root).unwrap();
    assert_eq!(data.marker().unwrap().created, "2020-01-02T03:04:05Z");
    let text = std::fs::read_to_string(&marker_path).unwrap();
    assert!(text.contains("2020-01-02T03:04:05Z"));
}

#[test]
fn a_marker_from_another_product_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join(".hushpen-data"),
        r#"{"product":"other","created":"2020-01-02T03:04:05Z"}"#,
    )
    .unwrap();
    assert!(DataDir::open(tmp.path()).is_err());
}
