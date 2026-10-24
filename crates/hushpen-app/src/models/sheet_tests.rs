//! The grouped Models view, the details sheet, voice models with several files, the free-disk
//! check, and the start-up sweep of old partial downloads.

use super::testkit::{self, Rig, settle};
use super::*;
use crate::shell::Shell;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Point, TestAppContext, WindowBounds, WindowOptions,
    px, size,
};
use hushpen_core::error::MODEL_NO_SPACE;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::SystemTime;

const SPEECH: &[u8] = b"speech model bytes";
const CLEANUP: &[u8] = b"cleanup model bytes";
const VOICE_A: &[u8] = b"voice model weights";
const VOICE_B: &[u8] = b"voice table";

fn sha(body: &[u8]) -> String {
    model_files::hex(&Sha256::digest(body))
}

fn details(speed: u8) -> Value {
    json!({"description": "What it is.", "speed": speed, "accuracy": 3, "ram_bytes": 300_000_000,
        "recommended_for": "Tests", "source_page": "https://huggingface.co/test/repo"})
}

/// One speech model of `bytes` bytes, a cleanup model, and a voice model of two files.
fn catalog(bytes: u64) -> &'static Catalog {
    let single = |id: &str, kind: &str, file: &str, body: &[u8], bytes: u64| {
        let mut entry = json!({"id": id, "kind": kind, "name": id.to_uppercase(), "file": file,
            "bytes": bytes, "sha256": sha(body), "url": format!("https://huggingface.co/test/repo/resolve/main/{file}"),
            "license": "MIT", "languages": "english", "default": true});
        entry
            .as_object_mut()
            .unwrap()
            .extend(details(4).as_object().unwrap().clone());
        entry
    };
    let file = |name: &str, body: &[u8]| {
        json!({"file": name, "bytes": body.len(), "sha256": sha(body),
            "url": format!("https://huggingface.co/test/repo/resolve/main/{name}")})
    };
    let mut voice = json!({"id": "voice", "kind": "voice", "name": "Voice", "license": "Apache-2.0",
        "languages": "english", "default": true,
        "files": [file("a.onnx", VOICE_A), file("b.bin", VOICE_B)]});
    voice
        .as_object_mut()
        .unwrap()
        .extend(details(2).as_object().unwrap().clone());
    testkit::leak(json!({
        "schema": 2,
        "whisper": [single("speech", "speech", "ggml-speech.bin", SPEECH, bytes)],
        "llm": [single("qwen3.5-2b", "cleanup", "qwen3.5-2b.gguf", CLEANUP, CLEANUP.len() as u64)],
        "voice": [voice],
    }))
}

fn body_of(spec: &Spec) -> &'static [u8] {
    [
        ("ggml-speech.bin", SPEECH),
        ("qwen3.5-2b.gguf", CLEANUP),
        ("a.onnx", VOICE_A),
        ("b.bin", VOICE_B),
    ]
    .into_iter()
    .find(|(name, _)| spec.url.ends_with(name))
    .map(|(_, body)| body)
    .unwrap()
}

/// A transfer that reports half of each file, then writes it with its stamp, and records the
/// files it fetched.
fn recording(fetched: Arc<std::sync::Mutex<Vec<String>>>) -> Arc<Transfer> {
    Arc::new(
        move |spec: &Spec, _: &AtomicBool, progress: &mut dyn FnMut(Progress)| {
            fetched
                .lock()
                .unwrap()
                .push(spec.dest.display().to_string());
            progress(Progress::Bytes {
                done: spec.bytes / 2,
                total: spec.bytes,
            });
            progress(Progress::Verifying);
            let body = body_of(spec);
            std::fs::create_dir_all(spec.dest.parent().unwrap()).unwrap();
            std::fs::write(&spec.dest, body).unwrap();
            model_files::write_stamp(&spec.dest, &spec.sha256).unwrap();
            Ok(Outcome::Done)
        },
    )
}

