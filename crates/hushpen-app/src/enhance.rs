//! Audio enhance for the microphone: the setting (Off, Auto, or Custom and its values) and the
//! tuning sheet behind Settings > Audio. The sheet keeps the last take on disk, as the
//! microphone gave it, and plays it through any setting, so a change is heard on the next play.
//! "Find best settings" transcribes one reading of known text with many settings and ranks them
//! by the words they got wrong.

mod bench;
mod score;
pub mod sheet;
#[cfg(test)]
mod tests;

pub use bench::{Candidate, Outcome, Scored, signed_db};

use crate::controller::{CancelToken, LANGUAGE_SETTING, Runner};
use crate::hook;
use crate::mic::{CaptureState, CaptureUse, Mic};
use crate::models::{Models, Spawner};
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{App, Context, Entity, FocusHandle, Window};
use hushpen_audio::decode::FileDecoder;
use hushpen_audio::{Enhance, Player, enhance, write_wav};
use hushpen_core::error::ENGINE_NO_SPEECH;
use hushpen_core::language;
use hushpen_engine::{JobOutcome, TranscribeSpec};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

pub const MODE: &str = "audio.enhance";
pub const LOW_CUT: &str = "audio.enhance.lowCutHz";
pub const BASS: &str = "audio.enhance.bassDb";
pub const TREBLE: &str = "audio.enhance.trebleDb";
pub const NOISE: &str = "audio.enhance.noiseReduction";

/// The rate of the kept take and of every preview: the rate of the session WAV.
const RATE: u32 = 16_000;
/// How long the take on the tuning page runs.
pub const SAMPLE_SECONDS: u32 = 8;
/// The longest reading for the search. It stops by itself after this.
pub const READING_SECONDS: u32 = 30;
/// A take shorter than this holds too little speech to judge a setting by.
const SHORTEST_TAKE: usize = RATE as usize;
/// How often a preview looks whether it has finished.
const PLAY_POLL: Duration = Duration::from_millis(100);

/// The text the search asks for: short plain lines with sounds that accents and speech models
/// often mix up (v and w, th, r, and a number).
pub const SENTENCES: [&str; 3] = [
    "We visited the village twice in very wet weather.",
    "The three brothers thought about it on Thursday.",
    "Ravi wrote 250 words for the radio report.",
];
/// How many words of the Dictionary the reading adds.
const DICTIONARY_WORDS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Off,
    Auto,
    Custom,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::Auto, Mode::Custom];

    pub fn key(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Auto => "auto",
            Mode::Custom => "custom",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Off => "Off",
            Mode::Auto => "Auto",
            Mode::Custom => "Custom",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }
}

pub fn mode(storage: &Storage) -> Mode {
    storage
        .settings
        .get(MODE)
        .and_then(|value| value.as_str().and_then(Mode::from_key))
        .unwrap_or(Mode::Auto)
}

