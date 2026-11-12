use crate::system_llm::{result, rule_text};
use crate::{CleanupResult, MobileError, lock};
use hushpen_core::catalog::{self, ModelKind};
use hushpen_core::cleanup::{guard, outcome::Outcome};
use hushpen_core::idle::Idle;
use hushpen_core::llm_prompt::{Profile, PromptFamily, Target, Tuned, cleanup_prompt};
use hushpen_engine::{EngineLog, InProcess, Link, LlmClient, LlmOutcome, LlmSpec, LlmWork, Timing};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(uniffi::Object)]
pub struct LocalCleanup {
    data: PathBuf,
    memory: u64,
    budget: AtomicU64,
    model: Mutex<Option<String>>,
    client: LlmClient,
    operation: Mutex<()>,
}

#[uniffi::export]
impl LocalCleanup {
    #[uniffi::constructor]
    pub fn new(data_dir: String, physical_memory_bytes: u64) -> Result<Arc<Self>, MobileError> {
        let data = PathBuf::from(data_dir);
        if !data.is_absolute() {
            return Err(MobileError::invalid("data folder must be absolute"));
        }
        std::fs::create_dir_all(&data).map_err(MobileError::failed)?;
        let log = EngineLog::open_scoped(&data.join("logs/local-cleanup.log"), "llm")
            .map_err(MobileError::failed)?;
        Ok(Arc::new(Self {
            data,
            memory: physical_memory_bytes,
            budget: AtomicU64::new(0),
            model: Mutex::new(None),
            client: LlmClient::start_link(
                Link::InProcess(InProcess::new(|input, output| {
                    match hushpen_llm::llama::LlamaRuntime::init() {
                        Ok(runtime) => {
                            hushpen_llm::server::serve(
                                input,
                                output,
                                hushpen_llm::server::ServerConfig {
                                    version: hushpen_core::BUILD_VERSION.into(),
                                    backend: Arc::new(runtime),
                                },
                            );
                        }
                        Err(problem) => log::error!("LOCAL_CLEANUP_INIT_FAILED {problem}"),
                    }
                })),
                Timing::llm(),
                log,
            ),
            operation: Mutex::new(()),
        }))
    }

    /// Selects a model without loading its weights.
    pub fn set_model(&self, model_id: String) -> Result<(), MobileError> {
        let _operation = self
            .operation
            .try_lock()
            .map_err(|_| MobileError::failed("cleanup engine is busy"))?;
        self.entry(&model_id)?;
        self.client.unload();
        *lock(&self.model) = Some(model_id);
        Ok(())
    }

    /// The native app refreshes this limit before a cleanup or server job.
    pub fn set_memory_budget(&self, bytes: u64) {
        self.budget.store(bytes, Ordering::Release);
    }

    pub fn available(&self) -> bool {
        lock(&self.model).as_deref().is_some_and(|id| {
            self.entry(id).is_ok_and(|entry| self.verified(entry)) && self.has_memory(id)
        })
    }

    pub fn clean(&self, raw: String, profile: String) -> Result<CleanupResult, MobileError> {
        let rule = rule_text(&raw)?;
        let profile = Profile::parse(&profile)
            .ok_or_else(|| MobileError::invalid("unknown cleanup profile"))?;
        self.run(rule, None, profile)
    }

    pub fn unload(&self) {
        self.client.unload();
        self.client
            .wait_for(Duration::from_secs(5), |status| status.model.is_none());
    }

    pub fn shutdown(&self) {
        self.client.shutdown();
    }
}

impl LocalCleanup {
    pub(crate) fn matches_model(&self, path: &str) -> bool {
        lock(&self.model).as_deref().is_some_and(|id| {
            self.entry(id).is_ok_and(|entry| {
                std::path::Path::new(path).file_name()
                    == Some(std::ffi::OsStr::new(&entry.files[0].file))
            })
        })
    }

    fn has_memory(&self, id: &str) -> bool {
        let configured = self.budget.load(Ordering::Acquire);
        let budget = crate::memory::available().map_or(configured, |live| live.min(configured));
        catalog::phone_cleanup_memory(id).is_some_and(|needed| needed <= budget)
    }

