//! The text a Chatterbox model reads: English words and the marks it reads as pauses, cut into
//! short pieces. Each piece is spoken on its own, so the first audio comes after the first
//! piece, and the model never reads so much at once that it loses its place.

use crate::numbers;
use crate::phonemes::fold;

/// The tags Turbo reads as a sound, such as `[laugh]`, not as words.
const TAGS: [&str; 19] = [
    "angry",
    "fear",
    "surprised",
    "whispering",
    "advertisement",
    "dramatic",
    "narration",
    "crying",
    "happy",
    "sarcastic",
    "clear throat",
    "sigh",
    "shush",
    "cough",
    "groan",
    "sniff",
    "gasp",
    "chuckle",
    "laugh",
];
/// Abbreviations read as their words. The dot after them ends no sentence.
const ABBREVIATIONS: [(&str, &str); 10] = [
    ("mr", "Mister"),
    ("mrs", "Missus"),
    ("ms", "Miz"),
    ("dr", "Doctor"),
    ("prof", "Professor"),
    ("st", "Saint"),
    ("jr", "Junior"),
    ("sr", "Senior"),
    ("vs", "versus"),
    ("approx", "approximately"),
];

/// The most words in one piece: about 12 seconds of speech.
const MAX_WORDS: usize = 30;
/// A first sentence longer than this is cut, so the first audio comes soon.
const FIRST_SENTENCE: usize = 12;
/// Where a first sentence without a clause mark is cut.
const FIRST_WORDS: usize = 8;
/// A cut at a clause mark leaves at least this many words before it.
const MIN_CLAUSE: usize = 3;

const SENTENCE_PAUSE_MS: u32 = 280;
const PARAGRAPH_PAUSE_MS: u32 = 550;
const CLAUSE_PAUSE_MS: u32 = 120;
const CUT_PAUSE_MS: u32 = 40;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    /// The silence after the piece, at speed 1.
    pub pause_ms: u32,
}

/// The pieces of `text` in order. Empty when the text has no word.
pub fn pieces(text: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    for (sentence, pause_ms) in sentences(&normalize(text)) {
        let words: Vec<&str> = sentence
            .split(' ')
            .filter(|word| !word.is_empty())
            .collect();
        let first = pieces.is_empty();
        cut(&words, first, true, pause_ms, &mut pieces);
    }
    pieces
}

/// The text as words one space apart, the marks `.` `!` `?` `,` right after a word, and a line
/// break where the text breaks a line. Numbers and symbols become words; letters of other
/// scripts and other symbols go.
fn normalize(text: &str) -> String {
    let chars: Vec<char> = text.chars().map(fold).collect();
    let mut out = String::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_ascii_digit() {
            let (words, end) = numbers::read(&chars, at);
            push_word(&mut out, &words);
            at = end;
            continue;
        }
        if c.is_alphabetic() {
            let end = word_end(&chars, at);
            let word: String = chars[at..end].iter().collect();
            at = end;
            if !word
                .chars()
                .all(|c| c.is_ascii_alphabetic() || matches!(c, '\'' | '-'))
            {
                continue;
            }
            let lower = word.to_ascii_lowercase();
            if chars.get(at) == Some(&'.')
                && let Some((_, long)) = ABBREVIATIONS.iter().find(|(short, _)| *short == lower)
            {
                push_word(&mut out, long);
                at += 1;
                continue;
            }
            push_word(&mut out, &word);
            continue;
        }
        let next = chars.get(at + 1).copied();
        match c {
            '[' => {
                if let Some((tag, end)) = tag_at(&chars, at) {
                    push_word(&mut out, &format!("[{tag}]"));
                    at = end;
                    continue;
                }
            }
            '.' if next == Some('.') => {
                while chars.get(at + 1) == Some(&'.') {
                    at += 1;
                }
                push_mark(&mut out, ',');
            }
            '…' | ',' | ';' | ':' | '—' | '(' | ')' => push_mark(&mut out, ','),
            '.' | '!' | '?' => push_mark(&mut out, c),
            '-' if next.is_some_and(|next| next.is_ascii_digit()) && !after_word(&chars, at) => {
                push_word(&mut out, "minus");
            }
            '-' if spaced(&chars, at) => push_mark(&mut out, ','),
            '&' => push_word(&mut out, "and"),
            '+' => push_word(&mut out, "plus"),
            '@' => push_word(&mut out, "at"),
            '%' => push_word(&mut out, "percent"),
            '\n' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
            }
            _ => {}
        }
        at += 1;
    }
    out
}

