//! macOS window chrome. The Hushpen traffic lights are drawn by the app, so the
//! native ones are hidden, and the window can neither zoom nor enter full screen.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::ChromeError;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSView, NSWindowButton, NSWindowCollectionBehavior, NSWindowStyleMask};

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

    for button in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(button) {
            button.setHidden(true);
        }
    }

    // Minimize stays available to the app's own button. Resize and zoom do not.
    let mask =
        (window.styleMask() | NSWindowStyleMask::Miniaturizable) & !NSWindowStyleMask::Resizable;
    window.setStyleMask(mask);
    window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenNone);
    Ok(())
}
