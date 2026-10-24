//! First-run onboarding: five steps in the main window, and the repair of a lost grant.
//!
//! The pure decisions (the steps, where the window opens, what lets a step pass, which rows the
//! permissions page shows) live in `hushpen_core::onboarding`. This entity keeps the state of
//! the steps, reads the system for the facts, and tells the controller which key gate to hold.
//! The step is stored in `onboarding.step`, so a restart resumes at the same step.

mod guide;
mod mic_test;
mod model;
pub mod panel;
mod permissions;

pub use guide::{Guide, SystemGuide};
use mic_test::Message;
pub use mic_test::{MicPhase, MicTest};
use permissions::AccessibilityWait;
pub use permissions::{FOLLOWS_HINT, STALE_HINT};

use crate::controller::{Controller, Phase};
use crate::hook;
use crate::mic::Mic;
use crate::models::Models;
use crate::shortcuts::Shortcuts;
use crate::storage::Storage;
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui_kit::{App, Context, Entity, FocusHandle};
use hushpen_core::dictation::AppEvent;
use hushpen_core::onboarding::{self as rules, KeyGate, MicVerdict, Session, Start, Step};
use hushpen_core::permission::Permission;
use hushpen_core::shortcut::{Platform, Slot};
use serde_json::{Value, json};
use std::rc::Rc;
use std::time::Duration;

const COMPLETED: &str = "onboarding.completed";

const STEP: &str = "onboarding.step";

const UPDATES: &str = "updates.check";

/// How often the permissions are read again while onboarding is on screen.
const REFRESH: Duration = Duration::from_secs(1);

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
    /// The start reads of the permissions still settle, and a finished onboarding opens the
    /// repair page if they end with a lost grant.
    waiting_for_grants: bool,
    mic_test: MicTest,
    practice: Option<String>,
    updates_on: bool,
    notice: Option<String>,
    /// The permissions whose row shows [`STALE_HINT`].
    stale: Vec<Permission>,
    /// The permissions whose pane click opened the Accessibility pane instead, so the row shows
    /// [`FOLLOWS_HINT`].
    follows: Vec<Permission>,
    /// The Accessibility prompt that is still being waited for.
    accessibility_wait: Option<AccessibilityWait>,
    allow_clicks: u64,
    messages: UnboundedSender<Message>,
    /// Allow and Open Settings of each permission, in `Permission::ALL` order, two per row.
    pub(crate) permission_focus: Vec<FocusHandle>,
    pub(crate) relaunch_focus: FocusHandle,
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
        let settling = controller.read(cx).permissions_settling();
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
            waiting_for_grants: completed && mode == Mode::Hidden && settling,
            mic_test: MicTest {
                phase: MicPhase::Idle,
                peak: 0.0,
                verdict: None,
                passed: false,
            },
            practice: None,
            updates_on,
            notice: None,
            stale: Vec::new(),
            follows: Vec::new(),
            accessibility_wait: None,
            allow_clicks: 0,
            messages,
            permission_focus: (0..Permission::ALL.len() * 2)
                .map(|_| cx.focus_handle().tab_stop(true))
                .collect(),
            relaunch_focus: cx.focus_handle().tab_stop(true),
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

    fn controller_changed(&mut self, cx: &mut Context<Self>) {
        self.grants_settled(cx);
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
                    "relaunch": row.relaunch,
                    "hint": row.opens.and_then(|permission| self.hint(permission)),
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
