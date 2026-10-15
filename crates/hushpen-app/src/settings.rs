//! The Settings view: sections, the controls that write the settings store,
//! launch at login, and the data folder move.

pub mod endpoint;
mod panel;

use crate::hook;
use crate::mic::Mic;
use crate::models::{ModelState, Models};
use crate::storage::Storage;
use crate::updater::Updater;
use gpui_kit::{App, Context, Entity, FocusHandle};
use hushpen_core::catalog::{Catalog, ModelEntry};
use hushpen_core::endpoint::Preset;
use hushpen_core::error::DATA_FOLDER_MOVE_FAILED;
use hushpen_core::shortcut::Platform;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

pub(crate) use panel::render;

/// Focus slots in [`Settings::control_focus`], shared with the panel.
pub(crate) mod slots {
    pub const LOGIN: usize = 0;
    pub const HIDDEN: usize = 1;
    pub const MIC: usize = 2;
    pub const SOUNDS: usize = 3;
    pub const RULES: usize = 4;
    pub const FLOWBAR: usize = 5;
    pub const FLOWBAR_IDLE: usize = 6;
    pub const POSITION: usize = 7;
    pub const CHANGE_FOLDER: usize = 8;
    pub const RETENTION: usize = 9;
    pub const RETENTION_OPTION: usize = 10; // ..=12
    pub const POSITION_OPTION: usize = 13; // ..=14
    pub const AI_CLEANUP: usize = 15;
    pub const AI_MODEL: usize = 16;
    pub const AI_MODEL_OPTION: usize = 17; // ..=18
    pub const AI_DOWNLOAD: usize = 19;
    pub const AI_PROVIDER: usize = 20;
    pub const AI_PROVIDER_OPTION: usize = 21; // ..=22
    pub const ENDPOINT_PRESET: usize = 23;
    pub const ENDPOINT_PRESET_OPTION: usize = 24; // ..=26
    pub const ENDPOINT_KEY_CLEAR: usize = 27;
    pub const ENDPOINT_TEST: usize = 28;
    pub const UPDATES_CHECK: usize = 29;
    pub const UPDATES_AUTO: usize = 30;
    pub const UPDATES_NOW: usize = 31;
    pub const UPDATES_NOTES: usize = 32;
    pub const COUNT: usize = 33;
}

/// The sections of the Settings view, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Shortcuts,
    Audio,
    Cleanup,
    FlowBar,
    Storage,
    Updates,
}

impl Section {
    pub const ALL: [Section; 7] = [
        Section::General,
        Section::Shortcuts,
        Section::Audio,
        Section::Cleanup,
        Section::FlowBar,
        Section::Storage,
        Section::Updates,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Shortcuts => "Shortcuts",
            Section::Audio => "Audio",
            Section::Cleanup => "Cleanup",
            Section::FlowBar => "Flow bar",
            Section::Storage => "Storage",
            Section::Updates => "Updates",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Section::General => "general",
            Section::Shortcuts => "shortcuts",
            Section::Audio => "audio",
            Section::Cleanup => "cleanup",
            Section::FlowBar => "flowbar",
            Section::Storage => "storage",
            Section::Updates => "updates",
        }
    }
}

/// Launch at login, behind a trait so tests never touch the login items of
/// the machine. The platform impl is [`hushpen_platform::login_item`].
///
/// `is_enabled` can take over a second on macOS (an XPC call to the background task
/// manager), so only a reading thread calls it. The views read [`Settings::launch_at_login`],
/// which answers from a cache.
pub trait LoginToggle: Send + Sync {
    fn is_enabled(&self) -> bool;
    fn set(&self, enabled: bool) -> Result<(), String>;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl LoginToggle for hushpen_platform::login_item::LoginItem {
    fn is_enabled(&self) -> bool {
        self.is_enabled()
    }

    fn set(&self, enabled: bool) -> Result<(), String> {
        self.set(enabled).map_err(|error| error.to_string())
    }
}

/// Where a data folder move stands. The failure carries the code the UI and
/// the hook show, never the payload of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveState {
    Idle,
    Choosing,
    Failed(&'static str),
    /// The move finished; the app restarts into the new folder.
    Restarting,
}

impl MoveState {
    pub fn key(&self) -> &'static str {
        match self {
            MoveState::Idle => "idle",
            MoveState::Choosing => "choosing",
            MoveState::Failed(_) => "failed",
            MoveState::Restarting => "restarting",
        }
    }
}

