//! Global keys: the hold key for push-to-talk, and Esc while a session runs.
//!
//! Both platforms turn their raw input into [`AppEvent`]s and hand them to a [`Sink`] from their
//! own thread. The sink only queues; it never touches app state. When keys cannot work (Wayland,
//! no display, a missing macOS permission) [`GlobalKeys::start`] says why and the app keeps
//! working through its buttons.

use hushpen_core::dictation::AppEvent;
use std::sync::Arc;

pub mod session;
pub mod shortcut;
pub mod tap;

pub use shortcut::Shortcut;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "macos")]
mod macos;

/// Where the platform thread sends what it saw. Called from the platform thread.
pub type Sink = Arc<dyn Fn(AppEvent) + Send + Sync>;

/// The modifier that starts a dictation while it is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldKey {
    /// Right Option on a Mac, Right Alt (X11 keycode 108) elsewhere.
    #[default]
    RightOption,
    /// The Fn key. Only macOS reports it.
    Fn,
}

impl HoldKey {
    pub fn key(self) -> &'static str {
        match self {
            HoldKey::RightOption => "right-option",
            HoldKey::Fn => "fn",
        }
    }
}

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
    /// `paste_last` is the "Paste last transcript" shortcut. The sink gets
    /// [`AppEvent::PasteLast`] when it is released.
    pub fn start(
        hold: HoldKey,
        paste_last: Option<Shortcut>,
        sink: Sink,
    ) -> Result<Self, Unavailable> {
        start(hold, paste_last, sink)
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
fn start(
    hold: HoldKey,
    paste_last: Option<Shortcut>,
    sink: Sink,
) -> Result<GlobalKeys, Unavailable> {
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
        inner: x11::X11Keys::start(hold, paste_last, sink)?,
    })
}

#[cfg(target_os = "macos")]
fn start(
    hold: HoldKey,
    paste_last: Option<Shortcut>,
    sink: Sink,
) -> Result<GlobalKeys, Unavailable> {
    Ok(GlobalKeys {
        inner: macos::MacKeys::start(hold, paste_last, sink)?,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn start(
    _hold: HoldKey,
    _paste_last: Option<Shortcut>,
    _sink: Sink,
) -> Result<GlobalKeys, Unavailable> {
    Err(Unavailable::new(
        Reason::Unsupported,
        "Global keys are not available on this system.",
    ))
}
