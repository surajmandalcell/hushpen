//! A supervised worker child on the frame protocol: the app side of `hushpen-llm` and
//! `hushpen-tts`. [`WorkerClient`] starts the worker, keeps it alive, and hands out jobs. One
//! supervisor thread owns the process (see `worker_manager`); this module is the thread-safe
//! handle. Every job ends in an outcome, whatever the child does.
//!
//! [`Protocol`] holds what differs between the workers: their messages, codes, and job types.
//! Only this crate implements it.

use crate::client::{ChildSpec, Failure, Timing};
use crate::engine_log::EngineLog;
use crate::worker_manager;
use hushpen_core::llm_protocol::UnloadReason;
use hushpen_core::protocol::Params;
use serde_json::Value;
use std::process::ChildStdout;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The messages and codes of one kind of worker.
pub trait Protocol: Send + 'static {
    /// The thread name and the start of panic messages, such as `llm`.
    const NAME: &'static str;
    const PROTOCOL_VERSION: u32;
    /// The child was not up in time, or the supervisor has stopped.
    const UNAVAILABLE: &'static str;
    /// The child died while the job ran.
    const CRASHED: &'static str;
    /// The worker's code for a model that did not load.
    const LOAD_FAILED: &'static str;

    /// A model to keep ready. After a restart the supervisor loads the last good one again.
    type Spec: Clone + Send + 'static;
    type Work: Send + 'static;
    type Done: Send + 'static;
    /// Output a job sends before it ends, such as audio.
    type Progress: Send + 'static;
    type Event: Send + 'static;

    fn hello() -> Value;
    fn ping() -> Value;
    fn unload() -> Value;
    fn shutdown() -> Value;
    fn cancel(job: u64) -> Value;
    fn load(spec: &Self::Spec) -> Value;
    fn job(job: u64, work: &Self::Work) -> Value;
    /// The model name for log lines.
    fn model_name(spec: &Self::Spec) -> String;
    /// The job kind for log lines, such as `cleanup`.
    fn kind(work: &Self::Work) -> &'static str;
    /// Reads the next event. `Ok(None)` means the output ended.
    fn read(stdout: &mut ChildStdout) -> Result<Option<Self::Event>, String>;
    fn signal(event: Self::Event) -> Signal<Self>;
}

