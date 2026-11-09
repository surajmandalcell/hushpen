//! Platform checks and the three macOS permissions Hushpen needs, as app state.
//!
//! Reading them never asks the user: [`Preflight`] has read calls only, and the request calls
//! live behind onboarding buttons. Linux has no such permissions, so every answer there is
//! [`Access::NotApplicable`].
//!
//! An ad-hoc signed build gets a new code hash on each rebuild and update, and macOS then
//! drops its grants. [`lost`] finds a grant that was held and now reads denied.

use serde_json::{Map, Value, json};
use std::time::Duration;

use crate::os::{Capabilities, Os};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Microphone,
    /// Posting key events and reading the focused field (`AXIsProcessTrusted`).
    Accessibility,
    /// Listening for the hold key (`IOHIDCheckAccess` for listen events). Accessibility lets the
    /// key tap run too, so on macOS 27 and later it reads granted while Accessibility is granted.
    InputMonitoring,
}

impl Permission {
    pub const ALL: [Permission; 3] = [
        Permission::Microphone,
        Permission::Accessibility,
        Permission::InputMonitoring,
    ];

    /// The key in `hookctl state` and in `permissions.lastGranted`.
    pub fn key(self) -> &'static str {
        match self {
            Permission::Microphone => "microphone",
            Permission::Accessibility => "accessibility",
            Permission::InputMonitoring => "inputMonitoring",
        }
    }

    /// The Privacy pane of System Settings that holds this permission.
    pub fn settings_url(self) -> &'static str {
        match self {
            Permission::Microphone => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            Permission::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Permission::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        }
    }

    pub fn settings_url_for(self, os: Os) -> Option<&'static str> {
        match (os, self) {
            (Os::Mac, _) => Some(self.settings_url()),
            (Os::Windows, Self::Microphone) => Some("ms-settings:privacy-microphone"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Granted,
    Denied,
    /// macOS has not asked yet, or the grant was reset.
    NotDetermined,
    /// This system has no such permission.
    NotApplicable,
}

impl Access {
    pub fn key(self) -> &'static str {
        match self {
            Access::Granted => "granted",
            Access::Denied => "denied",
            Access::NotDetermined => "notDetermined",
            Access::NotApplicable => "notApplicable",
        }
    }

    /// The permission is needed here and is not held.
    pub fn missing(self) -> bool {
        matches!(self, Access::Denied | Access::NotDetermined)
    }
}

/// Read-only answers, read live on each call. No method here may show a prompt.
pub trait Preflight: Send + Sync {
    fn microphone(&self) -> Access;
    /// Accessibility, from `AXIsProcessTrusted`.
    fn post_event(&self) -> Access;
    /// Whether the hold key can be heard: Input Monitoring, from `IOHIDCheckAccess`, or on
    /// macOS 27 and later the Accessibility grant, which lets the listen-only key tap run too.
    fn listen_event(&self) -> Access;
}

/// A system with no permissions to ask for.
pub struct NotApplicable;

impl Preflight for NotApplicable {
    fn microphone(&self) -> Access {
        Access::NotApplicable
    }

    fn post_event(&self) -> Access {
        Access::NotApplicable
    }

