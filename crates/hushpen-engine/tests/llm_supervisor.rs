//! The LLM supervisor against `fake-llm`, a scripted worker that speaks the real protocol, and
//! against `/bin/sh` children. No model is needed. Every failure ends in a coded outcome the
//! caller turns into the rule text, and the worker comes back.
#![cfg(unix)]

use hushpen_core::error;
use hushpen_core::llm_protocol::UnloadReason;
use hushpen_engine::{
    ChildSpec, EngineLog, LlmClient, LlmOutcome, LlmSpec, LlmState, LlmWork, Timing,
};
use std::path::{Path, PathBuf};
use std::process::Command;
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
        ready_timeout: Duration::from_millis(1500),
        submit_wait: Duration::from_millis(400),
    }
}

fn fake(mode: &str) -> ChildSpec {
    ChildSpec {
        program: PathBuf::from(env!("CARGO_BIN_EXE_fake-llm")),
        args: vec![mode.into()],
    }
}

fn sh(script: &str) -> ChildSpec {
    ChildSpec {
        program: PathBuf::from("/bin/sh"),
        args: vec!["-c".into(), script.into()],
    }
}

fn log_in(dir: &Path) -> (Arc<EngineLog>, PathBuf) {
    let path = dir.join("llm.log");
    (EngineLog::open_scoped(&path, "llm").unwrap(), path)
}

fn cleanup(user: &str) -> LlmWork {
    LlmWork::Cleanup {
        system: "fixed system prompt".into(),
        user: user.into(),
        max_tokens: 64,
    }
}

fn spec(name: &str) -> LlmSpec {
    LlmSpec::new(PathBuf::from(format!("/models/{name}.gguf")))
}

fn signal(pid: u32, name: &str) {
    let status = Command::new("kill")
        .arg(format!("-{name}"))
        .arg(pid.to_string())
        .status()
        .unwrap();
    assert!(status.success(), "kill -{name} {pid} failed");
}

fn started(mode: &str, dir: &Path, timing: Timing) -> (LlmClient, PathBuf) {
    let (log, path) = log_in(dir);
    let client = LlmClient::start(fake(mode), timing, log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready));
    (client, path)
}

fn loaded(client: &LlmClient, name: &str) {
    client.load_model(spec(name)).wait().unwrap();
    assert!(client.wait_for(Duration::from_secs(5), |s| s.model.is_some()));
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

fn done_text(outcome: LlmOutcome) -> String {
    match outcome {
        LlmOutcome::Done(done) => done.text,
        other => panic!("expected a result, got {other:?}"),
    }
}

#[test]
fn a_loaded_worker_answers_a_cleanup_and_the_log_line_has_counts_but_no_text() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    let done = client.load_model(spec("qwen-test")).wait().unwrap();
    assert_eq!(
        (done.model.as_str(), done.backend.as_str()),
        ("qwen-test", "cpu")
    );
    let outcome = client.run(cleanup("marker-zebra-quartz")).wait();
    assert_eq!(done_text(outcome), "MARKER-ZEBRA-QUARTZ");
    let status = client.status();
    assert_eq!(status.model.as_deref(), Some("qwen-test"));
    assert_eq!(status.backend.as_deref(), Some("cpu"));
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    let job = text
        .lines()
        .find(|l| l.contains(" JOB "))
        .expect("a JOB line");
    for needle in [
        "INFO llm JOB",
        "kind=cleanup",
        "model=qwen-test",
        "backend=cpu",
        "reused_tokens=6",
        "new_tokens=4",
        "code=OK",
    ] {
        assert!(job.contains(needle), "{needle} missing: {job}");
    }
    assert!(
        !text.to_lowercase().contains("zebra"),
        "text leaked: {text}"
    );
    assert!(!text.contains("fixed system prompt"), "{text}");
}

#[test]
fn a_command_job_reaches_the_worker() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    let outcome = client
        .run(LlmWork::Command {
            instruction: "make it formal".into(),
            selection: "hey".into(),
            max_tokens: 32,
        })
        .wait();
    assert_eq!(done_text(outcome), "make it formal");
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains("kind=command"), "{text}");
}

