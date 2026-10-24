//! Starting the global keys again once Input Monitoring is granted.
//!
//! The key listener starts once, at launch. When macOS had not allowed Input Monitoring then,
//! the listener stays off. A grant made while Hushpen runs starts it here, with no restart. If
//! the listener still cannot start after a few tries, macOS gives the grant to a new process
//! only, and the permissions page offers Quit and Reopen.

use super::{Controller, KeysStatus, unavailable};
use crate::hook;
use gpui_kit::{App, Context};
use hushpen_core::onboarding::Availability;
use hushpen_core::permission::Access;
use hushpen_platform::keys::{Reason, Unavailable};
use serde_json::{Value, json};
use std::rc::Rc;

/// Starts the listener and gives back the switch for the session state (the Esc grab). It
/// attaches the listener to the shortcuts too, so the caller keeps nothing else.
pub type KeysRestart = Rc<dyn Fn(&mut App) -> Result<Rc<dyn Fn(bool)>, Unavailable>>;

/// Failed starts in a row, after a grant, before the page asks for a reopen.
pub const RETRIES: u32 = 3;

pub enum Attempt {
    /// Nothing to start: no way to start, no grant, or the keys are not off for a missing grant.
    Idle,
    Started(Rc<dyn Fn(bool)>),
    Failed(Unavailable),
}

#[derive(Default)]
pub struct Restarter {
    restart: Option<KeysRestart>,
    failures: u32,
}

impl Restarter {
    pub fn attach(&mut self, restart: KeysRestart) {
        self.restart = Some(restart);
    }

    /// Input Monitoring is granted, yet the listener failed to start every time.
    pub fn needs_reopen(&self) -> bool {
        self.failures >= RETRIES
    }

    /// Tries once when the grant is there and the listener is off because of a missing grant.
    pub fn attempt(
        &mut self,
        cx: &mut App,
        input_monitoring: Access,
        status: Option<&KeysStatus>,
    ) -> Attempt {
        let Some(restart) = self.restart.clone() else {
            return Attempt::Idle;
        };
        if input_monitoring != Access::Granted {
            self.failures = 0;
            return Attempt::Idle;
        }
        let Some(KeysStatus::Unavailable(why)) = status else {
            return Attempt::Idle;
        };
        if why.reason != Reason::Permission {
            return Attempt::Idle;
        }
        match restart(cx) {
            Ok(session) => {
                self.failures = 0;
                Attempt::Started(session)
            }
            Err(why) => {
                self.failures = self.failures.saturating_add(1);
                Attempt::Failed(why)
            }
        }
    }
}

impl Controller {
    /// Starts the global keys when Input Monitoring is granted and they are still off, so the
    /// grant works with no restart. Failed starts add up to [`Controller::keys_reopen_needed`].
    pub(super) fn restart_keys(&mut self, cx: &mut Context<Self>) {
        let input_monitoring = self.access().input_monitoring;
        let attempt = self
            .keys_restarter
            .attempt(cx, input_monitoring, self.keys.as_ref());
        match attempt {
            Attempt::Idle => {}
            Attempt::Started(session_active) => {
                self.keys = Some(KeysStatus::Available);
                self.session_active = Some(session_active);
                hook::record_event("keys", "started after the Input Monitoring grant");
                cx.notify();
            }
            Attempt::Failed(why) => {
                log::warn!("global keys did not start after the grant: {}", why.message);
                self.keys = Some(KeysStatus::Unavailable(why));
                if self.keys_reopen_needed() {
                    hook::record_event("keys", "reopen needed");
                    cx.notify();
                }
            }
        }
    }

    /// How to start the global keys again after a late grant. Without it a grant changes
    /// nothing for the keys.
    pub fn attach_keys_restart(&mut self, restart: KeysRestart) {
        self.keys_restarter.attach(restart);
    }

    /// Input Monitoring is granted and the keys still do not start: the page offers Quit and
    /// Reopen, because macOS gives the grant to a new process.
    pub fn keys_reopen_needed(&self) -> bool {
        self.keys_restarter.needs_reopen()
    }

    /// Whether the global keys run, for the onboarding permissions page.
    pub fn keys_availability(&self) -> Availability {
        match &self.keys {
            Some(KeysStatus::Available) => Availability::Ready,
            Some(KeysStatus::Unavailable(why)) => unavailable(why),
            None => Availability::Off("Global keys are not running.".into()),
        }
    }

    /// `hookctl state` section `global_keys`.
    pub fn keys_json(&self) -> Value {
        match &self.keys {
            Some(KeysStatus::Available) => {
                json!({"available": true, "reason": null, "reopen": false})
            }
            Some(KeysStatus::Unavailable(why)) => json!({
                "available": false,
                "reason": why.reason.key(),
                "message": why.message,
                "reopen": self.keys_reopen_needed(),
            }),
            None => json!({"available": false, "reason": "not-started"}),
        }
    }

    /// The words Home shows in its notice row when the global keys are off, if they are.
    pub fn keys_notice(&self) -> Option<&str> {
        match &self.keys {
            Some(KeysStatus::Unavailable(why)) => Some(&why.message),
            _ => None,
        }
    }
}
