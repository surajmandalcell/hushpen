//! API models for communication with the ASR Pro backend
//! 
//! This module contains request and response structures for all API endpoints,
//! with proper serialization annotations.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall health status
    pub status: String,
    /// Currently active model
    pub current_model: Option<String>,
    /// Processing device information
    pub device: Option<String>,
}

/// Model information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Model identifier
    pub id: String,
    /// Object type (always "model")
    pub object: String,
    /// Model owner
    pub owned_by: String,
    /// Whether the model is ready for use
    pub ready: bool,
}

/// List of available models response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelListResponse {
    /// Object type (always "list")
    pub object: String,
    /// List of models
    pub data: Vec<ModelResponse>,
}

/// Request to set the active model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettingRequest {
    /// Model ID to set as active
    pub model_id: String,
}

/// Response for model setting request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettingResponse {
    /// Status of the operation
    pub status: String,
    /// Model ID that was set
    pub model: String,
}

/// Transcription response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    /// Transcribed text
    pub text: String,
    /// Detected language
    pub language: Option<String>,
    /// Language detection confidence
    pub language_probability: Option<f64>,
    /// Audio duration in seconds
    pub duration: Option<f64>,
    /// Transcription segments with timestamps
    pub segments: Option<Vec<TranscriptionSegment>>,
    /// Additional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    /// Special tokens or annotations
    pub specials: Option<Vec<serde_json::Value>>,
    /// Model ID used for transcription
    pub model_id: Option<String>,
    /// Processing time in seconds
    pub processing_time: Option<f64>,
    /// Backend type (always "docker")
    pub backend: Option<String>,
    /// Container information
    pub container_info: Option<ContainerInfo>,
}

/// Individual transcription segment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// Segment text
    pub text: String,
    /// Segment start time in seconds
    pub start: f64,
    /// Segment end time in seconds
    pub end: f64,
    /// Confidence score for this segment
    pub confidence: Option<f64>,
}

/// Container information for Docker-based processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInfo {
    /// Container status
    pub status: Option<String>,
    /// Whether GPU is allocated
    pub gpu_allocated: Option<bool>,
    /// Container image
    pub image: Option<String>,
}

/// API options response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiOptionsResponse {
    /// API version
    pub api_version: String,
    /// Server name
    pub server: String,
    /// Available endpoints
    pub endpoints: HashMap<String, EndpointInfo>,
    /// Supported file formats
    pub supported_formats: SupportedFormats,
    /// Available models
    pub available_models: Vec<String>,
    /// System capabilities
    pub capabilities: SystemCapabilities,
    /// Usage examples
    pub examples: HashMap<String, String>,
}

/// Information about an API endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointInfo {
    /// HTTP method
    pub method: String,
    /// Endpoint description
    pub description: String,
    /// Endpoint parameters
    pub parameters: HashMap<String, ParameterInfo>,
    /// Response schemas
    pub responses: HashMap<String, ResponseInfo>,
}

/// Information about a parameter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    /// Parameter type
    #[serde(rename = "type")]
    pub param_type: String,
    /// Whether parameter is required
    pub required: bool,
    /// Parameter description
    pub description: String,
    /// Available options (for enum parameters)
    pub options: Option<Vec<String>>,
    /// Default value
    pub default: Option<serde_json::Value>,
    /// Example values
    pub examples: Option<Vec<String>>,
    /// Validation rules
    pub validation: Option<ValidationInfo>,
}

/// Validation information for parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationInfo {
    /// Minimum size in bytes
    pub min_size_bytes: Option<u64>,
    /// Maximum size in MB
    pub max_size_mb: Option<u64>,
    /// Supported file extensions
    pub supported_extensions: Option<Vec<String>>,
}

/// Information about response formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInfo {
    /// Response description
    pub description: String,
    /// Response schema
    pub schema: serde_json::Value,
}

/// Supported file formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedFormats {
    /// Audio formats
    pub audio: FormatInfo,
    /// Video formats
    pub video: FormatInfo,
}