fn push_word(out: &mut String, word: &str) {
    if !(out.is_empty() || out.ends_with('\n') || out.ends_with(' ')) {
        out.push(' ');
    }
    out.push_str(word);
}

/// A mark joins the word before it. A mark with no word before it on its line goes, a comma
/// gives way to a stronger mark, and a word keeps one mark.
fn push_mark(out: &mut String, mark: char) {
    while out.ends_with(' ') {
        out.pop();
    }
    match out.chars().last() {
        None | Some('\n') => {}
        Some(',') if mark == ',' => {}
        Some(',') => {
            out.pop();
            out.push(mark);
        }
        Some('.' | '!' | '?') => {}
        Some(_) => out.push(mark),
    }
}

/// The tag such as `[laugh]` at `start`, and where the text goes on after it.
fn tag_at(chars: &[char], start: usize) -> Option<(&'static str, usize)> {
    let close = chars[start..].iter().position(|&c| c == ']')? + start;
    let inside: String = chars[start + 1..close].iter().collect();
    let inside = inside.trim().to_ascii_lowercase();
    TAGS.iter()
        .find(|tag| **tag == inside)
        .map(|tag| (*tag, close + 1))
}

fn after_word(chars: &[char], at: usize) -> bool {
    at.checked_sub(1)
        .and_then(|before| chars.get(before))
        .is_some_and(|c| c.is_alphanumeric())
}

/// A hyphen with a space or the end of the text on both sides, read as a dash.
fn spaced(chars: &[char], at: usize) -> bool {
    let before = at.checked_sub(1).and_then(|before| chars.get(before));
    let after = chars.get(at + 1);
    before.is_none_or(|c| c.is_whitespace()) && after.is_none_or(|c| c.is_whitespace())
}

/// A word runs over letters, and over an apostrophe or a hyphen between two letters.
fn word_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    while end < chars.len() {
        let c = chars[end];
        let joins = matches!(c, '\'' | '-')
            && end > start
            && chars.get(end + 1).is_some_and(|next| next.is_alphabetic());
        if !(c.is_alphabetic() || joins) {
            break;
        }
        end += 1;
    }
    end
}

/// The sentences of normalized text with the pause after each: a sentence ends at `.`, `!`, or
/// `?`, and at a line break.
fn sentences(normal: &str) -> Vec<(String, u32)> {
    let mut sentences: Vec<(String, u32)> = Vec::new();
    for line in normal.split('\n') {
        let words: Vec<&str> = line.split(' ').filter(|word| !word.is_empty()).collect();
        if words.is_empty() {
            if let Some((_, pause)) = sentences.last_mut() {
                *pause = PARAGRAPH_PAUSE_MS;
            }
            continue;
        }
        let mut current: Vec<&str> = Vec::new();
        for word in words {
            current.push(word);
            if word.ends_with(['.', '!', '?']) {
                sentences.push((current.join(" "), SENTENCE_PAUSE_MS));
                current.clear();
            }
        }
        if !current.is_empty() {
            sentences.push((current.join(" "), SENTENCE_PAUSE_MS));
        }
    }
    sentences
}

/// Cuts one sentence into pieces: whole when it is short, else at a clause mark, else into
/// even parts. `start` is true while `words` begin the sentence.
fn cut(words: &[&str], first: bool, start: bool, pause_ms: u32, out: &mut Vec<Piece>) {
    if words.is_empty() {
        return;
    }
    let limit = if first { FIRST_SENTENCE } else { MAX_WORDS };
    if words.len() <= limit {
        out.push(Piece {
            text: finished(words, start, true),
            pause_ms,
        });
        return;
    }
    let clause = |range: std::ops::RangeInclusive<usize>| {
        let mut ends = range.filter(|&count| words[count - 1].ends_with(','));
        if first { ends.next() } else { ends.next_back() }
    };
    let (count, pause) = match clause(MIN_CLAUSE..=limit.min(words.len() - 1)) {
        Some(count) => (count, CLAUSE_PAUSE_MS),
        None if first => (FIRST_WORDS, CUT_PAUSE_MS),
        None => {
            let parts = words.len().div_ceil(MAX_WORDS);
            (words.len().div_ceil(parts), CUT_PAUSE_MS)
        }
    };
    out.push(Piece {
        text: finished(&words[..count], start, false),
        pause_ms: pause,
    });
    cut(&words[count..], false, false, pause_ms, out);
}

