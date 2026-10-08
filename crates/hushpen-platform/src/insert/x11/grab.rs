//! Finds out whether another X11 client holds an active keyboard grab.
//!
//! X11 has no secure-field signal. A client that must own the keyboard (a passphrase dialog,
//! a screen locker) takes an active grab, and XTest key events then go to it instead of the
//! focused window. The only way to see a grab is to ask for one: `AlreadyGrabbed` means another
//! client has it, and a grab that succeeded is dropped at once.

use hushpen_core::insert::guard::Probe;
use x11rb::CURRENT_TIME;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt as _, GrabMode, GrabStatus, Window};
use x11rb::rust_connection::RustConnection;

/// Another client holds the grab. A frozen keyboard, a bad time, or a window that is not
/// viewable says nothing about a grabber, so those do not block.
fn held_by_another(status: GrabStatus) -> bool {
    status == GrabStatus::ALREADY_GRABBED
}

/// The X11 answers for the paste guard: the only limit here is a keyboard grab.
pub(super) struct GrabProbe<'a> {
    pub conn: &'a RustConnection,
    pub root: Window,
}

impl Probe for GrabProbe<'_> {
    fn keyboard_grabbed(&self) -> bool {
        keyboard_grabbed(self.conn, self.root)
    }
}

/// Whether a paste chord sent now would go to a grabbing client. A probe that cannot be asked
/// counts as no grab: the paste is not refused on a guess.
fn keyboard_grabbed(conn: &RustConnection, root: Window) -> bool {
    let reply = conn
        .grab_keyboard(false, root, CURRENT_TIME, GrabMode::ASYNC, GrabMode::ASYNC)
        .ok()
        .and_then(|cookie| cookie.reply().ok());
    let Some(reply) = reply else {
        return false;
    };
    if reply.status == GrabStatus::SUCCESS {
        let _ = conn.ungrab_keyboard(CURRENT_TIME);
        let _ = conn.flush();
    }
    held_by_another(reply.status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_already_grabbed_means_another_client_holds_the_keyboard() {
        assert!(held_by_another(GrabStatus::ALREADY_GRABBED));
        assert!(!held_by_another(GrabStatus::SUCCESS));
        assert!(!held_by_another(GrabStatus::FROZEN));
        assert!(!held_by_another(GrabStatus::INVALID_TIME));
        assert!(!held_by_another(GrabStatus::NOT_VIEWABLE));
    }
}
