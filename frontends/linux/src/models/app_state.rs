use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{
    AudioFile, Model, TranscriptionTask, TranscriptionConfig, FileConfig, ModelConfig,
    TranscriptionStats, FileStats, ModelStats, BackendConfig,
};
use crate::services::BackendClient;
use crate::utils::{AppError, AppResult};

/// Centralized application state using Arc<Mutex<T>> pattern for thread safety
#[derive(Debug)]
pub struct AppState {
    /// Transcription-related state
    pub transcription: Arc<RwLock<TranscriptionState>>,
    /// File-related state
    pub files: Arc<RwLock<FileState>>,
    /// Model-related state
    pub models: Arc<RwLock<ModelState>>,
    /// Application configuration
    pub config: Arc<RwLock<ConfigState>>,
    /// UI-related state
    pub ui: Arc<RwLock<UiState>>,
    /// Backend client for API communication
    pub backend_client: Arc<RwLock<Option<BackendClient>>>,
}

/// Transcription-related state
#[derive(Debug, Default)]
pub struct TranscriptionState {
    /// Active transcription tasks
    pub active_tasks: HashMap<Uuid, TranscriptionTask>,
    /// Completed transcription tasks
    pub completed_tasks: HashMap<Uuid, TranscriptionTask>,
    /// Current transcription configuration
    pub config: TranscriptionConfig,
    /// Transcription statistics
    pub stats: TranscriptionStats,
    /// Currently selected task
    pub selected_task_id: Option<Uuid>,
}

/// File-related state
#[derive(Debug, Default)]
pub struct FileState {
    /// All audio files
    pub files: HashMap<Uuid, AudioFile>,
    /// Currently selected file
    pub selected_file_id: Option<Uuid>,
    /// File configuration
    pub config: FileConfig,
    /// File statistics
    pub stats: FileStats,
    /// Recently accessed files
    pub recent_files: Vec<Uuid>,
}

/// Model-related state
#[derive(Debug, Default)]
pub struct ModelState {
    /// Available models
    pub models: HashMap<Uuid, Model>,
    /// Currently selected model
    pub selected_model_id: Option<Uuid>,
    /// Model configuration
    pub config: ModelConfig,
    /// Model statistics
    pub stats: ModelStats,
    /// Currently loaded models
    pub loaded_models: Vec<Uuid>,
}

/// Application configuration state
#[derive(Debug, Default)]
pub struct ConfigState {
    /// Backend URL
    pub backend_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Whether to use dark mode
    pub dark_mode: bool,
    /// Application theme
    pub theme: String,
    /// Language preference
    pub language: String,
    /// Whether to auto-save
    pub auto_save: bool,
    /// Auto-save interval in seconds
    pub auto_save_interval: u64,
    /// Whether to show notifications
    pub show_notifications: bool,
    /// Log level
    pub log_level: String,
}

/// UI-related state
#[derive(Debug, Default)]
pub struct UiState {
    /// Current view/page
    pub current_view: String,
    /// Whether the UI is busy
    pub is_busy: bool,
    /// Current progress (0.0 to 1.0)
    pub progress: f32,
    /// Status message
    pub status_message: String,
    /// Whether to show the settings dialog
    pub show_settings: bool,
    /// Whether to show the about dialog
    pub show_about: bool,
    /// Window state
    pub window_state: WindowState,
}

/// Window state information
#[derive(Debug, Default)]
pub struct WindowState {
    /// Window width
    pub width: i32,
    /// Window height
    pub height: i32,
    /// Whether the window is maximized
    pub is_maximized: bool,
    /// Window x position
    pub x: i32,
    /// Window y position
    pub y: i32,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            transcription: Arc::new(RwLock::new(TranscriptionState::default())),
            files: Arc::new(RwLock::new(FileState::default())),
            models: Arc::new(RwLock::new(ModelState::default())),
            config: Arc::new(RwLock::new(ConfigState::default())),
            ui: Arc::new(RwLock::new(UiState::default())),
            backend_client: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Initialize the application state with default values
    pub async fn initialize(&self) -> Result<(), AppError> {
        // Initialize configuration
        self.init_config().await?;
        
        // Initialize models
        self.init_models().await?;
        
        // Initialize file directories
        self.init_file_directories().await?;
        
        Ok(())
    }
    
    /// Initialize configuration
    async fn init_config(&self) -> Result<(), AppError> {
        let mut config = self.config.write().await;
        
        // Set default values
        config.backend_url = "http://localhost:8000".to_string();
        config.dark_mode = true;
        config.theme = "default".to_string();
        config.language = "en".to_string();
        config.auto_save = true;
        config.auto_save_interval = 30; // 30 seconds
        config.show_notifications = true;
        config.log_level = "info".to_string();
        
        // Try to load configuration from file
        self.load_config_from_file(&mut *config).await?;
        
        Ok(())
    }
    
