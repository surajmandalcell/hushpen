//! The paste chord through the XTest extension.

use crate::x11util::keycode_of;
use hushpen_core::insert::Chord;
use hushpen_core::insert::flow::ChordError;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::{ConnectionExt as _, KEY_PRESS_EVENT, KEY_RELEASE_EVENT, Window};
use x11rb::protocol::xtest::{self, ConnectionExt as _};
use x11rb::rust_connection::RustConnection;

const KEYSYM_CONTROL_L: u32 = 0xffe3;
const KEYSYM_SHIFT_L: u32 = 0xffe1;
const KEYSYM_V: u32 = 0x76;
const KEYSYM_INSERT: u32 = 0xff63;

/// The keys of a chord in the order they go down: modifiers first, the letter last.
fn keysyms(chord: Chord) -> Option<&'static [u32]> {
    match chord {
        Chord::CtrlV => Some(&[KEYSYM_CONTROL_L, KEYSYM_V]),
        Chord::CtrlShiftV => Some(&[KEYSYM_CONTROL_L, KEYSYM_SHIFT_L, KEYSYM_V]),
        Chord::ShiftInsert => Some(&[KEYSYM_SHIFT_L, KEYSYM_INSERT]),
        Chord::CmdV => None,
    }
}

fn fake(
    conn: &RustConnection,
    kind: u8,
    keycode: u8,
    root: Window,
) -> Result<(), x11rb::errors::ConnectionError> {
    conn.xtest_fake_input(kind, keycode, 0, root, 0, 0, 0)?;
    Ok(())
}

/// Presses the chord and lifts every key it pressed. A key that still reads as down afterwards
/// gets a second release, so no modifier stays stuck in the focused app.
pub(super) fn send(conn: &RustConnection, root: Window, chord: Chord) -> Result<(), ChordError> {
    let Some(syms) = keysyms(chord) else {
        return Err(ChordError::Unavailable);
    };
    if conn
        .extension_information(xtest::X11_EXTENSION_NAME)
        .ok()
        .flatten()
        .is_none()
    {
        return Err(ChordError::Unavailable);
    }
    let keycodes: Option<Vec<u8>> = syms.iter().map(|sym| keycode_of(conn, *sym)).collect();
    let Some(keycodes) = keycodes else {
        return Err(ChordError::Unavailable);
    };
    let pressed = keycodes
        .iter()
        .try_for_each(|code| fake(conn, KEY_PRESS_EVENT, *code, root));
    // Release even when a press failed halfway, in the reverse order.
    let released = keycodes
        .iter()
        .rev()
        .try_for_each(|code| fake(conn, KEY_RELEASE_EVENT, *code, root));
    if pressed.is_err() || released.is_err() || conn.flush().is_err() {
        return Err(ChordError::Unavailable);
    }
    release_stuck(conn, root, &keycodes);
    Ok(())
}

/// Lifts any key of the chord that the server still reports as down.
fn release_stuck(conn: &RustConnection, root: Window, keycodes: &[u8]) {
    let Ok(Ok(state)) = conn.query_keymap().map(|cookie| cookie.reply()) else {
        return;
    };
    let mut lifted = false;
    for code in keycodes {
        let down = state.keys[usize::from(*code) / 8] & (1 << (code % 8)) != 0;
        if down && fake(conn, KEY_RELEASE_EVENT, *code, root).is_ok() {
            lifted = true;
        }
    }
    if lifted {
        let _ = conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_go_down_before_the_letter() {
        assert_eq!(
            keysyms(Chord::CtrlShiftV),
            Some(&[KEYSYM_CONTROL_L, KEYSYM_SHIFT_L, KEYSYM_V][..])
        );
        assert_eq!(
            keysyms(Chord::ShiftInsert),
            Some(&[KEYSYM_SHIFT_L, KEYSYM_INSERT][..])
        );
        assert_eq!(keysyms(Chord::CmdV), None);
    }
}
