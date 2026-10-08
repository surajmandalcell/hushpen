//! The native window behind a GPUI window, as the plain handle the platform crate takes.

use gpui_kit::Window;
use hushpen_platform::window::WindowHandle;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub fn native_handle(window: &Window) -> WindowHandle {
    match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
        Ok(RawWindowHandle::Xcb(handle)) => WindowHandle::X11(handle.window.get()),
        Ok(RawWindowHandle::Xlib(handle)) => match u32::try_from(handle.window) {
            Ok(id) => WindowHandle::X11(id),
            Err(_) => WindowHandle::Other,
        },
        Ok(RawWindowHandle::AppKit(handle)) => {
            WindowHandle::AppKit(handle.ns_view.as_ptr() as usize)
        }
        _ => WindowHandle::Other,
    }
}
