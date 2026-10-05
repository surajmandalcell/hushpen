//! Window chrome that GPUI does not control the same way on every OS.
//!
//! GPUI 0.3.7 ignores `is_resizable: false` on X11 (it writes a 16384 px
//! `max_size`) and keeps the native traffic lights on macOS. The app calls
//! [`lock_chrome`] once, right after the window opens, with the native handle.

use std::fmt;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "macos")]
mod appkit;

/// The native window behind a GPUI window, as plain integers so the platform
/// layer never depends on GPUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHandle {
    /// An X11 window id.
    X11(u32),
    /// The address of the `NSView` that GPUI draws into.
    AppKit(usize),
    /// Wayland and every other backend.
    Other,
}

#[derive(Debug)]
pub struct ChromeError(String);

impl ChromeError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ChromeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ChromeError {}

/// `_MOTIF_WM_HINTS` flags: the `decorations` and `functions` fields are set.
const MWM_HINTS_FUNCTIONS: u32 = 1 << 0;
const MWM_HINTS_DECORATIONS: u32 = 1 << 1;
/// Window functions the app allows: move, minimize, close. Not resize or maximize.
const MWM_FUNC_MOVE: u32 = 1 << 2;
const MWM_FUNC_MINIMIZE: u32 = 1 << 3;
const MWM_FUNC_CLOSE: u32 = 1 << 5;

/// Payload for `_MOTIF_WM_HINTS` (`flags, functions, decorations, input_mode,
/// status`): no frame, and only move, minimize, and close allowed.
pub const MOTIF_FRAMELESS_FIXED: [u32; 5] = [
    MWM_HINTS_FUNCTIONS | MWM_HINTS_DECORATIONS,
    MWM_FUNC_MOVE | MWM_FUNC_MINIMIZE | MWM_FUNC_CLOSE,
    0,
    0,
    0,
];

/// Make the window frameless and fixed at `width` x `height`: no maximize, no
/// full screen, no resize. Safe to call on any backend; backends it does not
/// know are left alone.
pub fn lock_chrome(handle: WindowHandle, width: u16, height: u16) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => x11::lock(window, width, height),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => appkit::lock(view, width, height),
        _ => {
            let _ = (width, height);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motif_hints_remove_the_frame_and_the_resize_functions() {
        let [flags, functions, decorations, _, _] = MOTIF_FRAMELESS_FIXED;
        assert_eq!(flags & MWM_HINTS_DECORATIONS, MWM_HINTS_DECORATIONS);
        assert_eq!(decorations, 0, "no decorations at all");
        assert_eq!(flags & MWM_HINTS_FUNCTIONS, MWM_HINTS_FUNCTIONS);
        const MWM_FUNC_RESIZE: u32 = 1 << 1;
        const MWM_FUNC_MAXIMIZE: u32 = 1 << 4;
        assert_eq!(functions & MWM_FUNC_RESIZE, 0);
        assert_eq!(functions & MWM_FUNC_MAXIMIZE, 0);
        assert_ne!(functions & MWM_FUNC_MINIMIZE, 0);
        assert_ne!(functions & MWM_FUNC_CLOSE, 0);
    }

    #[test]
    fn unknown_backends_are_left_alone() {
        assert!(lock_chrome(WindowHandle::Other, 780, 520).is_ok());
    }
}
