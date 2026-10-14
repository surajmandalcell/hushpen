//! The live macOS permission calls. Reads never prompt; `request`, `open_url`, and
//! `open_path` run only after a click on an onboarding button.

// Calls into frameworks that the objc2 bindings of this workspace do not wrap.
#![allow(unsafe_code)]

use super::reader::{Api, Reader};
use block2::RcBlock;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use hushpen_core::permission::{Permission, Preflight};
use objc2::msg_send;
use objc2::runtime::{AnyClass, Bool};
use objc2_core_graphics::CGRequestListenEventAccess;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSString};
use std::ptr::NonNull;
use std::sync::Arc;

/// `AVMediaTypeAudio`.
const AUDIO_MEDIA_TYPE: &str = "soun";

/// `kIOHIDRequestTypeListenEvent`.
const HID_REQUEST_LISTEN_EVENT: u32 = 1;
/// `kIOHIDAccessTypeUnknown`: macOS has not been asked.
const HID_ACCESS_UNKNOWN: u32 = 2;

const APP_ACTIVE: &str = "NSApplicationDidBecomeActiveNotification";

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    safe fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    safe fn IOHIDCheckAccess(request_type: u32) -> u32;
    safe fn IOHIDRequestAccess(request_type: u32) -> bool;
}

pub(super) struct Live;

pub(super) type MacPreflight = Reader<Live>;

pub(super) fn preflight() -> MacPreflight {
    Reader(Live)
}

impl Api for Live {
    fn accessibility_trusted(&self) -> bool {
        accessibility_trusted()
    }

    fn listen_event_status(&self) -> u32 {
        IOHIDCheckAccess(HID_REQUEST_LISTEN_EVENT)
    }

    fn microphone_status(&self) -> isize {
        let Some(class) = AnyClass::get(c"AVCaptureDevice") else {
            return 2;
        };
        let media = NSString::from_str(AUDIO_MEDIA_TYPE);
        // SAFETY: `authorizationStatusForMediaType:` takes an NSString and returns an
        // NSInteger. It reads the status and does not prompt.
        unsafe { msg_send![class, authorizationStatusForMediaType: &*media] }
    }

    fn prompt_accessibility(&self) {
        // SAFETY: the key is a constant CFString that the framework owns; the get rule keeps
        // it alive without taking ownership.
        let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
        let options = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        // SAFETY: `options` is a valid CFDictionary for the whole call. The answer is read
        // later by the live preflight.
        let _ = unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
    }

    fn request_listen_event(&self) {
        let asked = IOHIDRequestAccess(HID_REQUEST_LISTEN_EVENT);
        if !asked && IOHIDCheckAccess(HID_REQUEST_LISTEN_EVENT) == HID_ACCESS_UNKNOWN {
            CGRequestListenEventAccess();
        }
    }

    fn request_microphone(&self) {
        let Some(class) = AnyClass::get(c"AVCaptureDevice") else {
            return;
        };
        let media = NSString::from_str(AUDIO_MEDIA_TYPE);
        // The answer is read later by the live preflight.
        let handler = RcBlock::new(|_granted: Bool| {});
        // SAFETY: `requestAccessForMediaType:completionHandler:` takes an NSString and a block
        // that gets a BOOL. AVFoundation copies the block.
        unsafe {
            let _: () = msg_send![
                class,
                requestAccessForMediaType: &*media,
                completionHandler: &*handler
            ];
        }
    }
}

/// `AXIsProcessTrusted`: read live, not cached per process.
pub(super) fn accessibility_trusted() -> bool {
    AXIsProcessTrusted() != 0
}

/// `IOHIDCheckAccess` for listen events: read live, not cached per process.
pub(super) fn input_monitoring_granted() -> bool {
    MacPreflight::listen_event(&preflight()) == hushpen_core::permission::Access::Granted
}

pub(super) fn request(permission: Permission) {
    preflight().request(permission);
}

pub(super) fn open_url(url: &str) -> std::io::Result<()> {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg(url);
    super::spawn_and_reap(command)
}

pub(super) fn open_path(path: &std::path::Path) -> std::io::Result<()> {
    let mut command = std::process::Command::new("/usr/bin/open");
    command.arg(path);
    super::spawn_and_reap(command)
}

/// Calls `callback` each time the user brings Hushpen to the front, on the main thread.
pub(super) fn on_app_activation(callback: Arc<dyn Fn() + Send + Sync>) {
    let name = NSString::from_str(APP_ACTIVE);
    let block = RcBlock::new(move |_: NonNull<NSNotification>| callback());
    // SAFETY: a nil object and a nil queue are allowed; the block runs on the posting thread.
    // The observer lives as long as the process, so its token is kept on purpose.
    let token = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(&name),
            None,
            None,
            &block,
        )
    };
    std::mem::forget(token);
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::permission::Access;

    #[test]
    fn the_real_preflight_answers_with_a_state_and_no_prompt() {
        let preflight = preflight();
        for access in [
            preflight.microphone(),
            preflight.post_event(),
            preflight.listen_event(),
        ] {
            assert_ne!(access, Access::NotApplicable);
        }
    }
}
