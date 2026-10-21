//! What the system lets Hushpen do, compared with what it let Hushpen do before.
//!
//! A grant that was held and now reads missing is "lost": an ad-hoc rebuild gets a new code
//! hash, and macOS drops its grants. The set of grants held so far lives in the internal
//! setting `permissions.lastGranted`. Reading is passive and never prompts.

use hushpen_core::permission::{self, Permission, Permissions, Preflight};
use hushpen_store::settings::SettingsStore;
use serde_json::{Map, Value};
use std::sync::Arc;
use std::time::Duration;

const LAST_GRANTED: &str = "permissions.lastGranted";

pub struct PermissionWatch {
    preflight: Arc<dyn Preflight>,
    current: Permissions,
    lost: Vec<Permission>,
    /// How long the start reads have settled so far, while a stored grant still reads missing.
    /// `None` once they are final.
    settling: Option<Duration>,
}

impl PermissionWatch {
    /// Reads the permissions once against the stored grants, with no wait. A stored grant that
    /// reads missing is not lost yet: [`Self::settle`] reads it again until it shows, for up to
    /// [`permission::SETTLE_FOR`], so a late first answer is no loss.
    pub fn start(preflight: Arc<dyn Preflight>, settings: &SettingsStore) -> Self {
        let current = Permissions::read(&*preflight);
        let mut watch = Self {
            preflight,
            current,
            lost: Vec::new(),
            settling: Some(Duration::ZERO),
        };
        watch.compare(settings);
        watch
    }

    /// The start reads are not final yet; the caller calls [`Self::settle`] every
    /// [`permission::SETTLE_EVERY`] until this is false.
    pub fn settling(&self) -> bool {
        self.settling.is_some()
    }

    /// One more start read, [`permission::SETTLE_EVERY`] after the last one. Returns whether
    /// anything changed.
    pub fn settle(&mut self, settings: &SettingsStore) -> bool {
        if let Some(waited) = &mut self.settling {
            *waited += permission::SETTLE_EVERY;
        }
        self.refresh(settings)
    }

    /// Reads again. Returns whether anything changed.
    pub fn refresh(&mut self, settings: &SettingsStore) -> bool {
        let before = (self.current, self.lost.clone(), self.settling());
        self.current = Permissions::read(&*self.preflight);
        self.compare(settings);
        before != (self.current, self.lost.clone(), self.settling())
    }

    fn stored_grants(settings: &SettingsStore) -> Map<String, Value> {
        settings
            .get(LAST_GRANTED)
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default()
    }

    fn compare(&mut self, settings: &SettingsStore) {
        let stored = Self::stored_grants(settings);
        if let Some(waited) = self.settling
            && permission::settled(&stored, &self.current, waited)
        {
            self.settling = None;
        }
        let lost = if self.settling() {
            Vec::new()
        } else {
            permission::lost(&stored, &self.current)
        };
        let before = std::mem::replace(&mut self.lost, lost);
        let remembered = permission::remember_granted(&stored, &self.current);
        if remembered != stored
            && let Err(error) = settings.set_internal(LAST_GRANTED, Value::Object(remembered))
        {
            log::warn!("the granted permissions could not be saved: {error}");
        }
        for permission in newly_lost(&before, &self.lost) {
            log::warn!("PERMISSION_LOST {} was granted before", permission.key());
        }
    }

    pub fn current(&self) -> &Permissions {
        &self.current
    }

    pub fn lost(&self) -> &[Permission] {
        &self.lost
    }

    pub fn to_json(&self) -> Value {
        self.current.to_json(&self.lost)
    }
}

fn newly_lost<'a>(before: &[Permission], now: &'a [Permission]) -> Vec<&'a Permission> {
    now.iter()
        .filter(|permission| !before.contains(permission))
        .collect()
}

/// The section for a system that has no permission preflight attached.
pub fn unattached_json() -> Value {
    let none = Permissions::read(&permission::NotApplicable);
    none.to_json(&[])
}

