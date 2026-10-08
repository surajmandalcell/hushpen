//! The supervisor thread. It owns the child process and every deadline: restart delays, the
//! ping watchdog, and the cancel grace. All writes to the child's stdin happen here, so frames
//! never interleave, and every request reaches a reply or a failure, whatever the child does.

use crate::backoff::Backoff;
use crate::client::{
    ChildSpec, Cmd, EngineState, Failure, JobOutcome, LoadSpec, Loaded, Shared, Timing,
    TranscribeSpec, Transcription,
};
use crate::engine_log::{EngineLog, Level};
use crate::server::model_name;
use hushpen_core::error;
use hushpen_core::protocol::{Event, PROTOCOL_VERSION, Request, read_frame, write_json};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
    spec: TranscribeSpec,
    reply: Sender<JobOutcome>,
    deadline: Instant,
}

struct Inflight {
    reply: Sender<JobOutcome>,
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

    // ---- commands from the app ----

    fn command(&mut self, command: Cmd) {
        match command {
            Cmd::Load { spec, reply } => {
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
            Cmd::Transcribe { job, spec, reply } => {
                if self.is_ready() {
                    self.send_job(job, &spec, reply);
                } else {
                    self.pending.push_back(Pending {
                        job,
                        spec,
                        reply,
                        deadline: Instant::now() + self.timing.submit_wait,
                    });
                }
            }
            Cmd::Cancel { job } => self.cancel(job),
            Cmd::Shutdown => {}
        }
    }

    fn cancel(&mut self, job: u64) {
        if let Some(position) = self.pending.iter().position(|p| p.job == job)
            && let Some(pending) = self.pending.remove(position)
        {
            let _ = pending.reply.send(JobOutcome::Cancelled);
            self.log.write(
                Level::Info,
                "JOB",
                &format!("job={job} model=- threads=- code=CANCELLED queued"),
            );
            return;
        }
        let grace = self.timing.cancel_grace;
        let Some(inflight) = self.inflight.get_mut(&job) else {
            return;
        };
        if inflight.cancel_deadline.is_none() {
            inflight.cancel_deadline = Some(Instant::now() + grace);
            self.write(&Request::Cancel { job });
        }
    }

    fn send_load(&mut self, entry: LoadEntry) {
        let request = Request::LoadModel {
            path: entry.spec.path.to_string_lossy().into_owned(),
            gpu: entry.spec.gpu,
            threads: u32::try_from(entry.spec.threads).unwrap_or(u32::MAX),
        };
        self.sent_loads.push_back(entry);
        self.write(&request);
        self.publish();
    }

    fn send_job(&mut self, job: u64, spec: &TranscribeSpec, reply: Sender<JobOutcome>) {
        let (model, threads) = self
            .sent_loads
            .back()
            .map(|entry| &entry.spec)
            .or(self.current.as_ref())
            .map_or_else(
                || ("-".to_owned(), 0),
                |load| (model_name(&load.path), load.threads),
            );
        self.inflight.insert(
            job,
            Inflight {
                reply,
                sent: Instant::now(),
                cancel_deadline: None,
                model,
                threads,
            },
        );
        self.write(&Request::Transcribe {
            job,
            wav_path: spec.wav_path.to_string_lossy().into_owned(),
            language: spec.language.clone(),
            prompt: spec.prompt.clone(),
        });
    }

    // ---- events from the child ----

    fn event(&mut self, event: Event) {
        match event {
            Event::Ready { protocol, .. } if protocol != PROTOCOL_VERSION => {
                self.down("protocol mismatch", Restart::Backoff);
            }
            Event::Ready { version, gpu, .. } => self.on_ready(&version, gpu),
            Event::Pong => {
                if let Some(running) = self.running.as_mut() {
                    running.last_pong = Instant::now();
                }
            }
            Event::Loading { .. } | Event::Segment { .. } => {}
            Event::Loaded { model, ms, gpu } => {
                let Some(entry) = self.sent_loads.pop_front() else {
                    return;
                };
                self.log.write(
                    Level::Info,
                    "LOADED",
                    &format!(
                        "model={model} threads={} gpu={} ms={ms}",
                        entry.spec.threads,
                        on_off(gpu)
                    ),
                );
                self.model = Some(model.clone());
                self.gpu = gpu;
                self.current = Some(entry.spec);
                if let Some(reply) = entry.reply {
                    let _ = reply.send(Ok(Loaded { model, ms, gpu }));
                }
                self.publish();
            }
            Event::Result {
                job,
                text,
                language,
                segments,
                audio_ms,
                ms,
            } => self.finish(
                job,
                JobOutcome::Done(Transcription {
                    text,
                    language,
                    segments,
                    audio_ms,
                    decode_ms: ms,
                }),
                Some((audio_ms, ms)),
            ),
            Event::Cancelled { job } => self.finish(job, JobOutcome::Cancelled, None),
            Event::Error {
                job: Some(job),
                code,
                params,
            } => {
                let failure = failure_from(code, &params);
                self.finish(job, JobOutcome::Failed(failure), None);
            }
            Event::Error {
                job: None,
                code,
                params,
            } => {
                let failure = failure_from(code.clone(), &params);
                if code == error::ENGINE_LOAD_FAILED
                    && let Some(entry) = self.sent_loads.pop_front()
                {
                    self.log.write(
                        Level::Warn,
                        "LOAD_FAILED",
                        &format!("model={} code={code}", model_name(&entry.spec.path)),
                    );
                    if let Some(reply) = entry.reply {
                        let _ = reply.send(Err(failure));
                    }
                    self.publish();
                } else {
                    self.log
                        .write(Level::Warn, "ENGINE_ERROR", &format!("code={code}"));
                }
            }
        }
    }

    fn on_ready(&mut self, version: &str, gpu: bool) {
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
            &format!("pid={pid} version={version} gpu_available={}", on_off(gpu)),
        );
        // A restart brings the dictation model back without anyone asking.
        if self.unsent_loads.is_empty()
            && let Some(spec) = self.current.clone()
        {
            self.unsent_loads.push_back(LoadEntry { spec, reply: None });
        }
        while let Some(entry) = self.unsent_loads.pop_front() {
            self.send_load(entry);
        }
        while let Some(pending) = self.pending.pop_front() {
            self.send_job(pending.job, &pending.spec, pending.reply);
        }
        self.publish();
    }

