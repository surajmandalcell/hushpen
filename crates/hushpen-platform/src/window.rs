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
#[cfg(target_os = "macos")]
mod overlay_appkit;
#[cfg(target_os = "linux")]
mod overlay_x11;
#[cfg(target_os = "macos")]
mod visibility_appkit;
#[cfg(target_os = "linux")]
mod visibility_x11;

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

/// Takes the window off the screen without closing it, so the tray can bring it back. Backends
/// it does not know are left alone; the caller then falls back to minimizing.
pub fn hide(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => visibility_x11::hide(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::hide(view),
        _ => Err(ChromeError::new("this window backend cannot be hidden")),
    }
}

/// Puts a hidden window back on the screen and in front.
pub fn show(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => visibility_x11::show(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::show(view),
        _ => Err(ChromeError::new("this window backend cannot be shown")),
    }
}

/// A window rectangle in screen pixels, origin at the top left of the primary screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Moves and sizes the flow bar in one step. The window manager does not manage a pop-up, so
/// the app has to do it. Backends it does not know are left alone.
pub fn place_overlay(handle: WindowHandle, frame: Frame) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => overlay_x11::place(window, frame),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => overlay_appkit::place(view, frame),
        _ => {
            let _ = frame;
            Ok(())
        }
    }
}

/// Where the pointer is on the screen and whether the left button is down. Pixels on X11,
/// points on macOS, from the top left of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    pub left: bool,
}

/// Reads the pointer, for dragging the flow bar. `None` where the backend cannot say.
pub fn pointer(handle: WindowHandle) -> Option<Pointer> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(_) => overlay_x11::pointer().ok(),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(_) => overlay_appkit::pointer().ok(),
        _ => None,
    }
}

/// Keeps the flow bar above every other window, and visible while another app is in front.
/// Call once, right after the window opens.
pub fn pin_overlay(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => overlay_x11::pin(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => overlay_appkit::pin(view),
        _ => Ok(()),
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
        let frame = Frame {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        assert!(place_overlay(WindowHandle::Other, frame).is_ok());
        assert!(pin_overlay(WindowHandle::Other).is_ok());
        assert!(pointer(WindowHandle::Other).is_none());
    }
}
