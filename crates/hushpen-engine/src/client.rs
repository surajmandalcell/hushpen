//! The app side of the engine child: [`EngineClient`] starts the child, keeps it alive, and
//! hands out jobs. All process work happens on one supervisor thread (see `manager`); this
//! module is the thread-safe handle the rest of the app holds.

use crate::engine_log::EngineLog;
use crate::manager;
use hushpen_core::protocol::Segment;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How to start the child. The app uses its own executable with the argument `engine`.
#[derive(Debug, Clone)]
pub struct ChildSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

/// Every timing rule of the supervisor, so tests can shorten them.
#[derive(Debug, Clone)]
pub struct Timing {
    /// Delay before the first, second, third restart after a crash; the last repeats.
    pub backoff: Vec<Duration>,
    /// A child that lived this long starts the backoff again.
    pub healthy_after: Duration,
    pub ping_interval: Duration,
    /// No `Pong` for this long: kill the child.
    pub pong_timeout: Duration,
    /// A cancel that is not settled after this long kills the child.
    pub cancel_grace: Duration,
    /// A child that does not answer `Hello` in this time is killed.
    pub ready_timeout: Duration,
    /// How long a job waits for a restarting child before it fails with `ENGINE_UNAVAILABLE`.
    pub submit_wait: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            backoff: vec![
                Duration::from_secs(1),
                Duration::from_secs(5),
                Duration::from_secs(30),
            ],
            healthy_after: Duration::from_secs(60),
            ping_interval: Duration::from_secs(2),
            pong_timeout: Duration::from_secs(5),
            cancel_grace: Duration::from_millis(800),
            ready_timeout: Duration::from_secs(10),
            submit_wait: Duration::from_secs(10),
        }
    }
}

/// A model to keep loaded. After a restart the supervisor loads the last good one again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSpec {
    pub path: PathBuf,
    pub gpu: bool,
    pub threads: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscribeSpec {
    pub wav_path: PathBuf,
    /// `None` or `auto` detects the language.
    pub language: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// The child is starting and has not said `Ready`.
    Starting,
    /// The child is up and no load is running. A model may or may not be loaded.
    Ready,
    /// A model load is running (the warm-up).
    Loading,
    /// The child is down and a restart is waiting for its delay.
    Backoff,
    Stopped,
}

