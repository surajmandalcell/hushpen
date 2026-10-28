//! Keyboard shortcuts: the setting text, key names for each system, the rules a new shortcut
//! must pass, and the engine that turns raw key presses into pipeline events.
//!
//! A shortcut is a [`Combo`]: modifier-only keys such as Cmd+Right Ctrl or Fn, or modifiers plus
//! one other key of any kind (a letter, ```, Tab, Space, an arrow, F5). Nothing here touches a
//! keyboard; the platform layer feeds [`Engine`] with physical key events and sends out what it
//! answers.

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

/// The function keys run from F1 to this one.
pub const LAST_F_KEY: u8 = 20;

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

/// A key that types no character: the editing and moving keys, and the function keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Named {
    Tab,
    Return,
    Backspace,
    /// Forward delete.
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// F1 to [`LAST_F_KEY`].
    F(u8),
}

impl Named {
    /// Every named key, in the order of the enum.
    pub fn all() -> impl Iterator<Item = Named> {
        use Named::*;
        [
            Tab, Return, Backspace, Delete, Up, Down, Left, Right, Home, End, PageUp, PageDown,
        ]
        .into_iter()
        .chain((1..=LAST_F_KEY).map(Named::F))
    }
}

/// One key of a shortcut. The order is the order of the setting text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    Fn,
    /// The modifier on either side.
    Any(Modifier),
    /// The modifier on the right side only.
    Right(Modifier),
    /// A character key, lowercase: a letter, a digit, a symbol key such as ``` or `-`, or
    /// Space as `' '`.
    Char(char),
    Named(Named),
}

/// Why a set of keys is not a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Empty,
    TooMany,
    BothSides,
    /// The Fn key exists on a Mac only.
    NoFn,
    /// Two keys that are not modifiers, or a key that cannot be a shortcut.
    BadKey,
    /// A key other than F1 to F20 needs Ctrl, Alt, Cmd, or Fn, or the shortcut would take
    /// ordinary typing.
    NeedsModifier,
    /// Modifiers alone need one on a named side, such as Right Alt, or plain Ctrl+Shift would
    /// start a session in the middle of every Ctrl+Shift shortcut.
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
            Problem::BadKey => "Use one key besides the modifiers.".to_owned(),
            Problem::NeedsModifier => format!("Add Ctrl, {alt}, or {cmd} to the key."),
            Problem::NeedsRightSide => {
                format!("Add a modifier on the right side, such as Right {alt}, or another key.")
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
        let mains: Vec<Key> = keys.iter().copied().filter(is_main).collect();
        if mains.len() > 1 || !mains.iter().all(|key| valid_main(*key)) {
            return Err(Problem::BadKey);
        }
        match mains.first() {
            None => {
                if !keys
                    .iter()
                    .any(|key| matches!(key, Key::Right(_) | Key::Fn))
                {
                    return Err(Problem::NeedsRightSide);
                }
            }
            // A function key types nothing, so it may be a shortcut on its own.
            Some(Key::Named(Named::F(_))) => {}
            Some(_) => {
                if !keys.iter().any(|key| {
                    *key == Key::Fn
                        || matches!(
                            modifier_of(*key),
                            Some(Modifier::Ctrl | Modifier::Alt | Modifier::Cmd)
                        )
                }) {
                    return Err(Problem::NeedsModifier);
                }
            }
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

    /// The one key that is not a modifier or Fn, such as `V`, Tab, or F5.
    pub fn main(&self) -> Option<Key> {
        self.keys.iter().copied().find(is_main)
    }

    /// Reads setting text such as `Ctrl+Alt+V`, `Cmd+Tab`, `Cmd+\``, or `RightOption`. Names are
    /// not case sensitive, and `Alt` and `Option`, `Cmd`, `Command`, and `Super` mean the same
    /// key. A side is written `RightAlt` or `Right Alt`. The `+` key is written `Plus`.
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

    /// Whether the system or most apps use this shortcut, so Hushpen must not take it. The keys
    /// on macOS still reach the front app, and an X11 grab takes them from every app, so either
    /// way the chord would stop doing its usual job.
    pub fn reserved(&self, platform: Platform) -> bool {
        use Modifier::{Alt, Cmd, Ctrl, Shift};
        // Cmd (Ctrl on Linux) with any letter or digit is an app shortcut almost everywhere.
        // Ctrl alone on a Mac moves the cursor in every text field.
        const MAC: &[(&[Modifier], &str)] = &[
            (&[Cmd], "abcdefghijklmnopqrstuvwxyz0123456789"),
            (&[Ctrl], "abdefhklnoptvy"),
            (&[Cmd, Shift], "cgnqtvwxz3456"),
            (&[Ctrl, Cmd], "fq"),
            (&[Alt, Cmd], "dhmw"),
            (&[Ctrl, Shift, Cmd], "345"),
        ];
        const LINUX: &[(&[Modifier], &str)] = &[
            (&[Ctrl], "abcdefghijklmnopqrstuvwxyz0123456789"),
            (&[Ctrl, Shift], "cnqtvwxz"),
            (&[Ctrl, Alt], "dlt"),
            (&[Cmd], "l"),
        ];
        let Some(letter) = self.letter() else {
            return false;
        };
        let modifiers: BTreeSet<Modifier> =
            self.keys.iter().filter_map(|k| modifier_of(*k)).collect();
        let table = if platform == Platform::MacOs {
            MAC
        } else {
            LINUX
        };
        table.iter().any(|(chord, letters)| {
            modifiers == chord.iter().copied().collect() && letters.contains(letter)
        })
    }
}

fn modifier_of(key: Key) -> Option<Modifier> {
    match key {
        Key::Any(modifier) | Key::Right(modifier) => Some(modifier),
        Key::Fn | Key::Char(_) | Key::Named(_) => None,
    }
}

fn is_main(key: &Key) -> bool {
    matches!(key, Key::Char(_) | Key::Named(_))
}

fn valid_main(key: Key) -> bool {
    match key {
        Key::Char(c) => c == ' ' || !(c.is_control() || c.is_whitespace() || c.is_uppercase()),
        Key::Named(Named::F(n)) => (1..=LAST_F_KEY).contains(&n),
        _ => true,
    }
}

/// A key as one character, lowercase.
fn char_key(c: char) -> Key {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(lower), None) => Key::Char(lower),
        _ => Key::Char(c),
    }
}

fn parse_named(name: &str) -> Option<Key> {
    let named = match name {
        "space" => return Some(Key::Char(' ')),
        "plus" => return Some(Key::Char('+')),
        "tab" => Named::Tab,
        "return" | "enter" => Named::Return,
        "backspace" => Named::Backspace,
        "delete" | "del" | "forwarddelete" => Named::Delete,
        "up" => Named::Up,
        "down" => Named::Down,
        "left" => Named::Left,
        "right" => Named::Right,
        "home" => Named::Home,
        "end" => Named::End,
        "pageup" | "pgup" => Named::PageUp,
        "pagedown" | "pgdn" => Named::PageDown,
        _ => {
            let n: u8 = name.strip_prefix('f')?.parse().ok()?;
            Named::F(n)
        }
    };
    Some(Key::Named(named))
}

fn parse_key(token: &str) -> Result<Key, Problem> {
    let trimmed = token.trim();
    let mut chars = trimmed.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return Ok(char_key(c));
    }
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
    parse_named(&name).ok_or(Problem::BadKey)
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
