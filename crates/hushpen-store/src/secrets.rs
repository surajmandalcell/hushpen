//! `config/secrets.json`: `{"endpointApiKey": "..."}`, mode 0600.
//!
//! The key of the opt-in endpoint is the only secret. It stays out of `settings.json`, the
//! logs, and the test hook. The file is created private from its first byte (see
//! [`atomic::write_private`]), and a wider mode found at open is tightened and logged.

use crate::{Error, Result, atomic};
use hushpen_core::endpoint::key_is_valid;
use serde_json::{Map, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const KEY_FIELD: &str = "endpointApiKey";
const PRIVATE_MODE: u32 = 0o600;

pub struct Secrets {
    path: PathBuf,
    state: Mutex<Map<String, Value>>,
}

impl Secrets {
    /// Opens `<config_dir>/secrets.json`. A missing file means no secrets and is not created
    /// until a key is saved. A file that is not a JSON object counts as empty and one
    /// `SECRETS_CORRUPT` line is logged; the next save replaces it.
    pub fn open(config_dir: &Path) -> Result<Self> {
        fs::create_dir_all(config_dir)
            .map_err(|e| Error::io(format!("could not create {}", config_dir.display()), e))?;
        let path = config_dir.join("secrets.json");
        let state = match fs::read(&path) {
            Ok(bytes) => {
                tighten(&path);
                match serde_json::from_slice::<Value>(&bytes) {
                    Ok(Value::Object(map)) => map,
                    _ => {
                        log::warn!("SECRETS_CORRUPT secrets.json is not a JSON object");
                        Map::new()
                    }
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(error) => {
                return Err(Error::io(
                    format!("could not read {}", path.display()),
                    error,
                ));
            }
        };
        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }

    pub fn endpoint_api_key(&self) -> Option<String> {
        self.lock()
            .get(KEY_FIELD)
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
    }

    /// Whether a key is stored. The hook and the views show this, never the key.
    pub fn endpoint_api_key_set(&self) -> bool {
        self.endpoint_api_key().is_some()
    }

    /// Saves the key, or removes it for `None` or text that is only space. A key that cannot
    /// go into a header is refused; the error names the field, never the value.
    pub fn set_endpoint_api_key(&self, key: Option<&str>) -> Result<()> {
        let key = key.map(str::trim).filter(|key| !key.is_empty());
        if let Some(key) = key
            && !key_is_valid(key)
        {
            return Err(Error::InvalidSetting(KEY_FIELD.into()));
        }
        let mut state = self.lock();
        let mut next = state.clone();
        match key {
            Some(key) => next.insert(KEY_FIELD.into(), Value::String(key.to_owned())),
            None => next.remove(KEY_FIELD),
        };
        let mut text = serde_json::to_string_pretty(&Value::Object(next.clone()))
            .map_err(|e| Error::io("could not encode secrets", e.into()))?;
        text.push('\n');
        atomic::write_private(&self.path, text.as_bytes(), PRIVATE_MODE)?;
        *state = next;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Map<String, Value>> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(unix)]
fn tighten(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(meta) = fs::metadata(path) else { return };
    if meta.permissions().mode() & 0o777 != PRIVATE_MODE {
        match fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_MODE)) {
            Ok(()) => log::warn!("SECRETS_MODE_FIXED secrets.json was readable by others"),
            Err(error) => log::warn!("SECRETS_MODE_FIXED could not tighten the mode: {error}"),
        }
    }
}

#[cfg(not(unix))]
fn tighten(_path: &Path) {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    fn open() -> (tempfile::TempDir, Secrets) {
        let tmp = tempfile::tempdir().unwrap();
        let secrets = Secrets::open(tmp.path()).unwrap();
        (tmp, secrets)
    }

    #[test]
    fn a_new_folder_has_no_key_and_no_file() {
        let (tmp, secrets) = open();
        assert_eq!(secrets.endpoint_api_key(), None);
        assert!(!secrets.endpoint_api_key_set());
        assert!(!tmp.path().join("secrets.json").exists());
    }

    #[test]
    fn a_saved_key_is_in_a_file_that_only_the_owner_reads() {
        let (tmp, secrets) = open();
        secrets.set_endpoint_api_key(Some("sk-test-123")).unwrap();
        let path = tmp.path().join("secrets.json");
        assert_eq!(mode(&path), 0o600);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("sk-test-123"));
        assert_eq!(secrets.endpoint_api_key().as_deref(), Some("sk-test-123"));
        let reopened = Secrets::open(tmp.path()).unwrap();
        assert_eq!(reopened.endpoint_api_key().as_deref(), Some("sk-test-123"));
    }

    #[test]
    fn the_mode_holds_after_a_change() {
        let (tmp, secrets) = open();
        secrets.set_endpoint_api_key(Some("first")).unwrap();
        secrets.set_endpoint_api_key(Some("second")).unwrap();
        let path = tmp.path().join("secrets.json");
        assert_eq!(mode(&path), 0o600);
        assert_eq!(secrets.endpoint_api_key().as_deref(), Some("second"));
    }

    #[test]
    fn clearing_removes_the_key_from_the_file() {
        let (tmp, secrets) = open();
        secrets.set_endpoint_api_key(Some("sk-test-123")).unwrap();
        secrets.set_endpoint_api_key(None).unwrap();
        let text = fs::read_to_string(tmp.path().join("secrets.json")).unwrap();
        assert!(!text.contains("sk-test-123"));
        assert!(!secrets.endpoint_api_key_set());
        assert_eq!(mode(&tmp.path().join("secrets.json")), 0o600);
        secrets.set_endpoint_api_key(Some("again")).unwrap();
        secrets.set_endpoint_api_key(Some("   ")).unwrap();
        assert!(!secrets.endpoint_api_key_set());
    }

    #[test]
    fn a_key_that_cannot_go_in_a_header_is_refused_without_naming_it() {
        let (_tmp, secrets) = open();
        for bad in ["two words", "line\nbreak", &"k".repeat(1000)] {
            let error = secrets.set_endpoint_api_key(Some(bad)).unwrap_err();
            assert!(!error.to_string().contains(bad));
        }
        assert!(!secrets.endpoint_api_key_set());
    }

    #[test]
    fn a_wider_mode_found_at_open_is_tightened() {
        let (tmp, secrets) = open();
        secrets.set_endpoint_api_key(Some("sk-test-123")).unwrap();
        let path = tmp.path().join("secrets.json");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let reopened = Secrets::open(tmp.path()).unwrap();
        assert_eq!(mode(&path), 0o600);
        assert_eq!(reopened.endpoint_api_key().as_deref(), Some("sk-test-123"));
    }

    #[test]
    fn a_corrupt_file_counts_as_empty_and_the_next_save_replaces_it() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("secrets.json");
        fs::write(&path, "not json").unwrap();
        let secrets = Secrets::open(tmp.path()).unwrap();
        assert!(!secrets.endpoint_api_key_set());
        secrets.set_endpoint_api_key(Some("sk-new")).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("sk-new"));
    }
}
