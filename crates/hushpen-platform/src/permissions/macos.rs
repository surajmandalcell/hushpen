//! macOS permission state. Every call here reads; none can show a prompt.

// A message to a framework class that the objc2 bindings of this workspace do not wrap.
#![allow(unsafe_code)]

use hushpen_core::permission::{Access, Preflight};
use objc2::msg_send;
use objc2::runtime::AnyClass;
use objc2_core_graphics::{CGPreflightListenEventAccess, CGPreflightPostEventAccess};
use objc2_foundation::NSString;

/// `AVMediaTypeAudio`.
const AUDIO_MEDIA_TYPE: &str = "soun";

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

pub(super) struct MacPreflight;

impl Preflight for MacPreflight {
    fn microphone(&self) -> Access {
        microphone_access()
    }

    fn post_event(&self) -> Access {
        granted(CGPreflightPostEventAccess())
    }

    fn listen_event(&self) -> Access {
        granted(CGPreflightListenEventAccess())
    }
}

fn granted(yes: bool) -> Access {
    if yes { Access::Granted } else { Access::Denied }
}

/// The `AVAuthorizationStatus` values: 0 not determined, 1 restricted, 2 denied, 3 authorized.
fn microphone_status(status: isize) -> Access {
    match status {
        3 => Access::Granted,
        0 => Access::NotDetermined,
        _ => Access::Denied,
    }
}

fn microphone_access() -> Access {
    let Some(class) = AnyClass::get(c"AVCaptureDevice") else {
        return Access::Denied;
    };
    let media = NSString::from_str(AUDIO_MEDIA_TYPE);
    // SAFETY: `authorizationStatusForMediaType:` takes an NSString and returns an NSInteger. It
    // reads the status and does not prompt.
    let status: isize = unsafe { msg_send![class, authorizationStatusForMediaType: &*media] };
    microphone_status(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_av_statuses_map_to_the_states() {
        assert_eq!(microphone_status(0), Access::NotDetermined);
        assert_eq!(microphone_status(1), Access::Denied);
        assert_eq!(microphone_status(2), Access::Denied);
        assert_eq!(microphone_status(3), Access::Granted);
    }

    #[test]
    fn the_real_preflight_answers_with_a_state_and_no_prompt() {
        let preflight = MacPreflight;
        for access in [
            preflight.microphone(),
            preflight.post_event(),
            preflight.listen_event(),
        ] {
            assert_ne!(access, Access::NotApplicable);
        }
    }
}
