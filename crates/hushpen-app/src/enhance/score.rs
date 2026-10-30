//! How many words of a known text a transcript got wrong: the word error count, after both
//! texts are put in one form, so "250" and "two hundred fifty", or "Thursday," and "thursday",
//! count as the same word.

/// The words of `text` in one form: lower case, no punctuation, numbers as spoken words, and
/// "and" dropped between number words ("two hundred and fifty" is "two hundred fifty").
pub fn words(text: &str) -> Vec<String> {
    let mut spread = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    for (index, &c) in chars.iter().enumerate() {
        let between_digits = |c: char| {
            index > 0
                && chars[index - 1].is_ascii_digit()
                && chars.get(index + 1).is_some_and(char::is_ascii_digit)
                && c == ','
        };
        if c.is_alphanumeric() {
            spread.extend(c.to_lowercase());
        } else if c == '\'' || c == '\u{2019}' || between_digits(c) {
            // "don't" is one word, and "1,250" one number.
        } else {
            spread.push(' ');
        }
    }
    let mut out: Vec<String> = Vec::new();
    for token in spread.split_whitespace() {
        match spoken(token) {
            Some(number) => out.extend(number),
            None => out.push(token.to_owned()),
        }
    }
    let keep: Vec<bool> = (0..out.len())
        .map(|index| {
            out[index] != "and"
                || index == 0
                || !(is_number_word(&out[index - 1])
                    && out.get(index + 1).is_some_and(|next| is_number_word(next)))
        })
        .collect();
    out.into_iter()
        .zip(keep)
        .filter_map(|(word, keep)| keep.then_some(word))
        .collect()
}

/// How many words of `reference` the `heard` text got wrong (swapped, missing, or extra), and
/// how many words the reference has.
pub fn word_errors(reference: &str, heard: &str) -> (usize, usize) {
    let (wanted, got) = (words(reference), words(heard));
    let mut previous: Vec<usize> = (0..=got.len()).collect();
    for (row, want) in wanted.iter().enumerate() {
        let mut current = vec![row + 1; got.len() + 1];
        for (column, word) in got.iter().enumerate() {
            let swap = previous[column] + usize::from(want != word);
            current[column + 1] = swap.min(previous[column + 1] + 1).min(current[column] + 1);
        }
        previous = current;
    }
    (previous[got.len()], wanted.len())
}

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

fn is_number_word(word: &str) -> bool {
    ONES.contains(&word)
        || TENS[2..].contains(&word)
        || matches!(word, "hundred" | "thousand" | "million")
}

/// A token of digits, with an ordinal ending or not ("3", "3rd", "250"), as spoken words.
fn spoken(token: &str) -> Option<Vec<String>> {
    let digits: String = token.chars().take_while(char::is_ascii_digit).collect();
    let rest = &token[digits.len()..];
    let ordinal = match rest {
        "" => false,
        "st" | "nd" | "rd" | "th" => true,
        _ => return None,
    };
    if digits.is_empty() || digits.len() > 9 {
        return None;
    }
    let mut words = cardinal(digits.parse().ok()?);
    if ordinal && let Some(last) = words.last_mut() {
        *last = ordinal_of(last);
    }
    Some(words)
}

fn cardinal(number: u64) -> Vec<String> {
    if number == 0 {
        return vec![ONES[0].to_owned()];
    }
    let mut words = Vec::new();
    let mut rest = number;
    for (scale, name) in [(1_000_000, "million"), (1_000, "thousand")] {
        if rest >= scale {
            words.extend(below_thousand(rest / scale));
            words.push(name.to_owned());
            rest %= scale;
        }
    }
    words.extend(below_thousand(rest));
    words
}

fn below_thousand(number: u64) -> Vec<String> {
    let mut words = Vec::new();
    let (hundreds, rest) = (number / 100, number % 100);
    if hundreds > 0 {
        words.push(ONES[hundreds as usize].to_owned());
        words.push("hundred".to_owned());
    }
    if rest >= 20 {
        words.push(TENS[(rest / 10) as usize].to_owned());
        if rest % 10 > 0 {
            words.push(ONES[(rest % 10) as usize].to_owned());
        }
    } else if rest > 0 {
        words.push(ONES[rest as usize].to_owned());
    }
    words
}

fn ordinal_of(word: &str) -> String {
    match word {
        "one" => "first".to_owned(),
        "two" => "second".to_owned(),
        "three" => "third".to_owned(),
        "five" => "fifth".to_owned(),
        "eight" => "eighth".to_owned(),
        "nine" => "ninth".to_owned(),
        "twelve" => "twelfth".to_owned(),
        tens if tens.ends_with('y') => format!("{}ieth", &tens[..tens.len() - 1]),
        other => format!("{other}th"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_and_punctuation_do_not_count() {
        assert_eq!(
            words("We visited, the VILLAGE. Don't!"),
            ["we", "visited", "the", "village", "dont"]
        );
    }

    #[test]
    fn written_and_spoken_numbers_are_the_same_words() {
        assert_eq!(words("250"), words("two hundred fifty"));
        assert_eq!(words("250"), words("two hundred and fifty"));
        assert_eq!(words("1,250"), words("one thousand two hundred fifty"));
        assert_eq!(words("the 3rd"), words("the third"));
        assert_eq!(words("21st 40th"), words("twenty first fortieth"));
        assert_eq!(words("9 or 0"), ["nine", "or", "zero"]);
        assert_eq!(words("bread and butter"), ["bread", "and", "butter"]);
    }

    #[test]
    fn errors_count_swapped_missing_and_extra_words() {
        let reference = "Ravi wrote 250 words for the radio report.";
        assert_eq!(
            word_errors(reference, "Ravi wrote 250 words for the radio report"),
            (0, 10)
        );
        assert_eq!(
            word_errors(
                reference,
                "Ravi rote two hundred and fifty words for radio report."
            ),
            (2, 10),
            "one swapped, one missing"
        );
        assert_eq!(word_errors(reference, ""), (10, 10));
        assert_eq!(word_errors("one two", "one two three four"), (2, 2));
    }
}
