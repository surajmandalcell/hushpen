//! The app's side of the speech engine: starts the `hushpen engine` child through the
//! supervisor, and loads the model the settings name once the model library has verified it.
//!
//! The UI never loads a model itself. A crash of the child fails the running job and nothing
//! else; the supervisor restarts it and reloads the last model.

use hushpen_core::threads::resolve_threads;
use hushpen_engine::{ChildSpec, EngineClient, EngineLog, LoadSpec, Status, Timing};
use hushpen_store::data_dir::DataDir;
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct EngineHost {
    client: EngineClient,
    models_dir: PathBuf,
    loaded: Mutex<Option<LoadSpec>>,
}

impl EngineHost {
    /// Starts the child with no model. The model library loads the chosen model through
    /// `apply_settings` once the file has passed its hash check.
    /// A child that cannot start is retried by the supervisor; this never fails.
    pub fn start(data: &DataDir) -> Self {
        let log = EngineLog::open(&data.engine_log_path()).unwrap_or_else(|error| {
            log::warn!("ENGINE_LOG_OFF could not open engine.log: {error}");
            EngineLog::discard()
        });
        let program = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hushpen"));
        let spec = ChildSpec {
            program,
            args: vec![OsString::from("engine")],
        };
        Self {
            client: EngineClient::start(spec, Timing::default(), log),
            models_dir: data.whisper_models_dir(),
            loaded: Mutex::new(None),
        }
    }

    #[cfg(all(test, feature = "test-automation"))]
    pub(crate) fn from_client(client: EngineClient, models_dir: PathBuf) -> Self {
        Self {
            client,
            models_dir,
            loaded: Mutex::new(None),
        }
    }

    pub fn client(&self) -> &EngineClient {
        &self.client
    }

    pub fn status(&self) -> Status {
        self.client.status()
    }

    /// Loads the model, GPU choice, and thread count the settings ask for when they differ from
    /// what the child already holds. The change takes effect in the running child.
    pub fn apply_settings(&self, settings: &Map<String, Value>) {
        let cpus = std::thread::available_parallelism().map_or(2, usize::from);
        let Some(wanted) = wanted_load(settings, &self.models_dir, cpus) else {
            return;
        };
        {
            let mut loaded = self
                .loaded
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if loaded.as_ref() == Some(&wanted) {
                return;
            }
            *loaded = Some(wanted.clone());
        }
        let handle = self.client.load_model(wanted.clone());
        // The wait must not block the UI thread; the outcome only needs a log line.
        std::thread::spawn(move || match handle.wait() {
            Ok(done) => log::info!(
                "ENGINE_MODEL_LOADED model={} gpu={} ms={}",
                done.model,
                done.gpu,
                done.ms
            ),
            Err(failure) => log::warn!(
                "ENGINE_MODEL_FAILED {} {} ({})",
                failure.code,
                failure.detail,
                wanted.path.display()
            ),
        });
    }

    /// Stops the child. Safe to call twice.
    pub fn shutdown(&self) {
        self.client.shutdown();
    }
}

/// The load the settings ask for, or `None` while no model is chosen.
pub fn wanted_load(
    settings: &Map<String, Value>,
    models_dir: &Path,
    cpus: usize,
) -> Option<LoadSpec> {
    let id = settings.get("dictation.modelId")?.as_str()?.trim();
    if id.is_empty() {
        return None;
    }
    let use_gpu = settings
        .get("engine.useGpu")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(LoadSpec {
        path: models_dir.join(format!("ggml-{id}.bin")),
        gpu: use_gpu,
        threads: resolve_threads(settings.get("engine.threads"), cpus),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(model: &str, threads: Value, gpu: bool) -> Map<String, Value> {
        let Value::Object(map) = json!({
            "dictation.modelId": model,
            "engine.threads": threads,
            "engine.useGpu": gpu,
        }) else {
            unreachable!()
        };
        map
    }

    #[test]
    fn no_model_chosen_means_no_load() {
        assert_eq!(
            wanted_load(&settings("", json!("auto"), false), Path::new("/m"), 4),
            None
        );
    }

    #[test]
    fn the_model_id_names_the_ggml_file() {
        let wanted = wanted_load(
            &settings("tiny.en", json!("auto"), false),
            Path::new("/m"),
            2,
        );
        assert_eq!(wanted.unwrap().path, Path::new("/m/ggml-tiny.en.bin"));
    }

    #[test]
    fn auto_threads_follow_the_cpu_count_and_a_number_wins() {
        let auto = wanted_load(&settings("t", json!("auto"), false), Path::new("/m"), 2).unwrap();
        assert_eq!(auto.threads, 2);
        let set = wanted_load(&settings("t", json!(3), false), Path::new("/m"), 2).unwrap();
        assert_eq!(set.threads, 3);
    }

    #[test]
    fn the_gpu_setting_is_passed_on() {
        let on = wanted_load(&settings("t", json!("auto"), true), Path::new("/m"), 2).unwrap();
        assert!(on.gpu);
        let off = wanted_load(&settings("t", json!("auto"), false), Path::new("/m"), 2).unwrap();
        assert!(!off.gpu);
    }
}
