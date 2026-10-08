//! Hide a window to the tray and bring it back on X11.
//!
//! GPUI has no per-window hide, so the app unmaps the window itself. GPUI hears the map and
//! unmap events and stops and restarts drawing on its own.

use super::ChromeError;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConnectionExt, EventMask, UNMAP_NOTIFY_EVENT, UnmapNotifyEvent, Window,
};
use x11rb::rust_connection::RustConnection;

/// The sender is an application, not a pager (EWMH source indication).
const SOURCE_APPLICATION: u32 = 1;

fn fail(what: &str, error: impl std::fmt::Display) -> ChromeError {
    ChromeError::new(format!("{what}: {error}"))
}

fn connect() -> Result<(RustConnection, Window), ChromeError> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
    let root = conn.setup().roots[screen].root;
    Ok((conn, root))
}

/// Withdraws the window. The synthetic `UnmapNotify` to the root is the ICCCM way to tell a
/// window manager that the client withdrew it; the window manager then moves the focus to the
/// window that had it before.
pub(super) fn hide(window: Window) -> Result<(), ChromeError> {
    let (conn, root) = connect()?;
    conn.unmap_window(window)
        .map_err(|e| fail("unmap the window", e))?;
    let notice = UnmapNotifyEvent {
        response_type: UNMAP_NOTIFY_EVENT,
        sequence: 0,
        event: root,
        window,
        from_configure: false,
    };
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        notice,
    )
    .map_err(|e| fail("tell the window manager", e))?;
    conn.flush().map_err(|e| fail("flush", e))?;
    conn.get_input_focus()
        .map_err(|e| fail("sync", e))?
        .reply()
        .map_err(|e| fail("sync", e))?;
    Ok(())
}

/// Maps the window and asks the window manager to focus it.
pub(super) fn show(window: Window) -> Result<(), ChromeError> {
    let (conn, root) = connect()?;
    conn.map_window(window)
        .map_err(|e| fail("map the window", e))?;
    let active = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")
        .map_err(|e| fail("intern _NET_ACTIVE_WINDOW", e))?
        .reply()
        .map_err(|e| fail("intern _NET_ACTIVE_WINDOW", e))?
        .atom;
    let request = ClientMessageEvent::new(32, window, active, [SOURCE_APPLICATION, 0, 0, 0, 0]);
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        request,
    )
    .map_err(|e| fail("ask for the focus", e))?;
    conn.flush().map_err(|e| fail("flush", e))?;
    conn.get_input_focus()
        .map_err(|e| fail("sync", e))?
        .reply()
        .map_err(|e| fail("sync", e))?;
    Ok(())
}
