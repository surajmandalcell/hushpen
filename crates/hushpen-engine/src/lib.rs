//! Speech engine child and the supervisors the app uses to run its children.

mod asr;
mod backoff;
mod child;
mod client;
mod cpu;
mod engine_log;
mod entry;
mod llm_client;
mod manager;
// madvise on the model's own mappings (see the module comment).
#[allow(unsafe_code)]
mod pageout;
mod server;
mod tts_client;
mod vad;
mod wav;
mod windows;
// Calls the whisper abort callback through the raw FFI API (see the module comment).
#[allow(unsafe_code)]
mod whisper;
mod worker;
mod worker_manager;

pub use asr::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, Segment, TranscribeOptions, Transcript,
};
pub use client::{
    ChildSpec, EngineClient, EngineState, Failure, ImportProgress, ImportSpec, JobHandle,
    JobOutcome, LoadHandle, LoadSpec, Loaded, ProgressSink, Status, Timing, TranscribeSpec,
    Transcription,
};
pub use cpu::missing_cpu_feature;
pub use engine_log::{EngineLog, Level};
pub use entry::{engine_main, gpu_available, gpu_disabled_by_env};
pub use llm_client::{
    LlmClient, LlmDone, LlmJobHandle, LlmLoadHandle, LlmLoaded, LlmOutcome, LlmSpec, LlmState,
    LlmStatus, LlmWork, LlmWorker,
};
pub use server::model_name;
pub use tts_client::{
    TtsAudio, TtsClient, TtsJobHandle, TtsLoadHandle, TtsLoaded, TtsOutcome, TtsReply, TtsSpec,
    TtsState, TtsStatus, TtsWork, TtsWorker,
};
pub use whisper::{WhisperEngine, whisper_cpp_version};
