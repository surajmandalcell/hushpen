//! Which key chord pastes into which app, and which selection carries the text.

use serde_json::Value;
use std::collections::BTreeMap;

/// A paste key chord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    CtrlV,
    CtrlShiftV,
    ShiftInsert,
    CmdV,
}

impl Chord {
    pub fn key(self) -> &'static str {
        match self {
            Chord::CtrlV => "ctrl+v",
            Chord::CtrlShiftV => "ctrl+shift+v",
            Chord::ShiftInsert => "shift+insert",
            Chord::CmdV => "cmd+v",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let flat: String = text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase();
        Some(match flat.as_str() {
            "ctrl+v" => Chord::CtrlV,
            "ctrl+shift+v" => Chord::CtrlShiftV,
            "shift+insert" => Chord::ShiftInsert,
            "cmd+v" => Chord::CmdV,
            _ => return None,
        })
    }

    /// Shift+Insert pastes the PRIMARY selection (xterm); every other chord pastes CLIPBOARD.
    pub fn selection(self) -> Selection {
        match self {
            Chord::ShiftInsert => Selection::Primary,
            _ => Selection::Clipboard,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Clipboard,
    Primary,
}

impl Selection {
    pub fn key(self) -> &'static str {
        match self {
            Selection::Clipboard => "clipboard",
            Selection::Primary => "primary",
        }
    }
}

/// How the text reaches the target app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Paste(Chord),
    /// Put the text on the clipboard and send no key.
    CopyOnly,
}

/// The `insert.appChords` setting: a WM_CLASS part or bundle id (any case) to a chord or
/// `copy-only`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Overrides(BTreeMap<String, Method>);

impl Overrides {
    /// Reads the setting value. Entries that are not a chord or `copy-only` are ignored.
    pub fn from_value(value: &Value) -> Self {
        let Some(map) = value.as_object() else {
            return Self::default();
        };
        let entries = map.iter().filter_map(|(app, chord)| {
            let chord = chord.as_str()?.trim();
            let method = if chord.eq_ignore_ascii_case("copy-only") {
                Method::CopyOnly
            } else {
                Method::Paste(Chord::parse(chord)?)
            };
            Some((app.trim().to_lowercase(), method))
        });
        Self(entries.collect())
    }

    fn find(&self, classes: &[String]) -> Option<Method> {
        classes
            .iter()
            .find_map(|class| self.0.get(&class.to_lowercase()).copied())
    }
}

/// What the table says for one `WM_CLASS` word, lower case.
fn terminal_chord(class: &str) -> Option<Chord> {
    const CTRL_SHIFT: [&str; 9] = [
        "konsole",
        "alacritty",
        "kitty",
        "xfce4-terminal",
        "tilix",
        "terminator",
        "wezterm",
        "urxvt",
        "urxvtd",
    ];
    if matches!(class, "xterm" | "uxterm") {
        Some(Chord::ShiftInsert)
    } else if class.starts_with("gnome-terminal")
        || class.ends_with(".wezterm")
        || CTRL_SHIFT.contains(&class)
    {
        Some(Chord::CtrlShiftV)
    } else {
        None
    }
}

/// The method for a target on X11. `classes` are the `WM_CLASS` words (instance and class).
/// A setting wins over the table.
pub fn choose_x11(classes: &[String], overrides: &Overrides) -> Method {
    if let Some(method) = overrides.find(classes) {
        return method;
    }
    classes
        .iter()
        .find_map(|class| terminal_chord(&class.to_lowercase()))
        .map_or(Method::Paste(Chord::CtrlV), Method::Paste)
}

