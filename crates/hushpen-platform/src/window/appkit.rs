//! macOS window chrome. AppKit draws the one set of traffic lights, so nothing here hides or
//! moves a standard button. The window can neither zoom nor enter full screen.
//!
//! GPUI already builds the style mask (titled, closable, miniaturizable, not resizable, full
//! size content), so this module leaves the mask alone: changing it on a live window makes
//! AppKit rebuild the title bar and a second row of buttons appears.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::ChromeError;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior};

pub(super) fn lock(view: usize, _width: u16, _height: u16) -> Result<(), ChromeError> {
    MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("window chrome needs the main thread"))?;
    if view == 0 {
        return Err(ChromeError::new("null NSView"));
    }
    // SAFETY: GPUI hands out the address of its live `NSView`, and the caller
    // runs on the main thread while the window is open.
    let view = unsafe { &*(view as *const NSView) };
    let window = view
        .window()
        .ok_or_else(|| ChromeError::new("the view is not in a window yet"))?;
    apply(&window);
    Ok(())
}

/// The zoom button stays on screen but cannot be pressed, and the window never goes full screen.
fn apply(window: &NSWindow) {
    if let Some(zoom) = window.standardWindowButton(NSWindowButton::ZoomButton) {
        zoom.setEnabled(false);
    }
    window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenNone);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_refuses_a_null_view() {
        // Off the main thread the call fails before it reads the address.
        assert!(lock(0, 940, 600).is_err());
    }
}