    fn listen_event(&self) -> Access {
        Access::NotApplicable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions {
    pub microphone: Access,
    pub accessibility: Access,
    pub input_monitoring: Access,
}

impl Permissions {
    pub fn read(preflight: &dyn Preflight) -> Self {
        Self {
            microphone: preflight.microphone(),
            accessibility: preflight.post_event(),
            input_monitoring: preflight.listen_event(),
        }
    }

    pub fn get(&self, permission: Permission) -> Access {
        match permission {
            Permission::Microphone => self.microphone,
            Permission::Accessibility => self.accessibility,
            Permission::InputMonitoring => self.input_monitoring,
        }
    }

    /// The `permissions` section of `hookctl state`.
    pub fn to_json(&self, lost: &[Permission]) -> Value {
        json!({
            "microphone": self.microphone.key(),
            "accessibility": self.accessibility.key(),
            "inputMonitoring": self.input_monitoring.key(),
            "lost": lost.iter().map(|permission| permission.key()).collect::<Vec<_>>(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckId {
    Microphone,
    Accessibility,
    InputMonitoring,
    GlobalKeys,
    AdminApps,
    X11Session,
    TrayWatcher,
}

impl CheckId {
    pub fn key(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Accessibility => "accessibility",
            Self::InputMonitoring => "inputMonitoring",
            Self::GlobalKeys => "keys",
            Self::AdminApps => "admin",
            Self::X11Session => "x11",
            Self::TrayWatcher => "tray",
        }
    }

    pub fn permission(self) -> Option<Permission> {
        match self {
            Self::Microphone => Some(Permission::Microphone),
            Self::Accessibility => Some(Permission::Accessibility),
            Self::InputMonitoring => Some(Permission::InputMonitoring),
            _ => None,
        }
    }

    pub fn help(self) -> Option<&'static str> {
        match self {
            Self::AdminApps => Some(
                "Windows blocks typing into apps with higher privileges. Hushpen copies the text instead.",
            ),
            Self::X11Session => {
                Some("On Wayland, use the record button and copy the finished text.")
            }
            Self::TrayWatcher => Some("Keep the Hushpen window open to use its controls."),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,
    NeedsAttention,
    Info,
    NotAvailable,
}

impl Status {
    pub fn key(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsAttention => "needsAttention",
            Self::Info => "info",
            Self::NotAvailable => "notAvailable",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::NeedsAttention => "Needs attention",
            Self::Info => "Info",
            Self::NotAvailable => "Not available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Allow,
    OpenSettings,
    LearnMore,
    Retry,
}

impl Action {
    pub fn key(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::OpenSettings => "open",
            Self::LearnMore => "learn",
            Self::Retry => "retry",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Allow => "Allow",
            Self::OpenSettings => "Open Settings",
            Self::LearnMore => "Learn more",
            Self::Retry => "Retry",
        }
    }
}

/// One row shared by onboarding and Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub id: CheckId,
    pub title: &'static str,
    pub detail: String,
    pub status: Status,
    pub actions: [Option<Action>; 2],
    /// Continue waits for this check when it needs attention.
    pub blocking: bool,
    /// Retry starts a new process after a macOS grant changed.
    pub relaunch: bool,
}

/// Live answers supplied by the platform and microphone service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    pub permissions: Permissions,
    pub mic_listed: bool,
    pub mic_present: bool,
    pub keys: bool,
    pub elevated: bool,
    pub tray_watcher: bool,
    pub relaunch: bool,
}

pub fn checks(os: Os, capabilities: Capabilities, facts: &Facts) -> Vec<Check> {
    let microphone = microphone_check(os, facts);
    match os {
        Os::Mac => vec![
            microphone,
            mac_check(
                CheckId::Accessibility,
                "Accessibility",
                Permission::Accessibility,
                facts,
                "Hushpen can press the paste key for you.",
                "Allow Accessibility so Hushpen can paste the text for you.",
            ),
            mac_check(
                CheckId::InputMonitoring,
                "Input Monitoring",
                Permission::InputMonitoring,
                facts,
                "Hushpen can hear the hold key.",
                "Allow Input Monitoring so Hushpen can hear the hold key.",
            ),
        ],
        Os::Windows => vec![
            microphone,
            Check {
                id: CheckId::AdminApps,
                title: "Admin apps",
                detail: if facts.elevated {
                    "Hushpen runs as administrator. Apps at the same privilege level can get typed text."
                } else {
                    "Apps that run as administrator cannot get typed text. Hushpen copies the text instead."
                }
                .to_owned(),
                status: Status::Info,
                actions: [Some(Action::LearnMore), None],
                blocking: false,
                relaunch: false,
            },
            Check {
                id: CheckId::GlobalKeys,
                title: "Global keys",
                detail: if facts.keys && capabilities.global_keys {
                    "The hold key can start a dictation from any app."
                } else {
                    "Global keys are not running. Use the record button in Hushpen instead."
                }
                .to_owned(),
                status: if facts.keys && capabilities.global_keys {
                    Status::Ready
                } else {
                    Status::NeedsAttention
                },
                actions: [(!(facts.keys && capabilities.global_keys)).then_some(Action::Retry), None],
                blocking: false,
                relaunch: false,
            },
        ],
        Os::Linux => {
            let mut rows = Vec::new();
            if !capabilities.global_keys || !capabilities.paste {
                rows.push(Check {
                    id: CheckId::X11Session,
                    title: "X11 session",
                    detail: "Global keys and paste need an X11 session".to_owned(),
                    status: Status::NeedsAttention,
                    actions: [Some(Action::LearnMore), None],
                    blocking: false,
                    relaunch: false,
                });
            } else if !facts.keys {
                rows.push(Check {
                    id: CheckId::GlobalKeys,
                    title: "Global keys",
                    detail: "Global keys are not running. Use the record button in Hushpen instead."
                        .to_owned(),
                    status: Status::NeedsAttention,
                    actions: [Some(Action::Retry), None],
                    blocking: false,
                    relaunch: false,
                });
            }
            rows.push(microphone);
            if !facts.tray_watcher {
                rows.push(Check {
                    id: CheckId::TrayWatcher,
                    title: "Tray icon",
                    detail: "No tray watcher was found. Keep the Hushpen window open to use its controls."
                        .to_owned(),
                    status: Status::Info,
                    actions: [Some(Action::LearnMore), None],
                    blocking: false,
                    relaunch: false,
                });
            }
            rows
        }
    }
}

fn microphone_check(os: Os, facts: &Facts) -> Check {
    let (status, detail) = if facts.permissions.microphone.missing() {
        (
            Status::NeedsAttention,
            "Allow Hushpen to use the microphone.",
        )
    } else if !facts.mic_listed {
        (Status::NeedsAttention, "Looking for a microphone.")
    } else if !facts.mic_present {
        (
            Status::NeedsAttention,
            "No microphone was found. Connect one to continue.",
        )
    } else {
        (
            if os == Os::Linux {
                Status::Info
            } else {
                Status::Ready
            },
            "A microphone is ready.",
        )
    };
    Check {
        id: CheckId::Microphone,
        title: "Microphone",
        detail: detail.to_owned(),
        status,
        actions: match os {
            Os::Mac => [
                (status != Status::Ready).then_some(Action::Allow),
                Some(Action::OpenSettings),
            ],
            Os::Windows => [Some(Action::OpenSettings), None],
            Os::Linux => [None, None],
        },
        blocking: true,
        relaunch: false,
    }
}

fn mac_check(
    id: CheckId,
    title: &'static str,
    permission: Permission,
    facts: &Facts,
    ready: &str,
    missing: &str,
) -> Check {
    let (status, detail) = if facts.permissions.get(permission).missing() {
        (Status::NeedsAttention, missing)
    } else if permission == Permission::InputMonitoring && facts.relaunch {
        (
            Status::NeedsAttention,
            "Input Monitoring is on, but the hold key works only after Hushpen opens again.",
        )
    } else {
        (Status::Ready, ready)
    };
    Check {
        id,
        title,
        detail: detail.to_owned(),
        status,
        actions: [
            (status != Status::Ready).then_some(
                if permission == Permission::InputMonitoring
                    && facts.relaunch
                    && !facts.permissions.get(permission).missing()
                {
                    Action::Retry
                } else {
                    Action::Allow
                },
            ),
            Some(Action::OpenSettings),
        ],
        blocking: true,
        relaunch: permission == Permission::InputMonitoring
            && facts.relaunch
            && !facts.permissions.get(permission).missing(),
    }
}

pub fn checks_ready(checks: &[Check]) -> bool {
    checks
        .iter()
        .all(|check| !check.blocking || matches!(check.status, Status::Ready | Status::Info))
}

fn was_granted(last_granted: &Map<String, Value>, permission: Permission) -> bool {
    last_granted
        .get(permission.key())
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The permissions that were granted before (`permissions.lastGranted`) and are missing now.
pub fn lost(last_granted: &Map<String, Value>, now: &Permissions) -> Vec<Permission> {
    Permission::ALL
        .into_iter()
        .filter(|permission| {
            was_granted(last_granted, *permission) && now.get(*permission).missing()
        })
        .collect()
}

/// How long a grant that reads missing gets to show up again before it counts as lost.
pub const SETTLE_FOR: Duration = Duration::from_secs(2);
/// How long the reader waits between two reads while it settles.
pub const SETTLE_EVERY: Duration = Duration::from_millis(100);

/// Whether a start read, made `waited` after the first one, is final. At login the system can
/// answer late or wrongly for a moment, so a stored grant that reads missing is read again
/// every [`SETTLE_EVERY`] until it shows, for at most [`SETTLE_FOR`]. A grant that is really
/// gone never shows, so it costs the whole wait. A permission that was never granted is not
/// waited for.
pub fn settled(last_granted: &Map<String, Value>, now: &Permissions, waited: Duration) -> bool {
    waited >= SETTLE_FOR || lost(last_granted, now).is_empty()
}

/// The value to store as `permissions.lastGranted`: everything granted before or now. A lost
/// grant stays in it until macOS grants it again, so the loss is not forgotten at the next
/// start.
pub fn remember_granted(
    last_granted: &Map<String, Value>,
    now: &Permissions,
) -> Map<String, Value> {
    Permission::ALL
        .into_iter()
        .filter(|permission| {
            was_granted(last_granted, *permission) || now.get(*permission) == Access::Granted
        })
        .map(|permission| (permission.key().to_owned(), Value::Bool(true)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn facts() -> Facts {
        Facts {
            permissions: Permissions {
                microphone: Access::Granted,
                accessibility: Access::Granted,
                input_monitoring: Access::Granted,
            },
            mic_listed: true,
            mic_present: true,
            keys: true,
            elevated: false,
            tray_watcher: true,
            relaunch: false,
        }
    }

    #[test]
    fn mac_checks_keep_the_three_permission_rows_and_only_needed_actions() {
        let facts = facts();
        let rows = checks(Os::Mac, Os::Mac.capabilities(true, true), &facts);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [
                CheckId::Microphone,
                CheckId::Accessibility,
                CheckId::InputMonitoring
            ]
        );
        assert!(rows.iter().all(|row| row.status == Status::Ready));
        assert!(
            rows.iter()
                .all(|row| row.actions == [None, Some(Action::OpenSettings)])
        );
        assert!(checks_ready(&rows));

        for permission in Permission::ALL {
            let mut facts = facts;
            match permission {
                Permission::Microphone => facts.permissions.microphone = Access::Denied,
                Permission::Accessibility => facts.permissions.accessibility = Access::Denied,
                Permission::InputMonitoring => facts.permissions.input_monitoring = Access::Denied,
            }
            let rows = checks(Os::Mac, Os::Mac.capabilities(true, true), &facts);
            let row = rows
                .iter()
                .find(|row| row.id.permission() == Some(permission))
                .unwrap();
            assert_eq!(row.status, Status::NeedsAttention);
            assert_eq!(
                row.actions,
                [Some(Action::Allow), Some(Action::OpenSettings)]
            );
            assert!(!checks_ready(&rows));
        }
    }

    #[test]
    fn microphone_failures_block_every_os_and_unknown_windows_access_blocks() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            for (access, listed, present) in [
                (Access::Denied, true, true),
                (Access::NotDetermined, true, true),
                (Access::Granted, false, false),
                (Access::Granted, true, false),
            ] {
                let facts = Facts {
                    permissions: Permissions {
                        microphone: access,
                        ..facts().permissions
                    },
                    mic_listed: listed,
                    mic_present: present,
                    ..facts()
                };
                let rows = checks(os, os.capabilities(true, true), &facts);
                assert!(
                    !checks_ready(&rows),
                    "{os:?}: {access:?}, {listed}, {present}"
                );
            }
        }
    }

    #[test]
    fn windows_admin_and_failed_hook_are_non_blocking_and_retry_is_available() {
        let mut facts = facts();
        facts.keys = false;
        let rows = checks(Os::Windows, Os::Windows.capabilities(false, false), &facts);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [CheckId::Microphone, CheckId::AdminApps, CheckId::GlobalKeys]
        );
        assert_eq!(rows[0].actions, [Some(Action::OpenSettings), None]);
        assert_eq!(rows[1].status, Status::Info);
        assert_eq!(rows[1].actions, [Some(Action::LearnMore), None]);
        assert_eq!(rows[2].status, Status::NeedsAttention);
        assert_eq!(rows[2].actions, [Some(Action::Retry), None]);
        assert!(checks_ready(&rows));
        facts.keys = true;
        facts.elevated = true;
        let rows = checks(Os::Windows, Os::Windows.capabilities(false, false), &facts);
        assert!(rows[1].detail.starts_with("Hushpen runs as administrator."));
        assert_eq!(rows[2].status, Status::Ready);
        assert_eq!(rows[2].actions, [None, None]);
    }

    #[test]
    fn linux_wayland_and_missing_tray_explain_limits_without_blocking() {
        let mut facts = facts();
        facts.tray_watcher = false;
        let rows = checks(Os::Linux, Os::Linux.capabilities(false, false), &facts);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [
                CheckId::X11Session,
                CheckId::Microphone,
                CheckId::TrayWatcher
            ]
        );
        assert_eq!(rows[0].status, Status::NeedsAttention);
        assert_eq!(rows[0].detail, "Global keys and paste need an X11 session");
        assert_eq!(rows[1].status, Status::Info);
        assert_eq!(rows[2].status, Status::Info);
        assert!(checks_ready(&rows));
        facts.tray_watcher = true;
        let rows = checks(Os::Linux, Os::Linux.capabilities(true, false), &facts);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, CheckId::Microphone);
    }

    #[test]
    fn a_mac_grant_that_needs_a_new_process_offers_retry_and_blocks() {
        let facts = Facts {
            relaunch: true,
            ..facts()
        };
        let rows = checks(Os::Mac, Os::Mac.capabilities(true, true), &facts);
        let row = &rows[2];
        assert!(row.relaunch);
        assert_eq!(row.status, Status::NeedsAttention);
        assert_eq!(
            row.actions,
            [Some(Action::Retry), Some(Action::OpenSettings)]
        );
        assert!(!checks_ready(&rows));
        let mut facts = facts;
        facts.permissions.input_monitoring = Access::Denied;
        let rows = checks(Os::Mac, Os::Mac.capabilities(true, true), &facts);
        assert!(!rows[2].relaunch);
        assert_eq!(rows[2].actions[0], Some(Action::Allow));
    }

    #[test]
    fn linux_x11_hook_failure_offers_retry_without_blocking() {
        let facts = Facts {
            keys: false,
            ..facts()
        };
        let rows = checks(Os::Linux, Os::Linux.capabilities(true, false), &facts);
        assert_eq!(rows[0].id, CheckId::GlobalKeys);
        assert_eq!(rows[0].status, Status::NeedsAttention);
        assert_eq!(rows[0].actions, [Some(Action::Retry), None]);
        assert!(checks_ready(&rows));
    }

    #[test]
    fn settings_links_only_name_the_platforms_permission_panes() {
        for permission in Permission::ALL {
            assert_eq!(
                permission.settings_url_for(Os::Mac),
                Some(permission.settings_url())
            );
            assert_eq!(permission.settings_url_for(Os::Linux), None);
        }
        assert_eq!(
            Permission::Microphone.settings_url_for(Os::Windows),
            Some("ms-settings:privacy-microphone")
        );
        assert_eq!(
            Permission::Accessibility.settings_url_for(Os::Windows),
            None
        );
        assert_eq!(
            Permission::InputMonitoring.settings_url_for(Os::Windows),
            None
        );
    }

    /// Answers like macOS and records every call. It has no request method to call.
    struct Mock {
        microphone: Access,
        post: Access,
        listen: Access,
        calls: Mutex<Vec<&'static str>>,
    }

    impl Mock {
        fn new(microphone: Access, post: Access, listen: Access) -> Self {
            Self {
                microphone,
                post,
                listen,
                calls: Mutex::default(),
            }
        }
    }

    impl Preflight for Mock {
        fn microphone(&self) -> Access {
            self.calls.lock().unwrap().push("microphone");
            self.microphone
        }

        fn post_event(&self) -> Access {
            self.calls.lock().unwrap().push("AXIsProcessTrusted");
            self.post
        }

        fn listen_event(&self) -> Access {
            self.calls.lock().unwrap().push("IOHIDCheckAccess");
            self.listen
        }
    }

    fn stored(keys: &[&str]) -> Map<String, Value> {
        keys.iter()
            .map(|key| ((*key).to_owned(), Value::Bool(true)))
            .collect()
    }

    #[test]
    fn each_answer_maps_to_its_permission_state() {
        let mock = Mock::new(Access::NotDetermined, Access::Granted, Access::Denied);

        let read = Permissions::read(&mock);

        assert_eq!(read.microphone, Access::NotDetermined);
        assert_eq!(read.accessibility, Access::Granted);
        assert_eq!(read.input_monitoring, Access::Denied);
    }

    #[test]
    fn reading_asks_each_preflight_once_and_calls_nothing_else() {
        let mock = Mock::new(Access::Granted, Access::Granted, Access::Granted);

        Permissions::read(&mock);

        assert_eq!(
            *mock.calls.lock().unwrap(),
            ["microphone", "AXIsProcessTrusted", "IOHIDCheckAccess"]
        );
    }

    #[test]
    fn the_state_json_has_every_key_and_the_lost_list() {
        let read = Permissions {
            microphone: Access::Granted,
            accessibility: Access::Denied,
            input_monitoring: Access::NotApplicable,
        };

        let json = read.to_json(&[Permission::Accessibility]);

        assert_eq!(
            json,
            json!({
                "microphone": "granted",
                "accessibility": "denied",
                "inputMonitoring": "notApplicable",
                "lost": ["accessibility"],
            })
        );
    }

    #[test]
    fn a_grant_that_was_stored_and_now_reads_denied_is_lost() {
        let now = Permissions::read(&Mock::new(Access::Granted, Access::Denied, Access::Granted));

        let lost = lost(
            &stored(&["microphone", "accessibility", "inputMonitoring"]),
            &now,
        );

        assert_eq!(lost, [Permission::Accessibility]);
    }

    #[test]
    fn a_reset_grant_that_reads_not_determined_is_lost_too() {
        let now = Permissions::read(&Mock::new(
            Access::NotDetermined,
            Access::Granted,
            Access::Granted,
        ));

        assert_eq!(
            lost(&stored(&["microphone"]), &now),
            [Permission::Microphone]
        );
    }

    #[test]
    fn a_permission_that_was_never_granted_is_not_lost() {
        let now = Permissions::read(&Mock::new(Access::Denied, Access::Denied, Access::Denied));

        assert_eq!(lost(&Map::new(), &now), []);
        assert_eq!(
            lost(&stored(&["microphone"]), &now),
            [Permission::Microphone]
        );
    }

    #[test]
    fn linux_never_loses_anything() {
        let now = Permissions::read(&NotApplicable);

        assert_eq!(
            lost(
                &stored(&["microphone", "accessibility", "inputMonitoring"]),
                &now
            ),
            []
        );
        assert!(remember_granted(&Map::new(), &now).is_empty());
    }

    #[test]
    fn the_stored_set_keeps_a_lost_grant_and_adds_a_new_one() {
        let now = Permissions::read(&Mock::new(Access::Granted, Access::Denied, Access::Denied));

        let next = remember_granted(&stored(&["accessibility"]), &now);

        assert_eq!(next, stored(&["microphone", "accessibility"]));
    }

    #[test]
    fn a_regranted_permission_is_no_longer_lost() {
        let first = Permissions::read(&Mock::new(Access::Granted, Access::Denied, Access::Granted));
        let remembered = remember_granted(&stored(&["microphone", "accessibility"]), &first);
        assert_eq!(lost(&remembered, &first), [Permission::Accessibility]);

        let again = Permissions::read(&Mock::new(
            Access::Granted,
            Access::Granted,
            Access::Granted,
        ));

        assert_eq!(lost(&remembered, &again), []);
    }

    /// Answers denied until `after` of virtual time has passed, then granted. The pause the
    /// reader makes moves the clock, so no test sleeps.
    struct Late {
        after: Duration,
        now: Mutex<Duration>,
        reads: AtomicU32,
    }

    impl Late {
        fn new(after: Duration) -> Self {
            Self {
                after,
                now: Mutex::new(Duration::ZERO),
                reads: AtomicU32::new(0),
            }
        }

        fn answer(&self) -> Access {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.elapsed() >= self.after {
                Access::Granted
            } else {
                Access::Denied
            }
        }

        fn elapsed(&self) -> Duration {
            *self.now.lock().unwrap()
        }

        fn pause(&self, wait: Duration) {
            *self.now.lock().unwrap() += wait;
        }
    }

    struct LateRead<'a>(&'a Late);

    impl Preflight for LateRead<'_> {
        fn microphone(&self) -> Access {
            self.0.answer()
        }

        fn post_event(&self) -> Access {
            Access::Granted
        }

        fn listen_event(&self) -> Access {
            Access::Granted
        }
    }

