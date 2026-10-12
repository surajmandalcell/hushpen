//! The app's side of the cleanup model worker: starts the `hushpen-llm` child through the
//! supervisor, but only while AI cleanup is on and a verified model file is there. Nothing runs
//! otherwise, so a user who never turns the feature on never starts a model process.
//!
//! A worker crash, hang, or missing model never reaches the user as an error. A job ends in an
//! outcome the cleanup provider turns into the rule text.

use hushpen_engine::{ChildSpec, EngineLog, LlmClient, LlmSpec, LlmStatus, Timing};
use hushpen_store::data_dir::DataDir;
use hushpen_store::model_files;
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The worker binary's file name, beside the app's own executable.
#[cfg(not(windows))]
const WORKER_NAME: &str = "hushpen-llm";
#[cfg(windows)]
const WORKER_NAME: &str = "hushpen-llm.exe";

struct State {
    client: Option<LlmClient>,
    /// The load the running worker was last given.
    applied: Option<LlmSpec>,
}

pub struct LlmHost {
    models_dir: PathBuf,
    log_path: PathBuf,
    program: PathBuf,
    state: Mutex<State>,
}

impl LlmHost {
    /// Starts nothing. [`LlmHost::apply_settings`] starts the worker when the settings ask for it.
    pub fn new(data: &DataDir) -> Self {
        Self::with_program(data, worker_path())
    }

