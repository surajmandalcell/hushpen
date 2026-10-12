//! Test hook for the cleanup model worker: the `llm` state section and the actions that run a
//! real cleanup job and unload the model. Compiled only with the `test-automation` feature.

use super::{record_event, register_action, set_state_section};
use crate::llm_host::LlmHost;
use crate::storage::Storage;
use gpui_kit::App;
use hushpen_core::error;
use hushpen_core::llm_prompt::CLEANUP_SYSTEM;
use hushpen_engine::{LlmOutcome, LlmWork};
use serde_json::{Value, json};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_TOKENS: u32 = 256;

#[derive(Clone, Default)]
struct LastJob(Arc<Mutex<Option<Value>>>);

impl LastJob {
    fn set(&self, value: Value) {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(value);
    }

    fn get(&self) -> Value {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .unwrap_or(Value::Null)
    }
}

pub fn attach(cx: &mut App, host: Rc<LlmHost>, storage: Rc<Storage>) {
    let last = LastJob::default();
    set_state_section(cx, "llm", {
        let host = Rc::clone(&host);
        let last = last.clone();
        move |_| status_json(&host, &last)
    });
    let _ = register_action(
        cx,
        "llm-cleanup",
        "Run one real cleanup job through the LLM worker. Args: {\"text\": \"um hello\", \
         \"timeoutMs\": 3000}. The timeout defaults to cleanup.llm.timeoutMs. Returns the job \
         id at once; the result is `llm.last_job` in `hookctl state`.",
        {
            let host = Rc::clone(&host);
            let last = last.clone();
            move |_, args| {
                let default_ms = storage
                    .settings
                    .get("cleanup.llm.timeoutMs")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(3000);
                start_job(&host, &last, &args, default_ms)
            }
        },
    );
    let _ = register_action(
        cx,
        "llm-unload",
        "Drop the model from memory now. The next job loads it again.",
        move |_, _| {
            let client = host
                .client()
                .ok_or_else(|| "the LLM worker is not running".to_string())?;
            client.unload();
            Ok(json!({}))
        },
    );
}

fn status_json(host: &LlmHost, last: &LastJob) -> Value {
    let Some(status) = host.status() else {
        return json!({"running": false, "last_job": last.get()});
    };
    json!({
        "running": true,
        "pid": status.pid,
        "state": status.state.as_str(),
        "loaded": status.model.is_some(),
        "model": status.model,
        "backend": status.backend,
        "gpu": status.gpu,
        "unloaded": status.unloaded.map(|reason| reason.as_str()),
        "restarts": status.restarts,
        "last_job": last.get(),
    })
}

fn start_job(
    host: &LlmHost,
    last: &LastJob,
    args: &Value,
    default_timeout_ms: u64,
) -> Result<Value, String> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "needs args like {\"text\": \"um hello\"}".to_string())?
        .to_owned();
    let timeout = Duration::from_millis(
        args.get("timeoutMs")
            .and_then(Value::as_u64)
            .unwrap_or(default_timeout_ms),
    );
    let client = host
        .client()
        .ok_or_else(|| "AI cleanup is off or has no verified model".to_string())?;
    let handle = client.run(LlmWork::Cleanup {
        system: CLEANUP_SYSTEM.to_owned(),
        user: text,
        max_tokens: MAX_TOKENS,
    });
    let job = handle.id();
    last.set(json!({"job": job, "state": "running"}));
    record_event("llm", &format!("job {job} started"));
    let last = last.clone();
    std::thread::spawn(move || {
        let began = Instant::now();
        let outcome = match handle.wait_timeout(timeout) {
            Some(outcome) => outcome,
            None => {
                handle.cancel();
                match handle.wait() {
                    LlmOutcome::Cancelled => LlmOutcome::Failed(hushpen_engine::Failure::new(
                        error::LLM_TIMEOUT,
                        "the cleanup timed out",
                    )),
                    other => other,
                }
            }
        };
        let ms = began.elapsed().as_millis();
        let result = match outcome {
            LlmOutcome::Done(done) => json!({
                "job": job, "state": "done", "outcome": "used", "code": "OK",
                "text": done.text, "ms": ms,
                "prompt_tokens": done.stats.prompt_tokens,
                "reused_tokens": done.stats.reused_tokens,
                "new_tokens": done.stats.new_tokens,
                "gen_tokens": done.stats.gen_tokens,
                "load_ms": done.stats.load_ms,
            }),
            LlmOutcome::Cancelled => json!({
                "job": job, "state": "cancelled", "outcome": "error", "code": "CANCELLED", "ms": ms,
            }),
            LlmOutcome::Failed(failure) => {
                let outcome = if failure.code == error::LLM_TIMEOUT {
                    "timeout"
                } else {
                    "error"
                };
                json!({
                    "job": job, "state": "failed", "outcome": outcome, "code": failure.code,
                    "detail": failure.detail, "ms": ms,
                })
            }
        };
        record_event("llm", &format!("job {job} {}", result["state"]));
        last.set(result);
    });
    Ok(json!({"job": job}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_store::data_dir::DataDir;
    use std::path::PathBuf;

    fn idle_host() -> (LlmHost, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let data = DataDir::open(dir.path()).unwrap();
        (
            LlmHost::with_program(&data, PathBuf::from("/bin/false")),
            dir,
        )
    }

    #[test]
    fn with_ai_cleanup_off_the_section_says_not_running() {
        let (host, _dir) = idle_host();
        let section = status_json(&host, &LastJob::default());
        assert_eq!(section["running"], false);
        assert!(section["last_job"].is_null());
    }

    #[test]
    fn a_job_without_a_worker_is_refused() {
        let (host, _dir) = idle_host();
        let error =
            start_job(&host, &LastJob::default(), &json!({"text": "um"}), 3000).unwrap_err();
        assert!(error.contains("off"), "{error}");
    }

    #[test]
    fn a_job_without_text_is_refused() {
        let (host, _dir) = idle_host();
        let error = start_job(&host, &LastJob::default(), &json!({}), 3000).unwrap_err();
        assert!(error.contains("text"), "{error}");
    }
}
