//! The worker's request loop with a fake model: ordering, cancel, idle unload, lazy reload.

use hushpen_core::error;
use hushpen_core::llm_protocol::{
    JobStats, LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason,
};
use hushpen_core::protocol::{FrameKind, read_frame, write_frame, write_json};
use hushpen_llm::backend::{
    Backend, CancelFlag, JobError, JobOutput, JobSpec, LoadError, LoadInfo, LoadSpec, Session,
    SessionBody, SessionEnd,
};
use hushpen_llm::server::{EXIT_BAD_FRAME, EXIT_BAD_PROTOCOL, ServerConfig, serve};
use std::io::{PipeReader, PipeWriter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// A model that upper-cases the user text. A path with `bad` fails to load, a path with `slow`
/// takes 300 ms to load.
struct Fake {
    job_ms: u64,
    loads: Arc<AtomicUsize>,
}

struct FakeSession {
    job_ms: u64,
    jobs_run: u32,
}

impl Session for FakeSession {
    fn run(&mut self, job: &JobSpec, cancel: &CancelFlag) -> Result<JobOutput, JobError> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(self.job_ms) {
            if cancel.is_cancelled() {
                return Err(JobError::Cancelled);
            }
            thread::sleep(Duration::from_millis(2));
        }
        if job.user == "explode" {
            return Err(JobError::failed(error::LLM_GENERATE_FAILED, "boom"));
        }
        self.jobs_run += 1;
        let prefix = job.system.len() as u32;
        Ok(JobOutput {
            text: job.user.to_uppercase(),
            stats: JobStats {
                prompt_tokens: prefix + job.user.len() as u32,
                reused_tokens: if self.jobs_run > 1 { prefix } else { 0 },
                new_tokens: job.user.len() as u32 + if self.jobs_run > 1 { 0 } else { prefix },
                gen_tokens: job.user.len() as u32,
                ms: 0,
                load_ms: 0,
            },
        })
    }
}

impl Backend for Fake {
    fn backend_name(&self) -> &'static str {
        "cpu"
    }

    fn with_session(
        &self,
        spec: &LoadSpec,
        body: &mut SessionBody<'_>,
    ) -> Result<SessionEnd, LoadError> {
        let name = spec.path.to_string_lossy();
        if name.contains("slow") {
            thread::sleep(Duration::from_millis(300));
        }
        if name.contains("bad") {
            return Err(LoadError {
                detail: "not a model".into(),
            });
        }
        self.loads.fetch_add(1, Ordering::SeqCst);
        let mut session = FakeSession {
            job_ms: self.job_ms,
            jobs_run: 0,
        };
        Ok(body(&mut session, LoadInfo { ms: 7, gpu: false }))
    }
}

struct Worker {
    to_worker: PipeWriter,
    from_worker: PipeReader,
    exit: thread::JoinHandle<u8>,
    loads: Arc<AtomicUsize>,
}

impl Worker {
    fn start(job_ms: u64) -> Self {
        let (worker_in, to_worker) = std::io::pipe().unwrap();
        let (from_worker, worker_out) = std::io::pipe().unwrap();
        let loads = Arc::new(AtomicUsize::new(0));
        let config = ServerConfig {
            version: "test".into(),
            backend: Arc::new(Fake {
                job_ms,
                loads: Arc::clone(&loads),
            }),
        };
        let exit = thread::spawn(move || serve(worker_in, worker_out, config));
        Self {
            to_worker,
            from_worker,
            exit,
            loads,
        }
    }

    fn send(&mut self, request: &LlmRequest) {
        write_json(&mut self.to_worker, &request.to_json()).unwrap();
    }

    fn next(&mut self) -> LlmEvent {
        let frame = read_frame(&mut self.from_worker).unwrap().expect("a frame");
        LlmEvent::from_frame(&frame).unwrap()
    }

    fn load_with(&mut self, name: &str, idle_secs: u64) -> LlmEvent {
        self.send(&LlmRequest::LoadModel {
            path: format!("/models/{name}"),
            family: None,
            gpu: false,
            threads: 2,
            idle_secs,
        });
        assert!(matches!(self.next(), LlmEvent::Loading { .. }));
        self.next()
    }

    fn load(&mut self, name: &str) -> LlmEvent {
        self.load_with(name, 300)
    }

    fn cleanup(&mut self, job: u64, user: &str) {
        self.send(&LlmRequest::Cleanup {
            job,
            system: "System.".into(),
            user: user.into(),
            max_tokens: 64,
        });
    }

    fn finish(mut self) -> u8 {
        self.send(&LlmRequest::Shutdown);
        self.exit.join().unwrap()
    }
}

