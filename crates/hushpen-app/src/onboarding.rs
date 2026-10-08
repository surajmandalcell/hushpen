//! First-run onboarding: five steps in the main window, and the repair of a lost grant.
//!
//! The pure decisions (the steps, where the window opens, what lets a step pass, which rows the
//! permissions page shows) live in `hushpen_core::onboarding`. This entity keeps the state of
//! the steps, reads the system for the facts, and tells the controller which key gate to hold.
//! The step is stored in `onboarding.step`, so a restart resumes at the same step.

pub mod panel;

use crate::controller::{CancelToken, Controller, Phase};
use crate::hook;
use crate::mic::{CaptureState, Mic};
use crate::models::{ModelState, Models};
use crate::shortcuts::Shortcuts;
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{App, Context, Entity, FocusHandle};
use hushpen_core::dictation::AppEvent;
use hushpen_core::error::ENGINE_NO_SPEECH;
use hushpen_core::onboarding::{
    self as rules, Facts, Heard, KeyGate, MicVerdict, Row, Session, Start, Step,
};
use hushpen_core::permission::Permission;
use hushpen_core::shortcut::{Platform, Slot};
use hushpen_engine::{JobOutcome, TranscribeSpec};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const COMPLETED: &str = "onboarding.completed";
const STEP: &str = "onboarding.step";
const UPDATES: &str = "updates.check";

/// How often the permissions are read again while onboarding is on screen.
const REFRESH: Duration = Duration::from_secs(1);

/// What a click on a permission button does. The real one asks macOS and opens System
/// Settings; tests pass a fake.
pub trait Guide {
    /// Asks the system to list Hushpen for the permission, which shows its prompt once.
    fn request(&self, permission: Permission);
    fn open(&self, url: &str) -> Result<(), String>;
}

pub struct SystemGuide;

impl Guide for SystemGuide {
    fn request(&self, permission: Permission) {
        if permission == Permission::Microphone {
            // Opening the stream is what makes macOS ask for the microphone.
            hushpen_audio::warm_microphone(hushpen_audio::DEFAULT_ID);
        } else {
            hushpen_platform::permissions::request(permission);
        }
    }

    fn open(&self, url: &str) -> Result<(), String> {
        hushpen_platform::permissions::open_url(url).map_err(|error| error.to_string())
    }
}

/// What the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The normal window.
    Hidden,
    /// First run: the five steps in order.
    Setup,
    /// A grant is gone: the permissions step alone.
    Repair,
}

