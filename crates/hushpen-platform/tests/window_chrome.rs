//! The macOS window lock on a real AppKit window that is never shown, so no app or window comes
//! to the front. AppKit views need the main thread, so this target has its own `main`.

// Creating an `NSWindow` is unsafe in objc2: AppKit may release it on close.
#![allow(unsafe_code)]

#[cfg(target_os = "macos")]
fn main() {
    use hushpen_platform::window::{WindowHandle, lock_chrome};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBackingStoreType, NSView, NSWindow, NSWindowButton, NSWindowCollectionBehavior,
        NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().expect("the test runs on the main thread");
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(940.0, 600.0));
    // SAFETY: the window is never closed, and it is not released on close either.
    let window = unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
                | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window
    };
    let content = window.contentView().expect("a content view");
    let gpui = NSView::initWithFrame(mtm.alloc(), content.bounds());
    content.addSubview(&gpui);
    let handle = WindowHandle::AppKit(&*gpui as *const NSView as usize);

    lock_chrome(handle, 940, 600).expect("lock the chrome");

    let zoom = window
        .standardWindowButton(NSWindowButton::ZoomButton)
        .expect("a titled window has a zoom button");
    assert!(!zoom.isEnabled(), "the zoom button cannot be pressed");
    assert!(
        zoom.isHidden(),
        "a hidden button is left out of the accessibility tree"
    );
    for kept in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
    ] {
        let button = window
            .standardWindowButton(kept)
            .expect("a standard button");
        assert!(!button.isHidden(), "{kept:?} stays");
    }
    assert_eq!(
        window.collectionBehavior(),
        NSWindowCollectionBehavior::FullScreenNone
    );
    assert!(!window.isVisible(), "the window never showed");
    println!("window_chrome: ok");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
