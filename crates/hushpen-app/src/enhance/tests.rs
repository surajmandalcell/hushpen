//! The Audio enhance row and its sheet in a window: the mode, the tuning page and its kept
//! sample, and "Find best settings" with a fake speech engine.

use super::*;
use crate::mic::{MicBackend, MicSession};
use crate::settings::{Parts as SettingsParts, Settings};
use crate::shell::Shell;
use crate::theme::space;
use crate::views::View;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Point, SharedString, TestAppContext, WindowBounds,
    WindowOptions, px, size,
};
use hushpen_audio::{CaptureError, EventSink, FeedInfo, Finished, InputDevice};
use hushpen_engine::{Failure, Transcription};
use hushpen_store::data_dir::DataDir;
use std::sync::Mutex;

/// Two seconds of a 440 Hz tone, a quarter of full scale.
fn tone() -> Vec<f32> {
    (0..2 * RATE as usize)
        .map(|n| 0.25 * (n as f32 * 440.0 * std::f32::consts::TAU / RATE as f32).sin())
        .collect()
}

struct FakeMic;

impl MicBackend for FakeMic {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        Ok(Vec::new())
    }

    fn start(
        &self,
        _device: &str,
        path: PathBuf,
        _sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        Ok(Box::new(FakeSession(path)))
    }
}

/// A take that recorded the tone.
struct FakeSession(PathBuf);

impl MicSession for FakeSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError> {
        std::fs::create_dir_all(self.0.parent().unwrap()).unwrap();
        write_wav(&self.0, RATE, &tone()).unwrap();
        Ok(Finished {
            path: self.0,
            samples: 2 * u64::from(RATE),
            duration_ms: 2_000,
        })
    }

    fn feed_wav(&self, _path: &Path) -> Result<FeedInfo, CaptureError> {
        Err(CaptureError {
            code: "test",
            detail: "no feed".into(),
        })
    }
}

/// How the fake engine hears a take.
#[derive(Clone, Copy)]
enum Ear {
    /// The louder the take, the fewer words it gets wrong.
    Loudness,
    /// Every take is heard the same.
    Same,
    Broken,
}

struct Rig {
    storage: Rc<Storage>,
    tuning: Entity<Tuning>,
    handle: AnyWindowHandle,
    runs: Arc<Mutex<Vec<TranscribeSpec>>>,
    _tmp: tempfile::TempDir,
}

/// The reference text with the first `wrong` words left out.
fn heard(wrong: usize) -> String {
    let text = SENTENCES.join(" ");
    text.split_whitespace()
        .skip(wrong)
        .collect::<Vec<_>>()
        .join(" ")
}

fn runner(ear: Ear, runs: Arc<Mutex<Vec<TranscribeSpec>>>) -> Runner {
    Arc::new(move |spec: TranscribeSpec, _| {
        let peak = read_samples(&spec.wav_path)
            .unwrap()
            .into_iter()
            .fold(0.0_f32, |peak, sample| peak.max(sample.abs()));
        runs.lock().unwrap().push(spec);
        match ear {
            Ear::Loudness => JobOutcome::Done(Transcription {
                text: heard(((1.0 - peak) * 10.0).round() as usize),
                language: "en".into(),
                segments: Vec::new(),
                audio_ms: 2_000,
                decode_ms: 10,
            }),
            Ear::Same => JobOutcome::Done(Transcription {
                text: heard(2),
                language: "en".into(),
                segments: Vec::new(),
                audio_ms: 2_000,
                decode_ms: 10,
            }),
            Ear::Broken => JobOutcome::Failed(Failure::new("ENGINE_UNAVAILABLE", "no model")),
        }
    })
}

