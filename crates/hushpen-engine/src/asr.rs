//! The speech recognition seam: a trait the engine child and the tests share.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use hushpen_core::protocol::Segment;

/// The full result of one job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    /// All segment texts joined, trimmed.
    pub text: String,
    /// Language code whisper used or detected, for example `en`.
    pub language: String,
    pub segments: Vec<Segment>,
}

/// How to load a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadOptions {
    /// Ask for the GPU. It is honored only where the build has a GPU backend (Metal on macOS).
    pub gpu: bool,
    pub threads: usize,
}

/// Per-job options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscribeOptions {
    /// A language code such as `en`; `None` detects the language (needs a multilingual model).
    pub language: Option<String>,
    /// Text that biases the decoder toward the user's words.
    pub prompt: Option<String>,
}

/// A flag that stops a running job. Clones share one flag.
#[derive(Debug, Clone, Default)]
pub struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub(crate) fn as_atomic(&self) -> &Arc<AtomicBool> {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The model file could not be opened or is not a whisper model.
    LoadFailed(String),
    /// The job was stopped through its [`CancelFlag`].
    Cancelled,
    /// The language code is not one whisper knows.
    BadLanguage(String),
    /// The audio has no samples.
    NoAudio,
    /// whisper failed while running the job.
    Failed(String),
}

impl EngineError {
    /// The stable code the UI matches on. A cancel is not an error and has none.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::LoadFailed(_) => Some("ENGINE_LOAD_FAILED"),
            Self::Cancelled => None,
            Self::BadLanguage(_) => Some("ENGINE_BAD_LANGUAGE"),
            Self::NoAudio => Some("ENGINE_NO_SPEECH"),
            Self::Failed(_) => Some("ENGINE_CRASHED"),
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed(detail) => write!(f, "model load failed: {detail}"),
            Self::Cancelled => f.write_str("job cancelled"),
            Self::BadLanguage(code) => write!(f, "unknown language code: {code}"),
            Self::NoAudio => f.write_str("no audio samples"),
            Self::Failed(detail) => write!(f, "transcription failed: {detail}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// A speech recognizer that holds one loaded model.
pub trait AsrEngine {
    /// True when the model runs on the GPU.
    fn gpu_in_use(&self) -> bool;

    /// Transcribe 16 kHz mono `f32` samples. Setting `cancel` makes this return
    /// [`EngineError::Cancelled`] within milliseconds.
    fn transcribe(
        &mut self,
        pcm: &[f32],
        options: &TranscribeOptions,
        cancel: &CancelFlag,
    ) -> Result<Transcript, EngineError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_flag_clones_share_state() {
        let flag = CancelFlag::new();
        let other = flag.clone();
        assert!(!other.is_cancelled());
        flag.cancel();
        assert!(other.is_cancelled());
    }

    #[test]
    fn error_codes_are_stable() {
        assert_eq!(EngineError::Cancelled.code(), None);
        assert_eq!(
            EngineError::LoadFailed("x".into()).code(),
            Some("ENGINE_LOAD_FAILED")
        );
        assert_eq!(
            EngineError::BadLanguage("xx".into()).code(),
            Some("ENGINE_BAD_LANGUAGE")
        );
    }
}
