//! One test per binary: it installs the process-wide logger.

use hushpen_store::log_file;
use hushpen_store::settings::SettingsStore;

#[test]
fn a_corrupt_settings_file_writes_exactly_one_coded_log_line() {
    let tmp = tempfile::tempdir().unwrap();
    let log_path = tmp.path().join("logs").join("hushpen.log");
    log_file::install(&log_path, Some("info")).unwrap();

    let config = tmp.path().join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("settings.json"), b"{not json").unwrap();
    drop(SettingsStore::open(&config).unwrap());
    drop(SettingsStore::open(&config).unwrap());

    let log = std::fs::read_to_string(&log_path).unwrap();
    let lines: Vec<_> = log
        .lines()
        .filter(|l| l.contains("SETTINGS_CORRUPT"))
        .collect();
    assert_eq!(lines.len(), 1, "{log}");
    let fields: Vec<_> = lines[0].splitn(4, ' ').collect();
    assert!(fields[0].ends_with('Z'), "{}", lines[0]);
    assert_eq!(fields[1], "WARN");
    assert!(fields[3].starts_with("SETTINGS_CORRUPT"));
    assert!(
        !log.contains("not json"),
        "the file content stays out of the log"
    );
}
