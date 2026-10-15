//! Imports through the real engine child: every format symphonia reads, the 10 minute file
//! across its windows, and the files that must be refused.
//!
//! Fixtures come from `HUSHPEN_TEST_FIXTURES` (the AIFF and ALAC files) and
//! `HUSHPEN_TEST_ASSETS/fixtures` (the rest). The long file uses `ggml-base.en`.

use super::child::{Started, load_spec, start_client};
use super::{assert_expected_words, model_path};
use hushpen_core::error;
use hushpen_engine::{ImportSpec, JobOutcome, Timing, Transcription};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> PathBuf {
    let places = [
        std::env::var_os("HUSHPEN_TEST_FIXTURES").map(PathBuf::from),
        std::env::var_os("HUSHPEN_TEST_ASSETS").map(|dir| PathBuf::from(dir).join("fixtures")),
    ];
    places
        .into_iter()
        .flatten()
        .map(|dir| dir.join(name))
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("the fixture {name} is in neither fixtures folder"))
}

fn spec(path: PathBuf) -> ImportSpec {
    ImportSpec {
        path,
        language: Some("en".into()),
        prompt: None,
    }
}

fn done(outcome: JobOutcome) -> Transcription {
    match outcome {
        JobOutcome::Done(done) => done,
        other => panic!("expected a transcript, got {other:?}"),
    }
}

fn failure_code(outcome: JobOutcome) -> (String, String) {
    match outcome {
        JobOutcome::Failed(failure) => (failure.code, failure.detail),
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn started_with_tiny() -> Started {
    let started = start_client(Timing::default());
    started
        .client
        .load_model(load_spec(&model_path()))
        .wait()
        .expect("load tiny.en");
    started
}

#[test]
fn every_supported_format_gives_the_expected_words() {
    let Started { client, .. } = started_with_tiny();
    for name in [
        "speech-short.wav",
        "speech-short.mp3",
        "speech-short.flac",
        "speech-short.m4a",
        "speech-short.ogg",
        "speech-short.aiff",
        "speech-short-alac.m4a",
    ] {
        let result = done(client.import(spec(fixture(name)), None).wait());
        eprintln!("{name}: {}", result.text);
        assert_expected_words(&result.text);
        assert!(!result.segments.is_empty(), "{name} has no segments");
        assert!(
            (4_500..5_700).contains(&result.audio_ms),
            "{name} is {} ms long",
            result.audio_ms
        );
    }
    client.shutdown();
}

#[test]
fn an_import_reports_progress_that_only_goes_up() {
    let Started { client, .. } = started_with_tiny();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&heard);
    let outcome = client
        .import(
            spec(fixture("speech-short.flac")),
            Some(Arc::new(move |fraction| {
                sink.lock().unwrap().push(fraction)
            })),
        )
        .wait();
    assert_expected_words(&done(outcome).text);
    let heard = heard.lock().unwrap().clone();
    assert!(heard.len() >= 2, "{heard:?}");
    assert!(heard.windows(2).all(|pair| pair[0] <= pair[1]), "{heard:?}");
    assert_eq!(heard.last(), Some(&1.0));
    client.shutdown();
}

#[test]
fn a_10_minute_file_keeps_every_word_across_the_windows() {
    let base = std::env::var_os("HUSHPEN_TEST_ASSETS")
        .map(|dir| PathBuf::from(dir).join("models/whisper/ggml-base.en.bin"))
        .filter(|path| path.exists())
        .expect("ggml-base.en.bin is in HUSHPEN_TEST_ASSETS/models/whisper");
    let Started {
        client,
        log_path,
        dir: _dir,
    } = start_client(Timing::default());
    client
        .load_model(load_spec(&base))
        .wait()
        .expect("load base.en");
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&heard);
    let result = done(
        client
            .import(
                spec(fixture("long-600s.wav")),
                Some(Arc::new(move |fraction| {
                    sink.lock().unwrap().push(fraction)
                })),
            )
            .wait(),
    );
    client.shutdown();

    let text = result.text.to_lowercase();
    let count = |word: &str| text.matches(word).count();
    // base.en spells the word both ways within one file, so both count.
    let harbor = count("harbor") + count("harbour");
    assert!(
        (11..=13).contains(&count("lighthouse")),
        "lighthouse: {}",
        count("lighthouse")
    );
    assert!((11..=13).contains(&harbor), "harbor: {harbor}");
    let starts: Vec<u64> = result.segments.iter().map(|s| s.start_ms).collect();
    assert!(
        starts.windows(2).all(|pair| pair[0] < pair[1]),
        "segment starts must rise: {starts:?}"
    );
    assert!(
        result.segments.last().is_some_and(|s| s.end_ms > 590_000),
        "the last segment ends too early: {:?}",
        result.segments.last()
    );
    assert!(result.audio_ms >= 599_000);

    let log = std::fs::read_to_string(log_path).unwrap();
    let windows: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("window "))
        .collect();
    assert_eq!(windows.len(), 6, "{windows:#?}");
    assert!(
        windows[0].contains("window 1/6 0..120 s, overlap 10 s"),
        "{}",
        windows[0]
    );
    assert!(
        windows[1].contains("window 2/6 110..230 s"),
        "{}",
        windows[1]
    );
    let heard = heard.lock().unwrap().clone();
    assert!(heard.windows(2).all(|pair| pair[0] <= pair[1]), "{heard:?}");
    assert!(heard.len() >= 7, "one report for each window: {heard:?}");
}