/// The Custom values as saved, whatever the mode.
pub fn custom(storage: &Storage) -> Enhance {
    let number = |key, default: f64| {
        storage
            .settings
            .get(key)
            .and_then(|value| value.as_f64())
            .unwrap_or(default)
    };
    Enhance {
        low_cut_hz: number(LOW_CUT, f64::from(Enhance::AUTO.low_cut_hz)) as u16,
        bass_db: number(BASS, f64::from(Enhance::AUTO.bass_db)) as f32,
        treble_db: number(TREBLE, f64::from(Enhance::AUTO.treble_db)) as f32,
        noise_reduction: storage
            .settings
            .get(NOISE)
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

/// What the microphone runs through now.
pub fn effective(storage: &Storage) -> Enhance {
    match mode(storage) {
        Mode::Off => Enhance::OFF,
        Mode::Auto => Enhance::AUTO,
        Mode::Custom => custom(storage),
    }
}

/// The page of the open sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Tune,
    Find,
}

impl Page {
    fn key(self) -> &'static str {
        match self {
            Page::Tune => "tune",
            Page::Find => "find",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    Idle,
    /// The microphone records a take: the reading of the search (`find`) or a sample.
    Recording {
        find: bool,
        seconds: u32,
    },
}

/// How far "Find best settings" is.
#[derive(Debug, Clone, PartialEq)]
pub enum Find {
    /// Waiting for the reading.
    Reading,
    Testing {
        done: usize,
        about: usize,
    },
    Results(Outcome),
    Failed(String),
}

/// What a preview plays: the sample with the current setting, or with result `n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    Sample,
    Result(usize),
}

/// Why a search stopped early.
enum Stop {
    Cancelled,
    Failed(String),
}

enum Message {
    Progress {
        run: u64,
        done: usize,
        about: usize,
    },
    Finished {
        run: u64,
        outcome: Result<Outcome, String>,
    },
}

/// The focus handles of the sheet. `trap` is the sheet itself, which keeps Tab inside it.
pub struct SheetFocus {
    pub trap: FocusHandle,
    pub record: FocusHandle,
    pub play: FocusHandle,
    pub low_cut: FocusHandle,
    pub bass: FocusHandle,
    pub treble: FocusHandle,
    pub noise: FocusHandle,
    pub reset: FocusHandle,
    pub find: FocusHandle,
    pub back: FocusHandle,
    pub cancel: FocusHandle,
    pub done: FocusHandle,
    pub result_play: Vec<FocusHandle>,
    pub result_use: Vec<FocusHandle>,
}

impl SheetFocus {
    fn new(cx: &mut App) -> Self {
        let stop = |cx: &mut App| cx.focus_handle().tab_stop(true);
        Self {
            trap: cx.focus_handle(),
            record: stop(cx),
            play: stop(cx),
            low_cut: stop(cx),
            bass: stop(cx),
            treble: stop(cx),
            noise: stop(cx),
            reset: stop(cx),
            find: stop(cx),
            back: stop(cx),
            cancel: stop(cx),
            done: stop(cx),
            result_play: (0..bench::SHOWN).map(|_| stop(cx)).collect(),
            result_use: (0..bench::SHOWN).map(|_| stop(cx)).collect(),
        }
    }
}

pub struct Parts {
    pub storage: Rc<Storage>,
    pub mic: Entity<Mic>,
    /// Runs one transcription with the loaded speech model, as a dictation does.
    pub run: Runner,
    pub spawn: Spawner,
}

pub struct Tuning {
    storage: Rc<Storage>,
    mic: Entity<Mic>,
    run: Runner,
    spawn: Spawner,
    messages: UnboundedSender<Message>,
    page: Option<Page>,
    take: Take,
    /// Counts the takes, so the countdown of an old one stops.
    takes: u64,
    /// The kept take, 16 kHz mono, as the microphone gave it.
    sample: Option<Arc<Vec<f32>>>,
    player: Option<(Player, Playing)>,
    /// Counts the previews, so the watch of an old one stops.
    plays: u64,
    find: Find,
    /// Counts the searches, so the messages of a cancelled one are dropped.
    runs: u64,
    cancel: Option<Arc<CancelToken>>,
    notice: Option<String>,
    /// Where focus goes when the sheet closes.
    return_focus: Option<FocusHandle>,
    /// Tells which speech model the search runs on, so only a Whisper model gets the
    /// dictionary prompt.
    models: Option<Entity<Models>>,
    /// Plays into no device, for tests.
    silent: bool,
    pub(crate) focus: SheetFocus,
}

impl Tuning {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let Parts {
            storage,
            mic,
            run,
            spawn,
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
        cx.observe(&mic, |me, mic, cx| {
            if let Take::Recording { .. } = me.take
                && mic.read(cx).session_use() != Some(CaptureUse::Sample)
            {
                // The microphone failed or went away under the take.
                me.take = Take::Idle;
                me.notice = Some("The microphone stopped. Record again.".into());
            }
            cx.notify();
        })
        .detach();
        let sample = read_samples(&sample_path(&storage)).ok().map(Arc::new);
        Self {
            storage,
            mic,
            run,
            spawn,
            messages,
            page: None,
            take: Take::Idle,
            takes: 0,
            sample,
            player: None,
            plays: 0,
            find: Find::Reading,
            runs: 0,
            cancel: None,
            notice: None,
            return_focus: None,
            models: None,
            silent: false,
            focus: SheetFocus::new(cx),
        }
    }

    pub fn attach_models(&mut self, models: Entity<Models>) {
        self.models = Some(models);
    }

    #[cfg(test)]
    pub(crate) fn play_silently(&mut self) {
        self.silent = true;
    }

    pub fn page(&self) -> Option<Page> {
        self.page
    }

    pub fn take(&self) -> Take {
        self.take
    }

    pub fn find(&self) -> &Find {
        &self.find
    }

    pub fn playing(&self) -> Option<Playing> {
        self.player.as_ref().map(|(_, playing)| *playing)
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// The length of the kept take in whole seconds, if there is one.
    pub fn sample_seconds(&self) -> Option<usize> {
        self.sample
            .as_ref()
            .map(|sample| sample.len().div_ceil(RATE as usize))
    }

    pub fn mode(&self) -> Mode {
        mode(&self.storage)
    }

    pub fn custom(&self) -> Enhance {
        custom(&self.storage)
    }

    /// The microphone level while a take records.
    pub fn meter(&self, cx: &App) -> Vec<f32> {
        self.mic.read(cx).meter()
    }

    fn gain(&self, cx: &App) -> f32 {
        self.mic.read(cx).gain()
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
            Page::Tune => &self.focus.done,
            Page::Find => &self.focus.record,
        };
        window.focus(focus, cx);
    }

    /// Opens the sheet on `page` without moving focus, as the test hook does.
    pub fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        if page == Page::Find && !matches!(self.find, Find::Testing { .. }) {
            self.find = Find::Reading;
        }
        self.page = Some(page);
        self.notice = None;
        hook::record_event("enhance", &format!("sheet {}", page.key()));
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
    /// dropped, a preview stops, and a search is cancelled.
    pub fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.page.take().is_none() {
            return;
        }
        if let Take::Recording { .. } = self.take {
            self.take = Take::Idle;
            let _ = self.mic.update(cx, |mic, cx| mic.stop(false, cx));
        }
        self.stop_playing(cx);
        self.cancel_search(cx);
        hook::record_event("enhance", "sheet closed");
        cx.notify();
    }

    /// Saves the mode and gives it to a running microphone.
    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.write(MODE, json!(mode.key()), cx);
    }

    /// Sets the low cut, from 0 (off) to 200 Hz in steps of 10 Hz.
    pub fn set_low_cut(&mut self, hz: f32, cx: &mut Context<Self>) {
        let stepped = ((hz.clamp(0.0, 200.0) / 10.0).round() * 10.0) as i64;
        self.write_value(LOW_CUT, stepped, cx);
    }

    /// Sets the bass shelf, from -12 to +6 dB in whole dB.
    pub fn set_bass(&mut self, db: f32, cx: &mut Context<Self>) {
        self.write_value(BASS, db.clamp(-12.0, 6.0).round() as i64, cx);
    }

    /// Sets the treble shelf, from -6 to +12 dB in whole dB.
    pub fn set_treble(&mut self, db: f32, cx: &mut Context<Self>) {
        self.write_value(TREBLE, db.clamp(-6.0, 12.0).round() as i64, cx);
    }

    pub fn set_noise_reduction(&mut self, on: bool, cx: &mut Context<Self>) {
        self.write_custom(NOISE, json!(on), cx);
    }

    /// Puts the Custom values back to the Auto preset.
    pub fn reset_to_auto(&mut self, cx: &mut Context<Self>) {
        self.write_enhance(Enhance::AUTO, cx);
        hook::record_event("enhance", "reset");
    }

    fn write_enhance(&mut self, enhance: Enhance, cx: &mut Context<Self>) {
        self.write_custom(LOW_CUT, json!(enhance.low_cut_hz), cx);
        self.write_custom(BASS, json!(enhance.bass_db.round() as i64), cx);
        self.write_custom(TREBLE, json!(enhance.treble_db.round() as i64), cx);
        self.write_custom(NOISE, json!(enhance.noise_reduction), cx);
    }

    /// Writes a whole number only when it changed, so a drag writes the file once a step.
    fn write_value(&mut self, key: &str, value: i64, cx: &mut Context<Self>) {
        let stored = self.storage.settings.get(key).and_then(|v| v.as_i64());
        if stored != Some(value) {
            self.write_custom(key, json!(value), cx);
        }
    }

    /// A Custom value changed: the mode is Custom from now on, as the sheet shows it.
    fn write_custom(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        self.write(key, value, cx);
        if self.mode() != Mode::Custom {
            self.set_mode(Mode::Custom, cx);
        }
    }

    fn write(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        match self.storage.settings.set(key, value) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", &format!("set {key}"));
            }
            Err(error) => self.notice = Some(format!("The setting could not be saved: {error}")),
        }
        self.mic.read(cx).apply_enhance();
        cx.notify();
    }

    /// The record button: starts a take, or stops the one that runs.
    pub fn toggle_take(&mut self, find: bool, cx: &mut Context<Self>) {
        match self.take {
            Take::Recording { .. } => self.stop_take(cx),
            Take::Idle => self.start_take(find, cx),
        }
    }

    /// Records a take as the microphone gives it: a sample of [`SAMPLE_SECONDS`], or the
    /// reading for the search, which runs until Stop or [`READING_SECONDS`].
    pub fn start_take(&mut self, find: bool, cx: &mut Context<Self>) {
        if self.take != Take::Idle || matches!(self.find, Find::Testing { .. }) {
            return;
        }
        self.stop_playing(cx);
        if let Err(error) = self
            .mic
            .update(cx, |mic, cx| mic.start(CaptureUse::Sample, cx))
        {
            self.notice = Some(match error.code {
                hushpen_core::error::MIC_BUSY => {
                    "The microphone is in use. Try again when it is free.".into()
                }
                _ => self.mic.read(cx).notice().map_or_else(
                    || "The microphone could not start.".into(),
                    |n| n.message.clone(),
                ),
            });
            cx.notify();
            return;
        }
        self.notice = None;
        if find {
            self.find = Find::Reading;
        }
        self.take = Take::Recording { find, seconds: 0 };
        self.takes += 1;
        let take = self.takes;
        hook::record_event(
            "enhance",
            if find {
                "reading start"
            } else {
                "sample start"
            },
        );
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
        let Take::Recording { find, seconds } = self.take else {
            return false;
        };
        if take != self.takes {
            return false;
        }
        let seconds = seconds + 1;
        self.take = Take::Recording { find, seconds };
        let limit = if find {
            READING_SECONDS
        } else {
            SAMPLE_SECONDS
        };
        if seconds >= limit {
            self.stop_take(cx);
            return false;
        }
        cx.notify();
        true
    }

    /// Ends the take and keeps it. The reading for the search starts the search.
    pub fn stop_take(&mut self, cx: &mut Context<Self>) {
        let Take::Recording { find, .. } = self.take else {
            return;
        };
        self.take = Take::Idle;
        let finished = self.mic.update(cx, |mic, cx| mic.stop(true, cx));
        hook::record_event("enhance", if find { "reading stop" } else { "sample stop" });
        let kept = match finished {
            Ok(Some(finished)) => self.keep(&finished.path),
            Ok(None) | Err(_) => Err("The take could not be read. Record again.".to_owned()),
        };
        match kept {
            Ok(()) if find => self.start_search(None, cx),
            Ok(()) => {}
            Err(message) => self.notice = Some(message),
        }
        cx.notify();
    }

    /// Makes the WAV at `path` the kept sample, in place of the one before.
    fn keep(&mut self, path: &Path) -> Result<(), String> {
        let samples = read_samples(path);
        let _ = std::fs::remove_file(path);
        let samples =
            samples.map_err(|_| "The take could not be read. Record again.".to_owned())?;
        self.keep_samples(samples)
    }

    fn keep_samples(&mut self, samples: Vec<f32>) -> Result<(), String> {
        if samples.len() < SHORTEST_TAKE {
            return Err("The take was too short. Record again.".to_owned());
        }
        let path = sample_path(&self.storage);
        if let Some(folder) = path.parent() {
            let _ = std::fs::create_dir_all(folder);
        }
        if let Err(error) = write_wav(&path, RATE, &samples) {
            log::warn!("the audio enhance sample could not be kept: {error}");
        }
        self.sample = Some(Arc::new(samples));
        Ok(())
    }

    /// Makes an audio file the kept sample, as if the microphone had recorded it.
    pub fn load_sample(&mut self, path: &Path, cx: &mut Context<Self>) -> Result<(), String> {
        let samples = read_samples(path)?;
        self.stop_playing(cx);
        self.keep_samples(samples)?;
        hook::record_event("enhance", "sample loaded");
        cx.notify();
        Ok(())
    }

    /// Plays the kept sample through the setting the microphone has now.
    pub fn play_sample(&mut self, cx: &mut Context<Self>) {
        let candidate = Candidate {
            enhance: effective(&self.storage),
            gain: self.gain(cx),
        };
        self.play(candidate, Playing::Sample, cx);
    }

    /// Plays the kept sample through result `index` of the search.
    pub fn play_result(&mut self, index: usize, cx: &mut Context<Self>) {
        let Find::Results(outcome) = &self.find else {
            return;
        };
        let Some(scored) = outcome.top().get(index) else {
            return;
        };
        let candidate = scored.candidate;
        self.play(candidate, Playing::Result(index), cx);
    }

    /// The play button of `what`: plays, or stops what plays.
    pub fn toggle_play(&mut self, what: Playing, cx: &mut Context<Self>) {
        if self.playing() == Some(what) {
            self.stop_playing(cx);
        } else {
            match what {
                Playing::Sample => self.play_sample(cx),
                Playing::Result(index) => self.play_result(index, cx),
            }
        }
    }

    fn play(&mut self, candidate: Candidate, what: Playing, cx: &mut Context<Self>) {
        let Some(sample) = self.sample.clone() else {
            return;
        };
        if self.take != Take::Idle {
            return;
        }
        let audio = enhance::apply(&sample, candidate.enhance, candidate.gain);
        let player = match Player::from_samples(RATE, audio) {
            Ok(player) if self.silent => player.silenced(),
            Ok(player) => player,
            Err(error) => {
                self.notice = Some(format!("The sample could not play: {error}"));
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
        hook::record_event("enhance", "play");
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

    fn still_playing(&mut self, play: u64, cx: &mut Context<Self>) -> bool {
        if play != self.plays {
            return false;
        }
        match &self.player {
            Some((player, _)) if player.is_playing() => true,
            Some((player, _)) => {
                if let Some(error) = player.error() {
                    self.notice = Some(format!("The sample could not play: {error}"));
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

    /// The lines the search asks the user to read: [`SENTENCES`], then a few Dictionary words.
    pub fn reading(&self) -> Vec<String> {
        let mut lines: Vec<String> = SENTENCES.iter().map(|line| (*line).to_owned()).collect();
        let words: Vec<String> = hushpen_store::dictionary::list(&self.storage.database)
            .unwrap_or_default()
            .into_iter()
            .map(|entry| entry.phrase)
            .filter(|phrase| phrase.split_whitespace().count() <= 2)
            .take(DICTIONARY_WORDS)
            .collect();
        if !words.is_empty() {
            lines.push(format!("{}.", words.join(", ")));
        }
        lines
    }

    /// Transcribes the kept take with each setting the search picks, on a worker thread, and
    /// scores it against `text` (the reading when `None`).
    pub fn start_search(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        let Some(sample) = self.sample.clone() else {
            self.notice = Some("Record the reading first.".into());
            cx.notify();
            return;
        };
        self.stop_playing(cx);
        self.cancel_search(cx);
        let reference = text.unwrap_or_else(|| self.reading().join(" "));
        let words = score::words(&reference).len();
        let auto = Candidate::auto(self.gain(cx));
        let language = self
            .storage
            .settings
            .get(LANGUAGE_SETTING)
            .and_then(|value| value.as_str().map(str::to_owned))
            .filter(|language| language != language::AUTO);
        let entries = hushpen_store::dictionary::list(&self.storage.database).unwrap_or_default();
        let model = self
            .models
            .as_ref()
            .map(|models| models.read(cx).effective())
            .unwrap_or_default();
        let prompt = hushpen_core::dictionary::prompt_for_model(&model, &entries);
        let path = sample_path(&self.storage).with_file_name("candidate.wav");
        let token = Arc::new(CancelToken::default());
        self.cancel = Some(Arc::clone(&token));
        self.runs += 1;
        let run_id = self.runs;
        let (run, messages) = (Arc::clone(&self.run), self.messages.clone());
        let job = Box::new(move || {
            let progress = |done, about| {
                let _ = messages.unbounded_send(Message::Progress {
                    run: run_id,
                    done,
                    about,
                });
            };
            let score = |candidate: &Candidate| {
                if token.is_cancelled() {
                    return Err(Stop::Cancelled);
                }
                let audio = enhance::apply(&sample, candidate.enhance, candidate.gain);
                write_wav(&path, RATE, &audio).map_err(|error| Stop::Failed(error.to_string()))?;
                let spec = TranscribeSpec {
                    wav_path: path.clone(),
                    language: language.clone(),
                    prompt: prompt.clone(),
                };
                match run(spec, Arc::clone(&token)) {
                    JobOutcome::Done(done) => Ok(score::word_errors(&reference, &done.text).0),
                    JobOutcome::Failed(failure) if failure.code == ENGINE_NO_SPEECH => Ok(words),
                    JobOutcome::Failed(failure) => Err(Stop::Failed(failure.code)),
                    JobOutcome::Cancelled => Err(Stop::Cancelled),
                }
            };
            let outcome = bench::search(auto, score, progress);
            let _ = std::fs::remove_file(&path);
            let outcome = match outcome {
                Ok(outcome) => Ok(outcome),
                Err(Stop::Cancelled) => return,
                Err(Stop::Failed(code)) => Err(code),
            };
            let _ = messages.unbounded_send(Message::Finished {
                run: run_id,
                outcome,
            });
        });
        self.find = Find::Testing { done: 0, about: 0 };
        self.notice = None;
        if let Err(error) = (self.spawn)(job) {
            log::warn!("the audio enhance search could not start: {error}");
            self.find = Find::Failed("The test could not start. Try again.".into());
        }
        hook::record_event("enhance", "search start");
        cx.notify();
    }

    pub fn cancel_search(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.cancel.take() {
            token.cancel();
        }
        if matches!(self.find, Find::Testing { .. }) {
            self.runs += 1;
            self.find = Find::Reading;
            hook::record_event("enhance", "search cancelled");
            cx.notify();
        }
    }

    fn handle(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Progress { run, done, about } if run == self.runs => {
                self.find = Find::Testing { done, about };
            }
            Message::Finished { run, outcome } if run == self.runs => {
                self.cancel = None;
                self.find = match outcome {
                    Ok(outcome) => {
                        hook::record_event("enhance", "search done");
                        Find::Results(outcome)
                    }
                    Err(code) => {
                        log::warn!("{code} the audio enhance search stopped");
                        hook::record_event("enhance", &format!("search failed {code}"));
                        Find::Failed(
                            "The speech model could not run. Check that one is ready in Models."
                                .into(),
                        )
                    }
                };
            }
            _ => return,
        }
        cx.notify();
    }

    /// Takes result `index` of the search: Custom with its values and its boost.
    pub fn use_result(&mut self, index: usize, cx: &mut Context<Self>) -> Result<(), String> {
        let Find::Results(outcome) = &self.find else {
            return Err("no search results to use".into());
        };
        let Some(scored) = outcome.top().get(index).copied() else {
            return Err(format!("no result {index}"));
        };
        self.stop_playing(cx);
        self.write_enhance(scored.candidate.enhance, cx);
        self.set_mode(Mode::Custom, cx);
        let gain = (f64::from(scored.candidate.gain) * 20.0).round() / 20.0;
        self.write(crate::mic::GAIN_SETTING, json!(gain), cx);
        self.mic.read(cx).apply_gain();
        self.page = Some(Page::Tune);
        hook::record_event("enhance", &format!("use {index}"));
        cx.notify();
        Ok(())
    }

    /// `hookctl state` section `enhance`.
    pub fn hook_state_json(&self, cx: &App) -> Value {
        let gain = self.gain(cx);
        let setting = |enhance: Enhance| {
            json!({
                "lowCutHz": enhance.low_cut_hz,
                "bassDb": enhance.bass_db,
                "trebleDb": enhance.treble_db,
                "noiseReduction": enhance.noise_reduction,
            })
        };
        let find = match &self.find {
            Find::Reading => json!({"state": "reading"}),
            Find::Testing { done, about } => {
                json!({"state": "testing", "done": done, "about": about})
            }
            Find::Failed(message) => json!({"state": "failed", "message": message}),
            Find::Results(outcome) => json!({
                "state": "results",
                "all_same": outcome.all_same(),
                "runs": outcome.ranked.len(),
                "results": outcome.top().iter().map(|scored| {
                    let mut entry = setting(scored.candidate.enhance);
                    entry["inputGain"] = json!(scored.candidate.gain);
                    entry["errors"] = json!(scored.errors);
                    entry["description"] = json!(scored.candidate.describe(gain));
                    entry
                }).collect::<Vec<_>>(),
            }),
        };
        json!({
            "mode": self.mode().key(),
            "custom": setting(self.custom()),
            "effective": setting(effective(&self.storage)),
            "sheet": self.page.map(Page::key),
            "take": match self.take {
                Take::Idle => json!({"state": "idle"}),
                Take::Recording { find, seconds } => json!({
                    "state": "recording",
                    "for": if find { "find" } else { "sample" },
                    "seconds": seconds,
                }),
            },
            "sample": self.sample.as_ref().map(|sample| json!({
                "path": sample_path(&self.storage).display().to_string(),
                "duration_ms": sample.len() as u64 * 1000 / u64::from(RATE),
            })),
            "playing": self.playing().map(|playing| match playing {
                Playing::Sample => json!("sample"),
                Playing::Result(index) => json!(index),
            }),
            "find": find,
            "reading": self.reading(),
            "notice": self.notice,
            "listening": self.mic.read(cx).state() == CaptureState::Listening,
        })
    }
}

/// `cache/enhance/sample.wav`: the last take, as the microphone gave it.
fn sample_path(storage: &Storage) -> PathBuf {
    storage
        .data
        .root()
        .join("cache")
        .join("enhance")
        .join("sample.wav")
}

/// The audio of a file as 16 kHz mono.
fn read_samples(path: &Path) -> Result<Vec<f32>, String> {
    let mut decoder = FileDecoder::open(path).map_err(|error| error.to_string())?;
    let mut samples = Vec::new();
    while decoder
        .read(&mut samples)
        .map_err(|error| error.to_string())?
    {}
    Ok(samples)
}
