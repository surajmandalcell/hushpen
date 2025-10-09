//! WebSocket message models for real-time communication
//! 
//! This module contains enhanced WebSocket message structures for real-time updates
//! between the frontend and backend during transcription and other operations.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// Enhanced WebSocket message types for real-time communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    /// Connection management
    Connected { session_id: String },
    Disconnected { reason: String },
    Error { error: String },
    
    /// Transcription events
    TranscriptionStarted(TranscriptionStartedEvent),
    TranscriptionProgress(TranscriptionProgressEvent),
    TranscriptionSegment(TranscriptionSegmentEvent),
    TranscriptionCompleted(TranscriptionCompletedEvent),
    TranscriptionFailed(TranscriptionFailedEvent),
    
    /// File operations
    FileUploadStarted(FileUploadStartedEvent),
    FileUploadProgress(FileUploadProgressEvent),
    FileUploadCompleted(FileUploadCompletedEvent),
    FileUploadFailed(FileUploadFailedEvent),
    
    /// Model operations
    ModelDownloadStarted(ModelDownloadStartedEvent),
    ModelDownloadProgress(ModelDownloadProgressEvent),
    ModelDownloadCompleted(ModelDownloadCompletedEvent),
    ModelDownloadFailed(ModelDownloadFailedEvent),
    ModelLoaded(ModelLoadedEvent),
    ModelUnloaded(ModelUnloadedEvent),
    
    /// System events
    SystemStatus(SystemStatusEvent),
    ContainerStatus(ContainerStatusEvent),
    
    /// Heartbeat
    Ping { timestamp: DateTime<Utc> },
    Pong { timestamp: DateTime<Utc> },
    
    /// Subscription management
    Subscribe { channels: Vec<String> },
    Unsubscribe { channels: Vec<String> },
    SubscriptionConfirmed { channels: Vec<String> },
}

/// Base event structure with common fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseEvent {
    /// Unique event ID
    pub id: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Session ID
    pub session_id: String,
}

/// Transcription started event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Transcription task ID
    pub task_id: Uuid,
    /// File being transcribed
    pub filename: String,
    /// Model being used
    pub model: String,
    /// Language code
    pub language: Option<String>,
    /// Estimated duration
    pub estimated_duration: Option<f64>,
}

/// Transcription progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionProgressEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Transcription task ID
    pub task_id: Uuid,
    /// Progress percentage (0-100)
    pub progress: f32,
    /// Current status message
    pub status: String,
    /// Current processing stage
    pub stage: TranscriptionStage,
    /// Elapsed time in seconds
    pub elapsed_time: f64,
    /// Estimated remaining time in seconds
    pub estimated_remaining: Option<f64>,
}

/// Transcription segment event (real-time transcription results)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegmentEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Transcription task ID
    pub task_id: Uuid,
    /// Segment text
    pub text: String,
    /// Segment start time in seconds
    pub start: f64,
    /// Segment end time in seconds
    pub end: f64,
    /// Confidence score for this segment
    pub confidence: f32,
    /// Whether this is a final or interim result
    pub is_final: bool,
}

/// Transcription completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Transcription task ID
    pub task_id: Uuid,
    /// Full transcribed text
    pub text: String,
    /// Detected language
    pub language: Option<String>,
    /// Language detection confidence
    pub language_probability: Option<f64>,
    /// Audio duration in seconds
    pub duration: f64,
    /// Processing time in seconds
    pub processing_time: f64,
    /// All transcription segments
    pub segments: Vec<TranscriptionSegmentData>,
    /// Model used for transcription
    pub model: String,
}

/// Transcription failed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionFailedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Transcription task ID
    pub task_id: Uuid,
    /// Error message
    pub error: String,
    /// Error code
    pub error_code: Option<String>,
    /// Model that was being used
    pub model: String,
}

/// File upload started event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// File ID
    pub file_id: Uuid,
    /// Filename
    pub filename: String,
    /// File size in bytes
    pub file_size: u64,
}

