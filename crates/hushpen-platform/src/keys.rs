//! Global keys: the hold key for push-to-talk, and Esc while a session runs.
//!
//! Both platforms turn their raw input into [`AppEvent`]s and hand them to a [`Sink`] from their
//! own thread. The sink only queues; it never touches app state. When keys cannot work (Wayland,
//! no display, a missing macOS permission) [`GlobalKeys::start`] says why and the app keeps
//! working through its buttons.

use hushpen_core::dictation::AppEvent;
use hushpen_core::shortcut::{Bindings, Combo, Recording};
use std::sync::Arc;

mod hub;
#[cfg(any(target_os = "linux", test))]
mod keymap;
pub mod session;
pub mod tap;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "macos")]
mod macos;

/// Where the platform thread sends what it saw. Called from the platform thread.
pub type Sink = Arc<dyn Fn(AppEvent) + Send + Sync>;

/// Where the recorder reports. Called from the platform thread.
pub type RecordSink = Arc<dyn Fn(Recording) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Wayland,
    NoDisplay,
    /// macOS has not allowed Input Monitoring for the app.
    Permission,
    Unsupported,
    Failed,
}

impl Reason {
    pub fn key(self) -> &'static str {
        match self {
            Reason::Wayland => "wayland",
            Reason::NoDisplay => "no-display",
            Reason::Permission => "permission",
            Reason::Unsupported => "unsupported",
            Reason::Failed => "failed",
        }
    }
}

/// Why global keys are off, in words the user can read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub reason: Reason,
    pub message: String,
}

impl Unavailable {
    pub fn new(reason: Reason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    pub fn wayland() -> Self {
        Self::new(
            Reason::Wayland,
            "Not available on Wayland. Global keys and paste need an X11 session.",
        )
    }
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Unavailable {}

/// The running key listener. It lives until the process ends.
pub struct GlobalKeys {
    #[cfg(target_os = "linux")]
    inner: x11::X11Keys,
    #[cfg(target_os = "macos")]
    inner: macos::MacKeys,
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    inner: (),
}

impl GlobalKeys {
    /// `bindings` are the live shortcuts. The sink gets their pipeline events; the record sink
    /// gets progress while [`GlobalKeys::start_recording`] is open.
    pub fn start(bindings: Bindings, sink: Sink, record: RecordSink) -> Result<Self, Unavailable> {
        start(bindings, sink, record)
    }

    /// Swaps the live shortcuts. A hold that is on is let go first.
    pub fn set_bindings(&self, bindings: Bindings) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        self.inner.set_bindings(bindings);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = bindings;
    }

    /// Opens the recorder: no live shortcut fires until it reports a result or is stopped.
    pub fn start_recording(&self) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        self.inner.start_recording();
    }

    pub fn stop_recording(&self) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        self.inner.stop_recording();
    }

    /// Whether another app already holds this chord. Only a chord with a letter can be held:
    /// a modifier alone is never grabbed. macOS has no way to ask, so it always answers no.
    pub fn in_use(&self, combo: &Combo) -> bool {
        #[cfg(target_os = "linux")]
        return self.inner.in_use(combo);
        #[cfg(not(target_os = "linux"))]
        {
            let _ = combo;
            false
        }
    }

    /// Esc belongs to the pipeline while a session runs and to the focused app otherwise.
    /// X11 grabs Esc only while `active`; macOS only listens, so Esc always reaches the app.
    pub fn set_session_active(&self, active: bool) {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        self.inner.set_session_active(active);
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let _ = active;
    }
}

#[cfg(target_os = "linux")]
fn start(bindings: Bindings, sink: Sink, record: RecordSink) -> Result<GlobalKeys, Unavailable> {
    match session::detect() {
        session::Session::Wayland => return Err(Unavailable::wayland()),
        session::Session::NoDisplay => {
            return Err(Unavailable::new(
                Reason::NoDisplay,
                "No X display is available, so global keys are off.",
            ));
        }
        session::Session::X11 => {}
    }
    Ok(GlobalKeys {
        inner: x11::X11Keys::start(bindings, sink, record)?,
    })
}

#[cfg(target_os = "macos")]
fn start(bindings: Bindings, sink: Sink, record: RecordSink) -> Result<GlobalKeys, Unavailable> {
    Ok(GlobalKeys {
        inner: macos::MacKeys::start(bindings, sink, record)?,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn start(_bindings: Bindings, _sink: Sink, _record: RecordSink) -> Result<GlobalKeys, Unavailable> {
    Err(Unavailable::new(
        Reason::Unsupported,
        "Global keys are not available on this system.",
    ))
}