#[test]
fn opus_webm_and_video_are_refused_with_a_named_format() {
    let Started { client, .. } = started_with_tiny();
    for (name, named) in [
        ("speech-short.opus", "Opus"),
        ("speech-short.webm", "WebM"),
        ("speech-short.mp4", "Video"),
        ("speech-short.mkv", "Matroska"),
    ] {
        let started = Instant::now();
        let (code, detail) = failure_code(client.import(spec(fixture(name)), None).wait());
        assert_eq!(code, error::IMPORT_UNSUPPORTED_CODEC, "{name}");
        assert!(detail.contains(named), "{name}: {detail}");
        assert!(started.elapsed() < Duration::from_secs(10), "{name} hung");
    }
    let still_works = done(
        client
            .import(spec(fixture("speech-short.wav")), None)
            .wait(),
    );
    assert_expected_words(&still_works.text);
    client.shutdown();
}

#[test]
fn corrupt_and_empty_files_are_decode_failures_and_the_next_import_works() {
    let Started { client, .. } = started_with_tiny();
    let pid = client.status().pid;
    for name in ["corrupt-random.mp3", "corrupt-text-as.wav", "empty.wav"] {
        let (code, _) = failure_code(client.import(spec(fixture(name)), None).wait());
        assert_eq!(code, error::IMPORT_DECODE_FAILED, "{name}");
    }
    assert_eq!(
        client.status().pid,
        pid,
        "a bad file must not kill the engine"
    );
    assert_expected_words(
        &done(
            client
                .import(spec(fixture("speech-short.wav")), None)
                .wait(),
        )
        .text,
    );
    client.shutdown();
}

#[test]
fn a_device_is_refused_by_the_child_without_a_hang() {
    let Started { client, .. } = started_with_tiny();
    let started = Instant::now();
    let (code, _) = failure_code(client.import(spec("/dev/zero".into()), None).wait());
    assert_eq!(code, error::IMPORT_DECODE_FAILED);
    assert!(started.elapsed() < Duration::from_secs(5));
    client.shutdown();
}

#[test]
fn cancelling_a_long_import_settles_in_under_a_second() {
    let Started { client, .. } = started_with_tiny();
    let job = client.import(spec(fixture("long-600s.wav")), None);
    std::thread::sleep(Duration::from_millis(1_500));
    let asked = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), JobOutcome::Cancelled);
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "{:?}",
        asked.elapsed()
    );
    assert_expected_words(
        &done(
            client
                .import(spec(fixture("speech-short.wav")), None)
                .wait(),
        )
        .text,
    );
    client.shutdown();
}
