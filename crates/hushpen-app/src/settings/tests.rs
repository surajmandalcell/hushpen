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

/// A login item that records which thread asked for its status, and can answer slowly.
#[derive(Default)]
struct FakeLogin {
    enabled: Mutex<Option<bool>>,
    writes: Mutex<Vec<bool>>,
    fail: Mutex<bool>,
    read_threads: Mutex<Vec<std::thread::ThreadId>>,
    read_delay: Mutex<std::time::Duration>,
}

impl FakeLogin {
    fn reads(&self) -> usize {
        self.read_threads.lock().unwrap().len()
    }

    fn read_on_ui_thread(&self) -> bool {
        let ui = std::thread::current().id();
        self.read_threads.lock().unwrap().contains(&ui)
    }
}

impl LoginToggle for FakeLogin {
    fn is_enabled(&self) -> bool {
        self.read_threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        let delay = *self.read_delay.lock().unwrap();
        std::thread::sleep(delay);
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
    login: Arc<FakeLogin>,
    restarts: Rc<Mutex<u32>>,
    default_dir: std::path::PathBuf,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let rig = rig_with(cx, FakeLogin::default());
    settle_login(cx, &rig);
    rig
}

/// Builds Settings around `login`. The first status read is still on its way when this returns.
fn rig_with(cx: &mut TestAppContext, login: FakeLogin) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), std::sync::Arc::new(FakeMic), cx));
    let login = Arc::new(login);
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

/// Waits for the reading thread of the launch at login status to answer and for the answer to
/// reach the view. The thread is a real one, so this steps the test clock and the wall clock
/// together. The Mic poll (2 s of test time) must not come due, so a test keeps its reads short.
fn settle_login(cx: &mut TestAppContext, rig: &Rig) {
    let step = std::time::Duration::from_millis(20);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        cx.executor().advance_clock(step);
        cx.run_until_parked();
        if !read(cx, rig, |me| me.login_cache.reading) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the login status read never landed"
        );
        std::thread::sleep(step);
    }
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
    settle_login(cx, &rig);
    assert!(!read(cx, &rig, Settings::launch_at_login));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(false));
}

/// What the Settings > General rows ask of the model each time the window draws them.
fn render_general(cx: &mut TestAppContext, rig: &Rig) {
    read(cx, rig, Settings::launch_at_login);
    read(cx, rig, |me| me.stored_bool("startup.startHidden"));
}

#[gpui_kit::test]
fn hook_state_and_the_general_section_never_ask_the_login_item_on_the_ui_thread(
    cx: &mut TestAppContext,
) {
    let login = FakeLogin::default();
    *login.enabled.lock().unwrap() = Some(true);
    *login.read_delay.lock().unwrap() = std::time::Duration::from_millis(800);
    let rig = rig_with(cx, login);

    let started = std::time::Instant::now();
    let state = cx.read(|app| rig.settings.read(app).hook_state_json(app));
    render_general(cx, &rig);
    let shown = read(cx, &rig, Settings::launch_at_login);
    let took = started.elapsed();

    assert!(
        took < std::time::Duration::from_millis(50),
        "the views waited {took:?} for a slow login item"
    );
    assert!(
        !rig.login.read_on_ui_thread(),
        "the status query ran on the UI thread"
    );
    assert_eq!(
        state["launch_at_login"],
        json!(false),
        "the stored intent shows until the answer"
    );
    assert!(!shown);

    settle_login(cx, &rig);

    assert!(
        read(cx, &rig, Settings::launch_at_login),
        "the answer lands in the cache"
    );
    let state = cx.read(|app| rig.settings.read(app).hook_state_json(app));
    assert_eq!(state["launch_at_login"], json!(true));
    assert!(!rig.login.read_on_ui_thread());
    assert_eq!(
        rig.login.reads(),
        1,
        "views read the cache and nothing else"
    );
}

#[gpui_kit::test]
fn repeated_views_and_hook_reads_leave_the_login_item_alone(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let reads = rig.login.reads();

    for _ in 0..20 {
        cx.read(|app| rig.settings.read(app).hook_state_json(app));
        render_general(cx, &rig);
    }

    assert_eq!(rig.login.reads(), reads);
}