/// Information about a format category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatInfo {
    /// File extensions
    pub extensions: Vec<String>,
    /// MIME types
    pub mime_types: Vec<String>,
}

/// System capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// FFmpeg support
    pub ffmpeg_support: bool,
    /// Video processing capability
    pub video_processing: bool,
    /// Audio extraction capability
    pub audio_extraction: bool,
    /// GPU acceleration availability
    pub gpu_acceleration: bool,
    /// Current device
    pub current_device: String,
    /// WebSocket support
    pub websocket_support: bool,
    /// Real-time progress updates
    pub real_time_progress: bool,
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Transcription started notification
    TranscriptionStarted { data: TranscriptionStartedData },
    /// Transcription progress update
    TranscriptionProgress { data: TranscriptionProgressData },
    /// Transcription completed notification
    TranscriptionCompleted { data: TranscriptionCompletedData },
    /// Transcription error notification
    TranscriptionError { data: TranscriptionErrorData },
    /// Container status update
    ContainerStatus { data: ContainerStatusData },
    /// System status update
    SystemStatus { data: SystemStatusData },
    /// Ping message for connection health check
    Ping,
    /// Pong response to ping
    Pong,
}

/// Data for transcription started message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionStartedData {
    /// Filename being transcribed
    pub filename: String,
    /// Model being used
    pub model: String,
}

/// Data for transcription progress message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionProgressData {
    /// Filename being transcribed
    pub filename: String,
    /// Progress percentage (0-100)
    pub progress: i32,
    /// Status message
    pub status: String,
    /// Model being used
    pub model: String,
}

/// Data for transcription completed message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionCompletedData {
    /// Filename that was transcribed
    pub filename: String,
    /// Length of the result text
    pub result_length: usize,
    /// Model that was used
    pub model: String,
    /// Processing time in seconds
    pub processing_time: f64,
}

/// Data for transcription error message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionErrorData {
    /// Filename that failed
    pub filename: String,
    /// Error message
    pub error: String,
    /// Model that was being used
    pub model: String,
}

/// Data for container status message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatusData {
    /// Container ID or name
    pub container_id: String,
    /// Container status
    pub status: String,
    /// Model associated with container
    pub model: String,
}

/// Data for system status message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusData {
    /// Overall system status
    pub status: String,
    /// Docker availability
    pub docker_available: bool,
    /// GPU availability
    pub gpu_available: bool,
    /// Currently loaded model
    pub current_model: Option<String>,
    /// System resources usage
    pub resources: Option<HashMap<String, serde_json::Value>>,
}

/// Configuration for the backend client
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// Base URL for the API
    pub base_url: String,
    /// API key for authentication (if required)
    pub api_key: Option<String>,
    /// Request timeout in seconds
    pub timeout: u64,
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Retry delay in milliseconds
    pub retry_delay: u64,
    /// Whether to enable WebSocket connections
    pub enable_websocket: bool,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8000".to_string(),
            api_key: None,
            timeout: 60,
            max_retries: 3,
            retry_delay: 1000,
            enable_websocket: true,
        }
    }
}

/// Error response from the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error message
    pub detail: String,
    /// Error type (if available)
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    /// HTTP status code
    pub status: Option<u16>,
}

/// Transcription request parameters
#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    /// Path to the audio file
    pub file_path: String,
    /// Model to use for transcription
    pub model: Option<String>,
    /// Response format (json, text, srt)
    pub response_format: Option<String>,
    /// Language code (if known)
    pub language: Option<String>,
    /// Whether to include timestamps
    pub timestamp: Option<bool>,
    /// Whether to include word segments
    pub segments: Option<bool>,
}

impl Default for TranscriptionRequest {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            model: Some("whisper-base".to_string()),
            response_format: Some("json".to_string()),
            language: None,
            timestamp: Some(true),
            segments: Some(true),
        }
    }
}