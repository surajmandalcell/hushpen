//! The worker child: reads requests from one pipe, writes events to another.
//!
//! As in `hushpen-llm`, the reading thread never blocks on the model. It answers `Ping` and
//! `ListVoices`, handles `Cancel`, and queues loads and jobs for one worker thread that owns the
//! loaded model. A ping still gets its pong during a load or a long job, and a cancel settles a
//! queued job at once.
//!
//! The worker thread keeps the last `LoadModel` after it drops an idle model, so the next job
//! loads the model again without a new request from the app.

use crate::backend::{Backend, CancelFlag, JobError, LoadSpec, Session};
use hushpen_core::error;
use hushpen_core::kokoro::VOICES;
use hushpen_core::llm_protocol::UnloadReason;
use hushpen_core::protocol::{DecodeError, Frame, FrameError, FrameKind, read_frame};
use hushpen_core::tts_protocol::{
    MAX_AUDIO_SAMPLES, SpeakStats, TTS_PROTOCOL_VERSION, TtsEvent, TtsRequest, VoiceInfo,
    write_event,
};
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

struct SpeakJob {
    job: u64,
    text: String,
    voice: Option<String>,
    speed: f32,
}

enum Work {
    Load(LoadSpec),
    Speak(SpeakJob),
    Unload,
}

/// The English voices of the Kokoro model, for `ListVoices`.
pub fn voice_list() -> Vec<VoiceInfo> {
    VOICES
        .iter()
        .map(|voice| VoiceInfo {
            id: voice.id.to_owned(),
            name: voice.name(),
            language: voice.language().to_owned(),
        })
        .collect()
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
                eprintln!("hushpen-tts: {problem}");
                if !matches!(problem, FrameError::Io(_)) {
                    send(
                        output,
                        &TtsEvent::error(None, error::TTS_PROTOCOL, problem.to_string()),
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
                    &TtsEvent::error(None, error::TTS_PROTOCOL, problem.to_string()),
                );
                continue;
            }
        };
        match request {
            TtsRequest::Hello { protocol } if protocol == TTS_PROTOCOL_VERSION => send(
                output,
                &TtsEvent::Ready {
                    version: config.version.clone(),
                    protocol: TTS_PROTOCOL_VERSION,
                },
            ),
            TtsRequest::Hello { protocol } => {
                send(
                    output,
                    &TtsEvent::error(
                        None,
                        error::TTS_PROTOCOL,
                        format!(
                            "protocol {protocol} is not supported, this worker speaks {TTS_PROTOCOL_VERSION}"
                        ),
                    ),
                );
                return EXIT_BAD_PROTOCOL;
            }
            TtsRequest::LoadModel {
                dir,
                engine,
                threads,
                idle_secs,
            } => {
                let spec = LoadSpec {
                    dir: PathBuf::from(dir),
                    engine,
                    threads: threads.max(1) as usize,
                    idle: (idle_secs > 0).then(|| Duration::from_secs(idle_secs)),
                };
                let _ = work.send(Work::Load(spec));
            }
            TtsRequest::ListVoices => send(
                output,
                &TtsEvent::Voices {
                    voices: voice_list(),
                },
            ),
            TtsRequest::Speak {
                job,
                text,
                voice,
                speed,
            } => {
                if text.trim().is_empty() {
                    send(
                        output,
                        &TtsEvent::error(Some(job), error::SPEECH_NO_TEXT, "the text is empty"),
                    );
                    continue;
                }
                submit(
                    output,
                    jobs,
                    work,
                    SpeakJob {
                        job,
                        text,
                        voice,
                        speed,
                    },
                );
            }
            TtsRequest::Unload => {
                let _ = work.send(Work::Unload);
            }
            TtsRequest::Cancel { job } => cancel(job, jobs, output),
            TtsRequest::Ping => send(output, &TtsEvent::Pong),
            TtsRequest::Shutdown => return 0,
        }
    }
}

fn submit(output: &Output, jobs: &Jobs, work: &Sender<Work>, job: SpeakJob) {
    let fresh = match lock(jobs).entry(job.job) {
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
        let _ = work.send(Work::Speak(job));
    } else {
        send(
            output,
            &TtsEvent::error(Some(job.job), error::TTS_PROTOCOL, "job id is in use"),
        );
    }
}

fn decode(frame: &Frame) -> Result<TtsRequest, DecodeError> {
    if frame.kind == FrameKind::Binary {
        return Err(DecodeError::Invalid(
            "the worker takes no binary frames".into(),
        ));
    }
    TtsRequest::from_frame(frame)
}

/// A job that is running stops through its flag. A job that has not started is settled here, so
/// a cancel does not wait for the load or the job in front of it.
fn cancel(job: u64, jobs: &Jobs, output: &Output) {
    let mut jobs = lock(jobs);
    match jobs.get(&job) {
        Some(entry) if entry.started => entry.cancel.cancel(),
        Some(_) => {
            jobs.remove(&job);
            send(output, &TtsEvent::Cancelled { job });
        }
        None => {}
    }
}

