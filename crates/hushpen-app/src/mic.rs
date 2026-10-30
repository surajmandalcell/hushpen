//! Microphone state: the device list, the saved choice, one capture session,
//! the level history, and the notice the user sees when something is wrong.
//!
//! Audio threads never touch this entity. They send [`MicEvent`]s into a
//! channel and one task on the main thread applies them.

mod backend;
pub mod panel;

pub use backend::{CpalBackend, MicBackend, MicSession};

use crate::hook;
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{Context, FocusHandle};
use hushpen_audio::{
    CaptureError, CaptureEvent, DEFAULT_ID, Enhance, EventSink, Finished, InputDevice,
    RecoveredSession, Selection, Sweep, new_session_path, recover_orphans, resolve_selection,
};
use hushpen_core::error::{CAPTURE_FAILED, CAPTURE_RECOVERED, MIC_PERMISSION, MIC_UNAVAILABLE};
use hushpen_store::data_dir::DataDir;
use hushpen_store::time::now_unix_ms;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const SETTING: &str = "audio.inputDeviceId";
pub const GAIN_SETTING: &str = "audio.inputGain";

/// Bars in the level meter. Each bar is one level value, 50 ms.
pub const METER_BARS: usize = 32;

/// Room for the Default row and this many microphones.
pub const MAX_DEVICE_ROWS: usize = 16;

/// How often the idle app looks for plugged or unplugged microphones.
const DEVICE_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Idle,
    Listening,
    /// The last start or session failed. A new start is allowed.
    Failed,
}

impl CaptureState {
    pub fn key(self) -> &'static str {
        match self {
            CaptureState::Idle => "idle",
            CaptureState::Listening => "listening",
            CaptureState::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub code: &'static str,
    pub message: String,
}

/// Who the running capture session serves. A dictation and the test panel
/// never share one microphone stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureUse {
    Test,
    Dictation,
    /// A take for the audio enhance sheet. It records the microphone as it is, with no enhance
    /// and no boost, so a preview can apply any setting to it later.
    Sample,
}

impl CaptureUse {
    pub fn key(self) -> &'static str {
        match self {
            CaptureUse::Test => "test",
            CaptureUse::Dictation => "dictation",
            CaptureUse::Sample => "sample",
        }
    }
}

pub enum MicEvent {
    Capture {
        generation: u64,
        event: CaptureEvent,
    },
    Devices(Result<Vec<InputDevice>, CaptureError>),
}

struct Session {
    inner: Box<dyn MicSession>,
    path: PathBuf,
    generation: u64,
    used_for: CaptureUse,
}

/// A session that no longer holds the microphone but still has to finish its WAV. Finishing
/// waits for the tail of the speech and closes the file, which can take a few hundred
/// milliseconds, so a worker thread does it while the next session already records.
pub struct Detached {
    inner: Box<dyn MicSession>,
    keep: bool,
}

impl Detached {
    /// Finishes the WAV, or deletes it when it is not kept.
    pub fn finish(self) -> Result<Finished, CaptureError> {
        let result = self.inner.stop();
        if !self.keep
            && let Ok(finished) = &result
        {
            let _ = std::fs::remove_file(&finished.path);
        }
        result
    }
}

pub struct Mic {
    storage: Rc<Storage>,
    backend: Arc<dyn MicBackend>,
    events: UnboundedSender<MicEvent>,
    devices: Vec<InputDevice>,
    /// The microphone list was read at least once.
    listed: bool,
    saved: String,
    selection: Selection,
    /// The current notice says the saved microphone is missing.
    gone_notice: bool,
    refreshing: bool,
    state: CaptureState,
    levels: VecDeque<f32>,
    notice: Option<Notice>,
    session: Option<Session>,
    generation: u64,
    last_session: Option<PathBuf>,
    recovered: Vec<RecoveredSession>,
    pub(crate) button_focus: FocusHandle,
    pub(crate) picker_focus: FocusHandle,
    pub(crate) row_focus: Vec<FocusHandle>,
    /// The microphone list under the Home picker is open.
    pub(crate) picker_open: bool,
}

