//! The supervisor thread of the LLM child. It follows the engine supervisor (`manager`): it
//! owns the process and every deadline, all writes to the child's stdin happen here, and every
//! job reaches an outcome whatever the child does. The caller turns any failure into the rule
//! text.

use crate::backoff::Backoff;
use crate::client::{ChildSpec, Failure, Timing};
use crate::engine_log::{EngineLog, Level};
use crate::llm_client::{
    LlmCmd, LlmDone, LlmLoaded, LlmOutcome, LlmShared, LlmSpec, LlmState, LlmStatus, LlmWork,
};
use hushpen_core::error;
use hushpen_core::llm_protocol::{LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason};
use hushpen_core::protocol::{read_frame, write_json};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(crate) enum Msg {
    Cmd(LlmCmd),
    Child { generation: u64, event: ChildEvent },
}

pub(crate) enum ChildEvent {
    Event(LlmEvent),
    /// The child's output ended or held something that is not a frame.
    Closed(String),
}

pub(crate) struct Setup {
    pub spec: ChildSpec,
    pub timing: Timing,
    pub log: Arc<EngineLog>,
    pub shared: Arc<LlmShared>,
    pub sender: Sender<Msg>,
    pub receiver: Receiver<Msg>,
}

pub(crate) fn spawn(setup: Setup) -> JoinHandle<()> {
    thread::Builder::new()
        .name("llm-supervisor".into())
        .spawn(move || Manager::new(setup).run())
        .unwrap_or_else(|error| panic!("could not start the LLM supervisor thread: {error}"))
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

struct LoadEntry {
    spec: LlmSpec,
    /// `None` for the load after a restart, which nobody waits for.
    reply: Option<Sender<Result<LlmLoaded, Failure>>>,
}

struct Pending {
    job: u64,
    work: LlmWork,
    reply: Sender<LlmOutcome>,
    deadline: Instant,
}

struct Inflight {
    reply: Sender<LlmOutcome>,
    sent: Instant,
    cancel_deadline: Option<Instant>,
    kind: &'static str,
    model: String,
}

struct Manager {
    spec: ChildSpec,
    timing: Timing,
    log: Arc<EngineLog>,
    shared: Arc<LlmShared>,
    sender: Sender<Msg>,
    receiver: Receiver<Msg>,
    backoff: Backoff,
    /// Counts child generations so a late message of a dead child is ignored.
    generation: u64,
    running: Option<Running>,
    next_start: Option<Instant>,
    /// A write failed. The reason is acted on after the message that caused it.
    broken: Option<String>,
    /// The last model the worker confirmed. A restart gives it to the new child.
    current: Option<LlmSpec>,
    /// The model was in memory when the last child went down, so the new child loads it at once.
    warm: bool,
    sent_loads: VecDeque<LoadEntry>,
    unsent_loads: VecDeque<LoadEntry>,
    pending: VecDeque<Pending>,
    inflight: HashMap<u64, Inflight>,
    model: Option<String>,
    backend: Option<String>,
    gpu: bool,
    unloaded: Option<UnloadReason>,
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
                Ok(Msg::Cmd(LlmCmd::Shutdown)) | Err(RecvTimeoutError::Disconnected) => {
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

    // ---- commands from the app ----

    fn command(&mut self, command: LlmCmd) {
        match command {
            LlmCmd::Load { spec, reply } => {
                let entry = LoadEntry {
                    spec,
                    reply: Some(reply),
                };
                if self.is_ready() {
                    self.send_load(entry);
                } else {
                    self.unsent_loads.push_back(entry);
                    self.publish();
                }
            }
            LlmCmd::Run { job, work, reply } => {
                if self.is_ready() {
                    self.send_job(job, &work, reply);
                } else {
                    self.pending.push_back(Pending {
                        job,
                        work,
                        reply,
                        deadline: Instant::now() + self.timing.submit_wait,
                    });
                }
            }
            LlmCmd::Cancel { job } => self.cancel(job),
            LlmCmd::Unload => {
                if self.is_ready() {
                    self.write(&LlmRequest::Unload);
                }
            }
            LlmCmd::Shutdown => {}
        }
    }

    fn cancel(&mut self, job: u64) {
        if let Some(position) = self.pending.iter().position(|p| p.job == job)
            && let Some(pending) = self.pending.remove(position)
        {
            let _ = pending.reply.send(LlmOutcome::Cancelled);
            self.log.write(
                Level::Info,
                "JOB",
                &format!(
                    "job={job} kind={} model=- code=CANCELLED queued",
                    pending.work.kind()
                ),
            );
            return;
        }
        let grace = self.timing.cancel_grace;
        let Some(inflight) = self.inflight.get_mut(&job) else {
            return;
        };
        if inflight.cancel_deadline.is_none() {
            inflight.cancel_deadline = Some(Instant::now() + grace);
            self.write(&LlmRequest::Cancel { job });
        }
    }

    fn send_load(&mut self, entry: LoadEntry) {
        let request = LlmRequest::LoadModel {
            path: entry.spec.path.to_string_lossy().into_owned(),
            family: entry.spec.family.clone(),
            gpu: entry.spec.gpu,
            threads: u32::try_from(entry.spec.threads).unwrap_or(u32::MAX),
            idle_secs: entry.spec.idle_secs,
        };
        if let Some(running) = self.running.as_mut() {
            running.configured = true;
        }
        self.sent_loads.push_back(entry);
        self.write(&request);
        self.publish();
    }

    fn send_job(&mut self, job: u64, work: &LlmWork, reply: Sender<LlmOutcome>) {
        // After a restart that left the model unloaded, the new child learns the model here.
        if self.running.as_ref().is_some_and(|r| !r.configured)
            && let Some(spec) = self.current.clone()
        {
            self.send_load(LoadEntry { spec, reply: None });
        }
        let model = self
            .sent_loads
            .back()
            .map(|entry| &entry.spec)
            .or(self.current.as_ref())
            .map_or_else(
                || "-".to_owned(),
                |load| crate::server::model_name(&load.path),
            );
        self.inflight.insert(
            job,
            Inflight {
                reply,
                sent: Instant::now(),
                cancel_deadline: None,
                kind: work.kind(),
                model,
            },
        );
        let request = match work {
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
        };
        self.write(&request);
    }

    // ---- events from the child ----

    fn event(&mut self, event: LlmEvent) {
        match event {
            LlmEvent::Ready { protocol, .. } if protocol != LLM_PROTOCOL_VERSION => {
                self.down("protocol mismatch", Restart::Backoff);
            }
            LlmEvent::Ready {
                version, backend, ..
            } => self.on_ready(&version, &backend),
            LlmEvent::Pong => {
                if let Some(running) = self.running.as_mut() {
                    running.last_pong = Instant::now();
                }
            }
            LlmEvent::Loading { .. } => {}
            LlmEvent::Loaded {
                model,
                ms,
                gpu,
                backend,
            } => self.on_loaded(model, ms, gpu, backend),
            LlmEvent::Unloaded { reason } => {
                self.log.write(
                    Level::Info,
                    "UNLOADED",
                    &format!("reason={}", reason.as_str()),
                );
                self.model = None;
                self.unloaded = Some(reason);
                self.publish();
            }
            LlmEvent::Result { job, text, stats } => {
                let line = format!(
                    "prompt_tokens={} reused_tokens={} new_tokens={} gen_tokens={} load_ms={} ms={}",
                    stats.prompt_tokens,
                    stats.reused_tokens,
                    stats.new_tokens,
                    stats.gen_tokens,
                    stats.load_ms,
                    stats.ms
                );
                self.finish(job, LlmOutcome::Done(LlmDone { text, stats }), &line);
            }
            LlmEvent::Cancelled { job } => self.finish(job, LlmOutcome::Cancelled, ""),
            LlmEvent::Error {
                job: Some(job),
                code,
                params,
            } => {
                let failure = failure_from(code, &params);
                self.finish(job, LlmOutcome::Failed(failure), "");
            }
            LlmEvent::Error {
                job: None,
                code,
                params,
            } => {
                let failure = failure_from(code.clone(), &params);
                if code == error::LLM_LOAD_FAILED
                    && let Some(entry) = self.sent_loads.pop_front()
                {
                    self.log.write(
                        Level::Warn,
                        "LOAD_FAILED",
                        &format!(
                            "model={} code={code}",
                            crate::server::model_name(&entry.spec.path)
                        ),
                    );
                    if let Some(reply) = entry.reply {
                        let _ = reply.send(Err(failure));
                    }
                    self.publish();
                } else {
                    self.log
                        .write(Level::Warn, "LLM_ERROR", &format!("code={code}"));
                }
            }
        }
    }

    fn on_loaded(&mut self, model: String, ms: u64, gpu: bool, backend: String) {
        // A reload after an idle unload arrives with no request waiting for it.
        let entry = self.sent_loads.pop_front();
        self.log.write(
            Level::Info,
            "LOADED",
            &format!(
                "model={model} gpu={} backend={backend} ms={ms}",
                on_off(gpu)
            ),
        );
        self.model = Some(model.clone());
        self.backend = Some(backend.clone());
        self.gpu = gpu;
        self.unloaded = None;
        if let Some(entry) = entry {
            self.current = Some(entry.spec);
            if let Some(reply) = entry.reply {
                let _ = reply.send(Ok(LlmLoaded {
                    model,
                    ms,
                    gpu,
                    backend,
                }));
            }
        }
        self.publish();
    }

    fn on_ready(&mut self, version: &str, backend: &str) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        if running.ready {
            return;
        }
        let now = Instant::now();
        running.ready = true;
        running.last_pong = now;
        running.next_ping = now + self.timing.ping_interval;
        let pid = running.child.id();
        self.log.write(
            Level::Info,
            "READY",
            &format!("pid={pid} version={version} backend={backend}"),
        );
        // A restart brings the model back without anyone asking, unless it had been dropped.
        if self.unsent_loads.is_empty()
            && self.warm
            && let Some(spec) = self.current.clone()
        {
            self.unsent_loads.push_back(LoadEntry { spec, reply: None });
        }
        while let Some(entry) = self.unsent_loads.pop_front() {
            self.send_load(entry);
        }
        while let Some(pending) = self.pending.pop_front() {
            self.send_job(pending.job, &pending.work, pending.reply);
        }
        self.publish();
    }

    /// Answers a job and writes its line to `llm.log`. The line holds counts, never text.
    fn finish(&mut self, job: u64, outcome: LlmOutcome, counts: &str) {
        let Some(inflight) = self.inflight.remove(&job) else {
            return;
        };
        // The user asked to stop this job, so a result that raced the cancel is dropped.
        let outcome =
            if inflight.cancel_deadline.is_some() && matches!(outcome, LlmOutcome::Done(_)) {
                LlmOutcome::Cancelled
            } else {
                outcome
            };
        let (code, level) = match &outcome {
            LlmOutcome::Done(_) => ("OK", Level::Info),
            LlmOutcome::Cancelled => ("CANCELLED", Level::Info),
            LlmOutcome::Failed(failure) => (failure.code.as_str(), Level::Warn),
        };
        self.log.write(
            level,
            "JOB",
            &format!(
                "job={job} kind={} model={} backend={} gpu={} {counts} total_ms={} code={code}",
                inflight.kind,
                inflight.model,
                self.backend.as_deref().unwrap_or("-"),
                on_off(self.gpu),
                inflight.sent.elapsed().as_millis()
            ),
        );
        let _ = inflight.reply.send(outcome);
    }

    // ---- the child process ----

    fn is_ready(&self) -> bool {
        self.running.as_ref().is_some_and(|running| running.ready)
    }

    fn start_child(&mut self) {
        self.next_start = None;
        let spawned = Command::new(&self.spec.program)
            .args(&self.spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(failure) => {
                self.failed_start(&failure.to_string());
                return;
            }
        };
        let (Some(mut stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            self.failed_start("the child has no pipes");
            return;
        };
        let now = Instant::now();
        self.log
            .write(Level::Info, "SPAWN", &format!("pid={}", child.id()));
        let hello = write_json(
            &mut stdin,
            &LlmRequest::Hello {
                protocol: LLM_PROTOCOL_VERSION,
            }
            .to_json(),
        );
        self.spawn_readers(stdout, stderr);
        self.running = Some(Running {
            child,
            stdin,
            spawned: now,
            ready: false,
            configured: false,
            last_pong: now,
            next_ping: now,
        });
        if let Err(failure) = hello {
            self.broken = Some(format!("hello failed: {failure}"));
        }
        self.publish();
    }

    fn failed_start(&mut self, reason: &str) {
        let delay = self.backoff.next_delay(Duration::ZERO);
        self.restarts += 1;
        self.next_start = Some(Instant::now() + delay);
        self.log.write(
            Level::Warn,
            "SPAWN_FAILED",
            &format!("{reason} restart_in_ms={}", delay.as_millis()),
        );
        self.publish();
    }

    fn spawn_readers(&self, stdout: ChildStdout, stderr: ChildStderr) {
        let generation = self.generation;
        let sender = self.sender.clone();
        thread::spawn(move || read_events(stdout, &sender, generation));
        let log = Arc::clone(&self.log);
        thread::spawn(move || pump_stderr(stderr, &log));
    }

    fn write(&mut self, request: &LlmRequest) {
        let Some(running) = self.running.as_mut() else {
            return;
        };
        if let Err(failure) = write_json(&mut running.stdin, &request.to_json()) {
            self.broken
                .get_or_insert_with(|| format!("write failed: {failure}"));
        }
    }

    /// Takes the child down (if it is not already), settles every request that waited on it,
    /// and schedules the next start.
    fn down(&mut self, reason: &str, restart: Restart) {
        let Some(mut running) = self.running.take() else {
            return;
        };
        // A caller released below must not read the dead child's pid as the live worker.
        self.publish();
        self.generation += 1;
        let pid = running.child.id();
        let uptime = running.spawned.elapsed();
        let _ = running.child.kill();
        self.warm = self.model.is_some();

        let failure = Failure::new(error::LLM_CRASHED, reason);
        let jobs: Vec<u64> = self.inflight.keys().copied().collect();
        for job in jobs {
            let cancelled = self
                .inflight
                .get(&job)
                .is_some_and(|inflight| inflight.cancel_deadline.is_some());
            let outcome = if cancelled {
                LlmOutcome::Cancelled
            } else {
                LlmOutcome::Failed(failure.clone())
            };
            self.finish(job, outcome, "");
        }
        for entry in self.sent_loads.drain(..) {
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure.clone()));
            }
        }

        drop(running.stdin);
        let status = running
            .child
            .wait()
            .map_or_else(|failure| failure.to_string(), |status| status.to_string());
        let delay = match restart {
            Restart::Backoff => self.backoff.next_delay(uptime),
            Restart::Now => Duration::ZERO,
        };
        self.next_start = Some(Instant::now() + delay);
        self.restarts += 1;
        self.model = None;
        self.backend = None;
        self.gpu = false;
        let code = match restart {
            Restart::Backoff => "CRASH",
            Restart::Now => "CANCEL_KILL",
        };
        self.log.write(
            Level::Warn,
            code,
            &format!(
                "pid={pid} reason={reason} exit={status} uptime_ms={} restart_in_ms={}",
                uptime.as_millis(),
                delay.as_millis()
            ),
        );
        self.publish();
    }

    fn stop(&mut self) {
        self.stopped = true;
        let failure = Failure::new(error::LLM_UNAVAILABLE, "the worker is shutting down");
        for pending in self.pending.drain(..) {
            let _ = pending.reply.send(LlmOutcome::Failed(failure.clone()));
        }
        for (_, inflight) in self.inflight.drain() {
            let _ = inflight.reply.send(LlmOutcome::Failed(failure.clone()));
        }
        for entry in self.sent_loads.drain(..).chain(self.unsent_loads.drain(..)) {
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure.clone()));
            }
        }
        if let Some(mut running) = self.running.take() {
            let _ = write_json(&mut running.stdin, &LlmRequest::Shutdown.to_json());
            drop(running.stdin);
            let deadline = Instant::now() + Duration::from_secs(1);
            let exited = loop {
                match running.child.try_wait() {
                    Ok(Some(_)) => break true,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    _ => break false,
                }
            };
            if !exited {
                let _ = running.child.kill();
                let _ = running.child.wait();
            }
            self.log.write(
                Level::Info,
                "STOP",
                &format!("pid={} clean={exited}", running.child.id()),
            );
        }
        self.model = None;
        self.publish();
    }

    // ---- deadlines ----

    fn tick(&mut self) {
        let now = Instant::now();
        if let Some(reason) = self.broken.take() {
            self.down(&reason, Restart::Backoff);
        }
        if self.running.is_none() && self.next_start.is_some_and(|at| now >= at) {
            self.start_child();
        }
        self.watch_child(now);
        self.expire_pending(now);
    }

    fn watch_child(&mut self, now: Instant) {
        let Some(running) = self.running.as_ref() else {
            return;
        };
        if !running.ready {
            if now.duration_since(running.spawned) >= self.timing.ready_timeout {
                self.down("no ready message", Restart::Backoff);
            }
            return;
        }
        if now.duration_since(running.last_pong) >= self.timing.pong_timeout {
            self.log
                .write(Level::Warn, "WATCHDOG", "no pong, killing the worker");
            self.down("no pong", Restart::Backoff);
            return;
        }
        let cancel_expired = self
            .inflight
            .values()
            .any(|job| job.cancel_deadline.is_some_and(|deadline| now >= deadline));
        if cancel_expired {
            self.down("cancel did not settle", Restart::Now);
            return;
        }
        if now >= running.next_ping {
            let next = now + self.timing.ping_interval;
            if let Some(running) = self.running.as_mut() {
                running.next_ping = next;
            }
            self.write(&LlmRequest::Ping);
        }
    }

    fn expire_pending(&mut self, now: Instant) {
        let mut kept = VecDeque::new();
        for pending in self.pending.drain(..) {
            if now >= pending.deadline {
                let failure = Failure::new(
                    error::LLM_UNAVAILABLE,
                    "the worker did not come back in time",
                );
                self.log.write(
                    Level::Warn,
                    "JOB",
                    &format!(
                        "job={} kind={} model=- code={}",
                        pending.job,
                        pending.work.kind(),
                        error::LLM_UNAVAILABLE
                    ),
                );
                let _ = pending.reply.send(LlmOutcome::Failed(failure));
            } else {
                kept.push_back(pending);
            }
        }
        self.pending = kept;
    }

    fn next_wakeup(&self) -> Instant {
        let mut next = Instant::now() + Duration::from_secs(3600);
        let mut consider = |at: Instant| next = next.min(at);
        match &self.running {
            None => {
                if let Some(at) = self.next_start {
                    consider(at);
                }
            }
            Some(running) if !running.ready => {
                consider(running.spawned + self.timing.ready_timeout);
            }
            Some(running) => {
                consider(running.next_ping);
                consider(running.last_pong + self.timing.pong_timeout);
                for job in self.inflight.values() {
                    if let Some(at) = job.cancel_deadline {
                        consider(at);
                    }
                }
            }
        }
        for pending in &self.pending {
            consider(pending.deadline);
        }
        if self.broken.is_some() {
            consider(Instant::now());
        }
        next
    }

    fn publish(&self) {
        let state = if self.stopped {
            LlmState::Stopped
        } else {
            match &self.running {
                None => LlmState::Backoff,
                Some(running) if !running.ready => LlmState::Starting,
                Some(_) if !self.sent_loads.is_empty() => LlmState::Loading,
                Some(_) => LlmState::Ready,
            }
        };
        let pid = self.running.as_ref().map(|running| running.child.id());
        self.shared.update(|status: &mut LlmStatus| {
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

fn failure_from(code: String, params: &hushpen_core::protocol::Params) -> Failure {
    let (detail, reason) = hushpen_core::protocol::error_texts(params);
    Failure {
        code,
        detail,
        reason,
    }
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn read_events(stdout: ChildStdout, sender: &Sender<Msg>, generation: u64) {
    let mut stdout = stdout;
    loop {
        let event = match read_frame(&mut stdout) {
            Ok(Some(frame)) => match LlmEvent::from_frame(&frame) {
                Ok(event) => ChildEvent::Event(event),
                Err(problem) => ChildEvent::Closed(format!("unreadable message: {problem}")),
            },
            Ok(None) => ChildEvent::Closed("the worker closed its output".into()),
            Err(problem) => ChildEvent::Closed(problem.to_string()),
        };
        let last = matches!(event, ChildEvent::Closed(_));
        if sender.send(Msg::Child { generation, event }).is_err() || last {
            return;
        }
    }
}

fn pump_stderr(stderr: ChildStderr, log: &EngineLog) {
    for line in BufReader::new(stderr).split(b'\n') {
        let Ok(bytes) = line else { return };
        let text = String::from_utf8_lossy(&bytes);
        let text = text.trim();
        if !text.is_empty() {
            log.write(Level::Info, "STDERR", text);
        }
    }
}
