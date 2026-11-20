//! The real Nemotron Unified engine on long batch input.
//!
//! Inputs: `HUSHPEN_TEST_UNIFIED_MODEL` (nemotron-labs-unified-asr-0.6b-q8_0.gguf),
//! `HUSHPEN_TEST_UNIFIED_WAV`, 60 s of speech that full attention used to return empty
//! (the bench 30 min file cut at 880 s: `bench/speed/unified-empty-window-880s.wav`), and
//! `HUSHPEN_TEST_UNIFIED_TAIL_WAV`, the bench 1 minute file (`bench/speed/len_060.wav`),
//! which stops mid-sentence and used to lose its last 2.5 s of words.

use super::read_wav;
use hushpen_engine::{AsrEngine, CancelFlag, LoadOptions, TranscribeOptions};
use std::path::PathBuf;

fn input(name: &str) -> PathBuf {
    std::env::var_os(name)
        .unwrap_or_else(|| panic!("set {name}"))
        .into()
}

fn engine() -> Box<dyn AsrEngine> {
    let loader = hushpen_engine::server_config().loader;
    loader(
        &input("HUSHPEN_TEST_UNIFIED_MODEL"),
        LoadOptions {
            gpu: cfg!(target_os = "macos"),
            threads: 4,
        },
    )
    .expect("load Unified")
}

fn heard(engine: &mut dyn AsrEngine, wav: &str) -> (usize, String) {
    let pcm = read_wav(&input(wav));
    let result = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe");
    (result.segments.len(), result.text)
}

#[test]
fn a_one_minute_window_keeps_its_words() {
    let mut engine = engine();
    // The speaker says about 130 words in this minute; the bug returned none.
    let (words, text) = heard(engine.as_mut(), "HUSHPEN_TEST_UNIFIED_WAV");
    assert!(words > 100, "only {words} words: {text}");
    // The file stops mid-sentence: "... or maybe you're a politician in a developing".
    let (_, text) = heard(engine.as_mut(), "HUSHPEN_TEST_UNIFIED_TAIL_WAV");
    assert!(
        text.ends_with("politician in a developing"),
        "lost the last words: {text}"
    );
}
