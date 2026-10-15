//! What the main window shows in a Light or Dark baseline: the shell, Home, the onboarding
//! steps, and every view with the panels the app attaches and a fixed set of data.

use crate::scan::{Node, Rect};
use crate::{OneMicrophone, Revoked, headless_in, pixel_data_dir};
use gpui_kit::base::test_support::snapshots;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{AppContext as _, HeadlessAppContext, ScrollDelta, SharedString, point, px, size};
use hushpen_app::about::{About, SystemOpener};
use hushpen_app::controller::{Controller, Engine, KeysStatus, monotonic_clock};
use hushpen_app::dictation::Dictation;
use hushpen_app::dictionary::Dictionary;
use hushpen_app::history::History;
use hushpen_app::import::{Import, ImportRunner, Parts as ImportParts};
use hushpen_app::mic::Mic;
use hushpen_app::models::{self, Models};
use hushpen_app::net::endpoint::UreqTransport;
use hushpen_app::net::update::CheckError;
use hushpen_app::onboarding::{Onboarding, Parts as OnboardingParts, SystemGuide};
use hushpen_app::settings::endpoint::EndpointForm;
use hushpen_app::settings::{Parts as SettingsParts, Section, Settings};
use hushpen_app::shell::Shell;
use hushpen_app::shortcuts::{KeyControl, Shortcuts};
use hushpen_app::storage::{self, Storage};
use hushpen_app::theme::{Appearance, space};
use hushpen_app::updater::{Checker, Parts as UpdaterParts, Updater, wall_clock};
use hushpen_core::error::ENGINE_BAD_AUDIO;
use hushpen_core::onboarding::{Session, Step};
use hushpen_core::shortcut::{Bindings, Combo, Platform};
use hushpen_core::update::{Offer, Version};
use hushpen_engine::{Failure, JobOutcome, Segment as EngineSegment, Transcription};
use hushpen_store::data_dir::DataDir;
use hushpen_store::history::{self, Row};
use image::RgbaImage;
use serde_json::json;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// A key listener that holds nothing and hears nothing, so the shortcut fields can open.
struct QuietKeys;

/// The Updates view of a fresh folder never asks: the check is off.
struct NoFeed;

impl Checker for NoFeed {
    fn check(&self, _: &Version) -> Result<Offer, CheckError> {
        Ok(Offer::UpToDate)
    }
}

impl KeyControl for QuietKeys {
    fn set_bindings(&self, _: Bindings) {}

    fn start_recording(&self) {}

    fn stop_recording(&self) {}

    fn in_use(&self, _: &Combo) -> bool {
        false
    }
}

/// One capture and the elements the same frame registered for the hook.
pub struct Shot {
    pub image: RgbaImage,
    pub nodes: Vec<Node>,
}

/// What a Light and Dark baseline shows.
#[derive(Debug, Clone, Copy)]
pub enum Scene {
    /// The shell with its sidebar and title bar around a placeholder view.
    Shell,
    /// Home with the dictation and microphone panels the app attaches.
    Home,
    Onboarding(Step),
    /// Onboarding reopened by a revoked macOS permission.
    PermissionsChanged,
    History,
    HistoryDetail,
    /// Rows are being chosen.
    HistorySelect,
    /// The "Delete all N transcripts and their audio?" dialog.
    HistoryClear,
    Dictionary,
    /// A queue with a finished, a failed, a cancelled, a running, and a waiting file.
    Import,
    Models,
    About,
    Settings(Section),
    /// Cleanup on the endpoint, with the failure of a test connection in its status row.
    EndpointFailure,
    /// Shortcuts with the recorder of the first slot waiting for keys.
    ShortcutRecorder,
}

impl Scene {
    pub fn name(self) -> String {
        match self {
            Scene::Shell => "shell".into(),
            Scene::Home => "home".into(),
            Scene::Onboarding(step) => format!("onboarding-{}", step.key()),
            Scene::PermissionsChanged => "onboarding-permissions-changed".into(),
            Scene::History => "history".into(),
            Scene::HistoryDetail => "history-detail".into(),
            Scene::HistorySelect => "history-select".into(),
            Scene::HistoryClear => "history-clear".into(),
            Scene::Dictionary => "dictionary".into(),
            Scene::Import => "import".into(),
            Scene::Models => "models".into(),
            Scene::About => "about".into(),
            Scene::Settings(section) => format!("settings-{}", section.key()),
            Scene::EndpointFailure => "settings-cleanup-endpoint-failure".into(),
            Scene::ShortcutRecorder => "shortcut-recorder".into(),
        }
    }