    const ALL_STORED: [&str; 3] = ["microphone", "accessibility", "inputMonitoring"];

    /// Reads until [`settled`] says the read is final, as the start check does on its timer.
    fn read_settled(
        preflight: &dyn Preflight,
        last_granted: &Map<String, Value>,
        pause: &dyn Fn(Duration),
    ) -> Permissions {
        let mut waited = Duration::ZERO;
        loop {
            let now = Permissions::read(preflight);
            if settled(last_granted, &now, waited) {
                return now;
            }
            pause(SETTLE_EVERY);
            waited += SETTLE_EVERY;
        }
    }

    #[test]
    fn a_late_first_read_is_read_again_until_the_grant_shows() {
        let late = Late::new(Duration::from_millis(500));

        let read = read_settled(&LateRead(&late), &stored(&ALL_STORED), &|wait| {
            late.pause(wait)
        });

        assert_eq!(read.microphone, Access::Granted);
        assert!(lost(&stored(&ALL_STORED), &read).is_empty());
    }

    #[test]
    fn a_grant_that_stays_missing_for_two_seconds_is_still_missing() {
        let late = Late::new(Duration::from_secs(60));

        let read = read_settled(&LateRead(&late), &stored(&ALL_STORED), &|wait| {
            late.pause(wait)
        });

        assert_eq!(read.microphone, Access::Denied);
        assert_eq!(lost(&stored(&ALL_STORED), &read), [Permission::Microphone]);
        assert!(late.elapsed() >= SETTLE_FOR && late.elapsed() <= SETTLE_FOR + SETTLE_EVERY);
    }

    #[test]
    fn granted_grants_are_read_once_with_no_pause() {
        let late = Late::new(Duration::ZERO);

        read_settled(&LateRead(&late), &stored(&ALL_STORED), &|_| {
            panic!("no pause needed")
        });

        assert_eq!(late.reads.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_permission_that_was_never_granted_is_not_waited_for() {
        let late = Late::new(Duration::from_secs(60));

        let read = read_settled(&LateRead(&late), &Map::new(), &|_| {
            panic!("a first run waits for nobody")
        });

        assert_eq!(read.microphone, Access::Denied);
    }

    #[test]
    fn a_stored_value_that_is_not_a_true_flag_does_not_count() {
        let mut odd = Map::new();
        odd.insert("microphone".into(), json!("yes"));
        odd.insert("accessibility".into(), json!(false));
        let now = Permissions::read(&Mock::new(Access::Denied, Access::Denied, Access::Denied));

        assert_eq!(lost(&odd, &now), []);
    }
}
