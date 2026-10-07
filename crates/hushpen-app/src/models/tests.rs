use super::*;
use crate::storage;
use gpui_kit::{AppContext, Entity, TestAppContext};
use hushpen_store::data_dir::DataDir;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::VecDeque;

const ALPHA: &[u8] = b"alpha model bytes";
const BETA: &[u8] = b"beta model bytes!";
const GAMMA: &[u8] = b"gamma model byte";

type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

fn content(id: &str) -> &'static [u8] {
    match id {
        "alpha" => ALPHA,
        "beta" => BETA,
        _ => GAMMA,
    }
}

fn catalog() -> &'static Catalog {
    let entry = |id: &str| {
        json!({
            "id": id,
            "name": id.to_uppercase(),
            "file": format!("ggml-{id}.bin"),
            "bytes": content(id).len(),
            "sha256": model_files::hex(&Sha256::digest(content(id))),
            "url": format!("https://huggingface.co/test/{id}.bin"),
            "license": "MIT",
            "languages": "multilingual",
        })
    };
    let text = json!({"whisper": [entry("alpha"), entry("beta"), entry("gamma")]}).to_string();
    Box::leak(Box::new(Catalog::parse(&text).unwrap()))
}

fn transfer(
    run: impl Fn(&Spec, &AtomicBool, &mut dyn FnMut(Progress)) -> Result<Outcome, Failure>
    + Send
    + Sync
    + 'static,
) -> Arc<Transfer> {
    Arc::new(run)
}

/// Writes the model the way the real downloader ends: the file and its stamp.
fn finish_download(spec: &Spec, body: &[u8]) -> Result<Outcome, Failure> {
    std::fs::create_dir_all(spec.dest.parent().unwrap()).unwrap();
    std::fs::write(&spec.dest, body).unwrap();
    model_files::write_stamp(&spec.dest, &spec.sha256).unwrap();
    Ok(Outcome::Done)
}

fn id_of(spec: &Spec) -> &'static str {
    ["alpha", "beta", "gamma"]
        .into_iter()
        .find(|id| spec.url.contains(id))
        .unwrap()
}

fn instant() -> Arc<Transfer> {
    transfer(|spec, _, _| finish_download(spec, content(id_of(spec))))
}

struct Rig {
    models: Entity<Models>,
    storage: Rc<storage::Storage>,
    work: Work,
    _tmp: tempfile::TempDir,
}

impl Rig {
    fn file(&self, id: &str) -> PathBuf {
        self.storage
            .data
            .whisper_models_dir()
            .join(format!("ggml-{id}.bin"))
    }

    fn downloads(&self) -> Vec<String> {
        std::fs::read_dir(self.storage.data.downloads_dir())
            .map(|dir| {
                dir.flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn rig(
    cx: &mut TestAppContext,
    files: &[(&str, &[u8])],
    active: Option<&str>,
    transfer: Arc<Transfer>,
) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    std::fs::create_dir_all(storage.data.whisper_models_dir()).unwrap();
    for (id, body) in files {
        std::fs::write(
            storage
                .data
                .whisper_models_dir()
                .join(format!("ggml-{id}.bin")),
            body,
        )
        .unwrap();
    }
    if let Some(id) = active {
        storage.settings.set(SETTING, json!(id)).unwrap();
    }
    let work: Work = Rc::default();
    let queue = Rc::clone(&work);
    let spawn: Spawner = Rc::new(move |job| {
        queue.borrow_mut().push_back(job);
        Ok(())
    });
    let models =
        cx.new(|cx| Models::new(Rc::clone(&storage), catalog(), transfer, spawn, None, cx));
    Rig {
        models,
        storage,
        work,
        _tmp: tmp,
    }
}

/// Runs the queued worker jobs on this thread and applies the events they send, until nothing
/// is left to do.
fn settle(cx: &mut TestAppContext, rig: &Rig) {
    loop {
        cx.run_until_parked();
        let Some(job) = rig.work.borrow_mut().pop_front() else {
            return;
        };
        job();
    }
}

fn state_of(cx: &mut TestAppContext, rig: &Rig, id: &str) -> ModelState {
    rig.models.read_with(cx, |models, _| {
        models.rows()[models.index_of(id).unwrap()].state
    })
}

fn notice_code(cx: &mut TestAppContext, rig: &Rig) -> Option<&'static str> {
    rig.models
        .read_with(cx, |models, _| models.notice().map(|notice| notice.code))
}

fn active(cx: &mut TestAppContext, rig: &Rig) -> String {
    rig.models.read_with(cx, |models, _| models.active())
}

fn act<T>(
    cx: &mut TestAppContext,
    rig: &Rig,
    run: impl FnOnce(&mut Models, &mut Context<Models>) -> T,
) -> T {
    rig.models.update(cx, run)
}

fn progress(cx: &mut TestAppContext, rig: &Rig, id: &str, job: u64, done: u64, total: u64) {
    let event = ModelEvent::Progress {
        id: id.to_owned(),
        job,
        step: Progress::Bytes { done, total },
    };
    act(cx, rig, |models, cx| models.handle(event, cx));
}

#[gpui_kit::test]
fn the_view_lists_the_catalog_and_marks_a_present_verified_model(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], Some("alpha"), instant());
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::Verifying);
    settle(cx, &rig);
    let state = rig.models.read_with(cx, |models, _| models.state_json());
    let listed: Vec<_> = state["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| {
            (
                model["id"].as_str().unwrap(),
                model["state"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("alpha", "ready"),
            ("beta", "not_downloaded"),
            ("gamma", "not_downloaded")
        ]
    );
    assert_eq!(state["active"], "alpha");
    assert_eq!(state["models"][0]["active"], true);
    assert_eq!(state["models"][1]["active"], false);
}

