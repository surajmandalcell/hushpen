//! The pipe protocol between the app and the `hushpen-llm` worker.
//!
//! It uses the frame codec of [`crate::protocol`]. Control messages are JSON objects with a
//! `type` field. The text of a dictation travels inside `Cleanup` and `Result`; no log line
//! ever holds it.

use crate::protocol::{DecodeError, Fields, Frame, Params, parse_payload};
use serde_json::{Map, Value, json};

/// Bumped when a message changes shape. `Hello` carries it and `Ready` answers with it.
pub const LLM_PROTOCOL_VERSION: u32 = 1;

/// Default idle time before the worker drops the model, in seconds.
pub const DEFAULT_IDLE_SECS: u64 = 300;

/// App to worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmRequest {
    Hello {
        protocol: u32,
    },
    /// Loads (or reloads) a model. The worker keeps the settings, so after an idle unload the
    /// next job loads the same model again without a new `LoadModel`.
    LoadModel {
        path: String,
        /// `None` picks the prompt family from the file name.
        family: Option<String>,
        gpu: bool,
        threads: u32,
        /// Seconds without a job before the model is dropped. `0` keeps it for good.
        idle_secs: u64,
    },
    /// Cleans one dictation. `system` is the fixed prompt, which the worker caches.
    Cleanup {
        job: u64,
        system: String,
        user: String,
        max_tokens: u32,
    },
    /// Edits a selection by an instruction (Command Mode).
    Command {
        job: u64,
        instruction: String,
        selection: String,
        max_tokens: u32,
    },
    /// Drops the model now. A later job loads it again.
    Unload,
    Cancel {
        job: u64,
    },
    Ping,
    Shutdown,
}

impl LlmRequest {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Hello { protocol } => json!({"type": "hello", "protocol": protocol}),
            Self::LoadModel {
                path,
                family,
                gpu,
                threads,
                idle_secs,
            } => json!({
                "type": "load_model", "path": path, "family": family, "gpu": gpu,
                "threads": threads, "idle_secs": idle_secs,
            }),
            Self::Cleanup {
                job,
                system,
                user,
                max_tokens,
            } => json!({
                "type": "cleanup", "job": job, "system": system, "user": user,
                "max_tokens": max_tokens,
            }),
            Self::Command {
                job,
                instruction,
                selection,
                max_tokens,
            } => json!({
                "type": "command", "job": job, "instruction": instruction,
                "selection": selection, "max_tokens": max_tokens,
            }),
            Self::Unload => json!({"type": "unload"}),
            Self::Cancel { job } => json!({"type": "cancel", "job": job}),
            Self::Ping => json!({"type": "ping"}),
            Self::Shutdown => json!({"type": "shutdown"}),
        }
    }

    pub fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        match fields.kind()?.as_str() {
            "hello" => Ok(Self::Hello {
                protocol: fields.u32("protocol")?,
            }),
            "load_model" => Ok(Self::LoadModel {
                path: fields.string("path")?,
                family: fields.optional_string("family")?,
                gpu: fields.bool("gpu")?,
                threads: fields.u32("threads")?,
                idle_secs: fields
                    .optional_u64("idle_secs")?
                    .unwrap_or(DEFAULT_IDLE_SECS),
            }),
            "cleanup" => Ok(Self::Cleanup {
                job: fields.u64("job")?,
                system: fields.string("system")?,
                user: fields.string("user")?,
                max_tokens: fields.u32("max_tokens")?,
            }),
            "command" => Ok(Self::Command {
                job: fields.u64("job")?,
                instruction: fields.string("instruction")?,
                selection: fields.string("selection")?,
                max_tokens: fields.u32("max_tokens")?,
            }),
            "unload" => Ok(Self::Unload),
            "cancel" => Ok(Self::Cancel {
                job: fields.u64("job")?,
            }),
            "ping" => Ok(Self::Ping),
            "shutdown" => Ok(Self::Shutdown),
            other => Err(DecodeError::UnknownType(other.to_owned())),
        }
    }

    pub fn from_frame(frame: &Frame) -> Result<Self, DecodeError> {
        Self::from_json(&parse_payload(frame)?)
    }
}

