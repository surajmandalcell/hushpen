use super::*;
use crate::net::update::CheckError;
use crate::shell::Shell;
use crate::storage;
use crate::theme::space;
use crate::updater::{Checker, Parts as UpdaterParts, SystemInstaller};
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size,
};
use hushpen_core::update::{Offer, Version};
use hushpen_store::data_dir::DataDir;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeOpener {
    urls: Mutex<Vec<String>>,
    paths: Mutex<Vec<PathBuf>>,
    fail: Mutex<bool>,
}

impl Opener for FakeOpener {
    fn open_url(&self, url: &str) -> Result<(), String> {
        if *self.fail.lock().unwrap() {
            return Err("no browser".into());
        }
        self.urls.lock().unwrap().push(url.to_owned());
        Ok(())
    }

    fn open_path(&self, path: &std::path::Path) -> Result<(), String> {
        if *self.fail.lock().unwrap() {
            return Err("no file manager".into());
        }
        self.paths.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// The feed: counts the checks and answers that this version is the newest.
#[derive(Default)]
struct UpToDate {
    calls: Mutex<usize>,
}

impl Checker for UpToDate {
    fn check(&self, _: &Version) -> Result<Offer, CheckError> {
        *self.calls.lock().unwrap() += 1;
        Ok(Offer::UpToDate)
    }
}

const BUILD: Build = Build {
    version: "2.0.0",
    commit: Some("1a2b3c4"),
    date: Some("2026-05-15"),
};

struct Rig {
    about: Entity<About>,
    storage: Rc<storage::Storage>,
    opener: Rc<FakeOpener>,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    let opener = Rc::new(FakeOpener::default());
    let about = cx.new(|cx| About::new(Rc::clone(&storage), opener.clone(), BUILD, cx));
    Rig {
        about,
        storage,
        opener,
        _tmp: tmp,
    }
}

impl Rig {
    fn attach_updater(&self, cx: &mut TestAppContext) -> Arc<UpToDate> {
        let checker = Arc::new(UpToDate::default());
        let updater = cx.new(|cx| {
            Updater::new(
                UpdaterParts {
                    storage: Rc::clone(&self.storage),
                    checker: checker.clone(),
                    opener: self.opener.clone(),
                    installer: Arc::new(SystemInstaller::detect(self.storage.data.logs_dir())),
                    quit: Rc::new(|_| {}),
                    clock: Rc::new(|| 1_000),
                    current: Version::parse("2.0.0").unwrap(),
                },
                cx,
            )
        });
        self.about
            .update(cx, |me, cx| me.attach_updater(updater, cx));
        checker
    }

    fn attach_models(&self, cx: &mut TestAppContext) {
        let models = cx.new(|cx| {
            Models::new(
                Rc::clone(&self.storage),
                hushpen_core::catalog::embedded(),
                crate::models::production_transfer(),
                crate::models::thread_spawner(),
                None,
                cx,
            )
        });
        self.about.update(cx, |me, cx| me.attach_models(models, cx));
    }
}

fn read<T>(cx: &TestAppContext, rig: &Rig, f: impl FnOnce(&About, &App) -> T) -> T {
    cx.read(|app| f(rig.about.read(app), app))
}

fn default_model_name() -> String {
    hushpen_core::catalog::embedded()
        .default_whisper()
        .unwrap()
        .name
        .clone()
}

#[test]
fn the_build_line_names_the_version_the_commit_and_its_date() {
    assert_eq!(BUILD.line(), "Version 2.0.0 (1a2b3c4, 2026-05-15)");
    let commit_only = Build {
        date: None,
        ..BUILD
    };
    assert_eq!(commit_only.line(), "Version 2.0.0 (1a2b3c4)");
    let no_git = Build {
        commit: None,
        date: None,
        ..BUILD
    };
    assert_eq!(no_git.line(), "Version 2.0.0");
}

#[test]
fn this_build_carries_the_build_version_and_a_short_commit() {
    let build = Build::current();
    assert_eq!(build.version, hushpen_core::BUILD_VERSION);
    // A source tree with no git has no commit; a checkout has 7 hex digits.
    if let Some(commit) = build.commit {
        assert!(commit.len() >= 7, "{commit}");
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
    }
    if let Some(date) = build.date {
        assert_eq!(date.len(), "2026-05-15".len(), "{date}");
    }
}

#[gpui_kit::test]
fn the_version_is_the_version_of_the_build(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert_eq!(read(cx, &rig, |me, _| me.version()), "2.0.0");
    let hook = read(cx, &rig, |me, app| me.hook_state_json(app));
    assert_eq!(hook["version"], "2.0.0");
    assert_eq!(hook["commit"], "1a2b3c4");
    assert_eq!(hook["date"], "2026-05-15");
}

#[gpui_kit::test]
fn the_engine_names_whisper_and_the_model_in_use(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert_eq!(read(cx, &rig, |me, app| me.engine(app)), "whisper.cpp");
    rig.attach_models(cx);
    let want = format!("whisper.cpp, {}", default_model_name());
    assert_eq!(read(cx, &rig, |me, app| me.engine(app)), want);
    let hook = read(cx, &rig, |me, app| me.hook_state_json(app));
    assert_eq!(hook["engine"], want);
}

#[gpui_kit::test]
fn each_control_opens_its_target_and_logs_the_request(cx: &mut TestAppContext) {
    let rig = rig(cx);
    for target in [
        Target::DataFolder,
        Target::LogFolder,
        Target::Website,
        Target::Github,
        Target::Issues,
    ] {
        rig.about.update(cx, |me, cx| me.open(target, cx));
    }

    assert_eq!(
        *rig.opener.paths.lock().unwrap(),
        vec![
            rig.storage.data.root().to_path_buf(),
            rig.storage.data.logs_dir(),
        ]
    );
    assert_eq!(
        *rig.opener.urls.lock().unwrap(),
        vec![links::SITE_URL, links::REPO_URL, links::ISSUES_URL]
    );
    let requests = read(cx, &rig, |me, _| me.requests.borrow().clone());
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests[0],
        json!({
            "target": "data-folder",
            "url": rig.storage.data.root().display().to_string(),
            "ok": true,
        })
    );
    assert_eq!(
        requests[2],
        json!({"target": "website", "url": links::SITE_URL, "ok": true})
    );
    assert_eq!(
        requests[4],
        json!({"target": "issues", "url": links::ISSUES_URL, "ok": true})
    );
}

#[gpui_kit::test]
fn a_failed_open_is_logged_and_shown(cx: &mut TestAppContext) {
    let rig = rig(cx);
    *rig.opener.fail.lock().unwrap() = true;
    rig.about.update(cx, |me, cx| me.open(Target::Github, cx));
    let requests = read(cx, &rig, |me, _| {
        assert!(me.notice.as_deref().unwrap().contains("browser"));
        me.requests.borrow().clone()
    });
    assert_eq!(
        requests[0],
        json!({"target": "github", "url": links::REPO_URL, "ok": false})
    );
}

#[gpui_kit::test]
fn the_data_folder_shows_home_relative(cx: &mut TestAppContext) {
    let rig = rig(cx);
    // The rig folder is not under HOME, so it shows in full.
    let shown = read(cx, &rig, |me, _| me.data_folder_display());
    assert_eq!(shown, rig.storage.data.root().display().to_string());
}

#[gpui_kit::test]
fn opening_the_log_folder_recreates_it(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let logs = rig.storage.data.logs_dir();
    std::fs::remove_dir_all(&logs).unwrap();
    rig.about
        .update(cx, |me, cx| me.open(Target::LogFolder, cx));
    assert!(logs.is_dir(), "logs/ came back for the file manager");
}

#[gpui_kit::test]
fn check_for_updates_asks_once_with_the_check_off_and_leaves_it_off(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert!(!read(cx, &rig, |me, app| me.can_check_for_updates(app)));
    let checker = rig.attach_updater(cx);
    assert!(read(cx, &rig, |me, app| me.can_check_for_updates(app)));
    assert_eq!(read(cx, &rig, |me, app| me.update_status(app)), None);

    rig.about.update(cx, |me, cx| me.check_for_updates(cx));
    cx.run_until_parked();
    assert_eq!(*checker.calls.lock().unwrap(), 1);
    assert_eq!(
        read(cx, &rig, |me, app| me.update_status(app)),
        Some(("Hushpen is up to date".to_owned(), false))
    );
    assert_ne!(
        rig.storage.settings.get("updates.check"),
        Some(json!(true)),
        "the automatic check stays off"
    );
}

fn write_notices(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "<html></html>").unwrap();
}

