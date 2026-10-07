//! Heavy integration tests: the real whisper engine with a real model.
//!
//! Inputs: `HUSHPEN_TEST_MODEL` (a ggml model file) or `HUSHPEN_TEST_ASSETS`
//! (`models/whisper/ggml-tiny.en.bin`). The short clip falls back to `tests/fixtures`.
//! `HUSHPEN_TEST_GPU=1` turns on the GPU test (Metal, macOS only).
#![cfg(feature = "heavy")]

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use hushpen_engine::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, TranscribeOptions, WhisperEngine,
};

// The child process and supervisor tests share the helpers below.
#[path = "it/child.rs"]
mod child;

fn assets_dir() -> Option<PathBuf> {
    std::env::var_os("HUSHPEN_TEST_ASSETS").map(PathBuf::from)
}

fn model_path() -> PathBuf {
    if let Some(path) = std::env::var_os("HUSHPEN_TEST_MODEL") {
        return path.into();
    }
    let path = assets_dir()
        .expect("set HUSHPEN_TEST_MODEL or HUSHPEN_TEST_ASSETS")
        .join("models/whisper/ggml-tiny.en.bin");
    assert!(path.exists(), "missing model {}", path.display());
    path
}

fn short_wav() -> PathBuf {
    let shared = assets_dir().map(|d| d.join("fixtures/speech-short.wav"));
    match shared {
        Some(path) if path.exists() => path,
        _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/speech-short.wav"),
    }
}

fn read_wav(path: &std::path::Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("open wav");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000);
    assert_eq!(spec.channels, 1);
    reader
        .samples::<i16>()
        .map(|s| f32::from(s.expect("sample")) / 32768.0)
        .collect()
}

/// The 10 minute fixture when the shared assets have it, else the short clip repeated.
fn long_pcm() -> Vec<f32> {
    if let Some(dir) = assets_dir() {
        let path = dir.join("fixtures/long-600s.wav");
        if path.exists() {
            return read_wav(&path);
        }
    }
    let clip = read_wav(&short_wav());
    clip.iter().copied().cycle().take(16_000 * 600).collect()
}

