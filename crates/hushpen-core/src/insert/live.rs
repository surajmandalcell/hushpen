//! Split streaming dictation at word boundaries and reconcile only the recorded draft.

#[derive(Clone, Debug, Default)]
pub struct StablePrefix {
    previous: String,
}

impl StablePrefix {
    /// Words must agree in two consecutive partials, including their punctuation.
    /// The slices keep the original spacing and concatenate to exactly this partial.
    pub fn update<'a>(&mut self, partial: &'a str) -> (&'a str, &'a str) {
        let (common, rest) = suffix_edit(&self.previous, partial);
        let boundary = |rest: &str| rest.is_empty() || rest.starts_with(char::is_whitespace);
        let agreed = &partial[..common];
        let whole = if boundary(rest) && boundary(&self.previous[common..]) {
            agreed
        } else {
            agreed
                .rsplit_once(char::is_whitespace)
                .map_or("", |(words, _)| words)
        };
        let end = whole.trim_end().len();
        self.previous.clear();
        self.previous.push_str(partial);
        partial.split_at(end)
    }
}

/// Keep this many UTF-8 bytes of the draft and replace the remainder with the returned suffix.
/// The caller must verify ownership of the span and caret before editing it.
pub fn suffix_edit<'a>(typed: &str, desired: &'a str) -> (usize, &'a str) {
    let keep = typed
        .chars()
        .zip(desired.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    (keep, &desired[keep..])
}

/// Words with their trailing spaces. Leading spaces form their own token.
fn tokens(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = text.starts_with(char::is_whitespace);
    for (at, ch) in text.char_indices() {
        if !ch.is_whitespace() && in_space && at > start {
            out.push((start, &text[start..at]));
            start = at;
        }
        in_space = ch.is_whitespace();
    }
    if start < text.len() {
        out.push((start, &text[start..]));
    }
    out
}

/// One word-level fix: replace `typed[range]` with `with`.
#[derive(Debug, PartialEq, Eq)]
pub struct WordEdit<'a> {
    pub range: std::ops::Range<usize>,
    pub with: &'a str,
}

/// The smallest word-level fixes that turn `typed` into `desired`, first to last, or `None` when
/// the texts are too long to compare. Apply them last to first so earlier offsets stay valid.
pub fn word_edits<'a>(typed: &str, desired: &'a str) -> Option<Vec<WordEdit<'a>>> {
    let a = tokens(typed);
    let b = tokens(desired);
    let (n, m) = (a.len(), b.len());
    if n.saturating_mul(m) > 4_000_000 {
        return None;
    }
    // lcs[i][j] = the longest common token subsequence of a[i..] and b[j..].
    let mut lcs = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if a[i].1 == b[j].1 {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let same = |i: usize, j: usize| i < n && j < m && a[i].1 == b[j].1;
    let a_byte = |i: usize| a.get(i).map_or(typed.len(), |t| t.0);
    let b_byte = |j: usize| b.get(j).map_or(desired.len(), |t| t.0);
    let (mut i, mut j, mut edits) = (0, 0, Vec::new());
    while i < n || j < m {
        if same(i, j) {
            i += 1;
            j += 1;
            continue;
        }
        let (i0, j0) = (i, j);
        while (i < n || j < m) && !same(i, j) {
            if j == m || (i < n && lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
                i += 1;
            } else {
                j += 1;
            }
        }
        edits.push(WordEdit {
            range: a_byte(i0)..a_byte(i),
            with: &desired[b_byte(j0)..b_byte(j)],
        });
    }
    Some(edits)
}

struct Word<'a> {
    raw: &'a str,
    key: String,
    end: usize,
}

fn words(text: &str) -> impl Iterator<Item = Word<'_>> {
    let mut offset = 0;
    text.split_inclusive(char::is_whitespace)
        .filter_map(move |part| {
            let raw = part.trim_end();
            let end = offset + raw.len();
            offset += part.len();
            let letters = raw.trim_matches(|c: char| !c.is_alphanumeric());
            let key = if letters.is_empty() { raw } else { letters };
            (!raw.is_empty()).then(|| Word {
                raw,
                key: key.to_lowercase(),
                end,
            })
        })
}

/// Find the unsent suffix after typing stopped. Only the last confirmed draft counts.
/// A revision must have one best word alignment. An inserted word inside the draft
/// cannot be delivered by a suffix paste, so leave the full transcript for recovery.
pub fn untyped_suffix<'a>(typed: &str, raw_final: &'a str) -> Option<&'a str> {
    if let Some(rest) = raw_final.strip_prefix(typed) {
        return Some(rest);
    }
    let typed: Vec<_> = words(typed).collect();
    let mut incoming = words(raw_final).peekable();
    let mut last = None;
    let mut shared = 0;
    while shared < typed.len()
        && incoming
            .peek()
            .is_some_and(|word| word.key == typed[shared].key)
    {
        last = incoming.next();
        shared += 1;
    }
    let changed = &typed[shared..];
    if !changed.is_empty() {
        // Removing the entire remaining draft costs its word count. Prefixes longer
        // than twice that count cannot beat it, even if every draft word matches.
        let candidates: Vec<_> = incoming.take(changed.len().saturating_mul(2)).collect();
        let consumed = aligned_end(changed, &candidates)?;
        if consumed > 0 {
            last = candidates.into_iter().nth(consumed - 1);
        }
    }
    let last = last?;
    let mut end = last.end;
    if let Some(typed) = typed.last().filter(|word| word.key == last.key) {
        let punctuation = |word: &str| word.trim_end_matches(|c: char| !c.is_alphanumeric()).len();
        let typed_marks = &typed.raw[punctuation(typed.raw)..];
        let final_marks = &last.raw[punctuation(last.raw)..];
        if let Some(extra) = final_marks.strip_prefix(typed_marks) {
            end -= extra.len();
        }
    }
    Some(&raw_final[end..])
}