impl Mode {
    fn key(self) -> &'static str {
        match self {
            Mode::Hidden => "hidden",
            Mode::Setup => "setup",
            Mode::Repair => "repair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicPhase {
    Idle,
    Listening,
    /// The recording is being turned into words.
    Checking,
}

impl MicPhase {
    fn key(self) -> &'static str {
        match self {
            MicPhase::Idle => "idle",
            MicPhase::Listening => "listening",
            MicPhase::Checking => "checking",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MicTest {
    pub phase: MicPhase,
    /// The loudest level of the newest test, 0 to 1.
    pub peak: f32,
    pub verdict: Option<MicVerdict>,
    pub passed: bool,
}

enum Message {
    Heard(Heard),
}

/// What onboarding reads and changes.
pub struct Parts {
    pub storage: Rc<Storage>,
    pub mic: Entity<Mic>,
    pub models: Entity<Models>,
    pub controller: Entity<Controller>,
    pub guide: Rc<dyn Guide>,
    pub platform: Platform,
    pub session: Session,
}

pub struct Onboarding {
    storage: Rc<Storage>,
    pub(crate) mic: Entity<Mic>,
    models: Entity<Models>,
    controller: Entity<Controller>,
    shortcuts: Option<Entity<Shortcuts>>,
    guide: Rc<dyn Guide>,
    platform: Platform,
    session: Session,
    mode: Mode,
    step: Step,
    /// The microphone list is not read yet, and a finished onboarding checks it once.
    waiting_for_mic: bool,
    mic_test: MicTest,
    practice: Option<String>,
    updates_on: bool,
    notice: Option<String>,
    messages: UnboundedSender<Message>,
    pub(crate) permission_focus: Vec<FocusHandle>,
    pub(crate) continue_focus: FocusHandle,
    pub(crate) mic_focus: FocusHandle,
    pub(crate) download_focus: FocusHandle,
    pub(crate) practice_focus: FocusHandle,
    pub(crate) record_focus: FocusHandle,
    pub(crate) updates_focus: FocusHandle,
}

impl Onboarding {
    pub fn new(parts: Parts, cx: &mut Context<Self>) -> Self {
        let Parts {
            storage,
            mic,
            models,
            controller,
            guide,
            platform,
            session,
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
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(REFRESH).await;
                let ticked = this.update(cx, |me, cx| {
                    me.refresh(cx);
                    me.look_for_microphone(cx);
                });
                if ticked.is_err() {
                    break;
                }
            }
        })
        .detach();
        cx.observe(&mic, |me, _, cx| me.mic_changed(cx)).detach();
        cx.observe(&models, |_, _, cx| cx.notify()).detach();
        cx.observe(&controller, |me, _, cx| me.controller_changed(cx))
            .detach();
        let completed = storage
            .settings
            .get(COMPLETED)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let stored = storage
            .settings
            .get(STEP)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        let gone = !controller.read(cx).lost_permissions().is_empty();
        let (mode, step) = match rules::start(completed, &stored, gone) {
            Start::Main => (Mode::Hidden, Step::Permissions),
            Start::Setup(step) => (Mode::Setup, step),
            Start::Repair => (Mode::Repair, Step::Permissions),
        };
        let updates_on = storage
            .settings
            .get(UPDATES)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let me = Self {
            storage,
            mic,
            models,
            controller,
            shortcuts: None,
            guide,
            platform,
            session,
            mode,
            step,
            waiting_for_mic: completed && mode == Mode::Hidden,
            mic_test: MicTest {
                phase: MicPhase::Idle,
                peak: 0.0,
                verdict: None,
                passed: false,
            },
            practice: None,
            updates_on,
            notice: None,
            messages,
            permission_focus: Permission::ALL
                .iter()
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            continue_focus: cx.focus_handle().tab_stop(true),
            mic_focus: cx.focus_handle().tab_stop(true),
            download_focus: cx.focus_handle().tab_stop(true),
            practice_focus: cx.focus_handle().tab_stop(true),
            record_focus: cx.focus_handle().tab_stop(true),
            updates_focus: cx.focus_handle().tab_stop(true),
        };
        me.sync_gate(cx);
        hook::record_event(
            "onboarding",
            &format!("start {} {}", me.mode.key(), me.step.key()),
        );
        me
    }

    /// Lets the practice step name the hold key the user set.
    pub fn attach_shortcuts(&mut self, shortcuts: Entity<Shortcuts>, cx: &mut Context<Self>) {
        cx.observe(&shortcuts, |_, _, cx| cx.notify()).detach();
        self.shortcuts = Some(shortcuts);
        cx.notify();
    }

    pub fn active(&self) -> bool {
        self.mode != Mode::Hidden
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn step(&self) -> Step {
        self.step
    }

    pub fn mic_test(&self) -> &MicTest {
        &self.mic_test
    }

    pub fn practice_text(&self) -> Option<&str> {
        self.practice.as_deref()
    }

    pub fn updates_on(&self) -> bool {
        self.updates_on
    }

    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    /// The shortcut the practice step tells the user to hold, in this system's key names.
    pub fn hold_key(&self, cx: &App) -> String {
        self.shortcuts.as_ref().map_or_else(
            || "the hold key".to_owned(),
            |s| s.read(cx).display(Slot::Hold),
        )
    }

    pub fn keys_ready(&self, cx: &App) -> bool {
        self.controller.read(cx).keys_availability() == rules::Availability::Ready
    }

    pub fn facts(&self, cx: &App) -> Facts {
        let controller = self.controller.read(cx);
        let mic = self.mic.read(cx);
        Facts {
            platform: self.platform,
            session: self.session,
            keys: controller.keys_availability(),
            paste: controller.paste_availability(),
            mic_listed: mic.listed(),
            mic_present: !mic.devices().is_empty(),
            mac: controller.access(),
        }
    }

    pub fn rows(&self, cx: &App) -> Vec<Row> {
        rules::permission_rows(&self.facts(cx))
    }

    /// The id of the model the step downloads: the chosen one, else the catalog default.
    pub fn model_id(&self, cx: &App) -> String {
        self.models.read(cx).effective()
    }

    pub fn model_state(&self, cx: &App) -> Option<ModelState> {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| row.state)
    }

    /// The name and size of the model the step downloads.
    pub fn model_info(&self, cx: &App) -> Option<(String, u64)> {
        let models = self.models.read(cx);
        let id = models.effective();
        models
            .rows()
            .iter()
            .find(|row| row.entry.id == id)
            .map(|row| (row.entry.name.clone(), row.entry.bytes))
    }

