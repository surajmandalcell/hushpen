//! The system Reduce Motion setting, where the desktop states one.
//!
//! macOS asks AppKit (Accessibility > Display > Reduce motion). On Linux the answer comes from
//! the GNOME setting `org.gnome.desktop.interface enable-animations`, where false means reduce
//! motion. `None` means the desktop did not say, and the app then plays its motion.

/// Whether the user asked to reduce motion, or `None` when the system does not say. The answer
/// is read each time, because the user can change it while the app runs. On Linux it waits half
/// a second at most for `gsettings`.
pub fn system_reduce_motion() -> Option<bool> {
    imp::system_reduce_motion()
}

/// The value `gsettings get ... enable-animations` prints: `true` or `false`. Animations off
/// means reduce motion.
pub fn reduce_motion_of_animations(output: &str) -> Option<bool> {
    match output.trim() {
        "true" => Some(false),
        "false" => Some(true),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::NSWorkspace;

    pub(super) fn system_reduce_motion() -> Option<bool> {
        Some(NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion())
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::reduce_motion_of_animations;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;

    const WAIT: Duration = Duration::from_millis(500);

    pub(super) fn system_reduce_motion() -> Option<bool> {
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(read());
        });
        receive.recv_timeout(WAIT).ok().flatten()
    }

    fn read() -> Option<bool> {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        reduce_motion_of_animations(&String::from_utf8_lossy(&output.stdout))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    pub(super) fn system_reduce_motion() -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animations_off_means_reduce_motion() {
        assert_eq!(reduce_motion_of_animations("false\n"), Some(true));
        assert_eq!(reduce_motion_of_animations("true\n"), Some(false));
    }

    #[test]
    fn anything_else_is_unknown() {
        assert_eq!(reduce_motion_of_animations(""), None);
        assert_eq!(reduce_motion_of_animations("uint32 1"), None);
    }
}
