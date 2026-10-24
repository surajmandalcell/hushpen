//! The text to read aloud: the size limit, Markdown syntax removal, and chunks of whole
//! sentences, so audio and progress arrive while the rest is still being spoken.

use crate::SpeechError;
use hushpen_core::error::{SPEECH_NO_TEXT, SPEECH_TEXT_TOO_LONG, SPEECH_TEXT_UNREADABLE};
use markdown::mdast::Node;
use markdown::{Constructs, ParseOptions};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use unicode_segmentation::UnicodeSegmentation;

/// About 150,000 words, or 15 hours of speech.
pub const MAX_TEXT_BYTES: usize = 1 << 20;
/// A chunk holds whole sentences up to this length; a longer sentence is cut between words.
const CHUNK_BYTES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Plain,
    Markdown,
}

impl TextFormat {
    /// Markdown for `.md` and `.markdown` files, plain text for every other file.
    pub fn of_path(path: &Path) -> Self {
        let markdown = path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        });
        if markdown {
            Self::Markdown
        } else {
            Self::Plain
        }
    }
}

/// Reads a text file for `prepare`. Reads at most one byte over the limit.
pub fn read_text_file(path: &Path) -> Result<String, SpeechError> {
    let unreadable = |detail: String| SpeechError::new(SPEECH_TEXT_UNREADABLE, detail);
    let file = File::open(path).map_err(|error| unreadable(error.to_string()))?;
    let mut bytes = Vec::new();
    file.take(MAX_TEXT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable(error.to_string()))?;
    if bytes.len() > MAX_TEXT_BYTES {
        return Err(too_long());
    }
    String::from_utf8(bytes).map_err(|_| unreadable("the file is not UTF-8 text".into()))
}

/// The plain text to speak: Markdown syntax removed, within the size limit, with words in it.
pub fn prepare(text: &str, format: TextFormat) -> Result<String, SpeechError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(too_long());
    }
    let plain = match format {
        TextFormat::Plain => text.trim().to_owned(),
        TextFormat::Markdown => markdown_to_text(text),
    };
    if !plain.chars().any(char::is_alphanumeric) {
        return Err(SpeechError::new(SPEECH_NO_TEXT, "the text has no words"));
    }
    Ok(plain)
}

fn too_long() -> SpeechError {
    SpeechError::new(
        SPEECH_TEXT_TOO_LONG,
        format!("the text is over {MAX_TEXT_BYTES} bytes"),
    )
}

/// Splits prepared text into chunks of whole sentences of at most `CHUNK_BYTES`.
pub(crate) fn chunks(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let (mut start, mut end) = (0, 0);
    for (offset, sentence) in text.split_sentence_bound_indices() {
        let sentence_end = offset + sentence.len();
        if sentence_end - start > CHUNK_BYTES && end > start {
            push_trimmed(&mut chunks, &text[start..end]);
            start = end;
        }
        end = sentence_end;
        while end - start > CHUNK_BYTES {
            let cut = word_cut(&text[start..end]);
            push_trimmed(&mut chunks, &text[start..start + cut]);
            start += cut;
        }
    }
    push_trimmed(&mut chunks, &text[start..end]);
    chunks
}

fn push_trimmed<'a>(chunks: &mut Vec<&'a str>, chunk: &'a str) {
    let chunk = chunk.trim();
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
}

/// Where to cut a sentence longer than a chunk: the last space that fits, or inside a word
/// when there is none.
fn word_cut(sentence: &str) -> usize {
    let limit = sentence.floor_char_boundary(CHUNK_BYTES);
    match sentence[..limit].rfind(char::is_whitespace) {
        Some(space) if space > 0 => space,
        _ => limit,
    }
}

fn markdown_to_text(source: &str) -> String {
    let options = ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            ..Constructs::gfm()
        },
        ..ParseOptions::gfm()
    };
    // Only MDX syntax can fail to parse, and it is off.
    let Ok(root) = markdown::to_mdast(source, &options) else {
        return source.trim().to_owned();
    };
    let mut blocks = Vec::new();
    block_text(&root, &mut blocks);
    blocks.retain(|block| !block.trim().is_empty());
    blocks.join("\n\n")
}

fn block_text(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Root(_)
        | Node::Blockquote(_)
        | Node::List(_)
        | Node::ListItem(_)
        | Node::Table(_)
        | Node::FootnoteDefinition(_) => {
            for child in node.children().into_iter().flatten() {
                block_text(child, out);
            }
        }
        Node::Yaml(_)
        | Node::Toml(_)
        | Node::Html(_)
        | Node::ThematicBreak(_)
        | Node::Definition(_) => {}
        Node::TableRow(row) => {
            let cells: Vec<String> = row.children.iter().map(inline_text).collect();
            out.push(cells.join(", "));
        }
        _ => out.push(inline_text(node)),
    }
}

