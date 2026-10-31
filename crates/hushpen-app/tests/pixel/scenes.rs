//! What the main window shows in a Light or Dark baseline: the shell, Home, the onboarding
//! steps, and every view with the panels the app attaches and a fixed set of data.

use crate::scan::{Node, Rect};
use crate::{OneMicrophone, Revoked, headless_in, pixel_data_dir};
use gpui_kit::base::test_support::snapshots;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AppContext as _, ExternalPaths, FileDropEvent, HeadlessAppContext, InputEvent as _,
    ScrollDelta, SharedString, point, px, size,
};
use hushpen_app::about::{About, Build, SystemOpener};
use hushpen_app::controller::{Controller, Engine, KeysStatus, monotonic_clock};
use hushpen_app::dictation::Dictation;
use hushpen_app::dictionary::Dictionary;
use hushpen_app::enhance::{Parts as EnhanceParts, Tuning};
use hushpen_app::files::Files;
use hushpen_app::history::History;
use hushpen_app::import::{Import, ImportRunner, Parts as ImportParts};
use hushpen_app::mic::Mic;
use hushpen_app::models::{self, Models};
use hushpen_app::net::endpoint::UreqTransport;
use hushpen_app::net::update::CheckError;
use hushpen_app::onboarding::{Onboarding, Parts as OnboardingParts, SystemGuide};
use hushpen_app::read_aloud::{Parts as ReadParts, ReadAloud};
use hushpen_app::settings::endpoint::EndpointForm;
use hushpen_app::settings::{Parts as SettingsParts, Settings};
use hushpen_app::shell::Shell;
use hushpen_app::shortcuts::{KeyControl, Shortcuts};
use hushpen_app::storage::{self, Storage};
use hushpen_app::theme::{Appearance, space};
use hushpen_app::tts_host::built_in_voice;
use hushpen_app::updater::{Checker, Parts as UpdaterParts, SystemInstaller, Updater, wall_clock};
use hushpen_app::voices::{Parts as VoiceParts, VoiceLibrary};
use hushpen_core::error::ENGINE_BAD_AUDIO;
use hushpen_core::onboarding::{Session, Step};
use hushpen_core::shortcut::{Bindings, Combo, Conflict, Platform};
use hushpen_core::update::{Offer, Version};
use hushpen_engine::{
    Failure, ImportProgress, JobOutcome, Segment as EngineSegment, Transcription,
};
use hushpen_speech::{Pcm, SpeechEngine, SpeechError, Voice, VoiceQuality};
use hushpen_store::data_dir::DataDir;
use hushpen_store::history::{self, Row};
use hushpen_store::voices::Library;
use image::RgbaImage;
use serde_json::json;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// A key listener that holds nothing and hears nothing, so the shortcut fields can open.
struct QuietKeys;

/// The feed of every scene: this version is the newest. The Updates view of a fresh folder never
/// asks, because the check is off; Check for Updates in About asks once.
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

    fn conflict(&self, _: &Combo) -> Option<Conflict> {
        None
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
    /// History with a read aloud on top of the transcripts.
    HistoryRead,
    /// The detail of a read aloud, with Read aloud again in the place of Reprocess.
    HistoryReadDetail,
    Dictionary,
    Files(FilesShot),
    /// The speech models, with the default one downloaded and active.
    Models,
    /// Models scrolled to the end: the cleanup and voice sections.
    ModelsBottom,
    /// The details sheet of the default speech model.
    ModelsDetails,
    /// "dictation" in the Models search field: rows from two sections, and the third hidden.
    ModelsSearch,
    /// A Models query no model matches.
    ModelsNoMatch,
    /// The Home language pop-up, which has a hundred options, so it has a search field.
    Menu(MenuShot),
    About,
    /// About scrolled to the end: the links and the copyright line.
    AboutBottom,
    /// About after Check for Updates, with what the check found under the button.
    AboutChecked,
    Settings(SettingsShot),
    /// Cleanup on the endpoint, with the failure of a test connection in its status row.
    EndpointFailure,
    /// Shortcuts with the recorder of the first slot waiting for keys, one changed shortcut
    /// with its reset button, and one shortcut off.
    ShortcutRecorder,
    /// The audio enhance sheet on its tuning page, opened by Adjust on the Custom row.
    EnhanceTune,
    /// The audio enhance sheet on Find best settings, waiting for the reading.
    EnhanceFind,
    /// The Manage Voices sheet on its list: the built-in voice and a cloned default voice.
    Voices,
    /// The Manage Voices sheet on Add Voice, before a take.
    VoicesAdd,
}

