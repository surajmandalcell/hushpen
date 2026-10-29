//! The flow bar's native window: a GPUI pop-up that the app places itself.

use super::{FALLBACK_SCREEN, FlowBar, Host, Native, Screen};
use crate::native::native_handle;
use gpui_kit::{
    AnyWindowHandle, App, Bounds, Entity, Pixels, WeakEntity, WindowBackgroundAppearance,
    WindowBounds, WindowKind, WindowOptions,
};
use hushpen_platform::window::{Frame, pin_overlay, place_overlay};

/// The window class of the pop-up. It must not contain `hushpen`: the harness finds the main
/// window by that class.
const APP_ID: &str = "flowbar";

/// Opens the pop-up on first use, then moves and sizes it. The window never takes the focus:
/// it opens unfocused, as a pop-up that the window manager does not manage (X11) or a
/// non-activating panel (macOS).
pub struct PopUp {
    /// Weak, because the app's frame task owns the host until the app is dropped.
    view: WeakEntity<FlowBar>,
    handle: Option<AnyWindowHandle>,
}

impl PopUp {
    pub fn new(view: &Entity<FlowBar>) -> Self {
        Self {
            view: view.downgrade(),
            handle: None,
        }
    }

    fn open(&mut self, bounds: Bounds<Pixels>, cx: &mut App) -> Option<Native> {
        let view = self.view.upgrade()?;
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
                self.handle = Some(handle);
                let native = handle
                    .update(cx, |_, window, cx| {
                        // The Kit root paints the theme background under the pill, and GPUI's
                        // X11 renderer starts opaque until the background is set again.
                        crate::window_chrome::apply_see_through(true, window, cx);
                        native_handle(window)
                    })
                    .ok()?;
                Some(Box::new(move || {
                    if let Err(error) = pin_overlay(native) {
                        log::warn!("the flow bar could not be pinned on top: {error}");
                    }
                }))
            }
            Err(error) => {
                log::warn!("the flow bar window could not open: {error:#}");
                None
            }
        }
    }
}

impl Host for PopUp {
    fn screen(&self, cx: &App) -> Screen {
        let desktop = cx
            .primary_display()
            .map(|display| {
                let size = display.bounds().size;
                (f32::from(size.width), f32::from(size.height))
            })
            .unwrap_or(FALLBACK_SCREEN.size);
        let Some(found) = hushpen_platform::window::pointer_screen() else {
            return Screen {
                origin: (0.0, 0.0),
                size: desktop,
            };
        };
        // X11 counts device pixels and GPUI logical ones: GPUI's display is the whole root
        // window divided by its scale. AppKit counts points on both sides.
        let scale = if cfg!(target_os = "linux") && desktop.0 > 0.0 {
            (found.desktop_width / desktop.0).max(0.1)
        } else {
            1.0
        };
        let rect = found.screen;
        Screen {
            origin: (rect.x / scale, rect.y / scale),
            size: (rect.width / scale, rect.height / scale),
        }
    }

    fn show(&mut self, bounds: Bounds<Pixels>, cx: &mut App) -> Option<Native> {
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
                let native = native_handle(window);
                Box::new(move || {
                    if let Err(error) = place_overlay(native, frame) {
                        log::warn!("the flow bar could not move: {error}");
                    }
                }) as Native
            });
            if let Ok(native) = placed {
                return Some(native);
            }
            // The window is gone, for example because the display server dropped it.
            self.handle = None;
        }
        self.open(bounds, cx)
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
