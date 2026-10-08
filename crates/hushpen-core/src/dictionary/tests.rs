use super::*;
use crate::cleanup::{Options, clean};

fn word(id: i64, phrase: &str) -> Entry {
    Entry {
        id,
        phrase: phrase.to_owned(),
        heard_as: None,
    }
}

fn replace(id: i64, heard: &str, write: &str) -> Entry {
    Entry {
        id,
        phrase: write.to_owned(),
        heard_as: Some(heard.to_owned()),
    }
}

fn run(text: &str, entries: &[Entry]) -> String {
    apply(text, entries)
}

#[test]
fn a_replacement_changes_whole_words_in_any_case() {
    let entries = [replace(1, "cat", "Kat")];
    for input in ["cat", "Cat", "CAT"] {
        assert_eq!(run(input, &entries), "Kat", "{input}");
    }
    assert_eq!(run("a cat sat", &entries), "a Kat sat");
}

#[test]
fn a_replacement_leaves_longer_words_alone() {
    let entries = [replace(1, "cat", "Kat")];
    assert_eq!(run("category", &entries), "category");
    assert_eq!(run("bobcat", &entries), "bobcat");
    assert_eq!(run("cats and bobcats", &entries), "cats and bobcats");
    assert_eq!(run("cat_food", &entries), "cat_food");
}

#[test]
fn punctuation_is_a_word_boundary() {
    let entries = [replace(1, "cat", "Kat")];
    assert_eq!(
        run("The cat, the cat. (cat) cat's", &entries),
        "The Kat, the Kat. (Kat) Kat's"
    );
}

#[test]
fn fillers_and_spoken_punctuation_run_before_the_replacement() {
    let entries = [replace(1, "fox", "Foxtrel")];
    let cleaned = clean("um fox comma fox", &Options::default());
    assert_eq!(run(&cleaned, &entries), "Foxtrel, Foxtrel.");
}

#[test]
fn the_replacement_is_written_as_stored() {
    let entries = [
        replace(1, "dog", "café-ß-🎤"),
        replace(2, "iphone", "iPhone"),
    ];
    assert_eq!(run("Dog and IPHONE", &entries), "café-ß-🎤 and iPhone");
}

#[test]
fn a_heard_as_phrase_matches_across_any_spacing_and_case() {
    let entries = [replace(1, "new york", "NYC")];
    assert_eq!(run("in New  York today", &entries), "in NYC today");
    assert_eq!(run("in new\nyork", &entries), "in NYC");
    assert_eq!(run("newyork", &entries), "newyork");
    assert_eq!(run("renew york", &entries), "renew york");
}

#[test]
fn the_longer_phrase_wins_where_two_match() {
    let entries = [replace(1, "new", "N"), replace(2, "new york", "NYC")];
    assert_eq!(run("new york and new jersey", &entries), "NYC and N jersey");
}

#[test]
fn a_written_phrase_is_not_replaced_again() {
    let entries = [replace(1, "a", "b"), replace(2, "b", "c")];
    assert_eq!(run("a b", &entries), "b c");
}

#[test]
fn non_ascii_words_match_ignoring_case() {
    let entries = [replace(1, "über", "Uber"), replace(2, "ß", "ss")];
    assert_eq!(
        run("ÜBER and über, größe", &entries),
        "Uber and Uber, größe"
    );
    assert_eq!(run("ß", &entries), "ss");
}

#[test]
fn an_entry_with_symbols_matches_without_a_word_boundary_on_the_symbol_side() {
    let entries = [replace(1, "c++", "C plus plus")];
    assert_eq!(
        run("I like c++ a lot, c+++", &entries),
        "I like C plus plus a lot, C plus plus+"
    );
}

#[test]
fn words_without_a_heard_as_change_nothing() {
    let entries = [word(1, "Zyxtrel")];
    assert_eq!(run("zyxtrel", &entries), "zyxtrel");
    assert_eq!(run("", &[replace(1, "x", "y")]), "");
}

#[test]
fn the_prompt_lists_phrases_oldest_first_and_without_repeats() {
    let entries = [
        word(3, "Third"),
        word(1, "First"),
        replace(2, "frst", "Second"),
        word(4, "FIRST"),
    ];
    assert_eq!(build_prompt(&entries), "First, Second, Third");
    assert_eq!(build_prompt(&[]), "");
}

#[test]
fn the_prompt_uses_the_write_as_phrase_not_the_heard_as_text() {
    assert_eq!(build_prompt(&[replace(1, "fox", "Foxtrel")]), "Foxtrel");
}

#[test]
fn the_prompt_stays_within_the_token_cap_and_keeps_whole_phrases() {
    let entries: Vec<Entry> = (1..=400).map(|n| word(n, &format!("word{n:03}"))).collect();
    let prompt = build_prompt(&entries);
    let chars = prompt.chars().count();
    assert!(estimated_tokens(chars) <= PROMPT_TOKEN_CAP, "{chars} chars");
    // 7-char words and a 2-char separator: 66 words are 592 chars; a 67th would pass 600.
    assert_eq!(prompt.split(", ").count(), 66);
    assert!(prompt.starts_with("word001, word002, "));
    assert!(prompt.ends_with("word066"));
    for piece in prompt.split(", ") {
        assert!(piece.starts_with("word") && piece.len() == 7, "{piece}");
    }
}

