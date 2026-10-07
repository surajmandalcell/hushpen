use super::*;
use crate::storage;
use gpui_kit::{AppContext, Entity, TestAppContext};
use hushpen_audio::FeedInfo;
use hushpen_store::data_dir::DataDir;
use serde_json::json;
use std::sync::Mutex;

#[derive(Default)]
struct Fake {
    start_error: Mutex<Option<CaptureError>>,
    started: Mutex<Vec<(String, PathBuf)>>,
    stopped: Arc<Mutex<u32>>,
}

struct FakeSession {
    path: PathBuf,
    stopped: Arc<Mutex<u32>>,
}

impl MicBackend for Fake {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        Ok(Vec::new())
    }

    fn start(
        &self,
        device: &str,
        path: PathBuf,
        _sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        if let Some(error) = self.start_error.lock().unwrap().clone() {
            return Err(error);
        }
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"wav").unwrap();
        self.started
            .lock()
            .unwrap()
            .push((device.to_string(), path.clone()));
        Ok(Box::new(FakeSession {
            path,
            stopped: Arc::clone(&self.stopped),
        }))
    }
}

impl MicSession for FakeSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError> {
        *self.stopped.lock().unwrap() += 1;
        Ok(Finished {
            path: self.path,
            samples: 16_000,
            duration_ms: 1000,
        })
    }

    fn feed_wav(&self, _path: &Path) -> Result<FeedInfo, CaptureError> {
        Ok(FeedInfo {
            duration_ms: 1000,
            sample_rate: 44_100,
            channels: 2,
        })
    }
}

fn device(id: &str, name: &str, is_default: bool) -> InputDevice {
    InputDevice {
        id: id.into(),
        name: name.into(),
        is_default,
    }
}

fn virtual_mics() -> Vec<InputDevice> {
    vec![
        device("pulseaudio:vmic_src", "Virtual mic", true),
        device("pulseaudio:vmic2_src", "Second mic", false),
    ]
}

struct Rig {
    mic: Entity<Mic>,
    backend: Arc<Fake>,
    storage: Rc<storage::Storage>,
    root: PathBuf,
    _tmp: tempfile::TempDir,
}

fn rig(cx: &mut TestAppContext, saved: Option<&str>) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("data");
    let storage = Rc::new(storage::open(DataDir::open(&root).unwrap()).unwrap());
    if let Some(saved) = saved {
        storage.settings.set(SETTING, json!(saved)).unwrap();
    }
    let backend = Arc::new(Fake::default());
    let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), backend.clone(), cx));
    Rig {
        mic,
        backend,
        storage,
        root,
        _tmp: tmp,
    }
}

fn devices(cx: &mut TestAppContext, rig: &Rig, list: Vec<InputDevice>) {
    rig.mic
        .update(cx, |mic, cx| mic.handle(MicEvent::Devices(Ok(list)), cx));
}

fn state_json(cx: &mut TestAppContext, rig: &Rig) -> Value {
    rig.mic.read_with(cx, |mic, _| mic.state_json())
}

#[gpui_kit::test]
fn a_fresh_start_follows_the_default_with_no_notice(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    devices(cx, &rig, virtual_mics());
    let state = state_json(cx, &rig);
    assert_eq!(state["saved_device"], "default");
    assert_eq!(state["active_device"], "default");
    assert_eq!(state["notice"], Value::Null);
    assert_eq!(state["devices"].as_array().unwrap().len(), 2);
    assert_eq!(state["devices"][0]["default"], true);
    assert_eq!(state["state"], "idle");
}

#[gpui_kit::test]
fn the_selected_mic_is_saved_and_survives_a_restart(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    devices(cx, &rig, virtual_mics());
    rig.mic
        .update(cx, |mic, cx| mic.select("pulseaudio:vmic2_src", cx))
        .unwrap();
    assert_eq!(
        rig.storage.settings.get(SETTING),
        Some(json!("pulseaudio:vmic2_src"))
    );

    let reopened = Rc::new(storage::open(DataDir::open(&rig.root).unwrap()).unwrap());
    let backend = Arc::new(Fake::default());
    let mic = cx.new(|cx| Mic::new(reopened, backend, cx));
    mic.update(cx, |mic, cx| {
        mic.handle(MicEvent::Devices(Ok(virtual_mics())), cx)
    });
    let state = mic.read_with(cx, |mic, _| mic.state_json());
    assert_eq!(state["active_device"], "pulseaudio:vmic2_src");
    assert_eq!(state["notice"], Value::Null);
}

#[gpui_kit::test]
fn an_unknown_microphone_cannot_be_selected(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    devices(cx, &rig, virtual_mics());
    let error = rig
        .mic
        .update(cx, |mic, cx| mic.select("pulseaudio:nope", cx))
        .unwrap_err();
    assert!(error.contains("nope"));
    assert_eq!(rig.storage.settings.get(SETTING), Some(json!("default")));
}

#[gpui_kit::test]
fn a_saved_mic_that_is_gone_falls_back_to_the_default_with_a_notice(cx: &mut TestAppContext) {
    let rig = rig(cx, Some("pulseaudio:vmic2_src"));
    devices(
        cx,
        &rig,
        vec![device("pulseaudio:vmic_src", "Virtual mic", true)],
    );
    let state = state_json(cx, &rig);
    assert_eq!(state["active_device"], "default");
    assert_eq!(state["notice"]["code"], "MIC_UNAVAILABLE");
    assert!(
        state["notice"]["message"]
            .as_str()
            .unwrap()
            .contains("selected microphone is not available")
    );
    assert_eq!(
        state["saved_device"], "pulseaudio:vmic2_src",
        "the choice is kept for when the mic returns"
    );

    rig.mic
        .update(cx, |mic, cx| mic.start(cx))
        .expect("capture starts on the default");
    assert_eq!(rig.backend.started.lock().unwrap()[0].0, "default");

    rig.mic.update(cx, |mic, cx| {
        let _ = mic.stop(true, cx);
        mic.handle(MicEvent::Devices(Ok(virtual_mics())), cx)
    });
    let state = state_json(cx, &rig);
    assert_eq!(
        state["notice"],
        Value::Null,
        "the notice clears when it returns"
    );
    assert_eq!(state["active_device"], "pulseaudio:vmic2_src");
}

