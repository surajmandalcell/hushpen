//! The phone API shared by the native Android and iOS apps.

mod dictation;
mod engine;
mod host;
mod models;
mod system_llm;

pub use dictation::{Dictation, DictationEffect, DictationEvent, DictationSnapshot};
pub use engine::{Engine, Transcript};
pub use host::{Host, HostUnloadRule};
pub use models::{DownloadProgress, Model, ModelKind, ModelRepository, ModelStatus};
pub use system_llm::{CleanupResult, SystemCleanup, SystemLlm, clean_text};

uniffi::setup_scaffolding!();

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum MobileError {
    InvalidInput { detail: String },
    Failed { detail: String },
}

impl MobileError {
    pub(crate) fn invalid(detail: impl std::fmt::Display) -> Self {
        Self::InvalidInput {
            detail: detail.to_string(),
        }
    }

    pub(crate) fn failed(detail: impl std::fmt::Display) -> Self {
        Self::Failed {
            detail: detail.to_string(),
        }
    }
}

impl std::fmt::Display for MobileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput { detail } | Self::Failed { detail } => detail.fmt(formatter),
        }
    }
}

impl std::error::Error for MobileError {}

impl From<uniffi::UnexpectedUniFFICallbackError> for MobileError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::failed(error)
    }
}

pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
