//! Speech engine child and the generic child process supervisor.

mod asr;
mod cpu;
mod windows;
// Calls the whisper abort callback through the raw FFI API (see the module comment).
#[allow(unsafe_code)]
mod whisper;

pub use asr::{
    AsrEngine, CancelFlag, EngineError, LoadOptions, Segment, TranscribeOptions, Transcript,
};
pub use cpu::missing_cpu_feature;
pub use whisper::{WhisperEngine, whisper_cpp_version};