fn words(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn load(gpu: bool) -> WhisperEngine {
    WhisperEngine::load(&model_path(), LoadOptions { gpu, threads: 2 }).expect("load model")
}

fn english() -> TranscribeOptions {
    TranscribeOptions {
        language: Some("en".into()),
        prompt: None,
    }
}

fn assert_expected_words(text: &str) {
    let got = words(text);
    assert!(
        got.contains("the quick brown fox jumps over the lazy dog numbers"),
        "unexpected transcript: {got}"
    );
    assert!(
        got.ends_with("1 2 3 4 5") || got.ends_with("one two three four five"),
        "unexpected transcript tail: {got}"
    );
}

#[test]
fn transcribes_short_clip_on_cpu() {
    let mut engine = load(false);
    assert!(!engine.gpu_in_use());
    let result = engine
        .transcribe(&read_wav(&short_wav()), &english(), &CancelFlag::new())
        .expect("transcribe");
    assert_expected_words(&result.text);
    assert_eq!(result.language, "en");
    assert!(!result.segments.is_empty());
    let mut last_start = 0;
    for segment in &result.segments {
        assert!(segment.start_ms < segment.end_ms);
        assert!(segment.start_ms >= last_start);
        last_start = segment.start_ms;
    }
}

#[test]
fn transcribes_short_clip_on_gpu() {
    if std::env::var("HUSHPEN_TEST_GPU").as_deref() != Ok("1") {
        eprintln!("skipped: HUSHPEN_TEST_GPU is not 1");
        return;
    }
    let mut engine = load(true);
    assert_eq!(engine.gpu_in_use(), cfg!(target_os = "macos"));
    let result = engine
        .transcribe(&read_wav(&short_wav()), &english(), &CancelFlag::new())
        .expect("transcribe");
    assert_expected_words(&result.text);
}

#[test]
fn detects_language_when_none_is_given() {
    let mut engine = load(false);
    let result = engine
        .transcribe(
            &read_wav(&short_wav()),
            &TranscribeOptions::default(),
            &CancelFlag::new(),
        )
        .expect("transcribe");
    assert_eq!(result.language, "en");
}

#[test]
fn rejects_an_unknown_language_code() {
    let mut engine = load(false);
    let err = engine
        .transcribe(
            &read_wav(&short_wav()),
            &TranscribeOptions {
                language: Some("xx".into()),
                prompt: None,
            },
            &CancelFlag::new(),
        )
        .expect_err("bad language");
    assert_eq!(err, EngineError::BadLanguage("xx".into()));
}

#[test]
fn a_missing_model_is_a_load_error() {
    let err = WhisperEngine::load(
        std::path::Path::new("/nonexistent/model.bin"),
        LoadOptions {
            gpu: false,
            threads: 2,
        },
    )
    .err()
    .expect("load must fail");
    assert_eq!(err.code(), Some("ENGINE_LOAD_FAILED"));
}

#[test]
fn a_flag_set_before_the_job_cancels_at_once() {
    let mut engine = load(false);
    let cancel = CancelFlag::new();
    cancel.cancel();
    let started = Instant::now();
    let err = engine
        .transcribe(&read_wav(&short_wav()), &english(), &cancel)
        .expect_err("cancelled");
    let took = started.elapsed();
    eprintln!("pre-set flag: returned after {took:?}");
    assert_eq!(err, EngineError::Cancelled);
    assert!(took < Duration::from_secs(1), "took {took:?}");
}

/// Aborts a 10 minute job once at 0.5 s (encoder phase) and once at 5 s (decode phase).
#[test]
fn abort_of_a_long_job_returns_within_one_second() {
    let mut engine = load(false);
    let pcm = long_pcm();
    for delay_ms in [500_u64, 5_000] {
        let cancel = CancelFlag::new();
        let trigger = cancel.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let timer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(delay_ms));
            trigger.cancel();
            tx.send(Instant::now()).expect("send");
        });
        let result = engine.transcribe(&pcm, &english(), &cancel);
        let returned = Instant::now();
        timer.join().expect("timer");
        let set_at = rx.recv().expect("set time");
        let latency = returned.saturating_duration_since(set_at);
        eprintln!("abort at {delay_ms} ms: returned {latency:?} after the flag was set");
        assert_eq!(
            result,
            Err(EngineError::Cancelled),
            "abort at {delay_ms} ms"
        );
        assert!(latency < Duration::from_secs(1), "abort took {latency:?}");
    }
}

/// 130 s of speech is two windows; the merge must keep time order and not repeat the overlap.
#[test]
fn long_input_is_merged_in_time_order() {
    let mut engine = load(false);
    let clip = read_wav(&short_wav());
    let pcm: Vec<f32> = clip.iter().copied().cycle().take(16_000 * 130).collect();
    let result = engine
        .transcribe(&pcm, &english(), &CancelFlag::new())
        .expect("transcribe");
    // A segment that straddles the hand-over point may start inside the previous one, but the
    // end times still rise.
    let mut last_end = 0;
    for segment in &result.segments {
        assert!(segment.start_ms < segment.end_ms);
        assert!(segment.end_ms >= last_end, "segments out of order");
        last_end = segment.end_ms;
    }
    assert!(last_end > 120_000, "no segment from the second window");
    let fox = words(&result.text).matches("quick brown fox").count();
    assert!(
        (22..=27).contains(&fox),
        "expected about 25 repeats, got {fox}"
    );
}

/// The raw callback must not stop a job whose flag is never set.
#[test]
fn a_job_without_cancel_runs_to_the_full_text() {
    let mut engine = load(false);
    let cancel = CancelFlag::new();
    let result = engine
        .transcribe(&read_wav(&short_wav()), &english(), &cancel)
        .expect("transcribe");
    assert!(!cancel.is_cancelled());
    assert_expected_words(&result.text);
}
