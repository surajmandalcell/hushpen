//! What the macOS permission calls answer, and how each answer maps to a state.
//!
//! The calls sit behind [`Api`] so tests can swap in a mock that records which one ran. Every
//! read uses a live call. The `CGPreflight*EventAccess` calls answer from a per-process cache,
//! so a grant made in System Settings would not show until a restart.
//! A read never prompts and never resets. [`Reader::register`] only reads, at launch, so macOS
//! lists Hushpen without asking. Only [`Reader::request`] asks macOS, and only after a click. A
//! request resets Hushpen's own record first only to repair a lost grant, because an update
//! leaves a record that macOS keeps and ignores. A denied row is never reset: that would delete
//! the row the user has to switch on.

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
    fn access(&self, permission: Permission) -> Access {
        match permission {
            Permission::Accessibility => self.post_event(),
            Permission::InputMonitoring => self.listen_event(),
            Permission::Microphone => self.microphone(),
        }
    }

    /// Lists Hushpen in the Privacy panes at launch, with no prompt and no click. tccd writes
    /// the Accessibility record (denied, off) once a process has read both its Input
    /// Monitoring and its Accessibility state, and on macOS 27 that record is what the
    /// Input Monitoring answer comes from. A listen-event request adds no row there and a
    /// microphone request always shows an alert, so neither runs here. Reading is all this does.
    pub(super) fn register(&self) {
        let listen = self.listen_event();
        let accessibility = self.post_event();
        log::info!(
            "PERMISSION_REGISTER inputMonitoring={listen:?} accessibility={accessibility:?}"
        );
    }

    /// Calls the request API of `permission` once, whatever the state is. The request is what
    /// makes macOS list Hushpen in the Privacy pane and show its prompt. With `repair`, which is
    /// for a grant that was held and is gone, Hushpen's own record is reset first, because an
    /// update that changed the code hash leaves a record that macOS keeps but ignores. Without
    /// `repair` nothing is reset: a denied row stays in the list for the user to switch on, and
    /// a reset would delete it. Call it only after a click on Allow.
    pub(super) fn request(&self, permission: Permission, repair: bool) {
        if repair && self.access(permission) != Access::Granted {
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
    fn a_repair_of_a_missing_permission_resets_its_own_record_and_then_requests_once() {
        for (trusted, listen, microphone) in [(false, 2, 0), (false, 1, 2)] {
            for (permission, (read, reset, request)) in Permission::ALL.into_iter().zip(ALLOW_CALLS)
            {
                let reader = Mock::new(trusted, listen, microphone);

                reader.request(permission, true);

                assert_eq!(
                    reader.calls(),
                    [read, reset, request],
                    "{permission:?} with {trusted} {listen} {microphone}"
                );
            }
        }
    }

    #[test]
    fn a_repair_of_a_granted_permission_requests_once_and_never_resets() {
        for (permission, (read, _, request)) in Permission::ALL.into_iter().zip(ALLOW_CALLS) {
            let reader = Mock::new(true, 0, 3);

            reader.request(permission, true);

            assert_eq!(reader.calls(), [read, request], "{permission:?}");
        }
    }

    #[test]
    fn allow_without_a_repair_requests_once_and_never_resets_a_denied_or_unasked_row() {
        for (trusted, listen, microphone) in [(false, 2, 0), (false, 1, 2), (true, 0, 3)] {
            for (permission, (_, _, request)) in Permission::ALL.into_iter().zip(ALLOW_CALLS) {
                let reader = Mock::new(trusted, listen, microphone);

                reader.request(permission, false);

                assert_eq!(
                    reader.calls(),
                    [request],
                    "{permission:?} with {trusted} {listen} {microphone}: a reset would delete the row"
                );
            }
        }
    }

    #[test]
    fn the_launch_registration_reads_both_states_and_asks_nothing() {
        for (trusted, listen, microphone) in [(false, 2, 0), (false, 1, 2), (true, 0, 3)] {
            let reader = Mock::new(trusted, listen, microphone);

            reader.register();

            assert_eq!(
                reader.calls(),
                ["IOHIDCheckAccess", "AXIsProcessTrusted"],
                "no prompt, no request, and no reset with {trusted} {listen} {microphone}"
            );
        }
    }

    #[test]
    fn each_permission_resets_its_own_tcc_service() {
        assert_eq!(tcc_service(Permission::Accessibility), "Accessibility");
        assert_eq!(tcc_service(Permission::InputMonitoring), "ListenEvent");
        assert_eq!(tcc_service(Permission::Microphone), "Microphone");
    }
}
