//! The checks that run before any text is written or any key is pressed.
//!
//! A platform answers four yes-or-no questions through [`Probe`]; [`check`] turns the answers
//! into the reason to stop, or `None` when the paste may go on. A stop never writes the
//! clipboard and never sends a key, so the text can only reach a secure field or a keyboard
//! grabber through the user's own "Paste last transcript".

use super::report::Outcome;
use crate::error::{INSERT_KEYBOARD_GRABBED, INSERT_NO_PERMISSION, INSERT_SECURE_FIELD};

/// What the platform can see right now. The defaults describe a system with no such limit.
pub trait Probe {
    /// macOS may post key events for this app (`CGPreflightPostEventAccess`).
    fn post_event_allowed(&self) -> bool {
        true
    }

    /// macOS secure event input is on (`IsSecureEventInputEnabled`): a password field somewhere
    /// has the keyboard, and key events must not be sent.
    fn secure_event_input(&self) -> bool {
        false
    }

    /// The focused element is an `AXSecureTextField`.
    fn focused_secure_field(&self) -> bool {
        false
    }

    /// Another X11 client holds a keyboard grab, so a paste chord would go to it.
    fn keyboard_grabbed(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    /// macOS has not allowed Hushpen to post keys.
    NoPermission,
    /// Secure event input is on.
    SecureInput,
    /// The focused field is a password field.
    SecureField,
    KeyboardGrabbed,
}

impl Block {
    pub fn code(self) -> &'static str {
        match self {
            Block::NoPermission => INSERT_NO_PERMISSION,
            Block::SecureInput | Block::SecureField => INSERT_SECURE_FIELD,
            Block::KeyboardGrabbed => INSERT_KEYBOARD_GRABBED,
        }
    }

    pub fn outcome(self) -> Outcome {
        match self {
            Block::NoPermission => Outcome::NoPermission,
            Block::SecureInput | Block::SecureField => Outcome::BlockedSecure,
            Block::KeyboardGrabbed => Outcome::BlockedGrab,
        }
    }
}

/// The first reason to stop, in the order of the questions. A question after the first yes is
/// never asked: each one is a call into the system.
pub fn check(probe: &dyn Probe) -> Option<Block> {
    if !probe.post_event_allowed() {
        Some(Block::NoPermission)
    } else if probe.secure_event_input() {
        Some(Block::SecureInput)
    } else if probe.focused_secure_field() {
        Some(Block::SecureField)
    } else if probe.keyboard_grabbed() {
        Some(Block::KeyboardGrabbed)
    } else {
        None
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A platform with scripted answers that records every question it was asked.
    #[derive(Default)]
    pub(crate) struct MockPlatform {
        pub post_event_denied: bool,
        pub secure_input: bool,
        pub secure_field: bool,
        pub grabbed: bool,
        pub asked: RefCell<Vec<&'static str>>,
    }

    impl Probe for MockPlatform {
        fn post_event_allowed(&self) -> bool {
            self.asked.borrow_mut().push("post_event");
            !self.post_event_denied
        }

        fn secure_event_input(&self) -> bool {
            self.asked.borrow_mut().push("secure_input");
            self.secure_input
        }

        fn focused_secure_field(&self) -> bool {
            self.asked.borrow_mut().push("secure_field");
            self.secure_field
        }

        fn keyboard_grabbed(&self) -> bool {
            self.asked.borrow_mut().push("grab");
            self.grabbed
        }
    }

    #[test]
    fn all_clear_lets_the_paste_go_on() {
        let platform = MockPlatform::default();
        assert_eq!(check(&platform), None);
        assert_eq!(
            *platform.asked.borrow(),
            ["post_event", "secure_input", "secure_field", "grab"]
        );
    }

    #[test]
    fn secure_event_input_blocks_as_secure() {
        let platform = MockPlatform {
            secure_input: true,
            ..Default::default()
        };
        let block = check(&platform).unwrap();
        assert_eq!(block, Block::SecureInput);
        assert_eq!(block.outcome(), Outcome::BlockedSecure);
        assert_eq!(block.code(), "INSERT_SECURE_FIELD");
    }

    #[test]
    fn a_focused_secure_text_field_blocks_as_secure() {
        let platform = MockPlatform {
            secure_field: true,
            ..Default::default()
        };
        let block = check(&platform).unwrap();
        assert_eq!(block, Block::SecureField);
        assert_eq!(block.outcome(), Outcome::BlockedSecure);
        assert_eq!(block.code(), "INSERT_SECURE_FIELD");
    }

    #[test]
    fn a_keyboard_grab_blocks_as_a_grab() {
        let platform = MockPlatform {
            grabbed: true,
            ..Default::default()
        };
        let block = check(&platform).unwrap();
        assert_eq!(block.outcome(), Outcome::BlockedGrab);
        assert_eq!(block.code(), "INSERT_KEYBOARD_GRABBED");
    }

    #[test]
    fn missing_post_event_access_gives_no_permission_and_asks_nothing_else() {
        let platform = MockPlatform {
            post_event_denied: true,
            secure_input: true,
            ..Default::default()
        };
        let block = check(&platform).unwrap();
        assert_eq!(block.outcome(), Outcome::NoPermission);
        assert_eq!(block.code(), "INSERT_NO_PERMISSION");
        assert_eq!(*platform.asked.borrow(), ["post_event"]);
    }

    #[test]
    fn a_system_with_no_limits_never_blocks() {
        struct Open;
        impl Probe for Open {}
        assert_eq!(check(&Open), None);
    }
}