    pub(crate) fn with_program(data: &DataDir, program: PathBuf) -> Self {
        Self {
            models_dir: data.llm_models_dir(),
            log_path: data.llm_log_path(),
            program,
            state: Mutex::new(State {
                client: None,
                applied: None,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The running worker, or `None` while AI cleanup is off or has no model.
    pub fn client(&self) -> Option<LlmClient> {
        self.lock().client.clone()
    }

    /// The running worker with the load it was given, or `None` while none runs.
    pub fn running(&self) -> Option<(LlmClient, LlmSpec)> {
        let state = self.lock();
        Some((state.client.clone()?, state.applied.clone()?))
    }

    pub fn status(&self) -> Option<LlmStatus> {
        self.client().map(|client| client.status())
    }

    /// Starts, reloads, or stops the worker to match the settings. Safe to call often: it
    /// changes nothing when the wanted load is the one the worker already has.
    pub fn apply_settings(&self, settings: &Map<String, Value>) {
        let cpus = std::thread::available_parallelism().map_or(2, usize::from);
        let wanted = wanted_load(settings, &self.models_dir, cpus);
        let mut state = self.lock();
        let Some(wanted) = wanted else {
            if let Some(client) = state.client.take() {
                state.applied = None;
                // Waiting for the child to exit must not hold the lock or the UI thread.
                std::thread::spawn(move || client.shutdown());
            }
            return;
        };
        if state.applied.as_ref() == Some(&wanted) && state.client.is_some() {
            return;
        }
        let client = state
            .client
            .get_or_insert_with(|| self.start_client())
            .clone();
        state.applied = Some(wanted.clone());
        let handle = client.load_model(wanted.clone());
        // The wait must not block the UI thread; the outcome needs a log line.
        std::thread::spawn(move || match handle.wait() {
            Ok(done) => log::info!(
                "LLM_MODEL_LOADED model={} backend={} gpu={} ms={}",
                done.model,
                done.backend,
                done.gpu,
                done.ms
            ),
            Err(failure) => log::warn!(
                "LLM_MODEL_FAILED {} {} ({})",
                failure.code,
                failure.detail,
                wanted.path.display()
            ),
        });
    }

    fn start_client(&self) -> LlmClient {
        let log = EngineLog::open_scoped(&self.log_path, "llm").unwrap_or_else(|error| {
            log::warn!("LLM_LOG_OFF could not open llm.log: {error}");
            EngineLog::discard()
        });
        let spec = ChildSpec {
            program: self.program.clone(),
            args: Vec::<OsString>::new(),
        };
        LlmClient::start(spec, Timing::default(), log)
    }

    /// Stops the worker. Safe to call twice.
    pub fn shutdown(&self) {
        let client = {
            let mut state = self.lock();
            state.applied = None;
            state.client.take()
        };
        if let Some(client) = client {
            client.shutdown();
        }
    }
}

/// The worker binary: beside the app's resolved executable. Debug builds also honor
/// `HUSHPEN_LLM_BIN`, so a test build can point at a freshly built worker.
fn worker_path() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("HUSHPEN_LLM_BIN").filter(|path| !path.is_empty()) {
        return PathBuf::from(path);
    }
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("hushpen"));
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.with_file_name(WORKER_NAME)
}

/// The load the settings ask for, or `None` while AI cleanup is off, uses the endpoint, names no
/// model, or the model file is not there with a matching `.verified` stamp.
pub fn wanted_load(
    settings: &Map<String, Value>,
    models_dir: &Path,
    cpus: usize,
) -> Option<LlmSpec> {
    let on = settings
        .get("cleanup.llm.enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let local = settings
        .get("cleanup.llm.provider")
        .and_then(Value::as_str)
        .is_none_or(|provider| provider == "local");
    if !on || !local {
        return None;
    }
    let id = settings
        .get("cleanup.llm.modelId")
        .and_then(Value::as_str)
        .map_or("", str::trim);
    if id.is_empty() || id.contains(['/', '\\']) {
        return None;
    }
    let path = models_dir.join(format!("{id}.gguf"));
    if !is_verified(&path) {
        return None;
    }
    let minutes = settings
        .get("cleanup.llm.idleUnloadMinutes")
        .and_then(Value::as_u64)
        .unwrap_or(5);
    let gpu = settings
        .get("engine.useGpu")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Some(LlmSpec {
        path,
        family: None,
        gpu,
        threads: cpus.clamp(1, 8),
        idle_secs: minutes.saturating_mul(60),
    })
}

/// A stamp exists and still matches the file's size and modification time.
fn is_verified(path: &Path) -> bool {
    let Some(stamp) = model_files::read_stamp(path) else {
        return false;
    };
    let size = std::fs::metadata(path).map_or(0, |meta| meta.len());
    size > 0 && model_files::check(path, size, &stamp.hash) == model_files::Check::Ready
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    fn settings(values: Value) -> Map<String, Value> {
        let Value::Object(map) = values else {
            unreachable!()
        };
        map
    }

    fn stamped_model(dir: &Path, id: &str) -> PathBuf {
        let path = dir.join(format!("{id}.gguf"));
        std::fs::write(&path, b"GGUF fake").unwrap();
        model_files::write_stamp(&path, "abc").unwrap();
        path
    }

    fn on(id: &str) -> Map<String, Value> {
        settings(json!({"cleanup.llm.enabled": true, "cleanup.llm.modelId": id}))
    }

    #[test]
    fn ai_cleanup_off_wants_no_worker() {
        let dir = tempfile::tempdir().unwrap();
        stamped_model(dir.path(), "m");
        let off = settings(json!({"cleanup.llm.enabled": false, "cleanup.llm.modelId": "m"}));
        assert_eq!(wanted_load(&off, dir.path(), 4), None);
        assert_eq!(wanted_load(&Map::new(), dir.path(), 4), None);
    }

    #[test]
    fn the_endpoint_provider_wants_no_local_worker() {
        let dir = tempfile::tempdir().unwrap();
        stamped_model(dir.path(), "m");
        let mut values = on("m");
        values.insert("cleanup.llm.provider".into(), json!("endpoint"));
        assert_eq!(wanted_load(&values, dir.path(), 4), None);
    }

    #[test]
    fn a_missing_or_unstamped_model_wants_no_worker() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(wanted_load(&on("m"), dir.path(), 4), None);
        std::fs::write(dir.path().join("m.gguf"), b"GGUF fake").unwrap();
        assert_eq!(wanted_load(&on("m"), dir.path(), 4), None);
        assert_eq!(wanted_load(&on(""), dir.path(), 4), None);
        assert_eq!(wanted_load(&on("../m"), dir.path(), 4), None);
    }

    #[test]
    fn a_changed_file_loses_its_stamp() {
        let dir = tempfile::tempdir().unwrap();
        let path = stamped_model(dir.path(), "m");
        assert!(wanted_load(&on("m"), dir.path(), 4).is_some());
        std::fs::write(&path, b"GGUF fake but longer now").unwrap();
        assert_eq!(wanted_load(&on("m"), dir.path(), 4), None);
    }

    #[test]
    fn a_verified_model_gives_the_load_with_the_idle_time_in_seconds() {
        let dir = tempfile::tempdir().unwrap();
        let path = stamped_model(dir.path(), "Qwen3.5-2B-Q4_K_M");
        let mut values = on("Qwen3.5-2B-Q4_K_M");
        values.insert("cleanup.llm.idleUnloadMinutes".into(), json!(7));
        values.insert("engine.useGpu".into(), json!(true));
        let wanted = wanted_load(&values, dir.path(), 16).unwrap();
        assert_eq!(wanted.path, path);
        assert_eq!(wanted.idle_secs, 420);
        assert!(wanted.gpu);
        assert_eq!(wanted.threads, 8);
        let default_idle = wanted_load(&on("Qwen3.5-2B-Q4_K_M"), dir.path(), 2).unwrap();
        assert_eq!(default_idle.idle_secs, 300);
        assert_eq!(default_idle.threads, 2);
    }

    #[test]
    fn the_worker_starts_only_when_asked_and_stops_when_turned_off() {
        let data_dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(data_dir.path()).unwrap();
        let models = data.llm_models_dir();
        std::fs::create_dir_all(&models).unwrap();
        stamped_model(&models, "m");
        let host = LlmHost::with_program(&data, PathBuf::from("/bin/sh"));
        assert!(host.status().is_none(), "nothing runs before the setting");
        host.apply_settings(&settings(json!({"cleanup.llm.enabled": false})));
        assert!(host.client().is_none());
        host.apply_settings(&on("m"));
        let client = host.client().expect("the worker starts");
        assert!(client.wait_for(Duration::from_secs(5), |s| s.pid.is_some()));
        host.apply_settings(&settings(json!({"cleanup.llm.enabled": false})));
        assert!(host.client().is_none());
        host.shutdown();
    }
}
