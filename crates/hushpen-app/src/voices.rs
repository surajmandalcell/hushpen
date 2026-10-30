//! The voice library behind Read aloud: the voices the user cloned for Chatterbox, and the
//! default one. The "Manage Voices" sheet lists them with the voice that comes with the model,
//! plays each clip, renames and deletes them, and sets the default. Its second page adds a
//! voice from a guided microphone take or an audio file.
//!
//! The clips live in `voices/<id>/` (see `hushpen_store::voices`); the default voice is the
//! setting [`DEFAULT_SETTING`], empty for the voice of the model.

pub mod clip;
pub mod sheet;
#[cfg(test)]
mod tests;

use crate::hook;
use crate::mic::{CaptureState, CaptureUse, Mic};
use crate::models::Spawner;
use crate::storage::Storage;
use clip::{Clip, ClipProblem};
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Window,
};
use hushpen_audio::{Player, write_wav};
use hushpen_core::chatterbox::{self, MAX_CLIP_SECS, MIN_CLIP_SECS, SAMPLE_RATE};
use hushpen_store::model_files;
use hushpen_store::voices::{Library, NameProblem, VoiceEntry};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The id of the default voice; empty for the voice that comes with the model.
pub const DEFAULT_SETTING: &str = "readAloud.defaultVoice";
/// The longest take. The clip keeps at most its first 20 seconds of speech.
pub const TAKE_SECONDS: u32 = MAX_CLIP_SECS as u32;
/// How often a preview looks whether it has finished.
const PLAY_POLL: Duration = Duration::from_millis(100);

/// What the guided take asks the user to read: plain lines with many of the sounds of English.
pub const READING: [&str; 4] = [
    "The north wind and the sun argued about which was stronger.",
    "A traveler came along, wrapped in a warm cloak.",
    "They agreed that whoever made him take it off would win.",
    "The wind blew hard, but the traveler held his cloak tighter.",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    List,
    Add,
}

impl Page {
    fn key(self) -> &'static str {
        match self {
            Page::List => "list",
            Page::Add => "add",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    Idle,
    Recording {
        seconds: u32,
    },
    /// The take or the file is being made into a clip.
    Preparing,
}

/// What a preview plays: a voice of the list by id, or the new clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Playing {
    Voice(String),
    NewClip,
}

/// The voices or the default changed, so Read aloud lists them again.
pub struct Changed;

enum Message {
    Prepared {
        take: u64,
        result: Result<Clip, ClipProblem>,
    },
}

/// The focus handles of one row of the list: play, rename, delete, and Set as Default.
pub struct RowFocus {
    pub play: FocusHandle,
    pub rename: FocusHandle,
    pub delete: FocusHandle,
    pub default: FocusHandle,
}

/// The focus handles of the sheet. `trap` is the sheet itself, which keeps Tab inside it.
pub struct SheetFocus {
    pub trap: FocusHandle,
    pub add: FocusHandle,
    pub done: FocusHandle,
    pub back: FocusHandle,
    pub save: FocusHandle,
    pub record: FocusHandle,
    pub play: FocusHandle,
    pub import: FocusHandle,
    pub confirm: FocusHandle,
    pub cancel: FocusHandle,
    /// The built-in voice, then each cloned voice.
    pub rows: Vec<RowFocus>,
}

pub struct Parts {
    pub storage: Rc<Storage>,
    /// `None` where no microphone can record, as in the pixel tests.
    pub mic: Option<Entity<Mic>>,
    pub spawn: Spawner,
    /// Plays into no device, for tests.
    pub silent: bool,
}

pub struct VoiceLibrary {
    storage: Rc<Storage>,
    library: Library,
    mic: Option<Entity<Mic>>,
    spawn: Spawner,
    silent: bool,
    window: AnyWindowHandle,
    messages: UnboundedSender<Message>,
    page: Option<Page>,
    voices: Vec<VoiceEntry>,
    take: Take,
    /// Counts the takes, so the countdown or the clip of an old one is dropped.
    takes: u64,
    /// The clip of the voice being added.
    clip: Option<Arc<Clip>>,
    pub(crate) name: Entity<InputState>,
    pub(crate) rename: Entity<InputState>,
    /// The voice whose name is being changed.
    renaming: Option<String>,
    /// The voice whose row asks to confirm the delete.
    deleting: Option<String>,
    player: Option<(Player, Playing)>,
    /// Counts the previews, so the watch of an old one stops.
    plays: u64,
    notice: Option<String>,
    /// Where focus goes when the sheet closes.
    return_focus: Option<FocusHandle>,
    pub(crate) focus: SheetFocus,
}

