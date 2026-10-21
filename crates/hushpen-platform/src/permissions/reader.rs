//! What the macOS permission calls answer, and how each answer maps to a state.
//!
//! The calls sit behind [`Api`] so tests can swap in a mock that records which one ran. Every
//! read uses a live call. The `CGPreflight*EventAccess` calls answer from a per-process cache,
//! so a grant made in System Settings would not show until a restart.
//! A read never prompts and never resets. Only [`Reader::request`] asks macOS, and only after a
//! click. When the live read says the permission is not granted, it first resets Hushpen's own
//! record, because macOS ignores a request from an app it holds a denied or stale record for.

use hushpen_core::permission::{Access, Permission, Preflight};

/// `IOHIDAccessType`: granted, denied, or not asked yet.
const HID_GRANTED: u32 = 0;
const HID_UNKNOWN: u32 = 2;

/// The `AVAuthorizationStatus` values: 0 not determined, 1 restricted, 2 denied, 3 authorized.
const AV_NOT_DETERMINED: isize = 0;
const AV_AUTHORIZED: isize = 3;

pub(super) trait Api: Send + Sync {
    /// `AXIsProcessTrusted`.
    fn accessibility_trusted(&self) -> bool;
    /// `IOHIDCheckAccess` for listen events: the raw `IOHIDAccessType`.
    fn listen_event_status(&self) -> u32;
    /// `AVCaptureDevice authorizationStatusForMediaType:` for audio.
    fn microphone_status(&self) -> isize;
    /// `AXIsProcessTrustedWithOptions` with the prompt option.
    fn prompt_accessibility(&self);
    /// `IOHIDRequestAccess` for listen events, with `CGRequestListenEventAccess` as the fallback.
    fn request_listen_event(&self);
    /// `AVCaptureDevice requestAccessForMediaType:completionHandler:` for audio.
    fn request_microphone(&self);
    /// `tccutil reset <service> <own bundle id>`, so macOS treats Hushpen as never asked.
    fn reset(&self, permission: Permission);
}

/// The `tccutil` service name of `permission`.
pub(super) fn tcc_service(permission: Permission) -> &'static str {
    match permission {
        Permission::Accessibility => "Accessibility",
        Permission::InputMonitoring => "ListenEvent",
        Permission::Microphone => "Microphone",
    }
}

pub(super) struct Reader<A>(pub(super) A);

impl<A: Api> Preflight for Reader<A> {
    fn microphone(&self) -> Access {
        microphone_access(self.0.microphone_status())
    }

    fn post_event(&self) -> Access {
        if self.0.accessibility_trusted() {
            Access::Granted
        } else {
            Access::Denied
        }
    }

    fn listen_event(&self) -> Access {
        listen_event_access(self.0.listen_event_status())
    }
}

impl<A: Api> Reader<A> {
    /// Calls the request API of `permission` once, whatever the state is. The request is what
    /// makes macOS list Hushpen in the Privacy pane and show its prompt. When the live read says
    /// the permission is not granted, Hushpen's own record is reset first, so a denied or stale
    /// record cannot swallow the request. Call it only after a click on Allow.
    pub(super) fn request(&self, permission: Permission) {
        let access = match permission {
            Permission::Accessibility => self.post_event(),
            Permission::InputMonitoring => self.listen_event(),
            Permission::Microphone => self.microphone(),
        };
        if access != Access::Granted {
            self.0.reset(permission);
        }
        match permission {
            Permission::Accessibility => self.0.prompt_accessibility(),
            Permission::InputMonitoring => self.0.request_listen_event(),
            Permission::Microphone => self.0.request_microphone(),
        }
    }
}

fn microphone_access(status: isize) -> Access {
    match status {
        AV_AUTHORIZED => Access::Granted,
        AV_NOT_DETERMINED => Access::NotDetermined,
        _ => Access::Denied,
    }
}