    pub fn shell_and_onboarding() -> Vec<Scene> {
        let mut scenes = vec![Scene::Shell, Scene::Home];
        scenes.extend(Step::ALL.into_iter().map(Scene::Onboarding));
        scenes.push(Scene::PermissionsChanged);
        scenes
    }

    pub fn views() -> Vec<Scene> {
        let mut scenes = vec![
            Scene::History,
            Scene::HistoryDetail,
            Scene::HistorySelect,
            Scene::HistoryClear,
            Scene::Dictionary,
            Scene::Import,
            Scene::Models,
            Scene::About,
        ];
        scenes.extend(Section::ALL.into_iter().map(Scene::Settings));
        scenes.push(Scene::EndpointFailure);
        scenes.push(Scene::ShortcutRecorder);
        scenes
    }

    pub fn all() -> Vec<Scene> {
        let mut scenes = Self::shell_and_onboarding();
        scenes.extend(Self::views());
        scenes
    }

    /// Whether the scene is one of the views whose rows stay one line plus one secondary line:
    /// History, Dictionary, Models, Settings, and About. Home and onboarding have their own
    /// panels (a level meter, a transcript box) that are taller on purpose.
    pub fn has_rows(self) -> bool {
        !matches!(
            self,
            Scene::Shell | Scene::Home | Scene::Onboarding(_) | Scene::PermissionsChanged
        )
    }
}

/// The text of the transcripts a History scene shows, newest first. An empty one is a failure.
const NAMES: [&str; 8] = [
    "Send the draft to Priya before lunch and ask if the numbers still match.",
    "Remind me to call the dentist on Thursday afternoon.",
    "Meeting notes: ship the new dictionary view on Friday, then review the models page.",
    "",
    "Interview with the maintainers, part one.",
    "Buy oat milk, coffee, and two lemons on the way home.",
    "The quarterly numbers look fine, but the second chart needs a clearer title.",
    "Thanks for the quick reply. I will send the invoice tomorrow morning.",
];

/// The newest transcript, in Unix milliseconds: 2025-06-01 09:15 UTC.
const FIRST_AT: i64 = 1_748_769_300_000;

fn seed_history(storage: &Storage) {
    // The seeded transcripts are old; the default retention would take their audio.
    storage
        .settings
        .set("history.audioRetention", json!("forever"))
        .expect("keep the audio");
    for (index, text) in NAMES.iter().enumerate() {
        let id = format!("row-{index}");
        let mut row = Row {
            id: id.clone(),
            created_at: FIRST_AT - index as i64 * 3_600_000,
            kind: "dictation".into(),
            status: "completed".into(),
            duration_ms: 2_000 + index as i64 * 700,
            model_id: Some("base".into()),
            language_detected: Some("en".into()),
            raw_text: Some(format!("{} um", text.to_lowercase())),
            rule_text: Some((*text).to_owned()),
            final_text: Some((*text).to_owned()),
            insert_outcome: Some("pasted".into()),
            target_app: Some("Firefox".into()),
            ..Row::default()
        };
        match index {
            0 => {
                let relative = format!("audio/{id}.wav");
                let path = storage.data.root().join(&relative);
                std::fs::create_dir_all(path.parent().expect("a folder")).expect("audio folder");
                std::fs::write(&path, b"wav").expect("an audio file");
                row.audio_path = Some(relative);
                row.llm_text = Some(
                    "Send the draft to Priya before lunch, and ask if the numbers still match."
                        .into(),
                );
                row.llm_outcome = Some("used".into());
            }
            1 => row.insert_outcome = Some("copied_only".into()),
            3 => {
                row.status = "failed".into();
                row.error_code = Some(ENGINE_BAD_AUDIO.into());
                row.raw_text = None;
                row.rule_text = None;
                row.final_text = None;
                row.insert_outcome = None;
            }
            4 => {
                row.kind = "import".into();
                row.source_name = Some("Interview.wav".into());
                row.target_app = None;
                row.insert_outcome = None;
                row.duration_ms = 183_000;
            }
            5 => {
                row.status = "cancelled".into();
                row.insert_outcome = None;
            }
            _ => {}
        }
        history::save(&storage.database, &row, &[]).expect("save a transcript");
    }
}

fn seed_dictionary(storage: &Storage) {
    for (phrase, heard) in [
        ("Hushpen", "hush pen"),
        ("Priya Raghavan", "pria ragavan"),
        ("Kubernetes", "cooper netties"),
        ("PostgreSQL", ""),
    ] {
        hushpen_store::dictionary::add(&storage.database, phrase, Some(heard))
            .expect("add a dictionary entry");
    }
}

