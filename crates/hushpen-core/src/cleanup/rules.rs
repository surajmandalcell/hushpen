//! The rule pass: spoken punctuation, fillers, "no wait" corrections, "scratch that", repeated
//! words, then capitals and spacing. Each stage takes tokens and returns tokens.

use super::token::{Tok, normalize, render, tokenize};

/// Which rules run. `cleanup.rules` off skips the whole pass, so it is not an option here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `cleanup.spokenPunctuation`.
    pub spoken_punctuation: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            spoken_punctuation: true,
        }
    }
}

/// Words a speaker says without meaning. `err` is not here: it is a real word.
const FILLERS: [&str; 10] = [
    "um", "umm", "uh", "uhh", "uhm", "er", "erm", "hmm", "hm", "mmm",
];

/// Words that may be said twice in a row on purpose.
const ALLOWED_DOUBLES: [&str; 13] = [
    "that", "had", "is", "was", "bye", "no", "ha", "very", "so", "yeah", "really", "knock", "go",
];

/// A spoken punctuation word after one of these names the thing ("the period of"), so it stays.
const DETERMINERS: [&str; 18] = [
    "the", "a", "an", "this", "these", "those", "my", "your", "his", "her", "its", "our", "their",
    "each", "every", "any", "same", "no",
];

/// Before "scratch that" these make it an ordinary phrase ("I need to scratch that").
const NOT_BEFORE_SCRATCH: [&str; 20] = [
    "to", "i", "you", "we", "they", "he", "she", "it", "can", "will", "would", "could", "should",
    "must", "not", "and", "then", "please", "just", "gonna",
];

const MAX_CORRECTION_WORDS: usize = 3;

/// Cleans one dictation. Empty input, and input that is only fillers, give an empty string.
pub fn clean(raw: &str, options: &Options) -> String {
    let mut tokens = tokenize(raw);
    if options.spoken_punctuation {
        tokens = spoken_punctuation(tokens);
    }
    let tokens = remove_fillers(normalize(tokens));
    let tokens = apply_corrections(normalize(tokens));
    let tokens = scratch_that(tokens);
    let tokens = collapse_repeats(normalize(tokens));
    render(tokens)
}

enum Spoken {
    Mark(&'static str),
    Break(u8),
}

fn spoken_command(tokens: &[Tok], at: usize) -> Option<(usize, Spoken)> {
    let first = tokens.get(at)?.word()?.to_lowercase();
    let second = tokens
        .get(at + 1)
        .and_then(Tok::word)
        .map(str::to_lowercase);
    Some(match (first.as_str(), second.as_deref()) {
        ("question", Some("mark")) => (2, Spoken::Mark("?")),
        ("exclamation", Some("mark" | "point")) => (2, Spoken::Mark("!")),
        ("full", Some("stop")) => (2, Spoken::Mark(".")),
        ("new", Some("line")) => (2, Spoken::Break(1)),
        ("new", Some("paragraph")) => (2, Spoken::Break(2)),
        ("comma", _) => (1, Spoken::Mark(",")),
        ("period", _) => (1, Spoken::Mark(".")),
        ("colon", _) => (1, Spoken::Mark(":")),
        ("semicolon", _) => (1, Spoken::Mark(";")),
        ("newline", _) => (1, Spoken::Break(1)),
        _ => return None,
    })
}

fn spoken_punctuation(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        if let Some((len, command)) = spoken_command(&tokens, at)
            && command_applies(&command, out.last())
        {
            out.push(match command {
                Spoken::Mark(mark) => Tok::Punct(mark.to_owned()),
                Spoken::Break(count) => Tok::Break(count),
            });
            at += len;
        } else {
            out.push(tokens[at].clone());
            at += 1;
        }
    }
    out
}

fn command_applies(command: &Spoken, previous: Option<&Tok>) -> bool {
    match previous {
        Some(Tok::Word(word)) => !DETERMINERS.contains(&super::token::key(word).as_str()),
        Some(Tok::Punct(_)) => true,
        Some(Tok::Break(_)) | None => matches!(command, Spoken::Break(_)),
    }
}

/// How a filler word was found: always a filler, or only where the speaker paused around it.
enum Filler {
    Always,
    WhenSet,
}

fn filler_at(tokens: &[Tok], at: usize) -> Option<(usize, Filler)> {
    let key = tokens.get(at)?.key()?;
    if FILLERS.contains(&key.as_str()) {
        return Some((1, Filler::Always));
    }
    match key.as_str() {
        "like" => Some((1, Filler::WhenSet)),
        "you" if tokens.get(at + 1).and_then(Tok::key).as_deref() == Some("know") => {
            Some((2, Filler::WhenSet))
        }
        _ => None,
    }
}