#[gpui_kit::test]
fn the_login_cache_refreshes_on_activation_on_open_and_after_a_toggle(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert!(!read(cx, &rig, Settings::launch_at_login));

    // System Settings turned launch at login on, and the user came back.
    *rig.login.enabled.lock().unwrap() = Some(true);
    let before = rig.login.reads();
    rig.settings.update(cx, |me, cx| me.app_activated(cx));
    settle_login(cx, &rig);
    assert_eq!(rig.login.reads(), before + 1);
    assert!(
        read(cx, &rig, Settings::launch_at_login),
        "activation refreshes"
    );

    // It was turned off there, and Settings opens.
    *rig.login.enabled.lock().unwrap() = Some(false);
    let before = rig.login.reads();
    rig.settings.update(cx, |me, cx| me.opened(cx));
    settle_login(cx, &rig);
    assert_eq!(rig.login.reads(), before + 1);
    assert!(
        !read(cx, &rig, Settings::launch_at_login),
        "opening Settings refreshes"
    );

    // A toggle writes the item and reads it back.
    let before = rig.login.reads();
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    assert!(
        read(cx, &rig, Settings::launch_at_login),
        "the row follows the click at once"
    );
    settle_login(cx, &rig);
    assert_eq!(
        rig.login.reads(),
        before + 1,
        "a toggle reads the status again"
    );
    assert!(read(cx, &rig, Settings::launch_at_login));
    assert!(!rig.login.read_on_ui_thread());
}

#[gpui_kit::test]
fn a_read_that_started_before_a_toggle_does_not_overwrite_it(cx: &mut TestAppContext) {
    let login = FakeLogin::default();
    *login.read_delay.lock().unwrap() = std::time::Duration::from_millis(100);
    let rig = rig_with(cx, login);

    // The first read (the item is off) is still running when the user clicks.
    rig.settings
        .update(cx, |me, cx| me.toggle_launch_at_login(cx));
    settle_login(cx, &rig);
    settle_login(cx, &rig);

    assert!(read(cx, &rig, Settings::launch_at_login));
    assert_eq!(value(&rig, "startup.launchAtLogin"), json!(true));
}

#[gpui_kit::test]
fn a_refresh_asked_for_during_a_read_runs_after_it(cx: &mut TestAppContext) {
    let login = FakeLogin::default();
    *login.read_delay.lock().unwrap() = std::time::Duration::from_millis(50);
    let rig = rig_with(cx, login);

    // Asked for while the first read runs, with a newer answer waiting at the system.
    *rig.login.enabled.lock().unwrap() = Some(true);
    rig.settings.update(cx, |me, cx| me.app_activated(cx));
    settle_login(cx, &rig);
    settle_login(cx, &rig);

    assert_eq!(rig.login.reads(), 2);
    assert!(read(cx, &rig, Settings::launch_at_login));
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
    rig.mic
        .update(cx, |mic, cx| {
            mic.start(crate::mic::CaptureUse::Dictation, cx)
        })
        .unwrap();
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
        // The pointer holds the real path; macOS temp folders sit behind the /var symlink.
        Some(target.canonicalize().unwrap())
    );
    let left: Vec<_> = std::fs::read_dir(rig.storage.data.root())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[gpui_kit::test]
fn escape_closes_the_picker_and_names_the_trigger(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.settings
        .update(cx, |me, cx| me.toggle_picker("storage.retention", cx));
    assert_eq!(read(cx, &rig, |me| me.picker), Some("storage.retention"));
    let focus = rig.settings.update(cx, |me, cx| me.close_picker(cx));
    assert!(focus.is_some());
    assert_eq!(read(cx, &rig, |me| me.picker), None);
    // Closing again is a no-op.
    let focus = rig.settings.update(cx, |me, cx| me.close_picker(cx));
    assert!(focus.is_none());
}

type Queue = Rc<std::cell::RefCell<std::collections::VecDeque<Box<dyn FnOnce() + Send>>>>;

/// Settings with the real catalog, on a machine of the given platform and memory. Downloads
/// are queued, then run by `run_work`.
struct AiRig {
    rig: Rig,
    started: std::sync::Arc<Mutex<Vec<String>>>,
    work: Queue,
    loading: Rc<std::cell::Cell<bool>>,
}

