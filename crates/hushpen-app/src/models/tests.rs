use super::testkit::{self, Rig, settle};
use super::*;
use gpui_kit::{AppContext, TestAppContext};
use sha2::{Digest, Sha256};

const ALPHA: &[u8] = b"alpha model bytes";
const BETA: &[u8] = b"beta model bytes!";
const GAMMA: &[u8] = b"gamma model byte";

#[test]
fn startup_replaces_retired_speech_and_removes_only_retired_files() {
    use hushpen_core::catalog::{RETIRED_SPEECH, SPEECH_DEFAULT};
    use hushpen_store::data_dir::DataDir;

    let dir = tempfile::tempdir().unwrap();
    let storage = crate::storage::open(DataDir::open(dir.path().join("data")).unwrap()).unwrap();
    let models = storage.data.whisper_models_dir();
    std::fs::create_dir_all(&models).unwrap();
    for id in RETIRED_SPEECH
        .iter()
        .copied()
        .chain(["base-q5_1", "personal-model"])
    {
        let path = models.join(format!("ggml-{id}.bin"));
        std::fs::write(&path, ALPHA).unwrap();
        model_files::write_stamp(&path, &"a".repeat(64)).unwrap();
    }
    storage.settings.set(SETTING, json!("base.en")).unwrap();
    sweep_retired_speech(&storage, hushpen_core::catalog::embedded());
    assert_eq!(storage.settings.get(SETTING), Some(json!(SPEECH_DEFAULT)));
    for id in RETIRED_SPEECH {
        let path = models.join(format!("ggml-{id}.bin"));
        assert!(!path.exists(), "{id}");
        assert!(!model_files::stamp_path(&path).exists(), "{id}");
    }
    for id in ["base-q5_1", "personal-model"] {
        let path = models.join(format!("ggml-{id}.bin"));
        assert_eq!(std::fs::read(&path).unwrap(), ALPHA);
        assert!(model_files::stamp_path(&path).exists(), "{id}");
    }
    storage.settings.set(SETTING, json!("small-q5_1")).unwrap();
    sweep_retired_speech(&storage, hushpen_core::catalog::embedded());
    assert_eq!(storage.settings.get(SETTING), Some(json!("small-q5_1")));
}

#[test]
fn startup_removes_a_retired_stamp_even_if_the_model_is_gone() {
    use hushpen_store::data_dir::DataDir;
    let dir = tempfile::tempdir().unwrap();
    let storage = crate::storage::open(DataDir::open(dir.path().join("data")).unwrap()).unwrap();
    let path = storage.data.whisper_models_dir().join("ggml-tiny.bin");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(model_files::stamp_path(&path), b"old stamp").unwrap();
    sweep_retired_speech(&storage, hushpen_core::catalog::embedded());
    assert!(!model_files::stamp_path(&path).exists());
}

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
    testkit::leak(json!({"whisper": [entry("alpha"), entry("beta"), entry("gamma")]}))
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

#[gpui_kit::test]
fn a_downloaded_default_is_active_without_a_saved_choice(cx: &mut TestAppContext) {
    let mut catalog = catalog().clone();
    catalog.whisper[0].default = true;
    let rig = testkit::rig(cx, Box::leak(Box::new(catalog)), instant(), |storage| {
        let path = storage.data.whisper_models_dir().join("ggml-alpha.bin");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, ALPHA).unwrap();
    });
    settle(cx, &rig);
    rig.models.read_with(cx, |models, _| {
        assert_eq!(models.effective(), "alpha");
        assert_eq!(models.state_json()["models"][0]["active"], true);
    });
    let error = rig
        .models
        .update(cx, |models, cx| models.delete("alpha", cx))
        .unwrap_err();
    assert!(error.contains(MODEL_IN_USE));
    assert!(rig.file("alpha").is_file());
}

