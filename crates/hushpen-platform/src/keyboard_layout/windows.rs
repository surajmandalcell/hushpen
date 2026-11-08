//! Character keys on the foreground app's Windows keyboard layout.

#![allow(unsafe_code)]

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, HKL, MAPVK_VK_TO_VSC, MapVirtualKeyExW, ToUnicodeEx,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

fn current() -> HKL {
    // SAFETY: these calls inspect the foreground thread and return its layout without changing focus.
    unsafe { GetKeyboardLayout(GetWindowThreadProcessId(GetForegroundWindow(), None)) }
}

fn lower_character(buffer: &[u16], length: i32) -> Option<char> {
    let length = usize::try_from(length).ok()?;
    let text = String::from_utf16(buffer.get(..length)?).ok()?;
    let mut chars = text.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_control() || (ch.is_whitespace() && ch != ' ') {
        return None;
    }
    let mut lower = ch.to_lowercase();
    let ch = lower.next()?;
    lower.next().is_none().then_some(ch)
}

fn typed(vk: u32, layout: HKL) -> Option<char> {
    let mut buffer = [0u16; 8];
    // SAFETY: the keyboard state and UTF-16 buffer have the documented lengths. Bit 2 keeps
    // ToUnicodeEx from changing the foreground app's dead-key state.
    let length = unsafe {
        let scan = MapVirtualKeyExW(vk, MAPVK_VK_TO_VSC, Some(layout));
        ToUnicodeEx(vk, scan, &[0; 256], &mut buffer, 4, Some(layout))
    };
    lower_character(&buffer, length)
}

pub(crate) fn character(vk: u32) -> Option<char> {
    typed(vk, current())
}

pub(crate) fn virtual_key_for(ch: char) -> Option<u32> {
    let layout = current();
    (1..=254).find(|vk| typed(*vk, layout) == Some(ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_character_is_lowercase_and_invalid_or_dead_keys_are_ignored() {
        assert_eq!(lower_character(&['V' as u16], 1), Some('v'));
        assert_eq!(lower_character(&[' ' as u16], 1), Some(' '));
        assert_eq!(lower_character(&[0xd83d, 0xde00], 2), Some('😀'));
        assert_eq!(lower_character(&['a' as u16, 'b' as u16], 2), None);
        assert_eq!(lower_character(&['\t' as u16], 1), None);
        assert_eq!(lower_character(&[0xd800], 1), None);
        assert_eq!(lower_character(&['^' as u16], -1), None);
        assert_eq!(lower_character(&[0], 0), None);
    }
}
