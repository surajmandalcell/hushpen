//! Keyboard shortcuts: the setting text, key names for each system, the rules a new shortcut
//! must pass, and the engine that turns raw key presses into pipeline events.
//!
//! A shortcut is a [`Combo`]: modifier-only keys such as Right Alt or Fn, or modifiers plus one
//! letter or digit. Nothing here touches a keyboard; the platform layer feeds [`Engine`] with
//! physical key events and sends out what it answers.

mod engine;
mod names;
mod register;
#[cfg(test)]
mod tests;

pub use engine::{Bindings, Engine, Output, Phys, Recording, Side};
pub use register::{Refusal, Setting, bindings, check};

use std::collections::BTreeSet;

/// The default hands-free setting: a double tap of the hold key, and the hold key with Space.
pub const DEFAULT_HANDS_FREE: &str = "DoubleTap+Hold+Space";

/// Most keys one shortcut may have.
const MAX_KEYS: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    MacOs,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else {
            Platform::Linux
        }
    }
}

/// The four shortcuts a user can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// Held while speaking.
    Hold,
    /// Starts a session, and stops it the next time.
    HandsFree,
    PasteLast,
    /// Held while speaking an instruction for the selected text.
    Command,
}

impl Slot {
    pub const ALL: [Slot; 4] = [Slot::Hold, Slot::HandsFree, Slot::PasteLast, Slot::Command];

    /// The name used in hook ids and the hook state.
    pub fn key(self) -> &'static str {
        match self {
            Slot::Hold => "hold",
            Slot::HandsFree => "handsFree",
            Slot::PasteLast => "pasteLast",
            Slot::Command => "command",
        }
    }

    pub fn from_key(key: &str) -> Option<Slot> {
        Self::ALL.into_iter().find(|slot| slot.key() == key)
    }

    pub fn setting_key(self) -> &'static str {
        match self {
            Slot::Hold => "shortcut.hold",
            Slot::HandsFree => "shortcut.handsFree",
            Slot::PasteLast => "shortcut.pasteLast",
            Slot::Command => "shortcut.command",
        }
    }

    /// The setting text of the factory default, which is what the store writes too.
    pub fn default_text(self, platform: Platform) -> &'static str {
        let mac = platform == Platform::MacOs;
        match self {
            Slot::Hold if mac => "RightOption",
            Slot::Hold => "RightAlt",
            Slot::HandsFree => DEFAULT_HANDS_FREE,
            Slot::PasteLast if mac => "Ctrl+Cmd+V",
            Slot::PasteLast => "Ctrl+Alt+V",
            Slot::Command if mac => "RightOption+RightShift",
            Slot::Command => "RightAlt+RightShift",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Cmd,
}

/// One key of a shortcut. The order is the order of the setting text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Fn,
    /// The modifier on either side.
    Any(Modifier),
    /// The modifier on the right side only.
    Right(Modifier),
    /// A lowercase ASCII letter or digit.
    Char(char),
}

/// Why a set of keys is not a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Empty,
    TooMany,
    BothSides,
    /// The Fn key exists on a Mac only.
    NoFn,
    BadKey,
    /// A letter needs Ctrl, Alt, or Cmd, or the shortcut would take ordinary typing.
    NeedsModifier,
    /// Modifiers alone must name the key, such as Right Alt.
    NeedsRightSide,
}

impl Problem {
    pub fn message(self, platform: Platform) -> String {
        let (alt, cmd) = names::modifier_words(platform);
        match self {
            Problem::Empty => "Press the keys for the shortcut.".to_owned(),
            Problem::TooMany => format!("Use at most {MAX_KEYS} keys."),
            Problem::BothSides => "Use the same side for one modifier.".to_owned(),
            Problem::NoFn => "The Fn key is only available on a Mac.".to_owned(),
            Problem::BadKey => "Use a letter or a digit with the modifiers.".to_owned(),
            Problem::NeedsModifier => format!("Add Ctrl, {alt}, or {cmd} to the letter."),
            Problem::NeedsRightSide => {
                format!("Press a modifier on the right side, such as Right {alt}, or add a letter.")
            }
        }
    }
}

/// A valid shortcut: sorted, no repeats.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Combo {
    keys: Vec<Key>,
}

impl Combo {
    pub fn new(keys: impl IntoIterator<Item = Key>, platform: Platform) -> Result<Self, Problem> {
        let keys: BTreeSet<Key> = keys.into_iter().collect();
        if keys.is_empty() {
            return Err(Problem::Empty);
        }
        if keys.len() > MAX_KEYS {
            return Err(Problem::TooMany);
        }
        let any: BTreeSet<Modifier> = keys
            .iter()
            .filter_map(|key| match key {
                Key::Any(modifier) => Some(*modifier),
                _ => None,
            })
            .collect();
        let right: BTreeSet<Modifier> = keys
            .iter()
            .filter_map(|key| match key {
                Key::Right(modifier) => Some(*modifier),
                _ => None,
            })
            .collect();
        if any.intersection(&right).next().is_some() {
            return Err(Problem::BothSides);
        }
        if keys.contains(&Key::Fn) && platform != Platform::MacOs {
            return Err(Problem::NoFn);
        }
        let letters: Vec<char> = keys
            .iter()
            .filter_map(|key| match key {
                Key::Char(c) => Some(*c),
                _ => None,
            })
            .collect();
        if letters.len() > 1 || letters.iter().any(|c| !c.is_ascii_alphanumeric()) {
            return Err(Problem::BadKey);
        }
        if letters.is_empty() {
            if keys
                .iter()
                .any(|key| !matches!(key, Key::Right(_) | Key::Fn))
            {
                return Err(Problem::NeedsRightSide);
            }
        } else if !keys.iter().any(|key| {
            matches!(
                modifier_of(*key),
                Some(Modifier::Ctrl | Modifier::Alt | Modifier::Cmd)
            )
        }) {
            return Err(Problem::NeedsModifier);
        }
        Ok(Self {
            keys: keys.into_iter().collect(),
        })
    }

    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    pub fn letter(&self) -> Option<char> {
        self.keys.iter().find_map(|key| match key {
            Key::Char(c) => Some(*c),
            _ => None,
        })
    }

