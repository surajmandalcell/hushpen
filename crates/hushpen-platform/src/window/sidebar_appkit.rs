//! The macOS sidebar material: a native `NSVisualEffectView` behind the GPUI view.
//!
//! GPUI's own blurred background (a `Selection` material with its tint and saturation removed)
//! shows no blur on current macOS. The main window is transparent instead, and this view puts the
//! system sidebar material under the sidebar column only. It sits below the GPUI view in the same
//! superview, so the parts that GPUI leaves transparent show it, and it takes no events.

// AppKit calls take a raw `NSView` address from the GPUI window handle.
#![allow(unsafe_code)]

use super::ChromeError;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// The view that holds the GPUI view, which is the window's content view.
fn host(view: usize) -> Result<(&'static NSView, Retained<NSView>), ChromeError> {
    MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("the sidebar material needs the main thread"))?;
    if view == 0 {
        return Err(ChromeError::new("null NSView"));
    }
    // SAFETY: GPUI hands out the address of its live `NSView`, and the caller runs on the main
    // thread while the window is open.
    let view = unsafe { &*(view as *const NSView) };
    // SAFETY: on the main thread, and the window keeps its content view alive while it is open.
    let host = unsafe { view.superview() }
        .ok_or_else(|| ChromeError::new("the view is not in a window yet"))?;
    Ok((view, host))
}

fn effect_views(host: &NSView) -> Vec<Retained<NSVisualEffectView>> {
    host.subviews()
        .iter()
        .filter_map(|sub| sub.downcast::<NSVisualEffectView>().ok())
        .collect()
}

/// Adds the sidebar material, `width` points wide and the full window height, at the left edge.
/// A second call adds nothing.
pub(super) fn add(view: usize, width: f64) -> Result<(), ChromeError> {
    let (view, host) = host(view)?;
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| ChromeError::new("the sidebar material needs the main thread"))?;
    if effect_views(&host)
        .iter()
        .any(|effect| effect.material() == NSVisualEffectMaterial::Sidebar)
    {
        return Ok(());
    }
    let frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width, host.bounds().size.height),
    );
    let effect = NSVisualEffectView::initWithFrame(mtm.alloc(), frame);
    effect.setMaterial(NSVisualEffectMaterial::Sidebar);
    effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    // Active, not FollowsWindowActiveState: the sidebar stays blurred while the window is not key.
    effect.setState(NSVisualEffectState::Active);
    effect.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewHeightSizable | NSAutoresizingMaskOptions::ViewMaxXMargin,
    );
    host.addSubview_positioned_relativeTo(&effect, NSWindowOrderingMode::Below, Some(view));
    Ok(())
}

/// The material behind the GPUI view: `sidebar`, `other` for any other effect view, or `None`.
pub(super) fn material(view: usize) -> Option<&'static str> {
    let (_, host) = host(view).ok()?;
    let effects = effect_views(&host);
    if effects
        .iter()
        .any(|effect| effect.material() == NSVisualEffectMaterial::Sidebar)
    {
        Some("sidebar")
    } else if effects.is_empty() {
        None
    } else {
        Some("other")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_view_gets_no_material() {
        // Off the main thread the calls fail before they read the address.
        assert!(add(0, 232.0).is_err());
        assert_eq!(material(0), None);
    }
}