#[gpui_kit::test]
fn a_file_without_a_stamp_is_hashed_and_then_stamped(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], None, instant());
    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::Ready);
    let stamp = model_files::read_stamp(&rig.file("alpha")).expect("a stamp");
    assert_eq!(stamp.hash, model_files::hex(&Sha256::digest(ALPHA)));
}

#[gpui_kit::test]
fn a_stamped_file_is_ready_without_hashing(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], None, instant());
    settle(cx, &rig);
    let again = cx.new(|cx| {
        Models::new(
            Rc::clone(&rig.storage),
            catalog(),
            instant(),
            Rc::new(|_| panic!("a stamped file needs no worker")),
            None,
            cx,
        )
    });
    let state = again.read_with(cx, |models, _| models.rows()[0].state);
    assert_eq!(state, ModelState::Ready);
}

#[gpui_kit::test]
fn a_changed_file_fails_the_check_and_cannot_be_selected(cx: &mut TestAppContext) {
    let mut changed = BETA.to_vec();
    changed[3] ^= 0xff;
    let rig = rig(
        cx,
        &[("alpha", ALPHA), ("beta", &changed)],
        Some("alpha"),
        instant(),
    );
    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "beta"), ModelState::Failed);

    let refused = act(cx, &rig, |models, cx| models.select("beta", cx));
    assert!(refused.unwrap_err().contains(MODEL_HASH_MISMATCH));
    assert_eq!(notice_code(cx, &rig), Some(MODEL_HASH_MISMATCH));
    assert_eq!(active(cx, &rig), "alpha");
    assert!(model_files::read_stamp(&rig.file("beta")).is_none());
}

#[gpui_kit::test]
fn a_file_of_the_wrong_size_is_failed_without_hashing(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("beta", b"short")], None, instant());
    assert_eq!(state_of(cx, &rig, "beta"), ModelState::Failed);
    assert!(rig.work.borrow().is_empty());
}

#[gpui_kit::test]
fn a_failed_row_puts_its_full_explanation_in_the_view_notice(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("beta", b"short")], None, instant());
    let (notice, detail) = rig.models.read_with(cx, |models, _| {
        let row = &models.rows()[models.index_of("beta").unwrap()];
        (models.notice().cloned(), failure_text(&row.entry))
    });
    let notice = notice.expect("a failed row sets the notice");
    assert_eq!(notice.code, MODEL_HASH_MISMATCH);
    assert_eq!(notice.message, detail);
    assert!(
        detail.len() > 100,
        "the row detail is long enough to need truncating"
    );
}

#[gpui_kit::test]
fn a_file_that_fails_the_hash_check_sets_the_notice_too(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("beta", b"wrong bytes ok!")], None, instant());
    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "beta"), ModelState::Failed);
    assert_eq!(notice_code(cx, &rig), Some(MODEL_HASH_MISMATCH));
}