struct Loaded {
    spec: LoadSpec,
    session: Box<dyn Session>,
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
        let mut loaded: Option<Loaded> = None;
        loop {
            let idle = loaded.as_ref().and_then(|model| model.spec.idle);
            let item = match idle {
                Some(idle) => match work.recv_timeout(idle) {
                    Ok(item) => item,
                    Err(RecvTimeoutError::Timeout) => {
                        loaded = None;
                        self.unloaded(UnloadReason::Idle);
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                },
                None => match work.recv() {
                    Ok(item) => item,
                    Err(_) => return,
                },
            };
            match item {
                Work::Load(spec) => {
                    remembered = Some(spec.clone());
                    if loaded.as_ref().is_some_and(|model| model.spec == spec) {
                        self.loaded(&spec, 0);
                        continue;
                    }
                    // The old model leaves memory before the new one comes in.
                    loaded = None;
                    if let Ok((model, _)) = self.load(&spec) {
                        loaded = Some(model);
                    }
                }
                Work::Speak(job) => {
                    let Some(cancel) = self.start(job.job) else {
                        continue;
                    };
                    let started = Instant::now();
                    let mut load_ms = 0;
                    if loaded.is_none() {
                        let Some(spec) = remembered.clone() else {
                            self.finish(
                                job.job,
                                &TtsEvent::error(
                                    Some(job.job),
                                    error::TTS_NO_MODEL,
                                    "no voice model is loaded",
                                ),
                            );
                            continue;
                        };
                        match self.load(&spec) {
                            Ok((model, ms)) => {
                                load_ms = ms;
                                loaded = Some(model);
                            }
                            Err(detail) => {
                                self.finish(
                                    job.job,
                                    &TtsEvent::error(Some(job.job), error::TTS_LOAD_FAILED, detail),
                                );
                                continue;
                            }
                        }
                    }
                    if let Some(model) = loaded.as_mut() {
                        self.run_job(model.session.as_mut(), &job, &cancel, started, load_ms);
                    }
                }
                Work::Unload => {
                    loaded = None;
                    self.unloaded(UnloadReason::Requested);
                }
            }
        }
    }

    /// Loads `spec` and reports it with the time it took. A failure is reported as an error of
    /// no job.
    fn load(&self, spec: &LoadSpec) -> Result<(Loaded, u64), String> {
        let model = spec.model_name();
        send(&self.output, &TtsEvent::Loading { model });
        let started = Instant::now();
        match self.backend.load(spec) {
            Ok(session) => {
                let ms = elapsed_ms(started);
                self.loaded(spec, ms);
                let model = Loaded {
                    spec: spec.clone(),
                    session,
                };
                Ok((model, ms))
            }
            Err(failure) => {
                send(
                    &self.output,
                    &TtsEvent::error(None, error::TTS_LOAD_FAILED, failure.detail.clone()),
                );
                Err(failure.detail)
            }
        }
    }

    fn loaded(&self, spec: &LoadSpec, ms: u64) {
        send(
            &self.output,
            &TtsEvent::Loaded {
                model: spec.model_name(),
                ms,
            },
        );
    }

    fn unloaded(&self, reason: UnloadReason) {
        send(&self.output, &TtsEvent::Unloaded { reason });
    }

    fn run_job(
        &self,
        session: &mut dyn Session,
        job: &SpeakJob,
        cancel: &CancelFlag,
        started: Instant,
        load_ms: u64,
    ) {
        let rate = session.sample_rate();
        let mut written: u64 = 0;
        let mut first_audio_ms = None;
        let voice = job.voice.as_deref();
        let result = session.speak(&job.text, voice, job.speed, cancel, &mut |samples| {
            if samples.is_empty() {
                return;
            }
            first_audio_ms.get_or_insert_with(|| elapsed_ms(started));
            for piece in samples.chunks(MAX_AUDIO_SAMPLES) {
                send(
                    &self.output,
                    &TtsEvent::Audio {
                        job: job.job,
                        sample_rate: rate,
                        samples: piece.to_vec(),
                    },
                );
            }
            written += samples.len() as u64;
        });
        let event = match result {
            Ok(()) => TtsEvent::Done {
                job: job.job,
                stats: SpeakStats {
                    audio_ms: written * 1000 / u64::from(rate.max(1)),
                    ms: elapsed_ms(started),
                    load_ms,
                    first_audio_ms: first_audio_ms.unwrap_or(0),
                },
            },
            Err(JobError::Cancelled) => TtsEvent::Cancelled { job: job.job },
            Err(JobError::Failed { code, detail }) => TtsEvent::error(Some(job.job), code, detail),
        };
        self.finish(job.job, &event);
    }

    /// Marks the job as running. `None` when a cancel already settled it.
    fn start(&self, job: u64) -> Option<CancelFlag> {
        let mut jobs = lock(&self.jobs);
        let entry = jobs.get_mut(&job)?;
        entry.started = true;
        Some(entry.cancel.clone())
    }

    fn finish(&self, job: u64, event: &TtsEvent) {
        if lock(&self.jobs).remove(&job).is_some() {
            send(&self.output, event);
        }
    }
}

fn elapsed_ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn send(output: &Output, event: &TtsEvent) {
    // A broken pipe means the app is gone; the reader sees the same and ends the loop.
    let _ = write_event(&mut *lock(output), event);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
