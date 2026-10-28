//! The shortcuts macOS itself runs (System Settings > Keyboard > Keyboard Shortcuts), read with
//! `CopySymbolicHotKeys`, so a chord the user turned on there is not taken twice. The window
//! server's own calls (`CGSGetSymbolicHotKeyValue` and friends, which AltTab and similar apps
//! use too) tell which one it is, so the user learns where to turn it off, and turn the app
//! switcher (Cmd+Tab), which has no switch in System Settings, off while Hushpen owns its chord.

// `CopySymbolicHotKeys` hands back an array this module then owns; the `CGS` calls take and
// fill plain integers.
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

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGSGetSymbolicHotKeyValue(
        hotkey: i32,
        key_equivalent: *mut u16,
        virtual_key: *mut u16,
        modifiers: *mut u32,
    ) -> i32;
    fn CGSIsSymbolicHotKeyEnabled(hotkey: i32) -> bool;
    fn CGSSetSymbolicHotKeyEnabled(hotkey: i32, enabled: bool) -> i32;
}

/// The event flag bits the window server keeps a hotkey's modifiers in.
const NX_SHIFT: u32 = 1 << 17;
const NX_CONTROL: u32 = 1 << 18;
const NX_OPTION: u32 = 1 << 19;
const NX_COMMAND: u32 = 1 << 20;
const NX_MODIFIERS: u32 = NX_SHIFT | NX_CONTROL | NX_OPTION | NX_COMMAND;

/// The app switcher: Cmd+Tab and Cmd+Shift+Tab.
pub(super) const APP_SWITCHER: [i32; 2] = [1, 2];

/// The highest hotkey number the search for a chord's owner looks at.
const LAST_HOTKEY: i32 = 300;

/// What System Settings calls a hotkey, and the list of Keyboard Shortcuts it is in.
fn name_of(id: i32) -> Option<(String, &'static str)> {
    let (name, section) = match id {
        7 => ("Move focus to the menu bar", "Keyboard"),
        8 => ("Move focus to the Dock", "Keyboard"),
        9 => ("Move focus to active or next window", "Keyboard"),
        10 => ("Move focus to the window toolbar", "Keyboard"),
        11 => ("Move focus to the floating window", "Keyboard"),
        12 => ("Turn keyboard access on or off", "Keyboard"),
        13 => ("Change the way Tab moves focus", "Keyboard"),
        27 => ("Move focus to next window", "Keyboard"),
        57 => ("Move focus to status menus", "Keyboard"),
        15 => ("Turn zoom on or off", "Accessibility"),
        17 => ("Zoom in", "Accessibility"),
        19 => ("Zoom out", "Accessibility"),
        21 => ("Invert colors", "Accessibility"),
        23 => ("Turn image smoothing on or off", "Accessibility"),
        25 => ("Increase contrast", "Accessibility"),
        26 => ("Decrease contrast", "Accessibility"),
        59 => ("Turn VoiceOver on or off", "Accessibility"),
        162 => ("Show Accessibility controls", "Accessibility"),
        28 => ("Save picture of screen as a file", "Screenshots"),
        29 => ("Copy picture of screen to the clipboard", "Screenshots"),
        30 => ("Save picture of selected area as a file", "Screenshots"),
        31 => (
            "Copy picture of selected area to the clipboard",
            "Screenshots",
        ),
        184 => ("Screenshot and recording options", "Screenshots"),
        32 => ("Mission Control", "Mission Control"),
        33 => ("Application windows", "Mission Control"),
        36 => ("Show Desktop", "Mission Control"),
        79 => ("Move left a space", "Mission Control"),
        81 => ("Move right a space", "Mission Control"),
        163 => ("Show Notification Center", "Mission Control"),
        118..=133 => {
            return Some((format!("Switch to Desktop {}", id - 117), "Mission Control"));
        }
        52 => ("Turn Dock hiding on/off", "Launchpad & Dock"),
        160 => ("Show Launchpad", "Launchpad & Dock"),
        60 => ("Select the previous input source", "Input Sources"),
        61 => ("Select next source in Input menu", "Input Sources"),
        64 => ("Show Spotlight search", "Spotlight"),
        65 => ("Show Finder search window", "Spotlight"),
        98 => ("Show Help menu", "App Shortcuts"),
        _ => return None,
    };
    Some((name.to_owned(), section))
}

