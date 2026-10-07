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
use std::sync::{Arc, Mutex};

/// Why the last model load failed. Home shows it instead of a record button that cannot work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    pub model: String,
    pub code: String,
    /// The processor lacks an instruction set the speech engine needs.
    pub unsupported_cpu: bool,
}

pub struct EngineHost {
    client: EngineClient,
    models_dir: PathBuf,
    /// The catalog default, which a load uses while `dictation.modelId` is empty.
    default_model: String,
    loaded: Mutex<Option<LoadSpec>>,
    problem: Arc<Mutex<Option<LoadProblem>>>,
}

impl EngineHost {
    /// Starts the child with no model. The model library loads the chosen model through
    /// `apply_settings` once the file has passed its hash check.
    /// A child that cannot start is retried by the supervisor; this never fails.
    pub fn start(data: &DataDir, default_model: &str) -> Self {
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
            default_model: default_model.to_owned(),
            loaded: Mutex::new(None),
            problem: Arc::default(),
        }
    }

    #[cfg(all(test, feature = "test-automation"))]
    pub(crate) fn from_client(client: EngineClient, models_dir: PathBuf) -> Self {
        Self {
            client,
            models_dir,
            default_model: String::new(),
            loaded: Mutex::new(None),
            problem: Arc::default(),
        }
    }

    /// The failure of the last model load, until a load works.
    pub fn load_problem(&self) -> Option<LoadProblem> {
        self.problem
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
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
        let Some(wanted) = wanted_load(settings, &self.models_dir, &self.default_model, cpus)
        else {
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
        *self
            .problem
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        let handle = self.client.load_model(wanted.clone());
        let problem = Arc::clone(&self.problem);
        // The wait must not block the UI thread; the outcome needs a log line and the problem.
        std::thread::spawn(move || match handle.wait() {
            Ok(done) => log::info!(
                "ENGINE_MODEL_LOADED model={} gpu={} ms={}",
                done.model,
                done.gpu,
                done.ms
            ),
            Err(failure) => {
                log::warn!(
                    "ENGINE_MODEL_FAILED {} {} ({})",
                    failure.code,
                    failure.detail,
                    wanted.path.display()
                );
                *problem
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(LoadProblem {
                    model: model_id(&wanted.path),
                    unsupported_cpu: failure.is_unsupported_cpu(),
                    code: failure.code,
                });
            }
        });
    }

    /// Stops the child. Safe to call twice.
    pub fn shutdown(&self) {
        self.client.shutdown();
    }
}

/// The model id in `ggml-<id>.bin`.
fn model_id(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy())
        .and_then(|name| {
            name.strip_prefix("ggml-")
                .and_then(|rest| rest.strip_suffix(".bin"))
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

/// The load the settings ask for, or `None` while no model is chosen and the catalog has no
/// default. An empty `dictation.modelId` means the catalog default (`default_model`).
pub fn wanted_load(
    settings: &Map<String, Value>,
    models_dir: &Path,
    default_model: &str,
    cpus: usize,
) -> Option<LoadSpec> {
    let chosen = settings
        .get("dictation.modelId")
        .and_then(Value::as_str)
        .map_or("", str::trim);
    let id = if chosen.is_empty() {
        default_model
    } else {
        chosen
    };
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

    fn wanted(settings: &Map<String, Value>, cpus: usize) -> Option<LoadSpec> {
        wanted_load(settings, Path::new("/m"), "", cpus)
    }

    #[test]
    fn no_model_chosen_and_no_default_means_no_load() {
        assert_eq!(wanted(&settings("", json!("auto"), false), 4), None);
    }

    #[test]
    fn an_empty_model_id_uses_the_catalog_default() {
        let spec = wanted_load(
            &settings("", json!("auto"), false),
            Path::new("/m"),
            "base",
            2,
        );
        assert_eq!(spec.unwrap().path, Path::new("/m/ggml-base.bin"));
    }

    #[test]
    fn a_chosen_model_beats_the_default() {
        let spec = wanted_load(
            &settings("tiny.en", json!("auto"), false),
            Path::new("/m"),
            "base",
            2,
        );
        assert_eq!(spec.unwrap().path, Path::new("/m/ggml-tiny.en.bin"));
    }

    #[test]
    fn the_model_id_names_the_ggml_file() {
        let spec = wanted(&settings("tiny.en", json!("auto"), false), 2);
        assert_eq!(spec.unwrap().path, Path::new("/m/ggml-tiny.en.bin"));
        assert_eq!(model_id(Path::new("/m/ggml-tiny.en.bin")), "tiny.en");
    }

    #[test]
    fn auto_threads_follow_the_cpu_count_and_a_number_wins() {
        let auto = wanted(&settings("t", json!("auto"), false), 2).unwrap();
        assert_eq!(auto.threads, 2);
        let set = wanted(&settings("t", json!(3), false), 2).unwrap();
        assert_eq!(set.threads, 3);
    }

    #[test]
    fn the_gpu_setting_is_passed_on() {
        let on = wanted(&settings("t", json!("auto"), true), 2).unwrap();
        assert!(on.gpu);
        let off = wanted(&settings("t", json!("auto"), false), 2).unwrap();
        assert!(!off.gpu);
    }
}
