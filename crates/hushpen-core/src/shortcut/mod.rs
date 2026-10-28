//! Keyboard shortcuts: the setting text, key names for each system, the rules a new shortcut
//! must pass, and the engine that turns raw key presses into pipeline events.
//!
//! A shortcut is a [`Combo`]: any keys the system reports, held together. That is modifiers
//! alone (Cmd+Right Ctrl, Fn, Left Ctrl+Right Ctrl), one key alone (F13, Play, Caps Lock, a
//! letter), or modifiers with other keys (Cmd+Tab, Cmd+`). Hushpen does not keep a list of
//! allowed chords: only the system and the other slots can refuse one. Nothing here touches a
//! keyboard; the platform layer feeds [`Engine`] with physical key events and sends out what it
//! answers.

mod engine;
mod names;
mod register;
#[cfg(test)]
mod tests;

pub use engine::{Bindings, Engine, Output, Phys, Recording, Side};
pub use register::{Conflict, Opener, Refusal, Setting, bindings, check};

use std::collections::BTreeSet;
use std::sync::OnceLock;

/// The default hands-free setting: a double tap of the hold key, and the hold key with Space.
pub const DEFAULT_HANDS_FREE: &str = "DoubleTap+Hold+Space";

/// Most keys one shortcut may have. Most keyboards report at most six keys down at once.
const MAX_KEYS: usize = 6;

/// The function keys run from F1 to this one (X11 names F1 to F35; a Mac has F1 to F20).
pub const LAST_F_KEY: u8 = 35;

/// The system's name for a [`Key::Code`], set once by the platform layer.
static CODE_NAMES: OnceLock<fn(u32) -> Option<String>> = OnceLock::new();

/// Lets the platform name the keys that have no name here, such as `XF86Calculator`.
pub fn set_code_names(names: fn(u32) -> Option<String>) {
    let _ = CODE_NAMES.set(names);
}

fn code_name(code: u32) -> String {
    CODE_NAMES
        .get()
        .and_then(|names| names(code))
        .unwrap_or_else(|| format!("Key {code}"))
}

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

/// A key that types no character and that both systems name the same way: the editing and
/// moving keys, the keypad, the lock keys, the function keys, and the media keys.
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
    /// Esc with other keys. Esc alone closes the recorder, so it is never a shortcut alone.
    Escape,
    Insert,
    Help,
    CapsLock,
    /// The context menu key of a PC keyboard.
    Menu,
    PrintScreen,
    ScrollLock,
    Pause,
    NumLock,
    /// A keypad key: a digit, `.`, `+`, `-`, `*`, `/`, or `=`.
    Keypad(char),
    KeypadEnter,
    KeypadClear,
    /// F1 to [`LAST_F_KEY`].
    F(u8),
    PlayPause,
    NextTrack,
    PreviousTrack,
    FastForward,
    Rewind,
    VolumeUp,
    VolumeDown,
    Mute,
    BrightnessUp,
    BrightnessDown,
    KeyboardLightUp,
    KeyboardLightDown,
    Eject,
}

/// The keys of a keypad that are not Enter or Clear.
pub const KEYPAD_CHARS: &str = "0123456789.+-*/=";

impl Named {
    /// Every named key, in the order of the enum.
    pub fn all() -> impl Iterator<Item = Named> {
        use Named::*;
        [
            Tab,
            Return,
            Backspace,
            Delete,
            Up,
            Down,
            Left,
            Right,
            Home,
            End,
            PageUp,
            PageDown,
            Escape,
            Insert,
            Help,
            CapsLock,
            Menu,
            PrintScreen,
            ScrollLock,
            Pause,
            NumLock,
        ]
        .into_iter()
        .chain(KEYPAD_CHARS.chars().map(Keypad))
        .chain([KeypadEnter, KeypadClear])
        .chain((1..=LAST_F_KEY).map(F))
        .chain([
            PlayPause,
            NextTrack,
            PreviousTrack,
            FastForward,
            Rewind,
            VolumeUp,
            VolumeDown,
            Mute,
            BrightnessUp,
            BrightnessDown,
            KeyboardLightUp,
            KeyboardLightDown,
            Eject,
        ])
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
    /// A key with no name here, by the system's own number: the macOS key code (a media key
    /// is `0x10000` plus its `NX_KEYTYPE`), or the X11 keysym.
    Code(u32),
}

/// Why a set of keys is not a shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    Empty,
    TooMany,
    /// The Fn key exists on a Mac only.
    NoFn,
    /// A key name in the setting text that no key has.
    BadKey,
}