    /// Reads setting text such as `Ctrl+Alt+V` or `RightOption`. Names are not case sensitive,
    /// and `Alt` and `Option`, `Cmd`, `Command`, and `Super` mean the same key. A side is
    /// written `RightAlt` or `Right Alt`.
    pub fn parse(text: &str, platform: Platform) -> Result<Self, Problem> {
        let keys: Result<Vec<Key>, Problem> = text.split('+').map(parse_key).collect();
        Self::new(keys?, platform)
    }

    /// The setting text. `parse` reads it back to the same shortcut.
    pub fn to_setting(&self, platform: Platform) -> String {
        names::setting(&self.keys, platform)
    }

    /// The words shown to the user: `Ctrl+Alt+V` on Linux, `⌃⌘V` on a Mac.
    pub fn display(&self, platform: Platform) -> String {
        names::display(&self.keys, platform)
    }

    /// The words for keys that are down in the recorder and may not be a shortcut yet.
    pub fn display_keys(keys: &[Key], platform: Platform) -> String {
        let keys: BTreeSet<Key> = keys.iter().copied().collect();
        names::display(&keys.into_iter().collect::<Vec<_>>(), platform)
    }

    /// Whether the system uses this shortcut, so Hushpen must not take it. Cmd (Ctrl on Linux)
    /// with the common editing, window, and quit letters, with or without Shift.
    pub fn reserved(&self, platform: Platform) -> bool {
        const LETTERS: &str = "acfnopqstvwxyz";
        const WITH_SHIFT: &str = "cqtvwxn";
        let Some(letter) = self.letter() else {
            return false;
        };
        let primary = if platform == Platform::MacOs {
            Modifier::Cmd
        } else {
            Modifier::Ctrl
        };
        let modifiers: BTreeSet<Modifier> =
            self.keys.iter().filter_map(|k| modifier_of(*k)).collect();
        if modifiers == BTreeSet::from([primary]) {
            LETTERS.contains(letter)
        } else {
            modifiers == BTreeSet::from([primary, Modifier::Shift]) && WITH_SHIFT.contains(letter)
        }
    }
}

fn modifier_of(key: Key) -> Option<Modifier> {
    match key {
        Key::Any(modifier) | Key::Right(modifier) => Some(modifier),
        Key::Fn | Key::Char(_) => None,
    }
}

fn parse_key(token: &str) -> Result<Key, Problem> {
    let name: String = token
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect();
    let (right, base) = match name.strip_prefix("right") {
        Some(rest) => (true, rest),
        None => (false, name.as_str()),
    };
    let modifier = match base {
        "ctrl" | "control" => Some(Modifier::Ctrl),
        "alt" | "option" | "opt" => Some(Modifier::Alt),
        "shift" => Some(Modifier::Shift),
        "cmd" | "command" | "super" => Some(Modifier::Cmd),
        _ => None,
    };
    if let Some(modifier) = modifier {
        return Ok(if right {
            Key::Right(modifier)
        } else {
            Key::Any(modifier)
        });
    }
    if name == "fn" {
        return Ok(Key::Fn);
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Ok(Key::Char(c)),
        _ => Err(Problem::BadKey),
    }
}

/// The hands-free setting: the built-in gestures, or one recorded shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandsFree {
    /// A double tap of the hold key, and the hold key with Space.
    Default,
    Custom(Combo),
}

impl HandsFree {
    pub fn parse(text: &str, platform: Platform) -> Result<Self, Problem> {
        if text.trim().eq_ignore_ascii_case(DEFAULT_HANDS_FREE) {
            Ok(HandsFree::Default)
        } else {
            Combo::parse(text, platform).map(HandsFree::Custom)
        }
    }

    pub fn to_setting(&self, platform: Platform) -> String {
        match self {
            HandsFree::Default => DEFAULT_HANDS_FREE.to_owned(),
            HandsFree::Custom(combo) => combo.to_setting(platform),
        }
    }

    /// `hold` is how the hold key is named for the user.
    pub fn display(&self, hold: &Combo, platform: Platform) -> String {
        match self {
            HandsFree::Default => names::default_hands_free(hold, platform),
            HandsFree::Custom(combo) => combo.display(platform),
        }
    }

    pub fn combo(&self) -> Option<&Combo> {
        match self {
            HandsFree::Default => None,
            HandsFree::Custom(combo) => Some(combo),
        }
    }
}
