//! Numbers as English words, for a lexicon that has no digits: `45` is `forty five`.

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
const SCALES: [(u64, &str); 4] = [
    (1_000_000_000_000, "trillion"),
    (1_000_000_000, "billion"),
    (1_000_000, "million"),
    (1_000, "thousand"),
];
/// Above this a number is read digit by digit.
const LARGEST: u64 = 999_999_999_999_999;

fn below_hundred(n: u64, words: &mut Vec<&'static str>) {
    match n {
        0..=19 => words.push(ONES[n as usize]),
        _ => {
            words.push(TENS[(n / 10) as usize]);
            if !n.is_multiple_of(10) {
                words.push(ONES[(n % 10) as usize]);
            }
        }
    }
}

fn below_thousand(n: u64, words: &mut Vec<&'static str>) {
    if n >= 100 {
        words.push(ONES[(n / 100) as usize]);
        words.push("hundred");
    }
    if !n.is_multiple_of(100) || n == 0 {
        below_hundred(n % 100, words);
    }
}

/// `1234` is `one thousand two hundred thirty four`.
pub fn cardinal(n: u64) -> String {
    if n > LARGEST {
        return digits(&n.to_string());
    }
    let mut words = Vec::new();
    let mut rest = n;
    for (scale, name) in SCALES {
        if rest >= scale {
            below_thousand(rest / scale, &mut words);
            words.push(name);
            rest %= scale;
        }
    }
    if rest > 0 || words.is_empty() {
        below_thousand(rest, &mut words);
    }
    words.join(" ")
}

/// `007` is `zero zero seven`.
pub fn digits(text: &str) -> String {
    text.chars()
        .filter_map(|c| c.to_digit(10))
        .map(|digit| ONES[digit as usize])
        .collect::<Vec<_>>()
        .join(" ")
}

/// `21` is `twenty first`.
pub fn ordinal(n: u64) -> String {
    let words = cardinal(n);
    let (head, last) = words.rsplit_once(' ').unwrap_or(("", &words));
    let last = match last {
        "one" => "first".to_owned(),
        "two" => "second".to_owned(),
        "three" => "third".to_owned(),
        "five" => "fifth".to_owned(),
        "eight" => "eighth".to_owned(),
        "nine" => "ninth".to_owned(),
        "twelve" => "twelfth".to_owned(),
        tens if tens.ends_with('y') => format!("{}ieth", &tens[..tens.len() - 1]),
        other => format!("{other}th"),
    };
    if head.is_empty() {
        last
    } else {
        format!("{head} {last}")
    }
}

/// A four-digit year read in pairs: `1984` is `nineteen eighty four`, `2005` is `two thousand
/// five`. `None` for a number that is not read as a year.
pub fn year(n: u64) -> Option<String> {
    match n {
        2000..=2009 => Some(cardinal(n)),
        1100..=1999 | 2010..=2099 => {
            let (high, low) = (n / 100, n % 100);
            let low = match low {
                0 => "hundred".to_owned(),
                1..=9 => format!("oh {}", ONES[low as usize]),
                _ => cardinal(low),
            };
            Some(format!("{} {low}", cardinal(high)))
        }
        _ => None,
    }
}

/// `3:05` is `three oh five`, `3:00` is `three o'clock`.
pub fn time(hours: u64, minutes: u64) -> String {
    match minutes {
        0 => format!("{} o'clock", cardinal(hours)),
        1..=9 => format!("{} oh {}", cardinal(hours), ONES[minutes as usize]),
        _ => format!("{} {}", cardinal(hours), cardinal(minutes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cardinals_read_like_english() {
        let cases = [
            (0, "zero"),
            (7, "seven"),
            (13, "thirteen"),
            (45, "forty five"),
            (100, "one hundred"),
            (101, "one hundred one"),
            (1_000, "one thousand"),
            (1_234, "one thousand two hundred thirty four"),
            (2_000_005, "two million five"),
            (
                999_999_999_999_999,
                "nine hundred ninety nine trillion nine hundred ninety nine billion nine hundred ninety nine million nine hundred ninety nine thousand nine hundred ninety nine",
            ),
        ];
        for (n, words) in cases {
            assert_eq!(cardinal(n), words, "{n}");
        }
        assert_eq!(
            cardinal(1_000_000_000_000_000),
            "one zero zero zero zero zero zero zero zero zero zero zero zero zero zero zero"
        );
    }

    #[test]
    fn ordinals_change_the_last_word() {
        let cases = [
            (1, "first"),
            (2, "second"),
            (3, "third"),
            (12, "twelfth"),
            (20, "twentieth"),
            (21, "twenty first"),
            (100, "one hundredth"),
            (44, "forty fourth"),
        ];
        for (n, words) in cases {
            assert_eq!(ordinal(n), words, "{n}");
        }
    }

    #[test]
    fn years_read_in_pairs_and_times_read_as_spoken() {
        assert_eq!(year(1984).as_deref(), Some("nineteen eighty four"));
        assert_eq!(year(1905).as_deref(), Some("nineteen oh five"));
        assert_eq!(year(1900).as_deref(), Some("nineteen hundred"));
        assert_eq!(year(2005).as_deref(), Some("two thousand five"));
        assert_eq!(year(2025).as_deref(), Some("twenty twenty five"));
        assert_eq!(year(3000), None);
        assert_eq!(year(999), None);
        assert_eq!(time(3, 30), "three thirty");
        assert_eq!(time(3, 5), "three oh five");
        assert_eq!(time(12, 0), "twelve o'clock");
        assert_eq!(digits("007"), "zero zero seven");
    }
}