/// How often the view looks for the answer of a running status read.
const LOGIN_POLL: std::time::Duration = std::time::Duration::from_millis(20);

/// What the system last said about launch at login. A read starts on a thread and its answer
/// comes back as a `LoginRead`.
#[derive(Default)]
struct LoginCache {
    /// `None` until the first read lands.
    value: Option<bool>,
    /// Counts the toggles, so a read that started before one never overwrites its result.
    generation: u64,
    reading: bool,
    /// A refresh was asked for while a read ran; one more read follows it.
    again: bool,
}

/// The answer of one reading thread, and the generation it started in.
struct LoginRead {
    generation: u64,
    enabled: bool,
}

pub struct Settings {
    pub(crate) storage: Rc<Storage>,
    pub(crate) mic: Entity<Mic>,
    login: Option<Arc<dyn LoginToggle>>,
    login_cache: LoginCache,
    default_data_dir: PathBuf,
    pub(crate) section: Section,
    /// The open picker, if any: `audio.mic`, `storage.retention`, `flowbar.position`.
    pub(crate) picker: Option<&'static str>,
    pub(crate) move_state: MoveState,
    pub(crate) notice: Option<String>,
    /// What a finished move does. Tests record it instead of restarting.
    pub(crate) restart: Restart,
    pub(crate) section_focus: Vec<FocusHandle>,
    pub(crate) control_focus: Vec<FocusHandle>,
    ai: Option<AiParts>,
    ai_loading: bool,
    pub(crate) endpoint: Option<Entity<endpoint::EndpointForm>>,
    pub(crate) updater: Option<Entity<Updater>>,
    in_flight: Option<InFlight>,
}

/// How the app hands over to a fresh copy after the move.
pub type Restart = Rc<dyn Fn(&mut App)>;

/// Whether a transcription is running: a dictation after the microphone stopped, or a file
/// import. A data folder move waits for it, so no history row lands after the old
/// `history.db` is gone.
pub type InFlight = Rc<dyn Fn(&App) -> bool>;

/// What Settings > Cleanup needs to offer, download, and report the cleanup model.
pub struct AiParts {
    pub models: Entity<Models>,
    pub catalog: &'static Catalog,
    pub platform: Platform,
    pub ram_bytes: u64,
    /// Whether the model worker is loading the model right now.
    pub loading: Rc<dyn Fn() -> bool>,
}

/// What the cleanup model is doing, for the words under the AI model row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiStatus {
    NotDownloaded,
    Downloading(Option<u64>),
    Verifying,
    Failed,
    Preparing,
    Ready,
}

impl AiStatus {
    pub(crate) fn key(self) -> &'static str {
        match self {
            AiStatus::NotDownloaded => "not_downloaded",
            AiStatus::Downloading(_) => "downloading",
            AiStatus::Verifying => "verifying",
            AiStatus::Failed => "failed",
            AiStatus::Preparing => "preparing",
            AiStatus::Ready => "ready",
        }
    }
}

const AI_LOADING_POLL: std::time::Duration = std::time::Duration::from_millis(500);

pub struct Parts {
    pub storage: Rc<Storage>,
    pub mic: Entity<Mic>,
    pub login: Option<Arc<dyn LoginToggle>>,
    /// Where the pointer file lives. Defaults to the system's default data
    /// folder; tests pass a folder of their own.
    pub default_data_dir: Option<PathBuf>,
    /// Defaults to [`restart_into`]; tests inject a recorder.
    pub restart: Option<Restart>,
}

