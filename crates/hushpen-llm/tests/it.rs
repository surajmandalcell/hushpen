//! Heavy integration tests: the real `hushpen-llm` process with a real model.
//!
//! Models: `HUSHPEN_TEST_LLM` (a GGUF file, for the default model), or the file names below in
//! `$HUSHPEN_TEST_FIXTURES/llm/`, `$HUSHPEN_TEST_ASSETS/models/llm/`, or `HUSHPEN_TEST_LLM_DIR`.
//! `HUSHPEN_TEST_GPU=1` turns on the GPU (Metal, macOS only) and the Gemma 4 test.
//! `HUSHPEN_TEST_THREADS` sets the thread count (default 2).
#![cfg(feature = "heavy")]

use hushpen_core::llm_prompt::CLEANUP_SYSTEM;
use hushpen_core::llm_protocol::{
    JobStats, LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason,
};
use hushpen_core::protocol::{read_frame, write_json};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const QWEN_2B: &str = "Qwen3.5-2B-Q4_K_M.gguf";
const GEMMA_E2B: &str = "gemma-4-E2B-it-Q4_K_M.gguf";
const FILLER: &str = "um so I think we should uh meet on tuesday no wait wednesday at like three \
pm to go over the the budget you know";
const EVENT_WAIT: Duration = Duration::from_secs(120);

fn gpu() -> bool {
    std::env::var("HUSHPEN_TEST_GPU").is_ok_and(|value| value == "1")
}

