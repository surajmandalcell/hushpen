//! The flow bar's native window: a GPUI pop-up that the app places itself.

use super::{FlowBar, Host};
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, Window, WindowBackgroundAppearance, WindowBounds,
    WindowKind, WindowOptions,
};
use hushpen_platform::window::{Frame, WindowHandle, pin_overlay, place_overlay};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// The window class of the pop-up. It must not contain `hushpen`: the harness finds the main
/// window by that class.
const APP_ID: &str = "flowbar";

pub(crate) fn native_handle(window: &Window) -> WindowHandle {
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

/// Opens the pop-up on first use, then moves and sizes it. The window never takes the focus:
/// it opens unfocused, as a pop-up that the window manager does not manage (X11) or a
/// non-activating panel (macOS).
pub struct PopUp {
    view: Entity<FlowBar>,
    handle: Option<AnyWindowHandle>,
}

impl PopUp {
    pub fn new(view: Entity<FlowBar>) -> Self {
        Self { view, handle: None }
    }

    fn open(&mut self, bounds: Bounds<Pixels>, cx: &mut App) {
        let view = self.view.clone();
        let opened = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                focus: false,
                show: true,
                kind: WindowKind::PopUp,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                app_id: Some(APP_ID.into()),
                inactive_frame_interval: None,
                ..Default::default()
            },
            cx,
            move |_, _| view,
        );
        match opened {
            Ok((handle, _)) => {
                let _ = handle.update(cx, |_, window, _| {
                    if let Err(error) = pin_overlay(native_handle(window)) {
                        log::warn!("the flow bar could not be pinned on top: {error}");
                    }
                });
                self.handle = Some(handle);
            }
            Err(error) => log::warn!("the flow bar window could not open: {error:#}"),
        }
    }
}

impl Host for PopUp {
    fn screen(&self, cx: &App) -> (f32, f32) {
        cx.primary_display()
            .map(|display| {
                let size = display.bounds().size;
                (f32::from(size.width), f32::from(size.height))
            })
            .unwrap_or((1280.0, 800.0))
    }

    fn show(&mut self, bounds: Bounds<Pixels>, cx: &mut App) {
        if let Some(handle) = self.handle {
            let placed = handle.update(cx, |_, window, _| {
                // X11 takes device pixels, AppKit takes points.
                let scale = if cfg!(target_os = "linux") {
                    window.scale_factor()
                } else {
                    1.0
                };
                let device = |value: Pixels| f32::from(value) * scale;
                let frame = Frame {
                    x: device(bounds.origin.x).round() as i32,
                    y: device(bounds.origin.y).round() as i32,
                    width: device(bounds.size.width).round() as u32,
                    height: device(bounds.size.height).round() as u32,
                };
                if let Err(error) = place_overlay(native_handle(window), frame) {
                    log::warn!("the flow bar could not move: {error}");
                }
            });
            if placed.is_ok() {
                return;
            }
            // The window is gone, for example because the display server dropped it.
            self.handle = None;
        }
        self.open(bounds, cx);
    }

    fn hide(&mut self, cx: &mut App) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
        }
    }

    fn window(&self) -> Option<AnyWindowHandle> {
        self.handle
    }
}
