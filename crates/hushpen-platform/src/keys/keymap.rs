//! The X11 keyboard mapping as a table from key code to physical key.
//!
//! Pure data, so it is tested on every system. The listener reads the server's mapping and
//! calls [`build`]; a key the mapping does not name still resolves when it is a modifier or Esc
//! at its place on an evdev keyboard (key code is the evdev code plus 8).

use hushpen_core::shortcut::{Modifier, Phys, Side};
use std::collections::HashMap;

const KEYSYM_ESCAPE: u32 = 0xff1b;
/// `ISO_Level3_Shift`: AltGr, which many layouts put on the Right Alt key.
const KEYSYM_LEVEL3_SHIFT: u32 = 0xfe03;

const EVDEV_KEYS: [(u32, Phys); 10] = [
    (9, Phys::Esc),
    (37, Phys::Modifier(Modifier::Ctrl, Side::Left)),
    (50, Phys::Modifier(Modifier::Shift, Side::Left)),
    (62, Phys::Modifier(Modifier::Shift, Side::Right)),
    (64, Phys::Modifier(Modifier::Alt, Side::Left)),
    (65, Phys::Char(' ')),
    (105, Phys::Modifier(Modifier::Ctrl, Side::Right)),
    (108, Phys::Modifier(Modifier::Alt, Side::Right)),
    (133, Phys::Modifier(Modifier::Cmd, Side::Left)),
    (134, Phys::Modifier(Modifier::Cmd, Side::Right)),
];

fn modifier_keysym(keysym: u32) -> Option<Phys> {
    let (modifier, side) = match keysym {
        0xffe1 => (Modifier::Shift, Side::Left),
        0xffe2 => (Modifier::Shift, Side::Right),
        0xffe3 => (Modifier::Ctrl, Side::Left),
        0xffe4 => (Modifier::Ctrl, Side::Right),
        0xffe7 | 0xffe9 => (Modifier::Alt, Side::Left),
        0xffe8 | 0xffea | KEYSYM_LEVEL3_SHIFT => (Modifier::Alt, Side::Right),
        0xffeb => (Modifier::Cmd, Side::Left),
        0xffec => (Modifier::Cmd, Side::Right),
        _ => return None,
    };
    Some(Phys::Modifier(modifier, side))
}

fn char_keysym(keysym: u32) -> Option<Phys> {
    let c = char::from_u32(keysym).filter(|_| (0x20..0x7f).contains(&keysym))?;
    (c == ' ' || c.is_ascii_alphanumeric()).then(|| Phys::Char(c.to_ascii_lowercase()))
}

/// `keysyms` is the reply of `GetKeyboardMapping`: `per` keysyms for each key code from
/// `min_keycode` on.
pub(super) fn build(min_keycode: u8, per: usize, keysyms: &[u32]) -> HashMap<u32, Phys> {
    let mut table: HashMap<u32, Phys> = EVDEV_KEYS
        .iter()
        .map(|(code, phys)| (*code, *phys))
        .collect();
    let per = per.max(1);
    for (index, symbols) in keysyms.chunks(per).enumerate() {
        let code = u32::from(min_keycode) + index as u32;
        let found = symbols
            .iter()
            .find_map(|keysym| modifier_keysym(*keysym))
            .or_else(|| symbols.contains(&KEYSYM_ESCAPE).then_some(Phys::Esc))
            // The first shift group names the key on the main layout; a later group only
            // helps when the first has no Latin letter, as on a Cyrillic layout.
            .or_else(|| {
                symbols
                    .iter()
                    .take(2)
                    .find_map(|keysym| char_keysym(*keysym))
            })
            .or_else(|| symbols.iter().find_map(|keysym| char_keysym(*keysym)));
        if let Some(phys) = found {
            table.insert(code, phys);
        }
    }
    table
}

/// The lowest key code that stands for `key`.
pub(super) fn code_of(table: &HashMap<u32, Phys>, key: Phys) -> Option<u8> {
    table
        .iter()
        .filter(|(_, phys)| **phys == key)
        .map(|(code, _)| *code)
        .min()
        .and_then(|code| u8::try_from(code).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const R_ALT: Phys = Phys::Modifier(Modifier::Alt, Side::Right);

    #[test]
    fn the_evdev_keys_resolve_without_a_mapping() {
        let table = build(8, 4, &[]);
        assert_eq!(table[&108], R_ALT);
        assert_eq!(table[&105], Phys::Modifier(Modifier::Ctrl, Side::Right));
        assert_eq!(table[&9], Phys::Esc);
        assert_eq!(table[&65], Phys::Char(' '));
    }

    #[test]
    fn a_letter_key_resolves_by_its_keysym_in_either_case() {
        let mut keysyms = vec![0; 4 * 3];
        keysyms[0] = 0x76;
        keysyms[1] = 0x56;
        keysyms[4] = 0x39;
        keysyms[5] = 0x28;
        keysyms[8] = 0x56;
        let table = build(55, 4, &keysyms);
        assert_eq!(table[&55], Phys::Char('v'));
        assert_eq!(table[&56], Phys::Char('9'));
        assert_eq!(table[&57], Phys::Char('v'));
        assert_eq!(code_of(&table, Phys::Char('v')), Some(55));
        assert_eq!(code_of(&table, Phys::Esc), Some(9));
    }

    #[test]
    fn altgr_on_the_right_alt_key_is_still_right_alt() {
        let table = build(108, 4, &[KEYSYM_LEVEL3_SHIFT, 0, KEYSYM_LEVEL3_SHIFT, 0]);
        assert_eq!(table[&108], R_ALT);
    }

    #[test]
    fn a_layout_without_latin_in_its_first_group_falls_back_to_a_later_group() {
        let table = build(60, 4, &[0x6d0, 0x6b0, 0x76, 0x56]);
        assert_eq!(table[&60], Phys::Char('v'));
    }

    #[test]
    fn symbols_and_function_keys_are_not_shortcut_keys() {
        let table = build(80, 2, &[0x2d, 0x5f, 0xffbe, 0]);
        assert!(!table.contains_key(&80));
        assert!(!table.contains_key(&81));
    }

    #[test]
    fn the_server_mapping_wins_over_the_evdev_place() {
        let table = build(37, 2, &[0xffe4, 0]);
        assert_eq!(table[&37], Phys::Modifier(Modifier::Ctrl, Side::Right));
    }
}
