//! The voice worker supervisor against `fake-tts`, a scripted worker that speaks the real
//! protocol. No model is needed. The supervision is shared with the LLM worker, so this covers
//! what is TTS's own: streamed audio, its codes, and a crash in the middle of speech.
#![cfg(unix)]

use hushpen_core::error;
use hushpen_core::tts_protocol::TtsEngine;
use hushpen_engine::{
    ChildSpec, EngineLog, Timing, TtsAudio, TtsClient, TtsOutcome, TtsReply, TtsSpec, TtsState,
    TtsWork,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn fast() -> Timing {
    Timing {
        backoff: vec![Duration::from_millis(100), Duration::from_millis(300)],
        healthy_after: Duration::from_secs(60),
        ping_interval: Duration::from_millis(100),
        pong_timeout: Duration::from_millis(500),
        cancel_grace: Duration::from_millis(200),
        ready_timeout: Duration::from_millis(1500),
        submit_wait: Duration::from_millis(400),
    }
}

fn started(dir: &Path) -> (TtsClient, PathBuf) {
    let path = dir.join("tts.log");
    let log = EngineLog::open_scoped(&path, "tts").unwrap();
    let spec = ChildSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake-tts")),
        args: Vec::new(),
    };
    let client = TtsClient::start(spec, fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == TtsState::Ready));
    (client, path)
}

fn speak(text: &str) -> TtsWork {
    TtsWork {
        text: text.into(),
        voice: Some("af_heart".into()),
        speed: 1.0,
    }
}

/// Every audio piece of a job, then its outcome.
fn collect(client: &TtsClient, text: &str) -> (Vec<TtsAudio>, TtsOutcome) {
    let job = client.run(speak(text));
    let mut audio = Vec::new();
    loop {
        match job.next(Duration::from_secs(5)) {
            Some(TtsReply::Progress(piece)) => audio.push(piece),
            Some(TtsReply::End(outcome)) => return (audio, outcome),
            None => panic!("no outcome in time"),
        }
    }
}

#[test]
fn a_job_streams_its_audio_in_order_then_ends_with_its_times() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started(dir.path());
    let loaded = client
        .load_model(TtsSpec::new(
            TtsEngine::Kokoro,
            PathBuf::from("/models/tts/kokoro-82m-int8"),
        ))
        .wait()
        .unwrap();
    assert_eq!(loaded.model, "kokoro-82m-int8");
    let status = client.status();
    assert_eq!(status.model.as_deref(), Some("kokoro-82m-int8"));
    assert_eq!(status.backend.as_deref(), Some("cpu"));

    let (audio, outcome) = collect(&client, "Hello there.");
    let lengths: Vec<usize> = audio.iter().map(|piece| piece.samples.len()).collect();
    assert_eq!(lengths, [240, 120]);
    assert!(audio.iter().all(|piece| piece.sample_rate == 24_000));
    let TtsOutcome::Done(stats) = outcome else {
        panic!("a spoken job, got {outcome:?}");
    };
    assert_eq!(stats.audio_ms, 15);
    client.shutdown();

    let text = std::fs::read_to_string(log).unwrap();
    assert!(
        text.contains("kind=speak model=kokoro-82m-int8 backend=cpu"),
        "{text}"
    );
    assert!(text.contains("audio_ms=15 first_audio_ms=1"), "{text}");
    assert!(
        !text.contains("Hello there"),
        "the log holds no text: {text}"
    );
}

#[test]
fn cancel_ends_a_job_that_is_still_speaking() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started(dir.path());
    let job = client.run(speak("hang on"));
    assert!(job.wait_timeout(Duration::from_millis(100)).is_none());
    job.cancel();
    assert_eq!(job.wait(), TtsOutcome::Cancelled);
    client.shutdown();
}

#[test]
fn a_crash_while_speaking_fails_the_job_and_the_worker_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started(dir.path());
    let (_, outcome) = collect(&client, "crash now");
    let TtsOutcome::Failed(failure) = outcome else {
        panic!("a failed job, got {outcome:?}");
    };
    assert_eq!(failure.code, error::TTS_CRASHED);
    assert!(client.wait_for(Duration::from_secs(5), |s| {
        s.state == TtsState::Ready && s.restarts == 1
    }));
    let (audio, outcome) = collect(&client, "Back again.");
    assert_eq!(audio.len(), 2);
    assert!(matches!(outcome, TtsOutcome::Done(_)));
    client.shutdown();
}

#[test]
fn a_failed_load_gets_the_load_code() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started(dir.path());
    let failure = client
        .load_model(TtsSpec::new(
            TtsEngine::Kokoro,
            PathBuf::from("/models/tts/missing"),
        ))
        .wait()
        .unwrap_err();
    assert_eq!(failure.code, error::TTS_LOAD_FAILED);
    client.shutdown();
}
