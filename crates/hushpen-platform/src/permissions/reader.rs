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
//!
//! On macOS 27 and later, Input Monitoring reads as granted while Accessibility is granted. The
//! listen-only key tap starts for a process that has either one (`CGEventTapCreate`: root,
//! Accessibility, or Input Monitoring), and tccd on macOS 27 answers an Input Monitoring check
//! from the Accessibility record when it holds none of its own. So the keys never wait on the
//! one `IOHIDCheckAccess` answer. Older systems were not measured, and there a tap that
//! Accessibility alone does not start would show a row that lies, so they keep the raw answer.

use hushpen_core::permission::{Access, Permission, Preflight};

/// The first macOS major version where Accessibility alone starts the listen-only key tap.
const LISTEN_FROM_ACCESSIBILITY: u32 = 27;

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
    /// The major version of macOS.
    fn os_major(&self) -> u32;
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
        let hid = self.hid_listen();
        let composed = self.0.os_major() >= LISTEN_FROM_ACCESSIBILITY;
        if hid != Access::Granted && composed && self.post_event() == Access::Granted {
            return Access::Granted;
        }
        hid
    }
}

impl<A: Api> Reader<A> {
    /// The `IOHIDCheckAccess` answer as it is, without the Accessibility rule.
    fn hid_listen(&self) -> Access {
        listen_event_access(self.0.listen_event_status())
    }

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
        let hid = self.hid_listen();
        let accessibility = self.post_event();
        log::info!("PERMISSION_REGISTER iohid={hid:?} accessibility={accessibility:?}");
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
        os_major: u32,
        calls: Mutex<Vec<&'static str>>,
    }

    impl Mock {
        fn new(trusted: bool, listen: u32, microphone: isize) -> Reader<Self> {
            Self::on(27, trusted, listen, microphone)
        }

        fn on(os_major: u32, trusted: bool, listen: u32, microphone: isize) -> Reader<Self> {
            Reader(Self {
                trusted,
                listen,
                microphone,
                os_major,
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

        fn os_major(&self) -> u32 {
            self.os_major
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
    fn accessibility_alone_counts_as_input_monitoring() {
        for listen in [1, 2] {
            let reader = Mock::new(true, listen, 3);

            assert_eq!(
                reader.listen_event(),
                Access::Granted,
                "IOHIDCheckAccess {listen}"
            );
            assert_eq!(reader.calls(), ["IOHIDCheckAccess", "AXIsProcessTrusted"]);
        }
    }

    #[test]
    fn a_granted_input_monitoring_is_read_without_asking_accessibility() {
        let reader = Mock::new(false, 0, 3);

        assert_eq!(reader.listen_event(), Access::Granted);
        assert_eq!(reader.calls(), ["IOHIDCheckAccess"]);
    }

    #[test]
    fn without_either_grant_input_monitoring_keeps_its_own_answer() {
        assert_eq!(Mock::new(false, 1, 3).listen_event(), Access::Denied);
        assert_eq!(Mock::new(false, 2, 3).listen_event(), Access::NotDetermined);
    }

    #[test]
    fn an_older_system_keeps_input_monitoring_as_its_own_permission() {
        let denied = Mock::on(26, true, 1, 3);
        assert_eq!(denied.listen_event(), Access::Denied);
        assert_eq!(denied.calls(), ["IOHIDCheckAccess"]);
        assert_eq!(
            Mock::on(15, true, 2, 3).listen_event(),
            Access::NotDetermined
        );
        assert_eq!(Mock::on(26, false, 0, 3).listen_event(), Access::Granted);
    }

    #[test]
    fn accessibility_with_a_denied_input_monitoring_keeps_the_keys_ready_and_repairs_nothing() {
        use hushpen_core::onboarding::{
            Availability, Facts, RowKey, RowState, Session, permission_rows,
        };
        use hushpen_core::permission::Permissions;
        use hushpen_core::shortcut::Platform;
        let reader = Mock::new(true, 1, 3);

        let now = Permissions::read(&reader);

        assert_eq!(now.input_monitoring, Access::Granted);
        assert!(
            !now.input_monitoring.missing(),
            "a grant that reads there is never lost"
        );
        let rows = permission_rows(&Facts {
            platform: Platform::MacOs,
            session: Session::X11,
            keys: Availability::Ready,
            paste: Availability::Ready,
            mic_listed: true,
            mic_present: true,
            mac: now,
            relaunch: false,
        });
        let row = rows.iter().find(|row| row.key == RowKey::InputMonitoring);
        assert_eq!(row.map(|row| row.state), Some(RowState::Ready));
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

                let mut expected = vec![read];
                if permission == Permission::InputMonitoring {
                    expected.push("AXIsProcessTrusted");
                }
                expected.extend([reset, request]);
                assert_eq!(
                    reader.calls(),
                    expected,
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
