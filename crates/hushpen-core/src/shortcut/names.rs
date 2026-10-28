//! How keys are written: in the setting text, and for the user on each system.

use super::{Combo, Key, Modifier, Named, Platform};

/// The words for the Alt and Cmd keys on this system.
pub(super) fn modifier_words(platform: Platform) -> (&'static str, &'static str) {
    match platform {
        Platform::MacOs => ("Option", "Command"),
        Platform::Linux => ("Alt", "Super"),
    }
}

fn setting_name(modifier: Modifier, platform: Platform) -> &'static str {
    let (alt, _) = modifier_words(platform);
    match modifier {
        Modifier::Ctrl => "Ctrl",
        Modifier::Alt => alt,
        Modifier::Shift => "Shift",
        Modifier::Cmd if platform == Platform::MacOs => "Cmd",
        Modifier::Cmd => "Super",
    }
}

fn keypad_word(c: char) -> String {
    match c {
        '+' => "Plus".into(),
        '-' => "Minus".into(),
        '*' => "Multiply".into(),
        '/' => "Divide".into(),
        '=' => "Equals".into(),
        '.' => "Decimal".into(),
        c => c.to_string(),
    }
}

/// The setting word of a named key, which `parse` reads back.
fn named_setting(named: Named) -> String {
    match named {
        Named::Keypad(c) => format!("Keypad{}", keypad_word(c)),
        Named::F(n) => format!("F{n}"),
        other => format!("{other:?}"),
    }
}

/// A named key in words, for Linux and for the keys a Mac has no symbol for.
fn named_words(named: Named) -> String {
    match named {
        Named::Return => "Enter".into(),
        Named::PageUp => "Page Up".into(),
        Named::PageDown => "Page Down".into(),
        Named::Escape => "Esc".into(),
        Named::CapsLock => "Caps Lock".into(),
        Named::PrintScreen => "Print Screen".into(),
        Named::ScrollLock => "Scroll Lock".into(),
        Named::NumLock => "Num Lock".into(),
        Named::Keypad(c) => format!("Keypad {c}"),
        Named::KeypadEnter => "Keypad Enter".into(),
        Named::KeypadClear => "Keypad Clear".into(),
        Named::PlayPause => "Play/Pause".into(),
        Named::NextTrack => "Next Track".into(),
        Named::PreviousTrack => "Previous Track".into(),
        Named::FastForward => "Fast Forward".into(),
        Named::VolumeUp => "Volume Up".into(),
        Named::VolumeDown => "Volume Down".into(),
        Named::BrightnessUp => "Brightness Up".into(),
        Named::BrightnessDown => "Brightness Down".into(),
        Named::KeyboardLightUp => "Keyboard Light Up".into(),
        Named::KeyboardLightDown => "Keyboard Light Down".into(),
        other => named_setting(other),
    }
}

/// A character key in the setting text and on Linux. `+` is the separator, so it has a name.
fn char_word(c: char) -> String {
    match c {
        ' ' => "Space".into(),
        '+' => "Plus".into(),
        c => c.to_uppercase().collect(),
    }
}

pub(super) fn setting(keys: &[Key], platform: Platform) -> String {
    keys.iter()
        .map(|key| match key {
            Key::Fn => "Fn".to_owned(),
            Key::Any(modifier) => setting_name(*modifier, platform).to_owned(),
            Key::Right(modifier) => format!("Right{}", setting_name(*modifier, platform)),
            Key::Char(c) => char_word(*c),
            Key::Named(named) => named_setting(*named),
            Key::Code(code) => format!("Key{code}"),
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn symbol(modifier: Modifier) -> &'static str {
    match modifier {
        Modifier::Ctrl => "⌃",
        Modifier::Alt => "⌥",
        Modifier::Shift => "⇧",
        Modifier::Cmd => "⌘",
    }
}

/// The key symbol of macOS menus, for the keys that have one.
fn mac_symbol(named: Named) -> Option<&'static str> {
    Some(match named {
        Named::Tab => "⇥",
        Named::Return => "↩",
        Named::Backspace => "⌫",
        Named::Delete => "⌦",
        Named::Up => "↑",
        Named::Down => "↓",
        Named::Left => "←",
        Named::Right => "→",
        Named::Home => "↖",
        Named::End => "↘",
        Named::PageUp => "⇞",
        Named::PageDown => "⇟",
        Named::Escape => "⎋",
        Named::CapsLock => "⇪",
        Named::KeypadEnter => "⌤",
        Named::KeypadClear => "⌧",
        Named::Eject => "⏏",
        _ => return None,
    })
}

/// Whether the plain modifier of `keys` stands for the left key, as in Left Ctrl+Right Ctrl.
fn left_only(keys: &[Key], modifier: Modifier) -> bool {
    keys.contains(&Key::Right(modifier))
}

pub(super) fn display(keys: &[Key], platform: Platform) -> String {
    match platform {
        Platform::Linux => keys
            .iter()
            .map(|key| match key {
                Key::Fn => "Fn".to_owned(),
                Key::Any(modifier) if left_only(keys, *modifier) => {
                    format!("Left {}", setting_name(*modifier, platform))
                }
                Key::Any(modifier) => setting_name(*modifier, platform).to_owned(),
                Key::Right(modifier) => {
                    format!("Right {}", setting_name(*modifier, platform))
                }
                Key::Char(c) => char_word(*c),
                Key::Named(named) => named_words(*named),
                Key::Code(code) => super::code_name(*code),
            })
            .collect::<Vec<_>>()
            .join("+"),
        Platform::MacOs => mac_display(keys),
    }
}

/// A chord of modifiers and one key with a symbol reads as `⌃⌘V` or `⌘⇥`. A side, the Fn key,
/// or a key that needs words reads as `Right ⌥+Right ⇧` or `⌘+Volume Up`.
fn mac_display(keys: &[Key]) -> String {
    let mains = keys
        .iter()
        .filter(|key| matches!(key, Key::Char(_) | Key::Named(_) | Key::Code(_)))
        .count();
    let plain = mains <= 1
        && keys.iter().all(|key| match key {
            Key::Any(modifier) => !left_only(keys, *modifier),
            Key::Char(_) => true,
            Key::Named(named) => mac_symbol(*named).is_some() || matches!(named, Named::F(_)),
            Key::Fn | Key::Right(_) | Key::Code(_) => false,
        });
    let parts = keys.iter().map(|key| match key {
        Key::Fn => "Fn".to_owned(),
        Key::Any(modifier) if left_only(keys, *modifier) => format!("Left {}", symbol(*modifier)),
        Key::Any(modifier) => symbol(*modifier).to_owned(),
        Key::Right(modifier) => format!("Right {}", symbol(*modifier)),
        Key::Char(' ') => "Space".to_owned(),
        Key::Char(c) => c.to_uppercase().collect(),
        Key::Named(named) => mac_symbol(*named)
            .map(str::to_owned)
            .unwrap_or_else(|| named_words(*named)),
        Key::Code(code) => super::code_name(*code),
    });
    parts.collect::<Vec<_>>().join(if plain { "" } else { "+" })
}

/// `×2` is the double tap. The short form fits a shortcut field.
pub(super) fn default_hands_free(hold: &Combo, platform: Platform) -> String {
    let hold = hold.display(platform);
    format!("{hold} ×2 or {hold}+Space")
}
