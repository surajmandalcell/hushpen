//! Read aloud on the Manual view: typed text, or a `.txt` or `.md` file, spoken by the system
//! voice, by Kokoro, or by Chatterbox Turbo or Nano with a voice of the voice library. The audio
//! plays while the rest is still being spoken, and it exports as a WAV. The engine, the voice of
//! each engine, the speed, and the format the user picks are kept in settings.

pub mod panel;

use crate::history::export::{default_directory, shown_path};
use crate::history::save_dialog;
use crate::hook;
use crate::models::Spawner;
use crate::storage::Storage;
use crate::voices::{Changed as VoicesChanged, Page as VoicesPage, VoiceLibrary};
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::component::input::TextareaState;
use gpui_kit::{AnyWindowHandle, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle};
use hushpen_audio::Player;
use hushpen_core::chatterbox::BUILT_IN_VOICE;
use hushpen_core::error::{
    SPEECH_NO_TEXT, SPEECH_NOT_ENGLISH, SPEECH_TEXT_TOO_LONG, SPEECH_TEXT_UNREADABLE,
    SPEECH_UNAVAILABLE, SPEECH_VOICE_CLIP, SPEECH_VOICE_MISSING,
};
use hushpen_speech::{
    MAX_SPEED, MIN_SPEED, SAMPLE_RATE, SpeechEngine, SpeechError, SpeechEvent, TextFormat, Voice,
    prepare, read_text_file, synthesize,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::Duration;

/// The way the read-aloud shortcut reaches this player: the selection goes into the text
/// area and plays.
pub fn speaker(read: &Entity<ReadAloud>) -> crate::controller::Speaker {
    let (playing, stop, speak) = (read.clone(), read.clone(), read.clone());
    crate::controller::Speaker {
        playing: Rc::new(move |cx| playing.read(cx).playing()),
        stop: Rc::new(move |cx| stop.update(cx, |read, cx| read.stop(cx))),
        speak: Rc::new(move |text, cx| {
            speak.update(cx, |read, cx| {
                read.set_text(&text, TextFormat::Plain, cx);
                read.play(cx)
            })
        }),
    }
}

pub const ENGINE_SETTING: &str = "readAloud.engine";
pub const SPEED_SETTING: &str = "readAloud.speed";
pub const FORMAT_SETTING: &str = "readAloud.format";
/// The voice both Chatterbox models read with; empty for the default voice of the library.
pub const CHATTERBOX_VOICE_SETTING: &str = "readAloud.voice.chatterbox";

/// How often the thread that carries a synthesis looks for a Stop between two chunks.
const STOP_POLL: Duration = Duration::from_millis(100);
/// How often the view repaints while audio plays, so Stop turns back into Play at the end.
const PLAYING_TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    System,
    Kokoro,
    ChatterboxTurbo,
    ChatterboxNano,
}

impl EngineKind {
    pub const ALL: [EngineKind; 4] = [
        EngineKind::System,
        EngineKind::Kokoro,
        EngineKind::ChatterboxTurbo,
        EngineKind::ChatterboxNano,
    ];

    pub fn key(self) -> &'static str {
        match self {
            EngineKind::System => "system",
            EngineKind::Kokoro => "kokoro",
            EngineKind::ChatterboxTurbo => "chatterbox-turbo",
            EngineKind::ChatterboxNano => "chatterbox-nano",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    pub fn label(self) -> &'static str {
        match self {
            EngineKind::System => "System voice",
            EngineKind::Kokoro => "Kokoro",
            EngineKind::ChatterboxTurbo => "Chatterbox Turbo",
            EngineKind::ChatterboxNano => "Chatterbox Nano",
        }
    }

    /// Chatterbox reads with the voices of the voice library.
    pub fn is_chatterbox(self) -> bool {
        matches!(
            self,
            EngineKind::ChatterboxTurbo | EngineKind::ChatterboxNano
        )
    }

