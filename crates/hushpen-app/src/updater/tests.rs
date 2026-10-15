use super::*;
use crate::net::download::Progress;
use crate::settings::{Parts as SettingsParts, Section, Settings};
use crate::shell::Shell;
use crate::storage;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size,
};
use hushpen_core::error::{UPDATE_CHECK_FAILED, UPDATE_HASH_MISMATCH, UPDATE_SWAP_FAILED};
use hushpen_core::update::{Artifact, CHECK_INTERVAL_MS, InstallKind, parse_manifest};
use hushpen_store::data_dir::DataDir;
use std::cell::Cell;
use std::collections::VecDeque;
use std::sync::Mutex;

const NOTES: &str = "https://github.com/surajmandalcell/asrpro/releases/tag/v2.1.0";
const HOUR_MS: u64 = 60 * 60 * 1000;

fn offered(version: &str) -> Offer {
    let json = format!(r#"{{"version":"{version}","notes_url":"{NOTES}"}}"#);
    Offer::Available(parse_manifest(json.as_bytes()).unwrap())
}

#[derive(Default)]
struct FakeChecker {
    answers: Mutex<VecDeque<Result<Offer, CheckError>>>,
    calls: Mutex<Vec<String>>,
}

impl FakeChecker {
    fn answer(&self, answer: Result<Offer, CheckError>) {
        self.answers.lock().unwrap().push_back(answer);
    }

    fn calls(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

impl Checker for FakeChecker {
    fn check(&self, current: &Version) -> Result<Offer, CheckError> {
        self.calls.lock().unwrap().push(current.as_str().to_owned());
        self.answers
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(Offer::UpToDate))
    }
}

#[derive(Default)]
struct FakeOpener {
    urls: Mutex<Vec<String>>,
}

impl Opener for FakeOpener {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.urls.lock().unwrap().push(url.to_owned());
        Ok(())
    }

    fn open_path(&self, _: &std::path::Path) -> Result<(), String> {
        unreachable!("the update check opens only addresses")
    }
}

struct FakeInstaller {
    kind: Mutex<InstallKind>,
    stages: Mutex<Vec<String>>,
    stage_error: Mutex<Option<CheckError>>,
    restart_error: Mutex<Option<CheckError>>,
    restarts: Mutex<Vec<std::path::PathBuf>>,
}

impl FakeInstaller {
    fn new() -> Self {
        Self {
            kind: Mutex::new(InstallKind::Unknown),
            stages: Mutex::default(),
            stage_error: Mutex::default(),
            restart_error: Mutex::default(),
            restarts: Mutex::default(),
        }
    }

    fn is(&self, kind: InstallKind) {
        *self.kind.lock().unwrap() = kind;
    }

    fn stages(&self) -> Vec<String> {
        self.stages.lock().unwrap().clone()
    }

    fn restarts(&self) -> usize {
        self.restarts.lock().unwrap().len()
    }
}

impl Installer for FakeInstaller {
    fn kind(&self) -> InstallKind {
        *self.kind.lock().unwrap()
    }

    fn stage(
        &self,
        artifact: &Artifact,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Staged, CheckError> {
        self.stages.lock().unwrap().push(artifact.url.clone());
        progress(Progress::Bytes {
            done: 50,
            total: 100,
        });
        progress(Progress::Verifying);
        match self.stage_error.lock().unwrap().take() {
            Some(error) => Err(error),
            None => Ok(Staged {
                path: "/stage/new".into(),
                cleanup: vec!["/stage".into()],
            }),
        }
    }

    fn restart(&self, staged: &Staged) -> Result<(), CheckError> {
        if let Some(error) = self.restart_error.lock().unwrap().take() {
            return Err(error);
        }
        self.restarts.lock().unwrap().push(staged.path.clone());
        Ok(())
    }
}

struct Rig {
    updater: Entity<Updater>,
    storage: Rc<storage::Storage>,
    checker: Arc<FakeChecker>,
    opener: Rc<FakeOpener>,
    installer: Arc<FakeInstaller>,
    quits: Rc<Cell<u32>>,
    now: Rc<Cell<u64>>,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    let checker = Arc::new(FakeChecker::default());
    let opener = Rc::new(FakeOpener::default());
    let installer = Arc::new(FakeInstaller::new());
    let quits = Rc::new(Cell::new(0));
    let now = Rc::new(Cell::new(1_000 * HOUR_MS));
    let clock = Rc::clone(&now);
    let quit = Rc::clone(&quits);
    let updater = cx.new(|cx| {
        Updater::new(
            Parts {
                storage: Rc::clone(&storage),
                checker: checker.clone(),
                opener: opener.clone(),
                installer: installer.clone(),
                quit: Rc::new(move |_| quit.set(quit.get() + 1)),
                clock: Rc::new(move || clock.get()),
                current: Version::parse("2.0.0").unwrap(),
            },
            cx,
        )
    });
    Rig {
        updater,
        storage,
        checker,
        opener,
        installer,
        quits,
        now,
        _tmp: tmp,
    }
}

const ARTIFACT_URL: &str = "http://127.0.0.1:8765/Hushpen.AppImage";

/// A signed manifest that lists a file for the build running this test.
fn offered_with_file(version: &str) -> Offer {
    let target = hushpen_core::update::current_target().expect("a supported test host");
    let json = format!(
        r#"{{"version":"{version}","notes_url":"{NOTES}","platforms":{{"{target}":{{"url":"{ARTIFACT_URL}","sha256":"{}","size":100}}}}}}"#,
        "ab".repeat(32)
    );
    Offer::Available(parse_manifest(json.as_bytes()).unwrap())
}

impl Rig {
    fn set(&self, key: &str, value: Value) {
        self.storage.settings.set(key, value).unwrap();
    }

    fn state(&self, cx: &TestAppContext) -> State {
        cx.read(|app| self.updater.read(app).state().clone())
    }

    fn hook(&self, cx: &TestAppContext) -> Value {
        cx.read(|app| self.updater.read(app).hook_state_json())
    }

    fn start(&self, cx: &mut TestAppContext) {
        self.updater.update(cx, |me, cx| me.start(cx));
        cx.run_until_parked();
    }

    fn check_now(&self, cx: &mut TestAppContext) {
        self.updater.update(cx, |me, cx| me.check_now(cx));
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn both_switches_are_off_by_default_and_no_check_runs(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.start(cx);
    rig.check_now(cx);
    let hook = rig.hook(cx);
    assert_eq!(hook["check"], false);
    assert_eq!(hook["auto_install"], false);
    assert_eq!(
        rig.checker.calls(),
        0,
        "the check is off, so nothing is asked"
    );
    assert_eq!(rig.state(cx), State::Idle);
    assert_eq!(
        rig.storage.settings.get("updates.lastCheckAt"),
        Some(json!(0))
    );
}

#[gpui_kit::test]
fn a_due_check_runs_at_start_and_the_next_start_within_a_day_does_not(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(Offer::UpToDate));
    rig.start(cx);
    assert_eq!(rig.checker.calls(), 1);
    assert_eq!(rig.state(cx), State::UpToDate);
    assert_eq!(
        rig.storage.settings.get("updates.lastCheckAt"),
        Some(json!(rig.now.get()))
    );

    rig.now.set(rig.now.get() + HOUR_MS);
    rig.start(cx);
    rig.start(cx);
    assert_eq!(rig.checker.calls(), 1, "starts within 24 h make no request");

    rig.now.set(rig.now.get() + CHECK_INTERVAL_MS);
    rig.start(cx);
    assert_eq!(
        rig.checker.calls(),
        2,
        "a day later the next start checks again"
    );
}

#[gpui_kit::test]
fn check_now_ignores_the_daily_limit(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.start(cx);
    rig.check_now(cx);
    assert_eq!(rig.checker.calls(), 2);
}

#[gpui_kit::test]
fn turning_the_check_on_runs_a_due_check_once(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.start(cx);
    assert_eq!(rig.checker.calls(), 0);
    rig.set("updates.check", json!(true));
    rig.updater.update(cx, |me, cx| me.settings_changed(cx));
    cx.run_until_parked();
    assert_eq!(rig.checker.calls(), 1);
    rig.updater.update(cx, |me, cx| me.settings_changed(cx));
    cx.run_until_parked();
    assert_eq!(rig.checker.calls(), 1, "not again inside the day");
}

#[gpui_kit::test]
fn a_newer_version_is_available_with_its_notes_link_and_a_banner(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(offered("2.1.0")));
    rig.check_now(cx);
    let State::Available(manifest) = rig.state(cx) else {
        panic!("{:?}", rig.state(cx));
    };
    assert_eq!(manifest.version.as_str(), "2.1.0");
    let hook = rig.hook(cx);
    assert_eq!(hook["state"], "available");
    assert_eq!(hook["version"], "2.1.0");
    assert_eq!(hook["banner"], true);

    rig.updater.update(cx, |me, cx| me.open_notes(cx));
    assert_eq!(*rig.opener.urls.lock().unwrap(), [NOTES]);
    assert_eq!(rig.hook(cx)["last_open"], json!({"url": NOTES, "ok": true}));
}

#[gpui_kit::test]
fn the_banner_waits_for_a_person_only_while_installs_are_by_hand(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(offered("2.1.0")));
    rig.check_now(cx);
    assert_eq!(rig.hook(cx)["banner"], true);

    rig.updater.update(cx, |me, cx| me.dismiss(cx));
    assert_eq!(rig.hook(cx)["banner"], false, "sent away for this version");
    assert_eq!(
        rig.storage.settings.get("updates.dismissedVersion"),
        Some(json!("2.1.0"))
    );
    assert_eq!(
        rig.hook(cx)["state"],
        "available",
        "Settings still shows it"
    );

    rig.checker.answer(Ok(offered("2.2.0")));
    rig.check_now(cx);
    assert_eq!(
        rig.hook(cx)["banner"],
        true,
        "a newer version brings it back"
    );

    rig.set("updates.autoInstall", json!(true));
    rig.updater.update(cx, |me, cx| me.settings_changed(cx));
    assert_eq!(
        rig.hook(cx)["banner"],
        false,
        "automatic installs need no banner"
    );
    assert_eq!(rig.hook(cx)["install_requests"], 1);
}

#[gpui_kit::test]
fn install_starts_only_from_an_offered_version(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.updater.update(cx, |me, cx| me.install(cx));
    assert_eq!(rig.hook(cx)["install_requests"], 0);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(offered("2.1.0")));
    rig.check_now(cx);
    rig.updater.update(cx, |me, cx| me.install(cx));
    assert_eq!(rig.hook(cx)["install_requests"], 1);
}

