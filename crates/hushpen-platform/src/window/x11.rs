use super::{ChromeError, MOTIF_FRAMELESS_FIXED, motif_hints_locked, size_hints_pinned};
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
///
/// GPUI writes its own `WM_NORMAL_HINTS` (maximum 16384) and `_MOTIF_WM_HINTS`
/// while it sets the window up, and which write comes last is a race with this
/// call. A guard thread therefore watches both properties and writes ours back
/// whenever they stop matching, so the window manager always ends on them.
pub(super) fn lock(window: u32, width: u16, height: u16) -> Result<(), ChromeError> {
    let (conn, screen) = x11rb::connect(None).map_err(|e| fail("x11 connect", e))?;
    let root = conn.setup().roots[screen].root;

    // The window manager may still grant a full screen request even though the
    // size hints pin the size, so undo it as soon as the state appears.
    let guard = Guard {
        window,
        root,
        width,
        height,
        motif: intern(&conn, b"_MOTIF_WM_HINTS")?,
        state: intern(&conn, b"_NET_WM_STATE")?,
        forbidden: [
            intern(&conn, b"_NET_WM_STATE_FULLSCREEN")?,
            intern(&conn, b"_NET_WM_STATE_MAXIMIZED_VERT")?,
            intern(&conn, b"_NET_WM_STATE_MAXIMIZED_HORZ")?,
        ],
    };
    guard.restore_hints(&conn)?;
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

struct Guard {
    window: Window,
    root: Window,
    width: u16,
    height: u16,
    motif: Atom,
    state: Atom,
    forbidden: [Atom; 3],
}

impl Guard {
    fn run(&self, conn: &RustConnection) {
        // A write or a state set before the watch began would otherwise go unnoticed.
        self.reconcile(conn);
        while let Ok(event) = conn.wait_for_event() {
            if let Event::PropertyNotify(change) = event
                && change.window == self.window
                && [self.state, self.motif, AtomEnum::WM_NORMAL_HINTS.into()].contains(&change.atom)
            {
                self.reconcile(conn);
            }
        }
    }

    fn reconcile(&self, conn: &RustConnection) {
        if let Err(error) = self.restore_hints(conn) {
            log::warn!("could not restore the window hints: {error}");
        }
        self.strip(conn);
    }

    /// Writes a hint back only when it no longer matches, so the notify for
    /// our own write finds nothing to do.
    fn restore_hints(&self, conn: &RustConnection) -> Result<(), ChromeError> {
        let normal = WmSizeHints::get_normal_hints(conn, self.window)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .flatten();
        let pinned = normal.is_some_and(|hints| {
            size_hints_pinned(hints.min_size, hints.max_size, self.width, self.height)
        });
        if !pinned {
            let size = (i32::from(self.width), i32::from(self.height));
            let mut hints = WmSizeHints::new();
            hints.min_size = Some(size);
            hints.max_size = Some(size);
            hints
                .set_normal_hints(conn, self.window)
                .map_err(|e| fail("set WM_NORMAL_HINTS", e))?
                .check()
                .map_err(|e| fail("set WM_NORMAL_HINTS", e))?;
            log::debug!("WM_NORMAL_HINTS written back on 0x{:x}", self.window);
        }

        let motif = conn
            .get_property(false, self.window, self.motif, self.motif, 0, 5)
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().map(Iterator::collect::<Vec<u32>>));
        if !motif_hints_locked(motif.as_deref()) {
            conn.change_property32(
                PropMode::REPLACE,
                self.window,
                self.motif,
                self.motif,
                &MOTIF_FRAMELESS_FIXED,
            )
            .map_err(|e| fail("set _MOTIF_WM_HINTS", e))?
            .check()
            .map_err(|e| fail("set _MOTIF_WM_HINTS", e))?;
            log::debug!("_MOTIF_WM_HINTS written back on 0x{:x}", self.window);
        }
        Ok(())
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