/// The files of the Import scene, in the order they are added. The runner gives them, in turn,
/// a result, a refusal, a cancel, a job that stays at 40%, and one that waits behind it.
const QUEUE_NAMES: [&str; 5] = [
    "Planning call.m4a",
    "Voice memo.opus",
    "Lecture 4.mp3",
    "Interview.wav",
    "Podcast episode.flac",
];

fn queue_runner() -> ImportRunner {
    let calls = std::sync::atomic::AtomicUsize::new(0);
    Arc::new(move |_, progress, _| {
        match calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
            0 => JobOutcome::Done(Transcription {
                text: "Let us start with the budget and then go through the hiring plan.".into(),
                language: "en".into(),
                segments: vec![EngineSegment {
                    start_ms: 0,
                    end_ms: 4_000,
                    text: "Let us start with the budget and then go through the hiring plan."
                        .into(),
                }],
                audio_ms: 4_000,
                decode_ms: 10,
            }),
            1 => JobOutcome::Failed(Failure {
                code: hushpen_core::error::IMPORT_UNSUPPORTED_CODEC.into(),
                detail: "Opus is not supported yet.".into(),
                reason: None,
            }),
            2 => JobOutcome::Cancelled,
            _ => {
                progress(0.4);
                // The scene is captured long before this ends.
                std::thread::sleep(std::time::Duration::from_secs(120));
                JobOutcome::Cancelled
            }
        }
    })
}

/// A model file that the app takes as downloaded and checked, so the Import view has no
/// "get a model" line: the right size, and the stamp a hash check leaves.
fn seed_model(storage: &Storage) {
    let entry = hushpen_core::catalog::embedded()
        .default_whisper()
        .expect("a default model")
        .clone();
    let dir = storage.data.whisper_models_dir();
    std::fs::create_dir_all(&dir).expect("create the models folder");
    let path = dir.join(&entry.file);
    std::fs::File::create(&path)
        .and_then(|file| file.set_len(entry.bytes))
        .expect("write the model file");
    hushpen_store::model_files::write_stamp(&path, &entry.sha256).expect("stamp the model");
}

fn focused(snapshot: &gpui_kit::base::test_support::ElementSnapshot) -> bool {
    panic::catch_unwind(AssertUnwindSafe(|| snapshot.focused()))
        .ok()
        .flatten()
        .unwrap_or(false)
}

fn nodes_of(window: &gpui_kit::Window) -> Vec<Node> {
    snapshots(window)
        .iter()
        .map(|snapshot| {
            let bounds = snapshot.bounds();
            Node {
                id: snapshot
                    .path()
                    .last()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                role: snapshot.role().map(|role| format!("{role:?}")),
                rect: Rect {
                    x: f32::from(bounds.origin.x),
                    y: f32::from(bounds.origin.y),
                    w: f32::from(bounds.size.width),
                    h: f32::from(bounds.size.height),
                },
                focused: focused(snapshot),
                checked: snapshot.checked(),
                visible: snapshot.visible(),
            }
        })
        .collect()
}

fn click(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    let id = SharedString::from(id.to_owned());
    cx.update_window(handle, move |_, window, cx| window.click(id, cx))
        .unwrap_or_else(|error| panic!("click: {error}"));
    cx.run_until_parked();
}

/// Clicks `id`, scrolling the page first like the mouse wheel does.
fn click_scrolled(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    let scroll = |cx: &mut HeadlessAppContext, pixels: f32| {
        cx.update_window(handle, |_, window, cx| {
            window.scroll(
                "content.scroll",
                ScrollDelta::Pixels(point(px(0.0), px(pixels))),
                cx,
            )
        })
        .expect("scroll the page");
        cx.run_until_parked();
    };
    scroll(cx, 5_000.0);
    let id = SharedString::from(id.to_owned());
    for _ in 0..40 {
        let shown = cx
            .update_window(handle, |_, window, _| {
                window
                    .try_find(id.clone())
                    .is_some_and(|found| found.visible())
            })
            .expect("look for the control");
        if shown {
            break;
        }
        scroll(cx, -120.0);
    }
    click(cx, handle, &id);
}