#[gpui_kit::test]
fn a_failed_check_shows_a_plain_sentence_and_keeps_the_code_in_the_hook(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Err(CheckError {
        code: UPDATE_SIGNATURE_INVALID,
        detail: "the signature does not match the update key".into(),
    }));
    rig.check_now(cx);
    let hook = rig.hook(cx);
    assert_eq!(hook["state"], "error");
    assert_eq!(hook["error"]["code"], "UPDATE_SIGNATURE_INVALID");
    assert_eq!(hook["version"], Value::Null, "no new version is shown");
    assert_eq!(hook["banner"], false);
    let text = cx.read(|app| rig.updater.read(app).status_text());
    assert_eq!(text, "The update could not be verified");

    rig.checker.answer(Err(CheckError {
        code: UPDATE_CHECK_FAILED,
        detail: "cannot reach github.com".into(),
    }));
    rig.check_now(cx);
    let text = cx.read(|app| rig.updater.read(app).status_text());
    assert_eq!(text, "The update check failed");
    assert!(!text.contains("UPDATE_"));
}

#[gpui_kit::test]
fn turning_the_check_off_while_it_runs_drops_the_answer(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(offered("2.1.0")));
    rig.updater.update(cx, |me, cx| me.check_now(cx));
    assert_eq!(rig.state(cx), State::Checking);
    rig.set("updates.check", json!(false));
    rig.updater.update(cx, |me, cx| me.settings_changed(cx));
    cx.run_until_parked();
    assert_eq!(rig.state(cx), State::Idle, "the late answer is ignored");
}

