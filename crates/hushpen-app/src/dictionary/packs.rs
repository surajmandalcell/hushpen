//! Word packs: ready lists of names, places, and terms that ship with the app. A pack that is on
//! adds its words after the user's own entries, so they bias the speech model, stay as written
//! through the AI cleanup, and replace their "heard as" text, like the user's words do. Pack
//! words never go into the user's entries.
//!
//! A pack file is `# Name`, `# One line about it`, then one entry a line: `Phrase`, or
//! `Phrase | heard as`.

use crate::storage::Storage;
use hushpen_core::dictionary::Entry;
use serde_json::{Map, Value};
use std::sync::OnceLock;

/// The packs that are on, by id: `{"indian-names": true}`.
pub const SETTING: &str = "dictionary.packs";

/// The ids of pack entries start here, after any id the database gives, so the speech prompt,
/// which is built in id order, names the user's words first.
const FIRST_ID: i64 = 1 << 40;
/// The room for the entries of one pack in the ids.
const PACK_IDS: i64 = 1 << 20;

const FILES: [(&str, &str); 4] = [
    (
        "indian-names",
        include_str!("../../../../assets/word-packs/indian-names.txt"),
    ),
    (
        "indian-places",
        include_str!("../../../../assets/word-packs/indian-places.txt"),
    ),
    (
        "hinglish",
        include_str!("../../../../assets/word-packs/hinglish.txt"),
    ),
    (
        "tech-terms",
        include_str!("../../../../assets/word-packs/tech-terms.txt"),
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub id: &'static str,
    pub name: String,
    /// One line about what it holds.
    pub about: String,
    /// In the order of the file, most useful first, since the speech prompt keeps only the
    /// first few words.
    pub entries: Vec<Entry>,
}

impl Pack {
    /// The distinct words of the pack, in order.
    pub fn words(&self) -> Vec<&str> {
        let mut words: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !words.contains(&entry.phrase.as_str()) {
                words.push(&entry.phrase);
            }
        }
        words
    }

    /// The "heard as" texts of `phrase`.
    pub fn heard_as(&self, phrase: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|entry| entry.phrase == phrase)
            .filter_map(|entry| entry.heard_as.as_deref())
            .collect()
    }
}

/// Reads one pack file. `index` sets where its entry ids start.
pub fn parse(id: &'static str, index: usize, text: &str) -> Pack {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .peekable();
    let mut comment = || {
        let line = lines.next_if(|line| line.starts_with('#'))?;
        Some(line.trim_start_matches('#').trim().to_owned())
    };
    let name = comment().unwrap_or_else(|| id.to_owned());
    let about = comment().unwrap_or_default();
    let base = FIRST_ID + PACK_IDS * index as i64;
    let entries = lines
        .filter(|line| !line.starts_with('#'))
        .enumerate()
        .filter_map(|(at, line)| {
            let (phrase, heard) = match line.split_once('|') {
                Some((phrase, heard)) => (phrase.trim(), Some(heard.trim())),
                None => (line, None),
            };
            (!phrase.is_empty()).then(|| Entry {
                id: base + at as i64,
                phrase: phrase.to_owned(),
                heard_as: heard.filter(|heard| !heard.is_empty()).map(str::to_owned),
            })
        })
        .collect();
    Pack {
        id,
        name,
        about,
        entries,
    }
}

/// Every pack that ships, in the order the Dictionary view lists them.
pub fn all() -> &'static [Pack] {
    static PACKS: OnceLock<Vec<Pack>> = OnceLock::new();
    PACKS.get_or_init(|| {
        FILES
            .iter()
            .enumerate()
            .map(|(index, (id, text))| parse(id, index, text))
            .collect()
    })
}

/// Whether the pack `id` is on. Every pack is off until the user turns it on.
pub fn is_on(storage: &Storage, id: &str) -> bool {
    storage
        .settings
        .get(SETTING)
        .and_then(|packs| packs.get(id).and_then(Value::as_bool))
        .unwrap_or(false)
}

/// The setting with the pack `id` on or off. An off pack leaves no key behind.
pub fn turned(storage: &Storage, id: &str, on: bool) -> Value {
    let mut packs: Map<String, Value> = storage
        .settings
        .get(SETTING)
        .and_then(|packs| packs.as_object().cloned())
        .unwrap_or_default();
    if on {
        packs.insert(id.to_owned(), Value::Bool(true));
    } else {
        packs.remove(id);
    }
    Value::Object(packs)
}

/// The entries of the packs that are on, in pack order.
pub fn entries_on(storage: &Storage) -> Vec<Entry> {
    all()
        .iter()
        .filter(|pack| is_on(storage, pack.id))
        .flat_map(|pack| pack.entries.iter().cloned())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pack_file_gives_its_name_its_line_and_its_entries() {
        let pack = parse(
            "test",
            2,
            "# Test pack\n# Words to test with.\nGitHub | git hub\nGitHub | github\n\nRust\n",
        );
        assert_eq!(pack.name, "Test pack");
        assert_eq!(pack.about, "Words to test with.");
        assert_eq!(pack.words(), ["GitHub", "Rust"]);
        assert_eq!(pack.heard_as("GitHub"), ["git hub", "github"]);
        assert_eq!(pack.entries[0].id, FIRST_ID + 2 * PACK_IDS);
        assert_eq!(pack.entries[2].heard_as, None);
    }

    #[test]
    fn every_shipped_pack_reads_whole_and_clean() {
        let ids: Vec<&str> = all().iter().map(|pack| pack.id).collect();
        assert_eq!(
            ids,
            ["indian-names", "indian-places", "hinglish", "tech-terms"]
        );
        for pack in all() {
            assert!(
                !pack.name.starts_with('#') && !pack.about.is_empty(),
                "{}",
                pack.id
            );
            let words = pack.words().len();
            assert!((90..=260).contains(&words), "{} has {words} words", pack.id);
            assert!(
                (pack.entries.len() as i64) < PACK_IDS,
                "{} fills its ids",
                pack.id
            );
            for entry in &pack.entries {
                assert!(!entry.phrase.is_empty() && !entry.phrase.contains('|'));
                if let Some(heard) = &entry.heard_as {
                    assert_eq!(heard, &heard.to_lowercase(), "{}", entry.phrase);
                    assert_ne!(heard, &entry.phrase, "{}", entry.phrase);
                }
            }
        }
    }
}