#[test]
fn a_failed_load_is_reported_and_keeps_the_worker_up() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started("answer", dir.path(), fast());
    let failure = client.load_model(spec("missing-one")).wait().unwrap_err();
    assert_eq!(failure.code, error::LLM_LOAD_FAILED);
    assert!(client.status().pid.is_some());
    assert!(client.status().model.is_none());
    client.shutdown();
}

#[test]
fn a_killed_worker_fails_the_job_with_llm_crashed_and_comes_back_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    let first = client.status().pid.unwrap();
    let job = client.run(cleanup("hang forever"));
    std::thread::sleep(Duration::from_millis(150));
    signal(first, "KILL");
    match job.wait() {
        LlmOutcome::Failed(failure) => assert_eq!(failure.code, error::LLM_CRASHED),
        other => panic!("expected LLM_CRASHED, got {other:?}"),
    }
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready
            && s.pid.is_some_and(|pid| pid != first)
            && s.model.is_some())
    );
    assert_eq!(done_text(client.run(cleanup("again")).wait()), "AGAIN");
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains(" CRASH "), "{text}");
    assert!(text.contains("code=LLM_CRASHED"), "{text}");
}

#[test]
fn a_stopped_worker_is_killed_by_the_watchdog_and_the_job_fails_within_the_pong_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    let first = client.status().pid.unwrap();
    signal(first, "STOP");
    let started_at = Instant::now();
    let job = client.run(cleanup("hello"));
    match job.wait() {
        LlmOutcome::Failed(failure) => assert_eq!(failure.code, error::LLM_CRASHED),
        other => panic!("expected LLM_CRASHED, got {other:?}"),
    }
    let waited = started_at.elapsed();
    assert!(waited < Duration::from_secs(3), "waited {waited:?}");
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready
            && s.pid.is_some_and(|pid| pid != first))
    );
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains(" WATCHDOG "), "{text}");
    assert!(text.contains("reason=no pong"), "{text}");
}

#[test]
fn three_kills_in_a_row_follow_the_backoff_steps() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    for _ in 0..3 {
        let pid = client.status().pid.unwrap();
        signal(pid, "KILL");
        assert!(
            client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready
                && s.pid.is_some_and(|new| new != pid))
        );
    }
    client.shutdown();
    assert_eq!(restart_delays(&log), [100, 300, 600]);
}

#[test]
fn a_cancel_the_worker_ignores_kills_it_within_the_grace_and_restarts_it_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let timing = Timing {
        pong_timeout: Duration::from_secs(30),
        ..fast()
    };
    let (client, log) = started("answer", dir.path(), timing);
    loaded(&client, "qwen-test");
    let first = client.status().pid.unwrap();
    let job = client.run(cleanup("hang stubborn"));
    std::thread::sleep(Duration::from_millis(100));
    let started_at = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), LlmOutcome::Cancelled);
    let settled = started_at.elapsed();
    assert!(
        settled >= Duration::from_millis(150) && settled < Duration::from_millis(600),
        "settled in {settled:?}"
    );
    assert!(
        client.wait_for(Duration::from_secs(2), |s| s.state == LlmState::Ready
            && s.pid.is_some_and(|pid| pid != first))
    );
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains(" CANCEL_KILL "), "{text}");
}

#[test]
fn a_cancel_the_worker_honors_ends_the_job_without_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    let first = client.status().pid.unwrap();
    let job = client.run(cleanup("hang"));
    std::thread::sleep(Duration::from_millis(100));
    job.cancel();
    assert_eq!(job.wait(), LlmOutcome::Cancelled);
    assert_eq!(client.status().pid, Some(first));
    client.shutdown();
}