/// The window on Settings, with the row and the sheet attached.
fn rig(cx: &mut TestAppContext, ear: Ear) -> Rig {
    let tmp = tempfile::tempdir().unwrap();
    let storage =
        Rc::new(crate::storage::open(DataDir::open(tmp.path().join("data")).unwrap()).unwrap());
    let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(FakeMic), cx));
    let runs = Arc::default();
    let tuning = cx.new(|cx| {
        let mut tuning = Tuning::new(
            Parts {
                storage: Rc::clone(&storage),
                mic: mic.clone(),
                run: runner(ear, Arc::clone(&runs)),
                // The search runs at once on the test thread; its messages arrive when the
                // test lets the app run.
                spawn: Rc::new(|job| {
                    job();
                    Ok(())
                }),
            },
            cx,
        );
        tuning.play_silently();
        tuning
    });
    let settings = cx.new(|cx| {
        Settings::new(
            SettingsParts {
                storage: Rc::clone(&storage),
                mic: mic.clone(),
                login: None,
                default_data_dir: Some(tmp.path().join("default")),
                restart: Some(Rc::new(|_| {})),
            },
            cx,
        )
    });
    settings.update(cx, |settings, cx| {
        settings.attach_tuning(tuning.clone(), cx)
    });
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
        shell.attach_settings(settings.clone(), cx);
        shell.select(View::Settings, cx);
    });
    let rig = Rig {
        storage,
        tuning,
        handle,
        runs,
        _tmp: tmp,
    };
    render(cx, &rig);
    rig
}