// ---- The views ----

struct Window {
    handle: AnyWindowHandle,
}

fn open_settings(cx: &mut TestAppContext, rig: &Rig) -> Window {
    cx.update(gpui_kit::init);
    let (handle, shell) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: Point::default(),
                    size: size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
                })),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| Shell::new(window, cx)),
        )
        .expect("open test window")
    });
    let storage = Rc::clone(&rig.storage);
    let settings = cx.new(|cx| {
        Settings::new(
            SettingsParts {
                storage: Rc::clone(&storage),
                mic: cx.new(|cx| {
                    crate::mic::Mic::new(
                        storage,
                        Arc::new(crate::controller::testkit::FakeMic {
                            start_error: Mutex::new(None),
                            starts: std::sync::atomic::AtomicUsize::new(0),
                            sink: Mutex::new(None),
                        }),
                        cx,
                    )
                }),
                login: None,
                default_data_dir: None,
                restart: None,
            },
            cx,
        )
    });
    settings.update(cx, |settings, cx| {
        settings.attach_updater(rig.updater.clone(), cx);
        settings.select_section(Section::Updates, cx);
    });
    shell.update(cx, |shell, cx| {
        shell.attach_updater(rig.updater.clone(), cx);
        shell.attach_settings(settings.clone(), cx);
        shell.select(View::Settings, cx);
    });
    let window = Window { handle };
    frame(cx, &window);
    window
}