    /// The engine runs a voice model that downloads in Models.
    pub fn has_model(self) -> bool {
        self != EngineKind::System
    }

    /// The setting that keeps the chosen voice of this engine.
    fn voice_setting(self) -> &'static str {
        match self {
            EngineKind::System => "readAloud.voice.system",
            EngineKind::Kokoro => "readAloud.voice.kokoro",
            EngineKind::ChatterboxTurbo | EngineKind::ChatterboxNano => CHATTERBOX_VOICE_SETTING,
        }
    }
}

/// What Read aloud asks of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadRequest {
    /// The engine needs its voice model: open Models.
    OpenModels,
}

/// The pop-up list that is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Engine,
    Language,
    Voice,
    Format,
}

/// What is spoken: when any of it changes, audio made before no longer fits.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    text: String,
    engine: EngineKind,
    voice: Option<String>,
    speed: f32,
}

/// The speech made for one request so far. The player holds the samples.
struct Audio {
    request: Request,
    player: Player,
    complete: bool,
}

/// A synthesis on its way.
struct Job {
    id: u64,
    stop: Arc<AtomicBool>,
    done: usize,
    total: usize,
    /// Play the audio as it arrives.
    play: bool,
    /// Write the audio here once it is complete.
    export_to: Option<PathBuf>,
}

enum ReadEvent {
    Voices {
        engine: EngineKind,
        voices: Result<Vec<Voice>, SpeechError>,
    },
    Speech {
        job: u64,
        event: SpeechEvent,
    },
}

pub struct Parts {
    pub storage: Rc<Storage>,
    pub system: Arc<dyn SpeechEngine>,
    pub kokoro: Arc<dyn SpeechEngine>,
    pub turbo: Arc<dyn SpeechEngine>,
    pub nano: Arc<dyn SpeechEngine>,
    /// The cloned voices of Chatterbox; `None` where the window could not give the sheet its
    /// fields.
    pub library: Option<Entity<VoiceLibrary>>,
    pub spawn: Spawner,
    /// Tests keep the sound off.
    pub silent: bool,
}

pub struct ReadAloud {
    storage: Rc<Storage>,
    system: Arc<dyn SpeechEngine>,
    kokoro: Arc<dyn SpeechEngine>,
    turbo: Arc<dyn SpeechEngine>,
    nano: Arc<dyn SpeechEngine>,
    library: Option<Entity<VoiceLibrary>>,
    /// The engines whose voice model is not downloaded, as the engine menu last found them.
    missing: Vec<EngineKind>,
    spawn: Spawner,
    silent: bool,
    window: AnyWindowHandle,
    events: UnboundedSender<ReadEvent>,
    pub(crate) text: Entity<TextareaState>,
    /// Markdown when the text came from a `.md` file.
    text_format: TextFormat,
    /// The voices of the chosen engine, or `None` while they load.
    voices: Option<Result<Vec<Voice>, SpeechError>>,
    /// The language whose voices the voice menu lists, such as `en`.
    language: Option<String>,
    menu: Option<Menu>,
    job: Option<Job>,
    next_job: u64,
    audio: Option<Audio>,
    /// Why the last action failed, in words.
    error: Option<String>,
    /// What the last export or file open did.
    info: Option<String>,
    choosing: bool,
    ticking: bool,
    pub(crate) focus: Focus,
}

/// The tab stops of the view, in the order they show.
pub(crate) struct Focus {
    pub open: FocusHandle,
    pub engine: FocusHandle,
    pub language: FocusHandle,
    pub voice: FocusHandle,
    pub speed: FocusHandle,
    pub format: FocusHandle,
    pub models: FocusHandle,
    pub manage: FocusHandle,
    pub other_engine: FocusHandle,
    pub play: FocusHandle,
    pub export: FocusHandle,
    /// The rows of the open menu.
    pub options: Vec<FocusHandle>,
}

impl EventEmitter<ReadRequest> for ReadAloud {}

