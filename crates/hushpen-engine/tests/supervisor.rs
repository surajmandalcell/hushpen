//! Supervisor behavior against scripted children (`/bin/sh`). No model is needed.
#![cfg(unix)]

use hushpen_core::error;
use hushpen_core::protocol::{Event, PROTOCOL_VERSION, write_json};
use hushpen_engine::{
    ChildSpec, EngineClient, EngineLog, EngineState, JobOutcome, Timing, TranscribeSpec,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn millis(values: &[u64]) -> Vec<Duration> {
    values.iter().map(|v| Duration::from_millis(*v)).collect()
}

fn fast() -> Timing {
    Timing {
        backoff: millis(&[100, 300, 600]),
        healthy_after: Duration::from_secs(60),
        ping_interval: Duration::from_millis(100),
        pong_timeout: Duration::from_millis(500),
        cancel_grace: Duration::from_millis(200),
        ready_timeout: Duration::from_millis(500),
        submit_wait: Duration::from_millis(400),
    }
}

fn sh(script: &str) -> ChildSpec {
    ChildSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), script.into()],
    }
}

/// A child that sends a valid `Ready` and then ignores every request, pings included.
fn deaf_ready_child(dir: &Path) -> ChildSpec {
    let frame = dir.join("ready.bin");
    let mut bytes = Vec::new();
    write_json(
        &mut bytes,
        &Event::Ready {
            version: "test".into(),
            protocol: PROTOCOL_VERSION,
            gpu: false,
        }
        .to_json(),
    )
    .unwrap();
    std::fs::write(&frame, bytes).unwrap();
    // The shell stays the child, so its stdout pipe stays open while `cat` swallows requests.
    sh(&format!("cat '{}'; cat >/dev/null", frame.display()))
}

fn log_in(dir: &Path) -> (Arc<EngineLog>, PathBuf) {
    let path = dir.join("engine.log");
    (EngineLog::open(&path).unwrap(), path)
}

fn restart_delays(log: &Path) -> Vec<u64> {
    std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .filter(|line| line.contains(" CRASH ") || line.contains(" SPAWN_FAILED "))
        .filter_map(|line| line.split("restart_in_ms=").nth(1))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|ms| ms.parse().ok())
        .collect()
}

#[test]
fn a_child_that_cannot_start_is_retried_with_growing_delays() {
    let dir = tempfile::tempdir().unwrap();
    let (log, log_path) = log_in(dir.path());
    let client = EngineClient::start(
        ChildSpec {
            program: dir.path().join("no-such-program"),
            args: vec![],
        },
        fast(),
        log,
    );
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 4));
    client.shutdown();
    let delays = restart_delays(&log_path);
    assert_eq!(&delays[..4], &[100, 300, 600, 600], "{delays:?}");
    assert_eq!(client.status().state, EngineState::Stopped);
}

#[test]
fn a_child_that_exits_at_once_is_restarted_after_each_delay() {
    let dir = tempfile::tempdir().unwrap();
    let (log, log_path) = log_in(dir.path());
    let client = EngineClient::start(sh("exit 1"), fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 4));
    client.shutdown();
    let delays = restart_delays(&log_path);
    assert_eq!(&delays[..4], &[100, 300, 600, 600], "{delays:?}");
}

#[test]
fn a_child_that_never_says_ready_is_killed_and_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let client = EngineClient::start(sh("cat >/dev/null"), fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.pid.is_some()));
    let first = client.status().pid;
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 1
        && s.pid.is_some()
        && s.pid != first));
    client.shutdown();
}

#[test]
fn a_child_that_stops_answering_pings_is_killed_by_the_watchdog() {
    let dir = tempfile::tempdir().unwrap();
    let (log, log_path) = log_in(dir.path());
    let client = EngineClient::start(deaf_ready_child(dir.path()), fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == EngineState::Ready));
    let first = client.status().pid.unwrap();
    let started = Instant::now();
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 1));
    assert!(started.elapsed() >= Duration::from_millis(300), "too early");
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.state == EngineState::Ready
            && s.pid != Some(first))
    );
    client.shutdown();
    let text = std::fs::read_to_string(log_path).unwrap();
    assert!(text.contains(" WATCHDOG "), "{text}");
    assert!(text.contains("reason=no pong"), "{text}");
}

