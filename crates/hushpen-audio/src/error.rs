//! Capture failures. The `code` comes from `hushpen_core::error`; the UI maps
//! codes to messages and never matches the detail text.

use hushpen_core::error::{CAPTURE_FAILED, MIC_PERMISSION, MIC_UNAVAILABLE};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureError {
    pub code: &'static str,
    /// For the log and for developers. Never shown as the user message.
    pub detail: String,
}

impl CaptureError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn unavailable(detail: impl Into<String>) -> Self {
        Self::new(MIC_UNAVAILABLE, detail)
    }

    pub fn failed(detail: impl Into<String>) -> Self {
        Self::new(CAPTURE_FAILED, detail)
    }

    pub fn permission(detail: impl Into<String>) -> Self {
        Self::new(MIC_PERMISSION, detail)
    }
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.code, self.detail)
    }
}

impl std::error::Error for CaptureError {}

impl From<std::io::Error> for CaptureError {
    fn from(error: std::io::Error) -> Self {
        Self::failed(error.to_string())
    }
}

impl From<hound::Error> for CaptureError {
    fn from(error: hound::Error) -> Self {
        Self::failed(error.to_string())
    }
}

impl From<cpal::Error> for CaptureError {
    fn from(error: cpal::Error) -> Self {
        use cpal::ErrorKind::{DeviceNotAvailable, HostUnavailable, PermissionDenied};
        let detail = format!("{:?}: {error}", error.kind());
        match error.kind() {
            PermissionDenied => Self::permission(detail),
            DeviceNotAvailable | HostUnavailable => Self::unavailable(detail),
            _ => Self::failed(detail),
        }
    }
}
