//! The supervisor thread of a worker child. It follows the engine supervisor (`manager`): it
//! owns the process and every deadline, all writes to the child's stdin happen here, and every
//! job reaches an outcome whatever the child does.

mod commands;
mod deadlines;
mod events;
mod process;

use crate::backoff::Backoff;
use crate::client::{ChildSpec, Failure, Timing};
use crate::engine_log::EngineLog;
use crate::worker::{Cmd, Loaded, Protocol, Reply, Shared, WorkerState};
use hushpen_core::llm_protocol::UnloadReason;
use std::collections::{HashMap, VecDeque};
use std::process::{Child, ChildStdin};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub(crate) enum Msg<P: Protocol> {
    Cmd(Cmd<P>),
    Child {
        generation: u64,
        event: ChildEvent<P::Event>,
    },
}

pub(crate) enum ChildEvent<E> {
    Event(E),
    /// The child's output ended or held something that is not a frame.
    Closed(String),
}

pub(crate) struct Setup<P: Protocol> {
    pub spec: ChildSpec,
    pub timing: Timing,
    pub log: Arc<EngineLog>,
    pub shared: Arc<Shared>,
    pub sender: Sender<Msg<P>>,
    pub receiver: Receiver<Msg<P>>,
}

pub(crate) fn spawn<P: Protocol>(setup: Setup<P>) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("{}-supervisor", P::NAME))
        .spawn(move || Manager::new(setup).run())
        .unwrap_or_else(|error| {
            panic!("could not start the {} supervisor thread: {error}", P::NAME)
        })
}

/// How the next start is timed after the child went down.
#[derive(Clone, Copy)]
enum Restart {
    /// After the backoff delay: the child crashed or hung.
    Backoff,
    /// At once: a cancel killed a child that would not settle.
    Now,
}

struct Running {
    child: Child,
    stdin: ChildStdin,
    spawned: Instant,
    ready: bool,
    /// This child has been told which model to use.
    configured: bool,
    last_pong: Instant,
    next_ping: Instant,
}

struct LoadEntry<S> {
    spec: S,
    /// `None` for the load after a restart, which nobody waits for.
    reply: Option<Sender<Result<Loaded, Failure>>>,
}

struct Pending<P: Protocol> {
    job: u64,
    work: P::Work,
    reply: Sender<Reply<P>>,
    deadline: Instant,
}

struct Inflight<P: Protocol> {
    reply: Sender<Reply<P>>,
    sent: Instant,
    cancel_deadline: Option<Instant>,
    kind: &'static str,
    model: String,
}

struct Manager<P: Protocol> {
    spec: ChildSpec,
    timing: Timing,
    log: Arc<EngineLog>,
    shared: Arc<Shared>,
    sender: Sender<Msg<P>>,
    receiver: Receiver<Msg<P>>,
    backoff: Backoff,
    /// Counts child generations so a late message of a dead child is ignored.
    generation: u64,
    running: Option<Running>,
    next_start: Option<Instant>,
    /// A write failed. The reason is acted on after the message that caused it.
    broken: Option<String>,
    /// The last model the worker confirmed. A restart gives it to the new child.
    current: Option<P::Spec>,
    /// The model was in memory when the last child went down, so the new child loads it at once.
    warm: bool,
    sent_loads: VecDeque<LoadEntry<P::Spec>>,
    unsent_loads: VecDeque<LoadEntry<P::Spec>>,
    pending: VecDeque<Pending<P>>,
    inflight: HashMap<u64, Inflight<P>>,
    model: Option<String>,
    backend: Option<String>,
    gpu: bool,
    unloaded: Option<UnloadReason>,
    restarts: u32,
    stopped: bool,
}

impl<P: Protocol> Manager<P> {
    fn new(setup: Setup<P>) -> Self {
        let backoff = Backoff::new(setup.timing.backoff.clone(), setup.timing.healthy_after);
        Self {
            spec: setup.spec,
            timing: setup.timing,
            log: setup.log,
            shared: setup.shared,
            sender: setup.sender,
            receiver: setup.receiver,
            backoff,
            generation: 0,
            running: None,
            next_start: None,
            broken: None,
            current: None,
            warm: false,
            sent_loads: VecDeque::new(),
            unsent_loads: VecDeque::new(),
            pending: VecDeque::new(),
            inflight: HashMap::new(),
            model: None,
            backend: None,
            gpu: false,
            unloaded: None,
            restarts: 0,
            stopped: false,
        }
    }

    fn run(mut self) {
        self.start_child();
        loop {
            let wait = self.next_wakeup().saturating_duration_since(Instant::now());
            match self.receiver.recv_timeout(wait) {
                Ok(Msg::Cmd(Cmd::Shutdown)) | Err(RecvTimeoutError::Disconnected) => {
                    self.stop();
                    return;
                }
                Ok(message) => self.handle(message),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.tick();
        }
    }

    fn handle(&mut self, message: Msg<P>) {
        match message {
            Msg::Cmd(command) => self.command(command),
            Msg::Child { generation, event } if generation == self.generation => match event {
                ChildEvent::Event(event) => self.event(P::signal(event)),
                ChildEvent::Closed(reason) => self.down(&reason, Restart::Backoff),
            },
            Msg::Child { .. } => {}
        }
    }

    fn publish(&self) {
        let state = if self.stopped {
            WorkerState::Stopped
        } else {
            match &self.running {
                None => WorkerState::Backoff,
                Some(running) if !running.ready => WorkerState::Starting,
                Some(_) if !self.sent_loads.is_empty() => WorkerState::Loading,
                Some(_) => WorkerState::Ready,
            }
        };
        let pid = self.running.as_ref().map(|running| running.child.id());
        self.shared.update(|status| {
            status.pid = pid;
            status.state = state;
            status.model = self.model.clone();
            status.backend = self.backend.clone();
            status.gpu = self.gpu;
            status.unloaded = self.unloaded;
            status.restarts = self.restarts;
        });
    }
}