    /// Initialize models
    async fn init_models(&self) -> Result<(), AppError> {
        let mut models = self.models.write().await;
        
        // Add default Whisper models
        for model in crate::models::presets::whisper_models() {
            models.models.insert(model.id, model);
        }
        
        // Mark the first model as selected
        if let Some(first_model) = models.models.values().next() {
            models.selected_model_id = Some(first_model.id);
        }
        
        Ok(())
    }
    
    /// Initialize file directories
    async fn init_file_directories(&self) -> Result<(), AppError> {
        let files = self.files.read().await;
        
        // Create directories if they don't exist
        std::fs::create_dir_all(&files.config.temp_dir)
            .map_err(|e| AppError::file_with_source("Failed to create temp directory", e))?;
        
        std::fs::create_dir_all(&files.config.upload_dir)
            .map_err(|e| AppError::file_with_source("Failed to create upload directory", e))?;
        
        Ok(())
    }
    
    /// Load configuration from file
    async fn load_config_from_file(&self, _config: &mut ConfigState) -> Result<(), AppError> {
        // This would typically load from a config file
        // For now, we'll use the defaults
        Ok(())
    }
    
    /// Save configuration to file
    pub async fn save_config(&self) -> Result<(), AppError> {
        let _config = self.config.read().await;
        
        // This would typically save to a config file
        // For now, we'll just return Ok
        Ok(())
    }
    