fn render(cx: &mut TestAppContext, rig: &Rig) {
    cx.run_until_parked();
    cx.update_window(rig.handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn click(cx: &mut TestAppContext, rig: &Rig, id: &str) {
    let id = SharedString::from(id.to_owned());
    cx.update_window(rig.handle, |_, window, cx| window.click(id, cx))
        .unwrap();
    render(cx, rig);
}

fn press(cx: &mut TestAppContext, rig: &Rig, key: &str) {
    cx.update_window(rig.handle, |_, window, cx| window.press(key, cx))
        .unwrap();
    render(cx, rig);
}

fn present(cx: &mut TestAppContext, rig: &Rig, id: &str) -> bool {
    let id = SharedString::from(id.to_owned());
    cx.update_window(rig.handle, |_, window, _| window.try_find(id).is_some())
        .unwrap()
}

fn label(cx: &mut TestAppContext, rig: &Rig, id: &str) -> String {
    let id = SharedString::from(id.to_owned());
    cx.update_window(rig.handle, |_, window, _| {
        window.find(id).label().unwrap_or_default().to_owned()
    })
    .unwrap()
}

fn selected(cx: &mut TestAppContext, rig: &Rig, id: &str) -> bool {
    let id = SharedString::from(id.to_owned());
    cx.update_window(rig.handle, |_, window, _| {
        window.find(id).selected() == Some(true)
    })
    .unwrap()
}

fn focused(cx: &mut TestAppContext, rig: &Rig, id: &str) -> bool {
    let id = SharedString::from(id.to_owned());
    cx.update_window(rig.handle, |_, window, _| {
        window.try_find(id).and_then(|found| found.focused()) == Some(true)
    })
    .unwrap()
}

fn state(cx: &mut TestAppContext, rig: &Rig) -> Value {
    rig.tuning
        .read_with(cx, |tuning, cx| tuning.hook_state_json(cx))
}

#[gpui_kit::test]
fn the_row_offers_off_auto_and_custom_and_starts_on_auto(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    assert_eq!(
        label(cx, &rig, "settings.audio.enhance.row.title"),
        "Audio enhance"
    );
    assert_eq!(
        label(cx, &rig, "settings.audio.enhance.row.detail"),
        "Makes speech clearer for the transcript."
    );
    assert!(selected(cx, &rig, "settings.audio.enhance.auto"));
    assert!(!present(cx, &rig, "settings.audio.enhance.adjust"));
    click(cx, &rig, "settings.audio.enhance.off");
    assert_eq!(rig.storage.settings.get(MODE), Some(json!("off")));
    assert!(selected(cx, &rig, "settings.audio.enhance.off"));
    assert!(!present(cx, &rig, "enhance.sheet"), "Off opens nothing");
}

#[gpui_kit::test]
fn picking_custom_opens_the_sheet_and_esc_gives_focus_back(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    click(cx, &rig, "settings.audio.enhance.custom");
    assert_eq!(rig.storage.settings.get(MODE), Some(json!("custom")));
    assert!(present(cx, &rig, "enhance.sheet"));
    assert!(focused(cx, &rig, "enhance.done"), "Done takes focus");
    assert_eq!(label(cx, &rig, "enhance.sheet.title"), "Audio enhance");
    assert_eq!(label(cx, &rig, "enhance.lowCut.value"), "80 Hz");
    assert_eq!(label(cx, &rig, "enhance.bass.value"), "-6 dB");
    assert_eq!(label(cx, &rig, "enhance.treble.value"), "0 dB");
    assert_eq!(
        label(cx, &rig, "enhance.noiseReduction.row.detail"),
        "Can make the text worse. Use it only in a noisy room."
    );
    assert!(
        present(cx, &rig, "enhance.play"),
        "play is off but keeps its place"
    );
    press(cx, &rig, "escape");
    assert!(!present(cx, &rig, "enhance.sheet"));
    assert!(focused(cx, &rig, "settings.audio.enhance"));

    click(cx, &rig, "settings.audio.enhance.adjust");
    assert!(present(cx, &rig, "enhance.sheet"), "Adjust opens it again");
    click(cx, &rig, "enhance.done");
    assert!(!present(cx, &rig, "enhance.sheet"));
    assert!(focused(cx, &rig, "settings.audio.enhance.adjust"));
}

#[gpui_kit::test]
fn the_values_write_custom_and_reset_puts_back_auto(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    rig.tuning.update(cx, |me, cx| {
        me.set_low_cut(123.0, cx);
        me.set_bass(2.4, cx);
        me.set_treble(20.0, cx);
        me.set_noise_reduction(true, cx);
    });
    assert_eq!(
        effective(&rig.storage),
        Enhance {
            low_cut_hz: 120,
            bass_db: 2.0,
            treble_db: 12.0,
            noise_reduction: true,
        },
        "the values step and stay in range, and the mode turns Custom"
    );
    click(cx, &rig, "settings.audio.enhance.adjust");
    assert_eq!(label(cx, &rig, "enhance.lowCut.value"), "120 Hz");
    assert_eq!(label(cx, &rig, "enhance.treble.value"), "+12 dB");
    click(cx, &rig, "enhance.reset");
    assert_eq!(effective(&rig.storage), Enhance::AUTO);
    rig.tuning.update(cx, |me, cx| me.set_low_cut(0.0, cx));
    render(cx, &rig);
    assert_eq!(label(cx, &rig, "enhance.lowCut.value"), "Off");
}

#[gpui_kit::test]
fn a_take_is_kept_raw_on_disk_and_plays_with_the_current_values(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    rig.storage.settings.set(MODE, json!("custom")).unwrap();
    render(cx, &rig);
    click(cx, &rig, "settings.audio.enhance.adjust");
    click(cx, &rig, "enhance.record");
    assert_eq!(state(cx, &rig)["take"]["state"], "recording");
    assert!(
        present(cx, &rig, "enhance.mic.meter"),
        "the level shows while it records"
    );
    assert_eq!(
        label(cx, &rig, "enhance.sample.detail"),
        format!("Speak now. {SAMPLE_SECONDS} s left")
    );
    click(cx, &rig, "enhance.record");
    let kept = state(cx, &rig)["sample"].clone();
    assert_eq!(kept["duration_ms"], 2_000);
    let path = PathBuf::from(kept["path"].as_str().unwrap());
    assert!(path.ends_with("cache/enhance/sample.wav"));
    let raw = read_samples(&path).unwrap();
    let peak = raw.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
    assert!((peak - 0.25).abs() < 0.01, "kept as recorded, peak {peak}");
    assert_eq!(
        label(cx, &rig, "enhance.sample.detail"),
        "2 s, plays with the values below"
    );
    click(cx, &rig, "enhance.play");
    assert_eq!(state(cx, &rig)["playing"], "sample");
    click(cx, &rig, "enhance.play");
    assert_eq!(state(cx, &rig)["playing"], Value::Null);
}

#[gpui_kit::test]
fn a_kept_sample_is_there_after_a_restart(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    rig.tuning
        .update(cx, |me, _| me.keep_samples(tone()))
        .unwrap();
    let again = cx.new(|cx| {
        Tuning::new(
            Parts {
                storage: Rc::clone(&rig.storage),
                mic: rig.tuning.read_with(cx, |me, _| me.mic.clone()),
                run: runner(Ear::Same, Arc::default()),
                spawn: Rc::new(|_| Ok(())),
            },
            cx,
        )
    });
    assert_eq!(again.read_with(cx, |me, _| me.sample_seconds()), Some(2));
}

#[gpui_kit::test]
fn the_search_ranks_the_settings_and_use_takes_one(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Loudness);
    rig.storage
        .settings
        .set("dictation.language", json!("en"))
        .unwrap();
    rig.tuning.update(cx, |me, cx| me.show(Page::Find, cx));
    render(cx, &rig);
    assert_eq!(label(cx, &rig, "enhance.sheet.title"), "Find best settings");
    assert_eq!(label(cx, &rig, "enhance.reading.line.0"), SENTENCES[0]);
    click(cx, &rig, "enhance.reading.record");
    assert_eq!(label(cx, &rig, "enhance.reading.record"), "Stop");
    click(cx, &rig, "enhance.reading.record");

    let found = state(cx, &rig)["find"].clone();
    assert_eq!(found["state"], "results", "{found}");
    let runs = rig.runs.lock().unwrap().clone();
    assert!((30..=80).contains(&runs.len()), "{} runs", runs.len());
    assert_eq!(
        runs[0].language.as_deref(),
        Some("en"),
        "the dictation language"
    );
    let results = found["results"].as_array().unwrap();
    assert_eq!(results.len(), 5);
    assert_eq!(results[0]["inputGain"], 2.0, "the loudest wins");
    assert!(results[0]["errors"].as_u64() <= results[4]["errors"].as_u64());
    assert!(present(cx, &rig, "enhance.result.play.4"));
    assert!(
        label(cx, &rig, "enhance.result.detail.0").ends_with("words wrong")
            || label(cx, &rig, "enhance.result.detail.0").starts_with("All ")
    );

    click(cx, &rig, "enhance.result.play.0");
    assert_eq!(state(cx, &rig)["playing"], 0);
    click(cx, &rig, "enhance.result.use.0");
    assert_eq!(rig.storage.settings.get(MODE), Some(json!("custom")));
    assert_eq!(
        rig.storage.settings.get("audio.inputGain"),
        Some(json!(2.0))
    );
    assert_eq!(state(cx, &rig)["sheet"], "tune", "back on the tuning page");
    assert!(focused(cx, &rig, "enhance.done"));
}

#[gpui_kit::test]
fn when_every_setting_scores_the_same_auto_stays(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    rig.tuning.update(cx, |me, cx| {
        me.keep_samples(tone()).unwrap();
        me.show(Page::Find, cx);
        me.start_search(None, cx);
    });
    render(cx, &rig);
    assert_eq!(state(cx, &rig)["find"]["all_same"], true);
    assert_eq!(
        label(cx, &rig, "enhance.notice"),
        "Every setting got the same words right, so Auto stays."
    );
    assert_eq!(
        rig.storage.settings.get(MODE),
        Some(json!("auto")),
        "nothing was changed"
    );
}

#[gpui_kit::test]
fn a_broken_engine_ends_the_search_with_a_plain_message(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Broken);
    rig.tuning.update(cx, |me, cx| {
        me.keep_samples(tone()).unwrap();
        me.show(Page::Find, cx);
        me.start_search(None, cx);
    });
    render(cx, &rig);
    assert_eq!(state(cx, &rig)["find"]["state"], "failed");
    assert_eq!(
        rig.runs.lock().unwrap().len(),
        1,
        "it stops at the first failure"
    );
    assert!(label(cx, &rig, "enhance.notice").contains("Models"));
}

