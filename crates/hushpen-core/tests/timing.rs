//! The rule pass and the dictionary replacements must stay far below the 50 ms budget per
//! dictation. The budget is for release builds; run with `--release` to check it.

use std::time::{Duration, Instant};

use hushpen_core::cleanup::{Options, clean};
use hushpen_core::dictionary::{Entry, apply};

// A debug build with sibling tests on a shared two-core runner runs far slower than a
// release build, so only a release build is held to the 50 ms of the contract.
const BUDGET: Duration = if cfg!(debug_assertions) {
    Duration::from_millis(500)
} else {
    Duration::from_millis(50)
};
const RUNS: usize = 100;

/// About 45 s of speech: 110 words with fillers, repeats, and spoken punctuation.
const DICTATION_45S: &str = "um so today I want to talk about the the quarterly report comma and uh how it \
went period you know the numbers were, like, better than we expected comma no wait comma much better \
than we expected period new paragraph first we need to review the budget and then we need to send it \
to the team period scratch that period we need to send it to finance question mark yes period I think \
that that is the right call and she had had enough of waiting period new line please add it to the \
agenda for friday and um let me know if there are any questions period";

/// 200 entries: 8 that hit words of the dictation, 12 words, and 180 that match nothing.
fn dictionary_of_200() -> Vec<Entry> {
    let hits = [
        ("quarterly", "Quarterly"),
        ("budget", "Budget"),
        ("finance", "Finance"),
        ("agenda", "Agenda"),
        ("friday", "Friday"),
        ("the team", "the Team"),
        ("new line", "newline"),
        ("questions", "Questions"),
    ];
    let mut entries: Vec<Entry> = hits
        .iter()
        .zip(1..)
        .map(|((heard, write), id)| Entry {
            id,
            phrase: (*write).to_owned(),
            heard_as: Some((*heard).to_owned()),
        })
        .collect();
    entries.extend((9..=20).map(|id| Entry {
        id,
        phrase: format!("Word{id}"),
        heard_as: None,
    }));
    entries.extend((21..=200).map(|id| Entry {
        id,
        phrase: format!("Written{id}"),
        heard_as: Some(format!("heard as number {id}")),
    }));
    assert_eq!(entries.len(), 200);
    entries
}

fn slowest(text: &str, entries: &[Entry]) -> Duration {
    let options = Options::default();
    (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            let cleaned = apply(&clean(text, &options), entries);
            let took = start.elapsed();
            assert!(!cleaned.is_empty());
            took
        })
        .max()
        .unwrap()
}

fn five_thousand_words() -> String {
    DICTATION_45S.repeat(5000 / DICTATION_45S.split_whitespace().count() + 1)
}

#[test]
fn a_45_second_dictation_cleans_in_under_50_ms() {
    let took = slowest(DICTATION_45S, &[]);
    println!("cleanup max over {RUNS} runs, 45 s dictation: {took:?}");
    assert!(took < BUDGET, "{took:?}");
}

#[test]
fn a_5000_word_input_cleans_in_under_50_ms() {
    let text = five_thousand_words();
    let took = slowest(&text, &[]);
    println!(
        "cleanup max over {RUNS} runs, {} words: {took:?}",
        text.split_whitespace().count()
    );
    assert!(took < BUDGET, "{took:?}");
}

#[test]
fn a_45_second_dictation_with_200_dictionary_entries_is_ready_in_under_50_ms() {
    let took = slowest(DICTATION_45S, &dictionary_of_200());
    println!("cleanup and 200 replacements, max over {RUNS} runs, 45 s dictation: {took:?}");
    assert!(took < BUDGET, "{took:?}");
}

#[test]
fn a_5000_word_input_with_200_dictionary_entries_is_ready_in_under_50_ms() {
    let text = five_thousand_words();
    let took = slowest(&text, &dictionary_of_200());
    println!(
        "cleanup and 200 replacements, max over {RUNS} runs, {} words: {took:?}",
        text.split_whitespace().count()
    );
    assert!(took < BUDGET, "{took:?}");
}

#[test]
fn the_200_entries_really_change_the_text() {
    let cleaned = apply(
        &clean(DICTATION_45S, &Options::default()),
        &dictionary_of_200(),
    );
    assert!(cleaned.contains("Quarterly report"), "{cleaned}");
    assert!(cleaned.contains("Finance"), "{cleaned}");
    assert!(cleaned.contains("Agenda for Friday"), "{cleaned}");
}
