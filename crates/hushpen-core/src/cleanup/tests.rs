use super::{Options, clean};

fn on(raw: &str) -> String {
    clean(raw, &Options::default())
}

fn without_spoken_punctuation(raw: &str) -> String {
    clean(
        raw,
        &Options {
            spoken_punctuation: false,
        },
    )
}

#[test]
fn fillers_go_but_like_stays_when_it_carries_meaning() {
    assert_eq!(on("um I think uh we should go"), "I think we should go.");
    assert_eq!(on("er, you know, it works"), "It works.");
    assert_eq!(on("I like apples"), "I like apples.");
    assert_eq!(on("it was, like, huge"), "It was huge.");
}

#[test]
fn repeated_words_collapse_except_allowed_doubles() {
    assert_eq!(on("the the cat sat"), "The cat sat.");
    assert_eq!(
        on("I think that that is fine"),
        "I think that that is fine."
    );
    assert_eq!(on("she had had enough"), "She had had enough.");
}

#[test]
fn spoken_punctuation_becomes_punctuation() {
    assert_eq!(on("hello comma world period"), "Hello, world.");
    assert_eq!(on("is it ready question mark"), "Is it ready?");
    assert_eq!(on("first new line second"), "First\nSecond");
    assert_eq!(on("first new paragraph second"), "First\n\nSecond");
}

#[test]
fn the_spoken_punctuation_setting_keeps_the_words() {
    assert_eq!(
        without_spoken_punctuation("hello comma world"),
        "Hello comma world."
    );
}

#[test]
fn scratch_that_and_the_narrow_self_correction() {
    assert_eq!(
        on("Send it today. Scratch that. Send it tomorrow."),
        "Send it tomorrow."
    );
    assert_eq!(on("Scratch that. Hello."), "Hello.");
    assert_eq!(on("meet at 3, no wait, 4"), "Meet at 4.");
    assert_eq!(on("I could not wait for it"), "I could not wait for it.");
    assert_eq!(on("no wait time is needed"), "No wait time is needed.");
}

#[test]
fn capitalization_spacing_and_empty_input() {
    assert_eq!(on("  hello   world . next"), "Hello world. Next.");
    assert_eq!(on("hello , world"), "Hello, world.");
    assert_eq!(on(""), "");
    assert_eq!(on("um uh er"), "");
    assert_eq!(on(" , . "), "");
}

#[test]
fn the_pass_is_stable_on_its_own_output() {
    for raw in [
        "um so the the report is due on friday comma no wait comma monday period",
        "Send it today. Scratch that. Send it tomorrow.",
    ] {
        let once = on(raw);
        assert_eq!(on(&once), once, "second pass changed {once:?}");
    }
}

#[test]
fn odd_input_never_panics() {
    for raw in [
        "\u{0}",
        "…",
        "....",
        ",,,",
        "?!",
        "comma",
        "new line",
        "scratch that",
        "no wait",
        ", no wait,",
        "a, no wait, b c d e",
        "🙂 🙂",
        "um, um, um,",
        "\"",
        "I",
        "ß",
    ] {
        let _ = on(raw);
        let _ = without_spoken_punctuation(raw);
    }
}