#[gpui_kit::test]
fn a_failed_model_can_be_downloaded_again(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("beta", b"short")], None, instant());
    act(cx, &rig, |models, cx| models.download("beta", cx)).unwrap();
    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "beta"), ModelState::Ready);
    assert_eq!(std::fs::read(rig.file("beta")).unwrap(), BETA);
}

#[gpui_kit::test]
fn a_download_reports_progress_and_ends_ready(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], None, instant());
    act(cx, &rig, |models, cx| models.download("gamma", cx)).unwrap();
    assert_eq!(
        state_of(cx, &rig, "gamma"),
        ModelState::Downloading {
            done: 0,
            total: GAMMA.len() as u64
        }
    );

    progress(cx, &rig, "gamma", 1, 5, 10);
    assert_eq!(
        state_of(cx, &rig, "gamma"),
        ModelState::Downloading { done: 5, total: 10 }
    );
    let state = rig.models.read_with(cx, |models, _| models.state_json());
    assert_eq!(state["models"][2]["state"], "downloading");
    assert_eq!(state["models"][2]["progress"], 50);

    progress(cx, &rig, "gamma", 99, 9, 10);
    assert_eq!(
        state_of(cx, &rig, "gamma"),
        ModelState::Downloading { done: 5, total: 10 },
        "progress of another job is ignored"
    );

    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "gamma"), ModelState::Ready);
    assert_eq!(notice_code(cx, &rig), None);
}

#[gpui_kit::test]
fn cancel_shows_not_downloaded_at_once_and_leaves_no_file(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[],
        None,
        transfer(|spec, cancel, progress| {
            std::fs::create_dir_all(spec.partial.parent().unwrap()).unwrap();
            std::fs::write(&spec.partial, b"half").unwrap();
            progress(Progress::Bytes { done: 4, total: 17 });
            if cancel.load(Ordering::Relaxed) {
                std::fs::remove_file(&spec.partial).unwrap();
                return Ok(Outcome::Cancelled);
            }
            finish_download(spec, ALPHA)
        }),
    );
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    act(cx, &rig, |models, cx| models.cancel("alpha", cx)).unwrap();
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::NotDownloaded);

    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::NotDownloaded);
    assert!(rig.downloads().is_empty());
    assert!(!rig.file("alpha").exists());
    assert_eq!(notice_code(cx, &rig), None);
}

#[gpui_kit::test]
fn a_late_success_after_cancel_is_dropped(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], None, instant());
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    act(cx, &rig, |models, cx| models.cancel("alpha", cx)).unwrap();
    settle(cx, &rig);
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::NotDownloaded);
    assert!(!rig.file("alpha").exists());
    assert!(!model_files::stamp_path(&rig.file("alpha")).exists());
    assert!(rig.models.read_with(cx, |models, _| models.jobs.is_empty()));
}

#[gpui_kit::test]
fn download_right_after_cancel_starts_once_the_old_worker_is_done(cx: &mut TestAppContext) {
    let starts = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&starts);
    let rig = rig(
        cx,
        &[],
        None,
        transfer(move |spec, cancel, _| {
            if cancel.load(Ordering::Relaxed) {
                return Ok(Outcome::Cancelled);
            }
            seen.store(true, Ordering::Relaxed);
            finish_download(spec, ALPHA)
        }),
    );
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    act(cx, &rig, |models, cx| models.cancel("alpha", cx)).unwrap();
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    assert_eq!(
        rig.work.borrow().len(),
        1,
        "one worker until the cancelled one ends"
    );

    settle(cx, &rig);
    assert!(starts.load(Ordering::Relaxed));
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::Ready);
}

#[gpui_kit::test]
fn a_blocked_host_shows_its_code_and_leaves_the_model_not_downloaded(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[],
        None,
        transfer(|_, _, _| {
            Err(Failure {
                code: MODEL_HOST_BLOCKED,
                detail: "example.com".into(),
            })
        }),
    );
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    settle(cx, &rig);
    assert_eq!(notice_code(cx, &rig), Some(MODEL_HOST_BLOCKED));
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::NotDownloaded);
    assert!(!rig.file("alpha").exists());
}

