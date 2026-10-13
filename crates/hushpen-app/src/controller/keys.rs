//! Starting the global keys again once Input Monitoring is granted.
//!
//! The key listener starts once, at launch. When macOS had not allowed Input Monitoring then,
//! the listener stays off. A grant made while Hushpen runs starts it here, with no restart. If
//! the listener still cannot start after a few tries, macOS gives the grant to a new process
//! only, and the permissions page offers Quit and Reopen.

use super::KeysStatus;
use gpui_kit::App;
use hushpen_core::permission::Access;
use hushpen_platform::keys::{Reason, Unavailable};
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