/// True where a mark, a line break, or an edge of the text stands.
fn is_pause(token: Option<&Tok>) -> bool {
    !matches!(token, Some(Tok::Word(_)))
}

fn remove_fillers(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        let Some((len, kind)) = filler_at(&tokens, at) else {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        };
        let after = tokens.get(at + len);
        if matches!(kind, Filler::WhenSet) && !(is_pause(out.last()) && is_pause(after)) {
            out.push(tokens[at].clone());
            at += 1;
            continue;
        }
        let own_mark = after.is_some_and(|token| {
            token.is_comma() || (matches!(kind, Filler::Always) && token.is_ellipsis())
        });
        let skip = usize::from(own_mark);
        let comma_before = out.last().is_some_and(Tok::is_comma);
        if comma_before && (own_mark || !matches!(after, Some(Tok::Word(_)))) {
            out.pop();
        }
        at += len + skip;
    }
    out
}

/// Index just after "no wait" or "no, wait", and an optional comma behind it.
fn no_wait_end(tokens: &[Tok], at: usize) -> Option<usize> {
    let mut next = at;
    if tokens.get(next)?.key().as_deref() != Some("no") {
        return None;
    }
    next += 1;
    if tokens.get(next).is_some_and(Tok::is_comma) {
        next += 1;
    }
    if tokens.get(next)?.key().as_deref() != Some("wait") {
        return None;
    }
    next += 1;
    if tokens.get(next).is_some_and(Tok::is_comma) {
        next += 1;
    }
    Some(next)
}

/// "X, no wait, Y": Y replaces the last word of X, or the last words when Y ends in the same
/// word as X ("3 pm, no wait, 4 pm"). Anything longer or looser stays as it was said.
fn apply_corrections(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        if tokens[at].is_comma()
            && let Some(start) = no_wait_end(&tokens, at + 1)
            && let Some(end) = correction_end(&tokens, start, &out)
        {
            let fix = &tokens[start..end];
            out.truncate(out.len() - fix.len());
            out.extend(fix.iter().cloned());
            at = end;
            continue;
        }
        out.push(tokens[at].clone());
        at += 1;
    }
    out
}

/// The end of the words Y after "no wait", when `out` ends in words Y can replace.
fn correction_end(tokens: &[Tok], start: usize, out: &[Tok]) -> Option<usize> {
    let end = start
        + tokens[start..]
            .iter()
            .take_while(|token| matches!(token, Tok::Word(_)))
            .count();
    let count = end - start;
    if count == 0 || count > MAX_CORRECTION_WORDS || count > out.len() {
        return None;
    }
    let replaced = &out[out.len() - count..];
    if !replaced.iter().all(|token| matches!(token, Tok::Word(_))) {
        return None;
    }
    let same_tail = replaced.last()?.key() == tokens[end - 1].key();
    (count == 1 || same_tail).then_some(end)
}

fn scratch_that_at(tokens: &[Tok], at: usize) -> bool {
    tokens.get(at).and_then(Tok::key).as_deref() == Some("scratch")
        && tokens.get(at + 1).and_then(Tok::key).as_deref() == Some("that")
}

fn scratch_that(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    let mut at = 0;
    while at < tokens.len() {
        if scratch_that_at(&tokens, at) && scratch_applies(out.last(), tokens.get(at + 2)) {
            if matches!(out.last(), Some(Tok::Punct(_))) {
                out.pop();
            }
            while out
                .last()
                .is_some_and(|token| !token.is_sentence_end() && !matches!(token, Tok::Break(_)))
            {
                out.pop();
            }
            at += 2;
            if matches!(tokens.get(at), Some(Tok::Punct(_))) {
                at += 1;
            }
            continue;
        }
        out.push(tokens[at].clone());
        at += 1;
    }
    out
}

/// "Scratch that" is a command when a mark or a line end follows it and a plain verb does not
/// lead into it.
fn scratch_applies(previous: Option<&Tok>, next: Option<&Tok>) -> bool {
    let after_ok = !matches!(next, Some(Tok::Word(_)));
    let before_ok = previous
        .and_then(Tok::key)
        .is_none_or(|key| !NOT_BEFORE_SCRATCH.contains(&key.as_str()));
    after_ok && before_ok
}

fn collapse_repeats(tokens: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::with_capacity(tokens.len());
    for token in tokens {
        if let (Some(key), Some(last)) = (token.key(), out.last().and_then(Tok::key))
            && key == last
            && key.chars().all(|c| c.is_alphabetic() || c == '\'')
            && !key.is_empty()
            && !ALLOWED_DOUBLES.contains(&key.as_str())
        {
            continue;
        }
        out.push(token);
    }
    out
}