impl Settings {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let Parts {
            storage,
            mic,
            login,
            default_data_dir,
            restart,
        } = parts;
        cx.observe(&mic, |_, _, cx| cx.notify()).detach();
        let mut me = Self {
            storage,
            mic,
            login,
            login_cache: LoginCache::default(),
            default_data_dir: default_data_dir
                .unwrap_or_else(hushpen_store::data_dir::resolve_from_env),
            section: Section::General,
            picker: None,
            move_state: MoveState::Idle,
            notice: None,
            restart: restart.unwrap_or_else(|| Rc::new(restart_into)),
            section_focus: Section::ALL
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            control_focus: (0..slots::COUNT)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            ai: None,
            ai_loading: false,
            endpoint: None,
            updater: None,
            in_flight: None,
        };
        me.refresh_login(cx);
        me
    }

    /// Shows the update check in Settings > Updates and repaints when it changes.
    pub fn attach_updater(&mut self, updater: Entity<Updater>, cx: &mut Context<Self>) {
        cx.observe(&updater, |_, _, cx| cx.notify()).detach();
        self.updater = Some(updater);
    }

    pub fn toggle_update_check(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("updates.check");
        self.set("updates.check", json!(on), cx);
        self.updates_changed(cx);
    }

    pub fn toggle_auto_install(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("updates.autoInstall");
        self.set("updates.autoInstall", json!(on), cx);
        self.updates_changed(cx);
    }

    fn updates_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(updater) = self.updater.clone() {
            updater.update(cx, |updater, cx| updater.settings_changed(cx));
        }
    }

    /// Tells Settings how to see a running transcription.
    pub fn attach_in_flight(&mut self, in_flight: InFlight) {
        self.in_flight = Some(in_flight);
    }

    /// Connects the endpoint rows of Cleanup. Their fields need a window, so the form is built
    /// by the caller.
    pub fn attach_endpoint(
        &mut self,
        form: Entity<endpoint::EndpointForm>,
        cx: &mut Context<Self>,
    ) {
        cx.observe(&form, |_, _, cx| cx.notify()).detach();
        self.endpoint = Some(form);
    }

    /// Where AI cleanup runs: `local` or `endpoint`.
    pub(crate) fn ai_provider(&self) -> &'static str {
        match self.stored_str("cleanup.llm.provider").as_str() {
            "endpoint" => "endpoint",
            _ => "local",
        }
    }

    pub fn set_ai_provider(&mut self, provider: &str, cx: &mut Context<Self>) {
        if matches!(provider, "local" | "endpoint") {
            self.set("cleanup.llm.provider", json!(provider), cx);
            if provider == "local" && self.stored_bool("cleanup.llm.enabled") {
                self.download_ai_model(cx);
            }
        }
        self.picker = None;
        cx.notify();
    }

    pub fn set_endpoint_preset(&mut self, preset: Preset, cx: &mut Context<Self>) {
        if let Some(form) = self.endpoint.clone() {
            form.update(cx, |form, cx| form.set_preset(preset, cx));
        }
        self.picker = None;
        cx.notify();
    }

    /// Connects Cleanup to the model library and the cleanup worker. Without it the AI rows
    /// show nothing to choose.
    pub fn attach_ai(&mut self, ai: AiParts, cx: &mut Context<Self>) {
        cx.observe(&ai.models, |_, _, cx| cx.notify()).detach();
        self.ai = Some(ai);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(AI_LOADING_POLL).await;
                if this.update(cx, |me, cx| me.poll_ai_loading(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn poll_ai_loading(&mut self, cx: &mut Context<Self>) {
        let loading = self.ai.as_ref().is_some_and(|ai| (ai.loading)());
        if loading != self.ai_loading {
            self.ai_loading = loading;
            cx.notify();
        }
    }

    /// The cleanup models this computer is offered, best first.
    pub(crate) fn ai_offered(&self) -> Vec<ModelEntry> {
        self.ai.as_ref().map_or_else(Vec::new, |ai| {
            ai.catalog
                .offered_llm(ai.platform, ai.ram_bytes)
                .into_iter()
                .cloned()
                .collect()
        })
    }

    /// The model AI cleanup uses: the stored choice while this computer is offered it,
    /// otherwise the first one offered.
    pub(crate) fn ai_model(&self) -> Option<ModelEntry> {
        let offered = self.ai_offered();
        let chosen = self.stored_str("cleanup.llm.modelId");
        offered
            .iter()
            .find(|entry| entry.id == chosen)
            .or_else(|| offered.first())
            .cloned()
    }

    pub(crate) fn ai_status(&self, cx: &App) -> Option<AiStatus> {
        let model = self.ai_model()?;
        let ai = self.ai.as_ref()?;
        let state = ai.models.read(cx).llm_state(&model.id)?;
        Some(match state {
            ModelState::NotDownloaded => AiStatus::NotDownloaded,
            ModelState::Downloading { .. } => AiStatus::Downloading(state.percent()),
            ModelState::Verifying => AiStatus::Verifying,
            ModelState::Failed => AiStatus::Failed,
            ModelState::Ready if self.ai_loading && self.stored_bool("cleanup.llm.enabled") => {
                AiStatus::Preparing
            }
            ModelState::Ready => AiStatus::Ready,
        })
    }

    /// Turns AI cleanup on or off. Turning it on picks the model, saves it, and starts its
    /// download when the file is not there yet; dictations before it is ready use the rules.
    pub fn toggle_ai_cleanup(&mut self, cx: &mut Context<Self>) {
        if self.stored_bool("cleanup.llm.enabled") {
            self.set("cleanup.llm.enabled", json!(false), cx);
            return;
        }
        if self.ai_provider() == "endpoint" {
            self.set("cleanup.llm.enabled", json!(true), cx);
            return;
        }
        let Some(model) = self.ai_model() else {
            self.notice = Some("No AI model is available for this computer.".into());
            cx.notify();
            return;
        };
        self.set("cleanup.llm.modelId", json!(model.id), cx);
        self.set("cleanup.llm.enabled", json!(true), cx);
        self.download_ai_model(cx);
    }

    pub fn set_ai_model(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.ai_offered().iter().any(|entry| entry.id == id) {
            self.set("cleanup.llm.modelId", json!(id), cx);
            if self.stored_bool("cleanup.llm.enabled") {
                self.download_ai_model(cx);
            }
        }
        self.picker = None;
        cx.notify();
    }

    /// Starts the download of the chosen cleanup model unless it is there or on its way.
    pub fn download_ai_model(&mut self, cx: &mut Context<Self>) {
        let (Some(model), Some(ai)) = (self.ai_model(), self.ai.as_ref()) else {
            return;
        };
        let models = ai.models.clone();
        let started = models.update(cx, |models, cx| match models.llm_state(&model.id) {
            Some(ModelState::NotDownloaded | ModelState::Failed) => models.download(&model.id, cx),
            _ => Ok(()),
        });
        if let Err(reason) = started {
            self.notice = Some(format!("The AI model could not be downloaded: {reason}"));
            cx.notify();
        }
    }

    /// Esc in an open picker: close it and give the focus back to the
    /// trigger. Returns the handle to focus, or None when nothing was open.
    pub fn close_picker(&mut self, cx: &mut Context<Self>) -> Option<FocusHandle> {
        let slot = match self.picker.take()? {
            "audio.mic" => slots::MIC,
            "storage.retention" => slots::RETENTION,
            "flowbar.position" => slots::POSITION,
            "cleanup.aiModel" => slots::AI_MODEL,
            "cleanup.provider" => slots::AI_PROVIDER,
            "cleanup.endpointPreset" => slots::ENDPOINT_PRESET,
            _ => return None,
        };
        cx.notify();
        Some(self.control_focus[slot].clone())
    }

    /// The Settings view opened: re-read launch at login from the system, so
    /// a login item removed by hand shows as off.
    pub fn opened(&mut self, cx: &mut Context<Self>) {
        self.refresh_login(cx);
        cx.notify();
    }

    /// The user came back to Hushpen, often from System Settings > Login Items: read launch at
    /// login again.
    pub fn app_activated(&mut self, cx: &mut Context<Self>) {
        self.refresh_login(cx);
    }

    pub fn select_section(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            self.picker = None;
            hook::record_event("settings", &format!("section {}", section.key()));
            cx.notify();
        }
    }

    /// Launch at login as the system last said it, or the stored intent until the first answer
    /// and on a platform with no login items. It never asks the system.
    pub fn launch_at_login(&self) -> bool {
        self.login_cache
            .value
            .unwrap_or_else(|| self.stored_bool("startup.launchAtLogin"))
    }

    /// Starts a read of the system's answer on a thread. A read already running gets one more
    /// after it, so the newest request is never lost.
    fn refresh_login(&mut self, cx: &mut Context<Self>) {
        let Some(login) = &self.login else { return };
        if self.login_cache.reading {
            self.login_cache.again = true;
            return;
        }
        let login = Arc::clone(login);
        let (answer, answered) = std::sync::mpsc::channel();
        let generation = self.login_cache.generation;
        let spawned = std::thread::Builder::new()
            .name("hushpen-login-status".into())
            .spawn(move || {
                let enabled = login.is_enabled();
                let _ = answer.send(LoginRead {
                    generation,
                    enabled,
                });
            });
        if spawned.is_err() {
            return;
        }
        self.login_cache.reading = true;
        // The thread cannot wake this task, so the task looks for the answer on a short timer.
        // A task woken from another thread would also break the test scheduler.
        cx.spawn(async move |this, cx| {
            loop {
                match answered.try_recv() {
                    Ok(read) => {
                        let _ = this.update(cx, |me, cx| me.login_read(read, cx));
                        return;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor().timer(LOGIN_POLL).await;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = this.update(cx, |me, _| me.login_cache.reading = false);
                        return;
                    }
                }
            }
        })
        .detach();
    }

    fn login_read(&mut self, read: LoginRead, cx: &mut Context<Self>) {
        self.login_cache.reading = false;
        if read.generation == self.login_cache.generation {
            self.login_cache.value = Some(read.enabled);
            // A login item removed by hand in System Settings shows as off, and stays off.
            if read.enabled != self.stored_bool("startup.launchAtLogin") {
                let _ = self
                    .storage
                    .settings
                    .set("startup.launchAtLogin", json!(read.enabled));
            }
            cx.notify();
        }
        if std::mem::take(&mut self.login_cache.again) {
            self.refresh_login(cx);
        }
    }

    pub fn toggle_launch_at_login(&mut self, cx: &mut Context<Self>) {
        let on = !self.launch_at_login();
        if let Some(login) = &self.login {
            if let Err(error) = login.set(on) {
                self.notice = Some(format!("Launch at login could not be changed: {error}"));
                cx.notify();
                return;
            }
            // The row follows the click at once. The system's own answer replaces it.
            self.login_cache.generation += 1;
            self.login_cache.value = Some(on);
            self.refresh_login(cx);
        }
        self.set("startup.launchAtLogin", json!(on), cx);
        hook::record_event("settings", &format!("launch-at-login {on}"));
    }

    pub fn toggle_start_hidden(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("startup.startHidden");
        self.set("startup.startHidden", json!(on), cx);
    }

    pub fn toggle_sounds(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("audio.cueSounds");
        self.set("audio.cueSounds", json!(on), cx);
    }

    pub fn toggle_rules(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("cleanup.rules");
        self.set("cleanup.rules", json!(on), cx);
    }

    pub fn toggle_flow_bar(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("overlay.enabled");
        self.set("overlay.enabled", json!(on), cx);
    }

    pub fn toggle_flow_bar_idle(&mut self, cx: &mut Context<Self>) {
        let on = !self.stored_bool("overlay.idleVisible");
        self.set("overlay.idleVisible", json!(on), cx);
    }

    /// The Flow bar position goes through the store, which clears a dragged
    /// custom position in the same write.
    pub fn set_position(&mut self, position: &str, cx: &mut Context<Self>) {
        self.set("overlay.position", json!(position), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn set_retention(&mut self, retention: &str, cx: &mut Context<Self>) {
        self.set("history.audioRetention", json!(retention), cx);
        self.picker = None;
        cx.notify();
    }

    pub fn toggle_picker(&mut self, picker: &'static str, cx: &mut Context<Self>) {
        self.picker = match self.picker {
            Some(open) if open == picker => None,
            _ => Some(picker),
        };
        cx.notify();
    }

    pub fn stored_bool(&self, key: &str) -> bool {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    pub(crate) fn stored_str(&self, key: &str) -> String {
        self.storage
            .settings
            .get(key)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    fn set(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        match self.storage.settings.set(key, value) {
            Ok(()) => {
                self.notice = None;
                hook::record_event("settings", &format!("set {key}"));
            }
            Err(error) => {
                self.notice = Some(format!("The setting could not be saved: {error}"));
            }
        }
        cx.notify();
    }

    /// The data folder row: the pick starts here. A running dictation keeps
    /// the button off so the move never copies a half-written recording.
    pub fn change_data_folder(&mut self, cx: &mut Context<Self>) {
        if self.move_state != MoveState::Idle {
            return;
        }
        if self.mic.read(cx).state() == crate::mic::CaptureState::Listening {
            self.notice = Some("The microphone is recording. Stop the dictation first.".into());
            cx.notify();
            return;
        }
        if self.in_flight.as_ref().is_some_and(|busy| busy(cx)) {
            self.notice = Some(
                "A transcription is running. Wait for it to finish, then move the folder.".into(),
            );
            cx.notify();
            return;
        }
        self.move_state = MoveState::Choosing;
        self.notice = None;
        cx.notify();
        cx.spawn(async move |me, cx| {
            let picked = pick_folder(cx).await;
            let _ = me.update(cx, |me, cx| me.finish_move(picked, cx));
        })
        .detach();
    }

    fn finish_move(&mut self, picked: Option<PathBuf>, cx: &mut Context<Self>) {
        let Some(target) = picked else {
            self.move_state = MoveState::Idle;
            cx.notify();
            return;
        };
        let storage = Rc::clone(&self.storage);
        let current = storage.data.root().to_path_buf();
        let default = self.default_data_dir.clone();
        // The WAL must fold into the database file before the copy reads it.
        if let Err(error) = storage.database.checkpoint() {
            self.move_failed(&format!("the database could not settle: {error}"), cx);
            return;
        }
        match hushpen_store::data_move::move_data(&current, &target, &default) {
            Ok(report) => {
                hook::record_event(
                    "settings",
                    &format!(
                        "data-folder moved to {} ({} files, {} bytes)",
                        target.display(),
                        report.files,
                        report.bytes
                    ),
                );
                self.move_state = MoveState::Restarting;
                cx.notify();
                let restart = Rc::clone(&self.restart);
                restart(cx);
            }
            Err(error) => {
                log::warn!("the data folder move failed: {error}");
                hook::record_event(
                    "settings",
                    &format!("data-folder failed {DATA_FOLDER_MOVE_FAILED}"),
                );
                self.move_failed(&error.to_string(), cx);
            }
        }
    }

    fn move_failed(&mut self, reason: &str, cx: &mut Context<Self>) {
        self.move_state = MoveState::Failed(DATA_FOLDER_MOVE_FAILED);
        self.notice = Some(format!(
            "The data folder could not be moved ({DATA_FOLDER_MOVE_FAILED}). {reason}"
        ));
        cx.notify();
    }

    pub fn move_state(&self) -> &MoveState {
        &self.move_state
    }

    /// `hookctl state` section `settings_view`: the section on screen, the
    /// move, and launch at login as the system has it. The hook module wires
    /// it once the hook is attached.
    pub fn hook_state_json(&self, cx: &gpui_kit::App) -> Value {
        json!({
            "section": self.section.key(),
            "ai": {
                "enabled": self.stored_bool("cleanup.llm.enabled"),
                "provider": self.ai_provider(),
                "endpoint": self.endpoint.as_ref().map(|form| form.read(cx).hook_state_json(cx)),
                "model": self.ai_model().map(|model| model.id),
                "offered": self.ai_offered().into_iter().map(|model| model.id).collect::<Vec<_>>(),
                "status": self.ai_status(cx).map(AiStatus::key),
            },
            "launch_at_login": self.launch_at_login(),
            "data_folder": self.storage.data.root().display().to_string(),
            "move": match &self.move_state {
                MoveState::Idle => json!({"state": "idle"}),
                MoveState::Choosing => json!({"state": "choosing"}),
                MoveState::Failed(code) => json!({"state": "failed", "error": code}),
                MoveState::Restarting => json!({"state": "restarting"}),
            },
        })
    }
}

/// The folder for the move: the path the test hook queued, or the system's
/// folder picker.
async fn pick_folder(cx: &mut gpui_kit::AsyncApp) -> Option<PathBuf> {
    #[cfg(feature = "test-automation")]
    if let Some(paths) = hushpen_testhook::dialogs::take_answer() {
        return paths.into_iter().next();
    }
    let asked = cx.update(|cx| {
        cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        })
    });
    match asked.await {
        Ok(Ok(Some(mut paths))) if !paths.is_empty() => paths.pop(),
        _ => None,
    }
}

/// Hands the process to a fresh copy of itself, so the store, the database,
/// and the engine reopen on the new data folder. The helper waits for this
/// process to end before the new one binds the single-instance socket.
fn restart_into(cx: &mut App) {
    if let Ok(exe) = std::env::current_exe() {
        let pid = std::process::id();
        let _ = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; exec \"$0\"",
            ))
            .arg(exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    cx.quit();
}

#[cfg(test)]
mod endpoint_tests;
#[cfg(test)]
mod tests;
