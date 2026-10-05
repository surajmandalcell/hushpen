use super::{ChromeError, MOTIF_FRAMELESS_FIXED};
use x11rb::connection::Connection;
use x11rb::properties::WmSizeHints;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ClientMessageEvent, ConnectionExt, EventMask,
    PropMode, Window,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

const NET_WM_STATE_REMOVE: u32 = 0;
/// The sender is an application, not a pager (EWMH source indication).
const SOURCE_APPLICATION: u32 = 1;

fn fail(what: &str, error: impl std::fmt::Display) -> ChromeError {
    ChromeError::new(format!("{what}: {error}"))
}

fn intern(conn: &RustConnection, name: &[u8]) -> Result<Atom, ChromeError> {
    let text = String::from_utf8_lossy(name);
    Ok(conn
        .intern_atom(false, name)
        .map_err(|e| fail(&format!("intern {text}"), e))?
        .reply()
        .map_err(|e| fail(&format!("intern {text}"), e))?
        .atom)
}

/// Set min = max = `width` x `height` in `WM_NORMAL_HINTS`, remove the frame
/// and the resize functions through `_MOTIF_WM_HINTS`, and keep the window out
/// of full screen and maximized states. Uses a second connection (GPUI owns
/// the first one).
pub(super) fn lock(window: u32, width: u16, height: u16) -> Result<(), ChromeError> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
    let root = conn.setup().roots[screen].root;

    let size = (i32::from(width), i32::from(height));
    let mut hints = WmSizeHints::new();
    hints.min_size = Some(size);
    hints.max_size = Some(size);
    hints
        .set_normal_hints(&conn, window)
        .map_err(|e| fail("set WM_NORMAL_HINTS", e))?
        .check()
        .map_err(|e| fail("set WM_NORMAL_HINTS", e))?;

    let motif = intern(&conn, b"_MOTIF_WM_HINTS")?;
    conn.change_property32(
        PropMode::REPLACE,
        window,
        motif,
        motif,
        &MOTIF_FRAMELESS_FIXED,
    )
    .map_err(|e| fail("set _MOTIF_WM_HINTS", e))?
    .check()
    .map_err(|e| fail("set _MOTIF_WM_HINTS", e))?;

    // The window manager may still grant a full screen request even though the
    // size hints pin the size, so undo it as soon as the state appears.
    let guard = StateGuard {
        window,
        root,
        state: intern(&conn, b"_NET_WM_STATE")?,
        forbidden: [
            intern(&conn, b"_NET_WM_STATE_FULLSCREEN")?,
            intern(&conn, b"_NET_WM_STATE_MAXIMIZED_VERT")?,
            intern(&conn, b"_NET_WM_STATE_MAXIMIZED_HORZ")?,
        ],
    };
    conn.change_window_attributes(
        window,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(|e| fail("watch window state", e))?
    .check()
    .map_err(|e| fail("watch window state", e))?;
    conn.flush().map_err(|e| fail("flush", e))?;

    std::thread::Builder::new()
        .name("x11-chrome-guard".into())
        .spawn(move || guard.run(&conn))
        .map_err(|e| fail("start the state guard", e))?;
    log::info!("window pinned to {width}x{height} on x11 window 0x{window:x}");
    Ok(())
}

struct StateGuard {
    window: Window,
    root: Window,
    state: Atom,
    forbidden: [Atom; 3],
}

impl StateGuard {
    fn run(&self, conn: &RustConnection) {
        // A state set before the watch began would otherwise go unnoticed.
        self.strip(conn);
        while let Ok(event) = conn.wait_for_event() {
            if let Event::PropertyNotify(change) = event
                && change.window == self.window
                && change.atom == self.state
            {
                self.strip(conn);
            }
        }
    }

    fn strip(&self, conn: &RustConnection) {
        let Ok(reply) = conn
            .get_property(false, self.window, self.state, AtomEnum::ATOM, 0, 32)
            .map_err(|_| ())
            .and_then(|cookie| cookie.reply().map_err(|_| ()))
        else {
            return;
        };
        let Some(current) = reply.value32() else {
            return;
        };
        let current: Vec<u32> = current.collect();
        for atom in self.forbidden.iter().filter(|atom| current.contains(atom)) {
            let message = ClientMessageEvent::new(
                32,
                self.window,
                self.state,
                [NET_WM_STATE_REMOVE, *atom, 0, SOURCE_APPLICATION, 0],
            );
            let _ = conn.send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                message,
            );
        }
        let _ = conn.flush();
    }
}
