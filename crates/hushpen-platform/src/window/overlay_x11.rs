//! The flow bar on X11. GPUI opens a pop-up as an override-redirect window, so the window
//! manager neither moves nor stacks it. The app places it itself and a watcher thread keeps it
//! above every other window.

use super::{ChromeError, Frame, Pointer};
use std::cell::RefCell;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, EventMask, KeyButMask, StackMode,
    Window,
};
use x11rb::rust_connection::RustConnection;

fn fail(what: &str, error: impl std::fmt::Display) -> ChromeError {
    ChromeError::new(format!("{what}: {error}"))
}

thread_local! {
    /// GPUI calls from its main thread, once for each pointer move while the bar is dragged.
    static CONNECTION: RefCell<Option<(RustConnection, Window)>> = const { RefCell::new(None) };
}

/// Runs `call` on the main thread's connection, which it opens on first use. A broken
/// connection is dropped so the next call reconnects.
fn with_connection<T>(
    call: impl FnOnce(&RustConnection, Window) -> Result<T, ChromeError>,
) -> Result<T, ChromeError> {
    CONNECTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
            let root = conn.setup().roots[screen].root;
            *slot = Some((conn, root));
        }
        let (conn, root) = slot
            .as_ref()
            .ok_or_else(|| ChromeError::new("no x11 connection"))?;
        let result = call(conn, *root);
        if result.is_err() {
            *slot = None;
        }
        result
    })
}

pub(super) fn place(window: Window, frame: Frame) -> Result<(), ChromeError> {
    with_connection(|conn, _| {
        conn.configure_window(
            window,
            &ConfigureWindowAux::new()
                .x(frame.x)
                .y(frame.y)
                .width(frame.width)
                .height(frame.height),
        )
        .map_err(|e| fail("move the flow bar", e))?;
        conn.flush().map_err(|e| fail("flush", e))
    })
}

/// Where the pointer is, in root window pixels, and whether the left button is down. The bar
/// reads it while it is dragged: the bar moves under the pointer, so the positions that the
/// window receives lag behind.
pub(super) fn pointer() -> Result<Pointer, ChromeError> {
    with_connection(|conn, root| {
        let reply = conn
            .query_pointer(root)
            .map_err(|e| fail("query the pointer", e))?
            .reply()
            .map_err(|e| fail("query the pointer", e))?;
        Ok(Pointer {
            x: f32::from(reply.root_x),
            y: f32::from(reply.root_y),
            left: reply.mask.contains(KeyButMask::BUTTON1),
        })
    })
}

/// Raises the window now, and again whenever another window is stacked above it.
pub(super) fn pin(window: Window) -> Result<(), ChromeError> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
    let root = conn.setup().roots[screen].root;
    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::SUBSTRUCTURE_NOTIFY),
    )
    .map_err(|e| fail("watch the root window", e))?
    .check()
    .map_err(|e| fail("watch the root window", e))?;
    raise_if_covered(&conn, root, window);
    std::thread::Builder::new()
        .name("x11-flow-bar-top".into())
        .spawn(move || {
            while let Ok(event) = conn.wait_for_event() {
                let other = match event {
                    Event::ConfigureNotify(change) => change.window != window,
                    Event::CirculateNotify(change) => change.window != window,
                    Event::MapNotify(change) => change.window != window,
                    _ => false,
                };
                if other {
                    raise_if_covered(&conn, root, window);
                }
            }
        })
        .map_err(|e| fail("start the stacking watcher", e))?;
    log::info!("flow bar pinned on top on x11 window 0x{window:x}");
    Ok(())
}

fn raise_if_covered(conn: &RustConnection, root: Window, window: Window) {
    let Ok(tree) = conn
        .query_tree(root)
        .map_err(|_| ())
        .and_then(|cookie| cookie.reply().map_err(|_| ()))
    else {
        return;
    };
    if tree.children.last() == Some(&window) {
        return;
    }
    let _ = conn.configure_window(
        window,
        &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
    );
    let _ = conn.flush();
}
