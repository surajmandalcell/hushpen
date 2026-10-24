//! The compositing manager on X11, and the KDE blur property.
//!
//! A running compositor owns the `_NET_WM_CM_S<screen>` selection (the compositing manager
//! conventions), so one selection-owner query says whether a transparent window shows what is
//! behind it or renders black. KWin blurs what shows through the rectangles listed in
//! `_KDE_NET_WM_BLUR_BEHIND_REGION` (the property Konsole sets); picom blurs ARGB windows from
//! its own configuration and needs no property.

use super::{ChromeError, Frame};
use crate::x11util::{fail, intern, with_x11};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, PropMode, Window};
use x11rb::wrapper::ConnectionExt as _;

/// The selection a compositor owns while it runs: `_NET_WM_CM_S0` on the first screen.
fn selection_name(screen: usize) -> String {
    format!("_NET_WM_CM_S{screen}")
}

/// Whether the selection has an owner.
fn composited(owner: Window) -> bool {
    owner != x11rb::NONE
}

/// Whether a compositing manager runs on the connection's screen. A connection or protocol
/// error reads as "no compositor": the window then keeps its solid, opaque look.
pub(super) fn running() -> bool {
    with_x11(|conn, screen, _| {
        let name = selection_name(screen);
        let selection = intern(conn, name.as_bytes())?;
        let owner = conn
            .get_selection_owner(selection)
            .map_err(|e| fail(&format!("read {name}"), e))?
            .reply()
            .map_err(|e| fail(&format!("read {name}"), e))?
            .owner;
        Ok(composited(owner))
    })
    .unwrap_or(false)
}

const BLUR_PROPERTY: &[u8] = b"_KDE_NET_WM_BLUR_BEHIND_REGION";

/// The property value for one rectangle: x, y, width, height in device pixels. An empty list
/// would blur the whole window, so the region is always explicit.
fn blur_region(region: Frame) -> [u32; 4] {
    [
        region.x.max(0) as u32,
        region.y.max(0) as u32,
        region.width,
        region.height,
    ]
}

/// Asks KWin to blur what shows through `region` of the window.
pub(super) fn blur_behind(window: Window, region: Frame) -> Result<(), ChromeError> {
    let values = blur_region(region);
    with_x11(|conn, _, _| {
        let property = intern(conn, BLUR_PROPERTY)?;
        conn.change_property32(
            PropMode::REPLACE,
            window,
            property,
            AtomEnum::CARDINAL,
            &values,
        )
        .map_err(|e| fail("set the blur region", e))?
        .check()
        .map_err(|e| fail("set the blur region", e))?;
        conn.flush().map_err(|e| fail("flush", e))
    })
}

/// Takes the blur request back, for a compositor that went away.
pub(super) fn clear_blur_behind(window: Window) -> Result<(), ChromeError> {
    with_x11(|conn, _, _| {
        let property = intern(conn, BLUR_PROPERTY)?;
        conn.delete_property(window, property)
            .map_err(|e| fail("clear the blur region", e))?
            .check()
            .map_err(|e| fail("clear the blur region", e))?;
        conn.flush().map_err(|e| fail("flush", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compositor_selection_names_its_screen() {
        assert_eq!(selection_name(0), "_NET_WM_CM_S0");
        assert_eq!(selection_name(1), "_NET_WM_CM_S1");
    }

    #[test]
    fn only_an_owned_selection_means_a_compositor() {
        assert!(composited(0x200001));
        assert!(!composited(x11rb::NONE));
    }

    #[test]
    fn the_blur_region_is_x_y_width_height_in_device_pixels() {
        let sidebar = Frame {
            x: 0,
            y: 0,
            width: 232,
            height: 600,
        };
        assert_eq!(blur_region(sidebar), [0, 0, 232, 600]);
        let off_screen = Frame {
            x: -4,
            y: -2,
            width: 10,
            height: 10,
        };
        assert_eq!(blur_region(off_screen), [0, 0, 10, 10]);
    }
}
