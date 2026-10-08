//! How keys are written: in the setting text, and for the user on each system.

use super::{Combo, Key, Modifier, Platform};

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

pub(super) fn setting(keys: &[Key], platform: Platform) -> String {
    keys.iter()
        .map(|key| match key {
            Key::Fn => "Fn".to_owned(),
            Key::Any(modifier) => setting_name(*modifier, platform).to_owned(),
            Key::Right(modifier) => format!("Right{}", setting_name(*modifier, platform)),
            Key::Char(c) => c.to_ascii_uppercase().to_string(),
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
                Key::Char(c) => c.to_ascii_uppercase().to_string(),
            })
            .collect::<Vec<_>>()
            .join("+"),
        Platform::MacOs => mac_display(keys),
    }
}

/// A chord of modifiers and a letter reads as `⌃⌘V`. A side or the Fn key needs words, so those
/// read as `Right ⌥+Right ⇧`.
fn mac_display(keys: &[Key]) -> String {
    let plain = keys
        .iter()
        .all(|key| matches!(key, Key::Any(_) | Key::Char(_)));
    let parts = keys.iter().map(|key| match key {
        Key::Fn => "Fn".to_owned(),
        Key::Any(modifier) => symbol(*modifier).to_owned(),
        Key::Right(modifier) => format!("Right {}", symbol(*modifier)),
        Key::Char(c) => c.to_ascii_uppercase().to_string(),
    });
    parts.collect::<Vec<_>>().join(if plain { "" } else { "+" })
}

pub(super) fn default_hands_free(hold: &Combo, platform: Platform) -> String {
    let hold = hold.display(platform);
    format!("Double tap {hold}, or {hold}+Space")
}
