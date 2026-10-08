//! Decoding of macOS event tap events, and the health rules for the tap itself.
//!
//! Pure logic with plain integers, so it is tested with synthetic events. The `CGEventTap`
//! glue in `macos.rs` only converts real events into [`TapEvent`] and runs what this module
//! decides. The key codes and flag bits are the macOS values (`kVK_RightOption`,
//! `kCGEventFlagMaskAlternate`, and so on), so this module compiles everywhere.

use hushpen_core::shortcut::{Modifier, Phys, Side};
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

pub const KEYCODE_ESCAPE: i64 = 53;
const KEYCODE_FN: i64 = 63;

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
    /// `kCGEventTapDisabledByTimeout`: the system turned the tap off because the callback was slow.
    DisabledByTimeout,
    /// `kCGEventTapDisabledByUserInput`.
    DisabledByUserInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapAction {
    /// A key went down (`true`) or up (`false`).
    Key(Phys, bool),
    /// Turn the tap on again.
    Reenable,
    Ignore,
}

/// Turns tap events into key presses and releases.
#[derive(Debug)]
pub struct Decoder {
    /// The letter and digit keys on the current layout, by key code.
    chars: HashMap<i64, char>,
    /// The modifiers that are down, so a flags event is a change or nothing.
    down: BTreeSet<Phys>,
}

impl Decoder {
    pub fn new(chars: HashMap<i64, char>) -> Self {
        Self {
            chars,
            down: BTreeSet::new(),
        }
    }

    pub fn decode(&mut self, event: TapEvent) -> TapAction {
        match event {
            TapEvent::FlagsChanged { keycode, flags } => self.flags_changed(keycode, flags),
            TapEvent::KeyDown { keycode, .. } => self.key(keycode, true),
            TapEvent::KeyUp { keycode, .. } => self.key(keycode, false),
            TapEvent::DisabledByTimeout | TapEvent::DisabledByUserInput => TapAction::Reenable,
        }
    }

    fn key(&self, keycode: i64, down: bool) -> TapAction {
        if keycode == KEYCODE_ESCAPE {
            return TapAction::Key(Phys::Esc, down);
        }
        match self.chars.get(&keycode) {
            Some(c) => TapAction::Key(Phys::Char(*c), down),
            None => TapAction::Ignore,
        }
    }

    fn flags_changed(&mut self, keycode: i64, flags: u64) -> TapAction {
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
        assert_eq!(decoder.decode(key_down(0)), TapAction::Ignore);
        assert_eq!(decoder.decode(flags(57, 0)), TapAction::Ignore, "Caps Lock");
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
        assert_eq!(held, [AppEvent::HoldDown, AppEvent::HoldUp]);
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
