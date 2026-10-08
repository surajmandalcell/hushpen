//! The virtual key that types `v` on the current keyboard layout.
//!
//! Cmd+V is Cmd plus the key that carries `v`. On QWERTY that is key code 9; on Dvorak and
//! other layouts it is another key, and a fixed 9 would paste with the wrong shortcut.

// UCKeyTranslate and the input source calls are plain C calls on memory this module owns.
#![allow(unsafe_code)]

use core_foundation::base::{CFType, TCFType};
use core_foundation::data::{CFData, CFDataRef};
use std::ffi::c_void;

/// The key code of `v` on an ANSI keyboard, used when the layout cannot be read.
pub(super) const ANSI_V: u16 = 9;

/// Finds the first key code below 128 whose unmodified press types `target`.
pub(super) fn find_key(target: char, mut typed: impl FnMut(u16) -> Option<char>) -> Option<u16> {
    (0..128u16)
        .find(|code| typed(*code).is_some_and(|ch| ch.to_lowercase().eq(target.to_lowercase())))
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentASCIICapableKeyboardLayoutInputSource() -> *const c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    static kTISPropertyUnicodeKeyLayoutData: *const c_void;
    fn UCKeyTranslate(
        layout: *const u8,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
    fn LMGetKbdType() -> u8;
}

const KEY_ACTION_DOWN: u16 = 0;
const NO_DEAD_KEYS: u32 = 1;

/// The key code that types `v` on the layout in use. A layout with no Latin letters (Russian, for
/// example) falls back to the Latin layout the system keeps for shortcuts.
pub(super) fn v_key_code() -> u16 {
    layout_v().unwrap_or(ANSI_V)
}

fn layout_v() -> Option<u16> {
    // SAFETY: the call returns an owned input source (create rule) or null.
    let source = unsafe {
        let raw = TISCopyCurrentASCIICapableKeyboardLayoutInputSource();
        if raw.is_null() {
            return None;
        }
        CFType::wrap_under_create_rule(raw.cast())
    };
    // SAFETY: the source is alive; the data it returns belongs to the source (get rule).
    let data = unsafe {
        let raw = TISGetInputSourceProperty(
            source.as_concrete_TypeRef().cast(),
            kTISPropertyUnicodeKeyLayoutData,
        );
        if raw.is_null() {
            return None;
        }
        CFData::wrap_under_get_rule(raw as CFDataRef)
    };
    let layout = data.bytes();
    // SAFETY: a plain C call with no arguments.
    let keyboard_type = u32::from(unsafe { LMGetKbdType() });
    find_key('v', |code| {
        let mut dead = 0u32;
        let mut length = 0usize;
        let mut buffer = [0u16; 4];
        // SAFETY: `layout` is the `uchr` table of a live input source, and the buffer holds
        // the four code units that the length argument allows.
        let status = unsafe {
            UCKeyTranslate(
                layout.as_ptr(),
                code,
                KEY_ACTION_DOWN,
                0,
                keyboard_type,
                NO_DEAD_KEYS,
                &mut dead,
                buffer.len(),
                &mut length,
                buffer.as_mut_ptr(),
            )
        };
        if status != 0 || length != 1 {
            return None;
        }
        char::from_u32(u32::from(buffer[0]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qwerty(code: u16) -> Option<char> {
        let row = [(0, 'a'), (1, 's'), (2, 'd'), (9, 'v'), (11, 'b'), (12, 'q')];
        row.iter().find(|(key, _)| *key == code).map(|(_, ch)| *ch)
    }

    fn dvorak(code: u16) -> Option<char> {
        let row = [(9, '.'), (11, 'x'), (14, 'p'), (45, 'b'), (40, 'v')];
        row.iter().find(|(key, _)| *key == code).map(|(_, ch)| *ch)
    }

    #[test]
    fn qwerty_types_v_on_key_nine() {
        assert_eq!(find_key('v', qwerty), Some(ANSI_V));
    }

    #[test]
    fn dvorak_types_v_on_another_key() {
        assert_eq!(find_key('v', dvorak), Some(40));
    }

    #[test]
    fn a_layout_without_v_finds_nothing() {
        assert_eq!(find_key('v', |_| Some('x')), None);
    }

    #[test]
    fn the_system_layout_gives_a_key_below_128() {
        assert!(v_key_code() < 128);
    }
}