impl EventEmitter<Changed> for VoiceLibrary {}

impl VoiceLibrary {
    pub fn new(parts: Parts, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Parts {
            storage,
            mic,
            spawn,
            silent,
        } = parts;
        let (messages, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(message) = queue.next().await {
                if this.update(cx, |me, cx| me.handle(message, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        if let Some(mic) = &mic {
            cx.observe(mic, |me, mic, cx| {
                if let Take::Recording { .. } = me.take
                    && mic.read(cx).session_use() != Some(CaptureUse::Voice)
                {
                    // The microphone failed or went away under the take.
                    me.take = Take::Idle;
                    me.notice = Some("The microphone stopped. Record again.".into());
                }
                cx.notify();
            })
            .detach();
        }
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let rename = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        cx.subscribe_in(&name, window, |me, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) && me.clip.is_some() {
                // A refusal shows in the notice row.
                let _ = me.save(cx);
            }
        })
        .detach();
        cx.subscribe_in(&rename, window, |me, _, event: &InputEvent, window, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                // A refusal shows in the notice row.
                let _ = me.save_rename(Some(window), cx);
            }
        })
        .detach();
        let stop = |cx: &mut Context<Self>| cx.focus_handle().tab_stop(true);
        let focus = SheetFocus {
            trap: cx.focus_handle(),
            add: stop(cx),
            done: stop(cx),
            back: stop(cx),
            save: stop(cx),
            record: stop(cx),
            play: stop(cx),
            import: stop(cx),
            confirm: stop(cx),
            cancel: stop(cx),
            rows: Vec::new(),
        };
        let library = Library::new(storage.data.voices_dir());
        let mut me = Self {
            storage,
            library,
            mic,
            spawn,
            silent,
            window: window.window_handle(),
            messages,
            page: None,
            voices: Vec::new(),
            take: Take::Idle,
            takes: 0,
            clip: None,
            name,
            rename,
            renaming: None,
            deleting: None,
            player: None,
            plays: 0,
            notice: None,
            return_focus: None,
            focus,
        };
        me.reload(cx);
        me
    }

    pub fn voices(&self) -> &[VoiceEntry] {
        &self.voices
    }

    pub fn page(&self) -> Option<Page> {
        self.page
    }

    pub fn take(&self) -> Take {
        self.take
    }

    pub fn clip(&self) -> Option<&Clip> {
        self.clip.as_deref()
    }

    pub fn playing(&self) -> Option<&Playing> {
        self.player.as_ref().map(|(_, playing)| playing)
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn renaming(&self) -> Option<&str> {
        self.renaming.as_deref()
    }

    pub fn deleting(&self) -> Option<&str> {
        self.deleting.as_deref()
    }

    pub fn can_record(&self) -> bool {
        self.mic.is_some()
    }

    /// The microphone level while a take records.
    pub fn meter(&self, cx: &App) -> Vec<f32> {
        self.mic
            .as_ref()
            .map(|mic| mic.read(cx).meter())
            .unwrap_or_default()
    }

    /// The default voice, or `None` for the voice that comes with the model.
    pub fn default_id(&self) -> Option<String> {
        default_voice(&self.storage, &self.voices)
    }

    /// The name of the default voice, as Read aloud and the sheet show it.
    pub fn default_name(&self) -> String {
        let id = self.default_id();
        self.voices
            .iter()
            .find(|voice| Some(&voice.id) == id.as_ref())
            .map_or_else(|| BUILT_IN_NAME.to_owned(), |voice| voice.name.clone())
    }

    /// The clip of the voice that comes with the model, from the first Chatterbox model that is
    /// downloaded.
    pub fn built_in_clip(&self) -> Option<PathBuf> {
        let models = self.storage.data.tts_models_dir();
        chatterbox::MODEL_IDS
            .iter()
            .map(|id| models.join(id).join(chatterbox::DEFAULT_VOICE_FILE))
            .find(|clip| model_files::is_verified(clip))
    }

    /// Reads the list again from disk.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.voices = self.library.list();
        while self.focus.rows.len() < self.voices.len() + 1 {
            let stop = |cx: &mut Context<Self>| cx.focus_handle().tab_stop(true);
            let row = RowFocus {
                play: stop(cx),
                rename: stop(cx),
                delete: stop(cx),
                default: stop(cx),
            };
            self.focus.rows.push(row);
        }
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.reload(cx);
        cx.emit(Changed);
    }

