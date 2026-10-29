//! Window chrome that GPUI does not control the same way on every OS.
//!
//! GPUI 0.3.7 ignores `is_resizable: false` on X11 (it writes a 16384 px
//! `max_size`) and leaves the zoom button and full screen on macOS. The app calls
//! [`lock_chrome`] once, right after the window opens, with the native handle. On X11 the lock
//! stays on watch and writes the hints back if a later write replaces them.

use std::fmt;

#[cfg(target_os = "linux")]
mod x11;

#[cfg(target_os = "linux")]
mod compositor_x11;

#[cfg(target_os = "macos")]
mod appkit;
#[cfg(target_os = "macos")]
mod overlay_appkit;
#[cfg(target_os = "linux")]
mod overlay_x11;
#[cfg(target_os = "macos")]
mod sidebar_appkit;
#[cfg(target_os = "macos")]
mod visibility_appkit;
#[cfg(target_os = "linux")]
mod visibility_x11;

/// The native window behind a GPUI window, as plain integers so the platform
/// layer never depends on GPUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHandle {
    /// An X11 window id.
    X11(u32),
    /// The address of the `NSView` that GPUI draws into.
    AppKit(usize),
    /// Wayland and every other backend.
    Other,
}

#[derive(Debug)]
pub struct ChromeError(String);

impl ChromeError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for ChromeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ChromeError {}

/// `_MOTIF_WM_HINTS` flags: the `decorations` and `functions` fields are set.
const MWM_HINTS_FUNCTIONS: u32 = 1 << 0;
const MWM_HINTS_DECORATIONS: u32 = 1 << 1;
/// Window functions the app allows: move, minimize, close. Not resize or maximize.
const MWM_FUNC_MOVE: u32 = 1 << 2;
const MWM_FUNC_MINIMIZE: u32 = 1 << 3;
const MWM_FUNC_CLOSE: u32 = 1 << 5;

/// Payload for `_MOTIF_WM_HINTS` (`flags, functions, decorations, input_mode,
/// status`): no frame, and only move, minimize, and close allowed.
pub const MOTIF_FRAMELESS_FIXED: [u32; 5] = [
    MWM_HINTS_FUNCTIONS | MWM_HINTS_DECORATIONS,
    MWM_FUNC_MOVE | MWM_FUNC_MINIMIZE | MWM_FUNC_CLOSE,
    0,
    0,
    0,
];

/// Whether a `WM_NORMAL_HINTS` read still pins the window: the program minimum and maximum both
/// equal the fixed size. GPUI writes its own hints (maximum 16384) and any later write replaces ours.
#[cfg(any(target_os = "linux", test))]
fn size_hints_pinned(
    min: Option<(i32, i32)>,
    max: Option<(i32, i32)>,
    width: u16,
    height: u16,
) -> bool {
    let size = Some((i32::from(width), i32::from(height)));
    min == size && max == size
}

/// Whether a `_MOTIF_WM_HINTS` read still carries [`MOTIF_FRAMELESS_FIXED`]. GPUI writes
/// `[2, 0, 0|1, 0, 0]` when it applies its decoration request, which brings the frame back.
#[cfg(any(target_os = "linux", test))]
fn motif_hints_locked(value: Option<&[u32]>) -> bool {
    value == Some(MOTIF_FRAMELESS_FIXED.as_slice())
}

/// Make the window frameless and fixed at `width` x `height`: no maximize, no
/// full screen, no resize. Safe to call on any backend; backends it does not
/// know are left alone.
pub fn lock_chrome(handle: WindowHandle, width: u16, height: u16) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => x11::lock(window, width, height),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => appkit::lock(view, width, height),
        _ => {
            let _ = (width, height);
            Ok(())
        }
    }
}

/// The height in points of the standard close button frame on macOS, which is taller than the
/// circle it draws. `None` on other backends or before AppKit made the button.
pub fn close_button_height(handle: WindowHandle) -> Option<f64> {
    match handle {
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => appkit::close_button_height(view),
        _ => None,
    }
}

/// Puts the native sidebar material behind the first `width` points of the window, full height,
/// on macOS. The window must be transparent there. Backends it does not know are left alone.
pub fn add_sidebar_material(handle: WindowHandle, width: f64) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => sidebar_appkit::add(view, width),
        _ => {
            let _ = width;
            Ok(())
        }
    }
}

/// Whether a compositor draws the screen, so a transparent window shows what is behind it
/// instead of black. macOS and Wayland always composite; on X11 a compositor owns the
/// `_NET_WM_CM_S<screen>` selection.
pub fn compositor_running() -> bool {
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(target_os = "linux")]
    {
        use crate::keys::session::{Session, detect};
        match detect() {
            Session::X11 => compositor_x11::running(),
            // A Wayland session is composited by design.
            Session::Wayland => true,
            Session::NoDisplay => false,
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return false;
}

/// Asks the compositor to blur what shows through `region` of the window: KWin reads
/// `_KDE_NET_WM_BLUR_BEHIND_REGION`; picom blurs ARGB windows from its own configuration and
/// needs no property. Backends it does not know are left alone.
pub fn blur_behind(handle: WindowHandle, region: Frame) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => compositor_x11::blur_behind(window, region),
        _ => {
            let _ = region;
            Ok(())
        }
    }
}

