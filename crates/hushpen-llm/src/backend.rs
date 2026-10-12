//! What the worker needs from a language model, apart from llama.cpp.
//!
//! A llama.cpp context borrows its model, so a loaded model cannot be handed around as a plain
//! value. [`Backend::with_session`] loads the model, calls the body while the model is alive, and
//! drops the model when the body returns. The server loop runs inside that body.

use hushpen_core::llm_prompt::PromptFamily;
use hushpen_core::llm_protocol::JobStats;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A flag that a running job reads between tokens.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A model to hold. The server keeps it after an idle unload and loads it again for the next job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSpec {
    pub path: PathBuf,
    pub family: PromptFamily,
    pub gpu: bool,
    pub threads: usize,
    /// `None` keeps the model until the app unloads it.
    pub idle: Option<Duration>,
}

impl LoadSpec {
    /// The name shown in logs: the file name without its extension.
    pub fn model_name(&self) -> String {
        model_name(&self.path)
    }
}

/// `Qwen3.5-2B-Q4_K_M.gguf` is the model `Qwen3.5-2B-Q4_K_M`.
pub fn model_name(path: &std::path::Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadInfo {
    pub ms: u64,
    /// True when the model runs on the GPU.
    pub gpu: bool,
}

/// One cleanup or command job, with the prompt parts already split.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobSpec {
    /// The fixed part. The session keeps the model state of its prompt for the next job.
    pub system: String,
    pub user: String,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutput {
    pub text: String,
    /// Token counts. The server fills `ms` and `load_ms`.
    pub stats: JobStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    Cancelled,
    Failed { code: &'static str, detail: String },
}

impl JobError {
    pub fn failed(code: &'static str, detail: impl Into<String>) -> Self {
        Self::Failed {
            code,
            detail: detail.into(),
        }
    }
}

/// A loaded model that can run jobs one at a time.
pub trait Session {
    fn run(&mut self, job: &JobSpec, cancel: &CancelFlag) -> Result<JobOutput, JobError>;
}

/// Why the body of a session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEnd {
    /// The model is dropped. The worker keeps waiting for work.
    Unload(hushpen_core::llm_protocol::UnloadReason),
    /// Another model replaces this one.
    Reload(LoadSpec),
    /// The worker is shutting down.
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub detail: String,
}

pub type SessionBody<'a> = dyn FnMut(&mut dyn Session, LoadInfo) -> SessionEnd + 'a;

pub trait Backend: Send + Sync {
    /// `metal` or `cpu`: where a model runs when the app asks for the GPU.
    fn backend_name(&self) -> &'static str;

    /// Loads the model, calls `body` with it, then drops the model.
    fn with_session(
        &self,
        spec: &LoadSpec,
        body: &mut SessionBody<'_>,
    ) -> Result<SessionEnd, LoadError>;
}
