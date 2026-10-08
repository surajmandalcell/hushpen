//! The permissions of this system, read without ever asking.
//!
//! On macOS the three answers come from the preflight calls of Microphone, Post Event, and Listen
//! Event access. Request calls and System Settings links belong to onboarding and run only after
//! a click.

use hushpen_core::permission::Preflight;
use std::sync::Arc;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub fn system() -> Arc<dyn Preflight> {
    Arc::new(macos::MacPreflight)
}

#[cfg(not(target_os = "macos"))]
pub fn system() -> Arc<dyn Preflight> {
    Arc::new(hushpen_core::permission::NotApplicable)
}
