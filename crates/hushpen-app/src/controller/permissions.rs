//! What the system lets Hushpen do, compared with what it let Hushpen do before.
//!
//! A grant that was held and now reads missing is "lost": an ad-hoc rebuild gets a new code
//! hash, and macOS drops its grants. The set of grants held so far lives in the internal
//! setting `permissions.lastGranted`. Reading is passive and never prompts.

use hushpen_core::permission::{self, Permission, Permissions, Preflight};
use hushpen_store::settings::SettingsStore;
use serde_json::Value;
use std::sync::Arc;

const LAST_GRANTED: &str = "permissions.lastGranted";

pub struct PermissionWatch {
    preflight: Arc<dyn Preflight>,
    current: Permissions,
    lost: Vec<Permission>,
}

impl PermissionWatch {
    /// Reads the permissions once against the stored grants.
    pub fn start(preflight: Arc<dyn Preflight>, settings: &SettingsStore) -> Self {
        let current = Permissions::read(&*preflight);
        let mut watch = Self {
            preflight,
            current,
            lost: Vec::new(),
        };
        watch.compare(settings);
        watch
    }

    /// Reads again. Returns whether anything changed.
    pub fn refresh(&mut self, settings: &SettingsStore) -> bool {
        let before = (self.current, self.lost.clone());
        self.current = Permissions::read(&*self.preflight);
        self.compare(settings);
        before != (self.current, self.lost.clone())
    }

    fn compare(&mut self, settings: &SettingsStore) {
        let stored = settings
            .get(LAST_GRANTED)
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        self.lost = permission::lost(&stored, &self.current);
        let remembered = permission::remember_granted(&stored, &self.current);
        if remembered != stored
            && let Err(error) = settings.set_internal(LAST_GRANTED, Value::Object(remembered))
        {
            log::warn!("the granted permissions could not be saved: {error}");
        }
        for permission in &self.lost {
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
        assert_eq!(watch.lost().len(), 1);

        preflight.set_accessibility(Access::Granted);

        assert!(watch.refresh(&settings));
        assert!(watch.lost().is_empty());
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