fn rig(cx: &mut TestAppContext) -> (Rig, Arc<std::sync::Mutex<Vec<String>>>) {
    let fetched = Arc::default();
    let rig = testkit::rig(
        cx,
        catalog(SPEECH.len() as u64),
        recording(Arc::clone(&fetched)),
        |_| {},
    );
    (rig, fetched)
}

fn voice_dir(rig: &Rig) -> PathBuf {
    rig.storage.data.tts_models_dir().join("voice")
}

fn voice_state(cx: &mut TestAppContext, rig: &Rig) -> ModelState {
    rig.models
        .read_with(cx, |models, _| models.of_kind(ModelKind::Voice).1[0].state)
}

#[gpui_kit::test]
fn every_kind_has_its_rows_in_view_order_and_the_recommended_ones_are_marked(
    cx: &mut TestAppContext,
) {
    let (rig, _) = rig(cx);
    rig.models.read_with(cx, |models, _| {
        let rows: Vec<_> = models
            .all_rows()
            .iter()
            .map(|row| (row.entry.id.as_str(), row.entry.kind, row.recommended))
            .collect();
        assert_eq!(
            rows,
            [
                ("speech", ModelKind::Speech, true),
                ("qwen3.5-2b", ModelKind::Cleanup, true),
                ("voice", ModelKind::Voice, true),
            ]
        );
        assert_eq!(models.of_kind(ModelKind::Voice).0, 2);
        let state = models.state_json();
        assert_eq!(
            state["voice_models"][0]["files"],
            json!(["a.onnx", "b.bin"])
        );
        assert_eq!(state["llm_models"][0]["recommended"], true);
    });
}

#[gpui_kit::test]
fn a_voice_model_downloads_every_file_into_its_folder_as_one_download(cx: &mut TestAppContext) {
    let (rig, fetched) = rig(cx);
    rig.models
        .update(cx, |models, cx| models.download("voice", cx))
        .unwrap();
    let total = (VOICE_A.len() + VOICE_B.len()) as u64;
    assert_eq!(
        voice_state(cx, &rig),
        ModelState::Downloading { done: 0, total }
    );
    settle(cx, &rig);
    assert_eq!(voice_state(cx, &rig), ModelState::Ready);
    assert_eq!(fetched.lock().unwrap().len(), 2);
    for (name, body) in [("a.onnx", VOICE_A), ("b.bin", VOICE_B)] {
        let path = voice_dir(&rig).join(name);
        assert_eq!(std::fs::read(&path).unwrap(), body);
        assert!(
            model_files::read_stamp(&path).is_some(),
            "{name} is stamped"
        );
    }

    rig.models
        .update(cx, |models, cx| models.delete("voice", cx))
        .unwrap();
    assert_eq!(voice_state(cx, &rig), ModelState::NotDownloaded);
    assert!(!voice_dir(&rig).exists(), "the emptied folder goes too");
}

#[test]
fn the_files_of_one_download_report_as_one_progress() {
    let specs: Vec<Spec> = [10, 30]
        .into_iter()
        .map(|bytes| Spec {
            url: String::new(),
            bytes,
            sha256: String::new(),
            partial: PathBuf::new(),
            dest: PathBuf::new(),
        })
        .collect();
    let transfer = |spec: &Spec, _: &AtomicBool, progress: &mut dyn FnMut(Progress)| {
        progress(Progress::Bytes {
            done: spec.bytes,
            total: spec.bytes,
        });
        progress(Progress::Verifying);
        Ok(Outcome::Done)
    };
    let mut steps = Vec::new();
    let outcome = files::run_download(&specs, &transfer, &AtomicBool::new(false), &mut |step| {
        steps.push(step)
    });
    assert_eq!(outcome, Ok(Outcome::Done));
    assert_eq!(
        steps,
        [
            Progress::Bytes {
                done: 10,
                total: 40
            },
            Progress::Bytes {
                done: 10,
                total: 40
            },
            Progress::Bytes {
                done: 40,
                total: 40
            },
            Progress::Verifying,
        ],
        "only the last file shows the check"
    );
}

