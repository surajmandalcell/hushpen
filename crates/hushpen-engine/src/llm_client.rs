//! The app side of the `hushpen-llm` child: [`LlmClient`] starts the worker, keeps it alive,
//! and hands out cleanup jobs. One supervisor thread owns the process (see `llm_manager`); this
//! module is the thread-safe handle. Every job ends in an outcome, whatever the child does, so
//! the caller can always fall back to the rule text.

use crate::client::{ChildSpec, Failure, Timing};
use crate::engine_log::EngineLog;
use crate::llm_manager;
use hushpen_core::llm_protocol::{DEFAULT_IDLE_SECS, JobStats, UnloadReason};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

impl LlmWork {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Cleanup { .. } => "cleanup",
            Self::Command { .. } => "command",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmState {
    /// The child is starting and has not said `Ready`.
    Starting,
    /// The child is up and no model load is running.
    Ready,
    /// A model load is running.
    Loading,
    /// The child is down and a restart is waiting for its delay.
    Backoff,
    Stopped,
}

impl LlmState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Backoff => "backoff",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmStatus {
    pub pid: Option<u32>,
    pub state: LlmState,
    /// The model the running child holds in memory, for example `Qwen3.5-2B-Q4_K_M`.
    pub model: Option<String>,
    /// `metal` or `cpu`, as the child reported it when it loaded the model.
    pub backend: Option<String>,
    pub gpu: bool,
    /// Why the model left memory, until the next load.
    pub unloaded: Option<UnloadReason>,
    /// How many times the child went down and was scheduled to restart.
    pub restarts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmDone {
    pub text: String,
    pub stats: JobStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmOutcome {
    Done(LlmDone),
    Cancelled,
    /// `LLM_CRASHED` means the child died while the job ran; `LLM_UNAVAILABLE` means it was
    /// not up in time. Either way the caller uses the rule text.
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmLoaded {
    pub model: String,
    pub ms: u64,
    pub gpu: bool,
    pub backend: String,
}

pub(crate) enum LlmCmd {
    Load {
        spec: LlmSpec,
        reply: Sender<Result<LlmLoaded, Failure>>,
    },
    Run {
        job: u64,
        work: LlmWork,
        reply: Sender<LlmOutcome>,
    },
    Cancel {
        job: u64,
    },
    Unload,
    Shutdown,
}

pub(crate) struct LlmShared {
    status: Mutex<LlmStatus>,
    changed: Condvar,
}

impl LlmShared {
    fn new() -> Self {
        Self {
            status: Mutex::new(LlmStatus {
                pid: None,
                state: LlmState::Starting,
                model: None,
                backend: None,
                gpu: false,
                unloaded: None,
                restarts: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, LlmStatus> {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut LlmStatus)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }
}

struct Inner {
    commands: Sender<llm_manager::Msg>,
    shared: Arc<LlmShared>,
    next_job: AtomicU64,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.commands.send(llm_manager::Msg::Cmd(LlmCmd::Shutdown));
    }
}

/// The handle to the LLM child. Clones share one child.
#[derive(Clone)]
pub struct LlmClient {
    inner: Arc<Inner>,
}

impl LlmClient {
    /// Starts the supervisor thread, which starts the child at once. The child holds no model
    /// until [`LlmClient::load_model`].
    pub fn start(spec: ChildSpec, timing: Timing, log: Arc<EngineLog>) -> Self {
        let shared = Arc::new(LlmShared::new());
        let (sender, receiver) = mpsc::channel();
        let thread = llm_manager::spawn(llm_manager::Setup {
            spec,
            timing,
            log,
            shared: Arc::clone(&shared),
            sender: sender.clone(),
            receiver,
        });
        Self {
            inner: Arc::new(Inner {
                commands: sender,
                shared,
                next_job: AtomicU64::new(1),
                thread: Mutex::new(Some(thread)),
            }),
        }
    }

    pub fn status(&self) -> LlmStatus {
        self.inner.shared.lock().clone()
    }

    /// Blocks until `ready` is true for the status or `timeout` passes. Returns whether it held.
    pub fn wait_for(&self, timeout: Duration, ready: impl Fn(&LlmStatus) -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        let mut status = self.inner.shared.lock();
        loop {
            if ready(&status) {
                return true;
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            status = self
                .inner
                .shared
                .changed
                .wait_timeout(status, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
    }

    /// Asks the child to load a model and remember it. A failed load keeps the model that was
    /// loaded before.
    pub fn load_model(&self, spec: LlmSpec) -> LlmLoadHandle {
        let (reply, answer) = mpsc::channel();
        if self
            .inner
            .commands
            .send(llm_manager::Msg::Cmd(LlmCmd::Load { spec, reply }))
            .is_err()
        {
            return LlmLoadHandle::failed("the supervisor has stopped");
        }
        LlmLoadHandle { answer }
    }

    /// Starts a job. The caller owns the timeout: wait with [`LlmJobHandle::wait_timeout`], then
    /// [`LlmJobHandle::cancel`].
    pub fn run(&self, work: LlmWork) -> LlmJobHandle {
        let job = self.inner.next_job.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let sent = self
            .inner
            .commands
            .send(llm_manager::Msg::Cmd(LlmCmd::Run { job, work, reply }))
            .is_ok();
        LlmJobHandle {
            job,
            client: self.clone(),
            answer,
            stopped: !sent,
        }
    }

    pub fn cancel(&self, job: u64) {
        let _ = self
            .inner
            .commands
            .send(llm_manager::Msg::Cmd(LlmCmd::Cancel { job }));
    }

    /// Drops the model now. The next job loads it again.
    pub fn unload(&self) {
        let _ = self
            .inner
            .commands
            .send(llm_manager::Msg::Cmd(LlmCmd::Unload));
    }

    /// Stops the child (a `Shutdown` frame, then a kill after 1 s) and the supervisor thread.
    pub fn shutdown(&self) {
        let _ = self
            .inner
            .commands
            .send(llm_manager::Msg::Cmd(LlmCmd::Shutdown));
        let thread = self
            .inner
            .thread
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

/// The answer to one `load_model`.
pub struct LlmLoadHandle {
    answer: Receiver<Result<LlmLoaded, Failure>>,
}

impl LlmLoadHandle {
    fn failed(detail: &str) -> Self {
        let (reply, answer) = mpsc::channel();
        let _ = reply.send(Err(Failure::new(
            hushpen_core::error::LLM_UNAVAILABLE,
            detail,
        )));
        Self { answer }
    }

    pub fn wait(self) -> Result<LlmLoaded, Failure> {
        self.answer.recv().unwrap_or_else(|_| {
            Err(Failure::new(
                hushpen_core::error::LLM_UNAVAILABLE,
                "the supervisor has stopped",
            ))
        })
    }
}

/// One running or finished job.
pub struct LlmJobHandle {
    job: u64,
    client: LlmClient,
    answer: Receiver<LlmOutcome>,
    stopped: bool,
}

impl LlmJobHandle {
    pub fn id(&self) -> u64 {
        self.job
    }

    /// Asks the worker to stop the job. [`LlmJobHandle::wait`] then returns
    /// [`LlmOutcome::Cancelled`], in under a second in every case.
    pub fn cancel(&self) {
        self.client.cancel(self.job);
    }

    pub fn wait(self) -> LlmOutcome {
        if self.stopped {
            return Self::stopped_outcome();
        }
        self.answer
            .recv()
            .unwrap_or_else(|_| Self::stopped_outcome())
    }

    /// The outcome if it is already known. Taking it uses it up.
    pub fn try_outcome(&self) -> Option<LlmOutcome> {
        self.answer.try_recv().ok()
    }

    /// Waits up to `timeout` for the outcome. Taking it uses it up.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<LlmOutcome> {
        if self.stopped {
            return Some(Self::stopped_outcome());
        }
        self.answer.recv_timeout(timeout).ok()
    }

    fn stopped_outcome() -> LlmOutcome {
        LlmOutcome::Failed(Failure::new(
            hushpen_core::error::LLM_UNAVAILABLE,
            "the supervisor has stopped",
        ))
    }
}