#[gpui_kit::test]
fn starting_with_no_microphone_fails_with_a_notice_and_never_listens(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    *rig.backend.start_error.lock().unwrap() = Some(CaptureError::unavailable("none"));
    devices(cx, &rig, Vec::new());
    let error = rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap_err();
    assert_eq!(error.code, "MIC_UNAVAILABLE");
    let state = state_json(cx, &rig);
    assert_eq!(state["state"], "failed");
    assert_eq!(state["notice"]["code"], "MIC_UNAVAILABLE");
    assert!(
        state["notice"]["message"]
            .as_str()
            .unwrap()
            .contains("No microphone is available")
    );
    assert_eq!(state["session"], Value::Null);
}

#[gpui_kit::test]
fn a_session_collects_levels_and_stop_keeps_or_deletes_the_file(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    devices(cx, &rig, virtual_mics());
    let path = rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    assert_eq!(path.parent().unwrap(), rig.root.join("cache/sessions"));
    let generation = 1;
    rig.mic.update(cx, |mic, cx| {
        for level in [0.0, 0.4, 0.8] {
            mic.handle(
                MicEvent::Capture {
                    generation,
                    event: CaptureEvent::Level(level),
                },
                cx,
            );
        }
    });
    let state = state_json(cx, &rig);
    assert_eq!(state["state"], "listening");
    assert_eq!(state["level"], 0.8f32 as f64);
    let bars = state["levels"].as_array().unwrap();
    assert_eq!(bars.len(), METER_BARS);
    assert_eq!(bars[METER_BARS - 1], 0.8f32 as f64);
    assert_eq!(bars[0], 0.0);

    let finished = rig
        .mic
        .update(cx, |mic, cx| mic.stop(true, cx))
        .unwrap()
        .unwrap();
    assert!(finished.path.exists());
    let state = state_json(cx, &rig);
    assert_eq!(state["state"], "idle");
    assert_eq!(state["level"], 0.0);
    assert_eq!(state["last_session"], path.to_string_lossy().as_ref());

    rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    let second = rig
        .mic
        .update(cx, |mic, cx| mic.stop(false, cx))
        .unwrap()
        .unwrap();
    assert!(
        !second.path.exists(),
        "a test session is deleted unless kept"
    );
    assert_eq!(
        rig.mic.update(cx, |mic, cx| mic.stop(true, cx)).unwrap(),
        None
    );
}

#[gpui_kit::test]
fn a_mic_error_during_a_session_closes_the_file_and_shows_a_notice(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    devices(cx, &rig, virtual_mics());
    let path = rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    rig.mic.update(cx, |mic, cx| {
        mic.handle(
            MicEvent::Capture {
                generation: 1,
                event: CaptureEvent::Error(CaptureError::unavailable("removed")),
            },
            cx,
        )
    });
    let state = state_json(cx, &rig);
    assert_eq!(state["state"], "failed");
    assert_eq!(state["notice"]["code"], "MIC_UNAVAILABLE");
    assert!(
        state["notice"]["message"]
            .as_str()
            .unwrap()
            .contains("stopped working")
    );
    assert_eq!(
        *rig.backend.stopped.lock().unwrap(),
        1,
        "the WAV was finalized"
    );
    assert!(path.exists(), "the audio so far is kept");
    assert_eq!(state["last_session"], path.to_string_lossy().as_ref());
}

#[gpui_kit::test]
fn events_from_an_ended_session_are_ignored(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    rig.mic
        .update(cx, |mic, cx| mic.stop(true, cx))
        .unwrap()
        .unwrap();
    rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    rig.mic.update(cx, |mic, cx| {
        mic.handle(
            MicEvent::Capture {
                generation: 1,
                event: CaptureEvent::Error(CaptureError::unavailable("late")),
            },
            cx,
        );
        mic.handle(
            MicEvent::Capture {
                generation: 1,
                event: CaptureEvent::Level(0.9),
            },
            cx,
        );
    });
    let state = state_json(cx, &rig);
    assert_eq!(state["state"], "listening");
    assert_eq!(state["level"], 0.0);
    assert_eq!(state["notice"], Value::Null);
}

#[gpui_kit::test]
fn a_second_start_reuses_the_running_session(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    let first = rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    let second = rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    assert_eq!(first, second);
    assert_eq!(rig.backend.started.lock().unwrap().len(), 1);
}

#[gpui_kit::test]
fn feeding_a_wav_needs_a_running_session(cx: &mut TestAppContext) {
    let rig = rig(cx, None);
    let wav = rig.root.join("x.wav");
    let error = rig.mic.update(cx, |mic, _| mic.feed_wav(&wav)).unwrap_err();
    assert!(error.contains("start one first"), "{error}");
    rig.mic.update(cx, |mic, cx| mic.start(cx)).unwrap();
    let fed = rig.mic.update(cx, |mic, _| mic.feed_wav(&wav)).unwrap();
    assert_eq!(fed["duration_ms"], 1000);
}
