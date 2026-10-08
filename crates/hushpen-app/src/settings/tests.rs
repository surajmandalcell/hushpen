use super::*;
use crate::mic::{Mic, MicBackend, MicSession};
use crate::storage;
use gpui_kit::{AppContext as _, Entity, TestAppContext};
use hushpen_audio::{CaptureError, EventSink, FeedInfo, Finished, InputDevice};
use hushpen_core::error::DATA_FOLDER_MOVE_FAILED;
use hushpen_store::data_dir::DataDir;
use serde_json::json;
use std::sync::Mutex;

struct FakeMic;

impl MicBackend for FakeMic {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        Ok(Vec::new())
    }

    fn start(
        &self,
        _device: &str,
        path: std::path::PathBuf,
        _sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"wav").unwrap();
        Ok(Box::new(FakeSession(path)))
    }
}

struct FakeSession(PathBuf);

impl MicSession for FakeSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError> {
        Ok(Finished {
            path: self.0,
            samples: 160,
            duration_ms: 10,
        })
    }

    fn feed_wav(&self, _path: &std::path::Path) -> Result<FeedInfo, CaptureError> {
        Err(CaptureError {
            code: "test",
            detail: "no feed".into(),
        })
    }
}

#[derive(Default)]
struct FakeLogin {
    enabled: Mutex<Option<bool>>,
    writes: Mutex<Vec<bool>>,
    fail: Mutex<bool>,
}

impl LoginToggle for FakeLogin {
    fn is_enabled(&self) -> bool {
        self.enabled.lock().unwrap().unwrap_or(false)
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        if *self.fail.lock().unwrap() {
            return Err("the system refused".into());
        }
        *self.enabled.lock().unwrap() = Some(enabled);
        self.writes.lock().unwrap().push(enabled);
        Ok(())
    }
}

struct Rig {
    settings: Entity<Settings>,
    storage: Rc<storage::Storage>,
    mic: Entity<Mic>,
    login: Rc<FakeLogin>,
    restarts: Rc<Mutex<u32>>,
    default_dir: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), std::sync::Arc::new(FakeMic), cx));
    let login = Rc::new(FakeLogin::default());
    let restarts = Rc::new(Mutex::new(0));
    let recorded = Rc::clone(&restarts);
    let default_dir = tmp.path().join("default");
    let settings = cx.new(|cx| {
        Settings::new(
            Parts {
                storage: Rc::clone(&storage),
                mic: mic.clone(),
                login: Some(login.clone()),
                default_data_dir: Some(default_dir.clone()),
                restart: Some(Rc::new(move |_| {
                    *recorded.lock().unwrap() += 1;
                })),
            },
            cx,
        )
    });
    Rig {
        settings,
        storage,
        mic,
        login,
        restarts,
        default_dir,
        _tmp: tmp,
    }
}

fn read<T>(cx: &TestAppContext, rig: &Rig, f: impl FnOnce(&Settings) -> T) -> T {
    cx.read(|app| f(rig.settings.read(app)))
}

fn value(rig: &Rig, key: &str) -> Value {
    rig.storage.settings.get(key).unwrap_or(Value::Null)
}

#[gpui_kit::test]
fn each_toggle_writes_its_key(cx: &mut TestAppContext) {
    let rig = rig(cx);
    type Toggle = fn(&mut Settings, &mut Context<Settings>);
    let cases: [(&str, Toggle); 5] = [
        ("startup.startHidden", Settings::toggle_start_hidden),
        ("audio.cueSounds", Settings::toggle_sounds),
        ("cleanup.rules", Settings::toggle_rules),
        ("overlay.enabled", Settings::toggle_flow_bar),
        ("overlay.idleVisible", Settings::toggle_flow_bar_idle),
    ];
    for (key, toggle) in cases {
        let before = value(&rig, key).as_bool().unwrap();
        rig.settings.update(cx, toggle);
        assert_eq!(
            value(&rig, key).as_bool().unwrap(),
            !before,
            "{key} did not flip"
        );
        rig.settings.update(cx, toggle);
        assert_eq!(value(&rig, key).as_bool().unwrap(), before, "{key}");
    }
}

#[gpui_kit::test]
fn launch_at_login_writes_the_login_item_and_the_key(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(true));
    assert_eq!(*rig.login.writes.lock().unwrap(), vec![true]);
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(false));
    assert_eq!(*rig.login.writes.lock().unwrap(), vec![true, false]);
}

