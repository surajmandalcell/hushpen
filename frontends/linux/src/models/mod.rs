//! Data models for the ASRPro application
//! 
//! This module contains all the data structures used throughout the application,
//! organized by domain.

pub mod transcription;
pub mod file;
pub mod model;
pub mod app_state;
pub mod api;
pub mod websocket;
pub mod settings;

// Re-export commonly used types
pub use transcription::{
    TranscriptionTask, TranscriptionResult, TranscriptionStatus, TranscriptionConfig,
    TranscriptionSegment, TranscriptionStats,
};
pub use file::{
    AudioFile, AudioMetadata, AudioFileType, FileStatus, FileConfig, FileStats,
};
pub use model::{
    Model, ModelType, ModelStatus, ModelParameters, ModelPerformance, ModelConfig,
    ModelStats, presets,
};
pub use app_state::AppState;
pub use api::{
    HealthResponse, ModelResponse, ModelListResponse, ModelSettingRequest, ModelSettingResponse,
    TranscriptionResponse, TranscriptionSegment as ApiTranscriptionSegment, ContainerInfo, ApiOptionsResponse,
    EndpointInfo, ParameterInfo, SupportedFormats, SystemCapabilities, WebSocketMessage,
    BackendConfig, TranscriptionRequest, ErrorResponse,
};
pub use websocket::{
    WsMessage, BaseEvent, TranscriptionStartedEvent, TranscriptionProgressEvent, TranscriptionSegmentEvent,
    TranscriptionCompletedEvent, TranscriptionFailedEvent, FileUploadStartedEvent, FileUploadProgressEvent,
    FileUploadCompletedEvent, FileUploadFailedEvent, ModelDownloadStartedEvent, ModelDownloadProgressEvent,
    ModelDownloadCompletedEvent, ModelDownloadFailedEvent, ModelLoadedEvent, ModelUnloadedEvent,
    SystemStatusEvent, ContainerStatusEvent, TranscriptionStage, SystemStatus, ContainerStatus,
    SystemResources, ContainerResources, NetworkIo, TranscriptionSegmentData, WordTimestamp,
    SubscriptionChannel, ConnectionState, WebSocketConfig,
};
pub use settings::{
    Settings, GeneralSettings, AudioSettings, TranscriptionSettings, AdvancedSettings,
    UiSettings, FilePathSettings, NotificationSettings, SettingsValidator, SettingsMigration,
};