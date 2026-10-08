//! Hide a window to the menu bar item and bring it back on macOS.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::ChromeError;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow};

fn window_of(view: usize) -> Result<Retained<NSWindow>, ChromeError> {
    MainThreadMarker::new().ok_or_else(|| ChromeError::new("the window needs the main thread"))?;
    if view == 0 {
        return Err(ChromeError::new("null NSView"));
    }
    // SAFETY: GPUI hands out the address of its live `NSView`, and the caller runs on the main
    // thread while the window is open.
    let view = unsafe { &*(view as *const NSView) };
    view.window()
        .ok_or_else(|| ChromeError::new("the view is not in a window yet"))
}

pub(super) fn hide(view: usize) -> Result<(), ChromeError> {
    window_of(view)?.orderOut(None);
    Ok(())
}

pub(super) fn show(view: usize) -> Result<(), ChromeError> {
    let window = window_of(view)?;
    if window.isMiniaturized() {
        window.deminiaturize(None);
    }
    window.makeKeyAndOrderFront(None);
    Ok(())
}
