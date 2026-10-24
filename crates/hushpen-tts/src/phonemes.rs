//! Text to Kokoro phonemes, without espeak-ng.
//!
//! sherpa-onnx 2.0 can read text through the lexicon of the model, but it skips every word the
//! lexicon lacks and logs that word. The lexicon has no digits, no names, and few inflected
//! forms (it has `open` but not `opens`), so the worker turns text into phonemes itself and gives
//! sherpa-onnx the phonemes. [`Lexicon::sentences`] looks each word up, reads the endings `-s`,
//! `-ed`, and `-ing` with the rules of misaki (the front end Kokoro was trained with), reads
//! numbers as words, splits an unknown word into known words or spells it, and cuts the text
//! into sentences.

use crate::numbers;
use std::collections::HashMap;
use std::path::Path;

/// Marks that end a sentence.
const ENDS: [char; 4] = ['.', '!', '?', '…'];
/// Marks the model reads as a pause inside a sentence.
const PAUSES: [char; 4] = [',', ';', ':', '—'];
/// A sentence of fewer words runs on into the next one: the model reads a lone word poorly.
const MIN_WORDS: usize = 3;
/// The most phonemes in one sentence. The model takes 510 tokens, and sherpa-onnx cuts a
/// sentence at 400, even inside a word.
const MAX_PHONEMES: usize = 300;
/// A longer unknown word is spelled instead of split into known words.
const MAX_SPLIT_LETTERS: usize = 32;
/// An unknown word in capitals up to this long is read as letters, like `API`.
const MAX_ACRONYM: usize = 6;

/// The phonemes before which `the` is `ði`, not `ðə`.
const VOWELS: &str = "AIOWYaeiouæɑɒɔəɛɜɪʊʌᵊᵻɐ";
/// The phonemes before a final `t` that American English reads as a flap: `waited` is
/// `wˈAɾᵻd`.
const FLAP_BEFORE: &str = "AIOWYiuæɑəɛɪɹʊʌ";
const STRESS: [char; 2] = ['ˈ', 'ˌ'];

/// The words of the model's lexicon with their phonemes.
pub struct Lexicon {
    words: HashMap<String, String>,
}

/// One piece of the text on its way to phonemes.
enum Piece {
    /// The phonemes of one spoken word.
    Word(String),
    /// `the`, whose sound depends on the word after it.
    The,
    Mark(char),
}