    /// Get the current transcription state
    pub async fn get_transcription_state(&self) -> tokio::sync::RwLockReadGuard<'_, TranscriptionState> {
        self.transcription.read().await
    }
    
    /// Get the current file state
    pub async fn get_file_state(&self) -> tokio::sync::RwLockReadGuard<'_, FileState> {
        self.files.read().await
    }
    
    /// Get the current model state
    pub async fn get_model_state(&self) -> tokio::sync::RwLockReadGuard<'_, ModelState> {
        self.models.read().await
    }
    
    /// Get the current config state
    pub async fn get_config_state(&self) -> tokio::sync::RwLockReadGuard<'_, ConfigState> {
        self.config.read().await
    }
    
    /// Get the current UI state
    pub async fn get_ui_state(&self) -> tokio::sync::RwLockReadGuard<'_, UiState> {
        self.ui.read().await
    }
    
    /// Update the UI state
    pub async fn update_ui_state<F, R>(&self, updater: F) -> R
    where
        F: FnOnce(&mut UiState) -> R,
    {
        let mut ui = self.ui.write().await;
        updater(&mut *ui)
    }
    
    /// Add a new transcription task
    pub async fn add_transcription_task(&self, task: TranscriptionTask) {
        let mut transcription = self.transcription.write().await;
        transcription.active_tasks.insert(task.id, task);
    }
    
    /// Get a transcription task by ID
    pub async fn get_transcription_task(&self, task_id: Uuid) -> Option<TranscriptionTask> {
        let transcription = self.transcription.read().await;
        
        // First check active tasks
        if let Some(task) = transcription.active_tasks.get(&task_id) {
            return Some(task.clone());
        }
        
        // Then check completed tasks
        if let Some(task) = transcription.completed_tasks.get(&task_id) {
            return Some(task.clone());
        }
        
        None
    }
    
    /// Update a transcription task
    pub async fn update_transcription_task<F>(&self, task_id: Uuid, updater: F) -> Result<(), AppError>
    where
        F: FnOnce(&mut TranscriptionTask),
    {
        let mut transcription = self.transcription.write().await;
        
        // First try to update active task
        if let Some(task) = transcription.active_tasks.get_mut(&task_id) {
            updater(task);
            
            // If the task is finished, move it to completed tasks
            if task.is_finished() {
                let task = transcription.active_tasks.remove(&task_id).unwrap();
                
                // Update statistics before moving
                if let Some(ref result) = task.result {
                    transcription.stats.update(result);
                }
                
                transcription.completed_tasks.insert(task_id, task);
            }
            
            return Ok(());
        }
        
        // Try to update completed task
        if let Some(task) = transcription.completed_tasks.get_mut(&task_id) {
            updater(task);
            return Ok(());
        }
        
        Err(AppError::generic(format!("Task with ID {} not found", task_id)))
    }
    
    /// Add a new audio file
    pub async fn add_audio_file(&self, file: AudioFile) {
        let mut files = self.files.write().await;
        files.files.insert(file.id, file.clone());
        
        // Update recent files
        files.recent_files.retain(|&id| id != file.id);
        files.recent_files.push(file.id);
        
        // Keep only the 10 most recent files
        if files.recent_files.len() > 10 {
            files.recent_files.remove(0);
        }
        
        // Update statistics
        files.stats.update(&file);
    }
    
    /// Get an audio file by ID
    pub async fn get_audio_file(&self, file_id: Uuid) -> Option<AudioFile> {
        let files = self.files.read().await;
        files.files.get(&file_id).cloned()
    }
    
    /// Update an audio file
    pub async fn update_audio_file<F>(&self, file_id: Uuid, updater: F) -> Result<(), AppError>
    where
        F: FnOnce(&mut AudioFile),
    {
        let mut files = self.files.write().await;
        
        if let Some(file) = files.files.get_mut(&file_id) {
            updater(file);
            Ok(())
        } else {
            Err(AppError::generic(format!("File with ID {} not found", file_id)))
        }
    }
    
    /// Get the selected model
    pub async fn get_selected_model(&self) -> Option<Model> {
        let models = self.models.read().await;
        
        if let Some(model_id) = models.selected_model_id {
            models.models.get(&model_id).cloned()
        } else {
            None
        }
    }
    
    /// Set the selected model
    pub async fn set_selected_model(&self, model_id: Uuid) -> Result<(), AppError> {
        let mut models = self.models.write().await;
        
        if models.models.contains_key(&model_id) {
            models.selected_model_id = Some(model_id);
            Ok(())
        } else {
            Err(AppError::generic(format!("Model with ID {} not found", model_id)))
        }
    }
    
    /// Update the status message
    pub async fn set_status_message(&self, message: String) {
        self.update_ui_state(|ui| {
            ui.status_message = message;
        }).await;
    }
    
    /// Update the progress
    pub async fn set_progress(&self, progress: f32) {
        self.update_ui_state(|ui| {
            ui.progress = progress.clamp(0.0, 1.0);
        }).await;
    }
    
    /// Set the busy state
    pub async fn set_busy(&self, busy: bool) {
        self.update_ui_state(|ui| {
            ui.is_busy = busy;
        }).await;
    }
    
    /// Show a notification
    pub async fn show_notification(&self, _title: String, message: String) {
        let config = self.config.read().await;
        
        if config.show_notifications {
            // This would typically show a system notification
            // For now, we'll just update the status message
            self.set_status_message(message).await;
        }
    }
    
    /// Initialize the backend client with the current configuration
    pub async fn initialize_backend_client(&self) -> AppResult<()> {
        let config = self.config.read().await;
        
        let backend_config = BackendConfig {
            base_url: config.backend_url.clone(),
            api_key: config.api_key.clone(),
            timeout: 60,
            max_retries: 3,
            retry_delay: 1000,
            enable_websocket: true,
        };
        
        let client = BackendClient::new(backend_config)?;
        
        let mut backend_client_guard = self.backend_client.write().await;
        *backend_client_guard = Some(client);
        
        Ok(())
    }
    
    /// Get the backend client
    pub async fn get_backend_client(&self) -> AppResult<Arc<RwLock<Option<BackendClient>>>> {
        // Ensure the client is initialized
        {
            let backend_client_guard = self.backend_client.read().await;
            if backend_client_guard.is_none() {
                drop(backend_client_guard);
                self.initialize_backend_client().await?;
            }
        }
        
        Ok(Arc::clone(&self.backend_client))
    }
    
    /// Execute an operation with the backend client
    pub async fn with_backend_client<F, R, Fut>(&self, operation: F) -> AppResult<R>
    where
        F: FnOnce(Arc<BackendClient>) -> Fut,
        Fut: std::future::Future<Output = AppResult<R>> + Send,
    {
        // Ensure the client is initialized
        {
            let backend_client_guard = self.backend_client.read().await;
            if backend_client_guard.is_none() {
                drop(backend_client_guard);
                self.initialize_backend_client().await?;
            }
        }
        
        let backend_client_guard = self.backend_client.read().await;
        let client = backend_client_guard.as_ref()
            .ok_or_else(|| AppError::api("Failed to initialize backend client"))?;
        
        // Clone the client for the operation
        let client_arc = Arc::new(client.clone());
        operation(client_arc).await
    }
    
    /// Update the backend configuration
    pub async fn update_backend_config(&self, backend_url: String, api_key: Option<String>) -> AppResult<()> {
        // Update the configuration
        {
            let mut config = self.config.write().await;
            config.backend_url = backend_url.clone();
            config.api_key = api_key;
        }
        
        // Reinitialize the backend client with new configuration
        self.initialize_backend_client().await
    }
    
    /// Check if the backend is healthy
    pub async fn check_backend_health(&self) -> AppResult<bool> {
        self.with_backend_client(|client| async move {
            match client.health_check().await {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }).await
    }
    
    /// Get available models from the backend
    pub async fn get_backend_models(&self) -> AppResult<Vec<crate::models::api::ModelResponse>> {
        self.with_backend_client(|client| async move {
            let models = client.list_models().await?;
            Ok(models.data)
        }).await
    }
    
    /// Set the active model on the backend
    pub async fn set_backend_model(&self, model_id: &str) -> AppResult<crate::models::api::ModelSettingResponse> {
        let model_id = model_id.to_string(); // Convert to owned String
        self.with_backend_client(move |client| async move {
            client.set_model(&model_id).await
        }).await
    }
    
    /// Transcribe audio using the backend
    pub async fn transcribe_audio(&self, request: crate::models::api::TranscriptionRequest) -> AppResult<crate::models::api::TranscriptionResponse> {
        self.with_backend_client(|client| async move {
            client.transcribe_audio(request).await
        }).await
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{TranscriptionStatus, AudioFileType};
    use std::path::PathBuf;
    
    #[tokio::test]
    async fn test_app_state_creation() {
        let state = AppState::new();
        
        // Test that all states are initialized
        let transcription = state.get_transcription_state().await;
        assert!(transcription.active_tasks.is_empty());
        assert!(transcription.completed_tasks.is_empty());
        
        let files = state.get_file_state().await;
        assert!(files.files.is_empty());
        
        let models = state.get_model_state().await;
        assert!(!models.models.is_empty()); // Should have default models
        
        let config = state.get_config_state().await;
        assert_eq!(config.backend_url, "http://localhost:8000");
        
        let ui = state.get_ui_state().await;
        assert!(!ui.is_busy);
        assert_eq!(ui.progress, 0.0);
    }
    
    #[tokio::test]
    async fn test_transcription_task_management() {
        let state = AppState::new();
        
        // Create a new task
        let task = TranscriptionTask::new(
            "test.mp3".to_string(),
            "whisper-1".to_string(),
            Some("en".to_string()),
        );
        let task_id = task.id;
        
        // Add the task
        state.add_transcription_task(task).await;
        
        // Get the task
        let retrieved_task = state.get_transcription_task(task_id).await;
        assert!(retrieved_task.is_some());
        assert_eq!(retrieved_task.unwrap().status, TranscriptionStatus::Pending);
        
        // Update the task
        state.update_transcription_task(task_id, |task| {
            task.start();
        }).await.unwrap();
        
        let updated_task = state.get_transcription_task(task_id).await;
        assert!(updated_task.is_some());
        assert_eq!(updated_task.unwrap().status, TranscriptionStatus::InProgress);
    }
    
    #[tokio::test]
    async fn test_audio_file_management() {
        let state = AppState::new();
        
        // Create a new file
        let file_path = PathBuf::from("/test/audio.mp3");
        let mut file = AudioFile::new(file_path);
        let file_id = file.id;
        
        // Add the file
        state.add_audio_file(file).await;
        
        // Get the file
        let retrieved_file = state.get_audio_file(file_id).await;
        assert!(retrieved_file.is_some());
        assert_eq!(retrieved_file.unwrap().file_name, "audio.mp3");
        
        // Update the file
        state.update_audio_file(file_id, |file| {
            file.mark_ready(crate::models::AudioMetadata {
                duration: std::time::Duration::from_secs(60),
                sample_rate: 16000,
                channels: 1,
                bit_rate: Some(128000),
                format: AudioFileType::Mp3,
                file_size: 1024 * 1024, // 1MB
                title: None,
                artist: None,
                album: None,
                date: None,
                genre: None,
            });
        }).await.unwrap();
        
        let updated_file = state.get_audio_file(file_id).await;
        assert!(updated_file.is_some());
        assert!(updated_file.unwrap().processed);
    }
    
    #[tokio::test]
    async fn test_ui_state_updates() {
        let state = AppState::new();
        
        // Set status message
        state.set_status_message("Test message".to_string()).await;
        let ui = state.get_ui_state().await;
        assert_eq!(ui.status_message, "Test message");
        
        // Set progress
        state.set_progress(0.5).await;
        let ui = state.get_ui_state().await;
        assert_eq!(ui.progress, 0.5);
        
        // Set busy state
        state.set_busy(true).await;
        let ui = state.get_ui_state().await;
        assert!(ui.is_busy);
    }
}