/// The method for a target on macOS, named by bundle id. Every app takes Cmd+V; a setting can
/// still turn paste off.
pub fn choose_mac(bundle_id: &str, overrides: &Overrides) -> Method {
    match overrides.find(&[bundle_id.to_owned()]) {
        Some(Method::CopyOnly) => Method::CopyOnly,
        _ => Method::Paste(Chord::CmdV),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn classes(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn pick(words: &[&str]) -> Method {
        choose_x11(&classes(words), &Overrides::default())
    }

    #[test]
    fn gnome_terminal_konsole_alacritty_and_kitty_use_ctrl_shift_v() {
        for words in [
            ["gnome-terminal-server", "Gnome-terminal"],
            ["konsole", "konsole"],
            ["Alacritty", "Alacritty"],
            ["kitty", "kitty"],
        ] {
            assert_eq!(pick(&words), Method::Paste(Chord::CtrlShiftV), "{words:?}");
        }
    }

    #[test]
    fn the_other_terminals_of_the_table_use_ctrl_shift_v() {
        for words in [
            ["xfce4-terminal", "Xfce4-terminal"],
            ["tilix", "Tilix"],
            ["terminator", "Terminator"],
            ["org.wezfurlong.wezterm", "org.wezfurlong.wezterm"],
            ["urxvt", "URxvt"],
        ] {
            assert_eq!(pick(&words), Method::Paste(Chord::CtrlShiftV), "{words:?}");
        }
    }

    #[test]
    fn xterm_and_xterm_class_use_shift_insert_with_primary() {
        for words in [["xterm", "XTerm"], ["uxterm", "UXTerm"], ["XTerm", "XTerm"]] {
            let method = pick(&words);
            assert_eq!(method, Method::Paste(Chord::ShiftInsert), "{words:?}");
            assert_eq!(Chord::ShiftInsert.selection(), Selection::Primary);
        }
    }

    #[test]
    fn every_other_class_uses_ctrl_v_with_the_clipboard() {
        for words in [
            &["gtk-target.py", "Gtk-target.py"][..],
            &["firefox", "Firefox"],
            &[],
        ] {
            assert_eq!(pick(words), Method::Paste(Chord::CtrlV), "{words:?}");
        }
        assert_eq!(Chord::CtrlV.selection(), Selection::Clipboard);
        assert_eq!(Chord::CtrlShiftV.selection(), Selection::Clipboard);
    }

    #[test]
    fn an_app_chords_entry_wins_over_the_table() {
        let overrides = Overrides::from_value(&json!({"xterm": "ctrl+shift+v"}));
        assert_eq!(
            choose_x11(&classes(&["xterm", "XTerm"]), &overrides),
            Method::Paste(Chord::CtrlShiftV)
        );
    }

    #[test]
    fn an_entry_can_name_the_class_in_any_case_and_can_turn_paste_off() {
        let overrides = Overrides::from_value(&json!({
            "Firefox": "Copy-Only",
            "Kitty": " Ctrl + V ",
        }));
        assert_eq!(
            choose_x11(&classes(&["Navigator", "firefox"]), &overrides),
            Method::CopyOnly
        );
        assert_eq!(
            choose_x11(&classes(&["kitty", "kitty"]), &overrides),
            Method::Paste(Chord::CtrlV)
        );
    }

    #[test]
    fn entries_that_are_not_chords_are_ignored() {
        let overrides = Overrides::from_value(&json!({"xterm": "alt+q", "kitty": 3}));
        assert_eq!(
            choose_x11(&classes(&["xterm", "XTerm"]), &overrides),
            Method::Paste(Chord::ShiftInsert)
        );
        assert_eq!(Overrides::from_value(&json!([1, 2])), Overrides::default());
    }

    #[test]
    fn the_mac_always_pastes_with_cmd_v_unless_the_app_is_copy_only() {
        let overrides = Overrides::from_value(&json!({"com.apple.Terminal": "copy-only"}));
        assert_eq!(
            choose_mac("com.apple.TextEdit", &overrides),
            Method::Paste(Chord::CmdV)
        );
        assert_eq!(
            choose_mac("com.apple.terminal", &overrides),
            Method::CopyOnly
        );
    }

    #[test]
    fn chord_names_round_trip() {
        for chord in [
            Chord::CtrlV,
            Chord::CtrlShiftV,
            Chord::ShiftInsert,
            Chord::CmdV,
        ] {
            assert_eq!(Chord::parse(chord.key()), Some(chord));
        }
    }
}
