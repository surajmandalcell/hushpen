//! The audio side of [`super::Mic`], behind a trait so tests never open a
//! real microphone (on macOS that shows a permission prompt).

use hushpen_audio::{Capture, CaptureError, EventSink, FeedInfo, Finished, InputDevice};
use std::path::{Path, PathBuf};

pub trait MicBackend: Send + Sync + 'static {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError>;
    /// `device` is `"default"` or an id from `list`.
    fn start(
        &self,
        device: &str,
        path: PathBuf,
        sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError>;
}

pub trait MicSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError>;
    fn feed_wav(&self, path: &Path) -> Result<FeedInfo, CaptureError>;
}

/// The real microphones, through cpal.
pub struct CpalBackend;

impl MicBackend for CpalBackend {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError> {
        hushpen_audio::list_inputs()
    }

    fn start(
        &self,
        device: &str,
        path: PathBuf,
        sink: EventSink,
    ) -> Result<Box<dyn MicSession>, CaptureError> {
        Ok(Box::new(CpalSession(Capture::start(device, path, sink)?)))
    }
}

struct CpalSession(Capture);

impl MicSession for CpalSession {
    fn stop(self: Box<Self>) -> Result<Finished, CaptureError> {
        self.0.stop()
    }

    fn feed_wav(&self, path: &Path) -> Result<FeedInfo, CaptureError> {
        self.0.feeder().feed_wav(path)
    }
}
