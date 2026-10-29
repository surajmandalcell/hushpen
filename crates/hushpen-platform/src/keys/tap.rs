//! Decoding of macOS event tap events, and the health rules for the tap itself.
//!
//! Pure logic with plain integers, so it is tested with synthetic events. The `CGEventTap`
//! glue in `macos.rs` only converts real events into [`TapEvent`] and runs what this module
//! decides. The key codes and flag bits are the macOS values (`kVK_RightOption`,
//! `kCGEventFlagMaskAlternate`, and so on), so this module compiles everywhere.

use hushpen_core::shortcut::{Modifier, Named, Phys, Side};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

pub const KEYCODE_ESCAPE: i64 = 53;
const KEYCODE_FN: i64 = 63;
const KEYCODE_CAPS_LOCK: i64 = 57;
pub const KEYCODE_SPACE: i64 = 49;

/// A media key has no key code; it is `MEDIA_CODE` plus its `NX_KEYTYPE` number.
pub const MEDIA_CODE: u32 = 0x1_0000;

/// The keys that type no character, by their fixed key code (`kVK_Tab`, `kVK_F1`,
/// `kVK_ANSI_Keypad1`, and so on).
const NAMED_KEYS: [(i64, Named); 52] = [
    (48, Named::Tab),
    (36, Named::Return),
    (76, Named::KeypadEnter),
    (71, Named::KeypadClear),
    (65, Named::Keypad('.')),
    (67, Named::Keypad('*')),
    (69, Named::Keypad('+')),
    (75, Named::Keypad('/')),
    (78, Named::Keypad('-')),
    (81, Named::Keypad('=')),
    (82, Named::Keypad('0')),
    (83, Named::Keypad('1')),
    (84, Named::Keypad('2')),
    (85, Named::Keypad('3')),
    (86, Named::Keypad('4')),
    (87, Named::Keypad('5')),
    (88, Named::Keypad('6')),
    (89, Named::Keypad('7')),
    (91, Named::Keypad('8')),
    (92, Named::Keypad('9')),
    (114, Named::Help),
    (110, Named::Menu),
    (51, Named::Backspace),
    (117, Named::Delete),
    (126, Named::Up),
    (125, Named::Down),
    (123, Named::Left),
    (124, Named::Right),
    (115, Named::Home),
    (119, Named::End),
    (116, Named::PageUp),
    (121, Named::PageDown),
    (122, Named::F(1)),
    (120, Named::F(2)),
    (99, Named::F(3)),
    (118, Named::F(4)),
    (96, Named::F(5)),
    (97, Named::F(6)),
    (98, Named::F(7)),
    (100, Named::F(8)),
    (101, Named::F(9)),
    (109, Named::F(10)),
    (103, Named::F(11)),
    (111, Named::F(12)),
    (105, Named::F(13)),
    (107, Named::F(14)),
    (113, Named::F(15)),
    (106, Named::F(16)),
    (64, Named::F(17)),
    (79, Named::F(18)),
    (80, Named::F(19)),
    (90, Named::F(20)),
];

/// The key code of a named key, when it has one. Media keys have none.
pub fn named_keycode(named: Named) -> Option<i64> {
    match named {
        Named::Escape => return Some(KEYCODE_ESCAPE),
        Named::CapsLock => return Some(KEYCODE_CAPS_LOCK),
        _ => {}
    }
    NAMED_KEYS
        .iter()
        .find(|(_, each)| *each == named)
        .map(|(code, _)| *code)
}

/// The media keys by `NX_KEYTYPE` number (`NX_KEYTYPE_SOUND_UP` is 0, and so on).
const MEDIA_KEYS: [(u32, Named); 13] = [
    (0, Named::VolumeUp),
    (1, Named::VolumeDown),
    (2, Named::BrightnessUp),
    (3, Named::BrightnessDown),
    (7, Named::Mute),
    (14, Named::Eject),
    (16, Named::PlayPause),
    (17, Named::NextTrack),
    (18, Named::PreviousTrack),
    (19, Named::FastForward),
    (20, Named::Rewind),
    (21, Named::KeyboardLightUp),
    (22, Named::KeyboardLightDown),
];

