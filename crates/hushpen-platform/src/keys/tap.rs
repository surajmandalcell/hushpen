//! Decoding of macOS event tap events, and the health rules for the tap itself.
//!
//! Pure logic with plain integers, so it is tested with synthetic events. The `CGEventTap`
//! glue in `macos.rs` only converts real events into [`TapEvent`] and runs what this module
//! decides. The key codes and flag bits are the macOS values (`kVK_RightOption`,
//! `kCGEventFlagMaskAlternate`, and so on), so this module compiles everywhere.

use super::HoldKey;
use hushpen_core::dictation::AppEvent;
use std::time::{Duration, Instant};

pub const KEYCODE_RIGHT_OPTION: i64 = 61;
pub const KEYCODE_FN: i64 = 63;
pub const KEYCODE_ESCAPE: i64 = 53;

pub const FLAG_ALTERNATE: u64 = 0x0008_0000;
pub const FLAG_SECONDARY_FN: u64 = 0x0080_0000;

/// How often the tap thread checks that the tap still lives.
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(5);

impl HoldKey {
    fn mac_keycode(self) -> i64 {
        match self {
            HoldKey::RightOption => KEYCODE_RIGHT_OPTION,
            HoldKey::Fn => KEYCODE_FN,
        }
    }

    fn mac_flag(self) -> u64 {
        match self {
            HoldKey::RightOption => FLAG_ALTERNATE,
            HoldKey::Fn => FLAG_SECONDARY_FN,
        }
    }
}

/// What the tap callback saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapEvent {
    FlagsChanged {
        keycode: i64,
        flags: u64,
    },
    KeyDown {
        keycode: i64,
    },
    KeyUp {
        keycode: i64,
    },
    /// `kCGEventTapDisabledByTimeout`: the system turned the tap off because the callback was slow.
    DisabledByTimeout,
    /// `kCGEventTapDisabledByUserInput`.
    DisabledByUserInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TapAction {
    Emit(AppEvent),
    /// Turn the tap on again.
    Reenable,
    Ignore,
}

/// Turns tap events into pipeline events for one hold key.
#[derive(Debug)]
pub struct Decoder {
    hold: HoldKey,
    down: bool,
}

impl Decoder {
    pub fn new(hold: HoldKey) -> Self {
        Self { hold, down: false }
    }

    pub fn decode(&mut self, event: TapEvent) -> TapAction {
        match event {
            TapEvent::FlagsChanged { keycode, flags } if keycode == self.hold.mac_keycode() => {
                let pressed = flags & self.hold.mac_flag() != 0;
                match (pressed, self.down) {
                    (true, false) => {
                        self.down = true;
                        TapAction::Emit(AppEvent::HoldDown)
                    }
                    (false, true) => {
                        self.down = false;
                        TapAction::Emit(AppEvent::HoldUp)
                    }
                    _ => TapAction::Ignore,
                }
            }
            TapEvent::KeyDown { keycode } if keycode == KEYCODE_ESCAPE => {
                TapAction::Emit(AppEvent::Esc)
            }
            TapEvent::DisabledByTimeout | TapEvent::DisabledByUserInput => TapAction::Reenable,
            _ => TapAction::Ignore,
        }
    }

    /// A reinstalled tap cannot see the release of a key that went down before it. The pipeline
    /// must not stay in a hold forever, so a key that was down is released now.
    pub fn reset(&mut self) -> Option<AppEvent> {
        std::mem::take(&mut self.down).then_some(AppEvent::HoldUp)
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
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    fn flags(keycode: i64, flags: u64) -> TapEvent {
        TapEvent::FlagsChanged { keycode, flags }
    }

    #[test]
    fn right_option_with_the_alternate_flag_is_hold_down_and_clearing_it_is_hold_up() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE)),
            TapAction::Emit(AppEvent::HoldDown)
        );
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, 0)),
            TapAction::Emit(AppEvent::HoldUp)
        );
    }

    #[test]
    fn a_repeated_flags_event_does_not_repeat_hold_down_or_hold_up() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
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
    fn left_option_keycode_58_gives_nothing() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
        assert_eq!(decoder.decode(flags(58, FLAG_ALTERNATE)), TapAction::Ignore);
        assert_eq!(decoder.decode(flags(58, 0)), TapAction::Ignore);
    }

    #[test]
    fn with_the_fn_setting_keycode_63_and_the_secondary_fn_flag_hold_the_key() {
        let mut decoder = Decoder::new(HoldKey::Fn);
        assert_eq!(
            decoder.decode(flags(KEYCODE_FN, FLAG_SECONDARY_FN)),
            TapAction::Emit(AppEvent::HoldDown)
        );
        assert_eq!(
            decoder.decode(flags(KEYCODE_FN, 0)),
            TapAction::Emit(AppEvent::HoldUp)
        );
        assert_eq!(
            decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE)),
            TapAction::Ignore,
            "Right Option does nothing while Fn is the hold key"
        );
    }

    #[test]
    fn the_default_setting_ignores_fn() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
        assert_eq!(
            decoder.decode(flags(KEYCODE_FN, FLAG_SECONDARY_FN)),
            TapAction::Ignore
        );
    }

    #[test]
    fn escape_key_down_is_esc_and_other_keys_are_ignored() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
        assert_eq!(
            decoder.decode(TapEvent::KeyDown {
                keycode: KEYCODE_ESCAPE
            }),
            TapAction::Emit(AppEvent::Esc)
        );
        assert_eq!(
            decoder.decode(TapEvent::KeyUp {
                keycode: KEYCODE_ESCAPE
            }),
            TapAction::Ignore
        );
        assert_eq!(
            decoder.decode(TapEvent::KeyDown { keycode: 0 }),
            TapAction::Ignore
        );
    }

    #[test]
    fn a_tap_disabled_event_asks_for_a_re_enable() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
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
    fn a_reset_releases_a_key_that_was_down_once() {
        let mut decoder = Decoder::new(HoldKey::RightOption);
        assert_eq!(decoder.reset(), None);
        decoder.decode(flags(KEYCODE_RIGHT_OPTION, FLAG_ALTERNATE));
        assert_eq!(decoder.reset(), Some(AppEvent::HoldUp));
        assert_eq!(decoder.reset(), None);
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
        let mut decoder = Decoder::new(HoldKey::RightOption);
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
