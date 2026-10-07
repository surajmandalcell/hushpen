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
    CaptureError, CaptureEvent, DEFAULT_ID, EventSink, Finished, InputDevice, RecoveredSession,
    Selection, Sweep, new_session_path, recover_orphans, resolve_selection,
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
}

pub struct Mic {
    storage: Rc<Storage>,
    backend: Arc<dyn MicBackend>,
    events: UnboundedSender<MicEvent>,
    devices: Vec<InputDevice>,
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
    pub(crate) row_focus: Vec<FocusHandle>,
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
            row_focus: (0..=MAX_DEVICE_ROWS)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
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
                    CaptureEvent::Error(error) => self.lost(&error, cx),
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
        if devices == self.devices && selection == self.selection {
            return;
        }
        self.devices = devices;
        self.selection = selection;
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
        self.selection = resolve_selection(&self.saved, &self.devices);
        if self.gone_notice {
            self.notice = None;
            self.gone_notice = false;
        }
        hook::record_event("capture", &format!("select {id}"));
        cx.notify();
        Ok(())
    }

    /// Starts one capture session into `cache/sessions`.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Result<PathBuf, CaptureError> {
        if let Some(session) = &self.session {
            return Ok(session.path.clone());
        }
        let device = match &self.selection {
            Selection::Device(id) => id.clone(),
            Selection::Default | Selection::Missing(_) => DEFAULT_ID.to_string(),
        };
        self.generation += 1;
        let generation = self.generation;
        let events = self.events.clone();
        let sink: EventSink = Arc::new(move |event| {
            let _ = events.unbounded_send(MicEvent::Capture { generation, event });
        });
        let path = new_session_path(&self.storage.data.sessions_dir(), now_unix_ms());
        match self.backend.start(&device, path.clone(), sink) {
            Ok(inner) => {
                self.session = Some(Session {
                    inner,
                    path: path.clone(),
                    generation,
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

    /// The microphone failed under a running session. The audio so far stays on disk.
    fn lost(&mut self, error: &CaptureError, cx: &mut Context<Self>) {
        let Some(session) = self.session.take() else {
            return;
        };
        self.levels.clear();
        match session.inner.stop() {
            Ok(finished) => self.last_session = Some(finished.path),
            Err(close) => log::warn!("{close}"),
        }
        self.state = CaptureState::Failed;
        self.notice = Some(notice_for(error, false));
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