#[test]
fn hello_gets_ready_with_the_version_protocol_and_backend() {
    let mut worker = Worker::start(0);
    worker.send(&LlmRequest::Hello {
        protocol: LLM_PROTOCOL_VERSION,
    });
    assert_eq!(
        worker.next(),
        LlmEvent::Ready {
            version: "test".into(),
            protocol: LLM_PROTOCOL_VERSION,
            backend: "cpu".into()
        }
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_protocol_mismatch_is_answered_with_a_coded_error_and_an_exit() {
    let mut worker = Worker::start(0);
    worker.send(&LlmRequest::Hello { protocol: 99 });
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { code, .. } if code == error::LLM_PROTOCOL
    ));
    assert_eq!(worker.exit.join().unwrap(), EXIT_BAD_PROTOCOL);
}

#[test]
fn a_bad_frame_ends_the_worker_with_the_bad_frame_code() {
    let mut worker = Worker::start(0);
    write_frame(&mut worker.to_worker, FrameKind::Json, b"not json at all").unwrap();
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { code, .. } if code == error::LLM_PROTOCOL
    ));
    worker.send(&LlmRequest::Ping);
    assert_eq!(worker.next(), LlmEvent::Pong);
    drop(worker.to_worker);
    assert_eq!(worker.exit.join().unwrap(), 0);

    let (worker_in, mut to_worker) = std::io::pipe().unwrap();
    let (_from_worker, worker_out) = std::io::pipe().unwrap();
    let exit = thread::spawn(move || {
        serve(
            worker_in,
            worker_out,
            ServerConfig {
                version: "test".into(),
                backend: Arc::new(Fake {
                    job_ms: 0,
                    loads: Arc::default(),
                }),
            },
        )
    });
    // A length over the limit cannot be read past.
    std::io::Write::write_all(&mut to_worker, &[0xff, 0xff, 0xff, 0xff, 0]).unwrap();
    assert_eq!(exit.join().unwrap(), EXIT_BAD_FRAME);
}

#[test]
fn load_then_cleanup_gives_a_result_with_counts_and_times() {
    let mut worker = Worker::start(0);
    let loaded = worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    assert_eq!(
        loaded,
        LlmEvent::Loaded {
            model: "Qwen3.5-2B-Q4_K_M".into(),
            ms: 7,
            gpu: false,
            backend: "cpu".into()
        }
    );
    worker.cleanup(1, "um hello");
    match worker.next() {
        LlmEvent::Result { job, text, stats } => {
            assert_eq!((job, text.as_str()), (1, "UM HELLO"));
            assert_eq!(stats.gen_tokens, 8);
            assert_eq!(stats.load_ms, 0, "the model was warm");
        }
        other => panic!("expected a result, got {other:?}"),
    }
    assert_eq!(worker.finish(), 0);
}

#[test]
fn command_mode_builds_its_own_prompt() {
    let mut worker = Worker::start(0);
    worker.load("gemma-4-E2B-it-Q4_K_M.gguf");
    worker.send(&LlmRequest::Command {
        job: 2,
        instruction: "shorter".into(),
        selection: "a b c".into(),
        max_tokens: 32,
    });
    match worker.next() {
        LlmEvent::Result { job: 2, text, .. } => {
            assert_eq!(text, "INSTRUCTION: SHORTER\n\nTEXT:\nA B C");
        }
        other => panic!("expected a result, got {other:?}"),
    }
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_job_before_any_model_gets_a_coded_error() {
    let mut worker = Worker::start(0);
    worker.cleanup(5, "hello");
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: Some(5), code, .. } if code == error::LLM_NO_MODEL
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_family_the_name_does_not_tell_is_refused_at_the_load() {
    let mut worker = Worker::start(0);
    worker.send(&LlmRequest::LoadModel {
        path: "/models/llama-3.gguf".into(),
        family: None,
        gpu: false,
        threads: 2,
        idle_secs: 300,
    });
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: None, code, .. } if code == error::LLM_LOAD_FAILED
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_failed_load_reports_the_load_code_and_fails_the_waiting_job() {
    let mut worker = Worker::start(0);
    worker.send(&LlmRequest::LoadModel {
        path: "/models/bad-Qwen.gguf".into(),
        family: None,
        gpu: false,
        threads: 2,
        idle_secs: 300,
    });
    assert!(matches!(worker.next(), LlmEvent::Loading { .. }));
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: None, code, .. } if code == error::LLM_LOAD_FAILED
    ));
    worker.cleanup(3, "hi");
    assert!(matches!(worker.next(), LlmEvent::Loading { .. }));
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: None, code, .. } if code == error::LLM_LOAD_FAILED
    ));
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: Some(3), code, .. } if code == error::LLM_LOAD_FAILED
    ));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_failing_job_gets_its_own_code_and_the_next_job_still_runs() {
    let mut worker = Worker::start(0);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "explode");
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: Some(1), code, .. } if code == error::LLM_GENERATE_FAILED
    ));
    worker.cleanup(2, "fine");
    assert!(matches!(worker.next(), LlmEvent::Result { job: 2, .. }));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn ping_is_answered_while_a_job_runs() {
    let mut worker = Worker::start(400);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "hello");
    let started = Instant::now();
    worker.send(&LlmRequest::Ping);
    assert_eq!(worker.next(), LlmEvent::Pong);
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "the pong waited for the job"
    );
    assert!(matches!(worker.next(), LlmEvent::Result { job: 1, .. }));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn cancel_stops_a_running_job_and_settles_a_queued_one_at_once() {
    let mut worker = Worker::start(3000);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "running");
    worker.cleanup(2, "queued");
    thread::sleep(Duration::from_millis(100));
    let started = Instant::now();
    worker.send(&LlmRequest::Cancel { job: 2 });
    assert_eq!(worker.next(), LlmEvent::Cancelled { job: 2 });
    worker.send(&LlmRequest::Cancel { job: 1 });
    assert_eq!(worker.next(), LlmEvent::Cancelled { job: 1 });
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "cancel took {:?}",
        started.elapsed()
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_reused_job_id_is_refused() {
    let mut worker = Worker::start(300);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "first");
    worker.cleanup(1, "again");
    assert!(matches!(
        worker.next(),
        LlmEvent::Error { job: Some(1), code, .. } if code == error::LLM_PROTOCOL
    ));
    assert!(matches!(worker.next(), LlmEvent::Result { job: 1, .. }));
    assert_eq!(worker.finish(), 0);
}

