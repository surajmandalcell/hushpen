//! Cleaning of the raw engine text before anyone sees it.

/// Longest bracketed group that still counts as a marker. A bracket pair with more text than
/// this is left alone.
const MAX_MARKER_CHARS: usize = 60;

/// Words that mark a parenthesized or starred group as a non-speech marker. Square brackets
/// need no list: whisper writes only markers inside them.
const MARKER_WORDS: [&str; 14] = [
    "silence",
    "silent",
    "blank",
    "blank audio",
    "no audio",
    "music",
    "inaudible",
    "applause",
    "laughter",
    "noise",
    "background noise",
    "speaking in foreign language",
    "static",
    "sound",
];

/// Removes the markers whisper writes for sound that is not speech: `[BLANK_AUDIO]`,
/// `[MUSIC]`, `[ Silence ]`, `(silence)`, `*music*`, and music notes. The words around them
/// stay, with one space where a marker stood. A text that only held markers gives an empty
/// string.
pub fn strip_blank_markers(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut kept = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let character = chars[index];
        if matches!(character, '♪' | '♫' | '♬') {
            index += 1;
            continue;
        }
        if let Some(close) = marker_end(&chars, index) {
            kept.push(' ');
            index = close + 1;
            continue;
        }
        kept.push(character);
        index += 1;
    }
    tidy(&kept)
}

/// True when the text has no words left after [`strip_blank_markers`].
pub fn is_blank(text: &str) -> bool {
    !strip_blank_markers(text)
        .chars()
        .any(|character| character.is_alphanumeric())
}

/// The index of the closing character when a marker starts at `start`.
fn marker_end(chars: &[char], start: usize) -> Option<usize> {
    let close = match chars[start] {
        '[' => ']',
        '(' => ')',
        '*' => '*',
        _ => return None,
    };
    let limit = (start + 1 + MAX_MARKER_CHARS).min(chars.len());
    let end = (start + 1..limit).find(|&at| chars[at] == close)?;
    let inner: String = chars[start + 1..end].iter().collect();
    if inner.contains('\n') {
        return None;
    }
    let is_marker = match chars[start] {
        '[' => !inner.trim().is_empty(),
        _ => MARKER_WORDS.contains(&normalize(&inner).as_str()),
    };
    is_marker.then_some(end)
}

fn normalize(inner: &str) -> String {
    inner
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Collapses the gaps a removed marker leaves and trims the ends.
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut gap = false;
    for character in text.chars() {
        if character == ' ' || character == '\t' {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() && !matches!(character, '.' | ',' | '!' | '?' | ';' | ':') {
            out.push(' ');
        }
        gap = false;
        out.push(character);
    }
    out.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_marker_alone_leaves_nothing() {
        for marker in [
            "[BLANK_AUDIO]",
            "[MUSIC]",
            "(silence)",
            "[ Silence ]",
            "[Music]",
            "(music)",
            "*music*",
            "[BLANK AUDIO]",
            " [BLANK_AUDIO] ",
            "♪",
        ] {
            assert_eq!(strip_blank_markers(marker), "", "{marker}");
            assert!(is_blank(marker), "{marker}");
        }
    }

    #[test]
    fn markers_mixed_with_words_leave_the_words() {
        assert_eq!(
            strip_blank_markers("The quick brown fox. [BLANK_AUDIO]"),
            "The quick brown fox."
        );
        assert_eq!(
            strip_blank_markers("[MUSIC] Hello there (silence) and goodbye [BLANK_AUDIO]"),
            "Hello there and goodbye"
        );
        assert_eq!(strip_blank_markers("Hello [Music], world"), "Hello, world");
    }

    #[test]
    fn spoken_words_in_parentheses_stay() {
        assert_eq!(
            strip_blank_markers("Call me (tomorrow) at noon"),
            "Call me (tomorrow) at noon"
        );
        assert_eq!(strip_blank_markers("2 * 3 * 4"), "2 * 3 * 4");
    }

    #[test]
    fn an_unclosed_bracket_is_not_a_marker() {
        assert_eq!(strip_blank_markers("a [ b"), "a [ b");
    }

    #[test]
    fn plain_text_is_not_blank_and_empty_text_is() {
        assert!(!is_blank("Hello."));
        assert!(is_blank(""));
        assert!(is_blank("   "));
        assert!(!is_blank("[MUSIC] one"));
    }

    #[test]
    fn accents_survive() {
        assert_eq!(
            strip_blank_markers("Él dijo [BLANK_AUDIO] adiós"),
            "Él dijo adiós"
        );
    }
}
