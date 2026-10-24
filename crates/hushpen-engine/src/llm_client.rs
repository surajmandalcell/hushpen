//! The app side of the `hushpen-llm` child: [`LlmClient`] runs cleanup and Command Mode jobs on
//! the worker. Every job ends in an outcome, whatever the child does, so the caller can always
//! fall back to the rule text. The supervision itself is shared with the voice worker (see
//! `worker`).

use crate::worker::{
    JobHandle, LoadHandle, Loaded, Outcome, Protocol, Signal, WorkerClient, WorkerState,
    WorkerStatus,
};
use hushpen_core::error;
use hushpen_core::llm_protocol::{
    DEFAULT_IDLE_SECS, JobStats, LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest,
};
use hushpen_core::protocol::read_frame;
use serde_json::Value;
use std::path::PathBuf;
use std::process::ChildStdout;

/// A model to keep ready. After a restart the supervisor loads the last good one again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmSpec {
    pub path: PathBuf,
    /// `None` picks the prompt family from the file name.
    pub family: Option<String>,
    pub gpu: bool,
    pub threads: usize,
    /// Seconds without a job before the worker drops the model. `0` keeps it.
    pub idle_secs: u64,
}

impl LlmSpec {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            family: None,
            gpu: false,
            threads: 4,
            idle_secs: DEFAULT_IDLE_SECS,
        }
    }
}

/// One piece of work for the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmWork {
    Cleanup {
        system: String,
        user: String,
        max_tokens: u32,
    },
    Command {
        instruction: String,
        selection: String,
        max_tokens: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmDone {
    pub text: String,
    pub stats: JobStats,
}

/// The messages of `hushpen-llm`.
pub struct LlmWorker;

pub type LlmClient = WorkerClient<LlmWorker>;
pub type LlmState = WorkerState;
pub type LlmStatus = WorkerStatus;
pub type LlmLoaded = Loaded;
/// `LLM_CRASHED` means the child died while the job ran; `LLM_UNAVAILABLE` means it was not up
/// in time. Either way the caller uses the rule text.
pub type LlmOutcome = Outcome<LlmDone>;
pub type LlmJobHandle = JobHandle<LlmWorker>;
pub type LlmLoadHandle = LoadHandle<LlmWorker>;

impl Protocol for LlmWorker {
    const NAME: &'static str = "llm";
    const PROTOCOL_VERSION: u32 = LLM_PROTOCOL_VERSION;
    const UNAVAILABLE: &'static str = error::LLM_UNAVAILABLE;
    const CRASHED: &'static str = error::LLM_CRASHED;
    const LOAD_FAILED: &'static str = error::LLM_LOAD_FAILED;

    type Spec = LlmSpec;
    type Work = LlmWork;
    type Done = LlmDone;
    /// A cleanup job sends nothing before its result.
    type Progress = ();
    type Event = LlmEvent;

    fn hello() -> Value {
        LlmRequest::Hello {
            protocol: LLM_PROTOCOL_VERSION,
        }
        .to_json()
    }

    fn ping() -> Value {
        LlmRequest::Ping.to_json()
    }

    fn unload() -> Value {
        LlmRequest::Unload.to_json()
    }

    fn shutdown() -> Value {
        LlmRequest::Shutdown.to_json()
    }

    fn cancel(job: u64) -> Value {
        LlmRequest::Cancel { job }.to_json()
    }

    fn load(spec: &LlmSpec) -> Value {
        LlmRequest::LoadModel {
            path: spec.path.to_string_lossy().into_owned(),
            family: spec.family.clone(),
            gpu: spec.gpu,
            threads: u32::try_from(spec.threads).unwrap_or(u32::MAX),
            idle_secs: spec.idle_secs,
        }
        .to_json()
    }

    fn job(job: u64, work: &LlmWork) -> Value {
        match work {
            LlmWork::Cleanup {
                system,
                user,
                max_tokens,
            } => LlmRequest::Cleanup {
                job,
                system: system.clone(),
                user: user.clone(),
                max_tokens: *max_tokens,
            },
            LlmWork::Command {
                instruction,
                selection,
                max_tokens,
            } => LlmRequest::Command {
                job,
                instruction: instruction.clone(),
                selection: selection.clone(),
                max_tokens: *max_tokens,
            },
        }
        .to_json()
    }

    fn model_name(spec: &LlmSpec) -> String {
        crate::server::model_name(&spec.path)
    }

    fn kind(work: &LlmWork) -> &'static str {
        match work {
            LlmWork::Cleanup { .. } => "cleanup",
            LlmWork::Command { .. } => "command",
        }
    }

    fn read(stdout: &mut ChildStdout) -> Result<Option<LlmEvent>, String> {
        match read_frame(stdout) {
            Ok(Some(frame)) => LlmEvent::from_frame(&frame)
                .map(Some)
                .map_err(|problem| format!("unreadable message: {problem}")),
            Ok(None) => Ok(None),
            Err(problem) => Err(problem.to_string()),
        }
    }

    fn signal(event: LlmEvent) -> Signal<Self> {
        match event {
            LlmEvent::Ready {
                version,
                protocol,
                backend,
            } => Signal::Ready {
                protocol,
                version,
                backend,
            },
            LlmEvent::Pong => Signal::Pong,
            LlmEvent::Loading { .. } => Signal::Other,
            LlmEvent::Loaded {
                model,
                ms,
                gpu,
                backend,
            } => Signal::Loaded(Loaded {
                model,
                ms,
                gpu,
                backend,
            }),
            LlmEvent::Unloaded { reason } => Signal::Unloaded(reason),
            LlmEvent::Result { job, text, stats } => Signal::Done {
                job,
                counts: format!(
                    "prompt_tokens={} reused_tokens={} new_tokens={} gen_tokens={} load_ms={} ms={}",
                    stats.prompt_tokens,
                    stats.reused_tokens,
                    stats.new_tokens,
                    stats.gen_tokens,
                    stats.load_ms,
                    stats.ms
                ),
                done: LlmDone { text, stats },
            },
            LlmEvent::Cancelled { job } => Signal::Cancelled { job },
            LlmEvent::Error { job, code, params } => Signal::Error { job, code, params },
        }
    }
}