impl Lexicon {
    /// Reads a lexicon file: a word, then its phonemes, one per field.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let entries = text.lines().filter_map(|line| {
            let mut fields = line.split_whitespace();
            let word = fields.next()?;
            Some((word, fields.collect::<String>()))
        });
        Ok(Self::new(entries))
    }

    pub fn new<K: AsRef<str>, V: Into<String>>(entries: impl IntoIterator<Item = (K, V)>) -> Self {
        Self {
            words: entries
                .into_iter()
                .map(|(word, phonemes)| (word.as_ref().to_lowercase(), phonemes.into()))
                .collect(),
        }
    }

    /// The phonemes of `text`, one string for each sentence: words one space apart, a mark
    /// right after the word before it. Empty when the text holds no word.
    pub fn sentences(&self, text: &str) -> Vec<String> {
        let pieces = self.pieces(text);
        let mut out = Sentences::default();
        for (index, piece) in pieces.iter().enumerate() {
            match piece {
                Piece::Word(phonemes) => out.word(phonemes),
                Piece::The => out.word(the(pieces.get(index + 1))),
                Piece::Mark(mark) => out.mark(*mark),
            }
        }
        out.finish()
    }

    fn pieces(&self, text: &str) -> Vec<Piece> {
        let chars: Vec<char> = text.chars().map(fold).collect();
        let mut pieces = Vec::new();
        let mut at = 0;
        while at < chars.len() {
            let c = chars[at];
            if c.is_ascii_digit() {
                let (words, end) = number(&chars, at);
                for word in words.split_whitespace() {
                    self.word(word, &mut pieces);
                }
                at = end;
                continue;
            }
            if c.is_alphabetic() {
                let end = word_end(&chars, at);
                let word: String = chars[at..end].iter().collect();
                self.word(&word, &mut pieces);
                at = end;
                continue;
            }
            let next_is_digit = chars.get(at + 1).is_some_and(char::is_ascii_digit);
            match c {
                '.' if chars.get(at + 1) == Some(&'.') => {
                    while chars.get(at + 1) == Some(&'.') {
                        at += 1;
                    }
                    pieces.push(Piece::Mark('…'));
                }
                _ if ENDS.contains(&c) || PAUSES.contains(&c) => pieces.push(Piece::Mark(c)),
                '-' if next_is_digit && !after_word(&chars, at) => self.word("minus", &mut pieces),
                '-' if spaced(&chars, at) => pieces.push(Piece::Mark('—')),
                '&' => self.word("and", &mut pieces),
                '+' => self.word("plus", &mut pieces),
                '@' => self.word("at", &mut pieces),
                '%' => self.word("percent", &mut pieces),
                // `number` reads a currency sign before its digits. Any other character only
                // parts two words.
                _ => {}
            }
            at += 1;
        }
        pieces
    }

    fn known(&self, word: &str) -> Option<&str> {
        self.words.get(word).map(String::as_str)
    }

    fn word(&self, word: &str, pieces: &mut Vec<Piece>) {
        if word.eq_ignore_ascii_case("the") {
            return pieces.push(Piece::The);
        }
        let letters = word.chars().count();
        let capitals = word.chars().all(char::is_uppercase);
        // `OS` and `US` are read as letters, not as the words `os` and `us`.
        if capitals && (2..=3).contains(&letters) {
            return self.spell(word, pieces);
        }
        if let Some(phonemes) = self.pronounce(word) {
            return pieces.push(Piece::Word(phonemes));
        }
        if let Some(stem) = word
            .strip_suffix("'s")
            .or_else(|| word.strip_suffix("'S"))
            .filter(|stem| !stem.is_empty())
        {
            let before = pieces.len();
            self.word(stem, pieces);
            if pieces.len() > before
                && let Some(Piece::Word(last)) = pieces.last_mut()
            {
                *last = s_ending(last);
            }
            return;
        }
        let parts = parts(word);
        if parts.len() > 1 {
            for part in &parts {
                self.word(part, pieces);
            }
            return;
        }
        let acronym = capitals && letters <= MAX_ACRONYM;
        if !acronym && let Some(phonemes) = self.split(&word.to_lowercase()) {
            return pieces.push(Piece::Word(phonemes));
        }
        self.spell(word, pieces);
    }

    /// The phonemes of a word the lexicon has, or of a known word with an ending.
    fn pronounce(&self, word: &str) -> Option<String> {
        let word = word.to_lowercase();
        // The article. The lexicon gives the name of the letter.
        if word == "a" {
            return Some("ɐ".to_owned());
        }
        self.known(&word)
            .map(str::to_owned)
            .or_else(|| self.with_s(&word))
            .or_else(|| self.with_ed(&word))
            .or_else(|| self.with_ing(&word))
    }

    /// `opens`, `boxes`, `cities`, `Smith's`.
    fn with_s(&self, word: &str) -> Option<String> {
        let stem = word
            .strip_suffix('s')
            .filter(|stem| stem.len() > 1 && !stem.ends_with('s'))
            .and_then(|stem| self.known(stem))
            .or_else(|| word.strip_suffix("'s").and_then(|stem| self.known(stem)))
            .or_else(|| {
                word.strip_suffix("es")
                    .filter(|stem| stem.len() > 2 && !stem.ends_with('i'))
                    .and_then(|stem| self.known(stem))
            })
            .map(str::to_owned)
            .or_else(|| self.with_y(word, "ies"))?;
        Some(s_ending(&stem))
    }

    /// `baked`, `opened`, `stopped`, `carried`.
    fn with_ed(&self, word: &str) -> Option<String> {
        let stem = word
            .strip_suffix('d')
            .filter(|stem| !stem.ends_with('d'))
            .and_then(|stem| self.known(stem))
            .or_else(|| {
                word.strip_suffix("ed")
                    .filter(|stem| !stem.ends_with('e'))
                    .and_then(|stem| self.known(stem))
            })
            .or_else(|| undouble(word, "ed").and_then(|stem| self.known(stem)))
            .map(str::to_owned)
            .or_else(|| self.with_y(word, "ied"))?;
        Some(ed_ending(&stem))
    }

    /// `opening`, `making`, `running`.
    fn with_ing(&self, word: &str) -> Option<String> {
        let stem = word.strip_suffix("ing").filter(|stem| stem.len() > 1)?;
        let stem = self
            .known(stem)
            .map(str::to_owned)
            .or_else(|| self.known(&format!("{stem}e")).map(str::to_owned))
            .or_else(|| {
                undouble(word, "ing")
                    .and_then(|stem| self.known(stem))
                    .map(str::to_owned)
            })?;
        Some(ing_ending(&stem))
    }

    /// The phonemes of the `y` word under an ending such as `ies`: `city` for `cities`.
    fn with_y(&self, word: &str, ending: &str) -> Option<String> {
        let stem = word.strip_suffix(ending).filter(|stem| stem.len() > 1)?;
        self.known(&format!("{stem}y")).map(str::to_owned)
    }

    /// The phonemes of the fewest lexicon words of two letters or more that make up `word`,
    /// such as `hush` and `pen` for `hushpen`, run together as one word.
    fn split(&self, word: &str) -> Option<String> {
        let chars: Vec<char> = word.chars().collect();
        let count = chars.len();
        if count > MAX_SPLIT_LETTERS {
            return None;
        }
        // best[end] is the fewest pieces that cover chars[..end], with the start of the last one.
        let mut best: Vec<Option<(usize, usize)>> = vec![None; count + 1];
        best[0] = Some((0, 0));
        for end in 2..=count {
            for start in 0..=end - 2 {
                let Some((pieces, _)) = best[start] else {
                    continue;
                };
                let better = best[end].is_none_or(|(known, _)| pieces + 1 < known);
                let piece: String = chars[start..end].iter().collect();
                if better && self.words.contains_key(&piece) {
                    best[end] = Some((pieces + 1, start));
                }
            }
        }
        best[count]?;
        let mut pieces = Vec::new();
        let mut end = count;
        while end > 0 {
            let (_, start) = best[end]?;
            let piece: String = chars[start..end].iter().collect();
            pieces.push(self.known(&piece)?);
            end = start;
        }
        pieces.reverse();
        Some(pieces.concat())
    }

    /// The names of the letters, each a word. A letter the lexicon lacks is left out.
    fn spell(&self, word: &str, pieces: &mut Vec<Piece>) {
        for letter in word.chars().flat_map(char::to_lowercase) {
            if let Some(name) = self.known(letter.encode_utf8(&mut [0; 4])) {
                pieces.push(Piece::Word(name.to_owned()));
            }
        }
    }
}