/// What a finished job cost, for `llm.log`. Counts only, never text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JobStats {
    /// Tokens in the whole prompt.
    pub prompt_tokens: u32,
    /// Leading prompt tokens that the cache already held, so they were not processed again.
    pub reused_tokens: u32,
    /// Prompt tokens the model processed for this job (`prompt_tokens - reused_tokens`).
    pub new_tokens: u32,
    /// Tokens the model wrote.
    pub gen_tokens: u32,
    /// Time from the request to the result, including a reload after an idle unload.
    pub ms: u64,
    /// Part of `ms` spent loading the model, `0` when it was warm.
    pub load_ms: u64,
}

impl JobStats {
    fn put(&self, object: &mut Value) {
        object["prompt_tokens"] = json!(self.prompt_tokens);
        object["reused_tokens"] = json!(self.reused_tokens);
        object["new_tokens"] = json!(self.new_tokens);
        object["gen_tokens"] = json!(self.gen_tokens);
        object["ms"] = json!(self.ms);
        object["load_ms"] = json!(self.load_ms);
    }

    fn read(fields: &Fields<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            prompt_tokens: fields.u32("prompt_tokens")?,
            reused_tokens: fields.u32("reused_tokens")?,
            new_tokens: fields.u32("new_tokens")?,
            gen_tokens: fields.u32("gen_tokens")?,
            ms: fields.u64("ms")?,
            load_ms: fields.u64("load_ms")?,
        })
    }
}

/// Why the worker dropped the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnloadReason {
    Idle,
    Requested,
}

impl UnloadReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Requested => "requested",
        }
    }

    fn parse(text: &str) -> Result<Self, DecodeError> {
        match text {
            "idle" => Ok(Self::Idle),
            "requested" => Ok(Self::Requested),
            other => Err(DecodeError::Invalid(format!(
                "unknown unload reason '{other}'"
            ))),
        }
    }
}

/// Worker to app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmEvent {
    Ready {
        version: String,
        protocol: u32,
        /// `metal` or `cpu`: the backend this build uses when the app asks for the GPU.
        backend: String,
    },
    Loading {
        model: String,
    },
    Loaded {
        model: String,
        ms: u64,
        gpu: bool,
        backend: String,
    },
    Unloaded {
        reason: UnloadReason,
    },
    Result {
        job: u64,
        text: String,
        stats: JobStats,
    },
    Cancelled {
        job: u64,
    },
    Error {
        /// `None` for an error that belongs to no job, such as a failed model load.
        job: Option<u64>,
        code: String,
        params: Params,
    },
    Pong,
}

impl LlmEvent {
    pub fn error(job: Option<u64>, code: &str, detail: impl Into<String>) -> Self {
        let mut params = Map::new();
        params.insert("detail".into(), Value::String(detail.into()));
        Self::Error {
            job,
            code: code.to_owned(),
            params,
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Ready {
                version,
                protocol,
                backend,
            } => json!({
                "type": "ready", "version": version, "protocol": protocol, "backend": backend,
            }),
            Self::Loading { model } => json!({"type": "loading", "model": model}),
            Self::Loaded {
                model,
                ms,
                gpu,
                backend,
            } => json!({
                "type": "loaded", "model": model, "ms": ms, "gpu": gpu, "backend": backend,
            }),
            Self::Unloaded { reason } => json!({"type": "unloaded", "reason": reason.as_str()}),
            Self::Result { job, text, stats } => {
                let mut value = json!({"type": "result", "job": job, "text": text});
                stats.put(&mut value);
                value
            }
            Self::Cancelled { job } => json!({"type": "cancelled", "job": job}),
            Self::Error { job, code, params } => {
                json!({"type": "error", "job": job, "code": code, "params": params})
            }
            Self::Pong => json!({"type": "pong"}),
        }
    }

    pub fn from_json(value: &Value) -> Result<Self, DecodeError> {
        let fields = Fields::of(value)?;
        match fields.kind()?.as_str() {
            "ready" => Ok(Self::Ready {
                version: fields.string("version")?,
                protocol: fields.u32("protocol")?,
                backend: fields.string("backend")?,
            }),
            "loading" => Ok(Self::Loading {
                model: fields.string("model")?,
            }),
            "loaded" => Ok(Self::Loaded {
                model: fields.string("model")?,
                ms: fields.u64("ms")?,
                gpu: fields.bool("gpu")?,
                backend: fields.string("backend")?,
            }),
            "unloaded" => Ok(Self::Unloaded {
                reason: UnloadReason::parse(&fields.string("reason")?)?,
            }),
            "result" => Ok(Self::Result {
                job: fields.u64("job")?,
                text: fields.string("text")?,
                stats: JobStats::read(&fields)?,
            }),
            "cancelled" => Ok(Self::Cancelled {
                job: fields.u64("job")?,
            }),
            "error" => Ok(Self::Error {
                job: fields.optional_u64("job")?,
                code: fields.string("code")?,
                params: fields.object("params")?,
            }),
            "pong" => Ok(Self::Pong),
            other => Err(DecodeError::UnknownType(other.to_owned())),
        }
    }

