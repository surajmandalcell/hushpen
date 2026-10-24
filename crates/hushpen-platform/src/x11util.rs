//! Small X11 helpers shared by the key listener, text insertion, and the window modules.

use crate::window::ChromeError;
use std::cell::RefCell;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, ConnectionExt as _, Window};
use x11rb::rust_connection::RustConnection;

/// The first keycode that produces `keysym` at any shift level.
pub(crate) fn keycode_of<C: Connection>(conn: &C, keysym: u32) -> Option<u8> {
    let setup = conn.setup();
    let (min, max) = (setup.min_keycode, setup.max_keycode);
    let map = conn
        .get_keyboard_mapping(min, max - min + 1)
        .ok()?
        .reply()
        .ok()?;
    let per = usize::from(map.keysyms_per_keycode).max(1);
    map.keysyms
        .chunks(per)
        .position(|keysyms| keysyms.contains(&keysym))
        .and_then(|index| u8::try_from(index).ok())
        .and_then(|index| min.checked_add(index))
}

/// A protocol failure with the step that was tried.
pub(crate) fn fail(what: &str, error: impl std::fmt::Display) -> ChromeError {
    ChromeError::new(format!("{what}: {error}"))
}

/// Interns an atom, naming it in any error.
pub(crate) fn intern(conn: &RustConnection, name: &[u8]) -> Result<Atom, ChromeError> {
    let text = String::from_utf8_lossy(name);
    Ok(conn
        .intern_atom(false, name)
        .map_err(|e| fail(&format!("intern {text}"), e))?
        .reply()
        .map_err(|e| fail(&format!("intern {text}"), e))?
        .atom)
}

thread_local! {
    /// The main thread's own connection (GPUI owns the one the windows came from). Opened on
    /// first use; a broken one is dropped so the next call reconnects.
    static CONNECTION: RefCell<Option<(RustConnection, usize, Window)>> =
        const { RefCell::new(None) };
}

/// Runs `call` with the main thread's connection, its screen index, and its root window.
pub(crate) fn with_x11<T>(
    call: impl FnOnce(&RustConnection, usize, Window) -> Result<T, ChromeError>,
) -> Result<T, ChromeError> {
    CONNECTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
            let root = conn.setup().roots[screen].root;
            *slot = Some((conn, screen, root));
        }
        let (conn, screen, root) = slot
            .as_ref()
            .ok_or_else(|| ChromeError::new("no x11 connection"))?;
        let result = call(conn, *screen, *root);
        if result.is_err() {
            *slot = None;
        }
        result
    })
}
