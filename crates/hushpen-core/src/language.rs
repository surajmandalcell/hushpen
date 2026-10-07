//! The languages whisper can decode, and the `dictation.language` setting value that picks one.

/// The setting value that asks the engine to detect the language.
pub const AUTO: &str = "auto";

/// Every language whisper knows: code and English name, in whisper's own order. The engine
/// crate checks this list against the linked whisper build.
pub const LANGUAGES: [(&str, &str); 100] = [
    ("en", "English"),
    ("zh", "Chinese"),
    ("de", "German"),
    ("es", "Spanish"),
    ("ru", "Russian"),
    ("ko", "Korean"),
    ("fr", "French"),
    ("ja", "Japanese"),
    ("pt", "Portuguese"),
    ("tr", "Turkish"),
    ("pl", "Polish"),
    ("ca", "Catalan"),
    ("nl", "Dutch"),
    ("ar", "Arabic"),
    ("sv", "Swedish"),
    ("it", "Italian"),
    ("id", "Indonesian"),
    ("hi", "Hindi"),
    ("fi", "Finnish"),
    ("vi", "Vietnamese"),
    ("he", "Hebrew"),
    ("uk", "Ukrainian"),
    ("el", "Greek"),
    ("ms", "Malay"),
    ("cs", "Czech"),
    ("ro", "Romanian"),
    ("da", "Danish"),
    ("hu", "Hungarian"),
    ("ta", "Tamil"),
    ("no", "Norwegian"),
    ("th", "Thai"),
    ("ur", "Urdu"),
    ("hr", "Croatian"),
    ("bg", "Bulgarian"),
    ("lt", "Lithuanian"),
    ("la", "Latin"),
    ("mi", "Maori"),
    ("ml", "Malayalam"),
    ("cy", "Welsh"),
    ("sk", "Slovak"),
    ("te", "Telugu"),
    ("fa", "Persian"),
    ("lv", "Latvian"),
    ("bn", "Bengali"),
    ("sr", "Serbian"),
    ("az", "Azerbaijani"),
    ("sl", "Slovenian"),
    ("kn", "Kannada"),
    ("et", "Estonian"),
    ("mk", "Macedonian"),
    ("br", "Breton"),
    ("eu", "Basque"),
    ("is", "Icelandic"),
    ("hy", "Armenian"),
    ("ne", "Nepali"),
    ("mn", "Mongolian"),
    ("bs", "Bosnian"),
    ("kk", "Kazakh"),
    ("sq", "Albanian"),
    ("sw", "Swahili"),
    ("gl", "Galician"),
    ("mr", "Marathi"),
    ("pa", "Punjabi"),
    ("si", "Sinhala"),
    ("km", "Khmer"),
    ("sn", "Shona"),
    ("yo", "Yoruba"),
    ("so", "Somali"),
    ("af", "Afrikaans"),
    ("oc", "Occitan"),
    ("ka", "Georgian"),
    ("be", "Belarusian"),
    ("tg", "Tajik"),
    ("sd", "Sindhi"),
    ("gu", "Gujarati"),
    ("am", "Amharic"),
    ("yi", "Yiddish"),
    ("lo", "Lao"),
    ("uz", "Uzbek"),
    ("fo", "Faroese"),
    ("ht", "Haitian Creole"),
    ("ps", "Pashto"),
    ("tk", "Turkmen"),
    ("nn", "Nynorsk"),
    ("mt", "Maltese"),
    ("sa", "Sanskrit"),
    ("lb", "Luxembourgish"),
    ("my", "Myanmar"),
    ("bo", "Tibetan"),
    ("tl", "Tagalog"),
    ("mg", "Malagasy"),
    ("as", "Assamese"),
    ("tt", "Tatar"),
    ("haw", "Hawaiian"),
    ("ln", "Lingala"),
    ("ha", "Hausa"),
    ("ba", "Bashkir"),
    ("jw", "Javanese"),
    ("su", "Sundanese"),
    ("yue", "Cantonese"),
];

/// The English name of a language code, or `None` for `auto` and unknown codes.
pub fn name(code: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(known, _)| *known == code)
        .map(|(_, name)| *name)
}

/// True for `auto` and for every code in [`LANGUAGES`]. Anything else is not a valid
/// `dictation.language`.
pub fn is_setting_value(value: &str) -> bool {
    value == AUTO || name(value).is_some()
}

/// The label for a setting value: `Auto` or the language name.
pub fn label(code: &str) -> &'static str {
    if code == AUTO {
        "Auto"
    } else {
        name(code).unwrap_or("Auto")
    }
}

/// The codes in the order a picker shows them after Auto: the recent ones first (known codes
/// only, no repeats), then English, then every other language by name.
pub fn picker_order(recent: &[String]) -> Vec<&'static str> {
    let mut order: Vec<&'static str> = Vec::with_capacity(LANGUAGES.len());
    for code in recent {
        if let Some((known, _)) = LANGUAGES.iter().find(|(known, _)| known == code)
            && !order.contains(known)
        {
            order.push(known);
        }
    }
    let mut rest: Vec<&(&str, &str)> = LANGUAGES
        .iter()
        .filter(|(code, _)| !order.contains(code))
        .collect();
    rest.sort_by_key(|(code, name)| (*code != "en", *name));
    order.extend(rest.into_iter().map(|(code, _)| *code));
    order
}

/// The recent list after choosing `code`: it goes first, repeats leave, at most five stay.
pub fn push_recent(recent: &[String], code: &str) -> Vec<String> {
    if code == AUTO {
        return recent.to_vec();
    }
    let mut next = vec![code.to_owned()];
    next.extend(recent.iter().filter(|old| *old != code).cloned());
    next.truncate(5);
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_has_at_least_90_unique_languages() {
        assert!(LANGUAGES.len() >= 90);
        for (index, (code, _)) in LANGUAGES.iter().enumerate() {
            assert!(
                LANGUAGES[index + 1..]
                    .iter()
                    .all(|(other, _)| other != code),
                "{code}"
            );
        }
    }

    #[test]
    fn spanish_german_and_japanese_are_there() {
        assert_eq!(name("es"), Some("Spanish"));
        assert_eq!(name("de"), Some("German"));
        assert_eq!(name("ja"), Some("Japanese"));
        assert_eq!(name("xx"), None);
    }

    #[test]
    fn only_auto_and_known_codes_are_setting_values() {
        assert!(is_setting_value("auto"));
        assert!(is_setting_value("en"));
        assert!(is_setting_value("yue"));
        assert!(!is_setting_value("xx-invalid"));
        assert!(!is_setting_value(""));
        assert!(!is_setting_value("EN"));
    }

    #[test]
    fn the_picker_puts_recent_codes_first_then_english_then_the_rest_by_name() {
        let order = picker_order(&["ja".to_owned(), "xx".to_owned(), "es".to_owned()]);
        assert_eq!(&order[..4], ["ja", "es", "en", "af"]);
        assert_eq!(order.len(), LANGUAGES.len());
    }

    #[test]
    fn choosing_a_language_moves_it_to_the_front_and_keeps_five() {
        let recent: Vec<String> = ["a", "b", "c", "d", "e"].map(str::to_owned).to_vec();
        assert_eq!(push_recent(&recent, "c"), ["c", "a", "b", "d", "e"]);
        assert_eq!(push_recent(&recent, "z"), ["z", "a", "b", "c", "d"]);
        assert_eq!(push_recent(&recent, AUTO), recent);
    }
}