    /// The failure the Models service last reported, such as a bad checksum.
    pub fn model_notice(&self, cx: &App) -> Option<String> {
        self.models
            .read(cx)
            .notice()
            .map(|notice| notice.message.clone())
    }

    pub fn model_ready(&self, cx: &App) -> bool {
        self.models.read(cx).usable().is_some()
    }

    pub fn can_continue(&self, cx: &App) -> bool {
        match self.step {
            Step::Permissions => rules::rows_ready(&self.rows(cx)),
            Step::MicTest => self.mic_test.passed,
            Step::Model => self.model_ready(cx),
            Step::Practice => self.practice.is_some(),
            Step::Updates => true,
        }
    }

    /// The gate the controller holds for the state of the steps.
    pub fn gate(&self) -> KeyGate {
        match self.mode {
            Mode::Hidden | Mode::Repair => KeyGate::Open,
            Mode::Setup => rules::key_gate(false, self.step, self.practice.is_some()),
        }
    }

    fn sync_gate(&self, cx: &mut Context<Self>) {
        let gate = self.gate();
        self.controller
            .update(cx, |controller, cx| controller.set_gate(gate, cx));
    }

    /// Reads the permissions again, and the page repaints. Reading never prompts.
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if !self.active() {
            return;
        }
        self.controller
            .update(cx, |controller, cx| controller.refresh_permissions(cx));
        cx.notify();
    }

    /// The page names the microphone, so it looks for one every second while it is on screen.
    /// The Mic service alone looks every 2 s.
    fn look_for_microphone(&mut self, cx: &mut Context<Self>) {
        if self.active() && self.mic.read(cx).state() != CaptureState::Listening {
            self.mic.update(cx, |mic, cx| mic.refresh(cx));
        }
    }

    fn mic_changed(&mut self, cx: &mut Context<Self>) {
        let (listed, none, loudest, listening) = {
            let mic = self.mic.read(cx);
            (
                mic.listed(),
                mic.devices().is_empty(),
                mic.meter().into_iter().fold(0.0_f32, f32::max),
                mic.state() == CaptureState::Listening,
            )
        };
        if self.waiting_for_mic && listed {
            self.waiting_for_mic = false;
            if none {
                self.reopen_for_repair(cx);
            }
        }
        if self.mic_test.phase == MicPhase::Listening {
            self.mic_test.peak = self.mic_test.peak.max(loudest);
            if !listening {
                // The microphone went away under the test.
                self.mic_test.phase = MicPhase::Idle;
                self.mic_test.verdict = None;
                self.notice = Some("The microphone stopped. Check it and test again.".into());
            }
        }
        cx.notify();
    }

    fn reopen_for_repair(&mut self, cx: &mut Context<Self>) {
        if self.mode != Mode::Hidden {
            return;
        }
        self.mode = Mode::Repair;
        self.step = Step::Permissions;
        hook::record_event("onboarding", "repair no-microphone");
        self.sync_gate(cx);
    }

    fn controller_changed(&mut self, cx: &mut Context<Self>) {
        let words = self
            .controller
            .update(cx, |controller, _| controller.take_practice());
        if let Some(words) = words {
            hook::record_event("onboarding", "practice-passed");
            self.practice = Some(words);
            self.sync_gate(cx);
        }
        cx.notify();
    }

    fn handle(&mut self, message: Message, cx: &mut Context<Self>) {
        match message {
            Message::Heard(heard) => {
                if self.mic_test.phase != MicPhase::Checking {
                    return;
                }
                self.finish_mic_test(heard, cx);
            }
        }
    }

    /// A click on a permission button: ask the system to list Hushpen if it never asked, then
    /// open the Privacy pane of that permission.
    pub fn open_permission(&mut self, permission: Permission, cx: &mut Context<Self>) {
        let access = self.controller.read(cx).access().get(permission);
        if access == hushpen_core::permission::Access::NotDetermined {
            self.guide.request(permission);
        }
        hook::record_event("onboarding", &format!("open {}", permission.key()));
        self.notice = self.guide.open(permission.settings_url()).err().map(|_| {
            "System Settings could not be opened. Open Privacy and Security yourself.".to_owned()
        });
        cx.notify();
    }