#[gpui_kit::test]
fn a_refused_login_item_keeps_the_setting_off_and_says_why(cx: &mut TestAppContext) {
    let rig = rig(cx);
    *rig.login.fail.lock().unwrap() = true;
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(false));
    assert!(read(cx, &rig, |me| me.notice.is_some()));
}

#[gpui_kit::test]
fn a_login_item_removed_by_hand_reads_as_off_on_open(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(true));
    // The user removes the login item in the system settings.
    *rig.login.enabled.lock().unwrap() = Some(false);
    rig.settings.update(cx, |me, cx| me.opened(cx));
    assert!(!read(cx, &rig, Settings::launch_at_login));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(false));
}

#[gpui_kit::test]
fn the_retention_and_position_controls_write_their_keys(cx: &mut TestAppContext) {
    let rig = rig(cx);
    for retention in ["never", "forever", "30d"] {
        rig.settings
            .update(cx, |me, cx| me.set_retention(retention, cx));
        assert_eq!(value(&rig, "history.audioRetention"), json!(retention));
    }
    // A dragged position is cleared by a position choice.
    rig.storage
        .settings
        .set_internal("overlay.customPos", json!({"x": 10.0, "y": 20.0}))
        .unwrap();
    rig.settings.update(cx, |me, cx| me.set_position("top", cx));
    assert_eq!(value(&rig, "overlay.position"), json!("top"));
    assert_eq!(value(&rig, "overlay.customPos"), Value::Null);
}

#[gpui_kit::test]
fn sections_switch_and_close_the_open_picker(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings
        .update(cx, |me, cx| me.toggle_picker("storage.retention", cx));
    assert_eq!(read(cx, &rig, |me| me.picker), Some("storage.retention"));
    rig.settings
        .update(cx, |me, cx| me.select_section(Section::Audio, cx));
    assert_eq!(read(cx, &rig, |me| me.section), Section::Audio);
    assert_eq!(read(cx, &rig, |me| me.picker), None);
}

#[gpui_kit::test]
fn a_move_during_a_recording_is_refused(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    rig.settings.update(cx, |me, cx| me.change_data_folder(cx));
    read(cx, &rig, |me| {
        assert_eq!(me.move_state, MoveState::Idle);
        assert!(me.notice.as_deref().unwrap().contains("recording"));
    });
}

#[gpui_kit::test]
fn a_cancelled_picker_leaves_everything_as_it_was(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings.update(cx, |me, cx| me.finish_move(None, cx));
    assert_eq!(read(cx, &rig, |me| me.move_state.clone()), MoveState::Idle);
}

#[gpui_kit::test]
fn a_failed_move_shows_the_code_and_changes_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let target = rig._tmp.path().join("occupied");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("mine.txt"), b"mine").unwrap();

    rig.settings
        .update(cx, |me, cx| me.finish_move(Some(target.clone()), cx));

    read(cx, &rig, |me| {
        assert_eq!(me.move_state, MoveState::Failed(DATA_FOLDER_MOVE_FAILED));
        assert!(
            me.notice
                .as_deref()
                .unwrap()
                .contains(DATA_FOLDER_MOVE_FAILED)
        );
    });
    // Nothing moved and nothing was pointed away.
    assert!(rig.storage.data.database_path().is_file());
    assert!(!target.join("history/history.db").exists());
    assert!(target.join("mine.txt").is_file());
    assert_eq!(*rig.restarts.lock().unwrap(), 0);
}

#[gpui_kit::test]
fn a_finished_move_restarts_into_the_new_folder(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let target = rig._tmp.path().join("moved");
    std::fs::create_dir_all(&target).unwrap();

    rig.settings
        .update(cx, |me, cx| me.finish_move(Some(target.clone()), cx));

    assert_eq!(
        read(cx, &rig, |me| me.move_state.clone()),
        MoveState::Restarting
    );
    assert_eq!(*rig.restarts.lock().unwrap(), 1);
    // The copy carries the database, the pointer names the target, and the
    // old folder is empty.
    assert!(target.join("history/history.db").is_file());
    assert_eq!(
        hushpen_store::data_dir::read_location(&rig.default_dir).unwrap(),
        Some(target.clone())
    );
    let left: Vec<_> = std::fs::read_dir(rig.storage.data.root())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}
