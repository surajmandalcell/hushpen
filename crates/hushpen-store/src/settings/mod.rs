//! `config/settings.json`: `{"schemaVersion": 2, "values": {...}}`.
//!
//! Values are flat dotted keys from the registry. Keys the registry does not
//! know are kept under `values._unknown`. Every write goes through a temp
//! file and a rename, so a crash never leaves a half-written file.

mod registry;

use crate::data_dir::Os;
use crate::{Error, Result, atomic, time};
use registry::{REGISTRY, Spec};
use serde_json::{Map, Value, json};
use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

/// 2: the Input boost default went from 100% to 150% and the Sound volume default from 50% to
/// 25%. Older files stored every default, so [`OLD_DEFAULTS`] values are dropped once.
const SCHEMA_VERSION: i64 = 2;
const UNKNOWN_KEY: &str = "_unknown";

/// Defaults that changed in schema version 2. A stored value that still equals one was never
/// chosen by the user, so the new default applies.
const OLD_DEFAULTS: [(&str, f64); 2] = [("audio.inputGain", 1.0), ("audio.cueVolume", 0.5)];

/// The smallest and the largest number the registry accepts for `key`, or `None` when the key
/// is not numeric.
pub fn range(key: &str) -> Option<RangeInclusive<f64>> {
    registry::find(key)?.range()
}

pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<State>,
}

#[derive(Clone)]
struct State {
    schema_version: i64,
    values: Map<String, Value>,
    unknown: Map<String, Value>,
}

impl State {
    fn defaults() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            values: REGISTRY
                .iter()
                .map(|spec| (spec.key.to_string(), spec.default_value(Os::CURRENT)))
                .collect(),
            unknown: Map::new(),
        }
    }

    fn to_document(&self) -> Value {
        let mut values = self.values.clone();
        if !self.unknown.is_empty() {
            values.insert(UNKNOWN_KEY.into(), Value::Object(self.unknown.clone()));
        }
        json!({"schemaVersion": self.schema_version, "values": values})
    }
}

