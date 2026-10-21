//! The sidebar material on a real AppKit window that is never shown, so no app or window comes
//! to the front. AppKit views need the main thread, so this target has its own `main`.

// Creating an `NSWindow` is unsafe in objc2: AppKit may release it on close.
#![allow(unsafe_code)]

#[cfg(target_os = "macos")]
fn main() {
    use hushpen_platform::window::{WindowHandle, add_sidebar_material, sidebar_material};
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSBackingStoreType, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowStyleMask,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    let mtm = MainThreadMarker::new().expect("the test runs on the main thread");
    let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(940.0, 600.0));
    // SAFETY: the window is never closed, and it is not released on close either.
    let window = unsafe {
        let window = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            rect,
            NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView,
            NSBackingStoreType::Buffered,
            false,
        );
        window.setReleasedWhenClosed(false);
        window
    };
    let content = window.contentView().expect("a content view");
    // Stands in for the GPUI view: a full-size subview of the content view.
    let gpui = NSView::initWithFrame(mtm.alloc(), content.bounds());
    content.addSubview(&gpui);
    let handle = WindowHandle::AppKit(&*gpui as *const NSView as usize);

    assert_eq!(
        sidebar_material(handle),
        None,
        "no material before the call"
    );
    add_sidebar_material(handle, 232.0).expect("add the sidebar material");
    add_sidebar_material(handle, 232.0).expect("a second call is harmless");
    assert_eq!(sidebar_material(handle), Some("sidebar"));

    let subviews = content.subviews();
    assert_eq!(subviews.len(), 2, "one material view and the GPUI view");
    let effect = subviews
        .objectAtIndex(0)
        .downcast::<NSVisualEffectView>()
        .expect("the material view is below the GPUI view");
    assert!(std::ptr::eq(&*subviews.objectAtIndex(1), &*gpui));
    assert_eq!(effect.material(), NSVisualEffectMaterial::Sidebar);
    assert_eq!(
        effect.blendingMode(),
        NSVisualEffectBlendingMode::BehindWindow
    );
    assert_eq!(effect.state(), NSVisualEffectState::Active);
    let frame = effect.frame();
    assert_eq!(
        (frame.origin.x, frame.size.width, frame.size.height),
        (0.0, 232.0, content.bounds().size.height)
    );
    assert!(!window.isVisible(), "the window never showed");
    println!("sidebar_material: ok");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
