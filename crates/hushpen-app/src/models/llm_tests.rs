use super::*;
use crate::storage;
use gpui_kit::{AppContext, Entity, TestAppContext};
use hushpen_store::data_dir::DataDir;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::VecDeque;

const SPEECH: &[u8] = b"speech model bytes";
const CLEANUP: &[u8] = b"cleanup model bytes, a little longer";

type Work = Rc<RefCell<VecDeque<Box<dyn FnOnce() + Send>>>>;

fn catalog() -> &'static Catalog {
    let entry = |id: &str, file: &str, body: &[u8]| {
        json!({
            "id": id,
            "name": id,
            "file": file,
            "bytes": body.len(),
            "sha256": model_files::hex(&Sha256::digest(body)),
            "url": format!("https://huggingface.co/test/{file}"),
            "license": "MIT",
            "languages": "multilingual",
            "default": id == "speech",
        })
    };
    let text = json!({
        "whisper": [entry("speech", "ggml-speech.bin", SPEECH)],
        "llm": [entry("qwen3.5-2b", "qwen3.5-2b.gguf", CLEANUP)],
    })
    .to_string();
    Box::leak(Box::new(Catalog::parse(&text).unwrap()))
}

struct Rig {
    models: Entity<Models>,
    storage: Rc<storage::Storage>,
    work: Work,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext, transfer: Arc<Transfer>) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
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

fn settle(cx: &mut TestAppContext, rig: &Rig) {
    loop {
        cx.run_until_parked();
        let Some(job) = rig.work.borrow_mut().pop_front() else {
            return;
        };
        job();
    }
}

fn downloads_the_file() -> Arc<Transfer> {
    Arc::new(
        |spec: &Spec, _: &AtomicBool, progress: &mut dyn FnMut(Progress)| {
            progress(Progress::Bytes {
                done: 10,
                total: 36,
            });
            std::fs::create_dir_all(spec.dest.parent().unwrap()).unwrap();
            std::fs::write(&spec.dest, CLEANUP).unwrap();
            model_files::write_stamp(&spec.dest, &spec.sha256).unwrap();
            Ok(Outcome::Done)
        },
    )
}

fn llm_state(cx: &mut TestAppContext, rig: &Rig) -> ModelState {
    rig.models
        .read_with(cx, |models, _| models.llm_state("qwen3.5-2b").unwrap())
}

#[gpui_kit::test]
fn cleanup_models_are_listed_apart_from_the_speech_models(cx: &mut TestAppContext) {
    let rig = rig(cx, downloads_the_file());
    rig.models.read_with(cx, |models, _| {
        assert_eq!(
            models
                .rows()
                .iter()
                .map(|row| &row.entry.id)
                .collect::<Vec<_>>(),
            ["speech"]
        );
        assert_eq!(
            models
                .llm_rows()
                .iter()
                .map(|row| &row.entry.id)
                .collect::<Vec<_>>(),
            ["qwen3.5-2b"]
        );
        let state = models.state_json();
        assert_eq!(state["models"].as_array().unwrap().len(), 1);
        assert_eq!(state["llm_models"][0]["id"], "qwen3.5-2b");
        assert_eq!(state["llm_models"][0]["state"], "not_downloaded");
    });
}

#[gpui_kit::test]
fn a_cleanup_download_lands_in_models_llm_with_a_verified_stamp(cx: &mut TestAppContext) {
    let rig = rig(cx, downloads_the_file());
    rig.models
        .update(cx, |models, cx| models.download("qwen3.5-2b", cx))
        .unwrap();
    assert!(matches!(
        llm_state(cx, &rig),
        ModelState::Downloading { .. }
    ));
    settle(cx, &rig);

    assert_eq!(llm_state(cx, &rig), ModelState::Ready);
    let path = rig.storage.data.llm_models_dir().join("qwen3.5-2b.gguf");
    assert!(path.is_file());
    let stamp = model_files::read_stamp(&path).expect("a .verified stamp");
    assert_eq!(stamp.hash, model_files::hex(&Sha256::digest(CLEANUP)));
    assert!(
        !rig.storage
            .data
            .whisper_models_dir()
            .join("qwen3.5-2b.gguf")
            .exists()
    );
}

#[gpui_kit::test]
fn a_cleanup_model_is_never_a_speech_choice(cx: &mut TestAppContext) {
    let rig = rig(cx, downloads_the_file());
    rig.models
        .update(cx, |models, cx| models.download("qwen3.5-2b", cx))
        .unwrap();
    settle(cx, &rig);
    let refused = rig
        .models
        .update(cx, |models, cx| models.select("qwen3.5-2b", cx));
    assert!(refused.is_err());
    assert_eq!(rig.models.read_with(cx, |models, _| models.active()), "");
}

#[gpui_kit::test]
fn a_deleted_cleanup_file_shows_as_not_downloaded_again(cx: &mut TestAppContext) {
    let rig = rig(cx, downloads_the_file());
    rig.models
        .update(cx, |models, cx| models.download("qwen3.5-2b", cx))
        .unwrap();
    settle(cx, &rig);
    assert_eq!(llm_state(cx, &rig), ModelState::Ready);

    model_files::remove(&rig.storage.data.llm_models_dir().join("qwen3.5-2b.gguf")).unwrap();
    cx.executor().advance_clock(PRESENCE_POLL * 2);
    cx.run_until_parked();

    assert_eq!(llm_state(cx, &rig), ModelState::NotDownloaded);
}

#[gpui_kit::test]
fn a_present_cleanup_file_is_hashed_at_start_and_ready(cx: &mut TestAppContext) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Rc::new(storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    std::fs::create_dir_all(storage.data.llm_models_dir()).unwrap();
    std::fs::write(
        storage.data.llm_models_dir().join("qwen3.5-2b.gguf"),
        CLEANUP,
    )
    .unwrap();
    let work: Work = Rc::default();
    let queue = Rc::clone(&work);
    let spawn: Spawner = Rc::new(move |job| {
        queue.borrow_mut().push_back(job);
        Ok(())
    });
    let models = cx.new(|cx| {
        Models::new(
            Rc::clone(&storage),
            catalog(),
            downloads_the_file(),
            spawn,
            None,
            cx,
        )
    });
    let rig = Rig {
        models,
        storage,
        work,
        _tmp: tmp,
    };
    assert_eq!(llm_state(cx, &rig), ModelState::Verifying);
    settle(cx, &rig);
    assert_eq!(llm_state(cx, &rig), ModelState::Ready);
}