#[test]
fn a_job_for_an_engine_that_stays_down_fails_with_a_coded_error_after_the_wait() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let client = EngineClient::start(
        ChildSpec {
            program: dir.path().join("no-such-program"),
            args: vec![],
        },
        fast(),
        log,
    );
    let started = Instant::now();
    let job = client.transcribe(TranscribeSpec {
        wav_path: dir.path().join("a.wav"),
        language: None,
        prompt: None,
    });
    match job.wait() {
        JobOutcome::Failed(failure) => assert_eq!(failure.code, error::ENGINE_UNAVAILABLE),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(started.elapsed() >= Duration::from_millis(350));
    client.shutdown();
}

#[test]
fn cancelling_a_job_that_waits_for_the_engine_settles_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let client = EngineClient::start(
        ChildSpec {
            program: dir.path().join("no-such-program"),
            args: vec![],
        },
        Timing {
            submit_wait: Duration::from_secs(30),
            ..fast()
        },
        log,
    );
    let job = client.transcribe(TranscribeSpec {
        wav_path: dir.path().join("a.wav"),
        language: None,
        prompt: None,
    });
    let started = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), JobOutcome::Cancelled);
    assert!(started.elapsed() < Duration::from_millis(500));
    client.shutdown();
}

#[test]
fn a_job_in_flight_when_the_child_dies_fails_with_engine_crashed() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    // Ready, then silence; the job is written to the child and the watchdog kills it.
    let client = EngineClient::start(deaf_ready_child(dir.path()), fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == EngineState::Ready));
    let wav = dir.path().join("a.wav");
    std::fs::write(&wav, b"keep me").unwrap();
    let job = client.transcribe(TranscribeSpec {
        wav_path: wav.clone(),
        language: None,
        prompt: None,
    });
    match job.wait() {
        JobOutcome::Failed(failure) => assert_eq!(failure.code, error::ENGINE_CRASHED),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(&wav).unwrap(),
        b"keep me",
        "the audio is kept"
    );
    client.shutdown();
}

#[test]
fn a_cancel_the_child_ignores_kills_it_within_the_grace_and_restarts_it_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (log, log_path) = log_in(dir.path());
    let timing = Timing {
        pong_timeout: Duration::from_secs(30),
        ..fast()
    };
    let client = EngineClient::start(deaf_ready_child(dir.path()), timing, log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == EngineState::Ready));
    let first = client.status().pid.unwrap();
    let job = client.transcribe(TranscribeSpec {
        wav_path: dir.path().join("a.wav"),
        language: None,
        prompt: None,
    });
    let started = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), JobOutcome::Cancelled);
    let settled = started.elapsed();
    assert!(
        settled < Duration::from_millis(500),
        "settled in {settled:?}"
    );
    let text = std::fs::read_to_string(&log_path).unwrap();
    assert!(
        settled >= Duration::from_millis(150),
        "killed before the grace: {settled:?}\n{text}"
    );
    assert!(
        client.wait_for(Duration::from_secs(2), |s| s.state == EngineState::Ready
            && s.pid != Some(first))
    );
    client.shutdown();
    let text = std::fs::read_to_string(log_path).unwrap();
    assert!(text.contains(" CANCEL_KILL "), "{text}");
    assert!(text.contains("code=CANCELLED"), "{text}");
}

#[test]
fn shutdown_stops_the_child() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let client = EngineClient::start(deaf_ready_child(dir.path()), fast(), log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == EngineState::Ready));
    let started = Instant::now();
    client.shutdown();
    assert!(started.elapsed() < Duration::from_secs(2));
    let status = client.status();
    assert_eq!((status.state, status.pid), (EngineState::Stopped, None));
}