    fn entry(&self, id: &str) -> Result<&'static catalog::ModelEntry, MobileError> {
        catalog::phone_models(self.memory)
            .into_iter()
            .find(|entry| entry.kind == ModelKind::Cleanup && entry.id == id)
            .ok_or_else(|| MobileError::invalid("cleanup model does not fit this phone"))
    }

    fn path(&self, entry: &catalog::ModelEntry) -> PathBuf {
        self.data.join("models/llm").join(&entry.files[0].file)
    }

    fn verified(&self, entry: &catalog::ModelEntry) -> bool {
        let pin = &entry.files[0];
        let path = self.path(entry);
        hushpen_store::model_files::is_verified(&path)
            && hushpen_store::model_files::check(&path, pin.bytes, &pin.sha256)
                == hushpen_store::model_files::Check::Ready
    }

    pub(crate) fn clean_with_instructions(
        &self,
        text: String,
        instructions: String,
    ) -> Result<CleanupResult, MobileError> {
        self.run(text, Some(instructions), Profile::General)
    }

    pub(crate) fn clean_rule(
        &self,
        rule: String,
        profile: Profile,
    ) -> Result<CleanupResult, MobileError> {
        self.run(rule, None, profile)
    }

    fn run(
        &self,
        rule: String,
        instructions: Option<String>,
        profile: Profile,
    ) -> Result<CleanupResult, MobileError> {
        if rule.len() > 1024 * 1024
            || instructions
                .as_ref()
                .is_some_and(|value| value.len() > 1024 * 1024)
        {
            return Err(MobileError::invalid(
                "cleanup text or instructions exceed 1 MiB",
            ));
        }
        let Ok(_operation) = self.operation.try_lock() else {
            return Ok(result(rule, Outcome::Unavailable));
        };
        let Some(id) = lock(&self.model).clone() else {
            return Ok(result(rule, Outcome::Unavailable));
        };
        let entry = self.entry(&id)?;
        if !self.verified(entry) || !self.has_memory(&id) {
            return Ok(result(rule, Outcome::Unavailable));
        }
        let family = PromptFamily::from_model_name(&entry.files[0].file)
            .ok_or_else(|| MobileError::failed("unknown cleanup model family"))?;
        let spec = LlmSpec {
            family: Some(family.as_str().into()),
            gpu: hushpen_engine::gpu_available(),
            threads: hushpen_core::threads::auto_threads(
                std::thread::available_parallelism().map_or(2, usize::from),
            ),
            idle: Idle::KEEP,
            ..LlmSpec::new(self.path(entry))
        };
        let loaded = self
            .client
            .load_model(spec)
            .wait_timeout(Duration::from_secs(120));
        if !matches!(loaded, Some(Ok(_))) {
            self.unload();
            return Ok(result(
                rule,
                if loaded.is_none() {
                    Outcome::Timeout
                } else {
                    Outcome::Error
                },
            ));
        }
        let job = self.client.run(LlmWork::Cleanup {
            system: job_instructions(family, profile, instructions),
            user: rule.clone(),
            max_tokens: 512,
        });
        let outcome = job.wait_timeout(Duration::from_secs(30));
        if outcome.is_none() {
            job.cancel();
        }
        self.unload();
        Ok(match outcome {
            Some(LlmOutcome::Done(done)) => match guard::check(&rule, &done.text) {
                Ok(text) => result(text, Outcome::Used),
                Err(_) => result(rule, Outcome::GuardRejected),
            },
            Some(LlmOutcome::Failed(failure)) => result(rule, Outcome::of_failure(&failure.code)),
            Some(LlmOutcome::Cancelled) => result(rule, Outcome::Unavailable),
            None => result(rule, Outcome::Timeout),
        })
    }
}

fn job_instructions(family: PromptFamily, profile: Profile, supplied: Option<String>) -> String {
    supplied.unwrap_or_else(|| {
        cleanup_prompt(Target::Local(family), &Tuned::built_in(profile), &[]).job_system()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_profiles_use_desktop_prompts_and_hosted_jobs_keep_their_instructions() {
        assert_eq!(
            job_instructions(PromptFamily::S1Mini, Profile::Email, None),
            "[Styling: semi-formal] [Structure: prose] [Context: email]"
        );
        assert_eq!(
            job_instructions(PromptFamily::Qwen35, Profile::Notes, None),
            Profile::Notes.prompt()
        );
        for family in [PromptFamily::S1Mini, PromptFamily::Qwen35] {
            assert_eq!(
                job_instructions(family, Profile::General, Some("Desktop prompt".into())),
                "Desktop prompt"
            );
        }
    }

    #[test]
    fn cleanup_rejects_unsafe_models_and_keeps_rule_text_without_verified_weights() {
        let folder = tempfile::tempdir().unwrap();
        let local =
            LocalCleanup::new(folder.path().to_string_lossy().into(), 6_000_000_000).unwrap();
        assert!(local.set_model("../../outside".into()).is_err());
        assert!(local.set_model("qwen3.5-2b".into()).is_err());
        local.set_model("s1-mini".into()).unwrap();
        assert!(local.matches_model("/models/llm/s1-mini.gguf"));
        assert!(!local.matches_model("/models/llm/qwen3.5-2b.gguf"));
        assert!(!local.matches_model("system"));
        assert!(!local.has_memory("s1-mini"));
        let needed = catalog::phone_cleanup_memory("s1-mini").unwrap();
        local.set_memory_budget(needed - 1);
        assert!(!local.has_memory("s1-mini"));
        local.set_memory_budget(needed);
        assert!(local.has_memory("s1-mini"));
        assert!(!local.available());
        let output = local
            .clean("um hello there".into(), "general".into())
            .unwrap();
        assert_eq!(
            (output.text.as_str(), output.outcome.as_str()),
            ("Hello there.", "unavailable")
        );
        assert!(local.clean("hello".into(), "unknown".into()).is_err());
        local.shutdown();
        assert!(LocalCleanup::new("relative".into(), 6_000_000_000).is_err());
    }
}
