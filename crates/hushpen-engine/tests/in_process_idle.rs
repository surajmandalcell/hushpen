use hushpen_core::idle::Idle;
use hushpen_core::llm_protocol::LLM_PROTOCOL_VERSION;
use hushpen_core::protocol::{PROTOCOL_VERSION, read_frame, write_json};
use hushpen_engine::{
    EngineClient, EngineLog, EngineState, InProcess, Link, LlmClient, LlmSpec, LlmState, LoadSpec,
    RemoteSpec, Timing,
};
use hushpen_remote::{Identity, RemoteStream, Resolver, WorkerKind, read_worker};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

fn worker(kind: WorkerKind, answer: bool, remote: bool) -> (Link, Receiver<String>) {
    let protocol = match kind {
        WorkerKind::Speech => PROTOCOL_VERSION,
        WorkerKind::Cleanup => LLM_PROTOCOL_VERSION,
    };
    let (sent, received) = mpsc::channel();
    let serve = move |mut input: Box<dyn Read + Send>, mut output: Box<dyn Write + Send>| {
        let _hello = read_frame(&mut input).unwrap().unwrap();
        write_json(&mut output, &serde_json::json!({
            "type": "ready", "protocol": protocol, "version": "test", "gpu": false, "backend": "cpu"
        })).unwrap();
        while let Ok(Some(frame)) = read_frame(&mut input) {
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
    };
    if !remote {
        return (Link::InProcess(InProcess::new(serve)), received);
    }
    let server = Identity::generate().unwrap();
    let client = Identity::generate().unwrap();
    let server_key = server.public();
    let client_key = client.public();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let timeout = Duration::from_secs(1);
    std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        let mut session =
            RemoteStream::accept(socket, &server, &[client_key], None, timeout).unwrap();
        assert_eq!(read_worker(&mut session.stream).unwrap(), kind);
        session.stream.allow_idle_wait().unwrap();
        let (input, output) = session.stream.split().unwrap();
        serve(Box::new(input), Box::new(output));
    });
    let stream = RemoteStream::connect(
        address,
        &client,
        server_key,
        None,
        timeout,
        &Resolver::default(),
    )
    .unwrap()
    .stream;
    (Link::Remote(RemoteSpec::new(stream, kind)), received)
}

fn timing() -> Timing {
    Timing {
        ping_interval: Duration::from_millis(50),
        pong_timeout: Duration::from_millis(500),
        ..Timing::default()
    }
}

fn idle_speech(remote: bool) {
    let (link, requests) = worker(WorkerKind::Speech, true, remote);
    let engine = EngineClient::start_link(link, timing(), EngineLog::discard());
    assert!(engine.wait_for(Duration::from_secs(2), |state| state.state
        == EngineState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(700)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = engine.load_model(LoadSpec {
        path: "/test/model.bin".into(),
        gpu: false,
        threads: 1,
        idle: Idle::KEEP,
    });
    assert!(load.wait_timeout(Duration::from_secs(2)).unwrap().is_ok());
    assert_eq!(engine.status().restarts, 0);
    engine.shutdown();
}

fn idle_cleanup(remote: bool) {
    let (link, requests) = worker(WorkerKind::Cleanup, true, remote);
    let cleanup = LlmClient::start_link(link, timing(), EngineLog::discard());
    assert!(cleanup.wait_for(Duration::from_secs(2), |state| state.state
        == LlmState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(700)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = cleanup.load_model(LlmSpec::new("/test/model.gguf".into()));
    assert!(load.wait_timeout(Duration::from_secs(2)).unwrap().is_ok());
    assert_eq!(cleanup.status().restarts, 0);
    cleanup.shutdown();
}

fn active_speech(remote: bool) {
    let (link, requests) = worker(WorkerKind::Speech, false, remote);
    let engine = EngineClient::start_link(link, timing(), EngineLog::discard());
    assert!(engine.wait_for(Duration::from_secs(2), |state| state.state
        == EngineState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(700)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = engine.load_model(LoadSpec {
        path: "/test/model.bin".into(),
        gpu: false,
        threads: 1,
        idle: Idle::KEEP,
    });
    assert!(load.wait_timeout(Duration::from_secs(2)).unwrap().is_err());
    engine.shutdown();
}

fn active_cleanup(remote: bool) {
    let (link, requests) = worker(WorkerKind::Cleanup, false, remote);
    let cleanup = LlmClient::start_link(link, timing(), EngineLog::discard());
    assert!(cleanup.wait_for(Duration::from_secs(2), |state| state.state
        == LlmState::Ready));
    assert!(matches!(
        requests.recv_timeout(Duration::from_millis(700)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let load = cleanup.load_model(LlmSpec::new("/test/model.gguf".into()));
    assert!(load.wait_timeout(Duration::from_secs(2)).unwrap().is_err());
    cleanup.shutdown();
}

#[test]
fn idle_speech_has_no_pings_and_loads_after_the_watchdog_interval() {
    idle_speech(false);
}

#[test]
fn idle_cleanup_has_no_pings_and_loads_after_the_watchdog_interval() {
    idle_cleanup(false);
}

#[test]
fn active_speech_still_has_a_watchdog_after_an_idle_stretch() {
    active_speech(false);
}

#[test]
fn active_cleanup_still_has_a_watchdog_after_an_idle_stretch() {
    active_cleanup(false);
}

#[test]
fn idle_remote_speech_has_no_pings_and_loads_after_the_watchdog_interval() {
    idle_speech(true);
}

#[test]
fn idle_remote_cleanup_has_no_pings_and_loads_after_the_watchdog_interval() {
    idle_cleanup(true);
}

#[test]
fn active_remote_speech_still_has_a_watchdog_after_an_idle_stretch() {
    active_speech(true);
}

#[test]
fn active_remote_cleanup_still_has_a_watchdog_after_an_idle_stretch() {
    active_cleanup(true);
}