impl Mic {
    pub fn new(storage: Rc<Storage>, backend: Arc<dyn MicBackend>, cx: &mut Context<Self>) -> Self {
        let (events, queue) = unbounded();
        cx.spawn(async move |this, cx| {
            let mut queue = queue;
            while let Some(event) = queue.next().await {
                if this.update(cx, |mic, cx| mic.handle(event, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(DEVICE_POLL).await;
                if this.update(cx, |mic, cx| mic.poll(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        let saved = storage
            .settings
            .get(SETTING)
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| DEFAULT_ID.to_string());
        Self {
            storage,
            backend,
            events,
            devices: Vec::new(),
            listed: false,
            saved,
            selection: Selection::Default,
            gone_notice: false,
            refreshing: false,
            state: CaptureState::Idle,
            levels: VecDeque::new(),
            notice: None,
            session: None,
            generation: 0,
            last_session: None,
            recovered: Vec::new(),
            button_focus: cx.focus_handle().tab_stop(true),
            picker_focus: cx.focus_handle().tab_stop(true),
            row_focus: (0..=MAX_DEVICE_ROWS)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            picker_open: false,
        }
    }

    /// Sessions the start-up sweep repaired. The history feature turns them into rows.
    pub fn set_recovered(&mut self, sweep: Sweep) {
        self.recovered = sweep.recovered;
    }

    pub fn state(&self) -> CaptureState {
        self.state
    }

    pub fn devices(&self) -> &[InputDevice] {
        &self.devices
    }

    /// Whether the list of microphones was read at least once. Before that, an empty list
    /// means "not looked yet", not "no microphone".
    pub fn listed(&self) -> bool {
        self.listed
    }

    pub fn saved(&self) -> &str {
        &self.saved
    }

    /// What the next capture will use.
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn last_session(&self) -> Option<&Path> {
        self.last_session.as_deref()
    }

    /// The newest `METER_BARS` level values, oldest first, padded with rest at the start.
    pub fn meter(&self) -> Vec<f32> {
        let mut bars = vec![0.0; METER_BARS - self.levels.len()];
        bars.extend(self.levels.iter().copied());
        bars
    }

    pub fn is_default_active(&self) -> bool {
        !matches!(self.selection, Selection::Device(_))
    }

    /// The name of the system default microphone, when the list has one.
    pub fn default_name(&self) -> Option<&str> {
        self.devices
            .iter()
            .find(|device| device.is_default)
            .map(|device| device.name.as_str())
    }

    /// The microphones a picker offers, Default first, as `(id, label, chosen)`. The chosen one
    /// is the saved choice, also while it is missing and the default stands in for it.
    pub fn choices(&self) -> Vec<(String, String, bool)> {
        let default = self
            .default_name()
            .map_or_else(|| "Default".to_owned(), |name| format!("Default ({name})"));
        std::iter::once((DEFAULT_ID.to_owned(), default))
            .chain(
                self.devices
                    .iter()
                    .take(MAX_DEVICE_ROWS)
                    .map(|device| (device.id.clone(), device.name.clone())),
            )
            .map(|(id, label)| {
                let chosen = id == self.saved;
                (id, label, chosen)
            })
            .collect()
    }

    /// The label of the saved choice for a picker button; a missing microphone shows its id.
    pub fn saved_label(&self) -> String {
        self.choices()
            .into_iter()
            .find_map(|(_, label, chosen)| chosen.then_some(label))
            .unwrap_or_else(|| self.saved.clone())
    }

    pub fn refresh(&mut self, _cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let backend = Arc::clone(&self.backend);
        let events = self.events.clone();
        let spawned = std::thread::Builder::new()
            .name("hushpen-mic-list".into())
            .spawn(move || {
                let _ = events.unbounded_send(MicEvent::Devices(backend.list()));
            });
        if spawned.is_err() {
            self.refreshing = false;
        }
    }

    /// Stops every refresh from starting its reading thread, which a test scheduler does not
    /// allow, until a `MicEvent::Devices` arrives.
    #[cfg(test)]
    pub(crate) fn hold_listing(&mut self) {
        self.refreshing = true;
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        if self.state != CaptureState::Listening {
            self.refresh(cx);
        }
    }

    pub fn handle(&mut self, event: MicEvent, cx: &mut Context<Self>) {
        match event {
            MicEvent::Devices(result) => {
                self.refreshing = false;
                self.apply_devices(result, cx);
            }
            MicEvent::Capture { generation, event } => {
                let live = self
                    .session
                    .as_ref()
                    .is_some_and(|session| session.generation == generation);
                if !live {
                    return;
                }
                match event {
                    CaptureEvent::Level(level) => {
                        if self.levels.len() == METER_BARS {
                            self.levels.pop_front();
                        }
                        self.levels.push_back(level);
                        cx.notify();
                    }
                    CaptureEvent::Started => hook::record_event("capture", "mic-open"),
                    CaptureEvent::StartFailed(error) => self.lost(&error, true, cx),
                    CaptureEvent::Error(error) => self.lost(&error, false, cx),
                }
            }
        }
    }

    fn apply_devices(
        &mut self,
        result: Result<Vec<InputDevice>, CaptureError>,
        cx: &mut Context<Self>,
    ) {
        let devices = match result {
            Ok(devices) => devices,
            Err(error) => {
                log::warn!("{error}");
                return;
            }
        };
        let selection = resolve_selection(&self.saved, &devices);
        let first = !std::mem::replace(&mut self.listed, true);
        if !first && devices == self.devices && selection == self.selection {
            return;
        }
        self.devices = devices;
        self.selection = selection;
        self.warm();
        match &self.selection {
            Selection::Missing(id) => {
                if !self.gone_notice {
                    log::warn!(
                        "{MIC_UNAVAILABLE} saved microphone {id} is not there; using the default"
                    );
                    self.notice = Some(Notice {
                        code: MIC_UNAVAILABLE,
                        message: "The selected microphone is not available. Using the default microphone."
                            .into(),
                    });
                    self.gone_notice = true;
                }
            }
            _ if self.gone_notice => {
                self.notice = None;
                self.gone_notice = false;
            }
            _ => {}
        }
        cx.notify();
    }

    /// Saves the choice. `"default"` or the id of a listed microphone.
    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        if id != DEFAULT_ID && self.devices.iter().all(|device| device.id != id) {
            return Err(format!("no microphone with id '{id}'"));
        }
        self.storage
            .settings
            .set(SETTING, json!(id))
            .map_err(|error| error.to_string())?;
        self.saved = id.to_string();
        self.picker_open = false;
        self.selection = resolve_selection(&self.saved, &self.devices);
        self.warm();
        if self.gone_notice {
            self.notice = None;
            self.gone_notice = false;
        }
        hook::record_event("capture", &format!("select {id}"));
        cx.notify();
        Ok(())
    }

    fn device_id(&self) -> String {
        match &self.selection {
            Selection::Device(id) => id.clone(),
            Selection::Default | Selection::Missing(_) => DEFAULT_ID.to_string(),
        }
    }

    /// Opens the selected microphone ahead of the next start, so a key press never waits for it
    /// and audio right after the press is kept.
    fn warm(&self) {
        if self.session.is_none() {
            self.backend.warm(&self.device_id());
        }
    }

    /// The saved boost of the microphone: 1.0 is none, 2.0 is 200%.
    pub fn gain(&self) -> f32 {
        self.storage
            .settings
            .get(GAIN_SETTING)
            .and_then(|value| value.as_f64())
            .map_or(1.5, |gain| gain as f32)
    }

    /// Gives the running session the saved boost, so a change is heard at once. A sample take
    /// stays as the microphone gave it.
    pub fn apply_gain(&self) {
        if let Some(session) = self.shaped_session() {
            session.inner.set_gain(self.gain());
        }
    }

    /// The saved audio enhance, as Off, Auto, or the Custom values make it.
    pub fn enhance(&self) -> Enhance {
        crate::enhance::effective(&self.storage)
    }

    /// Gives the running session the saved enhance, so a change is heard at once.
    pub fn apply_enhance(&self) {
        if let Some(session) = self.shaped_session() {
            session.inner.set_enhance(self.enhance());
        }
    }

    fn shaped_session(&self) -> Option<&Session> {
        self.session
            .as_ref()
            .filter(|session| session.used_for != CaptureUse::Sample)
    }

    /// Who the running session serves, if one runs.
    pub fn session_use(&self) -> Option<CaptureUse> {
        self.session.as_ref().map(|session| session.used_for)
    }

    /// Starts one capture session into `cache/sessions`. A session serves one
    /// use: a second caller of the same use shares it, a caller of another
    /// use gets `MIC_BUSY` instead of taking the stream over.
    pub fn start(
        &mut self,
        used_for: CaptureUse,
        cx: &mut Context<Self>,
    ) -> Result<PathBuf, CaptureError> {
        if let Some(session) = &self.session {
            if session.used_for == used_for {
                return Ok(session.path.clone());
            }
            let error = CaptureError {
                code: hushpen_core::error::MIC_BUSY,
                detail: format!(
                    "the microphone is in use by {}",
                    match session.used_for {
                        CaptureUse::Dictation => "dictation",
                        CaptureUse::Test => "the microphone test",
                        CaptureUse::Sample => "the audio enhance sample",
                    }
                ),
            };
            hook::record_event("capture", &format!("busy {}", used_for.key()));
            return Err(error);
        }
        let device = self.device_id();
        self.generation += 1;
        let generation = self.generation;
        let events = self.events.clone();
        let sink: EventSink = Arc::new(move |event| {
            let _ = events.unbounded_send(MicEvent::Capture { generation, event });
        });
        let path = new_session_path(&self.storage.data.sessions_dir(), now_unix_ms());
        match self.backend.start(&device, path.clone(), sink) {
            Ok(inner) => {
                if used_for == CaptureUse::Sample {
                    inner.set_gain(1.0);
                    inner.set_enhance(Enhance::OFF);
                } else {
                    inner.set_gain(self.gain());
                    inner.set_enhance(self.enhance());
                }
                self.session = Some(Session {
                    inner,
                    path: path.clone(),
                    generation,
                    used_for,
                });
                self.state = CaptureState::Listening;
                self.levels.clear();
                if !self.gone_notice {
                    self.notice = None;
                }
                hook::record_event("capture", "listening");
                cx.notify();
                Ok(path)
            }
            Err(error) => {
                self.state = CaptureState::Failed;
                self.levels.clear();
                self.notice = Some(notice_for(&error, true));
                self.gone_notice = false;
                log::warn!("{error}");
                hook::record_event("capture", &format!("failed {}", error.code));
                cx.notify();
                Err(error)
            }
        }
    }

    /// Ends the session and fixes the WAV header. With `keep` false the file
    /// is deleted. Returns `None` when no session was open.
    pub fn stop(
        &mut self,
        keep: bool,
        cx: &mut Context<Self>,
    ) -> Result<Option<Finished>, CaptureError> {
        let Some(session) = self.session.take() else {
            return Ok(None);
        };
        self.levels.clear();
        let result = session.inner.stop();
        // A stream that ended its session on dead input is gone; open the next one now, so a
        // recording that follows starts with a ready stream.
        self.warm();
        match &result {
            Ok(finished) => {
                self.state = CaptureState::Idle;
                if keep {
                    self.last_session = Some(finished.path.clone());
                } else {
                    let _ = std::fs::remove_file(&finished.path);
                }
                hook::record_event("capture", "idle");
            }
            Err(error) => {
                self.state = CaptureState::Failed;
                self.notice = Some(notice_for(error, false));
                log::warn!("{error}");
                hook::record_event("capture", &format!("failed {}", error.code));
            }
        }
        cx.notify();
        result.map(Some)
    }

    /// Ends the session at once and gives it to the caller to finish, so the next start never
    /// waits for the old file. Returns `None` when no session was open.
    pub fn detach(&mut self, keep: bool, cx: &mut Context<Self>) -> Option<Detached> {
        let session = self.session.take()?;
        self.levels.clear();
        self.state = CaptureState::Idle;
        if keep {
            self.last_session = Some(session.path.clone());
        }
        self.warm();
        hook::record_event("capture", "idle");
        cx.notify();
        Some(Detached {
            inner: session.inner,
            keep,
        })
    }

    /// The microphone failed under a running session, or did not open (`starting`). The audio so
    /// far stays on disk, except for a start that never got any.
    fn lost(&mut self, error: &CaptureError, starting: bool, cx: &mut Context<Self>) {
        let Some(session) = self.session.take() else {
            return;
        };
        self.levels.clear();
        match session.inner.stop() {
            Ok(finished) if starting => {
                let _ = std::fs::remove_file(&finished.path);
            }
            Ok(finished) => self.last_session = Some(finished.path),
            Err(close) => log::warn!("{close}"),
        }
        self.state = CaptureState::Failed;
        self.notice = Some(notice_for(error, starting));
        self.gone_notice = false;
        log::warn!("{error}");
        hook::record_event("capture", &format!("failed {}", error.code));
        cx.notify();
    }

    /// Plays a WAV into the running session in place of the microphone.
    pub fn feed_wav(&mut self, path: &Path) -> Result<Value, String> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| "no capture session is running; start one first".to_string())?;
        let info = session
            .inner
            .feed_wav(path)
            .map_err(|error| error.to_string())?;
        hook::record_event("capture", "feed-wav");
        Ok(json!({
            "duration_ms": info.duration_ms,
            "sample_rate": info.sample_rate,
            "channels": info.channels,
        }))
    }

    pub fn state_json(&self) -> Value {
        let active = match &self.selection {
            Selection::Device(id) => id.as_str(),
            Selection::Default | Selection::Missing(_) => DEFAULT_ID,
        };
        json!({
            "state": self.state.key(),
            "level": self.levels.back().copied().unwrap_or(0.0),
            "levels": self.meter(),
            "saved_device": self.saved,
            "active_device": active,
            "devices": self.devices.iter().map(|device| json!({
                "id": device.id,
                "name": device.name,
                "default": device.is_default,
            })).collect::<Vec<_>>(),
            "notice": self.notice.as_ref().map(|notice| json!({
                "code": notice.code,
                "message": notice.message,
            })),
            "session": self.session.as_ref().map(|session| session.path.to_string_lossy().into_owned()),
            "session_use": self.session.as_ref().map(|session| session.used_for.key()),
            "recovered": self.recovered.iter().map(|session| json!({
                "id": session.id,
                "path": session.path.to_string_lossy(),
                "duration_ms": session.duration_ms,
            })).collect::<Vec<_>>(),
            "last_session": self.last_session.as_ref().map(|path| path.to_string_lossy().into_owned()),
        })
    }
}

/// Repairs or deletes what a crashed or killed recording left in `cache/sessions`.
pub fn sweep_orphans(data: &DataDir) -> Sweep {
    let sweep = match recover_orphans(&data.sessions_dir(), now_unix_ms()) {
        Ok(sweep) => sweep,
        Err(error) => {
            log::warn!("{CAPTURE_FAILED} could not sweep the session folder: {error}");
            return Sweep::default();
        }
    };
    for session in &sweep.recovered {
        log::info!(
            "{CAPTURE_RECOVERED} id={} duration_ms={}",
            session.id,
            session.duration_ms
        );
    }
    for path in &sweep.removed {
        log::info!(
            "CAPTURE_ORPHAN_REMOVED file={}",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default()
        );
    }
    sweep
}

/// The message for a session that could not finish its WAV.
pub fn stop_message(error: &CaptureError) -> String {
    notice_for(error, false).message
}

/// The message for a failed start (`starting`) or a session that broke.
fn notice_for(error: &CaptureError, starting: bool) -> Notice {
    let message = match (error.code, starting) {
        (MIC_UNAVAILABLE, true) => "No microphone is available. Connect one and try again.",
        (MIC_UNAVAILABLE, false) => {
            "The microphone stopped working. The recording so far was saved."
        }
        (MIC_PERMISSION, _) => "Hushpen needs permission to use the microphone.",
        (CAPTURE_FAILED, true) => "The microphone could not start. Try again.",
        _ => "Recording stopped because of an error. The recording so far was saved.",
    };
    Notice {
        code: error.code,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