/// The physical key of a media key event: `NX_SUBTYPE_AUX_CONTROL_BUTTONS` (8) with the key in
/// the high half of `data1` and its state (`0xA` down, `0xB` up) in the byte below.
pub fn media_key(subtype: i64, data1: i64) -> Option<(Phys, bool)> {
    const AUX_CONTROL_BUTTONS: i64 = 8;
    if subtype != AUX_CONTROL_BUTTONS {
        return None;
    }
    let code = u32::try_from((data1 >> 16) & 0xFFFF).ok()?;
    let down = match (data1 >> 8) & 0xFF {
        0xA => true,
        0xB => false,
        _ => return None,
    };
    let phys = MEDIA_KEYS
        .iter()
        .find(|(each, _)| *each == code)
        .map(|(_, named)| Phys::Named(*named))
        .unwrap_or(Phys::Code(MEDIA_CODE + code));
    Some((phys, down))
}

/// The words for a Mac key that has no name in the shortcut model.
pub fn code_name(code: u32) -> Option<String> {
    let name = match code {
        10 => "§",
        93 => "¥",
        94 => "JIS _",
        95 => "Keypad ,",
        102 => "英数",
        104 => "かな",
        131 => "Launchpad",
        160 => "Mission Control",
        code if code >= MEDIA_CODE => return Some(format!("Media key {}", code - MEDIA_CODE)),
        _ => return None,
    };
    Some(name.to_owned())
}

fn named_of(keycode: i64) -> Option<Named> {
    NAMED_KEYS
        .iter()
        .find(|(code, _)| *code == keycode)
        .map(|(_, named)| *named)
}

pub const FLAG_ALPHA_SHIFT: u64 = 0x0001_0000;
pub const FLAG_ALTERNATE: u64 = 0x0008_0000;
pub const FLAG_SECONDARY_FN: u64 = 0x0080_0000;
pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;

/// The bits that say which physical key holds a modifier. Synthetic events may lack them.
const DEVICE_BITS: u64 = 0x0000_207F;

/// How often the tap thread checks that the tap still lives.
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(5);

/// A modifier key on the Mac keyboard: its key code, its flag in the event, its own bit, and
/// the key it stands for.
struct ModifierKey {
    keycode: i64,
    flag: u64,
    device_bit: u64,
    phys: Phys,
}

const fn modifier(
    keycode: i64,
    flag: u64,
    device_bit: u64,
    m: Modifier,
    side: Side,
) -> ModifierKey {
    ModifierKey {
        keycode,
        flag,
        device_bit,
        phys: Phys::Modifier(m, side),
    }
}

const MODIFIER_KEYS: [ModifierKey; 8] = [
    modifier(59, FLAG_CONTROL, 0x0001, Modifier::Ctrl, Side::Left),
    modifier(62, FLAG_CONTROL, 0x2000, Modifier::Ctrl, Side::Right),
    modifier(58, FLAG_ALTERNATE, 0x0020, Modifier::Alt, Side::Left),
    modifier(61, FLAG_ALTERNATE, 0x0040, Modifier::Alt, Side::Right),
    modifier(56, FLAG_SHIFT, 0x0002, Modifier::Shift, Side::Left),
    modifier(60, FLAG_SHIFT, 0x0004, Modifier::Shift, Side::Right),
    modifier(55, FLAG_COMMAND, 0x0008, Modifier::Cmd, Side::Left),
    modifier(54, FLAG_COMMAND, 0x0010, Modifier::Cmd, Side::Right),
];