impl ReadAloud {
    pub fn new(parts: Parts, window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(event) = queue.next().await {
                if this.update(cx, |me, cx| me.handle(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let text = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder("Type or paste the text to read aloud")
        });
        let tab = |cx: &mut Context<Self>| cx.focus_handle().tab_stop(true);
        let focus = Focus {
            open: tab(cx),
            engine: tab(cx),
            language: tab(cx),
            voice: tab(cx),
            speed: tab(cx),
            format: tab(cx),
            models: tab(cx),
            manage: tab(cx),
            other_engine: tab(cx),
            play: tab(cx),
            export: tab(cx),
            options: Vec::new(),
        };
        if let Some(library) = &parts.library {
            cx.subscribe(library, |me, _, _: &VoicesChanged, cx| {
                if me.engine().is_chatterbox() {
                    me.fetch_voices(cx);
                }
                cx.notify();
            })
            .detach();
        }
        let mut me = Self {
            storage: parts.storage,
            system: parts.system,
            kokoro: parts.kokoro,
            turbo: parts.turbo,
            nano: parts.nano,
            library: parts.library,
            missing: Vec::new(),
            spawn: parts.spawn,
            silent: parts.silent,
            window: window.window_handle(),
            events,
            text,
            text_format: TextFormat::Plain,
            voices: None,
            language: None,
            menu: None,
            job: None,
            next_job: 1,
            audio: None,
            error: None,
            info: None,
            choosing: false,
            ticking: false,
            focus,
        };
        me.load_voices(cx);
        me
    }

    pub fn engine(&self) -> EngineKind {
        self.storage
            .settings
            .get(ENGINE_SETTING)
            .and_then(|value| value.as_str().and_then(EngineKind::from_key))
            .unwrap_or(EngineKind::System)
    }

    /// The speed as a factor of the voice's normal speed.
    pub fn speed(&self) -> f32 {
        self.storage
            .settings
            .get(SPEED_SETTING)
            .and_then(|value| value.as_f64())
            .map_or(1.0, |speed| speed as f32)
            .clamp(MIN_SPEED, MAX_SPEED)
    }

    pub fn format(&self) -> &'static str {
        "wav"
    }

    /// The chosen voice of the engine, or `None` for its default voice.
    pub fn voice(&self) -> Option<String> {
        self.storage
            .settings
            .get(self.engine().voice_setting())
            .and_then(|value| value.as_str().map(str::to_owned))
            .filter(|id| !id.is_empty())
    }

    /// The voice a read speaks with: the chosen one, else for Chatterbox the default voice of
    /// the library, which is the voice of the model when no cloned voice is the default.
    pub fn spoken_voice(&self, cx: &App) -> Option<String> {
        self.voice().or_else(|| {
            self.engine().is_chatterbox().then(|| {
                self.default_voice(cx)
                    .unwrap_or_else(|| BUILT_IN_VOICE.to_owned())
            })
        })
    }

    /// The default voice of the library, or `None` for the voice of the model.
    pub fn default_voice(&self, cx: &App) -> Option<String> {
        self.library
            .as_ref()
            .and_then(|library| library.read(cx).default_id())
    }

    /// The name of the voice Chatterbox reads with by default.
    pub fn default_voice_name(&self, cx: &App) -> String {
        self.library.as_ref().map_or_else(
            || crate::voices::BUILT_IN_NAME.to_owned(),
            |library| library.read(cx).default_name(),
        )
    }

    pub fn library(&self) -> Option<&Entity<VoiceLibrary>> {
        self.library.as_ref()
    }

    /// Opens the voice library sheet on its list, from Manage Voices.
    pub fn manage_voices(&mut self, window: &mut gpui_kit::Window, cx: &mut Context<Self>) {
        self.menu = None;
        let back = self.focus.manage.clone();
        if let Some(library) = &self.library {
            library.update(cx, |library, cx| {
                library.open(VoicesPage::List, Some(back), window, cx)
            });
        }
        cx.notify();
    }