/// The stored grants, for tests.
#[cfg(test)]
pub fn stored(settings: &SettingsStore) -> serde_json::Map<String, Value> {
    settings
        .get(LAST_GRANTED)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_core::permission::Access;
    use serde_json::json;
    use std::sync::Mutex;

    struct Scripted(Mutex<Permissions>);

    impl Scripted {
        fn new(microphone: Access, accessibility: Access, input_monitoring: Access) -> Arc<Self> {
            Arc::new(Self(Mutex::new(Permissions {
                microphone,
                accessibility,
                input_monitoring,
            })))
        }

        fn set_accessibility(&self, access: Access) {
            self.0.lock().unwrap().accessibility = access;
        }
    }

    impl Preflight for Scripted {
        fn microphone(&self) -> Access {
            self.0.lock().unwrap().microphone
        }

        fn post_event(&self) -> Access {
            self.0.lock().unwrap().accessibility
        }

        fn listen_event(&self) -> Access {
            self.0.lock().unwrap().input_monitoring
        }
    }

    fn settings() -> (tempfile::TempDir, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path()).unwrap();
        (dir, store)
    }

    #[test]
    fn a_first_start_with_everything_granted_stores_the_grants_and_loses_nothing() {
        let (_dir, settings) = settings();
        let preflight = Scripted::new(Access::Granted, Access::Granted, Access::Granted);

        let watch = PermissionWatch::start(preflight, &settings);

        assert!(watch.lost().is_empty());
        assert_eq!(
            Value::Object(stored(&settings)),
            json!({"microphone": true, "accessibility": true, "inputMonitoring": true})
        );
    }

    #[test]
    fn a_grant_that_is_gone_at_the_next_start_is_lost_and_stays_lost() {
        let (_dir, settings) = settings();
        let first = Scripted::new(Access::Granted, Access::Granted, Access::Granted);
        PermissionWatch::start(first, &settings);

        let rebuilt = Scripted::new(Access::Granted, Access::Denied, Access::Granted);
        let mut watch = PermissionWatch::start(rebuilt, &settings);
        assert!(
            watch.lost().is_empty(),
            "not lost before the settle window ends"
        );
        settle_fully(&mut watch, &settings);

        assert_eq!(watch.lost(), [Permission::Accessibility]);
        assert!(!watch.refresh(&settings), "nothing changed since the start");
        assert_eq!(watch.lost(), [Permission::Accessibility]);
        assert_eq!(
            watch.to_json(),
            json!({
                "microphone": "granted",
                "accessibility": "denied",
                "inputMonitoring": "granted",
                "lost": ["accessibility"],
            })
        );
    }

    #[test]
    fn granting_it_again_ends_the_loss_on_the_next_refresh() {
        let (_dir, settings) = settings();
        PermissionWatch::start(
            Scripted::new(Access::Granted, Access::Granted, Access::Granted),
            &settings,
        );
        let preflight = Scripted::new(Access::Granted, Access::Denied, Access::Granted);
        let mut watch = PermissionWatch::start(preflight.clone(), &settings);
        settle_fully(&mut watch, &settings);
        assert_eq!(watch.lost().len(), 1);

        preflight.set_accessibility(Access::Granted);

        assert!(watch.refresh(&settings));
        assert!(watch.lost().is_empty());
    }

    /// Makes the settle reads the controller makes on its timer, and counts them.
    fn settle_fully(watch: &mut PermissionWatch, settings: &SettingsStore) -> u32 {
        let mut reads = 0;
        while watch.settling() {
            watch.settle(settings);
            reads += 1;
        }
        reads
    }

    #[test]
    fn a_missing_grant_settles_for_the_whole_window_before_it_counts_as_lost() {
        let (_dir, settings) = settings();
        PermissionWatch::start(
            Scripted::new(Access::Granted, Access::Granted, Access::Granted),
            &settings,
        );
        let mut watch = PermissionWatch::start(
            Scripted::new(Access::Granted, Access::Denied, Access::Granted),
            &settings,
        );

        let reads = settle_fully(&mut watch, &settings);

        let window = permission::SETTLE_FOR.as_millis() / permission::SETTLE_EVERY.as_millis();
        assert_eq!(u128::from(reads), window);
        assert_eq!(watch.lost(), [Permission::Accessibility]);
    }

    #[test]
    fn a_start_reads_once_and_a_grant_that_shows_while_it_settles_is_not_lost() {
        let (_dir, settings) = settings();
        PermissionWatch::start(
            Scripted::new(Access::Granted, Access::Granted, Access::Granted),
            &settings,
        );
        let late = Scripted::new(Access::Granted, Access::Denied, Access::Granted);
        let mut watch = PermissionWatch::start(late.clone(), &settings);
        assert!(watch.settling());
        for _ in 0..4 {
            watch.settle(&settings);
        }
        assert!(watch.lost().is_empty(), "400 ms in, still settling");

        late.set_accessibility(Access::Granted);
        assert!(watch.settle(&settings), "the grant shows and settling ends");

        assert!(!watch.settling());
        assert!(watch.lost().is_empty());
        assert_eq!(
            Value::Object(stored(&settings)),
            json!({"microphone": true, "accessibility": true, "inputMonitoring": true})
        );
    }

    #[test]
    fn a_start_with_every_grant_there_is_final_at_once() {
        let (_dir, settings) = settings();
        let preflight = Scripted::new(Access::Granted, Access::Granted, Access::Granted);
        PermissionWatch::start(preflight.clone(), &settings);

        let watch = PermissionWatch::start(preflight, &settings);

        assert!(!watch.settling());
    }

    #[test]
    fn a_loss_is_logged_once_when_it_starts_and_not_on_each_refresh() {
        let lost = [Permission::Accessibility];

        assert_eq!(newly_lost(&[], &lost), [&Permission::Accessibility]);
        assert!(newly_lost(&lost, &lost).is_empty());
        assert_eq!(
            newly_lost(&lost, &[Permission::Accessibility, Permission::Microphone]),
            [&Permission::Microphone]
        );
    }

    #[test]
    fn a_permission_that_was_never_granted_is_denied_but_not_lost() {
        let (_dir, settings) = settings();

        let watch = PermissionWatch::start(
            Scripted::new(Access::Denied, Access::Denied, Access::Denied),
            &settings,
        );

        assert!(watch.lost().is_empty());
        assert!(stored(&settings).is_empty());
        assert_eq!(watch.current().accessibility, Access::Denied);
    }

    #[test]
    fn a_system_without_the_permissions_reports_not_applicable_and_an_empty_lost_list() {
        let (_dir, settings) = settings();

        let watch = PermissionWatch::start(Arc::new(permission::NotApplicable), &settings);

        assert_eq!(watch.to_json(), unattached_json());
        assert_eq!(
            unattached_json(),
            json!({
                "microphone": "notApplicable",
                "accessibility": "notApplicable",
                "inputMonitoring": "notApplicable",
                "lost": [],
            })
        );
    }
}
