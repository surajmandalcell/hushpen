//! What the worker needs from a voice model, apart from sherpa-onnx.

pub use hushpen_core::cancel::CancelFlag;
use std::path::PathBuf;
use std::time::Duration;

/// A model to hold. The server keeps it after an idle unload and loads it again for the next job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadSpec {
    /// The model's folder, such as `models/tts/kokoro-82m-int8`.
    pub dir: PathBuf,
    pub threads: usize,
    /// `None` keeps the model until the app unloads it.
    pub idle: Option<Duration>,
}

impl LoadSpec {
    /// The name shown in logs: the folder name.
    pub fn model_name(&self) -> String {
        self.dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    Cancelled,
    Failed { code: &'static str, detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    pub detail: String,
}

/// A loaded voice model. It speaks one job at a time.
pub trait Session: Send {
    fn sample_rate(&self) -> u32;

    /// Speaks `text` as the model's speaker number `speaker`. `sink` gets the audio piece by
    /// piece while it is made.
    fn speak(
        &mut self,
        text: &str,
        speaker: i32,
        speed: f32,
        cancel: &CancelFlag,
        sink: &mut dyn FnMut(&[f32]),
    ) -> Result<(), JobError>;
}

pub trait Backend: Send + Sync {
    fn load(&self, spec: &LoadSpec) -> Result<Box<dyn Session>, LoadError>;
}
