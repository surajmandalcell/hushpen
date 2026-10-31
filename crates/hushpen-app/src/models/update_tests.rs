//! A model whose files on disk are an earlier pin of the current files updates by itself.

use super::testkit::{self, Rig, settle};
use super::*;
use gpui_kit::TestAppContext;
use sha2::{Digest, Sha256};
use std::sync::Mutex;

const OLD: &[u8] = b"voice weights v1";
const NEW: &[u8] = b"voice weights, the fixed v2";
const TABLE: &[u8] = b"voice table";

fn sha(body: &[u8]) -> String {
    model_files::hex(&Sha256::digest(body))
}

/// A voice model of two files; the first replaces [`OLD`].
fn catalog() -> &'static Catalog {
    let file = |name: &str, body: &[u8], replaces: Value| {
        json!({"file": name, "bytes": body.len(), "sha256": sha(body), "replaces": replaces,
            "url": format!("https://huggingface.co/test/repo/resolve/main/{name}")})
    };
    testkit::leak(json!({
        "whisper": [],
        "voice": [{"id": "voice", "kind": "voice", "name": "Voice", "license": "MIT",
            "languages": "english",
            "files": [
                file("a.onnx", NEW, json!([{"bytes": OLD.len(), "sha256": sha(OLD)}])),
                file("b.bin", TABLE, json!([])),
            ]}],
    }))
}

fn write(storage: &Storage, name: &str, body: &[u8], stamp: Option<&str>) {
    let dir = storage.data.tts_models_dir().join("voice");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
    if let Some(hash) = stamp {
        model_files::write_stamp(&dir.join(name), hash).unwrap();
    }
}

/// A transfer that records the files it fetched and writes them as the downloader does.
fn fetching(log: &Arc<Mutex<Vec<String>>>) -> Arc<Transfer> {
    let log = Arc::clone(log);
    Arc::new(
        move |spec: &Spec, _: &AtomicBool, progress: &mut dyn FnMut(Progress)| {
            let name = spec
                .dest
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            progress(Progress::Bytes {
                done: 5,
                total: spec.bytes,
            });
            log.lock().unwrap().push(name.clone());
            std::fs::write(&spec.dest, if name == "a.onnx" { NEW } else { TABLE }).unwrap();
            model_files::write_stamp(&spec.dest, &spec.sha256).unwrap();
            Ok(Outcome::Done)
        },
    )
}

fn rig(cx: &mut TestAppContext, transfer: Arc<Transfer>, a: &[u8], a_stamp: Option<&str>) -> Rig {
    testkit::rig(cx, catalog(), transfer, |storage| {
        write(storage, "a.onnx", a, a_stamp);
        write(storage, "b.bin", TABLE, Some(&sha(TABLE)));
    })
}

fn row(cx: &mut TestAppContext, rig: &Rig) -> (ModelState, bool) {
    rig.models.read_with(cx, |models, _| {
        let row = &models.all_rows()[0];
        (row.state, row.updating)
    })
}

fn notice(cx: &mut TestAppContext, rig: &Rig) -> Option<&'static str> {
    rig.models
        .read_with(cx, |models, _| models.notice().map(|notice| notice.code))
}

#[gpui_kit::test]
fn an_old_pin_updates_in_the_background_and_only_the_changed_file_is_fetched(
    cx: &mut TestAppContext,
) {
    let log = Arc::default();
    let rig = rig(cx, fetching(&log), OLD, Some(&sha(OLD)));
    let (state, updating) = row(cx, &rig);
    assert!(matches!(state, ModelState::Downloading { .. }), "{state:?}");
    assert!(updating);
    assert_eq!(notice(cx, &rig), None, "an update is not an error");

    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::Ready, false));
    assert_eq!(notice(cx, &rig), None);
    assert_eq!(*log.lock().unwrap(), ["a.onnx"]);
    let folder = rig.storage.data.tts_models_dir().join("voice");
    assert_eq!(std::fs::read(folder.join("a.onnx")).unwrap(), NEW);
}

