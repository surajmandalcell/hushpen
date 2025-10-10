# ASR Pro Linux Frontend API Documentation

This document provides comprehensive API documentation for the services and utilities used in the ASR Pro Linux frontend. It covers all public interfaces, their usage, and implementation details.

## Table of Contents

- [Services API](#services-api)
  - [BackendClient](#backendclient)
  - [FileManager](#filemanager)
  - [ModelManager](#modelmanager)
  - [TranscriptionService](#transcriptionservice)
  - [WebSocketClient](#websocketclient)
  - [ConfigManager](#configmanager)
- [Utilities API](#utilities-api)
  - [Error Handling](#error-handling)
  - [File Utils](#file-utils)
  - [Audio Processor](#audio-processor)
  - [Platform Integration](#platform-integration)
- [Data Models API](#data-models-api)
  - [AppState](#appstate)
  - [AudioFile](#audiofile)
  - [TranscriptionTask](#transcriptiontask)
  - [Model](#model)
  - [Settings](#settings)

## Services API

### BackendClient

The `BackendClient` service handles all communication with the ASR Pro backend API.

#### Constructor

```rust
pub fn new(base_url: String, api_key: Option<String>) -> Self
```

**Parameters:**
- `base_url`: The base URL of the backend API (e.g., "http://localhost:8000")
- `api_key`: Optional API key for authentication

**Example:**
```rust
let client = BackendClient::new(
    "http://localhost:8000".to_string(),
    Some("your-api-key".to_string())
);
```

#### Methods

##### health_check

```rust
pub async fn health_check(&self) -> AppResult<HealthResponse>
```

Checks the health status of the backend.

**Returns:**
- `AppResult<HealthResponse>`: Health status information

**Example:**
```rust
match client.health_check().await {
    Ok(health) => println!("Backend is healthy: {:?}", health),
    Err(e) => eprintln!("Backend health check failed: {}", e),
}
```

##### get_models

```rust
pub async fn get_models(&self) -> AppResult<Vec<Model>>
```

Retrieves the list of available AI models from the backend.

**Returns:**
- `AppResult<Vec<Model>>`: List of available models

**Example:**
```rust
let models = client.get_models().await?;
for model in models {
    println!("Model: {} ({})", model.name, model.size_bytes);
}
```

##### start_transcription

```rust
pub async fn start_transcription(
    &self,
    file_path: &str,
    model: &str,
    language: Option<&str>,
    config: &TranscriptionConfig,
) -> AppResult<Uuid>
```

Starts a new transcription task.

**Parameters:**
- `file_path`: Path to the audio file to transcribe
- `model`: Name of the model to use for transcription
- `language`: Optional language code (e.g., "en", "es")
- `config`: Transcription configuration options

**Returns:**
- `AppResult<Uuid>`: Task ID for tracking the transcription

**Example:**
```rust
let config = TranscriptionConfig {
    include_timestamps: true,
    include_segments: true,
    language: Some("en".to_string()),
};

let task_id = client.start_transcription(
    "/path/to/audio.mp3",
    "whisper-base",
    Some("en"),
    &config
).await?;
```

##### get_transcription_status

```rust
pub async fn get_transcription_status(&self, task_id: Uuid) -> AppResult<TranscriptionStatus>
```

Retrieves the current status of a transcription task.

**Parameters:**
- `task_id`: The ID of the transcription task

**Returns:**
- `AppResult<TranscriptionStatus>`: Current status of the task

**Example:**
```rust
let status = client.get_transcription_status(task_id).await?;
match status.status {
    TaskStatus::Completed => println!("Transcription completed"),
    TaskStatus::InProgress => println!("Progress: {}%", status.progress * 100.0),
    TaskStatus::Failed => println!("Error: {}", status.error_message),
}
```

##### cancel_transcription

```rust
pub async fn cancel_transcription(&self, task_id: Uuid) -> AppResult<()>
```

Cancels an ongoing transcription task.

**Parameters:**
- `task_id`: The ID of the transcription task to cancel

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
if let Err(e) = client.cancel_transcription(task_id).await {
    eprintln!("Failed to cancel transcription: {}", e);
}
```

### FileManager

The `FileManager` service handles all file operations and metadata extraction.

#### Constructor

```rust
pub fn new(app_state: Arc<AppState>) -> Self
```

**Parameters:**
- `app_state`: Shared application state

#### Methods

##### add_file

```rust
pub async fn add_file(&self, file_path: PathBuf) -> AppResult<Uuid>
```

Adds a new audio file to the application.

**Parameters:**
- `file_path`: Path to the audio file

**Returns:**
- `AppResult<Uuid>`: ID of the added file

**Example:**
```rust
let file_id = file_manager.add_file(PathBuf::from("/path/to/audio.mp3")).await?;
println!("Added file with ID: {}", file_id);
```

##### get_file

```rust
pub async fn get_file(&self, file_id: Uuid) -> AppResult<AudioFile>
```

Retrieves information about a specific file.

**Parameters:**
- `file_id`: The ID of the file to retrieve

**Returns:**
- `AppResult<AudioFile>`: The audio file information

**Example:**
```rust
let file = file_manager.get_file(file_id).await?;
println!("File: {} (Duration: {:?})", file.file_path.display(), file.metadata.duration);
```

##### delete_file

```rust
pub async fn delete_file(&self, file_id: Uuid) -> AppResult<()>
```

Removes a file from the application.

**Parameters:**
- `file_id`: The ID of the file to remove

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
file_manager.delete_file(file_id).await?;
println!("File deleted successfully");
```

##### validate_file

```rust
pub fn validate_file(&self, file_path: &Path) -> AppResult<ValidationResult>
```

Validates that a file is supported for transcription.

**Parameters:**
- `file_path`: Path to the file to validate

**Returns:**
- `AppResult<ValidationResult>`: Validation result with details

**Example:**
```rust
let result = file_manager.validate_file(Path::new("/path/to/audio.mp3"))?;
if result.is_supported {
    println!("File is supported: {:?}", result.file_type);
} else {
    println!("File not supported: {}", result.reason);
}
```

##### extract_metadata

```rust
pub async fn extract_metadata(&self, file_path: &Path) -> AppResult<AudioMetadata>
```

Extracts metadata from an audio file.

**Parameters:**
- `file_path`: Path to the audio file

**Returns:**
- `AppResult<AudioMetadata>`: Extracted metadata

**Example:**
```rust
let metadata = file_manager.extract_metadata(Path::new("/path/to/audio.mp3")).await?;
println!("Duration: {:?}, Sample Rate: {}", metadata.duration, metadata.sample_rate);
```

### ModelManager

The `ModelManager` service handles AI model management operations.

#### Constructor

```rust
pub fn new(backend_client: Arc<BackendClient>) -> Self
```

**Parameters:**
- `backend_client`: Backend client for API communication

#### Methods

##### list_models

```rust
pub async fn list_models(&self) -> AppResult<Vec<Model>>
```

Lists all available models.

**Returns:**
- `AppResult<Vec<Model>>`: List of available models

**Example:**
```rust
let models = model_manager.list_models().await?;
for model in models {
    println!("{}: {} ({})", model.display_name, model.name, model.status);
}
```

##### get_model

```rust
pub async fn get_model(&self, model_name: &str) -> AppResult<Model>
```

Retrieves information about a specific model.

**Parameters:**
- `model_name`: Name of the model to retrieve

**Returns:**
- `AppResult<Model>`: Model information

**Example:**
```rust
let model = model_manager.get_model("whisper-base").await?;
println!("Model size: {} bytes", model.size_bytes);
```

##### download_model

```rust
pub async fn download_model(&self, model_name: &str, progress_callback: Option<Box<dyn Fn(f32)>>) -> AppResult<()>
```

Downloads a model if not already available.

**Parameters:**
- `model_name`: Name of the model to download
- `progress_callback`: Optional callback for download progress (0.0 to 1.0)

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
model_manager.download_model("whisper-large", Some(Box::new(|progress| {
    println!("Download progress: {:.1}%", progress * 100.0);
}))).await?;
```

##### delete_model

```rust
pub async fn delete_model(&self, model_name: &str) -> AppResult<()>
```

Removes a downloaded model.

**Parameters:**
- `model_name`: Name of the model to remove

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
model_manager.delete_model("whisper-base").await?;
println!("Model deleted successfully");
```

##### get_model_info

```rust
pub async fn get_model_info(&self, model_name: &str) -> AppResult<ModelInfo>
```

Gets detailed information about a model.

**Parameters:**
- `model_name`: Name of the model

**Returns:**
- `AppResult<ModelInfo>`: Detailed model information

**Example:**
```rust
let info = model_manager.get_model_info("whisper-base").await?;
println!("Languages: {:?}", info.supported_languages);
println!("Performance: {:?}", info.performance_metrics);
```

### TranscriptionService

The `TranscriptionService` service manages transcription tasks and workflow.

#### Constructor

```rust
pub fn new(
    app_state: Arc<AppState>,
    backend_client: Arc<BackendClient>,
    file_manager: Arc<FileManager>,
    model_manager: Arc<ModelManager>,
) -> Self
```

**Parameters:**
- `app_state`: Shared application state
- `backend_client`: Backend client for API communication
- `file_manager`: File manager service
- `model_manager`: Model manager service

#### Methods

##### start_transcription

```rust
pub async fn start_transcription(
    &self,
    file_id: Uuid,
    model_id: Uuid,
    config: TranscriptionConfig,
) -> AppResult<Uuid>
```

Starts a new transcription task.

**Parameters:**
- `file_id`: ID of the file to transcribe
- `model_id`: ID of the model to use
- `config`: Transcription configuration

**Returns:**
- `AppResult<Uuid>`: Task ID for tracking

**Example:**
```rust
let config = TranscriptionConfig {
    language: Some("en".to_string()),
    include_timestamps: true,
    include_segments: false,
};

let task_id = transcription_service.start_transcription(
    file_id,
    model_id,
    config
).await?;
```

##### get_transcription

```rust
pub async fn get_transcription(&self, task_id: Uuid) -> AppResult<TranscriptionResult>
```

Retrieves the result of a completed transcription.

**Parameters:**
- `task_id`: ID of the transcription task

**Returns:**
- `AppResult<TranscriptionResult>`: Transcription result

**Example:**
```rust
let result = transcription_service.get_transcription(task_id).await?;
println!("Transcription: {}", result.text);
println!("Language: {}", result.language);
println!("Confidence: {:.2}%", result.confidence * 100.0);
```

##### cancel_transcription

```rust
pub async fn cancel_transcription(&self, task_id: Uuid) -> AppResult<()>
```

Cancels an ongoing transcription task.

**Parameters:**
- `task_id`: ID of the task to cancel

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
transcription_service.cancel_transcription(task_id).await?;
println!("Transcription cancelled");
```

##### list_transcriptions

```rust
pub async fn list_transcriptions(&self) -> AppResult<Vec<TranscriptionTask>>
```

Lists all transcription tasks.

**Returns:**
- `AppResult<Vec<TranscriptionTask>>`: List of transcription tasks

**Example:**
```rust
let tasks = transcription_service.list_transcriptions().await?;
for task in tasks {
    println!("Task {}: {} ({})", task.id, task.file_name, task.status);
}
```

##### export_transcription

```rust
pub async fn export_transcription(
    &self,
    task_id: Uuid,
    format: ExportFormat,
    output_path: &Path,
) -> AppResult<()>
```

Exports a transcription result to a file.

**Parameters:**
- `task_id`: ID of the transcription task
- `format`: Export format (TXT, SRT, VTT, JSON)
- `output_path`: Path for the exported file

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
transcription_service.export_transcription(
    task_id,
    ExportFormat::SRT,
    Path::new("/path/to/output.srt")
).await?;
```

### WebSocketClient

The `WebSocketClient` service handles real-time communication with the backend.

#### Constructor

```rust
pub fn new(base_url: String, event_handler: Box<dyn WebSocketEventHandler>) -> Self
```

**Parameters:**
- `base_url`: WebSocket server URL (e.g., "ws://localhost:8000/ws")
- `event_handler`: Handler for WebSocket events

#### Methods

##### connect

```rust
pub async fn connect(&self) -> AppResult<()>
```

Connects to the WebSocket server.

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
ws_client.connect().await?;
println!("WebSocket connected");
```

##### disconnect

```rust
pub async fn disconnect(&self) -> AppResult<()>
```

Disconnects from the WebSocket server.

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
ws_client.disconnect().await?;
println!("WebSocket disconnected");
```

##### send_message

```rust
pub async fn send_message(&self, message: WebSocketMessage) -> AppResult<()>
```

Sends a message to the WebSocket server.

**Parameters:**
- `message`: Message to send

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
let message = WebSocketMessage::Subscribe(vec!["transcription".to_string()]);
ws_client.send_message(message).await?;
```

##### subscribe

```rust
pub async fn subscribe(&self, channels: Vec<String>) -> AppResult<()>
```

Subscribes to specific event channels.

**Parameters:**
- `channels`: List of channel names to subscribe to

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
ws_client.subscribe(vec![
    "transcription".to_string(),
    "model_download".to_string()
]).await?;
```

##### is_connected

```rust
pub fn is_connected(&self) -> bool
```

Checks if the WebSocket is connected.

**Returns:**
- `bool`: Connection status

**Example:**
```rust
if ws_client.is_connected() {
    println!("WebSocket is connected");
} else {
    println!("WebSocket is not connected");
}
```

### ConfigManager

The `ConfigManager` service handles application configuration.

#### Constructor

```rust
pub fn new() -> Self
```

#### Methods

##### load_config

```rust
pub fn load_config(&self) -> AppResult<Settings>
```

Loads the application configuration from disk.

**Returns:**
- `AppResult<Settings>`: Loaded settings

**Example:**
```rust
let settings = config_manager.load_config()?;
println!("Backend URL: {}", settings.backend.url);
```

##### save_config

```rust
pub fn save_config(&self, settings: &Settings) -> AppResult<()>
```

Saves the application configuration to disk.

**Parameters:**
- `settings`: Settings to save

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
let mut settings = config_manager.load_config()?;
settings.backend.url = "http://192.168.1.100:8000".to_string();
config_manager.save_config(&settings)?;
```

##### get_default_config

```rust
pub fn get_default_config() -> Settings
```

Returns the default configuration.

**Returns:**
- `Settings`: Default settings

**Example:**
```rust
let default_settings = ConfigManager::get_default_config();
println!("Default backend URL: {}", default_settings.backend.url);
```

##### validate_config

```rust
pub fn validate_config(&self, settings: &Settings) -> AppResult<()>
```

Validates a configuration object.

**Parameters:**
- `settings`: Settings to validate

**Returns:**
- `AppResult<()>`: Success or validation error

**Example:**
```rust
if let Err(e) = config_manager.validate_config(&settings) {
    eprintln!("Invalid configuration: {}", e);
}
```

## Utilities API

### Error Handling

The error handling utilities provide consistent error types and handling across the application.

#### Types

##### AppError

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Backend connection failed: {source}")]
    BackendConnection { source: reqwest::Error },
    
    #[error("File operation failed: {path} - {source}")]
    FileOperation { path: PathBuf, source: std::io::Error },
    
    #[error("Transcription failed: {message}")]
    Transcription { message: String },
    
    #[error("Configuration error: {message}")]
    Configuration { message: String },
    
    #[error("WebSocket error: {source}")]
    WebSocket { source: tokio_tungstenite::tungstenite::Error },
    
    #[error("Validation error: {message}")]
    Validation { message: String },
    
    #[error("Internal error: {message}")]
    Internal { message: String },
}
```

##### AppResult

```rust
pub type AppResult<T> = Result<T, AppError>;
```

#### Methods

##### user_message

```rust
impl AppError {
    pub fn user_message(&self) -> String
}
```

Converts a technical error to a user-friendly message.

**Returns:**
- `String`: User-friendly error message

**Example:**
```rust
match some_operation() {
    Ok(result) => process(result),
    Err(e) => show_error_dialog(e.user_message()),
}
```

##### is_retryable

```rust
impl AppError {
    pub fn is_retryable(&self) -> bool
}
```

Determines if an error is retryable.

**Returns:**
- `bool`: True if the operation can be retried

**Example:**
```rust
match operation() {
    Ok(result) => Ok(result),
    Err(e) if e.is_retryable() => {
        // Retry the operation
        retry_operation()
    },
    Err(e) => Err(e),
}
```

### File Utils

File utility functions for common file operations.

#### Functions

##### is_audio_file

```rust
pub fn is_audio_file(path: &Path) -> bool
```

Checks if a file is an audio file based on its extension.

**Parameters:**
- `path`: Path to the file

**Returns:**
- `bool`: True if the file is an audio file

**Example:**
```rust
if file_utils::is_audio_file(&file_path) {
    println!("This is an audio file");
}
```

##### get_audio_format

```rust
pub fn get_audio_format(path: &Path) -> Option<AudioFileType>
```

Determines the audio format of a file.

**Parameters:**
- `path`: Path to the file

**Returns:**
- `Option<AudioFileType>`: The audio format if recognized

**Example:**
```rust
if let Some(format) = file_utils::get_audio_format(&file_path) {
    println!("Audio format: {:?}", format);
}
```

##### ensure_directory_exists

```rust
pub fn ensure_directory_exists(path: &Path) -> AppResult<()>
```

Ensures a directory exists, creating it if necessary.

**Parameters:**
- `path`: Path to the directory

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
file_utils::ensure_directory_exists(Path::new("/path/to/directory"))?;
```

##### get_file_size

```rust
pub fn get_file_size(path: &Path) -> AppResult<u64>
```

Gets the size of a file in bytes.

**Parameters:**
- `path`: Path to the file

**Returns:**
- `AppResult<u64>`: File size in bytes

**Example:**
```rust
let size = file_utils::get_file_size(&file_path)?;
println!("File size: {} bytes", size);
```

##### format_file_size

```rust
pub fn format_file_size(bytes: u64) -> String
```

Formats a file size in human-readable format.

**Parameters:**
- `bytes`: Size in bytes

**Returns:**
- `String`: Formatted size (e.g., "1.5 MB")

**Example:**
```rust
let formatted = file_utils::format_file_size(1572864);
println!("Size: {}", formatted); // "Size: 1.5 MB"
```

### Audio Processor

Audio processing utilities for analyzing and processing audio files.

#### Functions

##### get_audio_metadata

```rust
pub async fn get_audio_metadata(path: &Path) -> AppResult<AudioMetadata>
```

Extracts metadata from an audio file.

**Parameters:**
- `path`: Path to the audio file

**Returns:**
- `AppResult<AudioMetadata>`: Audio metadata

**Example:**
```rust
let metadata = audio_processor::get_audio_metadata(&file_path).await?;
println!("Duration: {:?}", metadata.duration);
println!("Sample rate: {}", metadata.sample_rate);
```

##### validate_audio_file

```rust
pub async fn validate_audio_file(path: &Path) -> AppResult<ValidationResult>
```

Validates that a file is a valid audio file.

**Parameters:**
- `path`: Path to the file to validate

**Returns:**
- `AppResult<ValidationResult>`: Validation result

**Example:**
```rust
let result = audio_processor::validate_audio_file(&file_path).await?;
if result.is_valid {
    println!("Valid audio file");
} else {
    println!("Invalid audio file: {}", result.error_message);
}
```

##### convert_audio_format

```rust
pub async fn convert_audio_format(
    input_path: &Path,
    output_path: &Path,
    target_format: AudioFileType,
) -> AppResult<()>
```

Converts an audio file to a different format.

**Parameters:**
- `input_path`: Path to the input file
- `output_path`: Path for the output file
- `target_format`: Target audio format

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
audio_processor::convert_audio_format(
    Path::new("/path/to/input.wav"),
    Path::new("/path/to/output.mp3"),
    AudioFileType::Mp3
).await?;
```

##### generate_waveform_data

```rust
pub async fn generate_waveform_data(
    path: &Path,
    width: u32,
    height: u32,
) -> AppResult<Vec<f32>>
```

Generates waveform data for visualization.

**Parameters:**
- `path`: Path to the audio file
- `width`: Width of the waveform visualization
- `height`: Height of the waveform visualization

**Returns:**
- `AppResult<Vec<f32>>`: Waveform data points

**Example:**
```rust
let waveform = audio_processor::generate_waveform_data(
    &file_path,
    800,
    200
).await?;
// Use the waveform data for visualization
```

### Platform Integration

Platform-specific utilities for desktop integration.

#### Functions

##### detect_desktop_environment

```rust
pub fn detect_desktop_environment() -> DesktopEnvironment
```

Detects the current desktop environment.

**Returns:**
- `DesktopEnvironment`: Detected desktop environment

**Example:**
```rust
let desktop = platform::detect_desktop_environment();
match desktop {
    DesktopEnvironment::GNOME => println!("Running on GNOME"),
    DesktopEnvironment::KDE => println!("Running on KDE"),
    DesktopEnvironment::XFCE => println!("Running on XFCE"),
    DesktopEnvironment::Other => println!("Unknown desktop environment"),
}
```

##### show_notification

```rust
pub fn show_notification(title: &str, message: &str) -> AppResult<()>
```

Shows a desktop notification.

**Parameters:**
- `title`: Notification title
- `message`: Notification message

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
platform::show_notification(
    "Transcription Complete",
    "Your audio file has been transcribed successfully."
)?;
```

##### open_file_manager

```rust
pub fn open_file_manager(path: &Path) -> AppResult<()>
```

Opens the file manager at a specific path.

**Parameters:**
- `path`: Path to open in the file manager

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
platform::open_file_manager(Path::new("/path/to/transcriptions"))?;
```

##### register_file_associations

```rust
pub fn register_file_associations() -> AppResult<()>
```

Registers file associations for the application.

**Returns:**
- `AppResult<()>`: Success or error

**Example:**
```rust
platform::register_file_associations()?;
```

##### get_system_theme

```rust
pub fn get_system_theme() -> Theme
```

Gets the current system theme.

**Returns:**
- `Theme`: Current system theme (Light or Dark)

**Example:**
```rust
let theme = platform::get_system_theme();
match theme {
    Theme::Light => apply_light_theme(),
    Theme::Dark => apply_dark_theme(),
}
```

## Data Models API

### AppState

The central application state that manages all data.

#### Constructor

```rust
pub fn new() -> Self
```

#### Methods

##### add_audio_file

```rust
pub async fn add_audio_file(&self, file: AudioFile)
```

Adds an audio file to the application state.

**Parameters:**
- `file`: Audio file to add

##### get_audio_file

```rust
pub async fn get_audio_file(&self, id: Uuid) -> Option<AudioFile>
```

Retrieves an audio file by ID.

**Parameters:**
- `id`: ID of the file to retrieve

**Returns:**
- `Option<AudioFile>`: The audio file if found

##### add_transcription_task

```rust
pub async fn add_transcription_task(&self, task: TranscriptionTask)
```

Adds a transcription task to the application state.

**Parameters:**
- `task`: Transcription task to add

##### update_transcription_task

```rust
pub async fn update_transcription_task<F>(&self, id: Uuid, updater: F)
where
    F: FnOnce(&mut TranscriptionTask)
```

Updates a transcription task.

**Parameters:**
- `id`: ID of the task to update
- `updater`: Function that updates the task

##### set_status_message

```rust
pub async fn set_status_message(&self, message: String)
```

Sets the status message displayed in the UI.

**Parameters:**
- `message`: Status message to display

##### get_selected_model

```rust
pub async fn get_selected_model(&self) -> Option<Model>
```

Gets the currently selected model.

**Returns:**
- `Option<Model>`: The selected model if any

##### set_selected_model

```rust
pub async fn set_selected_model(&self, model: Option<Model>)
```

Sets the currently selected model.

**Parameters:**
- `model`: Model to select, or None to clear selection

### AudioFile

Represents an audio file in the application.

#### Constructor

```rust
pub fn new(file_path: PathBuf) -> Self
```

**Parameters:**
- `file_path`: Path to the audio file

#### Methods

##### is_supported

```rust
pub fn is_supported(&self) -> bool
```

Checks if the file format is supported.

**Returns:**
- `bool`: True if the format is supported

##### file_type

```rust
pub fn file_type(&self) -> AudioFileType
```

Gets the file type.

**Returns:**
- `AudioFileType`: The audio file type

##### mark_ready

```rust
pub fn mark_ready(&mut self, metadata: AudioMetadata)
```

Marks the file as ready with metadata.

**Parameters:**
- `metadata`: Extracted audio metadata

##### mark_failed

```rust
pub fn mark_failed(&mut self, error: String)
```

Marks the file as failed.

**Parameters:**
- `error`: Error message

### TranscriptionTask

Represents a transcription task.

#### Constructor

```rust
pub fn new(file_path: String, model_name: String, language: Option<String>) -> Self
```

**Parameters:**
- `file_path`: Path to the audio file
- `model_name`: Name of the model to use
- `language`: Optional language code

#### Methods

##### start

```rust
pub fn start(&mut self)
```

Marks the task as started.

##### update_progress

```rust
pub fn update_progress(&mut self, progress: f32)
```

Updates the task progress.

**Parameters:**
- `progress`: Progress value from 0.0 to 1.0

##### complete

```rust
pub fn complete(&mut self, result: TranscriptionResult)
```

Marks the task as completed.

**Parameters:**
- `result`: Transcription result

##### fail

```rust
pub fn fail(&mut self, error: String)
```

Marks the task as failed.

**Parameters:**
- `error`: Error message

##### cancel

```rust
pub fn cancel(&mut self)
```

Cancels the task.

##### is_finished

```rust
pub fn is_finished(&self) -> bool
```

Checks if the task is finished (completed, failed, or canceled).

**Returns:**
- `bool`: True if the task is finished

### Model

Represents an AI transcription model.

#### Constructor

```rust
pub fn new(
    name: String,
    display_name: String,
    model_type: ModelType,
    size_bytes: u64,
    description: Option<String>,
) -> Self
```

**Parameters:**
- `name`: Internal model name
- `display_name`: Human-readable name
- `model_type`: Type of model
- `size_bytes`: Size of the model in bytes
- `description`: Optional description

#### Methods

##### is_downloaded

```rust
pub fn is_downloaded(&self) -> bool
```

Checks if the model is downloaded.

**Returns:**
- `bool`: True if the model is downloaded

##### supports_language

```rust
pub fn supports_language(&self, language: &str) -> bool
```

Checks if the model supports a specific language.

**Parameters:**
- `language`: Language code to check

**Returns:**
- `bool`: True if the language is supported

### Settings

Represents application settings.

#### Methods

##### load

```rust
pub fn load() -> AppResult<Self>
```

Loads settings from the default location.

**Returns:**
- `AppResult<Settings>`: Loaded settings

##### save

```rust
pub fn save(&self) -> AppResult<()>
```

Saves settings to the default location.

**Returns:**
- `AppResult<()>`: Success or error

##### reset_to_defaults

```rust
pub fn reset_to_defaults(&mut self)
```

Resets all settings to their default values.

##### validate

```rust
pub fn validate(&self) -> AppResult<()>
```

Validates the current settings.

**Returns:**
- `AppResult<()>`: Success or validation error

This API documentation provides a comprehensive reference for all services, utilities, and data models used in the ASR Pro Linux frontend. Each API includes detailed parameter descriptions, return values, and usage examples to help developers understand and use the interfaces effectively.