#[gpui_kit::test]
fn only_the_missing_files_of_a_voice_model_are_fetched(cx: &mut TestAppContext) {
    let fetched: Arc<std::sync::Mutex<Vec<String>>> = Arc::default();
    let rig = testkit::rig(
        cx,
        catalog(SPEECH.len() as u64),
        recording(Arc::clone(&fetched)),
        |storage| {
            let dir = storage.data.tts_models_dir().join("voice");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("a.onnx"), VOICE_A).unwrap();
            model_files::write_stamp(&dir.join("a.onnx"), &sha(VOICE_A)).unwrap();
        },
    );
    assert_eq!(voice_state(cx, &rig), ModelState::NotDownloaded);
    rig.models
        .update(cx, |models, cx| models.download("voice", cx))
        .unwrap();
    settle(cx, &rig);
    assert_eq!(voice_state(cx, &rig), ModelState::Ready);
    let fetched = fetched.lock().unwrap();
    assert_eq!(fetched.len(), 1);
    assert!(fetched[0].ends_with("b.bin"), "{fetched:?}");
}

#[gpui_kit::test]
fn a_download_larger_than_the_free_space_is_refused_before_it_starts(cx: &mut TestAppContext) {
    let rig = testkit::rig(cx, catalog(u64::MAX / 4), recording(Arc::default()), |_| {});
    let refused = rig
        .models
        .update(cx, |models, cx| models.download("speech", cx));
    assert!(refused.unwrap_err().starts_with(MODEL_NO_SPACE));
    assert!(rig.work.borrow().is_empty(), "no worker starts");
    rig.models.read_with(cx, |models, _| {
        assert_eq!(models.rows()[0].state, ModelState::NotDownloaded);
        let notice = models.notice().unwrap();
        assert_eq!(notice.code, MODEL_NO_SPACE);
        assert!(notice.message.contains("plus 10%"), "{}", notice.message);
    });
}

#[gpui_kit::test]
fn partials_older_than_a_week_are_swept_at_start(cx: &mut TestAppContext) {
    let week = Duration::from_secs(7 * 24 * 3600);
    let rig = testkit::rig(
        cx,
        catalog(SPEECH.len() as u64),
        recording(Arc::default()),
        |storage| {
            let dir = storage.data.downloads_dir();
            std::fs::create_dir_all(&dir).unwrap();
            for (name, age) in [
                ("old.bin.download", week * 2),
                ("new.bin.download", week / 2),
            ] {
                std::fs::write(dir.join(name), b"part").unwrap();
                std::fs::File::options()
                    .write(true)
                    .open(dir.join(name))
                    .unwrap()
                    .set_modified(SystemTime::now() - age)
                    .unwrap();
            }
        },
    );
    assert_eq!(rig.downloads(), ["new.bin.download"]);
}

/// The main window on the Models view, with the library attached.
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

fn click(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, handle);
}

