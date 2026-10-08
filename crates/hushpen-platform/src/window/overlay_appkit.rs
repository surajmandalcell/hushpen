//! The flow bar on macOS. GPUI already opens a pop-up as a non-activating panel at the pop-up
//! window level; what is left is to keep it on screen when another app is in front, and to
//! move it, because the app dragged it itself.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::{ChromeError, Frame, Pointer};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSScreen, NSView, NSWindow, NSWindowCollectionBehavior};
use objc2_foundation::{NSPoint, NSRect, NSSize};

fn window_of(view: usize) -> Result<objc2::rc::Retained<NSWindow>, ChromeError> {
    MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("the flow bar needs the main thread"))?;
    if view == 0 {
        return Err(ChromeError::new("null NSView"));
    }
    // SAFETY: GPUI hands out the address of its live `NSView`, and the caller runs on the main
    // thread while the window is open.
    let view = unsafe { &*(view as *const NSView) };
    view.window()
        .ok_or_else(|| ChromeError::new("the view is not in a window yet"))
}

pub(super) fn place(view: usize, frame: Frame) -> Result<(), ChromeError> {
    let window = window_of(view)?;
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("the flow bar needs the main thread"))?;
    // Screen coordinates here have the origin at the bottom left of the primary screen.
    let primary_height = NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height);
    let rect = NSRect::new(
        NSPoint::new(
            f64::from(frame.x),
            primary_height - f64::from(frame.y) - f64::from(frame.height),
        ),
        NSSize::new(f64::from(frame.width), f64::from(frame.height)),
    );
    window.setFrame_display(rect, true);
    Ok(())
}

/// Where the pointer is, in points from the top left of the primary screen, and whether the
/// left button is down.
pub(super) fn pointer() -> Result<Pointer, ChromeError> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("the flow bar needs the main thread"))?;
    let primary_height = NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height);
    let at = NSEvent::mouseLocation();
    Ok(Pointer {
        x: at.x as f32,
        y: (primary_height - at.y) as f32,
        left: NSEvent::pressedMouseButtons() & 1 != 0,
    })
}

pub(super) fn pin(view: usize) -> Result<(), ChromeError> {
    let window = window_of(view)?;
    window.setHidesOnDeactivate(false);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    Ok(())
}