    pub fn from_frame(frame: &Frame) -> Result<Self, DecodeError> {
        Self::from_json(&parse_payload(frame)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{FrameKind, read_frame, write_json};

    #[test]
    fn requests_round_trip_through_json() {
        let requests = [
            LlmRequest::Hello { protocol: 1 },
            LlmRequest::LoadModel {
                path: "/m/Qwen3.5-2B-Q4_K_M.gguf".into(),
                family: Some("qwen35".into()),
                gpu: true,
                threads: 4,
                idle_secs: 300,
            },
            LlmRequest::LoadModel {
                path: "/m/x.gguf".into(),
                family: None,
                gpu: false,
                threads: 2,
                idle_secs: 0,
            },
            LlmRequest::Cleanup {
                job: 3,
                system: "Fix it.".into(),
                user: "um hello".into(),
                max_tokens: 128,
            },
            LlmRequest::Command {
                job: 4,
                instruction: "shorter".into(),
                selection: "a long text".into(),
                max_tokens: 256,
            },
            LlmRequest::Unload,
            LlmRequest::Cancel { job: 3 },
            LlmRequest::Ping,
            LlmRequest::Shutdown,
        ];
        for request in requests {
            assert_eq!(LlmRequest::from_json(&request.to_json()), Ok(request));
        }
    }

    #[test]
    fn a_load_without_an_idle_time_uses_the_default() {
        let value = json!({"type": "load_model", "path": "/m", "gpu": false, "threads": 2});
        let Ok(LlmRequest::LoadModel { idle_secs, .. }) = LlmRequest::from_json(&value) else {
            panic!("a load request");
        };
        assert_eq!(idle_secs, DEFAULT_IDLE_SECS);
    }

    #[test]
    fn events_round_trip_through_json() {
        let stats = JobStats {
            prompt_tokens: 90,
            reused_tokens: 80,
            new_tokens: 10,
            gen_tokens: 12,
            ms: 410,
            load_ms: 0,
        };
        let events = [
            LlmEvent::Ready {
                version: "2.0.0".into(),
                protocol: 1,
                backend: "cpu".into(),
            },
            LlmEvent::Loading {
                model: "Qwen3.5-2B-Q4_K_M".into(),
            },
            LlmEvent::Loaded {
                model: "Qwen3.5-2B-Q4_K_M".into(),
                ms: 900,
                gpu: true,
                backend: "metal".into(),
            },
            LlmEvent::Unloaded {
                reason: UnloadReason::Idle,
            },
            LlmEvent::Unloaded {
                reason: UnloadReason::Requested,
            },
            LlmEvent::Result {
                job: 3,
                text: "Hello.".into(),
                stats,
            },
            LlmEvent::Cancelled { job: 3 },
            LlmEvent::error(Some(3), "LLM_TIMEOUT", "slow"),
            LlmEvent::error(None, "LLM_LOAD_FAILED", "no file"),
            LlmEvent::Pong,
        ];
        for event in events {
            assert_eq!(LlmEvent::from_json(&event.to_json()), Ok(event));
        }
    }

    #[test]
    fn an_unknown_type_is_told_apart_from_a_malformed_message() {
        assert_eq!(
            LlmRequest::from_json(&json!({"type": "dance"})),
            Err(DecodeError::UnknownType("dance".into()))
        );
        assert!(matches!(
            LlmRequest::from_json(&json!({"type": "cleanup", "job": 1})),
            Err(DecodeError::Invalid(_))
        ));
        assert!(matches!(
            LlmEvent::from_json(&json!({"type": "unloaded", "reason": "boredom"})),
            Err(DecodeError::Invalid(_))
        ));
    }

    #[test]
    fn a_request_crosses_the_frame_codec() {
        let mut bytes = Vec::new();
        write_json(&mut bytes, &LlmRequest::Ping.to_json()).unwrap();
        let frame = read_frame(&mut &bytes[..]).unwrap().unwrap();
        assert_eq!(frame.kind, FrameKind::Json);
        assert_eq!(LlmRequest::from_frame(&frame), Ok(LlmRequest::Ping));
    }
}