#[test]
fn the_second_job_reuses_the_prompt_prefix() {
    let mut worker = Worker::start(0);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "first text");
    worker.cleanup(2, "second text");
    let (LlmEvent::Result { stats: first, .. }, LlmEvent::Result { stats: second, .. }) =
        (worker.next(), worker.next())
    else {
        panic!("two results");
    };
    assert_eq!(first.reused_tokens, 0);
    assert!(second.reused_tokens > 0);
    assert!(second.new_tokens < first.new_tokens);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn an_idle_model_is_dropped_and_the_next_job_loads_it_again_by_itself() {
    let mut worker = Worker::start(0);
    worker.load_with("Qwen3.5-2B-Q4_K_M.gguf", 1);
    worker.cleanup(1, "before");
    assert!(matches!(worker.next(), LlmEvent::Result { job: 1, .. }));
    assert_eq!(
        worker.next(),
        LlmEvent::Unloaded {
            reason: UnloadReason::Idle
        }
    );
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);

    worker.cleanup(2, "after");
    assert!(matches!(worker.next(), LlmEvent::Loading { .. }));
    assert!(matches!(worker.next(), LlmEvent::Loaded { .. }));
    match worker.next() {
        LlmEvent::Result {
            job: 2,
            text,
            stats,
        } => {
            assert_eq!(text, "AFTER");
            assert_eq!(stats.load_ms, 7, "the reload counts in the job");
            assert!(stats.ms >= 7);
        }
        other => panic!("expected a result, got {other:?}"),
    }
    assert_eq!(worker.loads.load(Ordering::SeqCst), 2);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn an_idle_time_of_zero_keeps_the_model_for_good() {
    let mut worker = Worker::start(0);
    worker.load_with("Qwen3.5-2B-Q4_K_M.gguf", 0);
    thread::sleep(Duration::from_millis(1300));
    worker.send(&LlmRequest::Ping);
    assert_eq!(worker.next(), LlmEvent::Pong);
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn unload_drops_the_model_at_once() {
    let mut worker = Worker::start(0);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.send(&LlmRequest::Unload);
    assert_eq!(
        worker.next(),
        LlmEvent::Unloaded {
            reason: UnloadReason::Requested
        }
    );
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_second_load_of_the_same_model_answers_without_loading_again() {
    let mut worker = Worker::start(0);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.send(&LlmRequest::LoadModel {
        path: "/models/Qwen3.5-2B-Q4_K_M.gguf".into(),
        family: None,
        gpu: false,
        threads: 2,
        idle_secs: 300,
    });
    assert!(matches!(worker.next(), LlmEvent::Loaded { ms: 0, .. }));
    assert_eq!(worker.loads.load(Ordering::SeqCst), 1);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn a_different_model_replaces_the_loaded_one() {
    let mut worker = Worker::start(0);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    let second = worker.load("gemma-4-E2B-it-Q4_K_M.gguf");
    assert!(matches!(
        &second,
        LlmEvent::Loaded { model, .. } if model == "gemma-4-E2B-it-Q4_K_M"
    ));
    assert_eq!(worker.loads.load(Ordering::SeqCst), 2);
    assert_eq!(worker.finish(), 0);
}

#[test]
fn the_end_of_the_input_ends_the_worker_even_during_a_job() {
    let mut worker = Worker::start(5000);
    worker.load("Qwen3.5-2B-Q4_K_M.gguf");
    worker.cleanup(1, "long");
    thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    drop(worker.to_worker);
    assert_eq!(worker.exit.join().unwrap(), 0);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the worker lingered {:?}",
        started.elapsed()
    );
}
