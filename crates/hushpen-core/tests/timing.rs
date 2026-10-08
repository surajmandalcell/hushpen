//! The rule pass must stay far below the 50 ms budget per dictation.

use std::time::{Duration, Instant};

use hushpen_core::cleanup::{Options, clean};

const BUDGET: Duration = Duration::from_millis(50);
const RUNS: usize = 100;

/// About 45 s of speech: 110 words with fillers, repeats, and spoken punctuation.
const DICTATION_45S: &str = "um so today I want to talk about the the quarterly report comma and uh how it \
went period you know the numbers were, like, better than we expected comma no wait comma much better \
than we expected period new paragraph first we need to review the budget and then we need to send it \
to the team period scratch that period we need to send it to finance question mark yes period I think \
that that is the right call and she had had enough of waiting period new line please add it to the \
agenda for friday and um let me know if there are any questions period";

fn slowest(text: &str) -> Duration {
    let options = Options::default();
    (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            let cleaned = clean(text, &options);
            let took = start.elapsed();
            assert!(!cleaned.is_empty());
            took
        })
        .max()
        .unwrap()
}

#[test]
fn a_45_second_dictation_cleans_in_under_50_ms() {
    let took = slowest(DICTATION_45S);
    println!("cleanup max over {RUNS} runs, 45 s dictation: {took:?}");
    assert!(took < BUDGET, "{took:?}");
}

#[test]
fn a_5000_word_input_cleans_in_under_50_ms() {
    let text = DICTATION_45S.repeat(5000 / DICTATION_45S.split_whitespace().count() + 1);
    let took = slowest(&text);
    println!(
        "cleanup max over {RUNS} runs, {} words: {took:?}",
        text.split_whitespace().count()
    );
    assert!(took < BUDGET, "{took:?}");
}