    /// Opens the sheet on `page` and puts focus in it. `back` gets the focus when it closes.
    pub fn open(
        &mut self,
        page: Page,
        back: Option<FocusHandle>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page.is_none() {
            self.return_focus = back;
        }
        self.show(page, cx);
        let focus = match page {
            Page::List => self.focus.add.clone(),
            Page::Add if self.can_record() => self.focus.record.clone(),
            Page::Add => self.focus.import.clone(),
        };
        window.focus(&focus, cx);
    }

    /// Opens the sheet on `page` without moving focus, as the test hook does.
    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == Page::Add && self.page != Some(Page::Add) {
            self.clip = None;
            let name = self.library.next_name();
            self.set_input(&self.name.clone(), name, cx);
        }
        if page == Page::List {
            self.drop_take(cx);
        }
        self.reload(cx);
        self.page = Some(page);
        self.renaming = None;
        self.deleting = None;
        self.notice = None;
        self.stop_playing(cx);
        hook::record_event("voices", &format!("sheet {}", page.key()));
        cx.notify();
    }

    /// Closes the sheet and gives focus back to the control that opened it.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page.is_none() {
            return;
        }
        self.dismiss(cx);
        if let Some(focus) = self.return_focus.take() {
            window.focus(&focus, cx);
        }
    }

    /// Closes the sheet without moving focus, for a view change that hides it. A take is
    /// dropped and a preview stops.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.page.take().is_none() {
            return;
        }
        self.drop_take(cx);
        self.stop_playing(cx);
        self.renaming = None;
        self.deleting = None;
        hook::record_event("voices", "sheet closed");
        cx.notify();
    }

    /// Makes voice `id` the default, or the voice of the model with `None`.
    pub fn set_default(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        if let Some(id) = id
            && !self.voices.iter().any(|voice| voice.id == id)
        {
            return Err(format!("There is no voice `{id}`."));
        }
        self.storage
            .settings
            .set(DEFAULT_SETTING, json!(id.unwrap_or("")))
            .map_err(|error| format!("The default voice could not be saved: {error}"))?;
        self.notice = None;
        hook::record_event("voices", &format!("default {}", id.unwrap_or("built-in")));
        self.changed(cx);
        Ok(())
    }

    /// Shows the name field in the row of voice `id`.
    pub fn start_rename(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(voice) = self.voices.iter().find(|voice| voice.id == id) else {
            return Err(format!("There is no voice `{id}`."));
        };
        let name = voice.name.clone();
        self.deleting = None;
        self.renaming = Some(id.to_owned());
        self.notice = None;
        self.set_input(&self.rename.clone(), name, cx);
        cx.notify();
        Ok(())
    }

    pub fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            self.notice = None;
            cx.notify();
        }
    }

    /// Keeps the name typed in the row that is being renamed. Focus goes back to its rename
    /// button when there is a window.
    pub fn save_rename(
        &mut self,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let Some(id) = self.renaming.clone() else {
            return Err("No voice is being renamed.".into());
        };
        let name = self.rename.read(cx).value().to_string();
        self.rename_voice(&id, &name, cx)?;
        if let Some(window) = window
            && let Some(index) = self.voices.iter().position(|voice| voice.id == id)
        {
            window.focus(&self.focus.rows[index + 1].rename, cx);
        }
        Ok(())
    }

    /// Gives voice `id` a new name.
    pub fn rename_voice(
        &mut self,
        id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let refused = match self.library.check_name(name, Some(id)) {
            Ok(_) => None,
            Err(problem) => Some(name_message(problem)),
        };
        if let Some(message) = refused {
            return self.refuse(message, cx);
        }
        if let Err(error) = self.library.rename(id, name) {
            log::warn!("VOICE_RENAME_FAILED {error}");
            return self.refuse("The new name could not be saved.".into(), cx);
        }
        self.renaming = None;
        self.notice = None;
        hook::record_event("voices", &format!("renamed {id}"));
        self.changed(cx);
        Ok(())
    }

    /// Asks in the row of voice `id` whether to delete it.
    pub fn ask_delete(&mut self, id: &str, cx: &mut Context<Self>) {
        self.renaming = None;
        self.deleting = Some(id.to_owned());
        cx.notify();
    }

    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        if self.deleting.take().is_some() {
            cx.notify();
        }
    }

    /// Deletes voice `id`, its clip, and what the voice worker computed from it. A setting
    /// that chose it goes back to the default.
    pub fn delete(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if self.playing() == Some(&Playing::Voice(id.to_owned())) {
            self.stop_playing(cx);
        }
        if let Err(error) = self.library.delete(id) {
            log::warn!("VOICE_DELETE_FAILED {error}");
            return self.refuse("The voice could not be deleted.".into(), cx);
        }
        for key in [DEFAULT_SETTING, crate::read_aloud::CHATTERBOX_VOICE_SETTING] {
            let chosen = self.storage.settings.get(key);
            if chosen.as_ref().and_then(Value::as_str) == Some(id)
                && let Err(error) = self.storage.settings.set(key, json!(""))
            {
                log::warn!("could not clear {key}: {error}");
            }
        }
        self.deleting = None;
        self.notice = None;
        hook::record_event("voices", &format!("deleted {id}"));
        self.changed(cx);
        Ok(())
    }

    /// The record button: starts a take, or stops the one that runs.
    pub fn toggle_take(&mut self, cx: &mut Context<Self>) {
        match self.take {
            Take::Recording { .. } => self.stop_take(cx),
            Take::Idle => self.start_take(cx),
            Take::Preparing => {}
        }
    }

    /// Records the microphone as it is, for at most [`TAKE_SECONDS`].
    pub fn start_take(&mut self, cx: &mut Context<Self>) {
        let Some(mic) = self.mic.clone() else {
            return;
        };
        if self.take != Take::Idle {
            return;
        }
        self.stop_playing(cx);
        if let Err(error) = mic.update(cx, |mic, cx| mic.start(CaptureUse::Voice, cx)) {
            self.notice = Some(match error.code {
                hushpen_core::error::MIC_BUSY => {
                    "The microphone is in use. Try again when it is free.".into()
                }
                _ => mic.read(cx).notice().map_or_else(
                    || "The microphone could not start.".into(),
                    |notice| notice.message.clone(),
                ),
            });
            cx.notify();
            return;
        }
        self.notice = None;
        self.take = Take::Recording { seconds: 0 };
        self.takes += 1;
        let take = self.takes;
        hook::record_event("voices", "take start");
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                match this.update(cx, |me, cx| me.tick(take, cx)) {
                    Ok(true) => {}
                    _ => break,
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// One second of take `take` has passed. Returns whether it still runs.
    fn tick(&mut self, take: u64, cx: &mut Context<Self>) -> bool {
        let Take::Recording { seconds } = self.take else {
            return false;
        };
        if take != self.takes {
            return false;
        }
        let seconds = seconds + 1;
        self.take = Take::Recording { seconds };
        if seconds >= TAKE_SECONDS {
            self.stop_take(cx);
            return false;
        }
        cx.notify();
        true
    }

    /// Ends the take and makes it into the clip of the new voice.
    pub fn stop_take(&mut self, cx: &mut Context<Self>) {
        let (Take::Recording { .. }, Some(mic)) = (self.take, self.mic.clone()) else {
            return;
        };
        self.take = Take::Idle;
        let finished = mic.update(cx, |mic, cx| mic.stop(true, cx));
        hook::record_event("voices", "take stop");
        match finished {
            Ok(Some(finished)) => self.prepare(finished.path, true, cx),
            Ok(None) | Err(_) => {
                self.notice = Some("The take could not be read. Record again.".into());
            }
        }
        cx.notify();
    }

    fn drop_take(&mut self, cx: &mut Context<Self>) {
        match self.take {
            Take::Recording { .. } => {
                if let Some(mic) = &self.mic {
                    let _ = mic.update(cx, |mic, cx| mic.stop(false, cx));
                }
            }
            Take::Preparing => self.takes += 1,
            Take::Idle => return,
        }
        self.take = Take::Idle;
        cx.notify();
    }

    /// Opens the system's file picker for an audio file to clone the voice from.
    pub fn choose_file(&mut self, cx: &mut Context<Self>) {
        if self.take != Take::Idle {
            return;
        }
        cx.spawn(async move |this, cx| {
            let paths = crate::import::pick_files(cx).await;
            let _ = this.update(cx, |me, cx| {
                if let Some(path) = paths.and_then(|paths| paths.into_iter().next()) {
                    me.load_file(path, cx);
                }
            });
        })
        .detach();
    }

    /// Makes the audio file at `path` into the clip of the new voice.
    pub fn load_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.take != Take::Idle {
            return;
        }
        self.stop_playing(cx);
        hook::record_event("voices", "file chosen");
        self.prepare(path, false, cx);
    }

    /// Decodes and trims `path` on a worker thread. A take's file is deleted after.
    fn prepare(&mut self, path: PathBuf, take_file: bool, cx: &mut Context<Self>) {
        self.takes += 1;
        let take = self.takes;
        self.take = Take::Preparing;
        self.notice = None;
        let messages = self.messages.clone();
        let job = Box::new(move || {
            let result = clip::read_file(&path).and_then(|audio| clip::prepare(&audio));
            if take_file {
                let _ = std::fs::remove_file(&path);
            }
            let _ = messages.unbounded_send(Message::Prepared { take, result });
        });
        if let Err(error) = (self.spawn)(job) {
            log::warn!("the voice clip could not be made: {error}");
            self.take = Take::Idle;
            self.notice = Some("The clip could not be made. Try again.".into());
        }
        cx.notify();
    }

    fn handle(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Prepared { take, result } if take == self.takes => {
                self.take = Take::Idle;
                match result {
                    Ok(clip) => {
                        self.notice = clip.cut.then(|| {
                            format!("The clip keeps the first {TAKE_SECONDS} s of speech.")
                        });
                        hook::record_event("voices", "clip ready");
                        self.clip = Some(Arc::new(clip));
                    }
                    Err(problem) => {
                        hook::record_event("voices", "clip refused");
                        self.notice = Some(problem.0);
                    }
                }
                cx.notify();
            }
            Message::Prepared { .. } => {}
        }
    }

    /// Keeps the new clip as a voice, under the name in the name field.
    pub fn save(&mut self, cx: &mut Context<Self>) -> Result<VoiceEntry, String> {
        let Some(clip) = self.clip.clone() else {
            return self.refuse("Record a clip or choose a file first.".into(), cx);
        };
        let name = self.name.read(cx).value().to_string();
        if let Err(problem) = self.library.check_name(&name, None) {
            return self.refuse(name_message(problem), cx);
        }
        let written = self.library.add(
            &name,
            clip.duration_ms(),
            hushpen_store::time::now_unix_ms(),
            |path| write_wav(path, SAMPLE_RATE, &clip.samples).map_err(std::io::Error::other),
        );
        let entry = match written {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!("VOICE_ADD_FAILED {error}");
                return self.refuse("The voice could not be saved.".into(), cx);
            }
        };
        self.stop_playing(cx);
        self.clip = None;
        hook::record_event("voices", &format!("added {}", entry.id));
        self.changed(cx);
        self.show(Page::List, cx);
        Ok(entry)
    }

    /// The play button of `what`: plays, or stops what plays.
    pub fn toggle_play(&mut self, what: Playing, cx: &mut Context<Self>) {
        if self.playing() == Some(&what) {
            self.stop_playing(cx);
            return;
        }
        let audio = match &what {
            Playing::NewClip => self
                .clip
                .as_ref()
                .map(|clip| (SAMPLE_RATE, clip.samples.clone())),
            Playing::Voice(id) => self
                .clip_of(id)
                .and_then(|path| clip::read_file(&path).ok())
                .map(|samples| (hushpen_audio::resample::TARGET_RATE, samples)),
        };
        let Some((rate, samples)) = audio else {
            self.notice = Some("This voice has no clip to play.".into());
            cx.notify();
            return;
        };
        let player = match Player::from_samples(rate, samples) {
            Ok(player) if self.silent => player.silenced(),
            Ok(player) => player,
            Err(error) => {
                self.notice = Some(format!("The clip could not play: {error}"));
                cx.notify();
                return;
            }
        };
        self.stop_playing(cx);
        let mut player = player;
        player.play();
        self.player = Some((player, what));
        self.plays += 1;
        let play = self.plays;
        hook::record_event("voices", "play");
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PLAY_POLL).await;
                match this.update(cx, |me, cx| me.still_playing(play, cx)) {
                    Ok(true) => {}
                    _ => break,
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn clip_of(&self, id: &str) -> Option<PathBuf> {
        if id == chatterbox::BUILT_IN_VOICE {
            self.built_in_clip()
        } else {
            self.library.clip_path(id).filter(|path| path.is_file())
        }
    }

    fn still_playing(&mut self, play: u64, cx: &mut Context<Self>) -> bool {
        if play != self.plays {
            return false;
        }
        match &self.player {
            Some((player, _)) if player.is_playing() => true,
            Some((player, _)) => {
                if let Some(error) = player.error() {
                    self.notice = Some(format!("The clip could not play: {error}"));
                }
                self.player = None;
                cx.notify();
                false
            }
            None => false,
        }
    }

    pub fn stop_playing(&mut self, cx: &mut Context<Self>) {
        if self.player.take().is_some() {
            self.plays += 1;
            cx.notify();
        }
    }

    /// Puts `text` in a field of the sheet.
    fn set_input(&self, input: &Entity<InputState>, text: String, cx: &mut Context<Self>) {
        let input = input.clone();
        let _ = self.window.update(cx, |_, window, cx| {
            input.update(cx, |state, cx| state.set_value(text, window, cx));
        });
    }

    fn refuse<T>(&mut self, message: String, cx: &mut Context<Self>) -> Result<T, String> {
        self.notice = Some(message.clone());
        hook::record_event("voices", "refused");
        cx.notify();
        Err(message)
    }

    /// `hookctl state` section `voices`.
    pub fn state_json(&self, cx: &App) -> Value {
        let default = self.default_id();
        json!({
            "sheet": self.page.map(Page::key),
            "voices": self.voices.iter().map(|voice| json!({
                "id": voice.id,
                "name": voice.name,
                "created_ms": voice.created_ms,
                "duration_ms": voice.duration_ms,
                "clip": self.library.clip_path(&voice.id).map(|path| path.display().to_string()),
                "default": Some(&voice.id) == default.as_ref(),
            })).collect::<Vec<_>>(),
            "default": default,
            "default_name": self.default_name(),
            "built_in_clip": self.built_in_clip().map(|path| path.display().to_string()),
            "take": match self.take {
                Take::Idle => json!({"state": "idle"}),
                Take::Recording { seconds } => json!({"state": "recording", "seconds": seconds}),
                Take::Preparing => json!({"state": "preparing"}),
            },
            "clip": self.clip.as_ref().map(|clip| json!({
                "duration_ms": clip.duration_ms(),
                "cut": clip.cut,
            })),
            "name": self.name.read(cx).value().to_string(),
            "renaming": self.renaming,
            "deleting": self.deleting,
            "playing": self.playing().map(|playing| match playing {
                Playing::Voice(id) => json!(id),
                Playing::NewClip => json!("new"),
            }),
            "notice": self.notice,
            "can_record": self.can_record(),
            "listening": self.mic.as_ref().is_some_and(|mic| mic.read(cx).state() == CaptureState::Listening),
        })
    }
}