/// The key code and modifier bits the window server has for hotkey `id`, when it has one.
fn value_of(id: i32) -> Option<(i64, u32)> {
    let (mut equivalent, mut code, mut modifiers) = (0u16, 0u16, 0u32);
    // SAFETY: the call only writes the three integers it is given.
    let status =
        unsafe { CGSGetSymbolicHotKeyValue(id, &mut equivalent, &mut code, &mut modifiers) };
    (status == 0 && code != u16::MAX).then_some((i64::from(code), modifiers & NX_MODIFIERS))
}

fn enabled(id: i32) -> bool {
    // SAFETY: a plain query by number.
    unsafe { CGSIsSymbolicHotKeyEnabled(id) }
}

/// The name and the Keyboard Shortcuts list of the system shortcut that is on for the chord of
/// key `code` with Carbon `modifiers`, when the system says which one it is.
pub(super) fn owner(code: i64, modifiers: i64) -> Option<(String, &'static str)> {
    let wanted = nx_of_carbon(modifiers);
    (0..=LAST_HOTKEY)
        .filter(|id| value_of(*id) == Some((code, wanted)) && enabled(*id))
        .find_map(name_of)
}

fn nx_of_carbon(modifiers: i64) -> u32 {
    [
        (CMD, NX_COMMAND),
        (SHIFT, NX_SHIFT),
        (OPTION, NX_OPTION),
        (CONTROL, NX_CONTROL),
    ]
    .into_iter()
    .filter(|(carbon, _)| modifiers & carbon != 0)
    .fold(0, |bits, (_, nx)| bits | nx)
}

/// Which app switcher hotkeys a chord of key `code` with Carbon `modifiers` is.
pub(super) fn switcher_hotkeys(code: i64, modifiers: i64) -> impl Iterator<Item = i32> {
    let wanted = nx_of_carbon(modifiers);
    APP_SWITCHER
        .into_iter()
        .filter(move |id| value_of(*id) == Some((code, wanted)))
}

/// Turns a system hotkey on or off for this login session. The window server keeps no record
/// of it past the session.
pub(super) fn set_enabled(id: i32, on: bool) -> bool {
    // SAFETY: a plain call by number.
    unsafe { CGSSetSymbolicHotKeyEnabled(id, on) == 0 }
}

pub(super) fn is_enabled(id: i32) -> bool {
    enabled(id)
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
        Key::Fn | Key::Char(_) | Key::Named(_) | Key::Code(_) => bits,
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

    /// The screenshot and the next-window shortcuts are in the list on every Mac, on or off, in
    /// the same bits.
    #[test]
    fn the_system_lists_the_screenshot_and_next_window_shortcuts() {
        const ANSI_GRAVE: i64 = 50;
        let hotkeys = system_hotkeys();
        let listed = |code: i64, bits: i64| {
            hotkeys
                .iter()
                .any(|hotkey| hotkey.code == code && hotkey.modifiers & MODIFIERS == bits)
        };
        assert!(listed(ANSI_3, CMD | SHIFT), "{hotkeys:?}");
        assert!(listed(ANSI_GRAVE, CMD), "{hotkeys:?}");
    }

    /// The window server keeps the same chords by number, so a conflict can be named. Only the
    /// value is read: whether a user turned one off differs from Mac to Mac.
    #[test]
    fn the_window_server_numbers_the_screenshot_next_window_and_switcher_hotkeys() {
        const ANSI_GRAVE: i64 = 50;
        const TAB: i64 = 48;
        assert_eq!(value_of(28), Some((ANSI_3, NX_SHIFT | NX_COMMAND)));
        assert_eq!(value_of(27), Some((ANSI_GRAVE, NX_COMMAND)));
        assert_eq!(value_of(1), Some((TAB, NX_COMMAND)));
        assert_eq!(switcher_hotkeys(TAB, CMD).collect::<Vec<_>>(), [1]);
        assert_eq!(switcher_hotkeys(TAB, CMD | SHIFT).collect::<Vec<_>>(), [2]);
        assert_eq!(switcher_hotkeys(TAB, CONTROL).count(), 0);
        assert_eq!(
            name_of(28),
            Some(("Save picture of screen as a file".to_owned(), "Screenshots"))
        );
        assert_eq!(
            name_of(121).map(|(name, _)| name).as_deref(),
            Some("Switch to Desktop 4")
        );
    }
}
