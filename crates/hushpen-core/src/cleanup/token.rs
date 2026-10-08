//! The tokens the rule pass works on, and the text they come from and go back to.

/// The punctuation the pass splits off the end of a word.
const PUNCT: [char; 7] = ['.', ',', '?', '!', ';', ':', '…'];

/// Words whose final dot is part of the word, so the next word does not start a sentence.
const ABBREVIATIONS: [&str; 11] = [
    "e.g", "i.e", "mr", "mrs", "ms", "dr", "prof", "vs", "st", "jr", "sr",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Tok {
    /// A word with any quotes or brackets still attached.
    Word(String),
    /// One mark: `,` `.` `?` `!` `;` `:`, a question or exclamation run, or an ellipsis.
    Punct(String),
    /// `1` for a new line, `2` for a new paragraph.
    Break(u8),
}

impl Tok {
    pub(super) fn word(&self) -> Option<&str> {
        match self {
            Tok::Word(word) => Some(word),
            _ => None,
        }
    }

    pub(super) fn is_comma(&self) -> bool {
        matches!(self, Tok::Punct(mark) if mark == ",")
    }

    pub(super) fn is_sentence_end(&self) -> bool {
        matches!(self, Tok::Punct(mark) if rank(mark) >= STOP_RANK && !is_ellipsis(mark))
    }

    pub(super) fn is_ellipsis(&self) -> bool {
        matches!(self, Tok::Punct(mark) if is_ellipsis(mark))
    }

    /// The lowercase form of a word without the quotes and brackets around it.
    pub(super) fn key(&self) -> Option<String> {
        self.word().map(key)
    }
}

const STOP_RANK: u8 = 3;

fn rank(mark: &str) -> u8 {
    if mark.contains(['?', '!']) {
        4
    } else if mark == "." || is_ellipsis(mark) {
        STOP_RANK
    } else if mark == ";" || mark == ":" {
        2
    } else {
        1
    }
}

fn is_ellipsis(mark: &str) -> bool {
    mark.contains('…') || (mark.len() >= 2 && mark.chars().all(|c| c == '.'))
}

pub(super) fn key(word: &str) -> String {
    word.replace('’', "'")
        .to_lowercase()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_owned()
}

/// One mark for a run such as `?!`, `.,`, or `...`.
fn canonical(run: &str) -> String {
    if is_ellipsis(run) {
        run.to_owned()
    } else if run.contains(['?', '!']) {
        run.chars().filter(|c| matches!(c, '?' | '!')).collect()
    } else if run.contains('.') {
        ".".to_owned()
    } else {
        run.chars().take(1).collect()
    }
}

pub(super) fn tokenize(raw: &str) -> Vec<Tok> {
    let mut tokens = Vec::new();
    for chunk in raw.split_whitespace() {
        let head = chunk.trim_end_matches(PUNCT);
        let run = &chunk[head.len()..];
        if run.is_empty() {
            tokens.push(Tok::Word(chunk.to_owned()));
        } else if head.is_empty() {
            tokens.push(Tok::Punct(canonical(run)));
        } else if run == "." && ABBREVIATIONS.contains(&head.to_lowercase().as_str()) {
            tokens.push(Tok::Word(chunk.to_owned()));
        } else {
            tokens.push(Tok::Word(head.to_owned()));
            tokens.push(Tok::Punct(canonical(run)));
        }
    }
    tokens
}

/// Drops marks that start the text or a line, merges neighbouring marks into the stronger one,
/// merges neighbouring breaks, and drops breaks at the ends.
pub(super) fn normalize(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    for token in tokens {
        match (out.last_mut(), token) {
            (None, Tok::Punct(_) | Tok::Break(_)) | (Some(Tok::Break(_)), Tok::Punct(_)) => {}
            (Some(Tok::Punct(last)), Tok::Punct(mark)) => {
                if rank(&mark) > rank(last) {
                    *last = mark;
                }
            }
            (Some(Tok::Break(last)), Tok::Break(more)) => *last = (*last + more).min(2),
            (_, token) => out.push(token),
        }
    }
    while matches!(out.last(), Some(Tok::Break(_))) {
        out.pop();
    }
    out
}

/// The text for the tokens: single spaces, no space before a mark, capital letters at the start
/// of each sentence and line, and a final period after a plain word.
pub(super) fn render(tokens: Vec<Tok>) -> String {
    let tokens = normalize(tokens);
    let has_break = tokens.iter().any(|t| matches!(t, Tok::Break(_)));
    let mut text = String::new();
    let mut capital = true;
    let mut after_break = true;
    for token in &tokens {
        match token {
            Tok::Word(word) => {
                if !after_break && !text.is_empty() {
                    text.push(' ');
                }
                push_word(&mut text, word, capital);
                capital = false;
                after_break = false;
            }
            Tok::Punct(mark) => {
                text.push_str(mark);
                capital = token.is_sentence_end();
            }
            Tok::Break(count) => {
                text.extend(std::iter::repeat_n('\n', usize::from(*count)));
                capital = true;
                after_break = true;
            }
        }
    }
    if !has_break
        && let Some(Tok::Word(word)) = tokens.last()
        && word.chars().next_back().is_some_and(char::is_alphanumeric)
    {
        text.push('.');
    }
    text
}

fn push_word(text: &mut String, word: &str, capital: bool) {
    let lower = word.replace('’', "'").to_lowercase();
    if matches!(lower.as_str(), "i" | "i'm" | "i'll" | "i've" | "i'd") {
        text.push('I');
        text.push_str(&word[1..]);
        return;
    }
    if !capital || word.contains(['@', '/']) {
        text.push_str(word);
        return;
    }
    let mut chars = word.chars();
    for first in chars.by_ref() {
        if first.is_alphabetic() {
            text.extend(first.to_uppercase());
            text.push_str(chars.as_str());
            return;
        }
        text.push(first);
        if first.is_alphanumeric() {
            break;
        }
    }
    text.push_str(chars.as_str());
}