#[test]
fn an_unload_empties_the_model_and_the_next_job_loads_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    client.unload();
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.model.is_none()
            && s.unloaded == Some(UnloadReason::Requested))
    );
    assert_eq!(done_text(client.run(cleanup("back")).wait()), "BACK");
    let status = client.status();
    assert_eq!(status.model.as_deref(), Some("qwen-test"));
    assert_eq!(status.unloaded, None);
    client.shutdown();
    let text = std::fs::read_to_string(log).unwrap();
    assert!(text.contains(" UNLOADED reason=requested"), "{text}");
}

#[test]
fn a_restart_after_an_unload_does_not_load_the_model_until_a_job_needs_it() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    client.unload();
    assert!(client.wait_for(Duration::from_secs(5), |s| s.model.is_none()));
    let first = client.status().pid.unwrap();
    signal(first, "KILL");
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready
            && s.pid.is_some_and(|pid| pid != first))
    );
    assert!(client.status().model.is_none(), "must stay unloaded");
    assert_eq!(done_text(client.run(cleanup("wake")).wait()), "WAKE");
    assert_eq!(client.status().model.as_deref(), Some("qwen-test"));
    client.shutdown();
}

#[test]
fn a_job_for_a_worker_that_stays_down_fails_with_llm_unavailable_after_the_wait() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let client = LlmClient::start(
        ChildSpec {
            program: dir.path().join("no-such-program"),
            args: vec![],
        },
        fast(),
        log,
    );
    let started_at = Instant::now();
    match client.run(cleanup("x")).wait() {
        LlmOutcome::Failed(failure) => assert_eq!(failure.code, error::LLM_UNAVAILABLE),
        other => panic!("expected a failure, got {other:?}"),
    }
    assert!(started_at.elapsed() >= Duration::from_millis(350));
    client.shutdown();
}

#[test]
fn a_job_that_waits_for_the_worker_can_be_cancelled_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let timing = Timing {
        submit_wait: Duration::from_secs(30),
        ..fast()
    };
    let client = LlmClient::start(
        ChildSpec {
            program: dir.path().join("no-such-program"),
            args: vec![],
        },
        timing,
        log,
    );
    let job = client.run(cleanup("x"));
    let started_at = Instant::now();
    job.cancel();
    assert_eq!(job.wait(), LlmOutcome::Cancelled);
    assert!(started_at.elapsed() < Duration::from_millis(500));
    client.shutdown();
}

#[test]
fn a_worker_that_never_says_ready_is_killed_and_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let (log, _) = log_in(dir.path());
    let timing = Timing {
        ready_timeout: Duration::from_millis(300),
        ..fast()
    };
    let client = LlmClient::start(sh("cat >/dev/null"), timing, log);
    assert!(client.wait_for(Duration::from_secs(5), |s| s.pid.is_some()));
    let first = client.status().pid;
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 1
        && s.pid.is_some()
        && s.pid != first));
    client.shutdown();
}

#[test]
fn a_deaf_worker_is_replaced_by_the_watchdog() {
    let dir = tempfile::tempdir().unwrap();
    let (client, log) = started("deaf", dir.path(), fast());
    let first = client.status().pid.unwrap();
    assert!(client.wait_for(Duration::from_secs(5), |s| s.restarts >= 1));
    assert!(
        client.wait_for(Duration::from_secs(5), |s| s.state == LlmState::Ready
            && s.pid != Some(first))
    );
    client.shutdown();
    assert!(std::fs::read_to_string(log).unwrap().contains(" WATCHDOG "));
}

#[test]
fn shutdown_stops_the_worker_and_fails_waiting_jobs() {
    let dir = tempfile::tempdir().unwrap();
    let (client, _) = started("answer", dir.path(), fast());
    loaded(&client, "qwen-test");
    let job = client.run(cleanup("hang"));
    std::thread::sleep(Duration::from_millis(100));
    let started_at = Instant::now();
    client.shutdown();
    assert!(started_at.elapsed() < Duration::from_secs(2));
    let status = client.status();
    assert_eq!((status.state, status.pid), (LlmState::Stopped, None));
    match job.wait() {
        LlmOutcome::Failed(failure) => assert_eq!(failure.code, error::LLM_UNAVAILABLE),
        other => panic!("expected a failure, got {other:?}"),
    }
}