    /// The test button: start a test, or stop the running one.
    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        match self.mic_test.phase {
            MicPhase::Checking => Err("the last test is still being read".into()),
            MicPhase::Listening => self.stop_mic_test(cx),
            MicPhase::Idle => {
                self.notice = None;
                match self.mic.update(cx, |mic, cx| mic.start(cx)) {
                    Ok(_) => {
                        self.mic_test = MicTest {
                            phase: MicPhase::Listening,
                            peak: 0.0,
                            verdict: None,
                            passed: false,
                        };
                        hook::record_event("onboarding", "mic-test start");
                        cx.notify();
                        Ok(())
                    }
                    Err(error) => {
                        self.notice = self
                            .mic
                            .read(cx)
                            .notice()
                            .map(|notice| notice.message.clone());
                        cx.notify();
                        Err(error.to_string())
                    }
                }
            }
        }
    }

    fn stop_mic_test(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let stopped = self.mic.update(cx, |mic, cx| mic.stop(true, cx));
        let loudest = self
            .mic
            .read(cx)
            .meter()
            .into_iter()
            .fold(0.0_f32, f32::max);
        self.mic_test.peak = self.mic_test.peak.max(loudest);
        hook::record_event("onboarding", "mic-test stop");
        let finished = match stopped {
            Ok(Some(finished)) => finished,
            Ok(None) | Err(_) => {
                self.mic_test.phase = MicPhase::Idle;
                self.notice = Some("The recording could not be read. Test again.".into());
                cx.notify();
                return Err("the test recording ended early".into());
            }
        };
        if self.mic_test.peak < rules::HEARD_LEVEL || self.models.read(cx).usable().is_none() {
            let _ = std::fs::remove_file(&finished.path);
            let heard = Heard::NotTried;
            self.mic_test.phase = MicPhase::Checking;
            self.finish_mic_test(heard, cx);
            return Ok(());
        }
        self.mic_test.phase = MicPhase::Checking;
        self.transcribe(finished.path, cx);
        cx.notify();
        Ok(())
    }

    /// Turns the test recording into words on a worker thread, then deletes it.
    fn transcribe(&mut self, wav: PathBuf, cx: &mut Context<Self>) {
        let (run, spawn, language) = {
            let controller = self.controller.read(cx);
            (
                controller.runner(),
                controller.spawner(),
                controller.language(),
            )
        };
        let spec = TranscribeSpec {
            wav_path: wav.clone(),
            language: (language != hushpen_core::language::AUTO).then_some(language),
            prompt: None,
        };
        let messages = self.messages.clone();
        let job = {
            let wav = wav.clone();
            Box::new(move || {
                let heard = match run(spec, Arc::new(CancelToken::default())) {
                    JobOutcome::Done(done) => Heard::Text(done.text),
                    JobOutcome::Failed(failure) if failure.code == ENGINE_NO_SPEECH => {
                        Heard::Text(String::new())
                    }
                    JobOutcome::Failed(_) | JobOutcome::Cancelled => Heard::Failed,
                };
                let _ = std::fs::remove_file(&wav);
                let _ = messages.unbounded_send(Message::Heard(heard));
            })
        };
        if let Err(error) = spawn(job) {
            log::warn!("the microphone test could not start the engine job: {error}");
            let _ = std::fs::remove_file(&wav);
            self.finish_mic_test(Heard::Failed, cx);
        }
    }

    fn finish_mic_test(&mut self, heard: Heard, cx: &mut Context<Self>) {
        let verdict = rules::mic_verdict(self.mic_test.peak, heard);
        self.mic_test.passed = matches!(verdict, MicVerdict::Passed { .. });
        hook::record_event(
            "onboarding",
            &format!(
                "mic-test {}",
                if self.mic_test.passed {
                    "passed"
                } else {
                    "not-passed"
                }
            ),
        );
        self.mic_test.verdict = Some(verdict);
        self.mic_test.phase = MicPhase::Idle;
        cx.notify();
    }

    pub fn download_model(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let id = self.model_id(cx);
        self.models
            .update(cx, |models, cx| models.download(&id, cx))
    }

    pub fn cancel_model(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let id = self.model_id(cx);
        self.models.update(cx, |models, cx| models.cancel(&id, cx))
    }

    /// The Record button of the practice step: the same pipeline as the hold key.
    pub fn toggle_practice_record(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if self.controller.read(cx).phase() == Phase::Transcribing {
            return Err("the last words are still being read".into());
        }
        self.controller.update(cx, |controller, cx| {
            controller.dispatch(AppEvent::HomeToggle, cx)
        })
    }

    pub fn practice_listening(&self, cx: &App) -> bool {
        self.controller.read(cx).phase() == Phase::Listening
    }

    pub fn practice_busy(&self, cx: &App) -> bool {
        self.controller.read(cx).phase() == Phase::Transcribing
    }

    pub fn can_practice_record(&self, cx: &App) -> bool {
        let controller = self.controller.read(cx);
        controller.phase() != Phase::Transcribing && controller.can_record(cx)
    }

    pub fn set_updates(&mut self, on: bool, cx: &mut Context<Self>) {
        self.updates_on = on;
        cx.notify();
    }

    /// The Continue button. Each step has its own condition, and nothing skips it.
    pub fn advance(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.active() {
            return Err("onboarding is not open".into());
        }
        if !self.can_continue(cx) {
            return Err(format!("the {} step is not done yet", self.step.key()));
        }
        if self.mode == Mode::Repair {
            self.mode = Mode::Hidden;
            hook::record_event("onboarding", "repair-done");
            self.sync_gate(cx);
            cx.notify();
            return Ok(());
        }
        if self.step == Step::Model {
            self.choose_default_model(cx);
        }
        match self.step.next() {
            Some(next) => self.go(next, cx),
            None => self.finish(cx),
        }
        Ok(())
    }

    /// The downloaded model becomes the chosen one, so the Models view shows it as active.
    fn choose_default_model(&mut self, cx: &mut Context<Self>) {
        let id = self.model_id(cx);
        self.models.update(cx, |models, cx| {
            if models.active().is_empty() {
                let _ = models.select(&id, cx);
            }
        });
    }

    fn go(&mut self, step: Step, cx: &mut Context<Self>) {
        self.step = step;
        self.notice = None;
        if let Err(error) = self.storage.settings.set_internal(STEP, json!(step.key())) {
            log::warn!("the onboarding step could not be saved: {error}");
        }
        hook::record_event("onboarding", &format!("step {}", step.key()));
        self.sync_gate(cx);
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let settings = &self.storage.settings;
        if let Err(error) = settings.set(UPDATES, json!(self.updates_on)) {
            log::warn!("the update choice could not be saved: {error}");
        }
        for (key, value) in [(COMPLETED, json!(true)), (STEP, json!(""))] {
            if let Err(error) = settings.set_internal(key, value) {
                log::warn!("onboarding could not be marked finished: {error}");
            }
        }
        self.mode = Mode::Hidden;
        hook::record_event("onboarding", "completed");
        self.sync_gate(cx);
        cx.notify();
    }

    /// `hookctl state` section `onboarding`.
    pub fn state_json(&self, cx: &App) -> Value {
        let rows = self.rows(cx);
        let mic = self.mic.read(cx);
        let verdict = self.mic_test.verdict.as_ref().map(|verdict| match verdict {
            MicVerdict::NoSound => json!({"result": "no-sound", "hint": rules::HINT_NO_SOUND}),
            MicVerdict::NoSpeech => json!({"result": "no-speech", "hint": rules::HINT_NO_SPEECH}),
            MicVerdict::TranscriptFailed => {
                json!({"result": "transcript-failed", "hint": rules::HINT_TRANSCRIPT_FAILED})
            }
            MicVerdict::Passed { transcript } => {
                json!({"result": "passed", "transcript": transcript})
            }
        });
        let model_state = self.model_state(cx);
        json!({
            "active": self.active(),
            "mode": self.mode.key(),
            "step": self.step.key(),
            "step_index": self.step.index(),
            "steps": Step::ALL.iter().map(|step| step.key()).collect::<Vec<_>>(),
            "can_continue": self.can_continue(cx),
            "gate": format!("{:?}", self.gate()).to_lowercase(),
            "completed": self.storage.settings.get(COMPLETED).and_then(|v| v.as_bool()).unwrap_or(false),
            "permissions": {
                "ready": rules::rows_ready(&rows),
                "rows": rows.iter().map(|row| json!({
                    "key": row.key.key(),
                    "state": row.state.key(),
                    "detail": row.detail,
                })).collect::<Vec<_>>(),
            },
            "mic": {
                "phase": self.mic_test.phase.key(),
                "level": mic.meter().last().copied().unwrap_or(0.0),
                "peak": self.mic_test.peak,
                "passed": self.mic_test.passed,
                "verdict": verdict,
            },
            "model": {
                "id": self.model_id(cx),
                "state": model_state.map(|state| state.key()),
                "progress": model_state.and_then(|state| state.percent()),
                "ready": self.model_ready(cx),
            },
            "practice": {
                "text": self.practice,
                "passed": self.practice.is_some(),
            },
            "updates": {"check": self.updates_on},
            "notice": self.notice,
        })
    }
}

#[cfg(test)]
mod tests;