fn ai_rig(cx: &mut TestAppContext, platform: Platform, gib: u64) -> AiRig {
    let rig = rig(cx);
    let work: Queue = Rc::default();
    let queue = Rc::clone(&work);
    let spawn: crate::models::Spawner = Rc::new(move |job| {
        queue.borrow_mut().push_back(job);
        Ok(())
    });
    let started: std::sync::Arc<Mutex<Vec<String>>> = Default::default();
    let record = std::sync::Arc::clone(&started);
    let transfer: std::sync::Arc<crate::models::Transfer> = std::sync::Arc::new(
        move |spec: &crate::net::download::Spec,
              _: &std::sync::atomic::AtomicBool,
              _: &mut dyn FnMut(crate::net::download::Progress)| {
            record.lock().unwrap().push(spec.url.clone());
            std::fs::create_dir_all(spec.dest.parent().unwrap()).unwrap();
            std::fs::write(&spec.dest, b"model").unwrap();
            hushpen_store::model_files::write_stamp(&spec.dest, &spec.sha256).unwrap();
            Ok(crate::net::download::Outcome::Done)
        },
    );
    let models = cx.new(|cx| {
        Models::new(
            Rc::clone(&rig.storage),
            hushpen_core::catalog::embedded(),
            transfer,
            spawn,
            None,
            cx,
        )
    });
    let loading = Rc::new(std::cell::Cell::new(false));
    let probe = Rc::clone(&loading);
    rig.settings.update(cx, |me, cx| {
        me.attach_ai(
            AiParts {
                models: models.clone(),
                catalog: hushpen_core::catalog::embedded(),
                platform,
                ram_bytes: gib << 30,
                loading: Rc::new(move || probe.get()),
            },
            cx,
        )
    });
    AiRig {
        rig,
        started,
        work,
        loading,
    }
}

fn run_work(cx: &mut TestAppContext, ai: &AiRig) {
    loop {
        cx.run_until_parked();
        let Some(job) = ai.work.borrow_mut().pop_front() else {
            return;
        };
        job();
    }
}

fn offered(cx: &TestAppContext, ai: &AiRig) -> Vec<String> {
    read(cx, &ai.rig, |me| {
        me.ai_offered().into_iter().map(|entry| entry.id).collect()
    })
}

#[gpui_kit::test]
fn a_mac_with_16_gib_is_offered_gemma_first_and_qwen_second(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::MacOs, 16);
    assert_eq!(offered(cx, &ai), ["gemma-4-e2b", "qwen3.5-2b"]);
    assert_eq!(
        read(cx, &ai.rig, |me| me.ai_model().unwrap().id),
        "gemma-4-e2b"
    );
}

#[gpui_kit::test]
fn a_mac_with_8_gib_and_linux_are_offered_only_qwen(cx: &mut TestAppContext) {
    for (platform, gib) in [(Platform::MacOs, 8), (Platform::Linux, 64)] {
        let ai = ai_rig(cx, platform, gib);
        assert_eq!(offered(cx, &ai), ["qwen3.5-2b"], "{platform:?} {gib} GiB");
        assert_eq!(
            read(cx, &ai.rig, |me| me.ai_model().unwrap().id),
            "qwen3.5-2b"
        );
    }
}

#[gpui_kit::test]
fn a_stored_model_this_computer_is_not_offered_is_ignored(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::Linux, 64);
    ai.rig
        .storage
        .settings
        .set("cleanup.llm.modelId", json!("gemma-4-e2b"))
        .unwrap();
    assert_eq!(
        read(cx, &ai.rig, |me| me.ai_model().unwrap().id),
        "qwen3.5-2b"
    );
}