fn listen_event_access(status: u32) -> Access {
    match status {
        HID_GRANTED => Access::Granted,
        HID_UNKNOWN => Access::NotDetermined,
        _ => Access::Denied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Answers like macOS and records every call, reads and requests alike.
    struct Mock {
        trusted: bool,
        listen: u32,
        microphone: isize,
        calls: Mutex<Vec<&'static str>>,
    }

    impl Mock {
        fn new(trusted: bool, listen: u32, microphone: isize) -> Reader<Self> {
            Reader(Self {
                trusted,
                listen,
                microphone,
                calls: Mutex::default(),
            })
        }
    }

    impl Reader<Mock> {
        fn calls(&self) -> Vec<&'static str> {
            self.0.calls.lock().unwrap().clone()
        }
    }

    impl Api for Mock {
        fn accessibility_trusted(&self) -> bool {
            self.calls.lock().unwrap().push("AXIsProcessTrusted");
            self.trusted
        }

        fn listen_event_status(&self) -> u32 {
            self.calls.lock().unwrap().push("IOHIDCheckAccess");
            self.listen
        }

        fn microphone_status(&self) -> isize {
            self.calls
                .lock()
                .unwrap()
                .push("AVCaptureDevice authorizationStatus");
            self.microphone
        }

        fn prompt_accessibility(&self) {
            self.calls
                .lock()
                .unwrap()
                .push("AXIsProcessTrustedWithOptions prompt");
        }

        fn request_listen_event(&self) {
            self.calls.lock().unwrap().push("IOHIDRequestAccess");
        }

        fn request_microphone(&self) {
            self.calls
                .lock()
                .unwrap()
                .push("AVCaptureDevice requestAccess");
        }

        fn reset(&self, permission: Permission) {
            let call = match permission {
                Permission::Accessibility => "tccutil reset Accessibility",
                Permission::InputMonitoring => "tccutil reset ListenEvent",
                Permission::Microphone => "tccutil reset Microphone",
            };
            self.calls.lock().unwrap().push(call);
        }
    }

    const REQUESTS: [&str; 3] = [
        "AXIsProcessTrustedWithOptions prompt",
        "IOHIDRequestAccess",
        "AVCaptureDevice requestAccess",
    ];

    #[test]
    fn each_live_answer_maps_to_its_state() {
        let granted = Mock::new(true, 0, 3);
        assert_eq!(granted.post_event(), Access::Granted);
        assert_eq!(granted.listen_event(), Access::Granted);
        assert_eq!(granted.microphone(), Access::Granted);

        let denied = Mock::new(false, 1, 2);
        assert_eq!(denied.post_event(), Access::Denied);
        assert_eq!(denied.listen_event(), Access::Denied);
        assert_eq!(denied.microphone(), Access::Denied);

        let unasked = Mock::new(false, 2, 0);
        assert_eq!(unasked.listen_event(), Access::NotDetermined);
        assert_eq!(unasked.microphone(), Access::NotDetermined);
        assert_eq!(Mock::new(false, 1, 1).microphone(), Access::Denied);
    }

    #[test]
    fn a_read_uses_the_live_calls_and_never_a_request() {
        let reader = Mock::new(true, 0, 3);

        reader.microphone();
        reader.post_event();
        reader.listen_event();

        assert_eq!(
            reader.calls(),
            [
                "AVCaptureDevice authorizationStatus",
                "AXIsProcessTrusted",
                "IOHIDCheckAccess"
            ]
        );
    }

    #[test]
    fn a_grant_that_is_gone_reads_as_missing() {
        let reader = Mock::new(false, 1, 2);
        assert!(reader.post_event().missing());
        assert!(reader.listen_event().missing());
        assert!(reader.microphone().missing());
    }

    /// The read, reset, and request calls of each permission, in `Permission::ALL` order.
    const ALLOW_CALLS: [(&str, &str, &str); 3] = [
        (
            "AVCaptureDevice authorizationStatus",
            "tccutil reset Microphone",
            REQUESTS[2],
        ),
        (
            "AXIsProcessTrusted",
            "tccutil reset Accessibility",
            REQUESTS[0],
        ),
        ("IOHIDCheckAccess", "tccutil reset ListenEvent", REQUESTS[1]),
    ];

    #[test]
    fn allow_on_a_missing_permission_resets_its_own_record_and_then_requests_once() {
        for (trusted, listen, microphone) in [(false, 2, 0), (false, 1, 2)] {
            for (permission, (read, reset, request)) in Permission::ALL.into_iter().zip(ALLOW_CALLS)
            {
                let reader = Mock::new(trusted, listen, microphone);

                reader.request(permission);

                assert_eq!(
                    reader.calls(),
                    [read, reset, request],
                    "{permission:?} with {trusted} {listen} {microphone}"
                );
            }
        }
    }

    #[test]
    fn allow_on_a_granted_permission_requests_once_and_never_resets() {
        for (permission, (read, _, request)) in Permission::ALL.into_iter().zip(ALLOW_CALLS) {
            let reader = Mock::new(true, 0, 3);

            reader.request(permission);

            assert_eq!(reader.calls(), [read, request], "{permission:?}");
        }
    }

    #[test]
    fn each_permission_resets_its_own_tcc_service() {
        assert_eq!(tcc_service(Permission::Accessibility), "Accessibility");
        assert_eq!(tcc_service(Permission::InputMonitoring), "ListenEvent");
        assert_eq!(tcc_service(Permission::Microphone), "Microphone");
    }
}
