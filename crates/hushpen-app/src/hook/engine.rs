//! Test hook for the speech engine: the `engine` state section and the actions that run a
//! job, cancel it, and change the model. Compiled only with the `test-automation` feature.

use super::{register_action, set_state_section};
use crate::engine_host::EngineHost;
use crate::storage::Storage;
use gpui_kit::App;
use hushpen_engine::{JobOutcome, TranscribeSpec};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

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

pub fn attach(cx: &mut App, host: Rc<EngineHost>, storage: Rc<Storage>) {
    let last = LastJob::default();
    set_state_section(cx, "engine", {
        let host = Rc::clone(&host);
        let last = last.clone();
        move |_| status_json(&host, &last)
    });

    let _ = register_action(
        cx,
        "engine-transcribe",
        "Run one engine job on a WAV file. Args: {\"wav\": \"/path.wav\", \"language\": \"en\"}. \
         Returns the job id at once; the result is `engine.last_job` in `hookctl state`.",
        {
            let host = Rc::clone(&host);
            let last = last.clone();
            move |_, args| start_job(&host, &last, &args)
        },
    );
    let _ = register_action(
        cx,
        "engine-cancel",
        "Cancel the job that `engine-transcribe` started last.",
        {
            let host = Rc::clone(&host);
            let last = last.clone();
            move |_, _| {
                let job = last
                    .get()
                    .get("job")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "no job to cancel".to_string())?;
                host.client().cancel(job);
                Ok(json!({"job": job}))
            }
        },
    );
    let _ = register_action(
        cx,
        "engine-set-model",
        "Choose the speech model like the model picker does. Args: {\"modelId\": \"tiny.en\"}. \
         The running engine loads it; no restart.",
        move |_, args| {
            let id = args
                .get("modelId")
                .and_then(Value::as_str)
                .ok_or_else(|| "needs args like {\"modelId\": \"tiny.en\"}".to_string())?;
            storage
                .settings
                .set("dictation.modelId", json!(id))
                .map_err(|error| error.to_string())?;
            host.apply_settings(&storage.settings.values());
            Ok(json!({"modelId": id}))
        },
    );
}

fn status_json(host: &EngineHost, last: &LastJob) -> Value {
    let status = host.status();
    json!({
        "pid": status.pid,
        "state": status.state.as_str(),
        "model": status.model,
        "gpu": status.gpu,
        "restarts": status.restarts,
        "last_job": last.get(),
    })
}

fn start_job(host: &EngineHost, last: &LastJob, args: &Value) -> Result<Value, String> {
    let wav = args
        .get("wav")
        .and_then(Value::as_str)
        .ok_or_else(|| "needs args like {\"wav\": \"/path.wav\"}".to_string())?;
    let language = args
        .get("language")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let handle = host.client().transcribe(TranscribeSpec {
        wav_path: PathBuf::from(wav),
        language,
        prompt: None,
    });
    let job = handle.id();
    last.set(json!({"job": job, "state": "running"}));
    let last = last.clone();
    std::thread::spawn(move || {
        let result = match handle.wait() {
            JobOutcome::Done(done) => json!({
                "job": job,
                "state": "done",
                "code": "OK",
                "text": done.text,
                "language": done.language,
                "segments": done.segments.len(),
                "audio_ms": done.audio_ms,
                "decode_ms": done.decode_ms,
            }),
            JobOutcome::Cancelled => json!({"job": job, "state": "cancelled", "code": "CANCELLED"}),
            JobOutcome::Failed(failure) => json!({
                "job": job,
                "state": "failed",
                "code": failure.code,
                "detail": failure.detail,
            }),
        };
        last.set(result);
    });
    Ok(json!({"job": job}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hushpen_engine::{ChildSpec, EngineClient, EngineLog, Timing};
    use std::ffi::OsString;
    use std::time::Duration;

    // A child that reads its input and never answers: the supervisor shows it as starting.
    fn silent_host() -> EngineHost {
        let spec = ChildSpec {
            program: "/bin/sh".into(),
            args: vec![OsString::from("-c"), OsString::from("cat >/dev/null")],
        };
        let client = EngineClient::start(spec, Timing::default(), EngineLog::discard());
        EngineHost::from_client(client, PathBuf::from("/nowhere"))
    }

    fn wait_for_pid(host: &EngineHost) {
        assert!(
            host.client()
                .wait_for(Duration::from_secs(5), |status| status.pid.is_some()),
            "the child did not start"
        );
    }

    #[test]
    fn the_engine_section_has_pid_state_model_restarts_and_the_last_job() {
        let host = silent_host();
        wait_for_pid(&host);
        let section = status_json(&host, &LastJob::default());
        assert!(section["pid"].is_u64(), "{section}");
        assert_eq!(section["state"], "starting");
        assert!(section["model"].is_null());
        assert_eq!(section["restarts"], 0);
        assert!(section["last_job"].is_null());
        host.shutdown();
    }

    #[test]
    fn a_started_job_is_recorded_as_running_with_its_id() {
        let host = silent_host();
        let last = LastJob::default();
        let started = start_job(&host, &last, &json!({"wav": "/x.wav", "language": "en"})).unwrap();
        assert_eq!(last.get()["job"], started["job"]);
        assert_eq!(last.get()["state"], "running");
        host.shutdown();
    }

    #[test]
    fn a_job_without_a_wav_is_refused() {
        let host = silent_host();
        let error = start_job(&host, &LastJob::default(), &json!({})).unwrap_err();
        assert!(error.contains("wav"), "{error}");
        host.shutdown();
    }
}
