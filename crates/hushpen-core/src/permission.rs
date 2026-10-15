//! The three macOS permissions Hushpen needs, as app state.
//!
//! Reading them never asks the user: [`Preflight`] has read calls only, and the request calls
//! live behind onboarding buttons. Linux has no such permissions, so every answer there is
//! [`Access::NotApplicable`].
//!
//! An ad-hoc signed build gets a new code hash on each rebuild and update, and macOS then
//! drops its grants. [`lost`] finds a grant that was held and now reads denied.

use serde_json::{Map, Value, json};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    Microphone,
    /// Posting key events and reading the focused field (`AXIsProcessTrusted`).
    Accessibility,
    /// Listening for the hold key (`IOHIDCheckAccess` for listen events).
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
    /// Input Monitoring, from `IOHIDCheckAccess`.
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

/// Reads the permissions at start. At login the system can answer late or wrongly for a
/// moment, so a stored grant that reads missing is read again until it shows, for at most
/// [`SETTLE_FOR`]. A grant that is really gone never shows, so it costs the whole wait. A
/// permission that was never granted is not waited for. `pause` sleeps.
pub fn read_settled(
    preflight: &dyn Preflight,
    last_granted: &Map<String, Value>,
    pause: &dyn Fn(Duration),
) -> Permissions {
    let mut waited = Duration::ZERO;
    loop {
        let now = Permissions::read(preflight);
        if waited >= SETTLE_FOR || lost(last_granted, &now).is_empty() {
            return now;
        }
        pause(SETTLE_EVERY);
        waited += SETTLE_EVERY;
    }
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
