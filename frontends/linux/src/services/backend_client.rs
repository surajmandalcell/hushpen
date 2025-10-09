//! Backend client for communicating with the ASR Pro Python sidecar
//! 
//! This module provides a comprehensive HTTP client for all API operations,
//! including file uploads, model management, and transcription requests.

use std::time::Duration;
use std::path::Path;
use tokio::time::timeout;
use futures_util::{StreamExt, SinkExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;

use crate::models::api::*;
use crate::utils::{AppError, AppResult};

/// Backend client for communicating with the ASR Pro API
#[derive(Debug, Clone)]
pub struct BackendClient {
    /// HTTP client for API requests
    http_client: reqwest::Client,
    /// Backend configuration
    config: BackendConfig,
}

impl BackendClient {
    /// Create a new backend client with the given configuration
    pub fn new(config: BackendConfig) -> AppResult<Self> {
        // Configure HTTP client with timeout and retry settings
        let timeout_duration = Duration::from_secs(config.timeout);
        let mut client_builder = reqwest::Client::builder()
            .timeout(timeout_duration)
            .user_agent("ASRPro-Linux/1.0.0");

        // Configure connection pool
        client_builder = client_builder
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30));

        // Build the client
        let http_client = client_builder
            .build()
            .map_err(|e| AppError::api_with_source("Failed to create HTTP client", e))?;

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Create a backend client with default configuration
    pub fn with_default_url(base_url: String) -> AppResult<Self> {
        let config = BackendConfig {
            base_url,
            ..Default::default()
        };
        Self::new(config)
    }

    /// Execute an HTTP request with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> AppResult<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>> + Send,
        T: serde::de::DeserializeOwned,
    {
        let mut last_error = None;
        
        for attempt in 1..=self.config.max_retries {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.clone());
                    
                    // Don't retry on client errors (4xx)
                    if let AppError::Api { message, .. } = &e {
                        if message.contains("status: 4") {
                            return Err(e);
                        }
                    }
                    
                    // If this is not the last attempt, wait before retrying
                    if attempt < self.config.max_retries {
                        let delay = Duration::from_millis(self.config.retry_delay * attempt as u64);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| AppError::api("Max retries exceeded")))
    }

    /// Build a request with common headers and authentication
    fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut request = self.http_client.request(method, url);
        
        // Add API key if provided
        if let Some(api_key) = &self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        // Add common headers
        request = request
            .header("Accept", "application/json")
            .header("Content-Type", "application/json");
        
        request
    }

    /// Get the health status of the backend
    pub async fn health_check(&self) -> AppResult<HealthResponse> {
        let url = format!("{}/health", self.config.base_url);
        
        let operation = || {
            let url = url.clone();
            async move {
                let request = self.build_request(reqwest::Method::GET, &url);
                let response = request
                    .send()
                    .await
                    .map_err(|e| AppError::api_with_source("Failed to send health check request", e))?;
                
                if response.status().is_success() {
                    let health = response
                        .json::<HealthResponse>()
                        .await
                        .map_err(|e| AppError::api_with_source("Failed to parse health check response", e))?;
                    Ok(health)
                } else {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(AppError::api(format!(
                        "Health check failed with status {}: {}",
                        status, error_text
                    )))
                }
            }
        };
        
        self.execute_with_retry(operation).await
    }

    /// List all available AI models
    pub async fn list_models(&self) -> AppResult<ModelListResponse> {
        let url = format!("{}/v1/models", self.config.base_url);
        
        let operation = || {
            let url = url.clone();
            async move {
                let request = self.build_request(reqwest::Method::GET, &url);
                let response = request
                    .send()
                    .await
                    .map_err(|e| AppError::api_with_source("Failed to send list models request", e))?;
                
                if response.status().is_success() {
                    let models = response
                        .json::<ModelListResponse>()
                        .await
                        .map_err(|e| AppError::api_with_source("Failed to parse models response", e))?;
                    Ok(models)
                } else {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(AppError::api(format!(
                        "Failed to list models with status {}: {}",
                        status, error_text
                    )))
                }
            }
        };
        
        self.execute_with_retry(operation).await
    }

    /// Set the active AI model
    pub async fn set_model(&self, model_id: &str) -> AppResult<ModelSettingResponse> {
        let url = format!("{}/v1/settings/model", self.config.base_url);
        
        let request_body = ModelSettingRequest {
            model_id: model_id.to_string(),
        };
        
        let operation = || {
            let url = url.clone();
            let request_body = request_body.clone();
            async move {
                let request = self.build_request(reqwest::Method::POST, &url)
                    .json(&request_body);
                
                let response = request
                    .send()
                    .await
                    .map_err(|e| AppError::api_with_source("Failed to send set model request", e))?;
                
                if response.status().is_success() {
                    let result = response
                        .json::<ModelSettingResponse>()
                        .await
                        .map_err(|e| AppError::api_with_source("Failed to parse set model response", e))?;
                    Ok(result)
                } else {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(AppError::api(format!(
                        "Failed to set model with status {}: {}",
                        status, error_text
                    )))
                }
            }
        };
        
        self.execute_with_retry(operation).await
    }

    /// Transcribe an audio file
    pub async fn transcribe_audio(&self, request: TranscriptionRequest) -> AppResult<TranscriptionResponse> {
        let url = format!("{}/v1/audio/transcriptions", self.config.base_url);
        
        // Validate file exists
        if !Path::new(&request.file_path).exists() {
            return Err(AppError::file(format!("File not found: {}", request.file_path)));
        }
        
        // Read file bytes
        let file_bytes = std::fs::read(&request.file_path)
            .map_err(|e| AppError::file_with_source("Failed to read audio file", e))?;
        
        // Extract filename from path
        let filename = Path::new(&request.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.mp3")
            .to_string();
        
        // Create multipart form
        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(filename.clone());
        
        let mut form = reqwest::multipart::Form::new()
            .part("file", file_part);
        
        // Add optional parameters
        if let Some(model) = &request.model {
            form = form.text("model", model.clone());
        }
        
        if let Some(response_format) = &request.response_format {
            form = form.text("response_format", response_format.clone());
        }
        
        if let Some(language) = &request.language {
            form = form.text("language", language.clone());
        }
        
        // For multipart forms, we need to recreate the form for each retry attempt
        let operation = || {
            let url = url.clone();
            let file_path = request.file_path.clone();
            let model = request.model.clone();
            let response_format = request.response_format.clone();
            let language = request.language.clone();
            let api_key = self.config.api_key.clone();
            
            async move {
                // Re-read file bytes for each attempt
                let file_bytes = std::fs::read(&file_path)
                    .map_err(|e| AppError::file_with_source("Failed to read audio file", e))?;
                
                // Re-extract filename
                let filename = Path::new(&file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("audio.mp3")
                    .to_string();
                
                // Recreate multipart form
                let file_part = reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(filename);
                
                let mut form = reqwest::multipart::Form::new()
                    .part("file", file_part);
                
                // Add optional parameters
                if let Some(model) = &model {
                    form = form.text("model", model.clone());
                }
                
                if let Some(response_format) = &response_format {
                    form = form.text("response_format", response_format.clone());
                }
                
                if let Some(language) = &language {
                    form = form.text("language", language.clone());
                }
                
                let mut request_builder = reqwest::Client::new().post(&url);
                
                // Add API key if provided
                if let Some(api_key) = &api_key {
                    request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));
                }
                
                let response = request_builder
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|e| AppError::api_with_source("Failed to send transcription request", e))?;
                
                if response.status().is_success() {
                    let result = response
                        .json::<TranscriptionResponse>()
                        .await
                        .map_err(|e| AppError::api_with_source("Failed to parse transcription response", e))?;
                    Ok(result)
                } else {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(AppError::api(format!(
                        "Transcription failed with status {}: {}",
                        status, error_text
                    )))
                }
            }
        };
        
        self.execute_with_retry(operation).await
    }

    /// Get API options and capabilities
    pub async fn get_options(&self) -> AppResult<ApiOptionsResponse> {
        let url = format!("{}/v1/options", self.config.base_url);
        
        let operation = || {
            let url = url.clone();
            async move {
                let request = self.build_request(reqwest::Method::GET, &url);
                let response = request
                    .send()
                    .await
                    .map_err(|e| AppError::api_with_source("Failed to send options request", e))?;
                
                if response.status().is_success() {
                    let options = response
                        .json::<ApiOptionsResponse>()
                        .await
                        .map_err(|e| AppError::api_with_source("Failed to parse options response", e))?;
                    Ok(options)
                } else {
                    let status = response.status();
                    let error_text = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unknown error".to_string());
                    Err(AppError::api(format!(
                        "Failed to get options with status {}: {}",
                        status, error_text
                    )))
                }
            }
        };
        
        self.execute_with_retry(operation).await
    }

    /// Create a WebSocket connection for real-time updates
    pub async fn create_websocket_connection(&self) -> AppResult<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
        if !self.config.enable_websocket {
            return Err(AppError::api("WebSocket connections are disabled"));
        }
        
        // Convert HTTP URL to WebSocket URL
        let ws_url = self.config.base_url
            .replace("http://", "ws://")
            .replace("https://", "wss://") + "/ws";
        
        let url = Url::parse(&ws_url)
            .map_err(|e| AppError::api_with_source("Invalid WebSocket URL", e))?;
        
        let (ws_stream, _) = timeout(
            Duration::from_secs(10),
            connect_async(url)
        )
        .await
        .map_err(|_| AppError::api("WebSocket connection timeout"))?
        .map_err(|e| AppError::api_with_source("Failed to connect to WebSocket", e))?;
        
        Ok(ws_stream)
    }

    /// Send a message through WebSocket
    pub async fn send_websocket_message(
        ws_stream: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        message: WebSocketMessage,
    ) -> AppResult<()> {
        let json = serde_json::to_string(&message)
            .map_err(|e| AppError::api_with_source("Failed to serialize WebSocket message", e))?;
        
        ws_stream.send(Message::Text(json))
            .await
            .map_err(|e| AppError::api_with_source("Failed to send WebSocket message", e))?;
        
        Ok(())
    }

    /// Receive a message from WebSocket
    pub async fn receive_websocket_message(
        ws_stream: &mut tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> AppResult<WebSocketMessage> {
        let message = timeout(
            Duration::from_secs(30),
            ws_stream.next()
        )
        .await
        .map_err(|_| AppError::api("WebSocket receive timeout"))?
        .ok_or_else(|| AppError::api("WebSocket connection closed"))?
        .map_err(|e| AppError::api_with_source("Failed to receive WebSocket message", e))?;
        
        match message {
            Message::Text(text) => {
                let ws_message = serde_json::from_str(&text)
                    .map_err(|e| AppError::api_with_source("Failed to deserialize WebSocket message", e))?;
                Ok(ws_message)
            },
            Message::Binary(data) => {
                let text = String::from_utf8(data)
                    .map_err(|e| AppError::api_with_source("Invalid UTF-8 in WebSocket message", e))?;
                let ws_message = serde_json::from_str(&text)
                    .map_err(|e| AppError::api_with_source("Failed to deserialize WebSocket message", e))?;
                Ok(ws_message)
            },
            Message::Close(_) => {
                Err(AppError::api("WebSocket connection closed"))
            },
            Message::Ping(_) => {
                // Respond with pong automatically
                Ok(WebSocketMessage::Ping)
            },
            Message::Pong(_) => {
                Ok(WebSocketMessage::Pong)
            },
            Message::Frame(_) => {
                // Handle raw frame messages
                Err(AppError::api("Unsupported WebSocket frame message"))
            },
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &BackendConfig {
        &self.config
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: BackendConfig) -> AppResult<()> {
        self.config = config;
        
        // Recreate the HTTP client with new settings
        let timeout_duration = Duration::from_secs(self.config.timeout);
        let client_builder = reqwest::Client::builder()
            .timeout(timeout_duration)
            .user_agent("ASRPro-Linux/1.0.0")
            .pool_max_idle_per_host(10)
            .pool_idle_timeout(Duration::from_secs(30));
        
        self.http_client = client_builder
            .build()
            .map_err(|e| AppError::api_with_source("Failed to recreate HTTP client", e))?;
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_backend_client_creation() {
        let config = BackendConfig::default();
        let client = BackendClient::new(config);
        assert!(client.is_ok());
    }
    
    #[test]
    fn test_backend_client_with_url() {
        let client = BackendClient::with_default_url("http://localhost:8000".to_string());
        assert!(client.is_ok());
    }
    
    #[tokio::test]
    async fn test_transcription_request_creation() {
        let request = TranscriptionRequest {
            file_path: "/test/audio.mp3".to_string(),
            model: Some("whisper-base".to_string()),
            response_format: Some("json".to_string()),
            language: Some("en".to_string()),
            timestamp: Some(true),
            segments: Some(true),
        };
        
        assert_eq!(request.file_path, "/test/audio.mp3");
        assert_eq!(request.model, Some("whisper-base".to_string()));
        assert_eq!(request.response_format, Some("json".to_string()));
        assert_eq!(request.language, Some("en".to_string()));
        assert_eq!(request.timestamp, Some(true));
        assert_eq!(request.segments, Some(true));
    }
}