/// One capture at a time: the scenes share one data folder, so the folder text in Storage and
/// About reads the same in every baseline.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The main window showing `scene` in `appearance`.
pub fn capture(scene: Scene, appearance: Appearance) -> Shot {
    let _guard = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let data = pixel_data_dir("scene");
    let storage = Rc::new(
        storage::open(DataDir::open(data.join("data")).expect("open the data folder"))
            .expect("open the storage"),
    );
    let mac = matches!(scene, Scene::PermissionsChanged);
    match scene {
        Scene::Onboarding(step) => storage
            .settings
            .set_internal("onboarding.step", json!(step.key()))
            .expect("store the step"),
        Scene::PermissionsChanged => {
            let settings = &storage.settings;
            settings
                .set_internal("onboarding.completed", json!(true))
                .expect("complete onboarding");
            settings
                .set_internal(
                    "permissions.lastGranted",
                    json!({"microphone": true, "accessibility": true, "inputMonitoring": true}),
                )
                .expect("store the earlier grants");
        }
        Scene::History | Scene::HistoryDetail | Scene::HistorySelect | Scene::HistoryClear => {
            seed_history(&storage);
        }
        Scene::Dictionary => seed_dictionary(&storage),
        Scene::Import => seed_model(&storage),
        _ => {}
    }
    let mut cx = headless_in(appearance);
    let handle = cx
        .open_window(
            size(px(space::WINDOW_WIDTH), px(space::WINDOW_HEIGHT)),
            |window, cx| {
                let shell = cx.new(|cx| Shell::new(window, cx));
                let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(OneMicrophone), cx));
                mic.update(cx, |mic, cx| mic.refresh(cx));
                let models = cx.new(|cx| {
                    Models::new(
                        Rc::clone(&storage),
                        hushpen_core::catalog::embedded(),
                        models::production_transfer(),
                        models::thread_spawner(),
                        None,
                        cx,
                    )
                });
                let engine = Engine::new(Arc::new(|_, _| JobOutcome::Cancelled), Rc::new(|| None));
                let controller = cx.new(|cx| {
                    Controller::new(
                        Rc::clone(&storage),
                        mic.clone(),
                        models.clone(),
                        engine,
                        models::thread_spawner(),
                        monotonic_clock(),
                        cx,
                    )
                });
                controller.update(cx, |controller, _| {
                    if mac {
                        controller.attach_permissions(Arc::new(Revoked));
                    }
                    controller.attach_keys(KeysStatus::Available, Rc::new(|_| {}));
                });
                match scene {
                    Scene::Onboarding(_) | Scene::PermissionsChanged => {
                        let onboarding = cx.new(|cx| {
                            Onboarding::new(
                                OnboardingParts {
                                    storage: Rc::clone(&storage),
                                    mic,
                                    models,
                                    controller,
                                    guide: Rc::new(SystemGuide),
                                    platform: if mac {
                                        Platform::MacOs
                                    } else {
                                        Platform::Linux
                                    },
                                    session: Session::X11,
                                },
                                cx,
                            )
                        });
                        shell.update(cx, |shell, cx| shell.attach_onboarding(onboarding, cx));
                    }
                    Scene::Shell => {}
                    _ => {
                        let dictation = cx.new(|cx| {
                            Dictation::new(
                                Rc::clone(&storage),
                                models.clone(),
                                controller.clone(),
                                cx,
                            )
                        });
                        let shortcuts =
                            cx.new(|cx| Shortcuts::new(Rc::clone(&storage), Platform::Linux, cx));
                        shortcuts.update(cx, |shortcuts, cx| {
                            shortcuts.attach_keys(Ok(Rc::new(QuietKeys)), cx)
                        });
                        let settings = cx.new(|cx| {
                            Settings::new(
                                SettingsParts {
                                    storage: Rc::clone(&storage),
                                    mic: mic.clone(),
                                    login: None,
                                    default_data_dir: None,
                                    restart: None,
                                },
                                cx,
                            )
                        });
                        let updater = cx.new(|cx| {
                            Updater::new(
                                UpdaterParts {
                                    storage: Rc::clone(&storage),
                                    checker: Arc::new(NoFeed),
                                    opener: Rc::new(SystemOpener),
                                    clock: wall_clock(),
                                    current: Version::parse("2.0.0").expect("a version"),
                                },
                                cx,
                            )
                        });
                        settings.update(cx, |settings, cx| settings.attach_updater(updater, cx));
                        if matches!(scene, Scene::EndpointFailure) {
                            let form = cx.new(|cx| {
                                EndpointForm::new(
                                    Rc::clone(&storage),
                                    Arc::new(UreqTransport),
                                    window,
                                    cx,
                                )
                            });
                            settings.update(cx, |settings, cx| settings.attach_endpoint(form, cx));
                        }
                        let about =
                            cx.new(|cx| About::new(Rc::clone(&storage), Rc::new(SystemOpener), cx));
                        let history = cx.new(|cx| {
                            History::new(Rc::clone(&storage), controller.clone(), window, cx)
                        });
                        let dictionary =
                            cx.new(|cx| Dictionary::new(Rc::clone(&storage), window, cx));
                        let import = matches!(scene, Scene::Import).then(|| {
                            let import = cx.new(|cx| {
                                Import::new(
                                    ImportParts {
                                        storage: Rc::clone(&storage),
                                        models: models.clone(),
                                        controller: controller.clone(),
                                        run: queue_runner(),
                                        spawn: models::thread_spawner(),
                                    },
                                    cx,
                                )
                            });
                            let folder = data.join("files");
                            std::fs::create_dir_all(&folder).expect("create the files folder");
                            let paths = QUEUE_NAMES
                                .iter()
                                .map(|name| {
                                    let path = folder.join(name);
                                    std::fs::write(&path, b"audio").expect("write a file");
                                    path
                                })
                                .collect();
                            import.update(cx, |import, cx| import.add_paths(paths, cx));
                            import
                        });
                        shell.update(cx, |shell, cx| {
                            shell.attach_mic(mic, cx);
                            shell.attach_dictation(dictation, cx);
                            shell.attach_shortcuts(shortcuts, cx);
                            shell.attach_settings(settings, cx);
                            shell.attach_about(about, cx);
                            shell.attach_history(history, cx);
                            shell.attach_dictionary(dictionary, cx);
                            if let Some(import) = import {
                                shell.attach_import(import, cx);
                            }
                            shell.attach_models(models, cx);
                        });
                    }
                }
                cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
            },
        )
        .expect("open the headless window");
    let handle: gpui_kit::AnyWindowHandle = handle.into();
    // The microphone list is read on a thread and arrives through a channel.
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    drive(&mut cx, handle, scene);
    let image = cx.capture_screenshot(handle).expect("capture the window");
    let nodes = cx
        .update_window(handle, |_, window, _| nodes_of(window))
        .expect("read the elements");
    Shot { image, nodes }
}