#[gpui_kit::test]
fn a_hash_mismatch_from_the_downloader_shows_its_code(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[],
        None,
        transfer(|_, _, _| {
            Err(Failure {
                code: MODEL_HASH_MISMATCH,
                detail: String::new(),
            })
        }),
    );
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    settle(cx, &rig);
    assert_eq!(notice_code(cx, &rig), Some(MODEL_HASH_MISMATCH));
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::NotDownloaded);
}

#[gpui_kit::test]
fn a_download_that_stops_says_so_and_can_be_retried(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[],
        None,
        transfer(|_, _, _| {
            Err(Failure {
                code: DOWNLOAD_FAILED,
                detail: "reset".into(),
            })
        }),
    );
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    settle(cx, &rig);
    assert_eq!(notice_code(cx, &rig), Some(DOWNLOAD_FAILED));
    assert!(act(cx, &rig, |models, cx| models.download("alpha", cx)).is_ok());
    assert_eq!(notice_code(cx, &rig), None);
}

#[gpui_kit::test]
fn delete_removes_the_file_and_its_stamp(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[("alpha", ALPHA), ("beta", BETA)],
        Some("alpha"),
        instant(),
    );
    settle(cx, &rig);
    assert!(model_files::stamp_path(&rig.file("beta")).exists());

    act(cx, &rig, |models, cx| models.delete("beta", cx)).unwrap();
    assert_eq!(state_of(cx, &rig, "beta"), ModelState::NotDownloaded);
    assert!(!rig.file("beta").exists());
    assert!(!model_files::stamp_path(&rig.file("beta")).exists());
}

#[gpui_kit::test]
fn the_active_model_cannot_be_deleted(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], Some("alpha"), instant());
    settle(cx, &rig);

    let refused = act(cx, &rig, |models, cx| models.delete("alpha", cx));
    assert!(refused.unwrap_err().contains(MODEL_IN_USE));
    assert_eq!(notice_code(cx, &rig), Some(MODEL_IN_USE));
    assert!(rig.file("alpha").exists());
    assert!(model_files::stamp_path(&rig.file("alpha")).exists());
    assert_eq!(state_of(cx, &rig, "alpha"), ModelState::Ready);
}

#[gpui_kit::test]
fn a_busy_model_cannot_be_deleted(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], None, instant());
    act(cx, &rig, |models, cx| models.download("alpha", cx)).unwrap();
    assert!(act(cx, &rig, |models, cx| models.delete("alpha", cx)).is_err());
}

#[gpui_kit::test]
fn selecting_a_ready_model_saves_it_as_the_active_one(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        &[("alpha", ALPHA), ("beta", BETA)],
        Some("alpha"),
        instant(),
    );
    settle(cx, &rig);

    act(cx, &rig, |models, cx| models.select("beta", cx)).unwrap();
    assert_eq!(active(cx, &rig), "beta");
    assert_eq!(rig.storage.settings.get(SETTING), Some(json!("beta")));
    let state = rig.models.read_with(cx, |models, _| models.state_json());
    assert_eq!(state["models"][1]["active"], true);
    assert_eq!(state["models"][0]["active"], false);
}

#[gpui_kit::test]
fn a_model_that_is_not_downloaded_cannot_be_selected(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], Some("alpha"), instant());
    settle(cx, &rig);
    let refused = act(cx, &rig, |models, cx| models.select("gamma", cx));
    assert!(refused.is_err());
    assert_eq!(active(cx, &rig), "alpha");
}

#[gpui_kit::test]
fn unknown_ids_are_refused_by_every_action(cx: &mut TestAppContext) {
    let rig = rig(cx, &[], None, instant());
    assert!(act(cx, &rig, |models, cx| models.download("nope", cx)).is_err());
    assert!(act(cx, &rig, |models, cx| models.cancel("nope", cx)).is_err());
    assert!(act(cx, &rig, |models, cx| models.delete("nope", cx)).is_err());
    assert!(act(cx, &rig, |models, cx| models.select("nope", cx)).is_err());
}

#[gpui_kit::test]
fn a_ready_model_cannot_be_downloaded_again(cx: &mut TestAppContext) {
    let rig = rig(cx, &[("alpha", ALPHA)], None, instant());
    settle(cx, &rig);
    assert!(act(cx, &rig, |models, cx| models.download("alpha", cx)).is_err());
}
