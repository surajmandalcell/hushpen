//! The text tokenizer of the Chatterbox models, read from the `tokenizer.json` a model ships:
//! GPT-2 byte-level BPE. Added tokens such as `[laugh]` are matched first, the rest of the text
//! is split into GPT-2 words, and each word's bytes are merged by rank. The ids are the ids the
//! Hugging Face tokenizer of the model gives, two end tokens included.

use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

const END_TOKEN: &str = "<|endoftext|>";
/// The cache of word ids is cleared at this size, so a long text cannot grow it without end.
const CACHE_WORDS: usize = 20_000;
/// The endings GPT-2 splits off a word: `don't` is `don` and `'t`.
const CONTRACTIONS: [&str; 7] = ["'s", "'t", "'re", "'ve", "'m", "'ll", "'d"];

pub struct Tokenizer {
    /// The id of each byte, through the GPT-2 table of bytes as characters.
    bytes: [u32; 256],
    /// For two ids side by side: the rank of their merge and the id it makes.
    merges: HashMap<(u32, u32), (u32, u32)>,
    /// The added tokens, the longest first.
    added: Vec<(String, u32)>,
    end: u32,
    cache: HashMap<String, Vec<u32>>,
}

impl Tokenizer {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&text).map_err(|problem| format!("{}: {problem}", path.display()))
    }

    pub fn parse(json: &str) -> Result<Self, String> {
        let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
        let model = &root["model"];
        if model["type"] != "BPE" {
            return Err("not a BPE tokenizer".into());
        }
        let vocab: HashMap<&str, u32> = model["vocab"]
            .as_object()
            .ok_or("no vocab")?
            .iter()
            .filter_map(|(token, id)| Some((token.as_str(), u32::try_from(id.as_u64()?).ok()?)))
            .collect();
        let id_of = |token: &str| {
            vocab
                .get(token)
                .copied()
                .ok_or_else(|| format!("the vocab has no `{token}`"))
        };
        let mut bytes = [0; 256];
        for (byte, c) in byte_chars().iter().enumerate() {
            bytes[byte] = id_of(c.encode_utf8(&mut [0; 4]))?;
        }
        let mut merges = HashMap::new();
        for (rank, merge) in model["merges"]
            .as_array()
            .ok_or("no merges")?
            .iter()
            .enumerate()
        {
            let (left, right) = match merge {
                Value::String(pair) => pair.split_once(' ').ok_or("a merge is not a pair")?,
                Value::Array(pair) => match (pair.first(), pair.get(1)) {
                    (Some(Value::String(left)), Some(Value::String(right))) => {
                        (left.as_str(), right.as_str())
                    }
                    _ => return Err("a merge is not a pair".into()),
                },
                _ => return Err("a merge is not a pair".into()),
            };
            let made = id_of(&format!("{left}{right}"))?;
            let rank = u32::try_from(rank).map_err(|_| "too many merges")?;
            merges
                .entry((id_of(left)?, id_of(right)?))
                .or_insert((rank, made));
        }
        let mut added: Vec<(String, u32)> = root["added_tokens"]
            .as_array()
            .map(|tokens| {
                tokens
                    .iter()
                    .filter_map(|token| {
                        let content = token["content"].as_str()?.to_owned();
                        let id = u32::try_from(token["id"].as_u64()?).ok()?;
                        Some((content, id))
                    })
                    .collect()
            })
            .unwrap_or_default();
        added.sort_by_key(|(content, _)| std::cmp::Reverse(content.len()));
        let end = added
            .iter()
            .find(|(content, _)| content == END_TOKEN)
            .map(|(_, id)| *id)
            .map_or_else(|| id_of(END_TOKEN), Ok)?;
        Ok(Self {
            bytes,
            merges,
            added,
            end,
            cache: HashMap::new(),
        })
    }

    /// The ids of `text`, then the two end tokens the model reads as the start of speech.
    pub fn encode(&mut self, text: &str) -> Vec<u32> {
        let mut ids = Vec::new();
        let mut rest = text;
        while !rest.is_empty() {
            match self.next_added(rest) {
                Some((at, len, id)) => {
                    self.encode_plain(&rest[..at], &mut ids);
                    ids.push(id);
                    rest = &rest[at + len..];
                }
                None => {
                    self.encode_plain(rest, &mut ids);
                    break;
                }
            }
        }
        ids.extend([self.end, self.end]);
        ids
    }

    /// The first added token in `text`, the longest of those that start there: where it
    /// starts, its length, and its id.
    fn next_added(&self, text: &str) -> Option<(usize, usize, u32)> {
        let mut best: Option<(usize, usize, u32)> = None;
        for (content, id) in &self.added {
            if let Some(at) = text.find(content.as_str())
                && best.is_none_or(|(first, len, _)| {
                    at < first || (at == first && content.len() > len)
                })
            {
                best = Some((at, content.len(), *id));
            }
        }
        best
    }

    fn encode_plain(&mut self, text: &str, ids: &mut Vec<u32>) {
        for word in words(text) {
            if let Some(known) = self.cache.get(word) {
                ids.extend_from_slice(known);
                continue;
            }
            let merged = self.merge(word);
            ids.extend_from_slice(&merged);
            if self.cache.len() >= CACHE_WORDS {
                self.cache.clear();
            }
            self.cache.insert(word.to_owned(), merged);
        }
    }

    /// The ids of one word: its bytes, merged pair by pair, the lowest rank first.
    fn merge(&self, word: &str) -> Vec<u32> {
        let mut parts: Vec<u32> = word
            .bytes()
            .map(|byte| self.bytes[usize::from(byte)])
            .collect();
        loop {
            let best = parts
                .windows(2)
                .filter_map(|pair| {
                    self.merges
                        .get(&(pair[0], pair[1]))
                        .map(|&(rank, made)| (rank, pair[0], pair[1], made))
                })
                .min_by_key(|&(rank, ..)| rank);
            let Some((_, left, right, made)) = best else {
                return parts;
            };
            let mut merged = Vec::with_capacity(parts.len());
            let mut at = 0;
            while at < parts.len() {
                if at + 1 < parts.len() && parts[at] == left && parts[at + 1] == right {
                    merged.push(made);
                    at += 2;
                } else {
                    merged.push(parts[at]);
                    at += 1;
                }
            }
            parts = merged;
        }
    }
}

