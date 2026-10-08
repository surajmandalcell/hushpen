//! Platform layer, with macOS and Linux modules behind shared traits.

pub mod insert;
pub mod keys;
pub mod permissions;
pub mod tray_host;
pub mod window;

#[cfg(target_os = "linux")]
mod x11util;