fn threads() -> u32 {
    std::env::var("HUSHPEN_TEST_THREADS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2)
}

fn model_file(name: &str) -> PathBuf {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for (var, sub) in [
        ("HUSHPEN_TEST_FIXTURES", "llm"),
        ("HUSHPEN_TEST_ASSETS", "models/llm"),
    ] {
        if let Some(root) = std::env::var_os(var) {
            dirs.push(PathBuf::from(root).join(sub));
        }
    }
    if let Some(dir) = std::env::var_os("HUSHPEN_TEST_LLM_DIR") {
        dirs.push(dir.into());
    }
    dirs.iter()
        .map(|dir| dir.join(name))
        .find(|path| path.exists())
        .unwrap_or_else(|| panic!("model {name} not found; set HUSHPEN_TEST_LLM_DIR"))
}

fn default_model() -> PathBuf {
    match std::env::var_os("HUSHPEN_TEST_LLM") {
        Some(path) => path.into(),
        None => model_file(QWEN_2B),
    }
}

/// The worker process with a thread that turns its output into events.
struct Worker {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Receiver<LlmEvent>,
    stderr: Receiver<String>,
}

impl Worker {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_hushpen-llm"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start hushpen-llm");
        let stdin = child.stdin.take();
        let mut stdout = child.stdout.take().expect("stdout");
        let stderr_pipe = child.stderr.take().expect("stderr");
        let (event_tx, events) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(Some(frame)) = read_frame(&mut stdout) {
                let event = LlmEvent::from_frame(&frame).expect("a readable event");
                if event_tx.send(event).is_err() {
                    return;
                }
            }
        });
        let (stderr_tx, stderr) = mpsc::channel();
        thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stderr_pipe)
                .lines()
                .map_while(Result::ok)
            {
                let _ = stderr_tx.send(line);
            }
        });
        let mut worker = Self {
            child,
            stdin,
            events,
            stderr,
        };
        worker.send(&LlmRequest::Hello {
            protocol: LLM_PROTOCOL_VERSION,
        });
        match worker.next() {
            LlmEvent::Ready { backend, .. } => {
                assert_eq!(
                    backend,
                    if cfg!(target_os = "macos") {
                        "metal"
                    } else {
                        "cpu"
                    }
                );
            }
            other => panic!("expected ready, got {other:?}"),
        }
        worker
    }

    fn send(&mut self, request: &LlmRequest) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        write_json(stdin, &request.to_json()).expect("write a request");
    }

    fn next(&mut self) -> LlmEvent {
        self.events
            .recv_timeout(EVENT_WAIT)
            .expect("an event from the worker")
    }

    fn next_within(&mut self, wait: Duration) -> Option<LlmEvent> {
        self.events.recv_timeout(wait).ok()
    }

    fn load(&mut self, model: &std::path::Path, idle_secs: u64) -> (u64, bool) {
        self.send(&LlmRequest::LoadModel {
            path: model.to_string_lossy().into_owned(),
            family: None,
            gpu: gpu(),
            threads: threads(),
            idle_secs,
        });
        assert!(matches!(self.next(), LlmEvent::Loading { .. }));
        match self.next() {
            LlmEvent::Loaded {
                ms, gpu, backend, ..
            } => {
                eprintln!("loaded in {ms} ms, gpu={gpu}, backend={backend}");
                (ms, gpu)
            }
            other => panic!("expected loaded, got {other:?}"),
        }
    }

    fn cleanup(&mut self, job: u64, text: &str, max_tokens: u32) {
        self.send(&LlmRequest::Cleanup {
            job,
            system: CLEANUP_SYSTEM.to_owned(),
            user: text.to_owned(),
            max_tokens,
        });
    }

    fn result(&mut self, job: u64) -> (String, JobStats) {
        match self.next() {
            LlmEvent::Result {
                job: got,
                text,
                stats,
            } => {
                assert_eq!(got, job);
                (text, stats)
            }
            other => panic!("expected a result, got {other:?}"),
        }
    }

    fn close_input(&mut self) {
        self.stdin = None;
    }

    fn wait_exit(&mut self, within: Duration) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + within;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn stderr_text(&self) -> String {
        self.stderr.try_iter().collect::<Vec<_>>().join("\n")
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stdin = None;
        if self.wait_exit(Duration::from_secs(3)).is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// The checks the app applies to a model's output: not empty, not much longer than the input,
/// no refusal, and most of the input's words still there.
fn passes_output_guards(input: &str, output: &str) -> bool {
    let refusals = ["i cannot", "i can't", "as an ai", "sorry"];
    let lower = output.to_lowercase();
    if output.trim().is_empty()
        || output.chars().count() > input.chars().count() * 3 / 2 + 40
        || refusals.iter().any(|prefix| lower.starts_with(prefix))
    {
        return false;
    }
    let input_words = words(input);
    let output_words = words(output);
    let kept = output_words
        .iter()
        .filter(|word| input_words.contains(word))
        .count();
    kept * 2 >= output_words.len()
}

fn assert_cleaned(text: &str) {
    let found = words(text);
    for filler in ["um", "uh", "er"] {
        assert!(
            !found.iter().any(|word| word == filler),
            "'{filler}' in {text:?}"
        );
    }
    assert!(!text.contains("<think>"), "reasoning in {text:?}");
    assert!(!text.contains("</think>"), "reasoning in {text:?}");
    assert!(
        found.contains(&"wednesday".to_owned()) && found.contains(&"budget".to_owned()),
        "the meaning is gone from {text:?}"
    );
    assert!(passes_output_guards(FILLER, text), "guards reject {text:?}");
}

#[test]
fn a_filler_sentence_is_cleaned() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, FILLER, 128);
    let (text, first) = worker.result(1);
    assert_cleaned(&text);
    worker.cleanup(2, FILLER, 128);
    let started = Instant::now();
    let (again, second) = worker.result(2);
    let warm = started.elapsed();
    eprintln!(
        "cold job {} ms, warm job {} ms ({warm:?} seen)",
        first.ms, second.ms
    );
    assert_cleaned(&again);
    let limit = if cfg!(target_os = "macos") && gpu() {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(8)
    };
    assert!(warm < limit, "a warm cleanup took {warm:?}");
}

#[test]
fn gemma_4_cleans_with_its_hand_built_prompt() {
    if !gpu() {
        eprintln!("skipped: set HUSHPEN_TEST_GPU=1 to run the Gemma 4 test");
        return;
    }
    let mut worker = Worker::start();
    let (_, on_gpu) = worker.load(&model_file(GEMMA_E2B), 300);
    assert_eq!(on_gpu, cfg!(target_os = "macos"), "Metal is not in use");
    worker.cleanup(1, FILLER, 128);
    let (text, _) = worker.result(1);
    assert_cleaned(&text);
    worker.cleanup(2, FILLER, 128);
    let started = Instant::now();
    let (again, _) = worker.result(2);
    assert_cleaned(&again);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn the_second_job_reuses_the_system_prompt() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, FILLER, 128);
    let (_, first) = worker.result(1);
    worker.cleanup(2, "uh please send me the the report by friday", 128);
    let (_, second) = worker.result(2);
    eprintln!("first {first:?}\nsecond {second:?}");
    assert_eq!(first.reused_tokens, 0);
    assert_eq!(first.new_tokens, first.prompt_tokens);
    let system_tokens = 40;
    assert!(
        second.reused_tokens >= system_tokens,
        "only {} tokens reused",
        second.reused_tokens
    );
    assert!(second.new_tokens < first.new_tokens);
    assert_eq!(
        second.new_tokens + second.reused_tokens,
        second.prompt_tokens
    );
}

