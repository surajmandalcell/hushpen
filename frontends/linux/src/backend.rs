use std::env;
use std::fs;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::blocking::multipart::{Form, Part};
use reqwest::blocking::{Client, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use uuid::Uuid;

const DEFAULT_BASE_URL: &str = "http://localhost:8000/api/v1";
const DEFAULT_USER_AGENT: &str = "asrpro-gtk4/1.0.0";

#[derive(Clone)]
pub struct BackendClient {
    base_url: String,
    api_key: Option<String>,
    client: Client,
}

impl BackendClient {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent(DEFAULT_USER_AGENT)
            .build()
            .context("failed to create HTTP client")?;

        Ok(Self {
            base_url: base_url.into(),
            api_key,
            client,
        })
    }

    pub fn from_env() -> Result<Self> {
        let base_url =
            env::var("ASRPRO_BACKEND_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let api_key = env::var("ASRPRO_BACKEND_TOKEN").ok();
        Self::new(base_url, api_key)
    }

    pub fn health(&self) -> Result<HealthResponse> {
        let url = self.url("/health");
        let response = self
            .client
            .get(url)
            .headers(self.default_headers())
            .send()?;
        self.parse_response(response)
    }

    pub fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = self.url("/models");
        let response = self
            .client
            .get(url)
            .headers(self.default_headers())
            .send()?;
        let payload: ModelsEnvelope = self.parse_response(response)?;
        Ok(payload.models)
    }

    pub fn start_transcription(
        &self,
        file_path: &Path,
        model: &str,
    ) -> Result<StartTranscriptionResponse> {
        let url = self.url("/transcribe");

        let file_bytes = fs::read(file_path)
            .with_context(|| format!("failed to read {}", file_path.display()))?;
        let file_name = file_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio-file".to_string());

        let audio_part = Part::bytes(file_bytes).file_name(file_name);

        let options = serde_json::json!({
            "include_timestamps": true,
            "include_segments": false,
            "detect_language": true,
            "translate": false,
            "temperature": 0.0,
            "best_of": 1
        });

        let form = Form::new()
            .part("audio", audio_part)
            .text("model", model.to_string())
            .text("options", serde_json::to_string(&options)?);

        let response = self
            .client
            .post(url)
            .headers(self.default_headers())
            .multipart(form)
            .send()
            .context("failed to send transcription request")?;

        self.parse_response(response)
    }

    pub fn transcription_status(&self, task_id: Uuid) -> Result<TranscriptionStatusResponse> {
        let url = self.url(&format!("/transcribe/{task_id}"));
        let response = self
            .client
            .get(url)
            .headers(self.default_headers())
            .send()?;
        self.parse_response(response)
    }

    pub fn transcription_result(&self, task_id: Uuid) -> Result<TranscriptionResultResponse> {
        let url = self.url(&format!("/transcribe/{task_id}/result"));
        let response = self
            .client
            .get(url)
            .headers(self.default_headers())
            .send()?;
        self.parse_response(response)
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn default_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(ACCEPT, "application/json".parse().unwrap());
        headers.insert(USER_AGENT, DEFAULT_USER_AGENT.parse().unwrap());
        if let Some(key) = &self.api_key {
            headers.insert(AUTHORIZATION, format!("Bearer {key}").parse().unwrap());
        }
        headers
    }

    fn parse_response<T>(&self, response: Response) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let status = response.status();
        let text = response.text().unwrap_or_default();

        if !status.is_success() {
            return Err(anyhow!("backend error {status}: {text}"));
        }

        let envelope: ApiEnvelope<T> = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse response: {text}"))?;

        if envelope.success {
            envelope
                .data
                .ok_or_else(|| anyhow!("backend response missing data"))
        } else {
            let message = envelope
                .error
                .and_then(|e| e.message)
                .or(envelope.message)
                .unwrap_or_else(|| "unknown backend error".to_string());
            Err(anyhow!(message))
        }
    }
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    pub code: Option<String>,
    pub message: Option<String>,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelInfo {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub languages: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct ModelsEnvelope {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
pub struct StartTranscriptionResponse {
    pub task_id: Uuid,
    pub status: String,
    #[serde(default)]
    pub estimated_duration: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionStatusResponse {
    pub task_id: Uuid,
    pub status: String,
    #[serde(default)]
    pub progress: Option<f64>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub eta_seconds: Option<f64>,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionResultResponse {
    pub task_id: Uuid,
    pub status: String,
    #[serde(default)]
    pub result: Option<TranscriptionResultPayload>,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptionResultPayload {
    #[serde(default)]
    pub text: Option<String>,
}