fn press(cx: &mut TestAppContext, handle: AnyWindowHandle, key: &str) {
    cx.update_window(handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    render(cx, handle);
}

fn present(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> bool {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn label(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> String {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(handle, |_, window, _| {
        window.find(id).label().unwrap_or_default().to_owned()
    })
    .unwrap()
}

fn focused(cx: &mut TestAppContext, handle: AnyWindowHandle, id: &str) -> bool {
    let id = gpui_kit::SharedString::from(id.to_owned());
    cx.update_window(handle, |_, window, _| {
        window.try_find(id).and_then(|found| found.focused()) == Some(true)
    })
    .unwrap()
}

#[gpui_kit::test]
fn the_view_has_a_section_for_each_kind_and_only_speech_rows_offer_use(cx: &mut TestAppContext) {
    let (rig, _) = rig(cx);
    for id in ["speech", "qwen3.5-2b", "voice"] {
        rig.models
            .update(cx, |models, cx| models.download(id, cx))
            .unwrap();
    }
    settle(cx, &rig);
    let handle = window(cx, &rig);
    for kind in ["speech", "cleanup", "voice"] {
        assert!(
            present(cx, handle, &format!("models.heading.{kind}")),
            "{kind}"
        );
        assert!(
            present(cx, handle, &format!("models.list.{kind}")),
            "{kind}"
        );
        assert!(
            present(cx, handle, &format!("models.help.{kind}")),
            "{kind}"
        );
    }
    for index in 0..3 {
        assert!(present(cx, handle, &format!("models.info.{index}")));
        assert!(present(cx, handle, &format!("models.recommended.{index}")));
        assert!(present(cx, handle, &format!("models.delete.{index}")));
    }
    assert!(
        present(cx, handle, "models.select.0"),
        "a speech model is put to use"
    );
    assert!(
        !present(cx, handle, "models.select.1"),
        "Settings picks the cleanup model"
    );
    assert!(
        !present(cx, handle, "models.select.2"),
        "there is one voice model"
    );
    assert_eq!(label(cx, handle, "models.info.2"), "Details of Voice");
}

#[gpui_kit::test]
fn the_details_sheet_shows_the_facts_and_its_actions_work(cx: &mut TestAppContext) {
    let (rig, _) = rig(cx);
    let handle = window(cx, &rig);
    assert!(!present(cx, handle, "models.sheet"));
    click(cx, handle, "models.info.2");

    assert!(present(cx, handle, "models.sheet"));
    assert!(focused(cx, handle, "models.sheet.done"), "Done takes focus");
    assert_eq!(label(cx, handle, "models.sheet.title"), "Voice");
    assert_eq!(label(cx, handle, "models.sheet.speed"), "Slow (2 of 5)");
    assert_eq!(label(cx, handle, "models.sheet.accuracy"), "Good (3 of 5)");
    assert_eq!(label(cx, handle, "models.sheet.memory"), "About 300 MB");
    assert_eq!(label(cx, handle, "models.sheet.size"), "0 KB in 2 files");
    assert_eq!(label(cx, handle, "models.sheet.languages"), "English only");
    assert_eq!(label(cx, handle, "models.sheet.license"), "Apache-2.0");
    assert_eq!(label(cx, handle, "models.sheet.status"), "Not downloaded");
    assert!(label(cx, handle, "models.sheet.location").ends_with("models/tts/voice"));
    assert_eq!(
        label(cx, handle, "models.sheet.checksum"),
        format!("{}  a.onnx, {}  b.bin", sha(VOICE_A), sha(VOICE_B))
    );

    click(cx, handle, "models.sheet.source");
    click(cx, handle, "models.sheet.show");
    let opened = rig.opened.0.borrow().clone();
    assert_eq!(opened[0], "https://huggingface.co/test/repo");
    assert!(
        voice_dir(&rig).starts_with(&opened[1]) && PathBuf::from(&opened[1]).is_dir(),
        "the closest folder that exists opens: {}",
        opened[1]
    );

    click(cx, handle, "models.sheet.copy");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(
        copied.as_deref(),
        Some(format!("{}  a.onnx\n{}  b.bin", sha(VOICE_A), sha(VOICE_B)).as_str())
    );
    let state = rig.models.read_with(cx, |models, _| models.state_json());
    assert_eq!(state["details"], "voice");
}

#[gpui_kit::test]
fn tab_stays_in_the_sheet_and_esc_gives_focus_back_to_the_info_button(cx: &mut TestAppContext) {
    let (rig, _) = rig(cx);
    let handle = window(cx, &rig);
    click(cx, handle, "models.info.0");
    let inside = [
        "models.sheet.source",
        "models.sheet.show",
        "models.sheet.copy",
        "models.sheet.done",
    ];
    for _ in 0..6 {
        press(cx, handle, "tab");
        assert!(
            inside.iter().any(|id| focused(cx, handle, id)),
            "Tab left the sheet"
        );
    }
    press(cx, handle, "escape");
    assert!(!present(cx, handle, "models.sheet"));
    assert!(focused(cx, handle, "models.info.0"));

    click(cx, handle, "models.info.0");
    click(cx, handle, "models.sheet.done");
    assert!(!present(cx, handle, "models.sheet"));
    assert!(focused(cx, handle, "models.info.0"));
}