fn inline_text(node: &Node) -> String {
    match node {
        Node::Text(text) => text.value.clone(),
        Node::InlineCode(code) => code.value.clone(),
        Node::Code(code) => code.value.clone(),
        Node::InlineMath(math) => math.value.clone(),
        Node::Math(math) => math.value.clone(),
        Node::Image(image) => image.alt.clone(),
        Node::ImageReference(image) => image.alt.clone(),
        Node::Break(_) => "\n".into(),
        _ => node
            .children()
            .into_iter()
            .flatten()
            .map(inline_text)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_syntax_is_removed_and_blocks_stay_apart() {
        let source = "---\ntitle: Notes\n---\n# Plan\n\nRead **this** and [the docs](https://x.y).\n\n\
                      - one\n- `two`\n\n> quoted\n\n![a cat](cat.png)\n\n<div>html</div>\n\n---\n\n\
                      | A | B |\n|---|---|\n| 1 | 2 |\n\nline<br>\nnext  \nhard";
        let text = prepare(source, TextFormat::Markdown).unwrap();
        assert_eq!(
            text,
            "Plan\n\nRead this and the docs.\n\none\n\ntwo\n\nquoted\n\na cat\n\nA, B\n\n1, 2\n\n\
             line\nnext\nhard"
        );
    }

    #[test]
    fn plain_text_keeps_its_symbols() {
        let text = prepare("  # not a heading *really*  \n", TextFormat::Plain).unwrap();
        assert_eq!(text, "# not a heading *really*");
    }

    #[test]
    fn empty_and_oversized_text_are_errors() {
        assert_eq!(
            prepare(" \n--- ", TextFormat::Plain).unwrap_err().code,
            SPEECH_NO_TEXT
        );
        assert_eq!(
            prepare("***\n\n<br>", TextFormat::Markdown)
                .unwrap_err()
                .code,
            SPEECH_NO_TEXT
        );
        let big = "a".repeat(MAX_TEXT_BYTES + 1);
        assert_eq!(
            prepare(&big, TextFormat::Plain).unwrap_err().code,
            SPEECH_TEXT_TOO_LONG
        );
        assert!(prepare(&big[1..], TextFormat::Plain).is_ok());
    }

    #[test]
    fn the_format_follows_the_file_extension() {
        assert_eq!(
            TextFormat::of_path(Path::new("a/notes.MD")),
            TextFormat::Markdown
        );
        assert_eq!(
            TextFormat::of_path(Path::new("b.markdown")),
            TextFormat::Markdown
        );
        assert_eq!(TextFormat::of_path(Path::new("c.txt")), TextFormat::Plain);
        assert_eq!(TextFormat::of_path(Path::new("md")), TextFormat::Plain);
    }

    #[test]
    fn a_text_file_is_read_within_the_limit_and_must_be_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        std::fs::write(&path, "Hello there.").unwrap();
        assert_eq!(read_text_file(&path).unwrap(), "Hello there.");

        std::fs::write(&path, [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(
            read_text_file(&path).unwrap_err().code,
            SPEECH_TEXT_UNREADABLE
        );

        std::fs::write(&path, vec![b'a'; MAX_TEXT_BYTES + 1]).unwrap();
        assert_eq!(
            read_text_file(&path).unwrap_err().code,
            SPEECH_TEXT_TOO_LONG
        );

        let missing = dir.path().join("missing.txt");
        assert_eq!(
            read_text_file(&missing).unwrap_err().code,
            SPEECH_TEXT_UNREADABLE
        );
    }

    #[test]
    fn chunks_hold_whole_sentences_up_to_the_limit() {
        let sentence = "This sentence has exactly fifty bytes in it, yes. ";
        assert_eq!(sentence.len(), 50);
        let text = sentence.repeat(13);
        let parts = chunks(&text);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.len() <= CHUNK_BYTES));
        assert!(parts.iter().all(|part| part.ends_with("yes.")));
        assert_eq!(parts.concat().len(), text.trim().len() - 2);
    }

    #[test]
    fn a_long_sentence_is_cut_between_words_and_a_long_word_inside_it() {
        let words = "word ".repeat(100);
        let parts = chunks(&words);
        assert!(parts.iter().all(|part| part.len() <= CHUNK_BYTES));
        assert!(
            parts
                .iter()
                .all(|part| part.split(' ').all(|word| word == "word"))
        );
        assert_eq!(parts.join(" "), words.trim());

        let letters = "é".repeat(400);
        let parts = chunks(&letters);
        assert_eq!(parts.concat(), letters);
        assert!(parts.iter().all(|part| part.len() <= CHUNK_BYTES));
    }

    #[test]
    fn paragraphs_and_short_text_make_few_chunks() {
        assert_eq!(chunks("Hi."), vec!["Hi."]);
        assert_eq!(chunks("One.\n\nTwo"), vec!["One.\n\nTwo"]);
        assert!(chunks("  \n ").is_empty());
    }
}