/// What the tap callback saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapEvent {
    FlagsChanged {
        keycode: i64,
        flags: u64,
    },
    KeyDown {
        keycode: i64,
        flags: u64,
    },
    KeyUp {
        keycode: i64,
        flags: u64,
    },
    /// A media key (`NX_SYSDEFINED`), with the event's subtype and `data1`.
    Media {
        subtype: i64,
        data1: i64,
    },
    /// `kCGEventTapDisabledByTimeout`: the system turned the tap off because the callback was slow.
    DisabledByTimeout,
    /// `kCGEventTapDisabledByUserInput`.
    DisabledByUserInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapAction {
    /// A key went down (`true`) or up (`false`).
    Key(Phys, bool),
    /// A key that macOS reports once, on its press: Caps Lock. It goes down and up at once.
    Tap(Phys),
    /// Turn the tap on again.
    Reenable,
    Ignore,
}

/// Turns tap events into key presses and releases.
#[derive(Debug)]
pub struct Decoder {
    /// The character keys on the current layout, by key code.
    chars: HashMap<i64, char>,
    /// The modifiers that are down, so a flags event is a change or nothing.
    down: BTreeSet<Phys>,
    /// The Caps Lock state of the last Caps Lock event, so a press counts once.
    caps: Option<bool>,
}

impl Decoder {
    pub fn new(chars: HashMap<i64, char>) -> Self {
        Self {
            chars,
            down: BTreeSet::new(),
            caps: None,
        }
    }

    pub fn decode(&mut self, event: TapEvent) -> TapAction {
        match event {
            TapEvent::FlagsChanged { keycode, flags } => self.flags_changed(keycode, flags),
            TapEvent::KeyDown { keycode, .. } => self.key(keycode, true),
            TapEvent::KeyUp { keycode, .. } => self.key(keycode, false),
            TapEvent::Media { subtype, data1 } => match media_key(subtype, data1) {
                Some((phys, down)) => TapAction::Key(phys, down),
                None => TapAction::Ignore,
            },
            TapEvent::DisabledByTimeout | TapEvent::DisabledByUserInput => TapAction::Reenable,
        }
    }

    fn key(&self, keycode: i64, down: bool) -> TapAction {
        if keycode == KEYCODE_ESCAPE {
            return TapAction::Key(Phys::Esc, down);
        }
        if let Some(named) = named_of(keycode) {
            return TapAction::Key(Phys::Named(named), down);
        }
        match self.chars.get(&keycode) {
            Some(c) => TapAction::Key(Phys::Char(*c), down),
            None => match u32::try_from(keycode) {
                Ok(code) if code < MEDIA_CODE => TapAction::Key(Phys::Code(code), down),
                _ => TapAction::Ignore,
            },
        }
    }

    fn flags_changed(&mut self, keycode: i64, flags: u64) -> TapAction {
        if keycode == KEYCODE_CAPS_LOCK {
            let on = flags & FLAG_ALPHA_SHIFT != 0;
            if self.caps.replace(on) == Some(on) {
                return TapAction::Ignore;
            }
            return TapAction::Tap(Phys::Named(Named::CapsLock));
        }
        let (phys, pressed) = if keycode == KEYCODE_FN {
            (Phys::Fn, flags & FLAG_SECONDARY_FN != 0)
        } else if let Some(key) = MODIFIER_KEYS.iter().find(|key| key.keycode == keycode) {
            // Both Options share one flag, so the key's own bit says which one is down. A
            // synthetic event with no device bits has only the shared flag.
            let pressed = if flags & DEVICE_BITS != 0 {
                flags & key.device_bit != 0
            } else {
                flags & key.flag != 0
            };
            (key.phys, pressed)
        } else {
            return TapAction::Ignore;
        };
        let changed = if pressed {
            self.down.insert(phys)
        } else {
            self.down.remove(&phys)
        };
        if changed {
            TapAction::Key(phys, pressed)
        } else {
            TapAction::Ignore
        }
    }

    /// The user switched layouts, so the same letter sits on another key.
    pub fn set_chars(&mut self, chars: HashMap<i64, char>) {
        self.chars = chars;
    }