#[test]
fn an_idle_model_unloads_and_the_next_job_loads_it_again() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 2);
    worker.cleanup(1, FILLER, 128);
    let (text, _) = worker.result(1);
    assert_cleaned(&text);
    let unloaded = worker
        .next_within(Duration::from_secs(20))
        .expect("an unload after the idle time");
    assert_eq!(
        unloaded,
        LlmEvent::Unloaded {
            reason: UnloadReason::Idle
        }
    );
    if let Some(kib) = resident_kib(worker.child.id()) {
        eprintln!("resident after unload: {} MB", kib / 1024);
        assert!(kib < 200 * 1024, "{} MB still resident", kib / 1024);
    }
    worker.cleanup(2, FILLER, 128);
    assert!(matches!(worker.next(), LlmEvent::Loading { .. }));
    assert!(matches!(worker.next(), LlmEvent::Loaded { .. }));
    let (again, stats) = worker.result(2);
    assert_cleaned(&again);
    assert!(stats.load_ms > 0, "the reload is not counted");
}

#[test]
fn cancel_settles_a_running_job_in_under_a_second() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, FILLER, 512);
    thread::sleep(Duration::from_millis(150));
    let started = Instant::now();
    worker.send(&LlmRequest::Cancel { job: 1 });
    match worker.next() {
        LlmEvent::Cancelled { job: 1 } | LlmEvent::Result { job: 1, .. } => {}
        other => panic!("expected the job to settle, got {other:?}"),
    }
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "{:?}",
        started.elapsed()
    );
    worker.cleanup(2, FILLER, 128);
    let (text, _) = worker.result(2);
    assert_cleaned(&text);
}

#[test]
fn a_missing_model_file_is_a_load_error_and_the_worker_lives() {
    let mut worker = Worker::start();
    worker.send(&LlmRequest::LoadModel {
        path: "/nonexistent/Qwen3.5-2B-Q4_K_M.gguf".into(),
        family: None,
        gpu: false,
        threads: 2,
        idle_secs: 300,
    });
    assert!(matches!(worker.next(), LlmEvent::Loading { .. }));
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: None, code, .. } if code == hushpen_core::error::LLM_LOAD_FAILED
    ));
    worker.send(&LlmRequest::Ping);
    assert_eq!(worker.next(), LlmEvent::Pong);
}

#[test]
fn the_worker_exits_when_the_app_closes_the_pipe() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, FILLER, 128);
    worker.close_input();
    let started = Instant::now();
    let status = worker
        .wait_exit(Duration::from_secs(2))
        .expect("the worker is still running 2 s after its input closed");
    eprintln!("exited with {status} after {:?}", started.elapsed());
}

#[test]
fn the_worker_holds_pipes_and_files_but_no_sockets() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, FILLER, 128);
    worker.result(1);
    let Ok(entries) = std::fs::read_dir(format!("/proc/{}/fd", worker.child.id())) else {
        eprintln!("skipped: no /proc on this system");
        return;
    };
    for entry in entries.flatten() {
        let target = std::fs::read_link(entry.path()).unwrap_or_default();
        let target = target.to_string_lossy();
        assert!(!target.starts_with("socket:"), "an open socket: {target}");
    }
}

#[test]
fn stderr_never_holds_the_dictation_text() {
    let mut worker = Worker::start();
    worker.load(&default_model(), 300);
    worker.cleanup(1, "zyxwvut quokka marker", 128);
    worker.result(1);
    worker.close_input();
    worker.wait_exit(Duration::from_secs(3));
    assert!(!worker.stderr_text().contains("quokka"));
}

fn resident_kib(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}