    pub fn voices(&self) -> Option<&Result<Vec<Voice>, SpeechError>> {
        self.voices.as_ref()
    }

    /// The languages the engine's voices speak, as primary subtags such as `en`, sorted by name.
    pub fn languages(&self) -> Vec<String> {
        let mut codes: Vec<String> = self
            .listed()
            .iter()
            .map(|voice| primary(&voice.language))
            .collect();
        codes.sort_by_key(|code| language_name(code));
        codes.dedup();
        codes
    }

    /// The language the voice menu lists: the one picked, else the chosen voice's, else
    /// English when a voice speaks it, else the first.
    pub fn language(&self) -> Option<String> {
        let languages = self.languages();
        self.language
            .clone()
            .filter(|code| languages.contains(code))
            .or_else(|| {
                let voice = self.voice()?;
                let chosen = self.listed().iter().find(|found| found.id == voice)?;
                Some(primary(&chosen.language))
            })
            .or_else(|| languages.iter().find(|code| *code == "en").cloned())
            .or_else(|| languages.first().cloned())
    }

    /// The voices of the language the voice menu lists.
    pub fn language_voices(&self) -> Vec<Voice> {
        let language = self.language();
        self.listed()
            .iter()
            .filter(|voice| Some(primary(&voice.language)) == language)
            .cloned()
            .collect()
    }

    fn listed(&self) -> &[Voice] {
        match &self.voices {
            Some(Ok(voices)) => voices,
            _ => &[],
        }
    }

    /// Why the engine cannot speak here, in words, or `None` while it can.
    pub fn unavailable(&self) -> Option<String> {
        match &self.voices {
            Some(Err(error)) => Some(match (self.engine(), error.code) {
                (engine, SPEECH_UNAVAILABLE) if engine.has_model() => {
                    format!(
                        "{} needs its voice model. Download it in Models.",
                        engine.label()
                    )
                }
                (_, SPEECH_UNAVAILABLE) => error.detail.clone(),
                _ => "The voices could not be listed. Try again in a moment.".to_owned(),
            }),
            _ => None,
        }
    }

    /// An engine with a voice model is chosen and its model is not downloaded.
    pub fn needs_model(&self) -> bool {
        self.engine().has_model()
            && matches!(&self.voices, Some(Err(error)) if error.code == SPEECH_UNAVAILABLE)
    }

    /// Whether the voice model of `engine` was missing when the engine menu last opened.
    pub fn missing(&self, engine: EngineKind) -> bool {
        self.missing.contains(&engine)
    }

    /// The last read failed on text Chatterbox cannot read, so the error offers another engine.
    pub fn offers_other_engine(&self) -> bool {
        self.error.as_deref() == Some(NOT_ENGLISH)
    }