#[gpui_kit::test]
fn ai_cleanup_is_off_by_default_and_turning_it_on_picks_and_downloads_the_model(
    cx: &mut TestAppContext,
) {
    let ai = ai_rig(cx, Platform::Linux, 16);
    assert_eq!(value(&ai.rig, "cleanup.llm.enabled"), json!(false));
    assert!(
        ai.started.lock().unwrap().is_empty(),
        "nothing downloads while it is off"
    );

    ai.rig
        .settings
        .update(cx, |me, cx| me.toggle_ai_cleanup(cx));
    run_work(cx, &ai);

    assert_eq!(value(&ai.rig, "cleanup.llm.enabled"), json!(true));
    assert_eq!(value(&ai.rig, "cleanup.llm.modelId"), json!("qwen3.5-2b"));
    let urls = ai.started.lock().unwrap().clone();
    assert_eq!(urls.len(), 1);
    assert!(urls[0].contains("Qwen3.5-2B"), "{urls:?}");
}

#[gpui_kit::test]
fn turning_it_off_keeps_the_model_and_starts_no_download(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::Linux, 16);
    ai.rig
        .settings
        .update(cx, |me, cx| me.toggle_ai_cleanup(cx));
    run_work(cx, &ai);
    ai.rig
        .settings
        .update(cx, |me, cx| me.toggle_ai_cleanup(cx));
    run_work(cx, &ai);

    assert_eq!(value(&ai.rig, "cleanup.llm.enabled"), json!(false));
    assert_eq!(value(&ai.rig, "cleanup.llm.modelId"), json!("qwen3.5-2b"));
    assert_eq!(ai.started.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn the_status_follows_the_download_and_the_load(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::Linux, 16);
    let status = |cx: &mut TestAppContext| read_status(cx, &ai);
    assert_eq!(status(cx), Some(AiStatus::NotDownloaded));

    ai.rig
        .settings
        .update(cx, |me, cx| me.toggle_ai_cleanup(cx));
    assert!(matches!(status(cx), Some(AiStatus::Downloading(_))));
    run_work(cx, &ai);
    assert_eq!(status(cx), Some(AiStatus::Ready));

    ai.loading.set(true);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    assert_eq!(status(cx), Some(AiStatus::Preparing));

    ai.loading.set(false);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(600));
    cx.run_until_parked();
    assert_eq!(status(cx), Some(AiStatus::Ready));
}

#[gpui_kit::test]
fn a_missing_file_offers_the_download_again(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::Linux, 16);
    ai.rig
        .settings
        .update(cx, |me, cx| me.toggle_ai_cleanup(cx));
    run_work(cx, &ai);
    assert_eq!(read_status(cx, &ai), Some(AiStatus::Ready));

    let path = ai.rig.storage.data.llm_models_dir().join("qwen3.5-2b.gguf");
    hushpen_store::model_files::remove(&path).unwrap();
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(1_100));
    cx.run_until_parked();
    assert_eq!(read_status(cx, &ai), Some(AiStatus::NotDownloaded));

    ai.rig
        .settings
        .update(cx, |me, cx| me.download_ai_model(cx));
    run_work(cx, &ai);
    assert_eq!(read_status(cx, &ai), Some(AiStatus::Ready));
    assert_eq!(ai.started.lock().unwrap().len(), 2);
}

fn read_status(cx: &TestAppContext, ai: &AiRig) -> Option<AiStatus> {
    cx.read(|app| ai.rig.settings.read(app).ai_status(app))
}

#[gpui_kit::test]
fn the_ai_rows_show_the_model_and_its_status_in_the_tree_state(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::Linux, 16);
    ai.loading.set(true);
    let state = cx.read(|app| ai.rig.settings.read(app).hook_state_json(app));
    assert_eq!(state["ai"]["offered"], json!(["qwen3.5-2b"]));
    assert_eq!(state["ai"]["model"], "qwen3.5-2b");
    assert_eq!(state["ai"]["enabled"], false);
}

#[gpui_kit::test]
fn picking_another_offered_model_saves_it_and_an_unoffered_one_is_ignored(cx: &mut TestAppContext) {
    let ai = ai_rig(cx, Platform::MacOs, 32);
    ai.rig
        .settings
        .update(cx, |me, cx| me.set_ai_model("qwen3.5-2b", cx));
    assert_eq!(value(&ai.rig, "cleanup.llm.modelId"), json!("qwen3.5-2b"));
    ai.rig
        .settings
        .update(cx, |me, cx| me.set_ai_model("not-offered", cx));
    assert_eq!(value(&ai.rig, "cleanup.llm.modelId"), json!("qwen3.5-2b"));
}