fn frame(cx: &mut TestAppContext, window: &Window) {
    cx.run_until_parked();
    cx.update_window(window.handle, |_, w, cx| w.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, window: &Window, id: &'static str) {
    cx.update_window(window.handle, |_, w, cx| w.click(id, cx))
        .unwrap();
    frame(cx, window);
}

fn present(cx: &mut TestAppContext, window: &Window, id: &'static str) -> bool {
    cx.update_window(window.handle, |_, w, _| w.try_find(id).is_some())
        .unwrap()
}

fn label(cx: &mut TestAppContext, window: &Window, id: &'static str) -> Option<String> {
    cx.update_window(window.handle, |_, w, _| {
        w.try_find(id)
            .and_then(|found| found.label().map(str::to_owned))
    })
    .unwrap()
}

#[gpui_kit::test]
fn settings_updates_shows_both_switches_off_and_check_now_disabled(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let window = open_settings(cx, &rig);
    for id in ["settings.updates.check", "settings.updates.autoInstall"] {
        assert!(present(cx, &window, id), "{id}");
    }
    assert_eq!(rig.hook(cx)["check"], false);
    assert_eq!(rig.hook(cx)["auto_install"], false);
    assert_eq!(
        label(cx, &window, "settings.updates.status").as_deref(),
        Some("Update checks are off")
    );
    click(cx, &window, "settings.updates.checkNow");
    assert_eq!(rig.checker.calls(), 0, "a disabled button asks nothing");
    assert!(!present(cx, &window, "update.banner"));
}

#[gpui_kit::test]
fn the_switches_write_the_settings_and_check_now_reaches_the_banner(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let window = open_settings(cx, &rig);
    click(cx, &window, "settings.updates.check");
    assert_eq!(rig.storage.settings.get("updates.check"), Some(json!(true)));
    assert_eq!(
        rig.checker.calls(),
        1,
        "turning the check on runs the due check"
    );

    rig.checker.answer(Ok(offered("2.1.0")));
    click(cx, &window, "settings.updates.checkNow");
    assert_eq!(
        label(cx, &window, "settings.updates.status").as_deref(),
        Some("Version 2.1.0 is available")
    );
    assert!(present(cx, &window, "settings.updates.notes"));
    assert!(
        present(cx, &window, "update.banner.install"),
        "the banner offers Download and install"
    );
    click(cx, &window, "settings.updates.notes");
    assert_eq!(*rig.opener.urls.lock().unwrap(), [NOTES]);

    click(cx, &window, "update.banner.install");
    assert_eq!(rig.hook(cx)["install_requests"], 1);

    click(cx, &window, "update.banner.dismiss");
    assert!(!present(cx, &window, "update.banner"));

    click(cx, &window, "settings.updates.autoInstall");
    assert_eq!(
        rig.storage.settings.get("updates.autoInstall"),
        Some(json!(true))
    );
}

// ---- Installing ----

fn check_in(rig: &Rig, cx: &mut TestAppContext, offer: Offer) {
    rig.set("updates.check", json!(true));
    rig.checker.answer(Ok(offer));
    rig.check_now(cx);
}

#[gpui_kit::test]
fn by_hand_nothing_downloads_until_download_and_install_is_pressed(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    assert_eq!(rig.state(cx).key(), "available");
    assert!(
        rig.installer.stages().is_empty(),
        "the offer downloads nothing"
    );

    rig.updater.update(cx, |me, cx| me.install(cx));
    assert_eq!(
        rig.state(cx).key(),
        "downloading",
        "the press starts at once"
    );
    cx.run_until_parked();
    assert_eq!(rig.installer.stages(), [ARTIFACT_URL]);
    assert_eq!(rig.state(cx).key(), "ready");
    assert_eq!(rig.installer.restarts(), 0, "the swap waits for a restart");
    assert_eq!(rig.quits.get(), 0);
}

#[gpui_kit::test]
fn with_automatic_installs_on_the_download_starts_by_itself_and_the_swap_waits(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    rig.set("updates.autoInstall", json!(true));
    check_in(&rig, cx, offered_with_file("2.1.0"));
    assert_eq!(rig.installer.stages(), [ARTIFACT_URL]);
    assert_eq!(rig.state(cx).key(), "ready");
    assert_eq!(
        rig.hook(cx)["banner"],
        true,
        "the ready banner asks for a restart"
    );
    assert_eq!(rig.installer.restarts(), 0);
    assert_eq!(rig.quits.get(), 0);

    rig.updater.update(cx, |me, cx| me.restart_to_update(cx));
    assert_eq!(rig.installer.restarts(), 1);
    assert_eq!(rig.quits.get(), 1, "the app quits so the helper can swap");
}

#[gpui_kit::test]
fn turning_automatic_installs_on_stages_an_offer_that_is_waiting(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    assert!(rig.installer.stages().is_empty());
    rig.set("updates.autoInstall", json!(true));
    rig.updater.update(cx, |me, cx| me.settings_changed(cx));
    cx.run_until_parked();
    assert_eq!(rig.state(cx).key(), "ready");
}

#[gpui_kit::test]
fn restart_does_nothing_before_the_new_version_is_ready(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    rig.updater.update(cx, |me, cx| me.restart_to_update(cx));
    assert_eq!(rig.installer.restarts(), 0);
    assert_eq!(rig.quits.get(), 0);
}

#[gpui_kit::test]
fn a_failed_download_ends_in_the_error_state_with_the_code_and_keeps_running(
    cx: &mut TestAppContext,
) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    *rig.installer.stage_error.lock().unwrap() = Some(CheckError {
        code: UPDATE_HASH_MISMATCH,
        detail: "the file does not match the manifest".into(),
    });
    rig.set("updates.autoInstall", json!(true));
    check_in(&rig, cx, offered_with_file("2.1.0"));
    let hook = rig.hook(cx);
    assert_eq!(hook["state"], "error");
    assert_eq!(hook["error"]["code"], "UPDATE_HASH_MISMATCH");
    assert_eq!(hook["version"], Value::Null);
    assert_eq!(rig.installer.restarts(), 0);
    assert_eq!(rig.quits.get(), 0, "the old version keeps running");
    let text = cx.read(|app| rig.updater.read(app).status_text());
    assert_eq!(text, "The downloaded update did not match its checksum");
}