#[gpui_kit::test]
fn a_cancelled_search_drops_its_late_results(cx: &mut TestAppContext) {
    let rig = rig(cx, Ear::Same);
    rig.tuning.update(cx, |me, cx| {
        me.keep_samples(tone()).unwrap();
        me.show(Page::Find, cx);
        me.start_search(None, cx);
        me.cancel_search(cx);
    });
    render(cx, &rig);
    assert_eq!(state(cx, &rig)["find"]["state"], "reading");
}

#[gpui_kit::test]
fn the_search_gives_the_dictionary_prompt_to_whisper_and_not_to_parakeet(cx: &mut TestAppContext) {
    use crate::controller::testkit::{PARAKEET_ID, SMALL_ID, rig as speech_rig};
    let speech = speech_rig(cx, &[PARAKEET_ID, SMALL_ID], PARAKEET_ID);
    hushpen_store::dictionary::add(&speech.storage.database, "Hushpen", None).unwrap();
    let runs = Arc::default();
    let tuning = cx.new(|cx| {
        let mut tuning = Tuning::new(
            Parts {
                storage: Rc::clone(&speech.storage),
                mic: speech.mic.clone(),
                run: runner(Ear::Same, Arc::clone(&runs)),
                spawn: Rc::new(|job| {
                    job();
                    Ok(())
                }),
            },
            cx,
        );
        tuning.attach_models(speech.models.clone());
        tuning
    });
    for (id, prompt) in [(PARAKEET_ID, None), (SMALL_ID, Some("Hushpen"))] {
        speech
            .models
            .update(cx, |models, cx| models.select(id, cx))
            .unwrap();
        runs.lock().unwrap().clear();
        tuning.update(cx, |me, cx| {
            me.keep_samples(tone()).unwrap();
            me.start_search(None, cx);
        });
        cx.run_until_parked();
        let specs = runs.lock().unwrap().clone();
        assert!(!specs.is_empty(), "{id}");
        assert!(
            specs.iter().all(|spec| spec.prompt.as_deref() == prompt),
            "{id}"
        );
    }
}