/// What a Settings baseline shows of the one long list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsShot {
    /// The top of the list, as the view opens.
    Top,
    /// Scrolled one page down.
    Middle,
    /// Scrolled to the end.
    Bottom,
    /// "audio" in the search field: rows from two sections under their headings.
    Search,
    /// A query no row matches.
    NoMatch,
    /// "memory" in the search field with the default idle policy of this system.
    Memory,
    /// "memory" with Keep in memory: the Idle time row is hidden.
    MemoryKeep,
    /// "memory" with Unload: the longest line under the policy row.
    MemoryUnload,
}

/// What an open long pop-up shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuShot {
    /// Just opened: the empty search field above the first eight options.
    Open,
    /// "ch" in the field: the four languages that have it.
    Query,
    /// A query no option has: the one disabled row.
    NoMatch,
}

impl MenuShot {
    pub const ALL: [MenuShot; 3] = [MenuShot::Open, MenuShot::Query, MenuShot::NoMatch];

    fn key(self) -> &'static str {
        match self {
            MenuShot::Open => "open",
            MenuShot::Query => "search",
            MenuShot::NoMatch => "empty",
        }
    }
}

/// What a Manual baseline shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesShot {
    /// Transcribe with no files: the drop zone and the options.
    Empty,
    /// A file dragged over the window lights up the drop zone.
    DragOver,
    /// A queue with a finished, a failed, a cancelled, a running, and a waiting file.
    Queue,
    /// Read aloud with text in the text area, opened in the mode the user last picked.
    ReadAloud,
    /// Read aloud on Chatterbox Turbo with a cloned default voice and the voice library row.
    ReadChatterbox,
    /// Read aloud on Kokoro with Heart blended with 30% of Adam: the Mix with and Mix rows.
    ReadKokoroMix,
    /// The open Mix with pop-up: nineteen voices, so it has a search field.
    ReadKokoroMixMenu,
}

impl FilesShot {
    pub const ALL: [FilesShot; 7] = [
        FilesShot::Empty,
        FilesShot::DragOver,
        FilesShot::Queue,
        FilesShot::ReadAloud,
        FilesShot::ReadChatterbox,
        FilesShot::ReadKokoroMix,
        FilesShot::ReadKokoroMixMenu,
    ];

    fn reads_aloud(self) -> bool {
        matches!(
            self,
            FilesShot::ReadAloud
                | FilesShot::ReadChatterbox
                | FilesShot::ReadKokoroMix
                | FilesShot::ReadKokoroMixMenu
        )
    }

    fn on_kokoro(self) -> bool {
        matches!(
            self,
            FilesShot::ReadKokoroMix | FilesShot::ReadKokoroMixMenu
        )
    }

    fn name(self) -> &'static str {
        match self {
            FilesShot::Empty => "files",
            FilesShot::DragOver => "files-drag-over",
            FilesShot::Queue => "files-queue",
            FilesShot::ReadAloud => "files-read-aloud",
            FilesShot::ReadChatterbox => "files-read-aloud-chatterbox",
            FilesShot::ReadKokoroMix => "files-read-aloud-kokoro-mix",
            FilesShot::ReadKokoroMixMenu => "files-read-aloud-kokoro-mix-menu",
        }
    }
}

impl SettingsShot {
    pub const ALL: [SettingsShot; 8] = [
        SettingsShot::Top,
        SettingsShot::Middle,
        SettingsShot::Bottom,
        SettingsShot::Search,
        SettingsShot::NoMatch,
        SettingsShot::Memory,
        SettingsShot::MemoryKeep,
        SettingsShot::MemoryUnload,
    ];