#[test]
fn the_notices_are_found_beside_the_binary_in_every_package_layout() {
    let tmp = tempfile::tempdir().unwrap();
    // tar.gz, deb, rpm, and the harness: the file sits next to the binary.
    let flat = tmp.path().join("flat");
    write_notices(&flat.join(NOTICES_FILE));
    assert_eq!(locate_notices(&flat), Some(flat.join(NOTICES_FILE)));
    // .app: Contents/MacOS/hushpen with Contents/Resources.
    let mac = tmp.path().join("Hushpen.app/Contents");
    write_notices(&mac.join("Resources").join(NOTICES_FILE));
    std::fs::create_dir_all(mac.join("MacOS")).unwrap();
    assert_eq!(
        locate_notices(&mac.join("MacOS")),
        Some(mac.join("MacOS/../Resources").join(NOTICES_FILE))
    );
    // AppImage: usr/bin/hushpen with usr/lib/hushpen.
    let usr = tmp.path().join("squashfs-root/usr");
    write_notices(&usr.join("lib/hushpen").join(NOTICES_FILE));
    std::fs::create_dir_all(usr.join("bin")).unwrap();
    assert_eq!(
        locate_notices(&usr.join("bin")),
        Some(usr.join("bin/../lib/hushpen").join(NOTICES_FILE))
    );
    assert_eq!(locate_notices(&tmp.path().join("empty")), None);
}

