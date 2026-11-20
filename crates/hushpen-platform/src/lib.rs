//! Platform layer, with macOS and Linux modules behind shared traits.

pub mod app_name;
pub mod appearance;
pub mod file_services;
pub mod insert;
pub mod instance;
pub mod keys;
pub mod local_time;
pub mod login_item;
pub mod memory;
pub mod permissions;
pub mod reduce_motion;
pub mod restart;
pub mod tray_host;
pub mod window;

#[cfg(target_os = "macos")]
mod keyboard_layout;
#[cfg(target_os = "windows")]
#[path = "keyboard_layout/windows.rs"]
mod keyboard_layout;
#[cfg(target_os = "linux")]
mod x11util;