/// Takes the blur request back, for a compositor that went away.
pub fn clear_blur_behind(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => compositor_x11::clear_blur_behind(window),
        _ => Ok(()),
    }
}

/// The native material behind the window content: `sidebar`, `other`, or `None` when there
/// is none or the backend has no such thing.
pub fn sidebar_material(handle: WindowHandle) -> Option<&'static str> {
    match handle {
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => sidebar_appkit::material(view),
        _ => None,
    }
}

/// Takes the window off the screen without closing it, so the tray can bring it back. Backends
/// it does not know are left alone; the caller then falls back to minimizing.
pub fn hide(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => visibility_x11::hide(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::hide(view),
        _ => Err(ChromeError::new("this window backend cannot be hidden")),
    }
}

/// Puts a hidden window back on the screen and in front.
pub fn show(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => visibility_x11::show(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::show(view),
        _ => Err(ChromeError::new("this window backend cannot be shown")),
    }
}

/// Sends a window to the Dock on macOS. Other backends minimize through GPUI, which keeps the
/// window manager's own request.
pub fn minimize(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::minimize(view),
        _ => Err(ChromeError::new(
            "this window backend minimizes through GPUI",
        )),
    }
}

/// Puts a hidden window back on the screen without taking the focus or the front app. For
/// starts that must stay in the background.
pub fn show_unfocused(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => visibility_x11::show_unfocused(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => visibility_appkit::show_unfocused(view),
        _ => Err(ChromeError::new("this window backend cannot be shown")),
    }
}

/// Whether this process was launched behind the front app (macOS `open -g`, a login item).
/// Always false where a launch has no background form.
pub fn launched_in_background() -> bool {
    #[cfg(target_os = "macos")]
    return visibility_appkit::launched_in_background();
    #[cfg(not(target_os = "macos"))]
    return false;
}

/// Brings the whole app to the front on macOS; other systems have no app-level front.
pub fn activate_app() -> Result<(), ChromeError> {
    #[cfg(target_os = "macos")]
    return visibility_appkit::activate_app();
    #[cfg(not(target_os = "macos"))]
    Ok(())
}

/// Hides every window of the app on macOS (the app menu's Hide); a no-op elsewhere.
pub fn hide_app() -> Result<(), ChromeError> {
    #[cfg(target_os = "macos")]
    return visibility_appkit::hide_app();
    #[cfg(not(target_os = "macos"))]
    Ok(())
}

/// A window rectangle in screen pixels, origin at the top left of the primary screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Moves and sizes the flow bar in one step. The window manager does not manage a pop-up, so
/// the app has to do it. Backends it does not know are left alone.
pub fn place_overlay(handle: WindowHandle, frame: Frame) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => overlay_x11::place(window, frame),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => overlay_appkit::place(view, frame),
        _ => {
            let _ = frame;
            Ok(())
        }
    }
}

/// Where the pointer is on the screen and whether the left button is down. Pixels on X11,
/// points on macOS, from the top left of the screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    pub left: bool,
}

/// Reads the pointer, for dragging the flow bar. `None` where the backend cannot say.
pub fn pointer(handle: WindowHandle) -> Option<Pointer> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(_) => overlay_x11::pointer().ok(),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(_) => overlay_appkit::pointer().ok(),
        _ => None,
    }
}

/// A screen rectangle from the top left of the primary screen. Pixels on X11, points on macOS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ScreenRect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    fn distance(&self, x: f32, y: f32) -> f32 {
        let dx = (self.x - x).max(x - (self.x + self.width)).max(0.0);
        let dy = (self.y - y).max(y - (self.y + self.height)).max(0.0);
        dx.hypot(dy)
    }
}

/// The screen with the pointer, and the width of the whole desktop in the same units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerScreen {
    pub screen: ScreenRect,
    pub desktop_width: f32,
}

/// The screen that holds the point, or the nearest one when the point is on none of them.
fn screen_at(screens: &[ScreenRect], x: f32, y: f32) -> Option<ScreenRect> {
    screens
        .iter()
        .find(|screen| screen.contains(x, y))
        .or_else(|| {
            screens
                .iter()
                .min_by(|a, b| a.distance(x, y).total_cmp(&b.distance(x, y)))
        })
        .copied()
}

/// The screen the pointer is on, for the flow bar. `None` where the backend cannot say.
pub fn pointer_screen() -> Option<PointerScreen> {
    #[cfg(target_os = "linux")]
    let layout = overlay_x11::screens();
    #[cfg(target_os = "macos")]
    let layout = overlay_appkit::screens();
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let layout: Result<Layout, ChromeError> = Err(ChromeError::new("no screens here"));
    let layout = layout
        .map_err(|error| log::debug!("the screens could not be read: {error}"))
        .ok()?;
    Some(PointerScreen {
        screen: screen_at(&layout.screens, layout.pointer.0, layout.pointer.1)?,
        desktop_width: layout.desktop_width,
    })
}

