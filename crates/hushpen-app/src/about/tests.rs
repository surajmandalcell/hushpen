use super::*;
use crate::storage;
use gpui_kit::{AppContext as _, Entity, TestAppContext};
use hushpen_store::data_dir::DataDir;
use std::sync::Mutex;

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
    let about = cx.new(|cx| About::new(Rc::clone(&storage), opener.clone(), cx));
    Rig {
        about,
        storage,
        opener,
        _tmp: tmp,
    }
}

fn read<T>(cx: &TestAppContext, rig: &Rig, f: impl FnOnce(&About) -> T) -> T {
    cx.read(|app| f(rig.about.read(app)))
}

#[gpui_kit::test]
fn the_version_is_the_build_version(cx: &mut TestAppContext) {
    let rig = rig(cx);
    assert_eq!(read(cx, &rig, About::version), hushpen_core::BUILD_VERSION);
}

#[gpui_kit::test]
fn each_row_opens_its_target_and_logs_the_request(cx: &mut TestAppContext) {
    let rig = rig(cx);
    rig.about
        .update(cx, |me, cx| me.open(Target::DataFolder, cx));
    rig.about
        .update(cx, |me, cx| me.open(Target::LogFolder, cx));
    rig.about.update(cx, |me, cx| me.open(Target::Github, cx));
    rig.about.update(cx, |me, cx| me.open(Target::Issues, cx));

    assert_eq!(
        *rig.opener.paths.lock().unwrap(),
        vec![
            rig.storage.data.root().to_path_buf(),
            rig.storage.data.logs_dir(),
        ]
    );
    assert_eq!(
        *rig.opener.urls.lock().unwrap(),
        vec![
            hushpen_core::links::REPO_URL.to_owned(),
            hushpen_core::links::ISSUES_URL.to_owned(),
        ]
    );
    let requests = read(cx, &rig, |me| me.requests.borrow().clone());
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[0], json!({"target": "data-folder", "ok": true}));
    assert_eq!(requests[3], json!({"target": "issues", "ok": true}));
}

#[gpui_kit::test]
fn a_failed_open_is_logged_and_shown(cx: &mut TestAppContext) {
    let rig = rig(cx);
    *rig.opener.fail.lock().unwrap() = true;
    rig.about.update(cx, |me, cx| me.open(Target::Github, cx));
    let requests = read(cx, &rig, |me| {
        assert!(me.notice.as_deref().unwrap().contains("browser"));
        me.requests.borrow().clone()
    });
    assert_eq!(requests[0], json!({"target": "github", "ok": false}));
}

#[gpui_kit::test]
fn the_data_folder_shows_home_relative(cx: &mut TestAppContext) {
    let rig = rig(cx);
    // The rig folder is not under HOME, so it shows in full.
    let shown = read(cx, &rig, About::data_folder_display);
    assert_eq!(shown, rig.storage.data.root().display().to_string());
}
