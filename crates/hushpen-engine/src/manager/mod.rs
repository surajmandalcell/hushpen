//! The supervisor thread. It owns the child process and every deadline: restart delays, the
//! ping watchdog, and the cancel grace. All writes to the child's stdin happen here, so frames
//! never interleave, and every request reaches a reply or a failure, whatever the child does.

mod commands;
mod deadlines;
mod events;
mod process;

use crate::backoff::Backoff;
use crate::client::{
    ChildSpec, Cmd, EngineState, Failure, JobOutcome, LoadSpec, Loaded, ProgressSink, Shared,
    Timing, Work,
};
use crate::engine_log::EngineLog;
use hushpen_core::protocol::Event;
use std::collections::{HashMap, VecDeque};
use std::process::{Child, ChildStdin};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub(crate) enum Msg {
    Cmd(Cmd),
    Child { generation: u64, event: ChildEvent },
}

pub(crate) enum ChildEvent {
    Event(Event),
    /// The child's output ended or held something that is not a frame.
    Closed(String),
}

pub(crate) struct Setup {
    pub spec: ChildSpec,
    pub timing: Timing,
    pub log: Arc<EngineLog>,
    pub shared: Arc<Shared>,
    pub sender: Sender<Msg>,
    pub receiver: Receiver<Msg>,
}

pub(crate) fn spawn(setup: Setup) -> JoinHandle<()> {
    thread::Builder::new()
        .name("engine-supervisor".into())
        .spawn(move || Manager::new(setup).run())
        .unwrap_or_else(|error| panic!("could not start the supervisor thread: {error}"))
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
    last_pong: Instant,
    next_ping: Instant,
}

struct LoadEntry {
    spec: LoadSpec,
    /// `None` for the reload after a restart, which nobody waits for.
    reply: Option<Sender<Result<Loaded, Failure>>>,
}

struct Pending {
    job: u64,
    work: Work,
    progress: Option<ProgressSink>,
    reply: Sender<JobOutcome>,
    deadline: Instant,
}

struct Inflight {
    reply: Sender<JobOutcome>,
    progress: Option<ProgressSink>,
    sent: Instant,
    cancel_deadline: Option<Instant>,
    model: String,
    threads: usize,
}

struct Manager {
    spec: ChildSpec,
    timing: Timing,
    log: Arc<EngineLog>,
    shared: Arc<Shared>,
    sender: Sender<Msg>,
    receiver: Receiver<Msg>,
    backoff: Backoff,
    /// Counts child generations so a late message of a dead child is ignored.
    generation: u64,
    running: Option<Running>,
    next_start: Option<Instant>,
    /// A write failed. The reason is acted on after the message that caused it.
    broken: Option<String>,
    /// The last model the engine confirmed. It is loaded again after a restart.
    current: Option<LoadSpec>,
    /// Loads written to the child and not answered yet.
    sent_loads: VecDeque<LoadEntry>,
    /// Loads asked for while the child was down.
    unsent_loads: VecDeque<LoadEntry>,
    pending: VecDeque<Pending>,
    inflight: HashMap<u64, Inflight>,
    model: Option<String>,
    gpu: bool,
    restarts: u32,
    stopped: bool,
}

impl Manager {
    fn new(setup: Setup) -> Self {
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
            sent_loads: VecDeque::new(),
            unsent_loads: VecDeque::new(),
            pending: VecDeque::new(),
            inflight: HashMap::new(),
            model: None,
            gpu: false,
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

    fn handle(&mut self, message: Msg) {
        match message {
            Msg::Cmd(command) => self.command(command),
            Msg::Child { generation, event } if generation == self.generation => match event {
                ChildEvent::Event(event) => self.event(event),
                ChildEvent::Closed(reason) => self.down(&reason, Restart::Backoff),
            },
            Msg::Child { .. } => {}
        }
    }

    fn publish(&self) {
        let state = if self.stopped {
            EngineState::Stopped
        } else {
            match &self.running {
                None => EngineState::Backoff,
                Some(running) if !running.ready => EngineState::Starting,
                Some(_) if !self.sent_loads.is_empty() => EngineState::Loading,
                Some(_) => EngineState::Ready,
            }
        };
        let pid = self.running.as_ref().map(|running| running.child.id());
        self.shared.update(|status| {
            status.pid = pid;
            status.state = state;
            status.model = self.model.clone();
            status.gpu = self.gpu;
            status.restarts = self.restarts;
        });
    }
}
