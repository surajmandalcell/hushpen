//! The personal dictionary: words that go into the whisper prompt, and "heard as" to "write as"
//! replacements that run on the cleaned text. Pure logic; the entries live in the store.
//!
//! An entry's `phrase` is what to write. Without `heard_as` it is a word for the prompt only.
//! With `heard_as`, the phrase also replaces that spoken text after the cleanup rules ran.

use std::collections::{HashMap, HashSet};

/// The prompt never goes over this many estimated tokens.
pub const PROMPT_TOKEN_CAP: usize = 150;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: i64,
    /// What to write. Never empty.
    pub phrase: String,
    /// What the engine hears. `None` makes the entry a prompt word only.
    pub heard_as: Option<String>,
}

/// Rough token count: one token for each 4 characters, rounded up.
pub fn estimated_tokens(chars: usize) -> usize {
    chars.div_ceil(4)
}

/// Why a phrase was not saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    EmptyPhrase,
    /// Another entry already writes this phrase.
    DuplicatePhrase(String),
    /// Another entry already replaces this spoken text.
    DuplicateHeardAs(String),
}

impl Refusal {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyPhrase => "Enter the word or phrase to write.".to_owned(),
            Self::DuplicatePhrase(phrase) => {
                format!("\u{201c}{phrase}\u{201d} is already in the dictionary.")
            }
            Self::DuplicateHeardAs(heard) => {
                format!("\u{201c}{heard}\u{201d} already has a replacement.")
            }
        }
    }
}

/// Trims the text and joins its words with single spaces.
pub fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn folded(text: &str) -> String {
    text.chars().map(fold).collect()
}

/// Checks a new or edited entry against the others and returns what to store. `editing` is the
/// id of the entry being changed, which does not clash with itself. A blank `heard_as` means
/// none.
pub fn validate(
    phrase: &str,
    heard_as: Option<&str>,
    existing: &[Entry],
    editing: Option<i64>,
) -> Result<(String, Option<String>), Refusal> {
    let phrase = normalize(phrase);
    if phrase.is_empty() {
        return Err(Refusal::EmptyPhrase);
    }
    let heard_as = heard_as.map(normalize).filter(|heard| !heard.is_empty());
    let others = || existing.iter().filter(|entry| Some(entry.id) != editing);
    let key = folded(&phrase);
    if others().any(|entry| folded(&entry.phrase) == key) {
        return Err(Refusal::DuplicatePhrase(phrase));
    }
    if let Some(heard) = &heard_as {
        let key = folded(heard);
        if others().any(|entry| entry.heard_as.as_deref().map(folded).as_deref() == Some(&key)) {
            return Err(Refusal::DuplicateHeardAs(heard.clone()));
        }
    }
    Ok((phrase, heard_as))
}

/// The whisper initial prompt: the phrases joined by ", ", oldest entry first (ascending id),
/// repeats left out. A phrase that would take the prompt over [`PROMPT_TOKEN_CAP`] estimated
/// tokens is left out whole, and later phrases that still fit are kept. The same entries always
/// give the same prompt. Empty when there is nothing to say.
pub fn build_prompt(entries: &[Entry]) -> String {
    let mut ordered: Vec<&Entry> = entries.iter().collect();
    ordered.sort_by_key(|entry| entry.id);
    let mut prompt = String::new();
    let mut chars = 0;
    let mut seen = HashSet::new();
    for entry in ordered {
        let phrase = normalize(&entry.phrase);
        if phrase.is_empty() || !seen.insert(folded(&phrase)) {
            continue;
        }
        let added = phrase.chars().count() + if prompt.is_empty() { 0 } else { 2 };
        if estimated_tokens(chars + added) > PROMPT_TOKEN_CAP {
            continue;
        }
        if !prompt.is_empty() {
            prompt.push_str(", ");
        }
        prompt.push_str(&phrase);
        chars += added;
    }
    prompt
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Pattern {
    /// Folded, with each run of spaces as one `' '`.
    heard: Vec<char>,
    write: String,
}

impl Pattern {
    /// The index after the match at `at`, when the pattern matches there as whole words.
    fn matches(&self, text: &[char], at: usize) -> Option<usize> {
        let first = *self.heard.first()?;
        if is_word(first) && at > 0 && is_word(text[at - 1]) {
            return None;
        }
        let mut end = at;
        for &wanted in &self.heard {
            if wanted == ' ' {
                let start = end;
                while end < text.len() && text[end].is_whitespace() {
                    end += 1;
                }
                if end == start {
                    return None;
                }
            } else {
                if end >= text.len() || fold(text[end]) != wanted {
                    return None;
                }
                end += 1;
            }
        }
        let last = *self.heard.last()?;
        if is_word(last) && end < text.len() && is_word(text[end]) {
            return None;
        }
        Some(end)
    }
}

/// Replaces each "heard as" text in `text` with its phrase. Matching is on whole words and
/// ignores case; the phrase is written as stored. One pass over the input, so a written phrase
/// is never replaced again. Where two entries match at the same place the longer one wins.
pub fn apply(text: &str, entries: &[Entry]) -> String {
    let mut patterns: Vec<Pattern> = entries
        .iter()
        .filter_map(|entry| {
            let heard = entry.heard_as.as_deref().map(normalize)?;
            (!heard.is_empty()).then(|| Pattern {
                heard: heard.chars().map(fold).collect(),
                write: entry.phrase.clone(),
            })
        })
        .collect();
    if patterns.is_empty() {
        return text.to_owned();
    }
    patterns.sort_by_key(|pattern| std::cmp::Reverse(pattern.heard.len()));
    let mut by_first: HashMap<char, Vec<usize>> = HashMap::new();
    for (index, pattern) in patterns.iter().enumerate() {
        by_first.entry(pattern.heard[0]).or_default().push(index);
    }

    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        let found = by_first.get(&fold(chars[at])).and_then(|candidates| {
            candidates
                .iter()
                .find_map(|&index| patterns[index].matches(&chars, at).map(|end| (index, end)))
        });
        match found {
            Some((index, end)) => {
                out.push_str(&patterns[index].write);
                at = end;
            }
            None => {
                out.push(chars[at]);
                at += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
