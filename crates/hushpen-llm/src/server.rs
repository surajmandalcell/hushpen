//! The worker child: reads requests from one pipe, writes events to another.
//!
//! The reading thread never blocks on a model. It answers `Ping`, handles `Cancel`, and queues
//! everything else for one worker thread that owns the loaded model. A ping still gets its pong
//! during a load or a long job, and a cancel settles a queued job at once.
//!
//! The worker thread keeps the last `LoadModel` after it drops an idle model, so the next job
//! loads the model again without a new request from the app.

use crate::backend::{
    Backend, CancelFlag, JobError, JobSpec, LoadError, LoadSpec, Session, SessionEnd,
};
use hushpen_core::error;
use hushpen_core::llm_prompt::{COMMAND_SYSTEM, PromptFamily, command_user};
use hushpen_core::llm_protocol::{LLM_PROTOCOL_VERSION, LlmEvent, LlmRequest, UnloadReason};
use hushpen_core::protocol::{DecodeError, Frame, FrameError, FrameKind, read_frame, write_json};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// Exit code after a frame the child cannot read past. The supervisor restarts it.
pub const EXIT_BAD_FRAME: u8 = 3;
/// Exit code after a `Hello` with a protocol version this child does not speak.
pub const EXIT_BAD_PROTOCOL: u8 = 4;

/// How long a shutdown waits for a running job to notice its cancel flag.
const SHUTDOWN_GRACE: Duration = Duration::from_millis(700);

pub struct ServerConfig {
    /// Reported in `Ready`.
    pub version: String,
    pub backend: Arc<dyn Backend>,
}

type Output = Arc<Mutex<Box<dyn Write + Send>>>;

struct JobEntry {
    cancel: CancelFlag,
    started: bool,
}

type Jobs = Arc<Mutex<HashMap<u64, JobEntry>>>;

enum Work {
    Load(LoadSpec),
    Job { job: u64, spec: JobSpec },
    Unload,
}