impl Problem {
    pub fn message(self, _platform: Platform) -> String {
        match self {
            Problem::Empty => "Press the keys for the shortcut.".to_owned(),
            Problem::TooMany => format!("Use at most {MAX_KEYS} keys."),
            Problem::NoFn => "The Fn key is only available on a Mac.".to_owned(),
            Problem::BadKey => "This shortcut names a key that does not exist.".to_owned(),
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
        if keys.contains(&Key::Fn) && platform != Platform::MacOs {
            return Err(Problem::NoFn);
        }
        if !keys.iter().all(|key| exists(*key)) {
            return Err(Problem::BadKey);
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

    /// The first key that is not a modifier or Fn, such as `V`, Tab, or F5.
    pub fn main(&self) -> Option<Key> {
        self.keys.iter().copied().find(is_main)
    }

    /// The keys that are not a modifier or Fn.
    pub fn mains(&self) -> impl Iterator<Item = Key> + '_ {
        self.keys.iter().copied().filter(is_main)
    }

    /// Whether the plain `modifier` key stands for the left one: the chord also has the right
    /// one, as in Left Ctrl+Right Ctrl.
    pub fn left_only(&self, modifier: Modifier) -> bool {
        self.keys.contains(&Key::Right(modifier))
    }

    /// Reads setting text such as `Ctrl+Alt+V`, `Cmd+Tab`, `Cmd+\``, `F13`, `PlayPause`, or
    /// `RightOption`. Names are not case sensitive, and `Alt` and `Option`, `Cmd`, `Command`,
    /// and `Super` mean the same key. A side is written `RightAlt` or `Right Alt`. The `+` key
    /// is written `Plus`, a keypad key `Keypad1`, and a key with no name `Key179`.
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
}

#[cfg(test)]
fn names_setting(named: Named) -> String {
    names::setting(&[Key::Named(named)], Platform::Linux)
}

fn is_main(key: &Key) -> bool {
    matches!(key, Key::Char(_) | Key::Named(_) | Key::Code(_))
}

/// Whether some keyboard has this key. Only hand-written setting text can name one that does
/// not, such as F99 or a keypad letter.
fn exists(key: Key) -> bool {
    match key {
        Key::Char(c) => c == ' ' || !(c.is_control() || c.is_whitespace() || c.is_uppercase()),
        Key::Named(Named::F(n)) => (1..=LAST_F_KEY).contains(&n),
        Key::Named(Named::Keypad(c)) => KEYPAD_CHARS.contains(c),
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
    if let Some(rest) = name.strip_prefix("keypad") {
        return Some(Key::Named(match rest {
            "enter" => Named::KeypadEnter,
            "clear" => Named::KeypadClear,
            "plus" => Named::Keypad('+'),
            "minus" => Named::Keypad('-'),
            "multiply" => Named::Keypad('*'),
            "divide" => Named::Keypad('/'),
            "equals" => Named::Keypad('='),
            "decimal" => Named::Keypad('.'),
            other => {
                let mut chars = other.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => Named::Keypad(c),
                    _ => return None,
                }
            }
        }));
    }
    if let Some(code) = name.strip_prefix("key").and_then(|n| n.parse().ok()) {
        return Some(Key::Code(code));
    }
    let named = match name {
        "space" => return Some(Key::Char(' ')),
        "plus" => return Some(Key::Char('+')),
        "escape" | "esc" => Named::Escape,
        "insert" | "ins" => Named::Insert,
        "help" => Named::Help,
        "capslock" => Named::CapsLock,
        "menu" => Named::Menu,
        "printscreen" | "print" => Named::PrintScreen,
        "scrolllock" => Named::ScrollLock,
        "pause" => Named::Pause,
        "numlock" => Named::NumLock,
        "playpause" | "play" => Named::PlayPause,
        "nexttrack" | "next" => Named::NextTrack,
        "previoustrack" | "previous" | "prev" => Named::PreviousTrack,
        "fastforward" => Named::FastForward,
        "rewind" => Named::Rewind,
        "volumeup" => Named::VolumeUp,
        "volumedown" => Named::VolumeDown,
        "mute" => Named::Mute,
        "brightnessup" => Named::BrightnessUp,
        "brightnessdown" => Named::BrightnessDown,
        "keyboardlightup" => Named::KeyboardLightUp,
        "keyboardlightdown" => Named::KeyboardLightDown,
        "eject" => Named::Eject,
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
