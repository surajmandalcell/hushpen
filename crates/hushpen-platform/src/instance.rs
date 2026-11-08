//! Single instance per data folder, with a current-user show channel.

#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(unix)]
pub use unix::{Guard, Instance, Start, socket_path, start};
#[cfg(target_os = "windows")]
pub use windows::{Guard, Instance, Start, socket_path, start};
