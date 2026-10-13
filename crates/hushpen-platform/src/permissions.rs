//! The permissions of this system, read live and without ever asking.
//!
//! On macOS the three answers come from `AXIsProcessTrusted`, `IOHIDCheckAccess`, and the
//! `AVCaptureDevice` status. The cached `CGPreflight*EventAccess` calls are not used, because
//! they would not show a grant made while Hushpen runs. Request calls and System Settings links
//! belong to onboarding and run only after a click.

use hushpen_core::permission::{Permission, Preflight};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(any(target_os = "macos", test))]
mod reader;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub fn system() -> Arc<dyn Preflight> {
    Arc::new(macos::preflight())
}

#[cfg(not(target_os = "macos"))]
pub fn system() -> Arc<dyn Preflight> {
    Arc::new(hushpen_core::permission::NotApplicable)
}

/// The live Accessibility grant, for the paste calls. Read it each time.
#[cfg(target_os = "macos")]
pub(crate) fn accessibility_trusted() -> bool {
    macos::accessibility_trusted()
}

/// The live Input Monitoring grant, for the key listener. Read it each time.
#[cfg(target_os = "macos")]
pub(crate) fn input_monitoring_granted() -> bool {
    macos::input_monitoring_granted()
}

/// Calls the request API of a macOS permission, which lists Hushpen in the Privacy pane and
/// shows the system prompt. It runs on every call, whatever the state is. Linux has nothing to
/// ask. Call this only after a click on Allow.
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

/// Opens a folder in the file manager (Finder on macOS). Linux has no
/// managed file manager call yet, so this is a no-op there. Call this only
/// after a click.
pub fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    return macos::open_path(path);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Ok(())
    }
}

/// Calls `callback` from the main thread each time the user brings Hushpen to the front
/// (`NSApplicationDidBecomeActiveNotification`). Linux has no such call yet, so it never runs.
pub fn on_app_activation(callback: Arc<dyn Fn() + Send + Sync>) {
    #[cfg(target_os = "macos")]
    macos::on_app_activation(callback);
    #[cfg(not(target_os = "macos"))]
    let _ = callback;
}

/// The `.app` folder that holds `exe`, if it runs from one.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}

/// The shell line that waits for process `pid` to end and then opens `bundle` as a new
/// instance. The wait matters: a new start that sees the old one alive only asks it to show
/// itself and exits.
pub fn relaunch_script(pid: u32) -> String {
    format!("while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; exec /usr/bin/open -n \"$0\"")
}

/// Hands Hushpen to a new copy of itself: `open -n` on the app bundle, started once this
/// process is gone. The caller quits the app after this returns. macOS only gives a new
/// Input Monitoring grant to a process that starts after the grant.
pub fn relaunch() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let bundle = bundle_of(&exe).ok_or_else(|| {
        std::io::Error::other("Hushpen is not running from an app bundle, so it cannot reopen")
    })?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(relaunch_script(std::process::id()))
            .arg(bundle)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        Err(std::io::Error::other("Quit and Reopen is for macOS"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn the_bundle_is_the_app_folder_above_the_executable() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Hushpen.app/Contents/MacOS/hushpen"
            )),
            Some(PathBuf::from("/Applications/Hushpen.app"))
        );
        assert_eq!(bundle_of(Path::new("/usr/local/bin/hushpen")), None);
    }

    #[test]
    fn the_relaunch_waits_for_the_old_process_and_then_opens_a_new_instance() {
        let script = relaunch_script(4242);
        assert!(script.contains("kill -0 4242"), "{script}");
        assert!(script.ends_with("exec /usr/bin/open -n \"$0\""), "{script}");
    }

    /// The cached calls answer from a snapshot taken at the first read, so a grant made while
    /// Hushpen runs would not show. The names are joined here so this file does not match itself.
    #[test]
    fn no_source_file_calls_the_cached_event_access_preflight() {
        let cached = [
            concat!("CGPre", "flightPostEventAccess"),
            concat!("CGPre", "flightListenEventAccess"),
        ];
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut found = Vec::new();
        let mut folders = vec![crates];
        while let Some(folder) = folders.pop() {
            for entry in fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                let name = entry.file_name();
                if path.is_dir() {
                    if name != "target" && name != "vendor" {
                        folders.push(path);
                    }
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let text = fs::read_to_string(&path).unwrap();
                    if cached.iter().any(|call| text.contains(call)) {
                        found.push(path);
                    }
                }
            }
        }
        assert!(found.is_empty(), "cached preflight calls in {found:?}");
    }
}
