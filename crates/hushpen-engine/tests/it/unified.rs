//! The real Nemotron Unified engine on a long batch window.
//!
//! Inputs: `HUSHPEN_TEST_UNIFIED_MODEL` (nemotron-labs-unified-asr-0.6b-q8_0.gguf) and
//! `HUSHPEN_TEST_UNIFIED_WAV`, 60 s of speech that full attention used to return empty
//! (the bench 30 min file cut at 880 s: `bench/speed/unified-empty-window-880s.wav`).

use super::read_wav;
use hushpen_engine::{CancelFlag, LoadOptions, TranscribeOptions};
use std::path::PathBuf;

fn input(name: &str) -> PathBuf {
    std::env::var_os(name)
        .unwrap_or_else(|| panic!("set {name}"))
        .into()
}

#[test]
fn a_one_minute_window_keeps_its_words() {
    let model = input("HUSHPEN_TEST_UNIFIED_MODEL");
    let loader = hushpen_engine::server_config().loader;
    let mut engine = loader(
        &model,
        LoadOptions {
            gpu: cfg!(target_os = "macos"),
            threads: 4,
        },
    )
    .expect("load Unified");
    let pcm = read_wav(&input("HUSHPEN_TEST_UNIFIED_WAV"));
    let result = engine
        .transcribe(&pcm, &TranscribeOptions::default(), &CancelFlag::new())
        .expect("transcribe");
    // The speaker says about 130 words in this minute; the bug returned none.
    assert!(
        result.segments.len() > 100,
        "only {} words: {}",
        result.segments.len(),
        result.text
    );
}
