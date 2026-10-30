//! The app side of the `hushpen-tts` child: [`TtsClient`] speaks text with a voice model
//! (Kokoro or Chatterbox) on the worker. A job streams its audio as [`Reply::Progress`] and ends in one outcome,
//! whatever the child does. The supervision itself is shared with the LLM worker (see `worker`).
//!
//! [`Reply::Progress`]: crate::worker::Reply::Progress

use crate::worker::{
    JobHandle, LoadHandle, Loaded, Outcome, Protocol, Reply, Signal, WorkerClient, WorkerState,
    WorkerStatus,
};
use hushpen_core::error;
use hushpen_core::tts_protocol::{
    DEFAULT_IDLE_SECS, ReadError, SpeakStats, TTS_PROTOCOL_VERSION, TtsEngine, TtsEvent,
    TtsRequest, read_event,
};
use serde_json::Value;
use std::path::PathBuf;
use std::process::ChildStdout;

/// The voice model to keep ready: the folder that holds its files and the code that runs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsSpec {
    pub dir: PathBuf,
    pub engine: TtsEngine,
    pub threads: usize,
    /// Seconds without a job before the worker drops the model. `0` keeps it.
    pub idle_secs: u64,
}

impl TtsSpec {
    pub fn new(engine: TtsEngine, dir: PathBuf) -> Self {
        Self {
            dir,
            engine,
            threads: 4,
            idle_secs: DEFAULT_IDLE_SECS,
        }
    }
}

/// One text to speak. The worker cuts it into sentences and streams each one's audio.
#[derive(Debug, Clone, PartialEq)]
pub struct TtsWork {
    pub text: String,
    /// A Kokoro voice id such as `af_heart` or the path of a Chatterbox voice clip, `None` for
    /// the default voice.
    pub voice: Option<String>,
    pub speed: f32,
}

/// The next piece of a job's audio: mono samples at `sample_rate`.
#[derive(Debug, Clone, PartialEq)]
pub struct TtsAudio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

/// The messages of `hushpen-tts`.
pub struct TtsWorker;

pub type TtsClient = WorkerClient<TtsWorker>;
pub type TtsState = WorkerState;
pub type TtsStatus = WorkerStatus;
pub type TtsLoaded = Loaded;
/// `TTS_CRASHED` means the child died while the job ran; `TTS_UNAVAILABLE` means it was not up
/// in time.
pub type TtsOutcome = Outcome<SpeakStats>;
pub type TtsReply = Reply<TtsWorker>;
pub type TtsJobHandle = JobHandle<TtsWorker>;
pub type TtsLoadHandle = LoadHandle<TtsWorker>;

/// The worker runs the model on the CPU only (see D-27).
const BACKEND: &str = "cpu";

impl Protocol for TtsWorker {
    const NAME: &'static str = "tts";
    const PROTOCOL_VERSION: u32 = TTS_PROTOCOL_VERSION;
    const UNAVAILABLE: &'static str = error::TTS_UNAVAILABLE;
    const CRASHED: &'static str = error::TTS_CRASHED;
    const LOAD_FAILED: &'static str = error::TTS_LOAD_FAILED;

    type Spec = TtsSpec;
    type Work = TtsWork;
    type Done = SpeakStats;
    type Progress = TtsAudio;
    type Event = TtsEvent;

    fn hello() -> Value {
        TtsRequest::Hello {
            protocol: TTS_PROTOCOL_VERSION,
        }
        .to_json()
    }

    fn ping() -> Value {
        TtsRequest::Ping.to_json()
    }

    fn unload() -> Value {
        TtsRequest::Unload.to_json()
    }

    fn shutdown() -> Value {
        TtsRequest::Shutdown.to_json()
    }

    fn cancel(job: u64) -> Value {
        TtsRequest::Cancel { job }.to_json()
    }

    fn load(spec: &TtsSpec) -> Value {
        TtsRequest::LoadModel {
            dir: spec.dir.to_string_lossy().into_owned(),
            engine: spec.engine,
            threads: u32::try_from(spec.threads).unwrap_or(u32::MAX),
            idle_secs: spec.idle_secs,
        }
        .to_json()
    }

    fn job(job: u64, work: &TtsWork) -> Value {
        TtsRequest::Speak {
            job,
            text: work.text.clone(),
            voice: work.voice.clone(),
            speed: work.speed,
        }
        .to_json()
    }

    fn model_name(spec: &TtsSpec) -> String {
        spec.dir.file_name().map_or_else(
            || spec.dir.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    fn kind(_: &TtsWork) -> &'static str {
        "speak"
    }

    fn read(stdout: &mut ChildStdout) -> Result<Option<TtsEvent>, String> {
        read_event(stdout).map_err(|problem| match problem {
            ReadError::Frame(frame) => frame.to_string(),
            ReadError::Decode(decode) => format!("unreadable message: {decode}"),
        })
    }

    fn signal(event: TtsEvent) -> Signal<Self> {
        match event {
            TtsEvent::Ready { version, protocol } => Signal::Ready {
                protocol,
                version,
                backend: BACKEND.into(),
            },
            TtsEvent::Pong => Signal::Pong,
            TtsEvent::Loading { .. } | TtsEvent::Voices { .. } => Signal::Other,
            TtsEvent::Loaded { model, ms } => Signal::Loaded(Loaded {
                model,
                ms,
                gpu: false,
                backend: BACKEND.into(),
            }),
            TtsEvent::Unloaded { reason } => Signal::Unloaded(reason),
            TtsEvent::Audio {
                job,
                sample_rate,
                samples,
            } => Signal::Progress {
                job,
                progress: TtsAudio {
                    sample_rate,
                    samples,
                },
            },
            TtsEvent::Done { job, stats } => Signal::Done {
                job,
                counts: format!(
                    "audio_ms={} first_audio_ms={} load_ms={} ms={}",
                    stats.audio_ms, stats.first_audio_ms, stats.load_ms, stats.ms
                ),
                done: stats,
            },
            TtsEvent::Cancelled { job } => Signal::Cancelled { job },
            TtsEvent::Error { job, code, params } => Signal::Error { job, code, params },
        }
    }
}