/// What an event means to the supervisor.
pub enum Signal<P: Protocol + ?Sized> {
    Ready {
        protocol: u32,
        version: String,
        /// Where models run, such as `metal` or `cpu`.
        backend: String,
    },
    Pong,
    Loaded(Loaded),
    Unloaded(UnloadReason),
    Progress {
        job: u64,
        progress: P::Progress,
    },
    Done {
        job: u64,
        done: P::Done,
        /// Counts and times for the job's log line, never text.
        counts: String,
    },
    Cancelled {
        job: u64,
    },
    Error {
        job: Option<u64>,
        code: String,
        params: Params,
    },
    /// An event the supervisor does not act on, such as `Loading`.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
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

impl WorkerState {
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
pub struct WorkerStatus {
    pub pid: Option<u32>,
    pub state: WorkerState,
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
pub struct Loaded {
    pub model: String,
    pub ms: u64,
    pub gpu: bool,
    pub backend: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<D> {
    Done(D),
    Cancelled,
    /// The worker's crash code means the child died while the job ran, its unavailable code
    /// that it was not up in time.
    Failed(Failure),
}

/// What a job sends to its handle: progress, then one outcome.
pub enum Reply<P: Protocol> {
    Progress(P::Progress),
    End(Outcome<P::Done>),
}

pub(crate) enum Cmd<P: Protocol> {
    Load {
        spec: P::Spec,
        reply: Sender<Result<Loaded, Failure>>,
    },
    Run {
        job: u64,
        work: P::Work,
        reply: Sender<Reply<P>>,
    },
    Cancel {
        job: u64,
    },
    Unload,
    Shutdown,
}

pub(crate) struct Shared {
    status: Mutex<WorkerStatus>,
    changed: Condvar,
}

impl Shared {
    fn new() -> Self {
        Self {
            status: Mutex::new(WorkerStatus {
                pid: None,
                state: WorkerState::Starting,
                model: None,
                backend: None,
                gpu: false,
                unloaded: None,
                restarts: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, WorkerStatus> {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut WorkerStatus)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }
}

struct Inner<P: Protocol> {
    commands: Sender<worker_manager::Msg<P>>,
    shared: Arc<Shared>,
    next_job: AtomicU64,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl<P: Protocol> Drop for Inner<P> {
    fn drop(&mut self) {
        let _ = self.commands.send(worker_manager::Msg::Cmd(Cmd::Shutdown));
    }
}

/// The handle to a worker child. Clones share one child.
pub struct WorkerClient<P: Protocol> {
    inner: Arc<Inner<P>>,
}

impl<P: Protocol> Clone for WorkerClient<P> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<P: Protocol> WorkerClient<P> {
    /// Starts the supervisor thread, which starts the child at once. The child holds no model
    /// until [`WorkerClient::load_model`].
    pub fn start(spec: ChildSpec, timing: Timing, log: Arc<EngineLog>) -> Self {
        let shared = Arc::new(Shared::new());
        let (sender, receiver) = mpsc::channel();
        let thread = worker_manager::spawn(worker_manager::Setup {
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

    pub fn status(&self) -> WorkerStatus {
        self.inner.shared.lock().clone()
    }

    /// Blocks until `ready` is true for the status or `timeout` passes. Returns whether it held.
    pub fn wait_for(&self, timeout: Duration, ready: impl Fn(&WorkerStatus) -> bool) -> bool {
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

    fn send(&self, command: Cmd<P>) -> bool {
        self.inner
            .commands
            .send(worker_manager::Msg::Cmd(command))
            .is_ok()
    }

    /// Asks the child to load a model and remember it. A failed load keeps the model that was
    /// loaded before.
    pub fn load_model(&self, spec: P::Spec) -> LoadHandle<P> {
        let (reply, answer) = mpsc::channel();
        if !self.send(Cmd::Load { spec, reply }) {
            let (reply, answer) = mpsc::channel();
            let _ = reply.send(Err(stopped::<P>()));
            return LoadHandle::new(answer);
        }
        LoadHandle::new(answer)
    }

    /// Starts a job. The caller owns the timeout: wait with [`JobHandle::wait_timeout`], then
    /// [`JobHandle::cancel`].
    pub fn run(&self, work: P::Work) -> JobHandle<P> {
        let job = self.inner.next_job.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let sent = self.send(Cmd::Run { job, work, reply });
        JobHandle {
            job,
            client: self.clone(),
            answer,
            stopped: !sent,
        }
    }

    pub fn cancel(&self, job: u64) {
        self.send(Cmd::Cancel { job });
    }

    /// Drops the model now. The next job loads it again.
    pub fn unload(&self) {
        self.send(Cmd::Unload);
    }

    /// Stops the child (a `Shutdown` frame, then a kill after 1 s) and the supervisor thread.
    pub fn shutdown(&self) {
        self.send(Cmd::Shutdown);
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

fn stopped<P: Protocol>() -> Failure {
    Failure::new(P::UNAVAILABLE, "the supervisor has stopped")
}

/// The answer to one `load_model`.
pub struct LoadHandle<P: Protocol> {
    answer: Receiver<Result<Loaded, Failure>>,
    protocol: std::marker::PhantomData<P>,
}

impl<P: Protocol> LoadHandle<P> {
    fn new(answer: Receiver<Result<Loaded, Failure>>) -> Self {
        Self {
            answer,
            protocol: std::marker::PhantomData,
        }
    }

    pub fn wait(self) -> Result<Loaded, Failure> {
        self.answer.recv().unwrap_or_else(|_| Err(stopped::<P>()))
    }
}

/// One running or finished job.
pub struct JobHandle<P: Protocol> {
    job: u64,
    client: WorkerClient<P>,
    answer: Receiver<Reply<P>>,
    stopped: bool,
}

impl<P: Protocol> JobHandle<P> {
    pub fn id(&self) -> u64 {
        self.job
    }

    /// Asks the worker to stop the job. The outcome is then [`Outcome::Cancelled`], in under a
    /// second in every case.
    pub fn cancel(&self) {
        self.client.cancel(self.job);
    }

    /// Waits for the outcome. Progress on the way is dropped.
    pub fn wait(self) -> Outcome<P::Done> {
        if self.stopped {
            return Outcome::Failed(stopped::<P>());
        }
        loop {
            match self.answer.recv() {
                Ok(Reply::End(outcome)) => return outcome,
                Ok(Reply::Progress(_)) => {}
                Err(_) => return Outcome::Failed(stopped::<P>()),
            }
        }
    }

    /// The outcome if it is already known. Taking it uses it up. Progress on the way is dropped.
    pub fn try_outcome(&self) -> Option<Outcome<P::Done>> {
        loop {
            match self.answer.try_recv().ok()? {
                Reply::End(outcome) => return Some(outcome),
                Reply::Progress(_) => {}
            }
        }
    }

    /// Waits up to `timeout` for the outcome. Taking it uses it up. Progress on the way is
    /// dropped.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Outcome<P::Done>> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.next(deadline.saturating_duration_since(Instant::now()))? {
                Reply::End(outcome) => return Some(outcome),
                Reply::Progress(_) => {}
            }
        }
    }

    /// The next progress or the outcome, within `timeout`. After the outcome it returns `None`.
    pub fn next(&self, timeout: Duration) -> Option<Reply<P>> {
        if self.stopped {
            return Some(Reply::End(Outcome::Failed(stopped::<P>())));
        }
        self.answer.recv_timeout(timeout).ok()
    }
}