/// File upload progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadProgressEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// File ID
    pub file_id: Uuid,
    /// Progress percentage (0-100)
    pub progress: f32,
    /// Bytes uploaded
    pub bytes_uploaded: u64,
    /// Total bytes
    pub total_bytes: u64,
    /// Upload speed in bytes per second
    pub upload_speed: Option<f64>,
}

/// File upload completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// File ID
    pub file_id: Uuid,
    /// Filename
    pub filename: String,
    /// Upload time in seconds
    pub upload_time: f64,
}

/// File upload failed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadFailedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// File ID
    pub file_id: Uuid,
    /// Filename
    pub filename: String,
    /// Error message
    pub error: String,
}

/// Model download started event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadStartedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Model name
    pub model_name: String,
    /// Model size in bytes
    pub model_size: u64,
}

/// Model download progress event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgressEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Progress percentage (0-100)
    pub progress: f32,
    /// Bytes downloaded
    pub bytes_downloaded: u64,
    /// Total bytes
    pub total_bytes: u64,
    /// Download speed in bytes per second
    pub download_speed: Option<f64>,
}

/// Model download completed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadCompletedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Model name
    pub model_name: String,
    /// Download time in seconds
    pub download_time: f64,
}

/// Model download failed event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadFailedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Model name
    pub model_name: String,
    /// Error message
    pub error: String,
}

/// Model loaded event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Model name
    pub model_name: String,
    /// Load time in seconds
    pub load_time: f64,
    /// Memory usage in bytes
    pub memory_usage: Option<u64>,
}

/// Model unloaded event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUnloadedEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Model ID
    pub model_id: String,
    /// Model name
    pub model_name: String,
}

/// System status event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatusEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Overall system status
    pub status: SystemStatus,
    /// Docker availability
    pub docker_available: bool,
    /// GPU availability
    pub gpu_available: bool,
    /// Currently loaded model
    pub current_model: Option<String>,
    /// System resources usage
    pub resources: SystemResources,
}

/// Container status event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatusEvent {
    #[serde(flatten)]
    pub base: BaseEvent,
    /// Container ID or name
    pub container_id: String,
    /// Container status
    pub status: ContainerStatus,
    /// Model associated with container
    pub model: Option<String>,
    /// Container resource usage
    pub resources: Option<ContainerResources>,
}

/// Transcription processing stages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscriptionStage {
    /// Initializing transcription
    Initializing,
    /// Loading audio file
    LoadingAudio,
    /// Preprocessing audio
    Preprocessing,
    /// Running inference
    RunningInference,
    /// Post-processing results
    Postprocessing,
    /// Finalizing results
    Finalizing,
}

/// System status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemStatus {
    /// System is healthy and ready
    Healthy,
    /// System is busy but operational
    Busy,
    /// System is experiencing issues
    Degraded,
    /// System is unavailable
    Unavailable,
}

/// Container status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContainerStatus {
    /// Container is being created
    Creating,
    /// Container is running
    Running,
    /// Container is paused
    Paused,
    /// Container is restarting
    Restarting,
    /// Container is removing
    Removing,
    /// Container has exited
    Exited,
    /// Container is dead
    Dead,
    /// Container creation failed
    Failed,
}

/// System resources information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResources {
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Memory usage percentage (0-100)
    pub memory_usage: f32,
    /// Memory used in bytes
    pub memory_used: u64,
    /// Total memory in bytes
    pub memory_total: u64,
    /// Disk usage percentage (0-100)
    pub disk_usage: f32,
    /// Disk used in bytes
    pub disk_used: u64,
    /// Total disk in bytes
    pub disk_total: u64,
    /// GPU usage percentage (0-100) if available
    pub gpu_usage: Option<f32>,
    /// GPU memory used in bytes if available
    pub gpu_memory_used: Option<u64>,
    /// Total GPU memory in bytes if available
    pub gpu_memory_total: Option<u64>,
}

