//! Text insertion into the focused app.
//!
//! The app asks [`Inserter::target`] on its main thread when the text is ready, then runs
//! [`Inserter::insert`] on a worker thread. The order of the clipboard steps and the restore
//! rules live in `hushpen_core::insert::flow`; each platform supplies the clipboard and the key
//! press behind it.

use crate::keys::Unavailable;
use hushpen_core::insert::flow::{Clock, Step};
use hushpen_core::insert::{Method, Overrides, Report};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Instant;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::key_code_for;

/// The window name of Hushpen's own windows (`WM_CLASS`).
pub const OWN_APP_ID: &str = "hushpen";

/// The app that has the focus.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    /// The native window id (X11), or the process id (macOS). `None` when nothing has focus.
    pub window: Option<u64>,
    /// The name for the report: the class (X11) or the bundle id (macOS).
    pub label: String,
    /// Every name that can match an `insert.appChords` entry.
    pub classes: Vec<String>,
    /// The focused window is one of Hushpen's own.
    pub own: bool,
}

pub trait Inserter: Send + Sync {
    /// Reads the focus. Cheap, and called on the app's main thread.
    fn target(&self) -> Target;

    /// Puts `text` into `target`: saves the clipboard, writes the text, presses the paste chord,
    /// and puts the clipboard back. Blocks until the restore is over; `observe` hears the steps
    /// as they happen. One insertion runs at a time.
    fn insert(
        &self,
        text: &str,
        target: &Target,
        overrides: &Overrides,
        observe: &mut dyn FnMut(Step<'_>),
    ) -> Report;
}

/// The inserter of this system, or why there is none.
pub fn system() -> Result<Arc<dyn Inserter>, Unavailable> {
    system_inserter()
}

#[cfg(target_os = "linux")]
fn system_inserter() -> Result<Arc<dyn Inserter>, Unavailable> {
    use crate::keys::session::{self, Session};
    match session::detect() {
        Session::Wayland => return Err(Unavailable::wayland()),
        Session::NoDisplay => {
            return Err(Unavailable::new(
                crate::keys::Reason::NoDisplay,
                "No X display is available, so paste is off.",
            ));
        }
        Session::X11 => {}
    }
    Ok(Arc::new(x11::X11Inserter::start()?))
}

#[cfg(target_os = "macos")]
fn system_inserter() -> Result<Arc<dyn Inserter>, Unavailable> {
    Ok(Arc::new(macos::MacInserter::new()))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn system_inserter() -> Result<Arc<dyn Inserter>, Unavailable> {
    Err(Unavailable::new(
        crate::keys::Reason::Unsupported,
        "Paste is not available on this system.",
    ))
}

/// Milliseconds since the first call in this process.
pub fn mono_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// The real clock for the insertion flow.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        mono_ms()
    }

    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Why an app gets a copy and no key press, or the method to use.
pub(crate) fn plan(
    target: &Target,
    choose: impl FnOnce(&Target) -> Method,
) -> (Method, Option<&'static str>) {
    if target.window.is_none() {
        return (Method::CopyOnly, Some("no-target"));
    }
    if target.own {
        return (Method::CopyOnly, Some("own-window"));
    }
    match choose(target) {
        Method::CopyOnly => (Method::CopyOnly, Some("override")),
        paste => (paste, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::insert::Chord;

    fn target(window: Option<u64>, own: bool) -> Target {
        Target {
            window,
            label: "XTerm".into(),
            classes: vec!["xterm".into(), "XTerm".into()],
            own,
        }
    }

    #[test]
    fn no_focused_window_gives_a_copy_with_the_reason() {
        let (method, note) = plan(&target(None, false), |_| Method::Paste(Chord::CtrlV));
        assert_eq!((method, note), (Method::CopyOnly, Some("no-target")));
    }

    #[test]
    fn a_hushpen_window_is_never_pasted_into() {
        let (method, note) = plan(&target(Some(7), true), |_| Method::Paste(Chord::CtrlV));
        assert_eq!((method, note), (Method::CopyOnly, Some("own-window")));
    }

    #[test]
    fn a_copy_only_setting_gives_a_copy_and_any_other_target_gets_its_chord() {
        let (method, note) = plan(&target(Some(7), false), |_| Method::CopyOnly);
        assert_eq!((method, note), (Method::CopyOnly, Some("override")));
        let (method, note) = plan(&target(Some(7), false), |_| {
            Method::Paste(Chord::CtrlShiftV)
        });
        assert_eq!((method, note), (Method::Paste(Chord::CtrlShiftV), None));
    }

    #[test]
    fn the_clock_moves_forward() {
        let first = mono_ms();
        SystemClock.sleep_ms(3);
        assert!(mono_ms() >= first + 3);
    }
}
