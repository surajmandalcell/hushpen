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

/// The setting word of a named key, which `parse` reads back.
fn named_setting(named: Named) -> String {
    match named {
        Named::Tab => "Tab".into(),
        Named::Return => "Return".into(),
        Named::Backspace => "Backspace".into(),
        Named::Delete => "Delete".into(),
        Named::Up => "Up".into(),
        Named::Down => "Down".into(),
        Named::Left => "Left".into(),
        Named::Right => "Right".into(),
        Named::Home => "Home".into(),
        Named::End => "End".into(),
        Named::PageUp => "PageUp".into(),
        Named::PageDown => "PageDown".into(),
        Named::F(n) => format!("F{n}"),
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

fn linux_named(named: Named) -> String {
    match named {
        Named::Return => "Enter".into(),
        Named::PageUp => "Page Up".into(),
        Named::PageDown => "Page Down".into(),
        other => named_setting(other),
    }
}

/// The key symbols of macOS menus.
fn mac_named(named: Named) -> String {
    match named {
        Named::Tab => "⇥".into(),
        Named::Return => "↩".into(),
        Named::Backspace => "⌫".into(),
        Named::Delete => "⌦".into(),
        Named::Up => "↑".into(),
        Named::Down => "↓".into(),
        Named::Left => "←".into(),
        Named::Right => "→".into(),
        Named::Home => "↖".into(),
        Named::End => "↘".into(),
        Named::PageUp => "⇞".into(),
        Named::PageDown => "⇟".into(),
        Named::F(n) => format!("F{n}"),
    }
}

pub(super) fn display(keys: &[Key], platform: Platform) -> String {
    match platform {
        Platform::Linux => keys
            .iter()
            .map(|key| match key {
                Key::Fn => "Fn".to_owned(),
                Key::Any(modifier) => setting_name(*modifier, platform).to_owned(),
                Key::Right(modifier) => {
                    format!("Right {}", setting_name(*modifier, platform))
                }
                Key::Char(c) => char_word(*c),
                Key::Named(named) => linux_named(*named),
            })
            .collect::<Vec<_>>()
            .join("+"),
        Platform::MacOs => mac_display(keys),
    }
}

/// A chord of modifiers and one key reads as `⌃⌘V` or `⌘⇥`. A side or the Fn key needs words,
/// so those read as `Right ⌥+Right ⇧`.
fn mac_display(keys: &[Key]) -> String {
    let plain = keys
        .iter()
        .all(|key| matches!(key, Key::Any(_) | Key::Char(_) | Key::Named(_)));
    let parts = keys.iter().map(|key| match key {
        Key::Fn => "Fn".to_owned(),
        Key::Any(modifier) => symbol(*modifier).to_owned(),
        Key::Right(modifier) => format!("Right {}", symbol(*modifier)),
        Key::Char(' ') => "Space".to_owned(),
        Key::Char(c) => c.to_uppercase().collect(),
        Key::Named(named) => mac_named(*named),
    });
    parts.collect::<Vec<_>>().join(if plain { "" } else { "+" })
}

/// `×2` is the double tap. The short form fits a shortcut field.
pub(super) fn default_hands_free(hold: &Combo, platform: Platform) -> String {
    let hold = hold.display(platform);
    format!("{hold} ×2 or {hold}+Space")
}
