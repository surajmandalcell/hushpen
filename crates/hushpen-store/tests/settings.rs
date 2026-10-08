use hushpen_store::settings::SettingsStore;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;

fn read(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn a_missing_file_is_created_with_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    let doc = read(&tmp.path().join("settings.json"));
    assert_eq!(doc["schemaVersion"], 1);
    assert!(doc["values"].is_object());
    assert_eq!(store.get("dictation.maxMinutes"), Some(json!(6)));
    assert_eq!(store.get("dictation.language"), Some(json!("auto")));
    assert_eq!(doc["values"]["updates.check"], json!(false));
}

#[test]
fn the_fresh_max_duration_is_the_pipeline_default_of_six_minutes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    let minutes = store
        .get("dictation.maxMinutes")
        .and_then(|value| value.as_u64())
        .unwrap();
    assert_eq!(minutes, 6);
    assert_eq!(
        hushpen_core::dictation::Config::with_max_minutes(minutes),
        hushpen_core::dictation::Config::default()
    );
}

#[test]
fn a_change_survives_a_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    store.set("dictation.maxMinutes", json!(12)).unwrap();
    store.set("overlay.position", json!("top")).unwrap();
    drop(store);

    let store = SettingsStore::open(tmp.path()).unwrap();
    assert_eq!(store.get("dictation.maxMinutes"), Some(json!(12)));
    assert_eq!(store.get("overlay.position"), Some(json!("top")));
}

#[test]
fn thirty_two_parallel_writes_leave_one_complete_file_and_no_temp_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::open(tmp.path()).unwrap());
    let mut handles = Vec::new();
    for minutes in 2..34 {
        let shared = Arc::clone(&store);
        let dir = tmp.path().to_path_buf();
        handles.push(std::thread::spawn(move || {
            shared.set("dictation.maxMinutes", json!(minutes)).unwrap();
            // A second store on the same file races the first one.
            let other = SettingsStore::open(&dir).unwrap();
            other.set("audio.cueVolume", json!(0.25)).unwrap();
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    let doc = read(&tmp.path().join("settings.json"));
    assert_eq!(doc["schemaVersion"], 1);
    let minutes = doc["values"]["dictation.maxMinutes"].as_i64().unwrap();
    assert!((2..34).contains(&minutes));
    assert_eq!(doc["values"]["audio.cueVolume"], json!(0.25));
    let names: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["settings.json"]);
}

#[test]
fn a_corrupt_file_is_backed_up_and_defaults_return() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("settings.json"), b"{not json").unwrap();

    let store = SettingsStore::open(tmp.path()).unwrap();
    assert_eq!(store.get("dictation.maxMinutes"), Some(json!(6)));

    let backups: Vec<_> = std::fs::read_dir(tmp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("settings.json.corrupt-"))
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        std::fs::read(tmp.path().join(&backups[0])).unwrap(),
        b"{not json"
    );
    let suffix = backups[0].trim_start_matches("settings.json.corrupt-");
    assert!(suffix.parse::<u64>().is_ok(), "{suffix}");
    assert_eq!(read(&tmp.path().join("settings.json"))["schemaVersion"], 1);

    drop(store);
    SettingsStore::open(tmp.path()).unwrap();
    let count = std::fs::read_dir(tmp.path()).unwrap().count();
    assert_eq!(count, 2, "a second start makes no new backup");
}

#[test]
fn json_of_the_wrong_shape_counts_as_corrupt() {
    for body in [
        "[]",
        r#"{"values":{}}"#,
        r#"{"schemaVersion":1,"values":3}"#,
    ] {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("settings.json"), body).unwrap();
        SettingsStore::open(tmp.path()).unwrap();
        let has_backup = std::fs::read_dir(tmp.path()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".corrupt-")
        });
        assert!(has_backup, "{body}");
    }
}

#[test]
fn unknown_keys_are_kept_under_unknown() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("settings.json"),
        json!({"schemaVersion": 1, "values": {"future.key": [1, 2], "_unknown": {"older": true}}})
            .to_string(),
    )
    .unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    store.set("dictation.maxMinutes", json!(7)).unwrap();
    drop(store);

    let doc = read(&tmp.path().join("settings.json"));
    assert_eq!(doc["values"]["_unknown"]["future.key"], json!([1, 2]));
    assert_eq!(doc["values"]["_unknown"]["older"], json!(true));
    assert!(doc["values"].get("future.key").is_none());
}

