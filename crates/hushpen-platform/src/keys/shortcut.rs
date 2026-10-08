//! A global shortcut such as `Ctrl+Alt+V`: the setting text, and the match of key events.
//!
//! The matcher fires when the chord is released, not when it goes down. A paste that started
//! while the user still held Ctrl and Alt would send Ctrl+Alt+V to the focused app instead of
//! Ctrl+V.

use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
    /// A lowercase ASCII letter or digit.
    pub key: char,
}

impl Shortcut {
    /// `Ctrl+Cmd+V` on macOS, `Ctrl+Alt+V` elsewhere.
    pub fn default_paste_last() -> Self {
        Self {
            ctrl: true,
            alt: !cfg!(target_os = "macos"),
            shift: false,
            cmd: cfg!(target_os = "macos"),
            key: 'v',
        }
    }

    /// Reads `Ctrl+Alt+V` style text. Modifier names: Ctrl or Control, Alt or Option, Shift,
    /// Cmd, Command, or Super. A shortcut needs one letter or digit and at least one of Ctrl,
    /// Alt, or Cmd, or it would take ordinary typing.
    pub fn parse(text: &str) -> Option<Self> {
        let mut shortcut = Self {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            key: '\0',
        };
        for token in text.split('+') {
            let token = token.trim().to_ascii_lowercase();
            let seen = match token.as_str() {
                "ctrl" | "control" => &mut shortcut.ctrl,
                "alt" | "option" | "opt" => &mut shortcut.alt,
                "shift" => &mut shortcut.shift,
                "cmd" | "command" | "super" => &mut shortcut.cmd,
                _ => {
                    let mut chars = token.chars();
                    let key = chars.next()?;
                    if chars.next().is_some()
                        || shortcut.key != '\0'
                        || !key.is_ascii_alphanumeric()
                    {
                        return None;
                    }
                    shortcut.key = key;
                    continue;
                }
            };
            if std::mem::replace(seen, true) {
                return None;
            }
        }
        (shortcut.key != '\0' && (shortcut.ctrl || shortcut.alt || shortcut.cmd))
            .then_some(shortcut)
    }
}

/// The key codes of one modifier on this system, and whether the shortcut needs it.
#[derive(Debug, Clone)]
pub struct Group {
    pub wanted: bool,
    pub codes: Vec<u32>,
}

/// Follows raw key presses and releases and says when the shortcut was completed and let go.
#[derive(Debug)]
pub struct Matcher {
    key: u32,
    groups: Vec<Group>,
    down: BTreeSet<u32>,
    armed: bool,
}

impl Matcher {
    /// `groups` are the Ctrl, Alt, Shift, and Cmd keys. A modifier the shortcut does not use
    /// must not be down when the key goes down, so `Ctrl+V` is not `Ctrl+Shift+V`.
    pub fn new(key: u32, groups: Vec<Group>) -> Self {
        Self {
            key,
            groups,
            down: BTreeSet::new(),
            armed: false,
        }
    }

    pub fn press(&mut self, code: u32) {
        self.down.insert(code);
        if code == self.key && self.modifiers_match() {
            self.armed = true;
        }
    }

    /// True once, when the last key of a completed shortcut comes up.
    pub fn release(&mut self, code: u32) -> bool {
        self.down.remove(&code);
        if !self.armed || self.down.contains(&self.key) || self.wanted_down() {
            return false;
        }
        self.armed = false;
        true
    }

    fn modifiers_match(&self) -> bool {
        self.groups
            .iter()
            .all(|group| group.codes.iter().any(|code| self.down.contains(code)) == group.wanted)
    }

