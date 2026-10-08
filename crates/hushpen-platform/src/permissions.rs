//! The permissions of this system, read without ever asking.
//!
//! On macOS the three answers come from the preflight calls of Microphone, Post Event, and Listen
//! Event access. Request calls and System Settings links belong to onboarding and run only after
//! a click.

use hushpen_core::permission::{Permission, Preflight};
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

/// Asks macOS to list Hushpen for the Accessibility or Input Monitoring grant, which shows the
/// system prompt once. Microphone is asked by opening the stream, which the app does. Linux has
/// nothing to ask. Call this only after a click.
pub fn request(permission: Permission) {
    #[cfg(target_os = "macos")]
    macos::request(permission);
    #[cfg(not(target_os = "macos"))]
    let _ = permission;
}

/// Opens a System Settings link (`Permission::settings_url`) on macOS. Linux has no such
/// settings. Call this only after a click.
pub fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open_url(url);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        Ok(())
    }
}
