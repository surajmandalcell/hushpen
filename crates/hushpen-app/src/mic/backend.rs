//! The audio side of [`super::Mic`], behind a trait so tests never open a
//! real microphone (on macOS that shows a permission prompt).

use hushpen_audio::{Capture, CaptureError, EventSink, FeedInfo, Finished, InputDevice};
use hushpen_core::permission::Access;
use std::path::{Path, PathBuf};

pub trait MicBackend: Send + Sync + 'static {
    fn list(&self) -> Result<Vec<InputDevice>, CaptureError>;
    /// Gets the microphone ready so the next `start` begins at once. Returns at once.
    fn warm(&self, _device: &str) {}
    /// `device` is `"default"` or an id from `list`. Returns at once: a microphone that does
    /// not open ends in [`hushpen_audio::CaptureEvent::StartFailed`] on the sink.
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

    fn warm(&self, device: &str) {
        // Opening a stream is what makes macOS ask for the microphone, and that question
        // belongs to onboarding, never to start-up.
        let allowed = hushpen_platform::permissions::system().microphone();
        if matches!(allowed, Access::Granted | Access::NotApplicable) {
            hushpen_audio::warm_microphone(device);
        }
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