/// The words of a piece as the model reads them best: a capital first letter when the piece
/// starts a sentence, and a mark at the end, `.` after the end of a sentence and `,` after a
/// cut.
fn finished(words: &[&str], start: bool, end: bool) -> String {
    let mut text = words.join(" ");
    if let Some(first) = text.chars().next()
        && start
        && first.is_ascii_lowercase()
    {
        text.replace_range(..1, &first.to_ascii_uppercase().to_string());
    }
    if !text.ends_with(['.', '!', '?', ',']) {
        text.push(if end { '.' } else { ',' });
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(text: &str) -> Vec<String> {
        pieces(text).into_iter().map(|piece| piece.text).collect()
    }

    #[test]
    fn numbers_symbols_and_abbreviations_become_words() {
        assert_eq!(
            normalize("Dr. Smith paid $45.99 & 20% more at 3:30 — really…"),
            "Doctor Smith paid forty five dollars and ninety nine cents and twenty percent more \
             at three thirty, really,"
        );
        assert_eq!(normalize("“Don’t” stop - go!"), "Don't stop, go!");
        assert_eq!(
            normalize("It was -5 degrees, (very) cold."),
            "It was minus five degrees, very, cold."
        );
    }

    #[test]
    fn tags_stay_and_other_scripts_and_symbols_go() {
        assert_eq!(
            normalize("Oh [Chuckle] that's [funny] *great* 日本 #1"),
            "Oh [chuckle] that's funny great one"
        );
        assert_eq!(normalize("Café naïve"), "Cafe naive");
        assert_eq!(normalize("!!, Hello,, world?."), "Hello, world?");
    }

    #[test]
    fn sentences_end_at_marks_and_lines() {
        let pieces = pieces("Hello there. How are you?\nFine\n\nNext part");
        let got: Vec<(&str, u32)> = pieces
            .iter()
            .map(|piece| (piece.text.as_str(), piece.pause_ms))
            .collect();
        assert_eq!(
            got,
            [
                ("Hello there.", SENTENCE_PAUSE_MS),
                ("How are you?", SENTENCE_PAUSE_MS),
                ("Fine.", PARAGRAPH_PAUSE_MS),
                ("Next part.", SENTENCE_PAUSE_MS),
            ]
        );
        assert!(pieces_of_nothing().is_empty());
    }

    fn pieces_of_nothing() -> Vec<Piece> {
        pieces(" *** \n\n -- ")
    }

    #[test]
    fn a_long_first_sentence_is_cut_early_at_a_clause() {
        let text = "When the meeting starts, everyone should bring the report and the slides \
                    for the review.";
        assert_eq!(
            texts(text),
            [
                "When the meeting starts,",
                "everyone should bring the report and the slides for the review."
            ]
        );
        let plain = "One two three four five six seven eight nine ten eleven twelve thirteen.";
        assert_eq!(
            texts(plain),
            [
                "One two three four five six seven eight,",
                "nine ten eleven twelve thirteen."
            ]
        );
        assert_eq!(
            texts("Short first sentence here."),
            ["Short first sentence here."]
        );
    }

    #[test]
    fn later_sentences_are_cut_at_the_last_clause_that_fits_or_evenly() {
        let long = format!("Start. {}", "word ".repeat(70).trim_end());
        let pieces = pieces(&long);
        assert_eq!(pieces.len(), 4);
        assert!(
            pieces[1..]
                .iter()
                .all(|piece| piece.text.split(' ').count() <= MAX_WORDS)
        );
        let counts: Vec<usize> = pieces[1..]
            .iter()
            .map(|piece| piece.text.split(' ').count())
            .collect();
        assert_eq!(counts, [24, 23, 23]);
        assert_eq!(pieces[1].pause_ms, CUT_PAUSE_MS);
        assert!(pieces[3].text.ends_with("word."));

        let clauses = format!(
            "Start. {}, {}.",
            "a ".repeat(20).trim_end(),
            "b ".repeat(20).trim_end()
        );
        let got = texts(&clauses);
        assert_eq!(got.len(), 3);
        assert!(got[1].ends_with("a,") && got[2].starts_with('b'));
    }
}