/// Runs the request loop until `Shutdown`, the end of the input, or a bad frame. Returns the
/// exit code of the child.
pub fn serve(input: impl Read, output: impl Write + Send + 'static, config: ServerConfig) -> u8 {
    let output: Output = Arc::new(Mutex::new(Box::new(output)));
    let jobs: Jobs = Arc::default();
    let (work_tx, work_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker = Worker {
        output: Arc::clone(&output),
        jobs: Arc::clone(&jobs),
        backend: Arc::clone(&config.backend),
    };
    thread::spawn(move || {
        worker.run(&work_rx);
        let _ = done_tx.send(());
    });

    let code = read_requests(input, &output, &jobs, &work_tx, &config);

    for entry in lock(&jobs).values() {
        entry.cancel.cancel();
    }
    drop(work_tx);
    // A job that ignores its flag must not keep the child alive: the process exits anyway.
    let _ = done_rx.recv_timeout(SHUTDOWN_GRACE);
    code
}

fn read_requests(
    mut input: impl Read,
    output: &Output,
    jobs: &Jobs,
    work: &Sender<Work>,
    config: &ServerConfig,
) -> u8 {
    loop {
        let frame = match read_frame(&mut input) {
            Ok(Some(frame)) => frame,
            Ok(None) => return 0,
            Err(problem) => {
                eprintln!("hushpen-llm: {problem}");
                if !matches!(problem, FrameError::Io(_)) {
                    send(
                        output,
                        &LlmEvent::error(None, error::LLM_PROTOCOL, problem.to_string()),
                    );
                }
                return EXIT_BAD_FRAME;
            }
        };
        let request = match decode(&frame) {
            Ok(request) => request,
            Err(problem) => {
                send(
                    output,
                    &LlmEvent::error(None, error::LLM_PROTOCOL, problem.to_string()),
                );
                continue;
            }
        };
        match request {
            LlmRequest::Hello { protocol } if protocol == LLM_PROTOCOL_VERSION => send(
                output,
                &LlmEvent::Ready {
                    version: config.version.clone(),
                    protocol: LLM_PROTOCOL_VERSION,
                    backend: config.backend.backend_name().to_owned(),
                },
            ),
            LlmRequest::Hello { protocol } => {
                send(
                    output,
                    &LlmEvent::error(
                        None,
                        error::LLM_PROTOCOL,
                        format!(
                            "protocol {protocol} is not supported, this worker speaks {LLM_PROTOCOL_VERSION}"
                        ),
                    ),
                );
                return EXIT_BAD_PROTOCOL;
            }
            LlmRequest::LoadModel {
                path,
                family,
                gpu,
                threads,
                idle_secs,
            } => match load_spec(path, family.as_deref(), gpu, threads, idle_secs) {
                Ok(spec) => {
                    let _ = work.send(Work::Load(spec));
                }
                Err(detail) => send(
                    output,
                    &LlmEvent::error(None, error::LLM_LOAD_FAILED, detail),
                ),
            },
            LlmRequest::Cleanup {
                job,
                system,
                user,
                max_tokens,
            } => submit(
                output,
                jobs,
                work,
                job,
                JobSpec {
                    system,
                    user,
                    max_tokens,
                },
            ),
            LlmRequest::Command {
                job,
                instruction,
                selection,
                max_tokens,
            } => submit(
                output,
                jobs,
                work,
                job,
                JobSpec {
                    system: COMMAND_SYSTEM.to_owned(),
                    user: command_user(&instruction, &selection),
                    max_tokens,
                },
            ),
            LlmRequest::Unload => {
                let _ = work.send(Work::Unload);
            }
            LlmRequest::Cancel { job } => cancel(job, jobs, output),
            LlmRequest::Ping => send(output, &LlmEvent::Pong),
            LlmRequest::Shutdown => return 0,
        }
    }
}

fn load_spec(
    path: String,
    family: Option<&str>,
    gpu: bool,
    threads: u32,
    idle_secs: u64,
) -> Result<LoadSpec, String> {
    let path = PathBuf::from(path);
    let family = match family {
        Some(name) => {
            PromptFamily::parse(name).ok_or_else(|| format!("unknown family '{name}'"))?
        }
        None => {
            let file = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            PromptFamily::from_model_name(&file)
                .ok_or_else(|| "the model family is not known from the file name".to_owned())?
        }
    };
    Ok(LoadSpec {
        path,
        family,
        gpu,
        threads: threads.max(1) as usize,
        idle: (idle_secs > 0).then(|| Duration::from_secs(idle_secs)),
    })
}

fn submit(output: &Output, jobs: &Jobs, work: &Sender<Work>, job: u64, spec: JobSpec) {
    let fresh = match lock(jobs).entry(job) {
        Entry::Occupied(_) => false,
        Entry::Vacant(slot) => {
            slot.insert(JobEntry {
                cancel: CancelFlag::new(),
                started: false,
            });
            true
        }
    };
    if fresh {
        let _ = work.send(Work::Job { job, spec });
    } else {
        send(
            output,
            &LlmEvent::error(Some(job), error::LLM_PROTOCOL, "job id is in use"),
        );
    }
}

fn decode(frame: &Frame) -> Result<LlmRequest, DecodeError> {
    if frame.kind == FrameKind::Binary {
        return Err(DecodeError::Invalid(
            "the worker takes no binary frames".into(),
        ));
    }
    LlmRequest::from_frame(frame)
}

/// A job that is running stops through its flag. A job that has not started is settled here, so
/// a cancel does not wait for the load or the job in front of it.
fn cancel(job: u64, jobs: &Jobs, output: &Output) {
    let mut jobs = lock(jobs);
    match jobs.get(&job) {
        Some(entry) if entry.started => entry.cancel.cancel(),
        Some(_) => {
            jobs.remove(&job);
            send(output, &LlmEvent::Cancelled { job });
        }
        None => {}
    }
}

struct Worker {
    output: Output,
    jobs: Jobs,
    backend: Arc<dyn Backend>,
}

impl Worker {
    fn run(&self, work: &Receiver<Work>) {
        // The last model the app asked for. It survives an idle unload.
        let mut remembered: Option<LoadSpec> = None;
        let mut next: Option<Work> = None;
        loop {
            let item = match next.take() {
                Some(item) => item,
                None => match work.recv() {
                    Ok(item) => item,
                    Err(_) => return,
                },
            };
            let (spec, first_job) = match item {
                Work::Load(spec) => {
                    remembered = Some(spec.clone());
                    (spec, None)
                }
                Work::Job {
                    job,
                    spec: job_spec,
                } => match remembered.clone() {
                    Some(spec) => (spec, Some((job, job_spec))),
                    None => {
                        self.finish(
                            job,
                            &LlmEvent::error(Some(job), error::LLM_NO_MODEL, "no model is loaded"),
                        );
                        continue;
                    }
                },
                Work::Unload => {
                    send(
                        &self.output,
                        &LlmEvent::Unloaded {
                            reason: UnloadReason::Requested,
                        },
                    );
                    continue;
                }
            };
            match self.session(&spec, first_job, work) {
                Ok(SessionEnd::Quit) => return,
                Ok(SessionEnd::Reload(spec)) => next = Some(Work::Load(spec)),
                Ok(SessionEnd::Unload(reason)) => {
                    send(&self.output, &LlmEvent::Unloaded { reason });
                }
                Err((failure, abandoned)) => {
                    send(
                        &self.output,
                        &LlmEvent::error(None, error::LLM_LOAD_FAILED, failure.detail.clone()),
                    );
                    if let Some(job) = abandoned {
                        self.finish(
                            job,
                            &LlmEvent::error(Some(job), error::LLM_LOAD_FAILED, failure.detail),
                        );
                    }
                }
            }
        }
    }

    /// Loads `spec`, runs `first_job` and everything after it, and returns when the model is to
    /// be dropped. On a failed load the error carries the id of the job that waited for it.
    fn session(
        &self,
        spec: &LoadSpec,
        first_job: Option<(u64, JobSpec)>,
        work: &Receiver<Work>,
    ) -> Result<SessionEnd, (LoadError, Option<u64>)> {
        let model = spec.model_name();
        send(
            &self.output,
            &LlmEvent::Loading {
                model: model.clone(),
            },
        );
        let waiting = first_job.as_ref().map(|(job, _)| *job);
        let mut first_job = first_job;
        let backend_name = self.backend.backend_name().to_owned();
        let mut body = |session: &mut dyn Session, info: crate::backend::LoadInfo| {
            send(
                &self.output,
                &LlmEvent::Loaded {
                    model: model.clone(),
                    ms: info.ms,
                    gpu: info.gpu,
                    backend: backend_name.clone(),
                },
            );
            let mut load_ms = if first_job.is_some() { info.ms } else { 0 };
            loop {
                let item = match first_job.take() {
                    Some((job, job_spec)) => Work::Job {
                        job,
                        spec: job_spec,
                    },
                    None => match wait_for_work(work, spec.idle) {
                        Ok(item) => item,
                        Err(RecvTimeoutError::Timeout) => {
                            return SessionEnd::Unload(UnloadReason::Idle);
                        }
                        Err(RecvTimeoutError::Disconnected) => return SessionEnd::Quit,
                    },
                };
                match item {
                    Work::Job {
                        job,
                        spec: job_spec,
                    } => {
                        self.run_job(session, job, &job_spec, load_ms);
                        load_ms = 0;
                    }
                    Work::Load(new) if new == *spec => send(
                        &self.output,
                        &LlmEvent::Loaded {
                            model: model.clone(),
                            ms: 0,
                            gpu: info.gpu,
                            backend: backend_name.clone(),
                        },
                    ),
                    Work::Load(new) => return SessionEnd::Reload(new),
                    Work::Unload => return SessionEnd::Unload(UnloadReason::Requested),
                }
            }
        };
        self.backend
            .with_session(spec, &mut body)
            .map_err(|failure| (failure, waiting))
    }

    fn run_job(&self, session: &mut dyn Session, job: u64, spec: &JobSpec, load_ms: u64) {
        let Some(cancel) = self.start(job) else {
            return;
        };
        let started = Instant::now();
        let event = match session.run(spec, &cancel) {
            Ok(output) => {
                let mut stats = output.stats;
                stats.load_ms = load_ms;
                stats.ms = elapsed_ms(started) + load_ms;
                LlmEvent::Result {
                    job,
                    text: output.text,
                    stats,
                }
            }
            Err(JobError::Cancelled) => LlmEvent::Cancelled { job },
            Err(JobError::Failed { code, detail }) => LlmEvent::error(Some(job), code, detail),
        };
        self.finish(job, &event);
    }

    /// Marks the job as running. `None` when a cancel already settled it.
    fn start(&self, job: u64) -> Option<CancelFlag> {
        let mut jobs = lock(&self.jobs);
        let entry = jobs.get_mut(&job)?;
        entry.started = true;
        Some(entry.cancel.clone())
    }

    fn finish(&self, job: u64, event: &LlmEvent) {
        // A job that a cancel settled while it was queued has no entry any more and no answer to give.
        if lock(&self.jobs).remove(&job).is_some() {
            send(&self.output, event);
        }
    }
}

fn wait_for_work(work: &Receiver<Work>, idle: Option<Duration>) -> Result<Work, RecvTimeoutError> {
    match idle {
        Some(idle) => work.recv_timeout(idle),
        None => work.recv().map_err(|_| RecvTimeoutError::Disconnected),
    }
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn send(output: &Output, event: &LlmEvent) {
    // A broken pipe means the app is gone; the reader sees the same and ends the loop.
    let _ = write_json(&mut *lock(output), &event.to_json());
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