#[gpui_kit::test]
fn a_helper_that_cannot_start_leaves_the_app_running_with_an_error(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    rig.set("updates.autoInstall", json!(true));
    check_in(&rig, cx, offered_with_file("2.1.0"));
    *rig.installer.restart_error.lock().unwrap() = Some(CheckError {
        code: UPDATE_SWAP_FAILED,
        detail: "cannot start the update helper".into(),
    });
    rig.updater.update(cx, |me, cx| me.restart_to_update(cx));
    assert_eq!(rig.hook(cx)["error"]["code"], "UPDATE_SWAP_FAILED");
    assert_eq!(rig.quits.get(), 0);
    let text = cx.read(|app| rig.updater.read(app).status_text());
    assert_eq!(text, "The update could not be installed");
}

#[gpui_kit::test]
fn deb_rpm_and_tar_installs_get_a_notice_and_never_download(cx: &mut TestAppContext) {
    for kind in [InstallKind::Deb, InstallKind::Rpm, InstallKind::Tar] {
        let rig = rig(cx);
        rig.installer.is(kind);
        rig.set("updates.autoInstall", json!(true));
        check_in(&rig, cx, offered_with_file("2.1.0"));
        let hook = rig.hook(cx);
        assert_eq!(hook["state"], "available", "{kind:?} keeps the offer");
        assert_eq!(hook["delivery"], "notice", "{kind:?}");
        assert_eq!(hook["install_kind"], kind.key());
        assert_eq!(
            hook["banner"], true,
            "{kind:?}: the notice shows even with auto install on"
        );
        rig.updater.update(cx, |me, cx| me.install(cx));
        rig.updater.update(cx, |me, cx| me.settings_changed(cx));
        cx.run_until_parked();
        assert!(
            rig.installer.stages().is_empty(),
            "{kind:?} asks for no file"
        );
        assert_eq!(rig.hook(cx)["install_requests"], 0);
        assert_eq!(rig.state(cx).key(), "available");
    }
}

