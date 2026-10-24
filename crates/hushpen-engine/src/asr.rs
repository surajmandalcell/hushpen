//! The speech recognition seam: a trait the engine child and the tests share.

use std::fmt;

pub use hushpen_core::cancel::CancelFlag;
use hushpen_core::error;
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
    /// Write the transcript in English, whatever language is spoken. English-only models
    /// cannot do this.
    pub translate: bool,
}

/// 16 kHz mono samples that arrive in pieces, such as a file that is decoded while it is
/// transcribed.
pub trait PcmSource {
    /// Appends the next samples to `out`; a call can add none. Returns `Ok(false)` once there
    /// are no more.
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, EngineError>;

    /// How much of the input has been read, from 0 to 1.
    fn fraction_read(&self) -> f32;
}

/// A whole buffer, such as a dictation, given to the engine in one read.
pub struct SliceSource<'a> {
    pcm: &'a [f32],
    read: bool,
}

impl<'a> SliceSource<'a> {
    pub fn new(pcm: &'a [f32]) -> Self {
        Self { pcm, read: false }
    }
}

impl PcmSource for SliceSource<'_> {
    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool, EngineError> {
        if !self.read {
            self.read = true;
            out.extend_from_slice(self.pcm);
        }
        Ok(false)
    }

    fn fraction_read(&self) -> f32 {
        if self.read { 1.0 } else { 0.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    /// The model file could not be opened or is not a whisper model.
    LoadFailed(String),
    /// The CPU lacks an instruction set the build needs, for example `AVX2`. This is a load
    /// failure whose reason is `cpu`, so the UI can say the processor is not supported.
    UnsupportedCpu(&'static str),
    /// The job was stopped through its [`CancelFlag`].
    Cancelled,
    /// The language code is not one whisper knows.
    BadLanguage(String),
    /// The audio has no samples.
    NoAudio,
    /// Translate was asked of an English-only model.
    TranslateUnsupported,
    /// The imported file could not be decoded while it was transcribed. `code` is an `IMPORT_`
    /// code and `text` a sentence for the user.
    Decode { code: &'static str, text: String },
    /// whisper failed while running the job.
    Failed(String),
}

impl EngineError {
    /// The stable code the UI matches on. A cancel is not an error and has none.
    pub fn code(&self) -> Option<&'static str> {
        match self {
            Self::LoadFailed(_) | Self::UnsupportedCpu(_) => Some("ENGINE_LOAD_FAILED"),
            Self::Cancelled => None,
            Self::BadLanguage(_) => Some("ENGINE_BAD_LANGUAGE"),
            Self::NoAudio => Some("ENGINE_NO_SPEECH"),
            Self::TranslateUnsupported => Some(error::ENGINE_TRANSLATE_UNSUPPORTED),
            Self::Decode { code, .. } => Some(code),
            Self::Failed(_) => Some("ENGINE_CRASHED"),
        }
    }

    /// A stable word for why the error happened, when the code alone is not enough.
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::UnsupportedCpu(_) => Some(hushpen_core::protocol::REASON_CPU),
            _ => None,
        }
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoadFailed(detail) => write!(f, "model load failed: {detail}"),
            Self::UnsupportedCpu(feature) => {
                write!(f, "this CPU lacks {feature}, which the speech engine needs")
            }
            Self::Cancelled => f.write_str("job cancelled"),
            Self::BadLanguage(code) => write!(f, "unknown language code: {code}"),
            Self::NoAudio => f.write_str("no audio samples"),
            Self::TranslateUnsupported => {
                f.write_str("this model knows only English, so it cannot translate")
            }
            Self::Decode { text, .. } => f.write_str(text),
            Self::Failed(detail) => write!(f, "transcription failed: {detail}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// A speech recognizer that holds one loaded model.
pub trait AsrEngine {
    /// True when the model runs on the GPU.
    fn gpu_in_use(&self) -> bool;

    /// Transcribe 16 kHz mono `f32` samples as `source` gives them. Setting `cancel` makes this
    /// return [`EngineError::Cancelled`] within milliseconds. `progress` hears how far the job
    /// is, from 0 to 1, each time a window of the input is done.
    fn transcribe_stream(
        &mut self,
        source: &mut dyn PcmSource,
        options: &TranscribeOptions,
        cancel: &CancelFlag,
        progress: &mut dyn FnMut(f32),
    ) -> Result<Transcript, EngineError>;

    /// [`AsrEngine::transcribe_stream`] over one buffer, such as a dictation.
    fn transcribe(
        &mut self,
        pcm: &[f32],
        options: &TranscribeOptions,
        cancel: &CancelFlag,
    ) -> Result<Transcript, EngineError> {
        self.transcribe_stream(&mut SliceSource::new(pcm), options, cancel, &mut |_| {})
    }
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
