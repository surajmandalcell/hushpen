//! Read aloud: text preparation, the speech engine seam, and the system voice of each OS.
//!
//! An engine speaks one chunk of plain text at a time. `synthesize` splits the text into
//! chunks, runs the engine on a thread of its own, resamples every chunk to `SAMPLE_RATE`, and
//! streams the audio with progress, so every engine gets the same streaming and cancel.

#[cfg(any(target_os = "linux", test))]
mod espeak;
#[cfg(target_os = "macos")]
mod macos;
mod synthesis;
pub mod text;

pub use synthesis::{SpeechEvent, Synthesis, synthesize};
pub use text::{MAX_TEXT_BYTES, TextFormat, prepare, read_text_file};

use hushpen_core::error::SPEECH_FAILED;
use std::fmt;

/// The rate of all synthesized audio. Kokoro speaks at this rate, so its audio needs no
/// resampling.
pub const SAMPLE_RATE: u32 = 24_000;
/// The speed range, as a factor of the voice's normal speed.
pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoiceQuality {
    Default,
    Enhanced,
    Premium,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// What `SpeechEngine::speak` takes. Stable across launches.
    pub id: String,
    pub name: String,
    /// A BCP 47 tag such as `en-US`, or a bare language such as `en`.
    pub language: String,
    pub quality: VoiceQuality,
}

/// Mono audio as an engine speaks it, at the engine's own rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechError {
    /// A `SPEECH_*` code from `hushpen_core::error`.
    pub code: &'static str,
    /// For the log. For `SPEECH_UNAVAILABLE` it names the fix, such as the package to install.
    pub detail: String,
}

impl SpeechError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn failed(detail: impl Into<String>) -> Self {
        Self::new(SPEECH_FAILED, detail)
    }
}

impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.detail)
    }
}

impl std::error::Error for SpeechError {}

/// One way to turn text into speech: the system voice here, Kokoro in its own child.
pub trait SpeechEngine: Send + Sync {
    /// The installed voices, or `SPEECH_UNAVAILABLE` when this engine cannot run here.
    fn voices(&self) -> Result<Vec<Voice>, SpeechError>;

    /// Speaks one chunk of plain text and blocks until its audio is complete. `voice` is a
    /// `Voice::id`, or `None` for the default voice; `speed` is within `MIN_SPEED..=MAX_SPEED`.
    fn speak(&self, text: &str, voice: Option<&str>, speed: f32) -> Result<Pcm, SpeechError>;
}

/// The system voice of this OS: AVSpeechSynthesizer on macOS, espeak-ng on Linux.
pub fn system_engine() -> Box<dyn SpeechEngine> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::SystemVoice);
    #[cfg(target_os = "linux")]
    return Box::new(espeak::Espeak::default());
}