/// Opens the view of `scene` and does what it shows.
fn drive(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, scene: Scene) {
    match scene {
        Scene::Shell => click(cx, handle, "sidebar.history"),
        Scene::Home | Scene::Onboarding(_) | Scene::PermissionsChanged => {}
        Scene::History => click(cx, handle, "sidebar.history"),
        Scene::HistoryDetail => {
            click(cx, handle, "sidebar.history");
            click(cx, handle, "history.row.0");
        }
        Scene::HistorySelect => {
            click(cx, handle, "sidebar.history");
            click(cx, handle, "history.select");
            click(cx, handle, "history.row.1");
        }
        Scene::HistoryClear => {
            click(cx, handle, "sidebar.history");
            click(cx, handle, "history.clear");
        }
        Scene::Dictionary => click(cx, handle, "sidebar.dictionary"),
        Scene::Import => {
            click(cx, handle, "sidebar.import");
            // The jobs run on threads and report through a channel: wait for the third file
            // to end, then give the running one time to report its 40%.
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                cx.run_until_parked();
                let ready = cx
                    .update_window(handle, |_, window, _| {
                        window
                            .try_find(SharedString::from("import.remove.2"))
                            .is_some()
                    })
                    .expect("look for the third line");
                if ready {
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
            cx.run_until_parked();
        }
        Scene::Models => click(cx, handle, "sidebar.models"),
        Scene::About => click(cx, handle, "sidebar.about"),
        Scene::Settings(section) => {
            click(cx, handle, "sidebar.settings");
            if section != Section::General {
                click(cx, handle, &format!("settings.section.{}", section.key()));
            }
        }
        Scene::ShortcutRecorder => {
            click(cx, handle, "sidebar.settings");
            click(cx, handle, "settings.section.shortcuts");
            click(cx, handle, "settings.shortcut.hold");
        }
        Scene::EndpointFailure => {
            click(cx, handle, "sidebar.settings");
            click(cx, handle, "settings.section.cleanup");
            for step in [
                "settings.cleanup.provider",
                "settings.cleanup.provider.option.endpoint",
            ] {
                click_scrolled(cx, handle, step);
            }
            let other_computer = format!("http://{}:11434/v1", "10.0.0.2");
            click_scrolled(cx, handle, "settings.cleanup.endpointUrl");
            cx.update_window(handle, |_, window, cx| window.input(&other_computer, cx))
                .expect("type the address");
            cx.run_until_parked();
            click_scrolled(cx, handle, "settings.cleanup.endpointTest");
            std::thread::sleep(std::time::Duration::from_millis(300));
            cx.run_until_parked();
            cx.update_window(handle, |_, window, cx| {
                window.scroll(
                    "content.scroll",
                    ScrollDelta::Pixels(point(px(0.0), px(-5_000.0))),
                    cx,
                )
            })
            .expect("scroll to the status row");
            cx.run_until_parked();
        }
    }
}
