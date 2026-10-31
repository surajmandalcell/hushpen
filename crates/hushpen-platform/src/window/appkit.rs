//! macOS window chrome. AppKit draws the one set of traffic lights and GPUI places them, so
//! nothing here moves a standard button; it only reads the close button frame, and hides the
//! zoom button. The window can neither zoom nor enter full screen.
//!
//! GPUI already builds the style mask (titled, closable, miniaturizable, full size content),
//! while the window keeps a fixed width and resizes in height only, so this module leaves the
//! mask alone: changing it on a live window makes
//! AppKit rebuild the title bar and a second row of buttons appears.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::{ChromeError, SizeLimits};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior};
use objc2_foundation::NSSize;

pub(super) fn lock(view: usize, limits: SizeLimits) -> Result<(), ChromeError> {
    let window = window_of(view)?;
    apply(&window, limits);
    Ok(())
}

/// The height of the close button frame; GPUI places the traffic lights by this frame.
pub(super) fn close_button_height(view: usize) -> Option<f64> {
    let button = window_of(view)
        .ok()?
        .standardWindowButton(NSWindowButton::CloseButton)?;
    Some(button.frame().size.height)
}

fn window_of(view: usize) -> Result<Retained<NSWindow>, ChromeError> {
    MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("window chrome needs the main thread"))?;
    if view == 0 {
        return Err(ChromeError::new("null NSView"));
    }
    // SAFETY: GPUI hands out the address of its live `NSView`, and the caller
    // runs on the main thread while the window is open.
    let view = unsafe { &*(view as *const NSView) };
    view.window()
        .ok_or_else(|| ChromeError::new("the view is not in a window yet"))
}

/// The zoom button cannot be pressed and is hidden, so VoiceOver does not announce a disabled
/// control the window does not have. The window never goes full screen. Its content keeps one
/// width and a height between the two limits.
fn apply(window: &NSWindow, limits: SizeLimits) {
    let width = f64::from(limits.width);
    window.setContentMinSize(NSSize::new(width, f64::from(limits.min_height)));
    window.setContentMaxSize(NSSize::new(width, f64::from(limits.max_height)));
    if let Some(zoom) = window.standardWindowButton(NSWindowButton::ZoomButton) {
        zoom.setEnabled(false);
        zoom.setHidden(true);
    }
    window.setCollectionBehavior(NSWindowCollectionBehavior::FullScreenNone);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_refuses_a_null_view() {
        // Off the main thread the call fails before it reads the address.
        let limits = SizeLimits {
            width: 940,
            min_height: 600,
            max_height: 900,
        };
        assert!(lock(0, limits).is_err());
    }
}