#[gpui_kit::test]
fn an_appimage_without_a_file_for_this_platform_gets_the_notice(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    check_in(&rig, cx, offered("2.1.0"));
    assert_eq!(rig.hook(cx)["delivery"], "notice");
    rig.updater.update(cx, |me, cx| me.install(cx));
    cx.run_until_parked();
    assert!(rig.installer.stages().is_empty());
}

#[gpui_kit::test]
fn a_source_tree_build_keeps_the_by_hand_offer_and_replaces_nothing(cx: &mut TestAppContext) {
    let rig = rig(cx);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    assert_eq!(rig.hook(cx)["delivery"], "unpackaged");
    rig.updater.update(cx, |me, cx| me.install(cx));
    cx.run_until_parked();
    assert!(rig.installer.stages().is_empty());
    assert_eq!(rig.hook(cx)["install_requests"], 1);
    assert_eq!(rig.state(cx).key(), "available");
}

#[gpui_kit::test]
fn the_notice_banner_links_the_release_and_has_no_install_button(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::Deb);
    let window = open_settings(cx, &rig);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    frame(cx, &window);
    assert!(present(cx, &window, "update.banner"));
    assert!(!present(cx, &window, "update.banner.install"));
    assert!(present(cx, &window, "update.banner.notes"));
    click(cx, &window, "update.banner.notes");
    assert_eq!(*rig.opener.urls.lock().unwrap(), [NOTES]);
}

#[gpui_kit::test]
fn the_banner_follows_the_download_and_asks_for_a_restart_when_ready(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.installer.is(InstallKind::AppImage);
    let window = open_settings(cx, &rig);
    check_in(&rig, cx, offered_with_file("2.1.0"));
    frame(cx, &window);
    assert!(present(cx, &window, "update.banner.install"));

    click(cx, &window, "update.banner.install");
    assert_eq!(rig.state(cx).key(), "ready");
    assert!(!present(cx, &window, "update.banner.install"));
    assert_eq!(
        label(cx, &window, "update.banner.restart").as_deref(),
        Some("Restart to update")
    );
    assert_eq!(
        label(cx, &window, "settings.updates.restart").as_deref(),
        Some("Restart to update")
    );
    click(cx, &window, "update.banner.restart");
    assert_eq!(rig.installer.restarts(), 1);
    assert_eq!(rig.quits.get(), 1);
}
