//! The shortcuts macOS itself runs (System Settings > Keyboard > Keyboard Shortcuts), read with
//! `CopySymbolicHotKeys`, so a chord the user turned on there is not taken twice.

// `CopySymbolicHotKeys` is a plain C call that hands back an array this module then owns.
#![allow(unsafe_code)]

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use hushpen_core::shortcut::{Combo, Key, Modifier};

/// The Carbon modifier bits (`cmdKey`, `shiftKey`, `optionKey`, `controlKey`). Other bits, such
/// as the keypad and Fn flags, do not change which chord a hotkey is.
const CMD: i64 = 1 << 8;
const SHIFT: i64 = 1 << 9;
const OPTION: i64 = 1 << 11;
const CONTROL: i64 = 1 << 12;
const MODIFIERS: i64 = CMD | SHIFT | OPTION | CONTROL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Hotkey {
    pub code: i64,
    pub modifiers: i64,
    pub enabled: bool,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn CopySymbolicHotKeys(out: *mut CFArrayRef) -> i32;
}

/// Every system shortcut, on or off. Empty when the system does not answer.
pub(super) fn system_hotkeys() -> Vec<Hotkey> {
    let mut raw: CFArrayRef = std::ptr::null();
    // SAFETY: the call writes an owned array (copy rule) to `raw`, or fails and leaves it null.
    let status = unsafe { CopySymbolicHotKeys(&mut raw) };
    if status != 0 || raw.is_null() {
        return Vec::new();
    }
    // SAFETY: `raw` is the owned, non-null array from above; each item is a dictionary.
    let array: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { CFArray::wrap_under_create_rule(raw) };
    let code = CFString::from_static_string("kHISymbolicHotKeyCode");
    let modifiers = CFString::from_static_string("kHISymbolicHotKeyModifiers");
    let enabled = CFString::from_static_string("kHISymbolicHotKeyEnabled");
    array
        .iter()
        .filter_map(|entry| {
            let number = |key: &CFString| {
                entry
                    .find(key)
                    .and_then(|value| value.downcast::<CFNumber>())
                    .and_then(|number| number.to_i64())
            };
            Some(Hotkey {
                code: number(&code)?,
                modifiers: number(&modifiers)?,
                enabled: entry
                    .find(&enabled)
                    .and_then(|value| value.downcast::<CFBoolean>())
                    .is_some_and(bool::from),
            })
        })
        .collect()
}

/// The Carbon modifier bits of `combo`.
pub(super) fn carbon_modifiers(combo: &Combo) -> i64 {
    combo.keys().iter().fold(0, |bits, key| match key {
        Key::Any(modifier) | Key::Right(modifier) => {
            bits | match modifier {
                Modifier::Cmd => CMD,
                Modifier::Shift => SHIFT,
                Modifier::Alt => OPTION,
                Modifier::Ctrl => CONTROL,
            }
        }
        Key::Fn | Key::Char(_) => bits,
    })
}

/// Whether a system shortcut that is on is the chord of key `code` with `modifiers`.
pub(super) fn taken(hotkeys: &[Hotkey], code: i64, modifiers: i64) -> bool {
    hotkeys.iter().any(|hotkey| {
        hotkey.enabled && hotkey.code == code && hotkey.modifiers & MODIFIERS == modifiers
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::shortcut::Platform;

    const ANSI_3: i64 = 20;

    #[test]
    fn a_chord_matches_a_system_shortcut_only_when_that_shortcut_is_on() {
        let combo = Combo::parse("Cmd+Shift+3", Platform::MacOs).unwrap();
        let bits = carbon_modifiers(&combo);
        assert_eq!(bits, CMD | SHIFT);
        let on = Hotkey {
            code: ANSI_3,
            modifiers: CMD | SHIFT,
            enabled: true,
        };
        assert!(taken(&[on], ANSI_3, bits));
        assert!(!taken(
            &[Hotkey {
                enabled: false,
                ..on
            }],
            ANSI_3,
            bits
        ));
        assert!(!taken(&[on], ANSI_3, CMD | SHIFT | OPTION));
        // The Fn and keypad flags of an entry do not make it another chord.
        let with_fn = Hotkey {
            modifiers: CMD | SHIFT | (1 << 23),
            ..on
        };
        assert!(taken(&[with_fn], ANSI_3, bits));
    }

    /// The screenshot shortcut is in the list on every Mac, on or off, in the same bits.
    #[test]
    fn the_system_lists_the_screenshot_shortcut() {
        let hotkeys = system_hotkeys();
        assert!(
            hotkeys
                .iter()
                .any(|hotkey| hotkey.code == ANSI_3 && hotkey.modifiers & MODIFIERS == CMD | SHIFT),
            "{hotkeys:?}"
        );
    }
}
