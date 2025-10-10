//! Mock implementations for testing
//!
//! This module provides mock implementations of various components
//! for testing purposes, including mock services, clients, and data.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use asrpro_gtk4::{
    models::{
        Model, ModelType, ModelStatus, TranscriptionTask, TranscriptionStatus,
        TranscriptionResult, AudioFile, AudioMetadata, AudioFileType,
    },
    services::{BackendClient, ModelManager},
    utils::{AppError, AppResult},
};

/// Mock backend client for testing
pub struct MockBackendClient {
    pub models: Arc<RwLock<Vec<Model>>>,
    pub tasks: Arc<RwLock<std::collections::HashMap<Uuid, TranscriptionTask>>>,
    pub should_fail_health_check: Arc<RwLock<bool>>,
    pub should_fail_get_models: Arc<RwLock<bool>>,
    pub should_fail_start_transcription: Arc<RwLock<bool>>,
    pub should_fail_get_status: Arc<RwLock<bool>>,
}

impl MockBackendClient {
    /// Create a new mock backend client
    pub fn new() -> Self {
        let models = Arc::new(RwLock::new(vec![
            Model::new(
                "whisper-tiny".to_string(),
                "Whisper Tiny".to_string(),
                ModelType::Whisper,
                39 * 1024 * 1024,
                Some("Tiny Whisper model".to_string()),
            ),
            Model::new(
                "whisper-base".to_string(),
                "Whisper Base".to_string(),
                ModelType::Whisper,
                74 * 1024 * 1024,
                Some("Base Whisper model".to_string()),
            ),
        ]));
        
        Self {
            models,
            tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
            should_fail_health_check: Arc::new(RwLock::new(false)),
            should_fail_get_models: Arc::new(RwLock::new(false)),
            should_fail_start_transcription: Arc::new(RwLock::new(false)),
            should_fail_get_status: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Set whether health check should fail
    pub async fn set_health_check_failure(&self, should_fail: bool) {
        *self.should_fail_health_check.write().await = should_fail;
    }
    
    /// Set whether get models should fail
    pub async fn set_get_models_failure(&self, should_fail: bool) {
        *self.should_fail_get_models.write().await = should_fail;
    }
    
    /// Set whether start transcription should fail
    pub async fn set_start_transcription_failure(&self, should_fail: bool) {
        *self.should_fail_start_transcription.write().await = should_fail;
    }
    
    /// Set whether get status should fail
    pub async fn set_get_status_failure(&self, should_fail: bool) {
        *self.should_fail_get_status.write().await = should_fail;
    }
    
    /// Add a model to the mock backend
    pub async fn add_model(&self, model: Model) {
        self.models.write().await.push(model);
    }
    
    /// Get a model by name
    pub async fn get_model(&self, name: &str) -> Option<Model> {
        self.models.read().await
            .iter()
            .find(|m| m.name == name)
            .cloned()
    }
    
    /// Add a task to the mock backend
    pub async fn add_task(&self, task: TranscriptionTask) {
        self.tasks.write().await.insert(task.id, task);
    }
    
    /// Get a task by ID
    pub async fn get_task(&self, task_id: Uuid) -> Option<TranscriptionTask> {
        self.tasks.read().await.get(&task_id).cloned()
    }
    
    /// Update a task status
    pub async fn update_task_status(&self, task_id: Uuid, status: TranscriptionStatus) {
        if let Some(task) = self.tasks.write().await.get_mut(&task_id) {
            match status {
                TranscriptionStatus::Queued => task.start(),
                TranscriptionStatus::InProgress => task.update_progress(0.5),
                TranscriptionStatus::Completed => {
                    let result = TranscriptionResult {
                        id: task_id,
                        text: "Mock transcription result".to_string(),
                        confidence: 0.95,
                        language: "en".to_string(),
                        audio_duration: Duration::from_secs(60),
                        segments: vec![],
                        completed_at: chrono::Utc::now(),
                    };
                    task.complete(result);
                },
                TranscriptionStatus::Failed => task.fail("Mock failure".to_string()),
                TranscriptionStatus::Cancelled => task.cancel(),
            }
        }
    }
}

/// Mock model manager for testing
pub struct MockModelManager {
    pub backend_client: Arc<MockBackendClient>,
    pub models: Arc<RwLock<Vec<Model>>>,
    pub selected_model: Arc<RwLock<Option<Model>>>,
    pub should_fail_initialize: Arc<RwLock<bool>>,
    pub should_fail_load_model: Arc<RwLock<bool>>,
    pub should_fail_unload_model: Arc<RwLock<bool>>,
}

impl MockModelManager {
    /// Create a new mock model manager
    pub fn new(backend_client: Arc<MockBackendClient>) -> Self {
        Self {
            backend_client,
            models: Arc::new(RwLock::new(Vec::new())),
            selected_model: Arc::new(RwLock::new(None)),
            should_fail_initialize: Arc::new(RwLock::new(false)),
            should_fail_load_model: Arc::new(RwLock::new(false)),
            should_fail_unload_model: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Set whether initialize should fail
    pub async fn set_initialize_failure(&self, should_fail: bool) {
        *self.should_fail_initialize.write().await = should_fail;
    }
    
    /// Set whether load model should fail
    pub async fn set_load_model_failure(&self, should_fail: bool) {
        *self.should_fail_load_model.write().await = should_fail;
    }
    
    /// Set whether unload model should fail
    pub async fn set_unload_model_failure(&self, should_fail: bool) {
        *self.should_fail_unload_model.write().await = should_fail;
    }
    
    /// Get the selected model
    pub async fn get_selected_model(&self) -> Option<Model> {
        self.selected_model.read().await.clone()
    }
    
    /// Set the selected model
    pub async fn set_selected_model(&self, model: Option<Model>) {
        *self.selected_model.write().await = model;
    }
    
    /// Get all available models
    pub async fn get_models(&self) -> Vec<Model> {
        self.models.read().await.clone()
    }
    
    /// Add a model
    pub async fn add_model(&self, model: Model) {
        self.models.write().await.push(model);
    }
}

/// Mock audio file for testing
pub struct MockAudioFile {
    pub id: Uuid,
    pub file_path: std::path::PathBuf,
    pub file_type: AudioFileType,
    pub metadata: Option<AudioMetadata>,
    pub status: asrpro_gtk4::models::FileStatus,
}

impl MockAudioFile {
    /// Create a new mock audio file
    pub fn new(file_path: std::path::PathBuf) -> Self {
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown");
        
        let file_type = AudioFileType::from_extension(extension);
        
        Self {
            id: Uuid::new_v4(),
            file_path,
            file_type,
            metadata: None,
            status: asrpro_gtk4::models::FileStatus::Pending,
        }
    }
    
    /// Set the metadata
    pub fn with_metadata(mut self, metadata: AudioMetadata) -> Self {
        self.metadata = Some(metadata);
        self.status = asrpro_gtk4::models::FileStatus::Ready;
        self
    }
    
    /// Set the status
    pub fn with_status(mut self, status: asrpro_gtk4::models::FileStatus) -> Self {
        self.status = status;
        self
    }
    
    /// Mark as ready
    pub fn mark_ready(mut self) -> Self {
        self.status = asrpro_gtk4::models::FileStatus::Ready;
        self
    }
    
    /// Mark as failed
    pub fn mark_failed(mut self, error: String) -> Self {
        self.status = asrpro_gtk4::models::FileStatus::Failed(error);
        self
    }
}

/// Mock transcription task for testing
pub struct MockTranscriptionTask {
    pub id: Uuid,
    pub file_path: String,
    pub model_name: String,
    pub language: Option<String>,
    pub status: TranscriptionStatus,
    pub progress: f32,
    pub result: Option<TranscriptionResult>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl MockTranscriptionTask {
    /// Create a new mock transcription task
    pub fn new(file_path: String, model_name: String, language: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            file_path,
            model_name,
            language,
            status: TranscriptionStatus::Queued,
            progress: 0.0,
            result: None,
            error: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }
    
    /// Start the task
    pub fn start(&mut self) {
        self.status = TranscriptionStatus::InProgress;
        self.started_at = Some(chrono::Utc::now());
        self.progress = 0.1;
    }
    
    /// Update progress
    pub fn update_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }
    
    /// Complete the task
    pub fn complete(&mut self, result: TranscriptionResult) {
        self.status = TranscriptionStatus::Completed;
        self.progress = 1.0;
        self.result = Some(result);
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Fail the task
    pub fn fail(&mut self, error: String) {
        self.status = TranscriptionStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Cancel the task
    pub fn cancel(&mut self) {
        self.status = TranscriptionStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Check if the task is finished
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TranscriptionStatus::Completed | TranscriptionStatus::Failed | TranscriptionStatus::Cancelled
        )
    }
    
    /// Check if the task is completed
    pub fn is_completed(&self) -> bool {
        self.status == TranscriptionStatus::Completed
    }
    
    /// Check if the task is failed
    pub fn is_failed(&self) -> bool {
        self.status == TranscriptionStatus::Failed
    }
    
    /// Check if the task is cancelled
    pub fn is_cancelled(&self) -> bool {
        self.status == TranscriptionStatus::Cancelled
    }
    
    /// Check if the task is in progress
    pub fn is_in_progress(&self) -> bool {
        self.status == TranscriptionStatus::InProgress
    }
    
    /// Check if the task is queued
    pub fn is_queued(&self) -> bool {
        self.status == TranscriptionStatus::Queued
    }
}

/// Mock WebSocket client for testing
pub struct MockWebSocketClient {
    pub is_connected: Arc<RwLock<bool>>,
    pub messages: Arc<RwLock<Vec<serde_json::Value>>>,
    pub should_fail_connect: Arc<RwLock<bool>>,
    pub should_fail_send: Arc<RwLock<bool>>,
    pub should_fail_receive: Arc<RwLock<bool>>,
}

impl MockWebSocketClient {
    /// Create a new mock WebSocket client
    pub fn new() -> Self {
        Self {
            is_connected: Arc::new(RwLock::new(false)),
            messages: Arc::new(RwLock::new(Vec::new())),
            should_fail_connect: Arc::new(RwLock::new(false)),
            should_fail_send: Arc::new(RwLock::new(false)),
            should_fail_receive: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Set whether connect should fail
    pub async fn set_connect_failure(&self, should_fail: bool) {
        *self.should_fail_connect.write().await = should_fail;
    }
    
    /// Set whether send should fail
    pub async fn set_send_failure(&self, should_fail: bool) {
        *self.should_fail_send.write().await = should_fail;
    }
    
    /// Set whether receive should fail
    pub async fn set_receive_failure(&self, should_fail: bool) {
        *self.should_fail_receive.write().await = should_fail;
    }
    
    /// Connect to the WebSocket
    pub async fn connect(&self, url: &str) -> AppResult<()> {
        if *self.should_fail_connect.read().await {
            return Err(AppError::api("Failed to connect to WebSocket"));
        }
        
        *self.is_connected.write().await = true;
        Ok(())
    }
    
    /// Disconnect from the WebSocket
    pub async fn disconnect(&self) {
        *self.is_connected.write().await = false;
    }
    
    /// Send a message
    pub async fn send(&self, message: serde_json::Value) -> AppResult<()> {
        if *self.should_fail_send.read().await {
            return Err(AppError::api("Failed to send message"));
        }
        
        self.messages.write().await.push(message);
        Ok(())
    }
    
    /// Receive a message
    pub async fn receive(&self) -> AppResult<Option<serde_json::Value>> {
        if *self.should_fail_receive.read().await {
            return Err(AppError::api("Failed to receive message"));
        }
        
        let mut messages = self.messages.write().await;
        if messages.is_empty() {
            Ok(None)
        } else {
            Ok(Some(messages.remove(0)))
        }
    }
    
    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }
    
    /// Get the number of messages
    pub async fn message_count(&self) -> usize {
        self.messages.read().await.len()
    }
    
    /// Clear all messages
    pub async fn clear_messages(&self) {
        self.messages.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_backend_client() {
        let client = MockBackendClient::new();
        
        // Test default state
        assert!(!*client.should_fail_health_check.read().await);
        assert!(!*client.should_fail_get_models.read().await);
        
        // Test setting failure flags
        client.set_health_check_failure(true).await;
        assert!(*client.should_fail_health_check.read().await);
        
        // Test adding and getting models
        let model = Model::new(
            "test-model".to_string(),
            "Test Model".to_string(),
            ModelType::Whisper,
            1000000,
            Some("Test description".to_string()),
        );
        client.add_model(model.clone()).await;
        
        let retrieved_model = client.get_model("test-model").await;
        assert!(retrieved_model.is_some());
        assert_eq!(retrieved_model.unwrap().name, "test-model");
    }
    
    #[tokio::test]
    async fn test_mock_model_manager() {
        let backend_client = Arc::new(MockBackendClient::new());
        let manager = MockModelManager::new(backend_client);
        
        // Test default state
        assert!(manager.get_selected_model().await.is_none());
        assert!(manager.get_models().await.is_empty());
        
        // Test adding and selecting models
        let model = Model::new(
            "test-model".to_string(),
            "Test Model".to_string(),
            ModelType::Whisper,
            1000000,
            Some("Test description".to_string()),
        );
        manager.add_model(model.clone()).await;
        manager.set_selected_model(Some(model.clone())).await;
        
        let selected_model = manager.get_selected_model().await;
        assert!(selected_model.is_some());
        assert_eq!(selected_model.unwrap().name, "test-model");
    }
    
    #[test]
    fn test_mock_audio_file() {
        let file_path = std::path::PathBuf::from("/test/audio.mp3");
        let audio_file = MockAudioFile::new(file_path.clone())
            .with_metadata(AudioMetadata {
                duration: Duration::from_secs(60),
                sample_rate: 16000,
                channels: 1,
                bit_rate: Some(128000),
                format: AudioFileType::Mp3,
                file_size: 1024,
                title: None,
                artist: None,
                album: None,
                date: None,
                genre: None,
            });
        
        assert_eq!(audio_file.file_path, file_path);
        assert_eq!(audio_file.file_type, AudioFileType::Mp3);
        assert!(audio_file.metadata.is_some());
        assert_eq!(audio_file.status, asrpro_gtk4::models::FileStatus::Ready);
    }
    
    #[test]
    fn test_mock_transcription_task() {
        let mut task = MockTranscriptionTask::new(
            "/test/audio.mp3".to_string(),
            "whisper-tiny".to_string(),
            Some("en".to_string()),
        );
        
        assert_eq!(task.status, TranscriptionStatus::Queued);
        assert_eq!(task.progress, 0.0);
        assert!(!task.is_finished());
        
        task.start();
        assert_eq!(task.status, TranscriptionStatus::InProgress);
        assert!(task.is_in_progress());
        
        task.update_progress(0.5);
        assert_eq!(task.progress, 0.5);
        
        let result = TranscriptionResult {
            id: task.id,
            text: "Test transcription".to_string(),
            confidence: 0.95,
            language: "en".to_string(),
            audio_duration: Duration::from_secs(60),
            segments: vec![],
            completed_at: chrono::Utc::now(),
        };
        task.complete(result);
        
        assert_eq!(task.status, TranscriptionStatus::Completed);
        assert!(task.is_finished());
        assert!(task.is_completed());
        assert!(task.result.is_some());
    }
    
    #[tokio::test]
    async fn test_mock_websocket_client() {
        let client = MockWebSocketClient::new();
        
        // Test default state
        assert!(!client.is_connected().await);
        assert_eq!(client.message_count().await, 0);
        
        // Test connecting
        client.connect("ws://localhost:8080").await.unwrap();
        assert!(client.is_connected().await);
        
        // Test sending and receiving messages
        let message = serde_json::json!({"type": "test", "data": "hello"});
        client.send(message.clone()).await.unwrap();
        
        assert_eq!(client.message_count().await, 1);
        
        let received_message = client.receive().await.unwrap();
        assert!(received_message.is_some());
        assert_eq!(received_message.unwrap(), message);
        
        // Test disconnecting
        client.disconnect().await;
        assert!(!client.is_connected().await);
    }
}