impl SettingsStore {
    /// Opens `<config_dir>/settings.json`. A missing file is created with
    /// defaults. A file that is not a settings document is renamed to
    /// `settings.json.corrupt-<ms>`, defaults are written, and one
    /// `SETTINGS_CORRUPT` line is logged.
    pub fn open(config_dir: &Path) -> Result<Self> {
        fs::create_dir_all(config_dir)
            .map_err(|e| Error::io(format!("could not create {}", config_dir.display()), e))?;
        let path = config_dir.join("settings.json");
        let (state, needs_write) = match fs::read(&path) {
            Ok(bytes) => match parse(&bytes) {
                Some((state, changed)) => (state, changed),
                None => {
                    back_up_corrupt(&path)?;
                    (State::defaults(), true)
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => (State::defaults(), true),
            Err(error) => {
                return Err(Error::io(
                    format!("could not read {}", path.display()),
                    error,
                ));
            }
        };
        let store = Self {
            path,
            state: Mutex::new(state),
        };
        if needs_write {
            store.persist(&store.lock())?;
        }
        Ok(store)
    }

    pub fn get(&self, key: &str) -> Option<Value> {
        self.lock().values.get(key).cloned()
    }

    /// A milliseconds setting such as `cleanup.llm.timeoutMs`. The registry default is the one
    /// fallback, for a key that holds no number.
    pub fn millis(&self, key: &str) -> Duration {
        let ms = self
            .get(key)
            .or_else(|| Some(registry::find(key)?.default_value(Os::CURRENT)))
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        Duration::from_millis(ms)
    }

    /// A copy of every known key and its current value.
    pub fn values(&self) -> Map<String, Value> {
        self.lock().values.clone()
    }

    /// Sets a user-facing key. Internal keys are refused.
    pub fn set(&self, key: &str, value: Value) -> Result<()> {
        self.set_checked(key, value, false)
    }

    /// Sets an internal key such as `onboarding.step`. Views never call this.
    pub fn set_internal(&self, key: &str, value: Value) -> Result<()> {
        self.set_checked(key, value, true)
    }

    pub fn unknown(&self) -> Map<String, Value> {
        self.lock().unknown.clone()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn set_checked(&self, key: &str, value: Value, internal: bool) -> Result<()> {
        let spec = registry::find(key).ok_or_else(|| Error::UnknownSetting(key.into()))?;
        if spec.internal != internal {
            return Err(Error::InvalidSetting(key.into()));
        }
        let cleaned = spec
            .sanitize(&value)
            .ok_or_else(|| Error::InvalidSetting(key.into()))?;
        let mut state = self.lock();
        let mut next = state.clone();
        next.values.insert(key.into(), cleaned);
        // Picking a position preset replaces a dragged position; both land in one write.
        if key == "overlay.position" {
            next.values.insert("overlay.customPos".into(), Value::Null);
        }
        self.persist(&next)?;
        *state = next;
        Ok(())
    }

    fn persist(&self, state: &State) -> Result<()> {
        let mut text = serde_json::to_string_pretty(&state.to_document())
            .map_err(|e| Error::io("could not encode settings", e.into()))?;
        text.push('\n');
        atomic::write(&self.path, text.as_bytes())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Parses a settings document. `None` means it is not one (corrupt). The flag
/// says whether cleaning changed anything, so the file should be rewritten.
fn parse(bytes: &[u8]) -> Option<(State, bool)> {
    let document: Value = serde_json::from_slice(bytes).ok()?;
    let schema_version = document
        .get("schemaVersion")?
        .as_i64()
        .filter(|v| *v >= 1)?;
    let stored = document.get("values")?.as_object()?;

    let mut changed = false;
    let mut state = State::defaults();
    // A file from a newer build keeps its version; an older one is migrated and stamped.
    state.schema_version = schema_version.max(SCHEMA_VERSION);
    changed |= schema_version < SCHEMA_VERSION;
    if let Some(Value::Object(previous)) = stored.get(UNKNOWN_KEY) {
        state.unknown = previous.clone();
    } else if stored.contains_key(UNKNOWN_KEY) {
        changed = true;
    }
    for (key, value) in stored {
        if key == UNKNOWN_KEY {
            continue;
        }
        match registry::find(key) {
            Some(spec) => match spec.sanitize(value) {
                Some(_)
                    if schema_version < SCHEMA_VERSION
                        && OLD_DEFAULTS.iter().any(|(old_key, old)| {
                            key == old_key && value.as_f64() == Some(*old)
                        }) =>
                {
                    changed = true;
                }
                Some(cleaned) => {
                    changed |= &cleaned != value;
                    state.values.insert(key.clone(), cleaned);
                }
                None => {
                    log::warn!("SETTINGS_VALUE_RESET {key}");
                    changed = true;
                }
            },
            None => {
                state.unknown.insert(key.clone(), value.clone());
                changed = true;
            }
        }
    }
    changed |= REGISTRY
        .iter()
        .any(|Spec { key, .. }| !stored.contains_key(*key));
    Some((state, changed))
}

fn back_up_corrupt(path: &Path) -> Result<()> {
    let mut stamp = time::now_unix_ms();
    let backup = loop {
        let candidate = PathBuf::from(format!("{}.corrupt-{stamp}", path.display()));
        if !candidate.exists() {
            break candidate;
        }
        stamp += 1;
    };
    fs::rename(path, &backup).map_err(|e| {
        Error::io(
            format!("could not back up the corrupt {}", path.display()),
            e,
        )
    })?;
    log::warn!(
        "SETTINGS_CORRUPT settings.json was not valid; kept as {} and reset to defaults",
        backup.file_name().unwrap_or_default().to_string_lossy()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_and_idle_choices_survive_reopening_the_settings() {
        let root = tempfile::tempdir().unwrap();
        let settings = SettingsStore::open(root.path()).unwrap();
        settings
            .set("audio.muteWhileDictating", json!(false))
            .unwrap();
        settings.set("engine.idlePolicy", json!("pageout")).unwrap();
        settings.set("engine.idleMinutes", json!(30)).unwrap();
        drop(settings);
        let settings = SettingsStore::open(root.path()).unwrap();
        assert_eq!(settings.get("audio.muteWhileDictating"), Some(json!(false)));
        assert_eq!(settings.get("engine.idlePolicy"), Some(json!("pageout")));
        assert_eq!(settings.get("engine.idleMinutes"), Some(json!(30)));
    }
}