#[gpui_kit::test]
fn the_licenses_row_opens_the_notices_file_and_logs_the_request(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let notices = rig._tmp.path().join(NOTICES_FILE);
    write_notices(&notices);
    rig.about
        .update(cx, |me, _| me.set_notices(Some(notices.clone())));
    rig.about.update(cx, |me, cx| me.open(Target::Licenses, cx));

    assert_eq!(*rig.opener.paths.lock().unwrap(), vec![notices.clone()]);
    let requests = read(cx, &rig, |me, _| me.requests.borrow().clone());
    assert_eq!(
        requests[0],
        json!({"target": "licenses", "url": notices.display().to_string(), "ok": true})
    );
}

#[gpui_kit::test]
fn a_missing_notices_file_is_logged_and_shown(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.about.update(cx, |me, _| me.set_notices(None));
    rig.about.update(cx, |me, cx| me.open(Target::Licenses, cx));

    assert!(rig.opener.paths.lock().unwrap().is_empty());
    let requests = read(cx, &rig, |me, _| {
        assert!(me.notice.as_deref().unwrap().contains("licenses"));
        me.requests.borrow().clone()
    });
    assert_eq!(
        requests[0],
        json!({"target": "licenses", "url": "", "ok": false})
    );
}

// ---- The view ----

fn open_about(cx: &mut TestAppContext, rig: &Rig) -> AnyWindowHandle {
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
    shell.update(cx, |shell, cx| {
        shell.attach_about(rig.about.clone(), cx);
        shell.select(View::About, cx);
    });
    frame(cx, handle);
    handle
}

fn frame(cx: &mut TestAppContext, handle: AnyWindowHandle) {
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| w.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) {
    cx.update_window(handle, |_, w, cx| w.click(id, cx))
        .unwrap();
    frame(cx, handle);
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &'static str) -> Option<String> {
    cx.update_window(handle, |_, w, _| {
        w.try_find(id)
            .and_then(|found| found.label().map(str::to_owned))
    })
    .unwrap()
}

#[gpui_kit::test]
fn the_view_shows_the_header_the_facts_the_links_and_the_copyright(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.attach_models(cx);
    rig.attach_updater(cx);
    let window = open_about(cx, &rig);
    let engine = format!("whisper.cpp, {}", default_model_name());
    let folder = rig.storage.data.root().display().to_string();
    for (id, want) in [
        ("about.icon", "Hushpen app icon"),
        ("about.name", "Hushpen"),
        ("about.tagline", TAGLINE),
        ("about.build", "Version 2.0.0 (1a2b3c4, 2026-05-15)"),
        ("about.checkForUpdates", "Check for Updates"),
        ("about.version", "2.0.0"),
        ("about.engine", engine.as_str()),
        ("about.dataFolder", folder.as_str()),
        ("about.open.data-folder", "Show"),
        ("about.open.website", "Website"),
        ("about.open.github", "GitHub"),
        ("about.open.issues", "Report an issue"),
        ("about.open.licenses", "Open-source licenses"),
        ("about.open.log-folder", "Open log folder"),
        ("about.copyright", COPYRIGHT),
    ] {
        assert_eq!(label(cx, window, id).as_deref(), Some(want), "{id}");
    }
    assert_eq!(
        label(cx, window, "about.updates.status").as_deref(),
        Some(""),
        "no status before the first check"
    );
}

#[gpui_kit::test]
fn the_check_for_updates_button_runs_one_check_and_shows_what_it_found(cx: &mut TestAppContext) {
    let rig = rig(cx);
    let checker = rig.attach_updater(cx);
    let window = open_about(cx, &rig);
    click(cx, window, "about.checkForUpdates");
    assert_eq!(*checker.calls.lock().unwrap(), 1);
    assert_eq!(
        label(cx, window, "about.updates.status").as_deref(),
        Some("Hushpen is up to date")
    );
    click(cx, window, "about.open.website");
    assert_eq!(*rig.opener.urls.lock().unwrap(), [links::SITE_URL]);
}