/// Container resources information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerResources {
    /// CPU usage percentage (0-100)
    pub cpu_usage: f32,
    /// Memory usage in bytes
    pub memory_usage: u64,
    /// Network I/O in bytes
    pub network_io: Option<NetworkIo>,
}

/// Network I/O statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIo {
    /// Bytes received
    pub bytes_rx: u64,
    /// Bytes transmitted
    pub bytes_tx: u64,
}

/// Transcription segment data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegmentData {
    /// Segment text
    pub text: String,
    /// Segment start time in seconds
    pub start: f64,
    /// Segment end time in seconds
    pub end: f64,
    /// Confidence score for this segment
    pub confidence: f32,
    /// Word-level timestamps if available
    pub words: Option<Vec<WordTimestamp>>,
}

/// Word-level timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTimestamp {
    /// Word text
    pub word: String,
    /// Word start time in seconds
    pub start: f64,
    /// Word end time in seconds
    pub end: f64,
    /// Word confidence score
    pub confidence: f32,
}

/// WebSocket subscription channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionChannel {
    /// All transcription events
    Transcription,
    /// All file operation events
    Files,
    /// All model operation events
    Models,
    /// All system events
    System,
    /// Events for a specific transcription task
    TranscriptionTask(Uuid),
    /// Events for a specific file
    File(Uuid),
    /// Events for a specific model
    Model(String),
}

impl SubscriptionChannel {
    /// Convert to string representation
    pub fn to_string(&self) -> String {
        match self {
            SubscriptionChannel::Transcription => "transcription".to_string(),
            SubscriptionChannel::Files => "files".to_string(),
            SubscriptionChannel::Models => "models".to_string(),
            SubscriptionChannel::System => "system".to_string(),
            SubscriptionChannel::TranscriptionTask(id) => format!("transcription:{}", id),
            SubscriptionChannel::File(id) => format!("file:{}", id),
            SubscriptionChannel::Model(id) => format!("model:{}", id),
        }
    }
    
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        if s == "transcription" {
            Some(SubscriptionChannel::Transcription)
        } else if s == "files" {
            Some(SubscriptionChannel::Files)
        } else if s == "models" {
            Some(SubscriptionChannel::Models)
        } else if s == "system" {
            Some(SubscriptionChannel::System)
        } else if s.starts_with("transcription:") {
            let id_str = s.strip_prefix("transcription:")?;
            Uuid::parse_str(id_str).ok().map(SubscriptionChannel::TranscriptionTask)
        } else if s.starts_with("file:") {
            let id_str = s.strip_prefix("file:")?;
            Uuid::parse_str(id_str).ok().map(SubscriptionChannel::File)
        } else if s.starts_with("model:") {
            let id_str = s.strip_prefix("model:")?;
            Some(SubscriptionChannel::Model(id_str.to_string()))
        } else {
            None
        }
    }
}

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connection in progress
    Connecting,
    /// Connected and ready
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting,
    /// Connection failed
    Failed,
}

/// WebSocket configuration
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// WebSocket URL
    pub url: String,
    /// Authentication token (if required)
    pub auth_token: Option<String>,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Heartbeat interval in seconds
    pub heartbeat_interval: u64,
    /// Reconnection attempts
    pub max_reconnect_attempts: u32,
    /// Reconnection delay in seconds
    pub reconnect_delay: u64,
    /// Whether to enable exponential backoff for reconnection
    pub exponential_backoff: bool,
    /// Maximum reconnection delay in seconds
    pub max_reconnect_delay: u64,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            url: "ws://localhost:8000/ws".to_string(),
            auth_token: None,
            connection_timeout: 10,
            heartbeat_interval: 30,
            max_reconnect_attempts: 5,
            reconnect_delay: 2,
            exponential_backoff: true,
            max_reconnect_delay: 60,
        }
    }
}