/// `ði` before a vowel, `ðə` before anything else.
fn the(next: Option<&Piece>) -> &'static str {
    let vowel = match next {
        Some(Piece::Word(phonemes)) => phonemes
            .chars()
            .find(|c| !STRESS.contains(c))
            .is_some_and(|c| VOWELS.contains(c)),
        _ => false,
    };
    if vowel { "ði" } else { "ðə" }
}

fn last_two(phonemes: &str) -> (Option<char>, Option<char>) {
    let mut chars = phonemes.chars().rev();
    (chars.next(), chars.next())
}

/// `phonemes` without its last character.
fn but_last(phonemes: &str) -> &str {
    let mut chars = phonemes.chars();
    chars.next_back();
    chars.as_str()
}

fn s_ending(stem: &str) -> String {
    match last_two(stem).0 {
        Some(last) if "ptkfθ".contains(last) => format!("{stem}s"),
        Some(last) if "szʃʒʧʤ".contains(last) => format!("{stem}ᵻz"),
        _ => format!("{stem}z"),
    }
}

fn ed_ending(stem: &str) -> String {
    match last_two(stem) {
        (Some(last), _) if "pkfθʃsʧ".contains(last) => format!("{stem}t"),
        (Some('d'), _) => format!("{stem}ᵻd"),
        (Some('t'), Some(before)) if FLAP_BEFORE.contains(before) => {
            format!("{}ɾᵻd", but_last(stem))
        }
        (Some('t'), _) => format!("{stem}ᵻd"),
        _ => format!("{stem}d"),
    }
}

fn ing_ending(stem: &str) -> String {
    match last_two(stem) {
        (Some('t'), Some(before)) if FLAP_BEFORE.contains(before) => {
            format!("{}ɾɪŋ", but_last(stem))
        }
        _ => format!("{stem}ɪŋ"),
    }
}

/// The stem of `stopped` or `picnicking` under `ending`: `stop`, `picnic`.
fn undouble<'a>(word: &'a str, ending: &str) -> Option<&'a str> {
    let stem = word.strip_suffix(ending)?;
    let (last, before) = last_two(stem);
    let (last, before) = (last?, before?);
    let doubled = last == before && "bcdgklmnprstvxz".contains(last);
    (doubled || (before == 'c' && last == 'k')).then(|| but_last(stem))
}