#[gpui_kit::test]
fn an_old_file_without_a_stamp_updates_too(cx: &mut TestAppContext) {
    let log = Arc::default();
    let rig = rig(cx, fetching(&log), OLD, None);
    assert!(row(cx, &rig).1);
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::Ready, false));
}

#[gpui_kit::test]
fn the_state_json_tells_that_a_row_is_updating(cx: &mut TestAppContext) {
    let rig = rig(cx, fetching(&Arc::default()), OLD, Some(&sha(OLD)));
    let state = rig.models.read_with(cx, |models, _| models.state_json());
    assert_eq!(state["voice_models"][0]["state"], "downloading");
    assert_eq!(state["voice_models"][0]["updating"], true);
}

#[gpui_kit::test]
fn a_file_that_matches_no_pin_still_fails_verification(cx: &mut TestAppContext) {
    let log = Arc::default();
    let rig = rig(cx, fetching(&log), b"some other file", None);
    assert_eq!(row(cx, &rig), (ModelState::Failed, false));
    assert_eq!(notice(cx, &rig), Some(MODEL_HASH_MISMATCH));
    assert!(log.lock().unwrap().is_empty());
}

#[gpui_kit::test]
fn a_stamp_with_another_hash_is_not_the_old_pin(cx: &mut TestAppContext) {
    let rig = rig(cx, fetching(&Arc::default()), OLD, Some(&sha(b"other")));
    assert_eq!(row(cx, &rig), (ModelState::Failed, false));
}

#[gpui_kit::test]
fn an_old_file_beside_a_damaged_one_is_not_an_update(cx: &mut TestAppContext) {
    let rig = testkit::rig(cx, catalog(), fetching(&Arc::default()), |storage| {
        write(storage, "a.onnx", OLD, Some(&sha(OLD)));
        write(storage, "b.bin", b"cut", None);
    });
    assert_eq!(row(cx, &rig), (ModelState::Failed, false));
}

#[gpui_kit::test]
fn an_update_the_network_stops_is_quiet_and_can_be_downloaded_again(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        Arc::new(|_: &Spec, _: &AtomicBool, _: &mut dyn FnMut(Progress)| {
            Err(Failure {
                code: DOWNLOAD_FAILED,
                detail: "offline".into(),
            })
        }),
        OLD,
        Some(&sha(OLD)),
    );
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));
    assert_eq!(notice(cx, &rig), None);
    rig.models
        .update(cx, |models, cx| models.download("voice", cx))
        .unwrap();
    assert!(
        !row(cx, &rig).1,
        "a download the user starts is not an update"
    );
}

#[gpui_kit::test]
fn an_update_whose_download_does_not_match_its_hash_says_so(cx: &mut TestAppContext) {
    let rig = rig(
        cx,
        Arc::new(|_: &Spec, _: &AtomicBool, _: &mut dyn FnMut(Progress)| {
            Err(Failure {
                code: MODEL_HASH_MISMATCH,
                detail: "wrong".into(),
            })
        }),
        OLD,
        Some(&sha(OLD)),
    );
    settle(cx, &rig);
    assert_eq!(notice(cx, &rig), Some(MODEL_HASH_MISMATCH));
}

#[gpui_kit::test]
fn cancelling_an_update_leaves_a_plain_not_downloaded_row(cx: &mut TestAppContext) {
    let rig = rig(cx, fetching(&Arc::default()), OLD, Some(&sha(OLD)));
    rig.models
        .update(cx, |models, cx| models.cancel("voice", cx))
        .unwrap();
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));
}