#[test]
fn the_cap_counts_characters_not_bytes() {
    let entries: Vec<Entry> = (1..=400).map(|n| word(n, &format!("é{n:05}"))).collect();
    let prompt = build_prompt(&entries);
    assert!(estimated_tokens(prompt.chars().count()) <= PROMPT_TOKEN_CAP);
    assert!(prompt.len() > prompt.chars().count());
    assert!(prompt.chars().count() > 580);
}

#[test]
fn a_phrase_that_does_not_fit_is_dropped_whole_and_later_short_ones_still_fit() {
    let huge = "x".repeat(700);
    let entries = [word(1, "Alpha"), word(2, &huge), word(3, "Omega")];
    assert_eq!(build_prompt(&entries), "Alpha, Omega");
}

#[test]
fn the_prompt_is_exactly_at_the_cap_when_the_words_fill_it() {
    let entries: Vec<Entry> = ["a", "b", "c"]
        .iter()
        .zip(1..)
        .map(|(letter, id)| word(id, &letter.repeat(198)))
        .collect();
    // 198 + 2 + 198 + 2 + 198 = 598 chars: ceil(598 / 4) = 150.
    let prompt = build_prompt(&entries);
    assert_eq!(prompt.chars().count(), 598);
    assert_eq!(estimated_tokens(598), 150);
    let more = [entries.clone(), vec![word(4, "bb")]].concat();
    // Adding ", bb" makes 602 chars, which is 151 tokens.
    assert_eq!(build_prompt(&more), prompt);
}

#[test]
fn the_same_entries_give_the_same_prompt_in_any_input_order() {
    let mut entries: Vec<Entry> = (1..=80).map(|n| word(n, &format!("name{n}"))).collect();
    let first = build_prompt(&entries);
    assert_eq!(build_prompt(&entries), first);
    entries.reverse();
    assert_eq!(build_prompt(&entries), first);
}

#[test]
fn estimated_tokens_round_up() {
    assert_eq!(estimated_tokens(0), 0);
    assert_eq!(estimated_tokens(1), 1);
    assert_eq!(estimated_tokens(4), 1);
    assert_eq!(estimated_tokens(5), 2);
    assert_eq!(estimated_tokens(600), 150);
    assert_eq!(estimated_tokens(601), 151);
}

#[test]
fn validate_trims_and_collapses_spaces() {
    assert_eq!(
        validate("  New   York ", Some("  new  york "), &[], None),
        Ok(("New York".to_owned(), Some("new york".to_owned())))
    );
}

#[test]
fn validate_turns_a_blank_heard_as_into_none() {
    assert_eq!(
        validate("Zyxtrel", Some("   "), &[], None),
        Ok(("Zyxtrel".to_owned(), None))
    );
    assert_eq!(
        validate("Zyxtrel", None, &[], None),
        Ok(("Zyxtrel".to_owned(), None))
    );
}

#[test]
fn validate_refuses_an_empty_phrase() {
    assert_eq!(validate("", None, &[], None), Err(Refusal::EmptyPhrase));
    assert_eq!(
        validate(" \t\n", Some("fox"), &[], None),
        Err(Refusal::EmptyPhrase)
    );
    assert!(Refusal::EmptyPhrase.message().contains("Enter"));
}

#[test]
fn validate_refuses_a_duplicate_phrase_in_any_case() {
    let existing = [word(1, "fox"), word(2, "Café")];
    assert_eq!(
        validate("FOX", None, &existing, None),
        Err(Refusal::DuplicatePhrase("FOX".to_owned()))
    );
    assert_eq!(
        validate("CAFÉ", None, &existing, None),
        Err(Refusal::DuplicatePhrase("CAFÉ".to_owned()))
    );
    assert!(validate("foxes", None, &existing, None).is_ok());
}

#[test]
fn validate_refuses_a_second_replacement_for_the_same_heard_as() {
    let existing = [replace(1, "fox", "Foxtrel")];
    assert_eq!(
        validate("Vixen", Some("FOX"), &existing, None),
        Err(Refusal::DuplicateHeardAs("FOX".to_owned()))
    );
}

#[test]
fn validate_lets_an_entry_keep_its_own_values_when_edited() {
    let existing = [replace(1, "fox", "Foxtrel"), word(2, "Zyxtrel")];
    assert!(validate("Foxtrel", Some("fox"), &existing, Some(1)).is_ok());
    assert!(validate("Vixen", Some("fox"), &existing, Some(1)).is_ok());
    assert_eq!(
        validate("zyxtrel", None, &existing, Some(1)),
        Err(Refusal::DuplicatePhrase("zyxtrel".to_owned()))
    );
}
