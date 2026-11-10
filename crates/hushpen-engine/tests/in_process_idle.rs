use hushpen_core::idle::Idle;
use hushpen_core::llm_protocol::LLM_PROTOCOL_VERSION;
use hushpen_core::protocol::{PROTOCOL_VERSION, read_frame, write_json};
use hushpen_engine::{
    EngineClient, EngineLog, EngineState, InProcess, Link, LlmClient, LlmSpec, LlmState, LoadSpec,
    Timing,
};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

fn worker(protocol: u32, answer: bool) -> (Link, Receiver<String>) {
    let (sent, received) = mpsc::channel();
    let server = InProcess::new(move |mut input, mut output| {
        let _hello = read_frame(&mut input).unwrap().unwrap();
        write_json(&mut output, &serde_json::json!({
            "type": "ready", "protocol": protocol, "version": "test", "gpu": false, "backend": "cpu"
        })).unwrap();
        while let Some(frame) = read_frame(&mut input).unwrap() {
            let value: serde_json::Value = serde_json::from_slice(&frame.payload).unwrap();
            let kind = value["type"].as_str().unwrap();
            if kind == "shutdown" {
                return;
            }
            let _ = sent.send(kind.to_owned());
            if !answer {
                continue;
            }
            let reply = match kind {
                "ping" => serde_json::json!({"type": "pong"}),
                "load_model" => serde_json::json!({
                    "type": "loaded", "model": "test", "ms": 0, "gpu": false, "backend": "cpu"
                }),
                _ => continue,
            };
            write_json(&mut output, &reply).unwrap();
        }
    });
    (Link::InProcess(server), received)
}

fn timing() -> Timing {
    Timing {
        ping_interval: Duration::from_millis(10),
        pong_timeout: Duration::from_millis(40),
        ..Timing::default()
    }
}

#[test]
fn idle_speech_has_no_pings_and_loads_after_the_watchdog_interval() {
    let (link, requests) = worker(PROTOCOL_VERSION, true);
    let engine = EngineClient::start_link(link, timing(), EngineLog::discard());
    assert!(engine.wait_for(Duration::from_secs(1), |state| state.state
        == EngineState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = engine.load_model(LoadSpec {
        path: "/test/model.bin".into(),
        gpu: false,
        threads: 1,
        idle: Idle::KEEP,
    });
    assert!(load.wait_timeout(Duration::from_secs(1)).unwrap().is_ok());
    assert_eq!(engine.status().restarts, 0);
    engine.shutdown();
}

#[test]
fn idle_cleanup_has_no_pings_and_loads_after_the_watchdog_interval() {
    let (link, requests) = worker(LLM_PROTOCOL_VERSION, true);
    let cleanup = LlmClient::start_link(link, timing(), EngineLog::discard());
    assert!(cleanup.wait_for(Duration::from_secs(1), |state| state.state
        == LlmState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = cleanup.load_model(LlmSpec::new("/test/model.gguf".into()));
    assert!(load.wait_timeout(Duration::from_secs(1)).unwrap().is_ok());
    assert_eq!(cleanup.status().restarts, 0);
    cleanup.shutdown();
}

#[test]
fn active_speech_still_has_a_watchdog_after_an_idle_stretch() {
    let (link, requests) = worker(PROTOCOL_VERSION, false);
    let engine = EngineClient::start_link(link, timing(), EngineLog::discard());
    assert!(engine.wait_for(Duration::from_secs(1), |state| state.state
        == EngineState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = engine.load_model(LoadSpec {
        path: "/test/model.bin".into(),
        gpu: false,
        threads: 1,
        idle: Idle::KEEP,
    });
    assert!(load.wait_timeout(Duration::from_secs(1)).unwrap().is_err());
    engine.shutdown();
}

#[test]
fn active_cleanup_still_has_a_watchdog_after_an_idle_stretch() {
    let (link, requests) = worker(LLM_PROTOCOL_VERSION, false);
    let cleanup = LlmClient::start_link(link, timing(), EngineLog::discard());
    assert!(cleanup.wait_for(Duration::from_secs(1), |state| state.state
        == LlmState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(100)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = cleanup.load_model(LlmSpec::new("/test/model.gguf".into()));
    assert!(load.wait_timeout(Duration::from_secs(1)).unwrap().is_err());
    cleanup.shutdown();
}