/// The name of the voice that comes with a Chatterbox model.
pub const BUILT_IN_NAME: &str = "Built-in";

/// The default voice of `voices` as the setting names it, or `None` for the voice of the model
/// (also when the setting names a voice that is gone).
pub fn default_voice(storage: &Storage, voices: &[VoiceEntry]) -> Option<String> {
    storage
        .settings
        .get(DEFAULT_SETTING)
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|id| voices.iter().any(|voice| &voice.id == id))
}

/// What the sheet says for a name it cannot take.
fn name_message(problem: NameProblem) -> String {
    match problem {
        NameProblem::Empty => "Give the voice a name.".to_owned(),
        NameProblem::TooLong => format!(
            "A name has at most {} characters.",
            hushpen_store::voices::MAX_NAME_CHARS
        ),
        NameProblem::Taken => "Another voice has this name. Choose another one.".to_owned(),
    }
}

/// `12 s · Added 2026-05-15`: the length of the clip and the day it was added.
pub fn detail(voice: &VoiceEntry) -> String {
    let added = crate::history::panel::when(i64::try_from(voice.created_ms).unwrap_or(0));
    format!(
        "{} s \u{b7} Added {}",
        voice.duration_ms.div_ceil(1000),
        added.get(..10).unwrap_or(&added)
    )
}

/// The help under the take row.
pub fn take_detail(take: Take, clip: Option<&Clip>) -> String {
    let min = MIN_CLIP_SECS as u32;
    match (take, clip) {
        (Take::Recording { seconds }, _) if seconds < min => {
            format!("Speak now. Keep going for {} s", min - seconds)
        }
        (Take::Recording { seconds }, _) => {
            format!(
                "Speak now. {} s left, or press Stop",
                TAKE_SECONDS - seconds
            )
        }
        (Take::Preparing, _) => "Getting the clip ready\u{2026}".to_owned(),
        (Take::Idle, Some(clip)) => format!(
            "{} s of speech, ready to save",
            clip.duration_ms().div_ceil(1000)
        ),
        (Take::Idle, None) => format!("Read the lines above, {min} to {TAKE_SECONDS} s"),
    }
}
