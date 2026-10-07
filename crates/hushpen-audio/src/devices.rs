//! Input device list and the saved selection.

use crate::error::CaptureError;
use cpal::traits::{DeviceTrait, HostTrait};

/// The value of `audio.inputDeviceId` that follows the system default.
pub const DEFAULT_ID: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDevice {
    /// Stable across restarts; the value saved in `audio.inputDeviceId`.
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// What the saved setting means for the devices that exist now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    Default,
    Device(String),
    /// A specific device was saved and is not there now. Use the default and
    /// tell the user.
    Missing(String),
}

pub fn resolve_selection(saved: &str, devices: &[InputDevice]) -> Selection {
    if saved.is_empty() || saved == DEFAULT_ID {
        Selection::Default
    } else if devices.iter().any(|device| device.id == saved) {
        Selection::Device(saved.to_string())
    } else {
        Selection::Missing(saved.to_string())
    }
}

/// PulseAudio lists the monitor of every output as a source. They are not
/// microphones.
fn is_monitor(id: &str) -> bool {
    id.ends_with(".monitor")
}

/// Lists microphones. Opens no stream, so macOS shows no permission prompt.
pub fn list_inputs() -> Result<Vec<InputDevice>, CaptureError> {
    let host = cpal::default_host();
    inputs_of(&host)
}

fn inputs_of(host: &cpal::Host) -> Result<Vec<InputDevice>, CaptureError> {
    let default_id = host
        .default_input_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let mut devices: Vec<InputDevice> = Vec::new();
    for device in host.input_devices()? {
        let Ok(id) = device.id() else { continue };
        let id = id.to_string();
        if is_monitor(&id) || devices.iter().any(|known| known.id == id) {
            continue;
        }
        let name = device
            .description()
            .map(|description| description.name().to_string())
            .unwrap_or_else(|_| id.clone());
        let is_default = default_id.as_deref() == Some(id.as_str());
        devices.push(InputDevice {
            id,
            name,
            is_default,
        });
    }
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> InputDevice {
        InputDevice {
            id: id.into(),
            name: id.into(),
            is_default: false,
        }
    }

    #[test]
    fn default_and_empty_follow_the_system_default() {
        assert_eq!(resolve_selection("default", &[]), Selection::Default);
        assert_eq!(resolve_selection("", &[device("a")]), Selection::Default);
    }

    #[test]
    fn a_present_device_is_selected() {
        let devices = [
            device("pulseaudio:vmic_src"),
            device("pulseaudio:vmic2_src"),
        ];
        assert_eq!(
            resolve_selection("pulseaudio:vmic2_src", &devices),
            Selection::Device("pulseaudio:vmic2_src".into())
        );
    }

    #[test]
    fn a_device_that_is_gone_is_reported_missing() {
        assert_eq!(
            resolve_selection("pulseaudio:vmic2_src", &[device("pulseaudio:vmic_src")]),
            Selection::Missing("pulseaudio:vmic2_src".into())
        );
        assert_eq!(
            resolve_selection("coreaudio:x", &[]),
            Selection::Missing("coreaudio:x".into())
        );
    }

    #[test]
    fn monitors_are_not_microphones() {
        assert!(is_monitor("pulseaudio:vmic.monitor"));
        assert!(!is_monitor("pulseaudio:vmic_src"));
    }
}