#[gpui_kit::test]
fn the_search_and_the_reading_use_the_words_of_the_packs_that_are_on(cx: &mut TestAppContext) {
    use crate::controller::testkit::{PARAKEET_ID, SMALL_ID, rig as speech_rig};
    use crate::dictionary::packs;
    let speech = speech_rig(cx, &[PARAKEET_ID, SMALL_ID], SMALL_ID);
    hushpen_store::dictionary::add(&speech.storage.database, "Hushpen", None).unwrap();
    let pack = &packs::all()[0];
    speech
        .storage
        .settings
        .set(
            packs::SETTING,
            packs::turned(&speech.storage, pack.id, true),
        )
        .unwrap();
    let runs = Arc::default();
    let tuning = cx.new(|cx| {
        let mut tuning = Tuning::new(
            Parts {
                storage: Rc::clone(&speech.storage),
                mic: speech.mic.clone(),
                run: runner(Ear::Same, Arc::clone(&runs)),
                spawn: Rc::new(|job| {
                    job();
                    Ok(())
                }),
            },
            cx,
        );
        tuning.attach_models(speech.models.clone());
        tuning
    });
    let reading = tuning.read_with(cx, |me, _| me.reading());
    let last = reading.last().unwrap();
    assert!(last.starts_with("Hushpen, "), "{last}");
    tuning.update(cx, |me, cx| {
        me.keep_samples(tone()).unwrap();
        me.start_search(None, cx);
    });
    cx.run_until_parked();
    let specs = runs.lock().unwrap().clone();
    assert!(!specs.is_empty());
    let first_word = pack.words()[0];
    assert!(
        specs.iter().all(|spec| spec
            .prompt
            .as_deref()
            .is_some_and(|prompt| prompt.contains("Hushpen") && prompt.contains(first_word))),
        "{first_word}"
    );
}
