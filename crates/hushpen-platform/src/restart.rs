//! Restart after the current process exits.

use std::io;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
mod windows;

pub const HELPER_ARG: &str = "restart-after-exit";

/// Starts the Windows setup helper before the app releases its files.
pub fn apply_update(setup: &Path, target: &Path, log: &Path, show_window: bool) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    return windows::apply_update(setup, target, log, show_window);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (setup, target, log, show_window);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "The setup helper is Windows only.",
        ))
    }
}

/// Starts a detached helper before the app releases its files and instance lock.
pub fn after_exit(current: &Path, target: &Path, default: &Path) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    return windows::start(current, target, default);
    #[cfg(unix)]
    {
        let _ = (current, target, default);
        let exe = std::env::current_exe()?;
        let pid = std::process::id();
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; exec \"$0\"",
            ))
            .arg(exe)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(())
    }
}

/// Runs before the display, data folder, and single-instance lock are opened.
pub fn wait_for_exit(args: &[String]) -> io::Result<(PathBuf, PathBuf, PathBuf)> {
    #[cfg(target_os = "windows")]
    return windows::wait(args);
    #[cfg(not(target_os = "windows"))]
    {
        let _ = args;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "The restart helper is Windows only.",
        ))
    }
}

/// Starts the current executable after its old data handles have closed.
pub fn relaunch() -> io::Result<()> {
    #[cfg(target_os = "windows")]
    return windows::relaunch();
    #[cfg(not(target_os = "windows"))]
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "The restart helper is Windows only.",
    ))
}
