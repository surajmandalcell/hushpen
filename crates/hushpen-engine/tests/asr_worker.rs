//! The `hushpen-asr` binary: the speech engine child under its own name. No model is needed.
#![cfg(unix)]

use hushpen_engine::{ChildSpec, EngineClient, EngineLog, EngineState, Timing};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const ASR: &str = env!("CARGO_BIN_EXE_hushpen-asr");

#[test]
fn the_worker_prints_its_name_and_the_build_version() {
    let output = Command::new(ASR).arg("--version").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("hushpen-asr {}", hushpen_core::BUILD_VERSION)
    );
}

#[test]
fn the_supervisor_starts_the_worker_under_its_own_process_name() {
    let dir = tempfile::tempdir().unwrap();
    let spec = ChildSpec {
        program: PathBuf::from(ASR),
        args: Vec::new(),
    };
    let log = EngineLog::open(&dir.path().join("engine.log")).unwrap();
    let client = EngineClient::start(spec, Timing::default(), log);
    assert!(
        client.wait_for(Duration::from_secs(20), |status| {
            status.state == EngineState::Ready && status.pid.is_some()
        }),
        "the engine did not become ready"
    );
    let pid = client.status().pid.unwrap();
    let output = Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let name = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        Path::new(name.trim()).file_name().and_then(|n| n.to_str()),
        Some("hushpen-asr"),
        "ps shows {name:?}"
    );
    client.shutdown();
}