    /// Reads the voices again when the engine could not list them, as after its model
    /// downloaded in Models.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if matches!(self.voices, Some(Err(_))) {
            self.load_voices(cx);
        }
    }

    pub fn menu(&self) -> Option<Menu> {
        self.menu
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn info(&self) -> Option<&str> {
        self.info.as_deref()
    }

    pub fn choosing(&self) -> bool {
        self.choosing
    }

    /// The parts spoken and the parts in all, while a synthesis runs.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.job.as_ref().map(|job| (job.done, job.total))
    }

    pub fn exporting(&self) -> bool {
        self.job.as_ref().is_some_and(|job| job.export_to.is_some())
    }

    /// True while speech plays or is being made for Play.
    pub fn playing(&self) -> bool {
        self.job.as_ref().is_some_and(|job| job.play)
            || self
                .audio
                .as_ref()
                .is_some_and(|audio| audio.player.is_playing())
    }

    pub fn toggle_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        self.menu = (self.menu != Some(menu)).then_some(menu);
        if self.menu == Some(Menu::Engine) {
            self.missing = EngineKind::ALL
                .into_iter()
                .filter(|kind| !self.engine_of(*kind).available())
                .collect();
        }
        let rows = match menu {
            Menu::Engine => EngineKind::ALL.len(),
            Menu::Language => self.languages().len(),
            Menu::Voice => self.language_voices().len() + 1,
            Menu::Format => 1,
        };
        while self.focus.options.len() < rows {
            self.focus.options.push(cx.focus_handle().tab_stop(true));
        }
        cx.notify();
    }

    pub fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    pub fn set_engine(&mut self, engine: EngineKind, cx: &mut Context<Self>) -> Result<(), String> {
        self.set(ENGINE_SETTING, json!(engine.key()))?;
        self.menu = None;
        self.language = None;
        if self.offers_other_engine() {
            self.error = None;
        }
        self.load_voices(cx);
        cx.notify();
        Ok(())
    }

    pub fn set_language(&mut self, code: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.languages().iter().any(|known| known == code) {
            return Err(format!("No voice of this engine speaks `{code}`."));
        }
        self.language = Some(code.to_owned());
        self.menu = None;
        cx.notify();
        Ok(())
    }

    /// Chooses a voice by its id, or the engine's default voice with `None`.
    pub fn set_voice(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(id) = id
            && !self.listed().iter().any(|voice| voice.id == id)
        {
            return Err(format!("This engine has no voice `{id}`."));
        }
        self.set(self.engine().voice_setting(), json!(id.unwrap_or("")))?;
        self.menu = None;
        cx.notify();
        Ok(())
    }

    /// Sets the speed, rounded to a twentieth (1.05, 1.1).
    pub fn set_speed(&mut self, speed: f32, cx: &mut Context<Self>) -> Result<(), String> {
        if !speed.is_finite() {
            return Err("The speed must be a number.".into());
        }
        let twentieths = (f64::from(speed.clamp(MIN_SPEED, MAX_SPEED)) * 20.0).round();
        self.set(SPEED_SETTING, json!(twentieths / 20.0))?;
        cx.notify();
        Ok(())
    }

    pub fn set_format(&mut self, format: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.set(FORMAT_SETTING, json!(format))?;
        self.menu = None;
        cx.notify();
        Ok(())
    }

    fn set(&self, key: &str, value: Value) -> Result<(), String> {
        self.storage
            .settings
            .set(key, value)
            .map_err(|error| error.to_string())
    }

    /// Replaces the text, as typing or a file would.
    pub fn set_text(&mut self, text: &str, format: TextFormat, cx: &mut Context<Self>) {
        self.text_format = format;
        let state = self.text.clone();
        let text = text.to_owned();
        let _ = self.window.update(cx, |_, window, cx| {
            state.update(cx, |state, cx| state.set_value(text, window, cx));
        });
        cx.notify();
    }

    /// Asks the window to open Models, where the Kokoro model downloads.
    pub fn open_models(&mut self, cx: &mut Context<Self>) {
        cx.emit(ReadRequest::OpenModels);
    }

    /// Opens a `.txt` or `.md` file with the picker and puts its text in the text area.
    pub fn choose_text_file(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.choosing {
            return Err("The file picker is already open.".into());
        }
        self.choosing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let paths = crate::import::pick_files(cx).await;
            let _ = this.update(cx, |me, cx| {
                me.choosing = false;
                if let Some(path) = paths.and_then(|paths| paths.into_iter().next()) {
                    me.open_text_file(path, cx);
                }
                cx.notify();
            });
        })
        .detach();
        Ok(())
    }

    /// Reads a text file into the text area: Markdown for `.md`, plain text for the rest.
    pub fn open_text_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match read_text_file(&path) {
            Ok(text) => {
                self.error = None;
                self.info = None;
                self.set_text(&text, TextFormat::of_path(&path), cx);
                hook::record_event("read", "opened");
            }
            Err(error) => {
                log::warn!("{error}");
                self.error = Some(message_of(&error));
            }
        }
        cx.notify();
    }

    /// Plays the text: the audio made for it already, or new speech as it is made.
    pub fn play(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let request = match self.request(cx) {
            Ok(request) => request,
            Err(message) => return self.refuse(message, cx),
        };
        self.error = None;
        if !self.audio_fits(&request) {
            self.start(request, true, None, cx);
            return Ok(());
        }
        if let Some(job) = &mut self.job {
            job.play = true;
        }
        if let Some(audio) = &mut self.audio
            && audio.player.duration_ms() > 0
        {
            audio.player.seek_ms(0);
            audio.player.play();
        }
        hook::record_event("read", "play");
        self.tick(cx);
        cx.notify();
        Ok(())
    }

    /// Stops the sound and any speech still being made. Audio that is complete stays for the
    /// next Play or an export.
    pub fn stop(&mut self, cx: &mut Context<Self>) {
        match self.job.take() {
            // The export goes on; only the sound stops.
            Some(job) if job.export_to.is_some() => self.job = Some(Job { play: false, ..job }),
            Some(job) => {
                job.stop.store(true, Ordering::Relaxed);
                self.audio = None;
            }
            None => {}
        }
        if let Some(audio) = &mut self.audio {
            audio.player.pause();
        }
        hook::record_event("read", "stop");
        cx.notify();
    }

    /// Asks where to save the audio, then writes it: at once when the audio of this text is
    /// complete, else once it is made.
    pub fn export(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.exporting() || self.choosing {
            return self.refuse("An export is already running.".into(), cx);
        }
        let request = match self.request(cx) {
            Ok(request) => request,
            Err(message) => return self.refuse(message, cx),
        };
        self.error = None;
        self.info = None;
        self.choosing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let chosen = save_dialog::choose(
                cx,
                "Export audio",
                default_directory(),
                "Read aloud.wav".into(),
            )
            .await;
            let _ = this.update(cx, |me, cx| {
                me.choosing = false;
                match chosen {
                    Ok(Some(path)) => me.export_to(request, path.with_extension("wav"), cx),
                    Ok(None) => {}
                    Err(message) => me.error = Some(message),
                }
                cx.notify();
            });
        })
        .detach();
        Ok(())
    }

    fn export_to(&mut self, request: Request, path: PathBuf, cx: &mut Context<Self>) {
        if self.audio_fits(&request) && self.audio.as_ref().is_some_and(|audio| audio.complete) {
            self.write(&path);
            return;
        }
        match &mut self.job {
            Some(job)
                if self
                    .audio
                    .as_ref()
                    .is_some_and(|audio| audio.request == request) =>
            {
                job.export_to = Some(path);
            }
            _ => self.start(request, false, Some(path), cx),
        }
    }

    fn refuse(&mut self, message: String, cx: &mut Context<Self>) -> Result<(), String> {
        self.error = Some(message.clone());
        hook::record_event("read", "refused");
        cx.notify();
        Err(message)
    }

    /// The audio of `request` exists, complete or on its way.
    fn audio_fits(&self, request: &Request) -> bool {
        self.audio
            .as_ref()
            .is_some_and(|audio| &audio.request == request)
    }

    /// What Play or Export would speak now.
    fn request(&self, cx: &App) -> Result<Request, String> {
        if let Some(why) = self.unavailable() {
            return Err(why);
        }
        let raw = self.text.read(cx).value().to_string();
        let text = prepare(&raw, self.text_format).map_err(|error| message_of(&error))?;
        Ok(Request {
            text,
            engine: self.engine(),
            voice: self.spoken_voice(cx),
            speed: self.speed(),
        })
    }

    fn engine_of(&self, kind: EngineKind) -> Arc<dyn SpeechEngine> {
        Arc::clone(match kind {
            EngineKind::System => &self.system,
            EngineKind::Kokoro => &self.kokoro,
            EngineKind::ChatterboxTurbo => &self.turbo,
            EngineKind::ChatterboxNano => &self.nano,
        })
    }

    /// Starts a new synthesis; the old one stops.
    fn start(
        &mut self,
        request: Request,
        play: bool,
        export_to: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if let Some(old) = self.job.take() {
            old.stop.store(true, Ordering::Relaxed);
        }
        let player = match Player::from_samples(SAMPLE_RATE, Vec::new()) {
            Ok(player) if self.silent => player.silenced(),
            Ok(player) => player,
            Err(error) => {
                self.error = Some(format!("The sound could not start: {error}"));
                return;
            }
        };
        let id = self.next_job;
        self.next_job += 1;
        let stop = Arc::new(AtomicBool::new(false));
        let engine = self.engine_of(request.engine);
        let (text, voice, speed) = (request.text.clone(), request.voice.clone(), request.speed);
        let (flag, events) = (Arc::clone(&stop), self.events.clone());
        let spawned = (self.spawn)(Box::new(move || {
            let synthesis = synthesize(engine, text, voice, speed);
            loop {
                match synthesis.events().recv_timeout(STOP_POLL) {
                    Ok(event) => {
                        let last = !matches!(event, SpeechEvent::Audio { .. });
                        if events
                            .unbounded_send(ReadEvent::Speech { job: id, event })
                            .is_err()
                            || last
                        {
                            return;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) if flag.load(Ordering::Relaxed) => {
                        synthesis.cancel();
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        }));
        if let Err(error) = spawned {
            self.error = Some(format!("Read aloud could not start: {error}"));
            return;
        }
        hook::record_event("read", if play { "speak play" } else { "speak export" });
        self.audio = Some(Audio {
            request,
            player,
            complete: false,
        });
        self.job = Some(Job {
            id,
            stop,
            done: 0,
            total: 0,
            play,
            export_to,
        });
        self.tick(cx);
        cx.notify();
    }

    /// Repaints a few times a second while sound plays, so the buttons follow the end.
    fn tick(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PLAYING_TICK).await;
                let going = this.update(cx, |me, cx| {
                    cx.notify();
                    me.ticking = me.playing() || me.job.is_some();
                    me.ticking
                });
                if !matches!(going, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn load_voices(&mut self, cx: &mut Context<Self>) {
        self.voices = None;
        self.fetch_voices(cx);
    }

    /// Lists the voices of the engine again. The old list shows until the new one comes.
    fn fetch_voices(&mut self, cx: &mut Context<Self>) {
        let engine = self.engine();
        let speech = self.engine_of(engine);
        let events = self.events.clone();
        let spawned = (self.spawn)(Box::new(move || {
            let voices = speech.voices();
            let _ = events.unbounded_send(ReadEvent::Voices { engine, voices });
        }));
        if let Err(error) = spawned {
            self.voices = Some(Err(SpeechError::failed(error.to_string())));
        }
        cx.notify();
    }

    fn handle(&mut self, event: ReadEvent, cx: &mut Context<Self>) {
        match event {
            ReadEvent::Voices { engine, voices } if engine == self.engine() => {
                if let Err(error) = &voices {
                    log::info!("{error}");
                }
                self.voices = Some(voices);
            }
            ReadEvent::Voices { .. } => {}
            ReadEvent::Speech { job, event } => {
                if self.job.as_ref().is_some_and(|running| running.id == job) {
                    self.speech(event);
                }
            }
        }
        cx.notify();
    }

    fn speech(&mut self, event: SpeechEvent) {
        let (Some(job), Some(audio)) = (&mut self.job, &mut self.audio) else {
            return;
        };
        match event {
            SpeechEvent::Audio {
                samples,
                done,
                total,
            } => {
                audio.player.append(&samples);
                (job.done, job.total) = (done, total);
                if job.play && !audio.player.is_playing() {
                    audio.player.play();
                }
            }
            SpeechEvent::Done => {
                audio.complete = true;
                hook::record_event("read", "spoken");
                let export = job.export_to.take();
                self.job = None;
                if let Some(path) = export {
                    self.write(&path);
                }
            }
            SpeechEvent::Cancelled => {
                self.job = None;
                self.audio = None;
            }
            SpeechEvent::Failed(error) => {
                log::warn!("{error}");
                self.error = Some(message_of(&error));
                self.job = None;
                self.audio = None;
                hook::record_event("read", &format!("failed {}", error.code));
            }
        }
    }

    // ponytail: the WAV is written on the UI thread, a pause of about a second for an hour of
    // speech; move it to a thread when texts that long are common.
    fn write(&mut self, path: &std::path::Path) {
        let Some(audio) = &self.audio else {
            return;
        };
        match audio.player.write_wav(path) {
            Ok(()) => {
                self.info = Some(format!("Saved the audio to {}.", shown_path(path)));
                hook::record_event("read", &format!("exported {}", path.display()));
            }
            Err(error) => {
                log::warn!("the read aloud audio was not written: {error}");
                self.error = Some(format!(
                    "The audio could not be saved to {}: {error}",
                    shown_path(path)
                ));
            }
        }
    }

    pub fn state_json(&self, cx: &App) -> Value {
        let voices = self.listed();
        json!({
            "engine": self.engine().key(),
            "engines": EngineKind::ALL.iter().map(|kind| json!({
                "key": kind.key(),
                "label": kind.label(),
                "available": self.engine_of(*kind).available(),
            })).collect::<Vec<_>>(),
            "voice": self.voice(),
            "spoken_voice": self.spoken_voice(cx),
            "default_voice": self.default_voice(cx),
            "default_voice_name": self.default_voice_name(cx),
            "library": self.library.is_some(),
            "offers_other_engine": self.offers_other_engine(),
            "speed": self.speed(),
            "format": self.format(),
            "language": self.language(),
            "languages": self.languages(),
            "voices": voices.iter().map(|voice| json!({
                "id": voice.id,
                "name": voice.name,
                "language": voice.language,
            })).collect::<Vec<_>>(),
            "loading": self.voices.is_none(),
            "unavailable": self.unavailable(),
            "needs_model": self.needs_model(),
            "text_chars": self.text.read(cx).value().chars().count(),
            "playing": self.playing(),
            "exporting": self.exporting(),
            "choosing": self.choosing,
            "progress": self.progress().map(|(done, total)| json!({"done": done, "total": total})),
            "audio_ms": self.audio.as_ref().map(|audio| audio.player.duration_ms()),
            "complete": self.audio.as_ref().is_some_and(|audio| audio.complete),
            "error": self.error,
            "info": self.info,
        })
    }
}

/// `en` for `en-US`, `en_GB`, or `en`.
fn primary(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or(tag)
        .to_ascii_lowercase()
}

/// The English name of a language, or its code when Hushpen has no name for it.
pub fn language_name(code: &str) -> String {
    hushpen_core::language::name(code).map_or_else(|| code.to_owned(), str::to_owned)
}

const NOT_ENGLISH: &str = "Chatterbox reads English only. Use the system voice for this text.";

/// What the view says for a failure. The code and the detail stay in the log.
fn message_of(error: &SpeechError) -> String {
    match error.code {
        SPEECH_NO_TEXT => "There is no text to read. Type some, or open a text file.".to_owned(),
        SPEECH_TEXT_TOO_LONG => "The text is too long. Read aloud takes up to 1 MB.".to_owned(),
        SPEECH_TEXT_UNREADABLE => "That file could not be read as text.".to_owned(),
        SPEECH_VOICE_MISSING => "That voice is not installed. Choose another voice.".to_owned(),
        SPEECH_UNAVAILABLE => "This voice engine is not available here.".to_owned(),
        SPEECH_NOT_ENGLISH => NOT_ENGLISH.to_owned(),
        SPEECH_VOICE_CLIP => {
            "The clip of this voice could not be used. Choose another voice.".to_owned()
        }
        _ => "The voice stopped while reading. Try again.".to_owned(),
    }
}

#[cfg(test)]
mod tests;