impl EngineState {
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
pub struct Status {
    pub pid: Option<u32>,
    pub state: EngineState,
    /// The model the running child holds, for example `tiny.en`.
    pub model: Option<String>,
    /// True when that model runs on the GPU.
    pub gpu: bool,
    /// How many times the child went down and was scheduled to restart.
    pub restarts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: String,
    pub detail: String,
}

impl Failure {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcription {
    pub text: String,
    pub language: String,
    pub segments: Vec<Segment>,
    pub audio_ms: u64,
    pub decode_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    Done(Transcription),
    Cancelled,
    /// The job failed. `ENGINE_CRASHED` means the child died while it ran; the audio file is
    /// untouched.
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub model: String,
    pub ms: u64,
    pub gpu: bool,
}

pub(crate) enum Cmd {
    Load {
        spec: LoadSpec,
        reply: Sender<Result<Loaded, Failure>>,
    },
    Transcribe {
        job: u64,
        spec: TranscribeSpec,
        reply: Sender<JobOutcome>,
    },
    Cancel {
        job: u64,
    },
    Shutdown,
}

pub(crate) struct Shared {
    status: Mutex<Status>,
    changed: Condvar,
}

impl Shared {
    fn new() -> Self {
        Self {
            status: Mutex::new(Status {
                pid: None,
                state: EngineState::Starting,
                model: None,
                gpu: false,
                restarts: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Status> {
        self.status
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn update(&self, change: impl FnOnce(&mut Status)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }
}

struct Inner {
    commands: Sender<manager::Msg>,
    shared: Arc<Shared>,
    next_job: AtomicU64,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        let _ = self.commands.send(manager::Msg::Cmd(Cmd::Shutdown));
    }
}

/// The handle to the engine child. Clones share one child.
#[derive(Clone)]
pub struct EngineClient {
    inner: Arc<Inner>,
}

impl EngineClient {
    /// Starts the supervisor thread, which starts the child at once.
    pub fn start(spec: ChildSpec, timing: Timing, log: Arc<EngineLog>) -> Self {
        let shared = Arc::new(Shared::new());
        let (sender, receiver) = mpsc::channel();
        let thread = manager::spawn(manager::Setup {
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

    pub fn status(&self) -> Status {
        self.inner.shared.lock().clone()
    }

    /// Blocks until `ready` is true for the status or `timeout` passes. Returns whether it held.
    pub fn wait_for(&self, timeout: Duration, ready: impl Fn(&Status) -> bool) -> bool {
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

    /// Asks the child to load a model. A failed load keeps the model that was loaded before.
    pub fn load_model(&self, spec: LoadSpec) -> LoadHandle {
        let (reply, answer) = mpsc::channel();
        if self
            .inner
            .commands
            .send(manager::Msg::Cmd(Cmd::Load { spec, reply }))
            .is_err()
        {
            return LoadHandle::failed("the supervisor has stopped");
        }
        LoadHandle { answer }
    }

    /// Starts a job. The audio file is only read, never changed or deleted.
    pub fn transcribe(&self, spec: TranscribeSpec) -> JobHandle {
        let job = self.inner.next_job.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = mpsc::channel();
        let sent = self
            .inner
            .commands
            .send(manager::Msg::Cmd(Cmd::Transcribe { job, spec, reply }))
            .is_ok();
        JobHandle {
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
            .send(manager::Msg::Cmd(Cmd::Cancel { job }));
    }

    /// Stops the child (a `Shutdown` frame, then a kill after 1 s) and the supervisor thread.
    pub fn shutdown(&self) {
        let _ = self.inner.commands.send(manager::Msg::Cmd(Cmd::Shutdown));
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
pub struct LoadHandle {
    answer: Receiver<Result<Loaded, Failure>>,
}

impl LoadHandle {
    fn failed(detail: &str) -> Self {
        let (reply, answer) = mpsc::channel();
        let _ = reply.send(Err(Failure::new(
            hushpen_core::error::ENGINE_UNAVAILABLE,
            detail,
        )));
        Self { answer }
    }

    pub fn wait(self) -> Result<Loaded, Failure> {
        self.answer.recv().unwrap_or_else(|_| {
            Err(Failure::new(
                hushpen_core::error::ENGINE_UNAVAILABLE,
                "the supervisor has stopped",
            ))
        })
    }
}

/// One running or finished job.
pub struct JobHandle {
    job: u64,
    client: EngineClient,
    answer: Receiver<JobOutcome>,
    stopped: bool,
}

impl JobHandle {
    pub fn id(&self) -> u64 {
        self.job
    }

    /// Asks the engine to stop the job. [`JobHandle::wait`] then returns
    /// [`JobOutcome::Cancelled`], in under a second in every case.
    pub fn cancel(&self) {
        self.client.cancel(self.job);
    }

    pub fn wait(self) -> JobOutcome {
        if self.stopped {
            return Self::stopped_outcome();
        }
        self.answer
            .recv()
            .unwrap_or_else(|_| Self::stopped_outcome())
    }

    /// The outcome if it is already known. Taking it uses it up.
    pub fn try_outcome(&self) -> Option<JobOutcome> {
        self.answer.try_recv().ok()
    }

    /// Waits up to `timeout` for the outcome. Taking it uses it up.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<JobOutcome> {
        self.answer.recv_timeout(timeout).ok()
    }

    fn stopped_outcome() -> JobOutcome {
        JobOutcome::Failed(Failure::new(
            hushpen_core::error::ENGINE_UNAVAILABLE,
            "the supervisor has stopped",
        ))
    }
}