/// The GPT-2 table that shows every byte as a printable character: the printable bytes stand
/// for themselves, the others take the characters from U+0100 on, in byte order.
fn byte_chars() -> [char; 256] {
    let mut table = ['\0'; 256];
    let mut next = 256;
    for byte in 0..=255_u8 {
        let printable = matches!(byte, b'!'..=b'~' | 0xa1..=0xac | 0xae..=0xff);
        table[usize::from(byte)] = if printable {
            char::from(byte)
        } else {
            let c = char::from_u32(next).unwrap_or('\0');
            next += 1;
            c
        };
    }
    table
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Space,
    Letter,
    Number,
    Other,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Space
    } else if c.is_alphabetic() {
        Class::Letter
    } else if c.is_numeric() {
        Class::Number
    } else {
        Class::Other
    }
}

/// The GPT-2 word split, the pattern
/// `'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+`: a word takes
/// the one space before it, and a run of spaces leaves its last space to the word after it.
fn words(text: &str) -> Vec<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let offset = |index: usize| chars.get(index).map_or(text.len(), |(at, _)| *at);
    let mut words = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let (start, c) = chars[at];
        if c == '\''
            && let Some(ending) = CONTRACTIONS
                .iter()
                .find(|ending| text[start..].starts_with(**ending))
        {
            let end = at + ending.chars().count();
            words.push(&text[start..offset(end)]);
            at = end;
            continue;
        }
        let first = if c == ' ' { at + 1 } else { at };
        if let Some(&(_, lead)) = chars.get(first)
            && class(lead) != Class::Space
        {
            let kind = class(lead);
            let mut end = first + 1;
            while chars.get(end).is_some_and(|&(_, next)| class(next) == kind) {
                end += 1;
            }
            words.push(&text[start..offset(end)]);
            at = end;
            continue;
        }
        let mut end = at;
        while chars.get(end).is_some_and(|(_, next)| next.is_whitespace()) {
            end += 1;
        }
        let run = end - at;
        let take = if end == chars.len() || run == 1 {
            run
        } else {
            run - 1
        };
        words.push(&text[start..offset(at + take)]);
        at += take;
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A tokenizer with every byte, the merges that make `hello`, ` world`, and `'s`, and one
    /// added token.
    fn small() -> Tokenizer {
        let mut vocab = serde_json::Map::new();
        for (id, c) in byte_chars().iter().enumerate() {
            vocab.insert(c.to_string(), json!(id));
        }
        let merges = [
            ["h", "e"],
            ["l", "l"],
            ["he", "ll"],
            ["hell", "o"],
            ["Ġ", "w"],
            ["o", "r"],
            ["Ġw", "or"],
            ["l", "d"],
            ["Ġwor", "ld"],
            ["'", "s"],
        ];
        for [left, right] in merges {
            let id = vocab.len();
            vocab.insert(format!("{left}{right}"), json!(id));
        }
        vocab.insert(END_TOKEN.into(), json!(1000));
        let tokenizer = json!({
            "added_tokens": [
                {"id": 1000, "content": END_TOKEN},
                {"id": 1001, "content": "[laugh]"},
            ],
            "model": {"type": "BPE", "vocab": vocab, "merges": merges},
        });
        Tokenizer::parse(&tokenizer.to_string()).unwrap()
    }

    fn id(tokenizer: &Tokenizer, piece: &str) -> u32 {
        let mut parts = tokenizer.merge(piece);
        assert_eq!(parts.len(), 1, "{piece}");
        parts.remove(0)
    }

    #[test]
    fn words_follow_the_gpt2_pattern() {
        assert_eq!(
            words("Hello world, it's 42 dollars!!  Really?\n\nYes "),
            [
                "Hello", " world", ",", " it", "'s", " 42", " dollars", "!!", " ", " Really", "?",
                "\n", "\n", "Yes", " "
            ]
        );
        assert_eq!(words("don't  stop"), ["don", "'t", " ", " stop"]);
        assert_eq!(words("a \n b"), ["a", " \n", " b"]);
        assert_eq!(words("'hi' x"), ["'", "hi", "'", " x"]);
        assert_eq!(words("café 日本"), ["café", " 日本"]);
        assert!(words("").is_empty());
    }

    #[test]
    fn merges_go_by_rank_and_bytes_stand_alone() {
        let tokenizer = small();
        let hello = id(&tokenizer, "hello");
        let world = id(&tokenizer, " world");
        let mut tokenizer = small();
        let ids = tokenizer.encode("hello world");
        assert_eq!(ids, [hello, world, 1000, 1000]);
        // No merge joins these bytes, so each stays its byte's id.
        let ids = tokenizer.encode("box");
        assert_eq!(
            ids[..3],
            [u32::from(b'b'), u32::from(b'o'), u32::from(b'x')]
        );
    }

    #[test]
    fn added_tokens_are_matched_before_the_words() {
        let mut tokenizer = small();
        let hello = id(&tokenizer, "hello");
        let space = tokenizer.bytes[usize::from(b' ')];
        assert_eq!(
            tokenizer.encode("hello [laugh] hello"),
            [hello, space, 1001, space, hello, 1000, 1000]
        );
        assert_eq!(tokenizer.encode("[laugh]"), [1001, 1000, 1000]);
        assert_eq!(tokenizer.encode(""), [1000, 1000]);
    }

    #[test]
    fn a_tokenizer_that_is_not_bpe_is_refused() {
        for bad in [
            json!({"model": {"type": "WordPiece"}}),
            json!({"model": {"type": "BPE", "vocab": {}, "merges": []}}),
            json!("text"),
        ] {
            assert!(Tokenizer::parse(&bad.to_string()).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_byte_table_is_the_gpt2_one() {
        let table = byte_chars();
        assert_eq!(table[usize::from(b'A')], 'A');
        assert_eq!(table[usize::from(b' ')], 'Ġ');
        assert_eq!(table[usize::from(b'\n')], 'Ċ');
        assert_eq!(table[0], 'Ā');
        let mut unique = table.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 256);
    }
}