/// Typographic quotes and dashes become the plain ones the rest of the code reads, and a Latin
/// letter with a mark becomes the plain letter: the lexicon spells `cafe`, not `café`.
fn fold(c: char) -> char {
    match c {
        '\u{2018}' | '\u{2019}' | '\u{02bc}' => '\'',
        '\u{2013}' => '—',
        '\u{2212}' => '-',
        '\u{00a0}' => ' ',
        'à'..='å' => 'a',
        'À'..='Å' => 'A',
        'ç' => 'c',
        'Ç' => 'C',
        'è'..='ë' => 'e',
        'È'..='Ë' => 'E',
        'ì'..='ï' => 'i',
        'Ì'..='Ï' => 'I',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ò'..='ö' => 'o',
        'Ò'..='Ö' => 'O',
        'ù'..='ü' => 'u',
        'Ù'..='Ü' => 'U',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        other => other,
    }
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

/// `iPhone` is `i` and `Phone`, `HTTPServer` is `HTTP` and `Server`, `well-read` is `well` and
/// `read`.
fn parts(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if matches!(c, '\'' | '-') {
            parts.push(std::mem::take(&mut current));
            continue;
        }
        let previous = index.checked_sub(1).map(|before| chars[before]);
        let next = chars.get(index + 1);
        let lower_to_upper = previous.is_some_and(char::is_lowercase) && c.is_uppercase();
        let acronym_end = previous.is_some_and(char::is_uppercase)
            && c.is_uppercase()
            && next.is_some_and(|next| next.is_lowercase());
        if lower_to_upper || acronym_end {
            parts.push(std::mem::take(&mut current));
        }
        current.push(c);
    }
    parts.push(current);
    parts.retain(|part| !part.is_empty());
    parts
}

/// Reads the number at `start` as words, with a currency sign before it and an ordinal ending
/// after it. Returns the words and where the text goes on.
fn number(chars: &[char], start: usize) -> (String, usize) {
    let mut end = start;
    while end < chars.len() {
        let c = chars[end];
        let inner = matches!(c, ',' | '.' | ':')
            && end > start
            && chars.get(end + 1).is_some_and(char::is_ascii_digit);
        if !(c.is_ascii_digit() || inner) {
            break;
        }
        end += 1;
    }
    let raw: String = chars[start..end].iter().collect();
    let suffix: String = chars[end..chars.len().min(end + 2)]
        .iter()
        .collect::<String>()
        .to_lowercase();
    let ordinal = matches!(suffix.as_str(), "st" | "nd" | "rd" | "th")
        && !chars.get(end + 2).is_some_and(|c| c.is_alphanumeric());
    let currency = start
        .checked_sub(1)
        .and_then(|before| currency(chars[before]));
    let words = read_number(&raw, currency, ordinal);
    (words, if ordinal { end + 2 } else { end })
}

/// The names of one unit and its hundredth.
struct Currency {
    one: &'static str,
    many: &'static str,
    cents: &'static str,
}

fn currency(sign: char) -> Option<Currency> {
    let (one, many, cents) = match sign {
        '$' => ("dollar", "dollars", "cents"),
        '€' => ("euro", "euros", "cents"),
        '£' => ("pound", "pounds", "pence"),
        _ => return None,
    };
    Some(Currency { one, many, cents })
}

fn read_number(raw: &str, currency: Option<Currency>, ordinal: bool) -> String {
    if let Some((hours, minutes)) = raw.split_once(':') {
        return match (hours.parse::<u64>(), minutes.parse::<u64>()) {
            (Ok(h), Ok(m)) if h <= 24 && m <= 59 && minutes.len() == 2 => numbers::time(h, m),
            _ => raw
                .split(':')
                .map(|part| read_number(part, None, false))
                .collect::<Vec<_>>()
                .join(" "),
        };
    }
    let (whole, fraction) = match raw.split_once('.') {
        Some((whole, fraction)) if !fraction.contains('.') => (whole, Some(fraction)),
        Some(_) => {
            return raw
                .split('.')
                .map(|part| read_number(part, None, false))
                .collect::<Vec<_>>()
                .join(" point ");
        }
        None => (raw, None),
    };
    let grouped = whole.contains(',');
    let whole: String = whole.chars().filter(char::is_ascii_digit).collect();
    let Ok(n) = whole.parse::<u64>() else {
        return numbers::digits(raw);
    };
    if whole.len() > 1 && whole.starts_with('0') && fraction.is_none() {
        return numbers::digits(&whole);
    }
    if let Some(money) = currency {
        let unit = if n == 1 { money.one } else { money.many };
        let mut words = format!("{} {unit}", numbers::cardinal(n));
        if let Some(cents) = fraction.filter(|cents| cents.len() == 2)
            && let Ok(cents) = cents.parse::<u64>()
            && cents > 0
        {
            words = format!("{words} and {} {}", numbers::cardinal(cents), money.cents);
        }
        return words;
    }
    if ordinal && fraction.is_none() {
        return numbers::ordinal(n);
    }
    let words = match fraction {
        None if !grouped && whole.len() == 4 => {
            numbers::year(n).unwrap_or_else(|| numbers::cardinal(n))
        }
        _ => numbers::cardinal(n),
    };
    match fraction {
        Some(fraction) => format!("{words} point {}", numbers::digits(fraction)),
        None => words,
    }
}