/// The library with `files` already in the speech folder and `active` chosen.
fn rig(
    cx: &mut TestAppContext,
    files: &[(&str, &[u8])],
    active: Option<&str>,
    transfer: Arc<Transfer>,
) -> Rig {
    testkit::rig(cx, catalog(), transfer, |storage| {
        std::fs::create_dir_all(storage.data.whisper_models_dir()).unwrap();
        for (id, body) in files {
            let path = storage
                .data
                .whisper_models_dir()
                .join(format!("ggml-{id}.bin"));
            std::fs::write(path, body).unwrap();
        }
        if let Some(id) = active {
            storage.settings.set(SETTING, json!(id)).unwrap();
        }
    })
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
            Machine::test(),
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

/// The search of the Models view: which rows a query keeps, and the field in the title row.
mod search_tests {
    use super::super::search::matches;
    use super::testkit::{self, Rig};
    use super::*;
    use crate::shell::Shell;
    use crate::theme::space;
    use crate::views::View;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AnyWindowHandle, Bounds, Point, SharedString, WindowBounds, WindowOptions, px, size,
    };

    fn sha(body: &[u8]) -> String {
        model_files::hex(&Sha256::digest(body))
    }

    /// Two speech models, a cleanup model, and a voice, each with its own words and size.
    fn library() -> &'static Catalog {
        let entry = |id: &str, kind: &str, name: &str, languages: &str, bytes: u64, text: &str| {
            json!({"id": id, "kind": kind, "name": name, "file": format!("{id}.{}", if kind == "cleanup" { "gguf" } else { "bin" }),
                "bytes": bytes, "sha256": sha(id.as_bytes()),
                "url": format!("https://huggingface.co/test/{id}.bin"),
                "license": "MIT", "languages": languages, "default": id == "base",
                "description": text, "speed": 3, "accuracy": 3, "quantization": "INT8",
                "ram_idle_bytes": 1, "ram_peak_bytes": 2, "recommended_for": "Tests",
                "source_page": "https://huggingface.co/test/repo"})
        };
        let mut voice = entry(
            "kokoro",
            "voice",
            "Kokoro",
            "english",
            0,
            "Reads text aloud.",
        );
        let object = voice.as_object_mut().unwrap();
        for key in ["file", "bytes", "sha256", "url"] {
            object.remove(key);
        }
        object.insert(
            "files".into(),
            json!([{"file": "kokoro.onnx", "bytes": 330_000_000, "sha256": sha(b"kokoro"),
                "url": "https://huggingface.co/test/kokoro.onnx"}]),
        );
        testkit::leak(json!({
            "schema": 2,
            "whisper": [
                entry("base", "speech", "Base", "multilingual", 148_000_000,
                    "Quick, for everyday dictation."),
                entry("large", "speech", "Large v3", "multilingual", 3_100_000_000,
                    "The most accurate, and slow."),
            ],
            "llm": [entry("qwen3.5-2b", "cleanup", "Qwen Tidy", "english", 1_600_000_000,
                "Fixes grammar and punctuation.")],
            "voice": [voice],
        }))
    }

    fn rig(cx: &mut TestAppContext) -> Rig {
        testkit::rig(cx, library(), instant(), |_| {})
    }

    fn shown(cx: &mut TestAppContext, rig: &Rig, query: &str) -> Vec<String> {
        rig.models.read_with(cx, |models, _| {
            models
                .all_rows()
                .iter()
                .filter(|row| matches(row, query))
                .map(|row| row.entry.id.clone())
                .collect()
        })
    }

    #[gpui_kit::test]
    fn a_query_finds_a_model_by_name_description_language_or_size(cx: &mut TestAppContext) {
        let rig = rig(cx);
        assert_eq!(
            shown(cx, &rig, "").len(),
            4,
            "an empty query keeps every row"
        );
        assert_eq!(shown(cx, &rig, "  ").len(), 4);
        assert_eq!(shown(cx, &rig, "KOKO"), ["kokoro"], "name, in any case");
        assert_eq!(shown(cx, &rig, "large v3"), ["large"], "two words");
        assert_eq!(shown(cx, &rig, "accurate"), ["large"], "description");
        assert_eq!(shown(cx, &rig, "grammar"), ["qwen3.5-2b"]);
        assert_eq!(
            shown(cx, &rig, "multilingual"),
            ["base", "large"],
            "language"
        );
        assert_eq!(shown(cx, &rig, "english"), ["qwen3.5-2b", "kokoro"]);
        assert_eq!(
            shown(cx, &rig, "3.1 gb"),
            ["large"],
            "size, as the row shows it"
        );
        assert_eq!(shown(cx, &rig, "mb"), ["base", "kokoro"], "size unit");
        assert_eq!(shown(cx, &rig, "recommended"), ["base", "qwen3.5-2b"]);
        assert!(shown(cx, &rig, "zebra").is_empty());
        assert!(
            shown(cx, &rig, "kokoro zebra").is_empty(),
            "every word must match"
        );
    }

    fn window(cx: &mut TestAppContext, rig: &Rig) -> AnyWindowHandle {
        cx.update(gpui_kit::init);
        cx.update(crate::theme::install);
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
        cx.update_window(handle, |_, window, cx| {
            rig.models
                .update(cx, |models, cx| models.attach_search(window, cx))
        })
        .unwrap();
        shell.update(cx, |shell, cx| {
            shell.attach_models(rig.models.clone(), cx);
            shell.select(View::Models, cx);
        });
        render(cx, handle);
        handle
    }

    fn render(cx: &mut TestAppContext, handle: AnyWindowHandle) {
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }

    fn present(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> bool {
        let id = SharedString::from(id.to_owned());
        cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
            .unwrap()
    }

    fn focused(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> bool {
        let id = SharedString::from(id.to_owned());
        cx.update_window(handle, |_, window, _| {
            window.try_find(id).and_then(|found| found.focused()) == Some(true)
        })
        .unwrap()
    }

    fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) {
        let id = SharedString::from(id.to_owned());
        cx.update_window(handle, |_, window, cx| window.click(id, cx))
            .unwrap();
        render(cx, handle);
    }

    fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &str) {
        cx.update_window(handle, |_, window, cx| window.press(key, cx))
            .unwrap();
        render(cx, handle);
    }

    fn type_text(cx: &mut TestAppContext, handle: AnyWindowHandle, text: &str) {
        cx.update_window(handle, |_, window, cx| window.input(text, cx))
            .unwrap();
        render(cx, handle);
    }

    fn rows_shown(cx: &mut TestAppContext, handle: AnyWindowHandle) -> Vec<usize> {
        (0..4)
            .filter(|index| present(cx, handle, &format!("models.row.{index}")))
            .collect()
    }

    fn query(cx: &mut TestAppContext, rig: &Rig) -> String {
        rig.models
            .read_with(cx, |models, _| models.query().to_owned())
    }

    #[gpui_kit::test]
    fn the_search_field_sits_at_the_right_end_of_the_title_row(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        let id = SharedString::from("models.search");
        let (field, title) = cx
            .update_window(handle, |_, window, _| {
                (window.find(id).bounds(), window.find("view.title").bounds())
            })
            .unwrap();
        assert_eq!(field.size.width, px(crate::theme::size::SEARCH_FIELD_WIDTH));
        assert_eq!(field.size.height, px(crate::theme::size::BUTTON_HEIGHT));
        assert_eq!(
            field.right(),
            px(space::WINDOW_WIDTH - space::XXL),
            "24px from the edge"
        );
        assert_eq!(field.center().y, title.center().y, "on the chrome line");
        assert!(title.right() < field.left());
    }

    #[gpui_kit::test]
    fn typing_keeps_the_matching_rows_and_hides_empty_sections(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        assert_eq!(rows_shown(cx, handle), [0, 1, 2, 3]);
        click(cx, handle, "models.search");
        type_text(cx, handle, "kokoro");
        assert_eq!(query(cx, &rig), "kokoro");
        assert_eq!(rows_shown(cx, handle), [3]);
        assert!(present(cx, handle, "models.heading.voice"));
        assert!(present(cx, handle, "models.help.voice"));
        assert!(
            !present(cx, handle, "models.heading.speech"),
            "empty sections hide"
        );
        assert!(!present(cx, handle, "models.heading.cleanup"));
        assert!(!present(cx, handle, "models.search.empty"));
        let state = rig.models.read_with(cx, |models, _| models.state_json());
        assert_eq!(state["query"], "kokoro");
    }

    #[gpui_kit::test]
    fn no_match_says_so_and_clear_brings_every_row_back(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        click(cx, handle, "models.search");
        type_text(cx, handle, "zebra");
        assert_eq!(rows_shown(cx, handle), Vec::<usize>::new());
        let message = cx
            .update_window(handle, |_, window, _| {
                window
                    .find("models.search.empty")
                    .label()
                    .map(str::to_owned)
            })
            .unwrap();
        assert_eq!(message.as_deref(), Some("No models match \"zebra\""));
        click(cx, handle, "models.search.clear");
        assert_eq!(query(cx, &rig), "");
        assert_eq!(rows_shown(cx, handle), [0, 1, 2, 3]);
        assert!(!present(cx, handle, "models.search.empty"));
    }

    #[gpui_kit::test]
    fn esc_clears_the_query_first_and_leaves_the_field_the_second_time(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        click(cx, handle, "models.search");
        type_text(cx, handle, "large");
        assert_eq!(rows_shown(cx, handle), [1]);
        press(cx, handle, "escape");
        assert_eq!(query(cx, &rig), "");
        assert_eq!(rows_shown(cx, handle), [0, 1, 2, 3]);
        assert!(
            focused(cx, handle, "models.search"),
            "the first Esc keeps the caret"
        );
        press(cx, handle, "escape");
        assert!(
            !focused(cx, handle, "models.search"),
            "the second leaves it"
        );
    }

    #[gpui_kit::test]
    fn the_find_key_puts_the_caret_in_the_search_field(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        assert!(!focused(cx, handle, "models.search"));
        press(
            cx,
            handle,
            if cfg!(target_os = "macos") {
                "cmd-f"
            } else {
                "ctrl-f"
            },
        );
        assert!(focused(cx, handle, "models.search"));
    }

    #[gpui_kit::test]
    fn other_views_have_no_models_search_field(cx: &mut TestAppContext) {
        let rig = rig(cx);
        let handle = window(cx, &rig);
        assert!(present(cx, handle, "models.search"));
        click(cx, handle, "sidebar.history");
        assert!(!present(cx, handle, "models.search"));
    }
}