    fn key(self) -> &'static str {
        match self {
            SettingsShot::Top => "top",
            SettingsShot::Middle => "middle",
            SettingsShot::Bottom => "bottom",
            SettingsShot::Search => "search",
            SettingsShot::NoMatch => "search-empty",
            SettingsShot::Memory => "memory",
            SettingsShot::MemoryKeep => "memory-keep",
            SettingsShot::MemoryUnload => "memory-unload",
        }
    }
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
            Scene::HistoryRead => "history-read".into(),
            Scene::HistoryReadDetail => "history-read-detail".into(),
            Scene::Dictionary => "dictionary".into(),
            Scene::Files(shot) => shot.name().into(),
            Scene::Models => "models".into(),
            Scene::ModelsBottom => "models-bottom".into(),
            Scene::ModelsDetails => "models-details".into(),
            Scene::ModelsSearch => "models-search".into(),
            Scene::ModelsNoMatch => "models-search-empty".into(),
            Scene::Menu(shot) => format!("menu-{}", shot.key()),
            Scene::About => "about".into(),
            Scene::AboutBottom => "about-bottom".into(),
            Scene::AboutChecked => "about-checked".into(),
            Scene::Settings(shot) => format!("settings-{}", shot.key()),
            Scene::EndpointFailure => "settings-cleanup-endpoint-failure".into(),
            Scene::ShortcutRecorder => "shortcut-recorder".into(),
            Scene::EnhanceTune => "settings-enhance".into(),
            Scene::EnhanceFind => "settings-enhance-find".into(),
            Scene::Voices => "voices".into(),
            Scene::VoicesAdd => "voices-add".into(),
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
            Scene::HistoryRead,
            Scene::HistoryReadDetail,
            Scene::Dictionary,
            Scene::Models,
            Scene::ModelsBottom,
            Scene::ModelsDetails,
            Scene::ModelsSearch,
            Scene::ModelsNoMatch,
            Scene::About,
            Scene::AboutBottom,
            Scene::AboutChecked,
        ];
        scenes.extend(MenuShot::ALL.into_iter().map(Scene::Menu));
        scenes.extend(FilesShot::ALL.into_iter().map(Scene::Files));
        scenes.extend(SettingsShot::ALL.into_iter().map(Scene::Settings));
        scenes.push(Scene::EndpointFailure);
        scenes.push(Scene::ShortcutRecorder);
        scenes.push(Scene::EnhanceTune);
        scenes.push(Scene::EnhanceFind);
        scenes.push(Scene::Voices);
        scenes.push(Scene::VoicesAdd);
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
            Scene::Shell
                | Scene::Home
                | Scene::Menu(_)
                | Scene::Onboarding(_)
                | Scene::PermissionsChanged
        )
    }

    /// Whether a sheet floats over the page, so the scrim dims the whole window.
    pub fn has_sheet(self) -> bool {
        matches!(
            self,
            Scene::ModelsDetails
                | Scene::EnhanceTune
                | Scene::EnhanceFind
                | Scene::Voices
                | Scene::VoicesAdd
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

/// A read aloud half an hour after the newest transcript, so it leads the list.
fn seed_read(storage: &Storage) {
    let id = "read-0";
    let relative = format!("audio/{id}.wav");
    let path = storage.data.root().join(&relative);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("audio folder");
    std::fs::write(&path, b"wav").expect("an audio file");
    let row = Row {
        id: id.into(),
        created_at: FIRST_AT + 1_800_000,
        kind: "read".into(),
        status: "completed".into(),
        duration_ms: 9_400,
        model_id: Some("chatterbox-turbo".into()),
        language_requested: Some("en".into()),
        final_text: Some(READ_TEXT.into()),
        insert_outcome: Some("none".into()),
        source_name: Some("Anna".into()),
        audio_path: Some(relative),
        ..Row::default()
    };
    history::save(&storage.database, &row, &[]).expect("save a read aloud");
}

/// When the cloned voice was added: 2026-05-01 10:20 UTC.
const VOICE_ADDED_AT: u64 = 1_777_630_800_000;

/// Read aloud on Chatterbox Turbo, with one cloned voice that is the default voice.
fn seed_chatterbox(storage: &Storage) {
    let anna = Library::new(storage.data.voices_dir())
        .add("Anna", 8_000, VOICE_ADDED_AT, |path| {
            std::fs::write(path, b"clip")
        })
        .expect("add a cloned voice");
    let settings = &storage.settings;
    settings
        .set("readAloud.engine", json!("chatterbox-turbo"))
        .expect("store the engine");
    settings
        .set("readAloud.defaultVoice", json!(anna.id))
        .expect("store the default voice");
}

/// Read aloud on Kokoro: Heart with 30% of Adam.
fn seed_kokoro_mix(storage: &Storage) {
    let settings = &storage.settings;
    for (key, value) in [
        ("readAloud.engine", json!("kokoro")),
        ("readAloud.voice.kokoro", json!("af_heart")),
        ("readAloud.voice.kokoroMix", json!("am_adam")),
        ("readAloud.voice.kokoroMixPercent", json!(30)),
    ] {
        settings.set(key, value).expect("store a Kokoro setting");
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

/// The files of the queue scene and their sizes, in the order they are added. The runner gives
/// them, in turn, a result, a refusal, a cancel, a job that stays at 40%, and one that waits
/// behind it.
const QUEUE_FILES: [(&str, u64); 5] = [
    ("Planning call.m4a", 3_400_000),
    ("Voice memo.opus", 820_000),
    ("Lecture 4.mp3", 48_200_000),
    ("Interview.wav", 212_000_000),
    ("Podcast episode.flac", 1_340_000_000),
];

/// The first report of a job the engine child opened: the length from the headers.
fn length_report() -> ImportProgress {
    ImportProgress {
        fraction: 0.0,
        audio_ms: Some(47 * 60_000 + 12_000),
    }
}

/// The text the Read aloud scene types.
const READ_TEXT: &str = "Hushpen reads this text aloud with the voice you pick. Long texts are \
                         read in parts, and the first part plays while the rest is made.";

/// A speech engine with a fixed list of voices, so the voice menus read the same everywhere.
struct FixedVoices;

impl SpeechEngine for FixedVoices {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        let voice = |id: &str, name: &str, language: &str, quality| Voice {
            id: id.into(),
            name: name.into(),
            language: language.into(),
            quality,
        };
        Ok(vec![
            voice("samantha", "Samantha", "en-US", VoiceQuality::Enhanced),
            voice("daniel", "Daniel", "en-GB", VoiceQuality::Default),
            voice("anna", "Anna", "de-DE", VoiceQuality::Default),
        ])
    }

    fn speak(&self, _: &str, _: Option<&str>, _: f32) -> Result<Pcm, SpeechError> {
        Err(SpeechError::failed("the scenes play no sound"))
    }
}

/// Chatterbox as the app lists it: the voice of the model, then each voice of the library.
struct ChatterboxVoices(std::path::PathBuf);

impl SpeechEngine for ChatterboxVoices {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        let cloned = Library::new(&self.0).list().into_iter().map(|entry| Voice {
            id: entry.id,
            name: entry.name,
            language: "en".into(),
            quality: VoiceQuality::Default,
        });
        Ok(std::iter::once(built_in_voice()).chain(cloned).collect())
    }

    fn speak(&self, _: &str, _: Option<&str>, _: f32) -> Result<Pcm, SpeechError> {
        Err(SpeechError::failed("the scenes play no sound"))
    }
}

/// Kokoro as the app lists it: the English voices of the model.
struct KokoroVoices;

impl SpeechEngine for KokoroVoices {
    fn voices(&self) -> Result<Vec<Voice>, SpeechError> {
        Ok(hushpen_core::kokoro::VOICES
            .iter()
            .map(|voice| Voice {
                id: voice.id.into(),
                name: voice.name(),
                language: voice.language().into(),
                quality: VoiceQuality::Default,
            })
            .collect())
    }

    fn speak(&self, _: &str, _: Option<&str>, _: f32) -> Result<Pcm, SpeechError> {
        Err(SpeechError::failed("the scenes play no sound"))
    }
}

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
            2 => {
                progress(length_report());
                JobOutcome::Cancelled
            }
            _ => {
                progress(length_report());
                progress(ImportProgress {
                    fraction: 0.4,
                    audio_ms: None,
                });
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
    for file in &entry.files {
        let path = dir.join(&file.file);
        std::fs::File::create(&path)
            .and_then(|created| created.set_len(file.bytes))
            .expect("write the model file");
        hushpen_store::model_files::write_stamp(&path, &file.sha256).expect("stamp the model");
    }
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
                path: snapshot.path().iter().map(ToString::to_string).collect(),
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

/// Turns the mouse wheel over the content pane; a negative `pixels` moves the page up.
fn scroll_by(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, pixels: f32) {
    cx.update_window(handle, |_, window, cx| {
        window.scroll(
            "content.scroll",
            ScrollDelta::Pixels(point(px(0.0), px(pixels))),
            cx,
        )
    })
    .expect("scroll the page");
    cx.run_until_parked();
}

/// Clicks the Settings search field and types `query`, like a user.
fn type_search(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, query: &str) {
    click(cx, handle, "settings.search");
    cx.update_window(handle, |_, window, cx| window.input(query, cx))
        .expect("type the query");
    cx.run_until_parked();
}

/// Clicks `id`, scrolling the page first like the mouse wheel does.
fn click_scrolled(cx: &mut HeadlessAppContext, handle: gpui_kit::AnyWindowHandle, id: &str) {
    scroll_by(cx, handle, 5_000.0);
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
        scroll_by(cx, handle, -120.0);
    }
    click(cx, handle, &id);
}

/// One capture at a time: the scenes share one data folder, so the folder text in Storage and
/// About reads the same in every baseline.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The main window at its least height showing `scene` in `appearance`.
pub fn capture(scene: Scene, appearance: Appearance) -> Shot {
    capture_at(scene, appearance, space::WINDOW_HEIGHT)
}

/// The main window `height` points tall showing `scene` in `appearance`.
pub fn capture_at(scene: Scene, appearance: Appearance, height: f32) -> Shot {
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
        Scene::HistoryRead | Scene::HistoryReadDetail => {
            seed_history(&storage);
            seed_read(&storage);
        }
        Scene::Dictionary => seed_dictionary(&storage),
        Scene::ShortcutRecorder => {
            let settings = &storage.settings;
            settings
                .set("shortcut.command", json!("Ctrl+Alt+K"))
                .expect("store a changed shortcut");
            settings
                .set("shortcut.enabled", json!({"pasteLast": false}))
                .expect("turn a shortcut off");
        }
        Scene::Files(shot) => {
            seed_model(&storage);
            if shot.reads_aloud() {
                storage
                    .settings
                    .set("files.mode", json!("readAloud"))
                    .expect("store the mode");
            }
            if shot == FilesShot::ReadChatterbox {
                seed_chatterbox(&storage);
            }
            if shot.on_kokoro() {
                seed_kokoro_mix(&storage);
            }
        }
        Scene::Voices | Scene::VoicesAdd => {
            storage
                .settings
                .set("files.mode", json!("readAloud"))
                .expect("store the mode");
            seed_chatterbox(&storage);
        }
        Scene::Models
        | Scene::ModelsBottom
        | Scene::ModelsDetails
        | Scene::ModelsSearch
        | Scene::ModelsNoMatch => seed_model(&storage),
        // Automatic names the CPU count, which differs between machines.
        Scene::Settings(shot) => {
            storage
                .settings
                .set("engine.threads", json!(4))
                .expect("store the thread count");
            let policy = match shot {
                SettingsShot::MemoryKeep => Some("keep"),
                SettingsShot::MemoryUnload => Some("unload"),
                _ => None,
            };
            if let Some(policy) = policy {
                storage
                    .settings
                    .set("engine.idlePolicy", json!(policy))
                    .expect("store the idle policy");
            }
        }
        Scene::EnhanceTune | Scene::EnhanceFind => {
            let settings = &storage.settings;
            settings
                .set("engine.threads", json!(4))
                .expect("store the thread count");
            settings
                .set("audio.enhance", json!("custom"))
                .expect("store the mode");
        }
        _ => {}
    }
    let mut cx = headless_in(appearance);
    let handle = cx
        .open_window(size(px(space::WINDOW_WIDTH), px(height)), |window, cx| {
            let shell = cx.new(|cx| Shell::new(window, cx));
            let mic = cx.new(|cx| Mic::new(Rc::clone(&storage), Arc::new(OneMicrophone), cx));
            mic.update(cx, |mic, cx| mic.refresh(cx));
            let models = cx.new(|cx| {
                Models::new(
                    Rc::clone(&storage),
                    hushpen_core::catalog::embedded(),
                    models::Machine {
                        opener: Rc::new(SystemOpener),
                    },
                    models::production_transfer(),
                    models::thread_spawner(),
                    None,
                    cx,
                )
            });
            models.update(cx, |models, cx| models.attach_search(window, cx));
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
            controller.update(cx, |controller, cx| {
                if mac {
                    controller.attach_permissions(Arc::new(Revoked), cx);
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
                        Dictation::new(Rc::clone(&storage), models.clone(), controller.clone(), cx)
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
                                installer: Arc::new(SystemInstaller::detect(
                                    storage.data.logs_dir(),
                                )),
                                quit: Rc::new(|cx| cx.quit()),
                                clock: wall_clock(),
                                current: Version::parse("2.0.0").expect("a version"),
                            },
                            cx,
                        )
                    });
                    let tuning = cx.new(|cx| {
                        Tuning::new(
                            EnhanceParts {
                                storage: Rc::clone(&storage),
                                mic: mic.clone(),
                                run: Arc::new(|_, _| JobOutcome::Cancelled),
                                spawn: models::thread_spawner(),
                            },
                            cx,
                        )
                    });
                    settings.update(cx, |settings, cx| {
                        settings.attach_updater(updater.clone(), cx);
                        settings.attach_search(window, cx);
                        settings.attach_tuning(tuning, cx);
                    });
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
                    // A fixed commit and date, so the baselines do not change with each commit.
                    let build = Build {
                        commit: Some("1a2b3c4"),
                        date: Some("2026-05-15"),
                        ..Build::current()
                    };
                    let about = cx.new(|cx| {
                        About::new(Rc::clone(&storage), Rc::new(SystemOpener), build, cx)
                    });
                    about.update(cx, |about, cx| {
                        about.attach_models(models.clone(), cx);
                        about.attach_updater(updater, cx);
                    });
                    let history = cx.new(|cx| {
                        History::new(Rc::clone(&storage), controller.clone(), window, cx)
                    });
                    let dictionary = cx.new(|cx| Dictionary::new(Rc::clone(&storage), window, cx));
                    let manual =
                        matches!(scene, Scene::Files(_) | Scene::Voices | Scene::VoicesAdd);
                    let files = manual.then(|| {
                        let import = cx.new(|cx| {
                            Import::new(
                                ImportParts {
                                    storage: Rc::clone(&storage),
                                    models: models.clone(),
                                    controller: controller.clone(),
                                    run: queue_runner(),
                                    spawn: models::thread_spawner(),
                                    // A still clock: the time left reads the same each run.
                                    clock: Rc::new(|| 0),
                                },
                                cx,
                            )
                        });
                        if matches!(scene, Scene::Files(FilesShot::Queue)) {
                            let folder = data.join("files");
                            std::fs::create_dir_all(&folder).expect("create the files folder");
                            let paths = QUEUE_FILES
                                .iter()
                                .map(|(name, bytes)| {
                                    let path = folder.join(name);
                                    std::fs::File::create(&path)
                                        .and_then(|file| file.set_len(*bytes))
                                        .expect("write a file");
                                    path
                                })
                                .collect();
                            import.update(cx, |import, cx| import.add_paths(paths, cx));
                        }
                        let voices: Arc<dyn SpeechEngine> = Arc::new(FixedVoices);
                        let kokoro: Arc<dyn SpeechEngine> = Arc::new(KokoroVoices);
                        let chatterbox: Arc<dyn SpeechEngine> =
                            Arc::new(ChatterboxVoices(storage.data.voices_dir()));
                        let library = cx.new(|cx| {
                            VoiceLibrary::new(
                                VoiceParts {
                                    storage: Rc::clone(&storage),
                                    mic: Some(mic.clone()),
                                    spawn: models::thread_spawner(),
                                    silent: true,
                                },
                                window,
                                cx,
                            )
                        });
                        let read = cx.new(|cx| {
                            ReadAloud::new(
                                ReadParts {
                                    storage: Rc::clone(&storage),
                                    system: voices,
                                    kokoro,
                                    turbo: Arc::clone(&chatterbox),
                                    nano: chatterbox,
                                    library: Some(library),
                                    spawn: models::thread_spawner(),
                                    silent: true,
                                },
                                window,
                                cx,
                            )
                        });
                        cx.new(|cx| Files::new(Rc::clone(&storage), import, Some(read), cx))
                    });
                    shell.update(cx, |shell, cx| {
                        shell.attach_mic(mic, cx);
                        shell.attach_dictation(dictation, cx);
                        shell.attach_shortcuts(shortcuts, cx);
                        shell.attach_settings(settings, cx);
                        shell.attach_about(about, cx);
                        shell.attach_history(history, cx);
                        shell.attach_dictionary(dictionary, cx);
                        if let Some(files) = files {
                            shell.attach_files(files, cx);
                        }
                        shell.attach_models(models, cx);
                    });
                }
            }
            cx.new(|cx| gpui_kit::base::Root::new(shell, window, cx))
        })
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
        Scene::Home | Scene::Onboarding(_) => {}
        Scene::PermissionsChanged => {
            // A stored grant that reads missing counts as lost only after the start reads
            // settle for 2 s on a timer; the repair page opens then. The headless executor
            // runs on a fake clock, so a sleep never fires that timer.
            for _ in 0..50 {
                cx.advance_clock(hushpen_core::permission::SETTLE_EVERY);
                cx.run_until_parked();
                let repair = cx
                    .update_window(handle, |_, window, _| {
                        window
                            .try_find(SharedString::from("onboarding.page"))
                            .is_some()
                    })
                    .expect("look for the repair page");
                if repair {
                    break;
                }
            }
        }
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
        Scene::HistoryRead => click(cx, handle, "sidebar.history"),
        Scene::HistoryReadDetail => {
            click(cx, handle, "sidebar.history");
            click(cx, handle, "history.row.0");
        }
        Scene::Dictionary => click(cx, handle, "sidebar.dictionary"),
        Scene::Files(FilesShot::Empty) => click(cx, handle, "sidebar.files"),
        Scene::Files(FilesShot::DragOver) => {
            click(cx, handle, "sidebar.files");
            let event = FileDropEvent::Entered {
                position: point(
                    px(space::WINDOW_WIDTH / 2.0),
                    px(space::WINDOW_HEIGHT / 2.0),
                ),
                paths: ExternalPaths(vec![std::path::PathBuf::from("/Talks/Keynote.m4a")].into()),
            };
            cx.update_window(handle, |_, window, cx| {
                window.dispatch_event(event.to_platform_input(), cx);
                window.refresh();
            })
            .expect("drag a file over the window");
            cx.run_until_parked();
        }
        Scene::Files(
            shot @ (FilesShot::ReadAloud
            | FilesShot::ReadChatterbox
            | FilesShot::ReadKokoroMix
            | FilesShot::ReadKokoroMixMenu),
        ) => {
            click(cx, handle, "sidebar.files");
            click(cx, handle, "read.text");
            // The text area lets go of the focus, so the baseline shows it at rest.
            cx.update_window(handle, |_, window, cx| {
                window.input(READ_TEXT, cx);
                window.blur(cx);
            })
            .expect("type the text");
            cx.run_until_parked();
            if shot == FilesShot::ReadKokoroMixMenu {
                click(cx, handle, "read.mix");
            }
        }
        Scene::Files(FilesShot::Queue) => {
            click(cx, handle, "sidebar.files");
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
            // The queue sits under the drop zone and the options.
            scroll_by(cx, handle, -5_000.0);
        }
        Scene::Models => click(cx, handle, "sidebar.models"),
        Scene::ModelsBottom => {
            click(cx, handle, "sidebar.models");
            scroll_by(cx, handle, -5_000.0);
        }
        Scene::ModelsDetails => {
            click(cx, handle, "sidebar.models");
            let index = hushpen_core::catalog::embedded()
                .whisper
                .iter()
                .position(|entry| entry.default)
                .expect("a default speech model");
            click(cx, handle, &format!("models.info.{index}"));
        }
        Scene::ModelsSearch | Scene::ModelsNoMatch => {
            click(cx, handle, "sidebar.models");
            click(cx, handle, "models.search");
            let query = if matches!(scene, Scene::ModelsSearch) {
                "dictation"
            } else {
                "zebra"
            };
            cx.update_window(handle, |_, window, cx| window.input(query, cx))
                .expect("type the query");
            cx.run_until_parked();
        }
        Scene::Menu(shot) => {
            click(cx, handle, "home.language");
            // The search field of the open menu already has the focus.
            let query = match shot {
                MenuShot::Open => None,
                MenuShot::Query => Some("ch"),
                MenuShot::NoMatch => Some("zebra"),
            };
            if let Some(query) = query {
                cx.update_window(handle, |_, window, cx| window.input(query, cx))
                    .expect("type the query");
                cx.run_until_parked();
            }
        }
        Scene::About => click(cx, handle, "sidebar.about"),
        Scene::AboutBottom => {
            click(cx, handle, "sidebar.about");
            scroll_by(cx, handle, -5_000.0);
        }
        Scene::AboutChecked => {
            click(cx, handle, "sidebar.about");
            click(cx, handle, "about.checkForUpdates");
        }
        Scene::Settings(shot) => {
            click(cx, handle, "sidebar.settings");
            match shot {
                SettingsShot::Top => {}
                SettingsShot::Middle => scroll_by(cx, handle, -480.0),
                SettingsShot::Bottom => scroll_by(cx, handle, -5_000.0),
                SettingsShot::Search => type_search(cx, handle, "audio"),
                SettingsShot::NoMatch => type_search(cx, handle, "zebra"),
                SettingsShot::Memory | SettingsShot::MemoryKeep | SettingsShot::MemoryUnload => {
                    type_search(cx, handle, "memory")
                }
            }
        }
        Scene::ShortcutRecorder => {
            click(cx, handle, "sidebar.settings");
            click_scrolled(cx, handle, "settings.shortcut.hold");
        }
        Scene::EnhanceTune => {
            click(cx, handle, "sidebar.settings");
            click_scrolled(cx, handle, "settings.audio.enhance.adjust");
        }
        Scene::EnhanceFind => {
            click(cx, handle, "sidebar.settings");
            click_scrolled(cx, handle, "settings.audio.enhance.adjust");
            click(cx, handle, "enhance.find");
        }
        Scene::Voices => {
            click(cx, handle, "sidebar.files");
            click(cx, handle, "read.manage");
        }
        Scene::VoicesAdd => {
            click(cx, handle, "sidebar.files");
            click(cx, handle, "read.manage");
            click(cx, handle, "voices.add");
        }
        Scene::EndpointFailure => {
            click(cx, handle, "sidebar.settings");
            // The search keeps the endpoint form on one screen, with the failure under it.
            type_search(cx, handle, "endpoint");
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
            // The test button shares its row with the status, so the failure shows in place.
            click_scrolled(cx, handle, "settings.cleanup.endpointTest");
            std::thread::sleep(std::time::Duration::from_millis(300));
            cx.run_until_parked();
        }
    }
}