/// Every screen, where the pointer is, and how wide the whole desktop is, all in one unit.
struct Layout {
    screens: Vec<ScreenRect>,
    pointer: (f32, f32),
    desktop_width: f32,
}

/// Keeps the flow bar above every other window, and visible while another app is in front.
/// Call once, right after the window opens.
pub fn pin_overlay(handle: WindowHandle) -> Result<(), ChromeError> {
    match handle {
        #[cfg(target_os = "linux")]
        WindowHandle::X11(window) => overlay_x11::pin(window),
        #[cfg(target_os = "macos")]
        WindowHandle::AppKit(view) => overlay_appkit::pin(view),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motif_hints_remove_the_frame_and_the_resize_functions() {
        let [flags, functions, decorations, _, _] = MOTIF_FRAMELESS_FIXED;
        assert_eq!(flags & MWM_HINTS_DECORATIONS, MWM_HINTS_DECORATIONS);
        assert_eq!(decorations, 0, "no decorations at all");
        assert_eq!(flags & MWM_HINTS_FUNCTIONS, MWM_HINTS_FUNCTIONS);
        const MWM_FUNC_RESIZE: u32 = 1 << 1;
        const MWM_FUNC_MAXIMIZE: u32 = 1 << 4;
        assert_eq!(functions & MWM_FUNC_RESIZE, 0);
        assert_eq!(functions & MWM_FUNC_MAXIMIZE, 0);
        assert_ne!(functions & MWM_FUNC_MINIMIZE, 0);
        assert_ne!(functions & MWM_FUNC_CLOSE, 0);
    }

    #[test]
    fn size_hints_are_pinned_only_when_min_and_max_equal_the_window_size() {
        let size = Some((940, 600));
        assert!(size_hints_pinned(size, size, 940, 600));
        // What GPUI writes after the window is created.
        assert!(!size_hints_pinned(None, Some((16384, 16384)), 940, 600));
        assert!(!size_hints_pinned(size, Some((16384, 16384)), 940, 600));
        assert!(!size_hints_pinned(size, None, 940, 600));
        assert!(!size_hints_pinned(None, None, 940, 600));
        assert!(!size_hints_pinned(Some((1, 1)), size, 940, 600));
    }

    #[test]
    fn motif_hints_are_locked_only_when_they_match_the_fixed_payload() {
        assert!(motif_hints_locked(Some(&MOTIF_FRAMELESS_FIXED)));
        assert!(!motif_hints_locked(None));
        assert!(!motif_hints_locked(Some(&[1 << 1, 0, 1, 0, 0])));
        assert!(!motif_hints_locked(Some(&[1 << 1, 0, 0, 0, 0])));
        assert!(!motif_hints_locked(Some(&[])));
    }

    #[test]
    fn unknown_backends_are_left_alone() {
        assert!(lock_chrome(WindowHandle::Other, 940, 600).is_ok());
        assert!(add_sidebar_material(WindowHandle::Other, 232.0).is_ok());
        assert_eq!(sidebar_material(WindowHandle::Other), None);
        let frame = Frame {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
        assert!(place_overlay(WindowHandle::Other, frame).is_ok());
        assert!(blur_behind(WindowHandle::Other, frame).is_ok());
        assert!(clear_blur_behind(WindowHandle::Other).is_ok());
        assert!(pin_overlay(WindowHandle::Other).is_ok());
        assert!(pointer(WindowHandle::Other).is_none());
    }

    fn rect(x: f32, y: f32, width: f32, height: f32) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_pointer_picks_the_screen_it_is_on() {
        let left = rect(0.0, 0.0, 1440.0, 900.0);
        let right = rect(1440.0, -200.0, 2560.0, 1440.0);
        let screens = [left, right];
        assert_eq!(screen_at(&screens, 700.0, 450.0), Some(left));
        assert_eq!(screen_at(&screens, 1440.0, 0.0), Some(right));
        assert_eq!(screen_at(&screens, 3000.0, -150.0), Some(right));
    }

    #[test]
    fn a_pointer_on_no_screen_gets_the_nearest_one() {
        let left = rect(0.0, 0.0, 1440.0, 900.0);
        let right = rect(1440.0, 0.0, 1920.0, 1080.0);
        assert_eq!(screen_at(&[left, right], 2000.0, 1200.0), Some(right));
        assert_eq!(screen_at(&[left, right], -10.0, 5.0), Some(left));
        assert_eq!(screen_at(&[], 0.0, 0.0), None);
    }

    #[test]
    fn macos_always_has_a_compositor() {
        if cfg!(target_os = "macos") {
            assert!(compositor_running());
        }
    }
}