#[test]
fn bad_values_reset_to_their_defaults_on_load() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("settings.json"),
        json!({"schemaVersion": 1, "values": {
            "dictation.maxMinutes": 9999,
            "dictation.language": 7,
            "dictation.recentLanguages": ["en", "de", "fr", "es", "it", "pt", "ja"],
            "overlay.position": "left",
            "audio.cueVolume": 4.0,
            "history.audioRetention": "forever",
            "engine.threads": "auto",
            "cleanup.llm.provider": null,
            "insert.appChords": {"kitty": "ctrl+shift+v", "bad": 3}
        }})
        .to_string(),
    )
    .unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    assert_eq!(store.get("dictation.maxMinutes"), Some(json!(6)));
    assert_eq!(store.get("dictation.language"), Some(json!("auto")));
    assert_eq!(
        store.get("dictation.recentLanguages"),
        Some(json!(["en", "de", "fr", "es", "it"]))
    );
    assert_eq!(store.get("overlay.position"), Some(json!("bottom")));
    assert_eq!(store.get("audio.cueVolume"), Some(json!(0.5)));
    assert_eq!(store.get("history.audioRetention"), Some(json!("forever")));
    assert_eq!(store.get("engine.threads"), Some(json!("auto")));
    assert_eq!(store.get("cleanup.llm.provider"), Some(json!("local")));
    assert_eq!(
        store.get("insert.appChords"),
        Some(json!({"kitty": "ctrl+shift+v"}))
    );
    assert!(
        std::fs::read_dir(tmp.path()).unwrap().count() == 1,
        "no backup for bad values"
    );
}

#[test]
fn a_language_that_whisper_does_not_know_resets_to_auto() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("settings.json"),
        json!({"schemaVersion": 1, "values": {"dictation.language": "xx-invalid"}}).to_string(),
    )
    .unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    assert_eq!(store.get("dictation.language"), Some(json!("auto")));
    assert_eq!(
        read(&tmp.path().join("settings.json"))["values"]["dictation.language"],
        json!("auto")
    );
    store.set("dictation.language", json!("es")).unwrap();
    assert_eq!(store.get("dictation.language"), Some(json!("es")));
    assert!(store.set("dictation.language", json!("xx")).is_err());
    assert_eq!(store.get("dictation.language"), Some(json!("es")));
}

#[test]
fn set_rejects_unknown_internal_and_invalid_input() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    assert!(store.set("no.such.key", json!(1)).is_err());
    assert!(store.set("onboarding.completed", json!(true)).is_err());
    assert!(store.set("overlay.position", json!("left")).is_err());
    assert_eq!(store.get("overlay.position"), Some(json!("bottom")));

    store
        .set_internal("onboarding.completed", json!(true))
        .unwrap();
    assert_eq!(store.get("onboarding.completed"), Some(json!(true)));
    assert!(
        store
            .set_internal("overlay.position", json!("top"))
            .is_err()
    );
}

#[test]
fn values_lists_every_known_key_with_its_current_value() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    store.set("dictation.maxMinutes", json!(9)).unwrap();
    let values = store.values();
    assert_eq!(values.get("dictation.maxMinutes"), Some(&json!(9)));
    assert_eq!(values.get("updates.check"), Some(&json!(false)));
    assert!(!values.contains_key("_unknown"));
}

#[test]
fn choosing_a_flow_bar_position_clears_the_dragged_position() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    store
        .set_internal("overlay.customPos", json!({"x": 340.0, "y": 600.0}))
        .unwrap();
    assert_eq!(
        store.get("overlay.customPos"),
        Some(json!({"x": 340.0, "y": 600.0}))
    );

    // The same preset counts: the user picked a position, so the dragged one goes.
    store.set("overlay.position", json!("bottom")).unwrap();
    assert_eq!(store.get("overlay.customPos"), Some(Value::Null));
    drop(store);

    let store = SettingsStore::open(tmp.path()).unwrap();
    assert_eq!(store.get("overlay.customPos"), Some(Value::Null));
}

#[test]
fn a_refused_position_keeps_the_dragged_position() {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::open(tmp.path()).unwrap();
    store
        .set_internal("overlay.customPos", json!({"x": 1.0, "y": 2.0}))
        .unwrap();
    assert!(store.set("overlay.position", json!("left")).is_err());
    assert_eq!(
        store.get("overlay.customPos"),
        Some(json!({"x": 1.0, "y": 2.0}))
    );
}
