//! Small X11 helpers shared by the key listener and text insertion.

use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt as _;

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