/// The sentences being written.
#[derive(Default)]
struct Sentences {
    done: Vec<String>,
    current: String,
    words: usize,
    phonemes: usize,
}

impl Sentences {
    fn word(&mut self, phonemes: &str) {
        let count = phonemes.chars().count();
        if self.words > 0 && self.phonemes + count > MAX_PHONEMES {
            self.flush();
        }
        if !self.current.is_empty() {
            self.current.push(' ');
        }
        self.current.push_str(phonemes);
        self.words += 1;
        self.phonemes += count + 1;
    }

    /// A mark before the first word of a sentence is dropped.
    fn mark(&mut self, mark: char) {
        if self.words == 0 {
            return;
        }
        self.current.push(mark);
        self.phonemes += 1;
        if ENDS.contains(&mark) && self.words >= MIN_WORDS {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if self.words > 0 {
            self.done.push(std::mem::take(&mut self.current));
        }
        self.current.clear();
        self.words = 0;
        self.phonemes = 0;
    }

    fn finish(mut self) -> Vec<String> {
        self.flush();
        self.done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lexicon whose phonemes are the word in angle brackets, so a test reads like text.
    fn tagged(words: &str) -> Lexicon {
        Lexicon::new(
            words
                .split_whitespace()
                .map(|word| (word, format!("<{word}>"))),
        )
    }

    fn common() -> Lexicon {
        tagged(
            "meet me at on march see you then how are today hello world wait what now ok it \
             costs off and minus point dollars dollar cents percent zero one two three five \
             nine twelve twenty thirty forty fifty ninety thousand hundred first o'clock hush \
             pen git hub smith phone runs mac os us cafe a b c d e f g h i j k l m n o p q r s t \
             u v w x y z",
        )
    }

    /// Real phonemes, for the rules that read the sounds.
    fn phonemes() -> Lexicon {
        Lexicon::new([
            ("open", "ˈOpᵊn"),
            ("box", "bˈɑks"),
            ("stop", "stˈɑp"),
            ("bake", "bˈAk"),
            ("make", "mˈAk"),
            ("city", "sˈɪɾi"),
            ("carry", "kˈɛɹi"),
            ("wait", "wˈAt"),
            ("want", "wˈɑnt"),
            ("need", "nˈid"),
            ("run", "ɹˈʌn"),
            ("smith", "smˈɪθ"),
            ("picnic", "pˈɪknɪk"),
            ("the", "ði"),
        ])
    }

    fn one(lexicon: &Lexicon, text: &str) -> String {
        let sentences = lexicon.sentences(text);
        assert_eq!(sentences.len(), 1, "{sentences:?}");
        sentences[0].clone()
    }

    #[test]
    fn words_and_marks_make_sentences_of_three_words_or_more() {
        let lexicon = common();
        assert_eq!(
            lexicon.sentences("Hello, world! How are you today? See you then."),
            [
                "<hello>, <world>! <how> <are> <you> <today>?",
                "<see> <you> <then>.",
            ]
        );
        assert_eq!(
            one(&lexicon, "Wait... what - now \u{2014} ok"),
            "<wait>… <what>— <now>— <ok>"
        );
        assert!(lexicon.sentences(" ... !? ").is_empty());
        assert!(lexicon.sentences("").is_empty());
    }

    #[test]
    fn a_long_sentence_is_cut_between_words() {
        let lexicon = common();
        let text = "hello ".repeat(100);
        let sentences = lexicon.sentences(&text);
        assert!(sentences.len() > 1);
        for sentence in &sentences {
            assert!(sentence.chars().count() <= MAX_PHONEMES, "{sentence}");
            assert!(!sentence.starts_with(' ') && !sentence.ends_with(' '));
        }
        let words: usize = sentences.iter().map(|s| s.split(' ').count()).sum();
        assert_eq!(words, 100);
    }

    #[test]
    fn numbers_times_years_money_and_ordinals_become_words() {
        let lexicon = common();
        assert_eq!(
            one(&lexicon, "Meet at 3:30 on 12 March 2025."),
            "<meet> <at> <three> <thirty> <on> <twelve> <march> <twenty> <twenty> <five>."
        );
        assert_eq!(
            one(&lexicon, "It costs $45.99, 50% off."),
            "<it> <costs> <forty> <five> <dollars> <and> <ninety> <nine> <cents>, <fifty> \
             <percent> <off>."
        );
        assert_eq!(
            one(&lexicon, "the 1st, 1,500 and -2.5"),
            "ðə <first>, <one> <thousand> <five> <hundred> <and> <minus> <two> <point> <five>"
        );
        assert_eq!(one(&lexicon, "at 12:00"), "<at> <twelve> <o'clock>");
        assert_eq!(one(&lexicon, "$1"), "<one> <dollar>");
    }

    #[test]
    fn unknown_words_split_into_known_ones_or_letters() {
        let lexicon = common();
        assert_eq!(
            one(&lexicon, "Smith's iPhone runs macOS"),
            "<smith>z <i> <phone> <runs> <mac> <o> <s>"
        );
        assert_eq!(
            one(&lexicon, "Hushpen on GitHub's API"),
            "<hush><pen> <on> <git> <hub>z <a> <p> <i>"
        );
        assert_eq!(one(&lexicon, "xq & (ok)"), "<x> <q> <and> <ok>");
        assert_eq!(one(&lexicon, "OS and us"), "<o> <s> <and> <us>");
        assert_eq!(
            one(
                &lexicon,
                "Caf\u{e9} \u{4f60}\u{597d} \u{f8}x \u{201c}ok\u{201d}"
            ),
            "<cafe> <x> <ok>"
        );
    }

    #[test]
    fn endings_follow_the_sound_of_the_stem() {
        let lexicon = phonemes();
        let cases = [
            ("opens", "ˈOpᵊnz"),
            ("stops", "stˈɑps"),
            ("boxes", "bˈɑksᵻz"),
            ("cities", "sˈɪɾiz"),
            ("Smith's", "smˈɪθs"),
            ("opened", "ˈOpᵊnd"),
            ("baked", "bˈAkt"),
            ("waited", "wˈAɾᵻd"),
            ("wanted", "wˈɑntᵻd"),
            ("needed", "nˈidᵻd"),
            ("stopped", "stˈɑpt"),
            ("carried", "kˈɛɹid"),
            ("opening", "ˈOpᵊnɪŋ"),
            ("making", "mˈAkɪŋ"),
            ("running", "ɹˈʌnɪŋ"),
            ("waiting", "wˈAɾɪŋ"),
            ("picnicking", "pˈɪknɪkɪŋ"),
        ];
        for (word, expected) in cases {
            assert_eq!(lexicon.pronounce(word).as_deref(), Some(expected), "{word}");
        }
        assert_eq!(lexicon.pronounce("bss"), None);
    }

    #[test]
    fn the_and_a_read_as_spoken() {
        let lexicon = phonemes();
        assert_eq!(
            one(&lexicon, "the box, the open box"),
            "ðə bˈɑks, ði ˈOpᵊn bˈɑks"
        );
        assert_eq!(one(&lexicon, "a box"), "ɐ bˈɑks");
    }

    #[test]
    fn a_split_prefers_the_fewest_pieces() {
        let lexicon = Lexicon::new([
            ("ab", "1"),
            ("abc", "2"),
            ("cd", "3"),
            ("d", "4"),
            ("bcd", "5"),
        ]);
        assert_eq!(lexicon.split("abcd").as_deref(), Some("13"));
        assert_eq!(lexicon.split("abd"), None);
        assert_eq!(lexicon.split(&"ab".repeat(20)), None);
    }

    #[test]
    fn parts_split_at_case_changes_hyphens_and_apostrophes() {
        assert_eq!(parts("iPhone"), ["i", "Phone"]);
        assert_eq!(parts("HTTPServer"), ["HTTP", "Server"]);
        assert_eq!(parts("rock'n'roll"), ["rock", "n", "roll"]);
        assert_eq!(parts("plain"), ["plain"]);
    }
}