    fn wanted_down(&self) -> bool {
        self.groups
            .iter()
            .filter(|group| group.wanted)
            .any(|group| group.codes.iter().any(|code| self.down.contains(code)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: u32 = 55;
    const CTRL: u32 = 37;
    const ALT: u32 = 64;
    const SHIFT: u32 = 50;

    fn matcher(shift: bool) -> Matcher {
        Matcher::new(
            KEY,
            vec![
                Group {
                    wanted: true,
                    codes: vec![CTRL],
                },
                Group {
                    wanted: true,
                    codes: vec![ALT],
                },
                Group {
                    wanted: shift,
                    codes: vec![SHIFT],
                },
            ],
        )
    }

    #[test]
    fn the_default_is_ctrl_alt_v_here_and_ctrl_cmd_v_on_a_mac() {
        let default = Shortcut::default_paste_last();
        assert!(default.ctrl);
        assert_eq!(default.key, 'v');
        assert_eq!(default.alt, !cfg!(target_os = "macos"));
        assert_eq!(default.cmd, cfg!(target_os = "macos"));
    }

    #[test]
    fn the_setting_text_parses_in_any_order_and_case() {
        let parsed = Shortcut::parse("shift+CTRL+alt+P").unwrap();
        assert_eq!(
            parsed,
            Shortcut {
                ctrl: true,
                alt: true,
                shift: true,
                cmd: false,
                key: 'p'
            }
        );
        assert_eq!(
            Shortcut::parse("Ctrl+Cmd+V"),
            Some(Shortcut {
                ctrl: true,
                alt: false,
                shift: false,
                cmd: true,
                key: 'v'
            })
        );
        assert_eq!(Shortcut::parse("Control + Option + 7").unwrap().key, '7');
    }

    #[test]
    fn text_that_would_take_ordinary_typing_or_is_unclear_is_refused() {
        for text in [
            "",
            "V",
            "Shift+V",
            "Ctrl",
            "Ctrl+Ctrl+V",
            "Ctrl+V+B",
            "Ctrl+F5",
            "Ctrl+é",
            "Ctrl+ +V",
        ] {
            assert_eq!(Shortcut::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_shortcut_fires_once_when_the_last_key_is_let_go() {
        let mut m = matcher(false);
        m.press(CTRL);
        m.press(ALT);
        m.press(KEY);
        assert!(!m.release(KEY), "Ctrl and Alt are still down");
        assert!(!m.release(ALT));
        assert!(m.release(CTRL));
        assert!(!m.release(CTRL), "a second release fires nothing");
    }

    #[test]
    fn the_modifiers_may_come_up_before_the_letter() {
        let mut m = matcher(false);
        m.press(CTRL);
        m.press(ALT);
        m.press(KEY);
        assert!(!m.release(CTRL));
        assert!(!m.release(ALT));
        assert!(m.release(KEY));
    }

    #[test]
    fn a_letter_alone_or_with_the_wrong_modifiers_fires_nothing() {
        let mut m = matcher(false);
        m.press(KEY);
        assert!(!m.release(KEY));
        m.press(CTRL);
        m.press(KEY);
        assert!(!m.release(KEY));
        assert!(!m.release(CTRL));
        m.press(CTRL);
        m.press(ALT);
        m.press(SHIFT);
        m.press(KEY);
        assert!(!m.release(KEY), "Ctrl+Alt+Shift+V is another shortcut");
        assert!(!m.release(SHIFT));
        assert!(!m.release(ALT));
        assert!(!m.release(CTRL));
    }

    #[test]
    fn a_shift_in_the_shortcut_must_be_down() {
        let mut m = matcher(true);
        m.press(CTRL);
        m.press(ALT);
        m.press(KEY);
        assert!(!m.release(KEY));
        assert!(!m.release(ALT));
        assert!(!m.release(CTRL));
        m.press(CTRL);
        m.press(ALT);
        m.press(SHIFT);
        m.press(KEY);
        m.release(KEY);
        m.release(SHIFT);
        m.release(ALT);
        assert!(m.release(CTRL));
    }

    #[test]
    fn a_key_sent_by_the_paste_itself_after_everything_is_up_starts_a_new_round() {
        let mut m = matcher(false);
        m.press(CTRL);
        m.press(ALT);
        m.press(KEY);
        m.release(KEY);
        m.release(ALT);
        assert!(m.release(CTRL));
        m.press(CTRL);
        m.press(KEY);
        m.release(KEY);
        assert!(!m.release(CTRL), "Ctrl+V alone is not the shortcut");
    }
}