/// Fails with a network error while `offline` is set, and fetches like [`fetching`] after.
fn flaky(offline: &Arc<AtomicBool>, log: &Arc<Mutex<Vec<String>>>) -> Arc<Transfer> {
    let (offline, online) = (Arc::clone(offline), fetching(log));
    Arc::new(
        move |spec: &Spec, cancel: &AtomicBool, progress: &mut dyn FnMut(Progress)| {
            if offline.load(Ordering::Relaxed) {
                return Err(Failure {
                    code: DOWNLOAD_FAILED,
                    detail: "offline".into(),
                });
            }
            online(spec, cancel, progress)
        },
    )
}

fn open_models(cx: &mut TestAppContext, rig: &Rig) {
    rig.models.update(cx, |models, cx| models.opened(cx));
}

#[gpui_kit::test]
fn opening_models_retries_an_update_the_network_stopped(cx: &mut TestAppContext) {
    let (offline, log) = (Arc::new(AtomicBool::new(true)), Arc::default());
    let rig = rig(cx, flaky(&offline, &log), OLD, Some(&sha(OLD)));
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));

    offline.store(false, Ordering::Relaxed);
    open_models(cx, &rig);
    let (state, updating) = row(cx, &rig);
    assert!(matches!(state, ModelState::Downloading { .. }), "{state:?}");
    assert!(updating, "the retry shows Updating, not a plain download");
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::Ready, false));
    assert_eq!(notice(cx, &rig), None);
    assert_eq!(*log.lock().unwrap(), ["a.onnx"]);
}

#[gpui_kit::test]
fn opening_models_again_while_the_retry_runs_starts_nothing_more(cx: &mut TestAppContext) {
    let (offline, log) = (Arc::new(AtomicBool::new(true)), Arc::default());
    let rig = rig(cx, flaky(&offline, &log), OLD, Some(&sha(OLD)));
    settle(cx, &rig);
    offline.store(false, Ordering::Relaxed);
    open_models(cx, &rig);
    open_models(cx, &rig);
    settle(cx, &rig);
    assert_eq!(*log.lock().unwrap(), ["a.onnx"]);
}

#[gpui_kit::test]
fn opening_models_while_still_offline_stays_quiet_and_tries_next_time(cx: &mut TestAppContext) {
    let (offline, log) = (Arc::new(AtomicBool::new(true)), Arc::default());
    let rig = rig(cx, flaky(&offline, &log), OLD, Some(&sha(OLD)));
    settle(cx, &rig);
    open_models(cx, &rig);
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));
    assert_eq!(notice(cx, &rig), None);

    offline.store(false, Ordering::Relaxed);
    open_models(cx, &rig);
    settle(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::Ready, false));
}

#[gpui_kit::test]
fn opening_models_does_not_retry_an_update_the_user_cancelled(cx: &mut TestAppContext) {
    let rig = rig(cx, fetching(&Arc::default()), OLD, Some(&sha(OLD)));
    rig.models
        .update(cx, |models, cx| models.cancel("voice", cx))
        .unwrap();
    settle(cx, &rig);
    open_models(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));
}

#[gpui_kit::test]
fn opening_models_does_not_touch_a_model_that_never_had_an_update(cx: &mut TestAppContext) {
    let rig = testkit::rig(cx, catalog(), fetching(&Arc::default()), |_| {});
    open_models(cx, &rig);
    assert_eq!(row(cx, &rig), (ModelState::NotDownloaded, false));
}

#[test]
fn the_status_word_is_updating_with_the_percent_and_never_red() {
    let palette = crate::theme::Palette::of(crate::theme::Appearance::Dark);
    let state = ModelState::Downloading {
        done: 42,
        total: 100,
    };
    let (text, tone) = panel::status_text(palette, state, false, true);
    assert_eq!(text, "Updating 42%");
    assert_eq!(tone, palette.label);
    let starting = ModelState::Downloading { done: 0, total: 0 };
    assert_eq!(
        panel::status_text(palette, starting, false, true).0,
        "Updating"
    );
    assert_eq!(panel::status_text(palette, state, false, false).0, "42%");
}