    /// A reinstalled tap cannot see the release of a key that went down before it.
    pub fn reset(&mut self) {
        self.down.clear();
    }
}

/// The part of a live tap that the health rules need.
pub trait Tap {
    /// The tap's port is still valid, so the system can still deliver events to it.
    fn alive(&self) -> bool;
    fn enable(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Fine,
    /// A new tap replaced the dead one.
    Reinstalled,
    /// The tap is dead and could not be installed again; the next check tries again.
    Dead,
}

/// Owns the tap and decides when to re-enable or replace it.
pub struct TapGuard<T: Tap> {
    tap: T,
    last_check: Instant,
}

impl<T: Tap> TapGuard<T> {
    pub fn new(tap: T, now: Instant) -> Self {
        Self {
            tap,
            last_check: now,
        }
    }

    pub fn reenable(&self) {
        self.tap.enable();
    }

    pub fn tap(&self) -> &T {
        &self.tap
    }

    /// Puts a better tap in place of a live one, such as a filtering tap once it is allowed.
    pub fn replace(&mut self, tap: T) {
        self.tap = tap;
    }

    /// Once per [`HEALTH_INTERVAL`]: a tap that is no longer alive is replaced by `install`.
    /// Returns `None` between checks.
    pub fn health_check(
        &mut self,
        now: Instant,
        install: impl FnOnce() -> Option<T>,
    ) -> Option<Health> {
        if now.saturating_duration_since(self.last_check) < HEALTH_INTERVAL {
            return None;
        }
        self.last_check = now;
        if self.tap.alive() {
            return Some(Health::Fine);
        }
        Some(match install() {
            Some(fresh) => {
                self.tap = fresh;
                Health::Reinstalled
            }
            None => Health::Dead,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::dictation::AppEvent;
    use hushpen_core::shortcut::{Bindings, Combo, Engine, HandsFree, Output, Platform};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    const KEYCODE_RIGHT_OPTION: i64 = 61;
    const KEYCODE_V: i64 = 9;
    const CTRL_CMD: u64 = FLAG_CONTROL | FLAG_COMMAND;

    fn decoder() -> Decoder {
        Decoder::new(HashMap::from([(KEYCODE_V, 'v'), (49, ' ')]))
    }

    fn flags(keycode: i64, flags: u64) -> TapEvent {
        TapEvent::FlagsChanged { keycode, flags }
    }

    fn key_down(keycode: i64) -> TapEvent {
        TapEvent::KeyDown { keycode, flags: 0 }
    }

    fn key_up(keycode: i64) -> TapEvent {
        TapEvent::KeyUp { keycode, flags: 0 }
    }

    const R_OPTION: Phys = Phys::Modifier(Modifier::Alt, Side::Right);

    #[test]
    fn right_option_with_the_alternate_flag_is_down_and_clearing_it_is_up() {
        let mut decoder = decoder();
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE)),
            TapAction::Key(R_OPTION, true)
        );
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, 0)),
            TapAction::Key(R_OPTION, false)
        );
    }

    #[test]
    fn a_repeated_flags_event_is_not_a_second_change() {
        let mut decoder = decoder();
        decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE));
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE)),
            TapAction::Ignore
        );
        decoder.decode(flags(KEYCODE_RIGHT_OPTION, 0));
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, 0)),
            TapAction::Ignore
        );
    }

    #[test]
    fn the_device_bit_tells_which_option_key_is_still_down() {
        let mut decoder = decoder();
        let both = FLAG_ALTERNATE | 0x0020 | 0x0040;
        decoder.decode(flags(58, FLAG_ALTERNATE | 0x0020));
        decoder.decode(flags(KEYCODE_RIGHT_OPTION, both));
        // Right Option comes up while Left Option stays down: the shared flag is still set.
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE | 0x0020)),
            TapAction::Key(R_OPTION, false)
        );
    }

    #[test]
    fn left_option_is_the_left_alt_key_and_not_the_right() {
        let mut decoder = decoder();
        assert_eq!(
            decoder.decode(flags(58, FLAG_ALTERNATE)),
            TapAction::Key(Phys::Modifier(Modifier::Alt, Side::Left), true)
        );
    }

    #[test]
    fn right_control_shift_and_command_have_their_own_key_codes() {
        let mut decoder = decoder();
        for (keycode, flag, expected) in [
            (
                62,
                FLAG_CONTROL,
                Phys::Modifier(Modifier::Ctrl, Side::Right),
            ),
            (60, FLAG_SHIFT, Phys::Modifier(Modifier::Shift, Side::Right)),
            (54, FLAG_COMMAND, Phys::Modifier(Modifier::Cmd, Side::Right)),
            (55, FLAG_COMMAND, Phys::Modifier(Modifier::Cmd, Side::Left)),
        ] {
            assert_eq!(
                decoder.decode(flags(keycode, flag)),
                TapAction::Key(expected, true)
            );
        }
    }

    #[test]
    fn fn_is_key_code_63_with_the_secondary_fn_flag() {
        let mut decoder = decoder();
        assert_eq!(
            decoder.decode(flags(KEYCODE_FN, FLAG_SECONDARY_FN)),
            TapAction::Key(Phys::Fn, true)
        );
        assert_eq!(
            decoder.decode(flags(KEYCODE_FN, 0)),
            TapAction::Key(Phys::Fn, false)
        );
    }

    #[test]
    fn letters_come_from_the_layout_table_and_escape_is_key_code_53() {
        let mut decoder = decoder();
        assert_eq!(
            decoder.decode(key_down(KEYCODE_V)),
            TapAction::Key(Phys::Char('v'), true)
        );
        assert_eq!(
            decoder.decode(key_up(KEYCODE_V)),
            TapAction::Key(Phys::Char('v'), false)
        );
        assert_eq!(
            decoder.decode(key_down(KEYCODE_ESCAPE)),
            TapAction::Key(Phys::Esc, true)
        );
        assert_eq!(
            decoder.decode(key_down(0)),
            TapAction::Key(Phys::Code(0), true),
            "a key the layout does not type is still a key"
        );
        assert_eq!(
            decoder.decode(flags(57, FLAG_ALPHA_SHIFT)),
            TapAction::Tap(Phys::Named(Named::CapsLock))
        );
        assert_eq!(
            decoder.decode(flags(57, FLAG_ALPHA_SHIFT)),
            TapAction::Ignore,
            "the same Caps Lock state again is not a second press"
        );
        assert_eq!(
            decoder.decode(flags(57, 0)),
            TapAction::Tap(Phys::Named(Named::CapsLock))
        );
    }

    #[test]
    fn media_keys_come_from_the_system_defined_event() {
        let mut decoder = decoder();
        let event = |code: i64, state: i64| TapEvent::Media {
            subtype: 8,
            data1: (code << 16) | (state << 8),
        };
        assert_eq!(
            decoder.decode(event(16, 0xA)),
            TapAction::Key(Phys::Named(Named::PlayPause), true)
        );
        assert_eq!(
            decoder.decode(event(0, 0xB)),
            TapAction::Key(Phys::Named(Named::VolumeUp), false)
        );
        assert_eq!(
            decoder.decode(event(6, 0xA)),
            TapAction::Key(Phys::Code(MEDIA_CODE + 6), true)
        );
        assert_eq!(
            decoder.decode(TapEvent::Media {
                subtype: 7,
                data1: 16 << 16 | 0xA << 8
            }),
            TapAction::Ignore
        );
        assert_eq!(code_name(MEDIA_CODE + 6).as_deref(), Some("Media key 6"));
    }

    #[test]
    fn named_keys_have_their_fixed_key_codes_and_symbol_keys_come_from_the_layout() {
        let mut decoder = Decoder::new(HashMap::from([(50, '`'), (27, '-')]));
        for (keycode, named) in [
            (48, Named::Tab),
            (36, Named::Return),
            (123, Named::Left),
            (96, Named::F(5)),
            (90, Named::F(20)),
            (83, Named::Keypad('1')),
            (76, Named::KeypadEnter),
            (114, Named::Help),
        ] {
            assert_eq!(
                decoder.decode(key_down(keycode)),
                TapAction::Key(Phys::Named(named), true)
            );
            assert_eq!(named_keycode(named), Some(keycode));
        }
        assert_eq!(
            decoder.decode(key_down(50)),
            TapAction::Key(Phys::Char('`'), true)
        );
        assert_eq!(
            decoder.decode(key_down(27)),
            TapAction::Key(Phys::Char('-'), true)
        );
        let media = MEDIA_KEYS.map(|(_, named)| named);
        assert!(
            Named::all()
                .filter(|named| !media.contains(named))
                .filter(|named| !matches!(named, Named::F(n) if *n > 20))
                .filter(|named| !matches!(
                    named,
                    Named::Insert
                        | Named::PrintScreen
                        | Named::ScrollLock
                        | Named::Pause
                        | Named::NumLock
                ))
                .all(|named| named_keycode(named).is_some())
        );
    }

    #[test]
    fn a_tap_disabled_event_asks_for_a_re_enable() {
        let mut decoder = decoder();
        assert_eq!(
            decoder.decode(TapEvent::DisabledByTimeout),
            TapAction::Reenable
        );
        assert_eq!(
            decoder.decode(TapEvent::DisabledByUserInput),
            TapAction::Reenable
        );
    }

    #[test]
    fn a_reset_forgets_the_modifiers_that_were_down() {
        let mut decoder = decoder();
        decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE));
        decoder.reset();
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE)),
            TapAction::Key(R_OPTION, true)
        );
    }

    /// Feeds tap events through the decoder and the engine, as the tap thread does.
    fn run(engine: &mut Engine, decoder: &mut Decoder, events: &[TapEvent]) -> Vec<AppEvent> {
        let mut out = Vec::new();
        for event in events {
            let outputs = match decoder.decode(*event) {
                TapAction::Key(key, true) => engine.press(key),
                TapAction::Key(key, false) => engine.release(key),
                TapAction::Tap(key) => {
                    let mut both = engine.press(key);
                    both.extend(engine.release(key));
                    both
                }
                TapAction::Reenable | TapAction::Ignore => Vec::new(),
            };
            out.extend(outputs.into_iter().filter_map(|output| match output {
                Output::App(event) => Some(event),
                Output::Record(_) => None,
            }));
        }
        out
    }

    fn mac_bindings() -> Bindings {
        let mac = |text: &str| Combo::parse(text, Platform::MacOs).unwrap();
        Bindings {
            hold: Some(mac("RightOption")),
            hands_free: Some(HandsFree::Default),
            paste_last: Some(mac("Ctrl+Cmd+V")),
            command: Some(mac("RightOption+RightShift")),
            read_aloud: Some(mac("Ctrl+Cmd+S")),
        }
    }

    #[test]
    fn the_default_mac_shortcuts_work_through_the_decoder() {
        let mut engine = Engine::new(mac_bindings());
        let mut decoder = decoder();
        let held = run(
            &mut engine,
            &mut decoder,
            &[
                flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE),
                flags(KEYCODE_RIGHT_OPTION, 0),
            ],
        );
        assert_eq!(held, [AppEvent::HoldArm, AppEvent::HoldUp]);
        let pasted = run(
            &mut engine,
            &mut decoder,
            &[
                flags(59, FLAG_CONTROL),
                flags(55, CTRL_CMD),
                key_down(KEYCODE_V),
                key_up(KEYCODE_V),
                flags(55, FLAG_CONTROL),
                flags(59, 0),
            ],
        );
        assert_eq!(pasted, [AppEvent::PasteLast]);
    }

    #[test]
    fn a_paste_key_the_app_sends_itself_is_not_the_shortcut() {
        let mut engine = Engine::new(mac_bindings());
        let mut decoder = decoder();
        let events = run(
            &mut engine,
            &mut decoder,
            &[
                flags(55, FLAG_COMMAND),
                key_down(KEYCODE_V),
                key_up(KEYCODE_V),
                flags(55, 0),
            ],
        );
        assert_eq!(events, []);
    }

    #[test]
    fn fn_can_be_the_hold_key() {
        let mut engine = Engine::new(Bindings {
            hold: Some(Combo::parse("Fn", Platform::MacOs).unwrap()),
            ..Bindings::default()
        });
        let mut decoder = decoder();
        let events = run(
            &mut engine,
            &mut decoder,
            &[
                flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE),
                flags(KEYCODE_RIGHT_OPTION, 0),
                flags(KEYCODE_FN, FLAG_SECONDARY_FN),
                flags(KEYCODE_FN, 0),
            ],
        );
        assert_eq!(events, [AppEvent::HoldDown, AppEvent::HoldUp]);
    }

    #[derive(Clone)]
    struct FakeTap {
        id: u32,
        alive: Rc<Cell<bool>>,
        enabled: Rc<RefCell<Vec<u32>>>,
    }

    impl Tap for FakeTap {
        fn alive(&self) -> bool {
            self.alive.get()
        }

        fn enable(&self) {
            self.enabled.borrow_mut().push(self.id);
        }
    }

    fn fake(id: u32, enabled: &Rc<RefCell<Vec<u32>>>) -> FakeTap {
        FakeTap {
            id,
            alive: Rc::new(Cell::new(true)),
            enabled: Rc::clone(enabled),
        }
    }

    #[test]
    fn a_disabled_event_turns_the_tap_on_again() {
        let enabled = Rc::default();
        let guard = TapGuard::new(fake(1, &enabled), Instant::now());
        let mut decoder = decoder();
        if decoder.decode(TapEvent::DisabledByTimeout) == TapAction::Reenable {
            guard.reenable();
        }
        assert_eq!(*enabled.borrow(), vec![1]);
    }

    #[test]
    fn the_health_check_runs_every_five_seconds_and_leaves_a_live_tap_alone() {
        let enabled = Rc::default();
        let start = Instant::now();
        let mut guard = TapGuard::new(fake(1, &enabled), start);
        let install = || -> Option<FakeTap> { panic!("a live tap must not be replaced") };
        assert_eq!(
            guard.health_check(start + Duration::from_secs(4), install),
            None
        );
        assert_eq!(
            guard.health_check(start + Duration::from_secs(5), install),
            Some(Health::Fine)
        );
    }

    #[test]
    fn a_dead_tap_is_reinstalled_and_the_new_tap_is_the_one_checked_next() {
        let enabled = Rc::default();
        let start = Instant::now();
        let dead = fake(1, &enabled);
        dead.alive.set(false);
        let mut guard = TapGuard::new(dead, start);
        let fresh = fake(2, &enabled);
        assert_eq!(
            guard.health_check(start + Duration::from_secs(5), || Some(fresh)),
            Some(Health::Reinstalled)
        );
        guard.reenable();
        assert_eq!(
            *enabled.borrow(),
            vec![2],
            "the replacement is the live tap"
        );
        assert_eq!(
            guard.health_check(start + Duration::from_secs(10), || None),
            Some(Health::Fine)
        );
    }

    #[test]
    fn a_dead_tap_that_cannot_be_reinstalled_is_reported_and_tried_again() {
        let enabled = Rc::default();
        let start = Instant::now();
        let dead = fake(1, &enabled);
        dead.alive.set(false);
        let mut guard = TapGuard::new(dead, start);
        assert_eq!(
            guard.health_check(start + Duration::from_secs(5), || None),
            Some(Health::Dead)
        );
        assert_eq!(
            guard.health_check(start + Duration::from_secs(7), || None),
            None
        );
        assert_eq!(
            guard.health_check(start + Duration::from_secs(10), || None),
            Some(Health::Dead)
        );
    }
}