    /// Answers a job and writes its line to `engine.log`.
    fn finish(&mut self, job: u64, outcome: JobOutcome, timings: Option<(u64, u64)>) {
        let Some(inflight) = self.inflight.remove(&job) else {
            return;
        };
        // The user asked to stop this job, so a result that raced the cancel is dropped.
        let outcome =
            if inflight.cancel_deadline.is_some() && matches!(outcome, JobOutcome::Done(_)) {
                JobOutcome::Cancelled
            } else {
                outcome
            };
        self.log_job(job, &inflight, &outcome, timings);
        let _ = inflight.reply.send(outcome);
    }

    fn log_job(
        &self,
        job: u64,
        inflight: &Inflight,
        outcome: &JobOutcome,
        timings: Option<(u64, u64)>,
    ) {
        let (code, level) = match outcome {
            JobOutcome::Done(_) => ("OK", Level::Info),
            JobOutcome::Cancelled => ("CANCELLED", Level::Info),
            JobOutcome::Failed(failure) => (failure.code.as_str(), Level::Warn),
        };
        let (audio, decode) = timings.map_or_else(
            || ("-".to_owned(), "-".to_owned()),
            |(audio, decode)| (audio.to_string(), decode.to_string()),
        );
        self.log.write(
            level,
            "JOB",
            &format!(
                "job={job} model={} threads={} gpu={} audio_ms={audio} decode_ms={decode} total_ms={} code={code}",
                inflight.model,
                inflight.threads,
                on_off(self.gpu),
                inflight.sent.elapsed().as_millis()
            ),
        );
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
                let delay = self.backoff.next_delay(Duration::ZERO);
                self.restarts += 1;
                self.next_start = Some(Instant::now() + delay);
                self.log.write(
                    Level::Warn,
                    "SPAWN_FAILED",
                    &format!("{failure} restart_in_ms={}", delay.as_millis()),
                );
                self.publish();
                return;
            }
        };
        let (Some(mut stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            self.down_failed_start("the child has no pipes");
            return;
        };
        let now = Instant::now();
        self.log
            .write(Level::Info, "SPAWN", &format!("pid={}", child.id()));
        let hello = write_json(
            &mut stdin,
            &Request::Hello {
                protocol: PROTOCOL_VERSION,
            }
            .to_json(),
        );
        self.spawn_readers(stdout, stderr);
        self.running = Some(Running {
            child,
            stdin,
            spawned: now,
            ready: false,
            last_pong: now,
            next_ping: now,
        });
        if let Err(failure) = hello {
            self.broken = Some(format!("hello failed: {failure}"));
        }
        self.publish();
    }

    fn down_failed_start(&mut self, reason: &str) {
        let delay = self.backoff.next_delay(Duration::ZERO);
        self.restarts += 1;
        self.next_start = Some(Instant::now() + delay);
        self.log.write(Level::Warn, "SPAWN_FAILED", reason);
        self.publish();
    }

    fn spawn_readers(&self, stdout: ChildStdout, stderr: ChildStderr) {
        let generation = self.generation;
        let sender = self.sender.clone();
        thread::spawn(move || read_events(stdout, &sender, generation));
        let log = Arc::clone(&self.log);
        thread::spawn(move || pump_stderr(stderr, &log));
    }

    fn write(&mut self, request: &Request) {
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
        // A caller released below must not read the dead child's pid as the live engine.
        self.publish();
        self.generation += 1;
        let pid = running.child.id();
        let uptime = running.spawned.elapsed();
        let _ = running.child.kill();

        let failure = Failure::new(error::ENGINE_CRASHED, reason);
        let jobs: Vec<u64> = self.inflight.keys().copied().collect();
        for job in jobs {
            let cancelled = self
                .inflight
                .get(&job)
                .is_some_and(|inflight| inflight.cancel_deadline.is_some());
            let outcome = if cancelled {
                JobOutcome::Cancelled
            } else {
                JobOutcome::Failed(failure.clone())
            };
            self.finish(job, outcome, None);
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
        self.gpu = false;
        let (code, level) = match restart {
            Restart::Backoff => ("CRASH", Level::Warn),
            Restart::Now => ("CANCEL_KILL", Level::Warn),
        };
        self.log.write(
            level,
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
        let failure = Failure::new(error::ENGINE_UNAVAILABLE, "the engine is shutting down");
        for pending in self.pending.drain(..) {
            let _ = pending.reply.send(JobOutcome::Failed(failure.clone()));
        }
        for (_, inflight) in self.inflight.drain() {
            let _ = inflight.reply.send(JobOutcome::Failed(failure.clone()));
        }
        for entry in self.sent_loads.drain(..).chain(self.unsent_loads.drain(..)) {
            if let Some(reply) = entry.reply {
                let _ = reply.send(Err(failure.clone()));
            }
        }
        if let Some(mut running) = self.running.take() {
            let _ = write_json(&mut running.stdin, &Request::Shutdown.to_json());
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
                .write(Level::Warn, "WATCHDOG", "no pong, killing the engine");
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
            self.write(&Request::Ping);
        }
    }

    fn expire_pending(&mut self, now: Instant) {
        let mut kept = VecDeque::new();
        for pending in self.pending.drain(..) {
            if now >= pending.deadline {
                let failure = Failure::new(
                    error::ENGINE_UNAVAILABLE,
                    "the engine did not come back in time",
                );
                self.log.write(
                    Level::Warn,
                    "JOB",
                    &format!(
                        "job={} model=- threads=- code={}",
                        pending.job,
                        error::ENGINE_UNAVAILABLE
                    ),
                );
                let _ = pending.reply.send(JobOutcome::Failed(failure));
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
            Ok(Some(frame)) => match Event::from_frame(&frame) {
                Ok(event) => ChildEvent::Event(event),
                Err(problem) => ChildEvent::Closed(format!("unreadable message: {problem}")),
            },
            Ok(None) => ChildEvent::Closed("the engine closed its output".into()),
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