fn aligned_end(typed: &[Word<'_>], incoming: &[Word<'_>]) -> Option<usize> {
    // Bound work on the input thread for long, unrelated transcripts.
    if typed.len().saturating_mul(incoming.len()) > 512 * 512 {
        return None;
    }
    // Each cell holds the cost and whether any cheapest path skips a final word.
    let mut row: Vec<_> = (0..=incoming.len()).map(|n| (n, n > 0)).collect();
    for draft in typed {
        let mut diagonal = row[0];
        row[0].0 += 1;
        for (i, word) in incoming.iter().enumerate() {
            let above = row[i + 1];
            let substitute = (diagonal.0 + usize::from(draft.key != word.key), diagonal.1);
            let delete = (above.0 + 1, above.1);
            let insert = (row[i].0 + 1, true);
            let best = substitute.0.min(delete.0).min(insert.0);
            row[i + 1] = (best, [substitute, delete, insert].contains(&(best, true)));
            diagonal = above;
        }
    }
    let best = row.iter().map(|cell| cell.0).min()?;
    let mut ends = row.iter().enumerate().filter(|(_, cell)| cell.0 == best);
    let (end, &(_, skipped_word)) = ends.next()?;
    (!skipped_word && ends.next().is_none()).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_edits_touch_only_the_changed_words() {
        let typed = "So it began. Uh we thought, you know, about that";
        let desired = "So it began. We thought about that.";
        let edits = word_edits(typed, desired).unwrap();
        let mut fixed = typed.to_owned();
        for edit in edits.iter().rev() {
            fixed.replace_range(edit.range.clone(), edit.with);
        }
        assert_eq!(fixed, desired);
        assert!(
            edits
                .iter()
                .all(|edit| edit.range.start >= "So it began. ".len())
        );
        assert_eq!(word_edits("same text", "same text").unwrap(), vec![]);
        let edits = word_edits("", "new words").unwrap();
        assert_eq!(
            edits,
            vec![WordEdit {
                range: 0..0,
                with: "new words"
            }]
        );
    }

    #[test]
    fn waits_for_complete_words_and_keeps_spacing() {
        let mut stream = StablePrefix::default();
        assert_eq!(stream.update("  we re"), ("", "  we re"));
        assert_eq!(stream.update("  we read"), ("  we", " read"));
        assert_eq!(stream.update("  we read\tthis"), ("  we read", "\tthis"));
        assert_eq!(stream.update("  we read\tthis"), ("  we read\tthis", ""));
    }

    #[test]
    fn repeated_words_have_distinct_positions() {
        let mut stream = StablePrefix::default();
        stream.update("go");
        assert_eq!(stream.update("go go"), ("go", " go"));
        assert_eq!(stream.update("go go go"), ("go go", " go"));
        assert_eq!(untyped_suffix("go go", "go go go now"), Some(" go now"));
    }

    #[test]
    fn revisions_wait_for_the_changed_word_to_settle() {
        let mut stream = StablePrefix::default();
        stream.update("read the form");
        stream.update("read the form now");
        assert_eq!(
            stream.update("read this form now"),
            ("read", " this form now")
        );
        assert_eq!(
            stream.update("read this form now."),
            ("read this form", " now.")
        );
        assert_eq!(stream.update(""), ("", ""));
        assert_eq!(stream.update("again"), ("", "again"));
    }

    #[test]
    fn punctuation_changes_use_the_smallest_suffix() {
        assert_eq!(suffix_edit("Ready?", "Ready! Go."), (5, "! Go."));
        assert_eq!(suffix_edit("the form", "the fork"), (7, "k"));
        assert_eq!(suffix_edit("red red", "red"), (3, ""));
        assert_eq!(suffix_edit("go", "go"), (2, ""));
        assert_eq!(suffix_edit("", "go"), (0, "go"));
    }

    #[test]
    fn diff_never_cuts_a_utf8_scalar() {
        assert_eq!(suffix_edit("café 🐈", "café 🐕"), ("café ".len(), "🐕"));
        assert_eq!(suffix_edit("a\u{301}", "a\u{300}"), (1, "\u{300}"));
    }

    #[test]
    fn interrupted_paste_preserves_user_input_and_appends_only_unsent_words() {
        let receipt = "we see stars";
        let mut field = String::from("we see stars [my note]");
        field.push_str(untyped_suffix(receipt, "We saw stars, then left.").unwrap());
        assert_eq!(field, "we see stars [my note], then left.");
    }

    #[test]
    fn interrupted_paste_handles_deleted_words_and_repetitions() {
        assert_eq!(
            untyped_suffix("we uh like tea", "We like tea with milk."),
            Some(" with milk.")
        );
        assert_eq!(
            untyped_suffix("I saw red red", "I see red red red!"),
            Some(" red!")
        );
        assert_eq!(untyped_suffix("HELLO!", "Hello!! Now."), Some("! Now."));
        assert_eq!(untyped_suffix("", "go now"), Some("go now"));
    }

    #[test]
    fn ambiguous_boundaries_or_missing_interior_words_require_recovery() {
        assert_eq!(untyped_suffix("go home", "go west now"), None);
        assert_eq!(
            untyped_suffix("one two four", "One two three four later"),
            None
        );
        assert_eq!(untyped_suffix("red", "blue"), None);
    }
}
