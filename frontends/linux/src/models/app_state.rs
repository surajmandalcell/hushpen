use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{
    AudioFile, Model, TranscriptionTask, TranscriptionConfig, FileConfig, ModelConfig,
    TranscriptionStats, FileStats, ModelStats, BackendConfig, FileStatus,
    websocket::{WsMessage, SubscriptionChannel, ConnectionState},
};
use crate::services::{BackendClient, FileManager, ConfigManager};
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
    /// File manager for handling file operations
    pub file_manager: Arc<FileManager>,
    /// Configuration manager for handling application settings
    pub config_manager: Arc<ConfigManager>,
    /// WebSocket client for real-time updates
    pub websocket_client: Arc<RwLock<Option<Arc<crate::services::WebSocketClient>>>>,
    /// WebSocket connection state
    pub websocket_state: Arc<RwLock<ConnectionState>>,
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
    /// WebSocket connection status for UI display
    pub websocket_status: String,
    /// Whether to show WebSocket connection status
    pub show_websocket_status: bool,
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
    pub fn new() -> AppResult<Self> {
        // Create the configuration manager
        let config_manager = Arc::new(ConfigManager::with_default_path()?);
        
        // Create the file manager with default config
        let file_manager = Arc::new(FileManager::with_default_config());
        
        Ok(Self {
            transcription: Arc::new(RwLock::new(TranscriptionState::default())),
            files: Arc::new(RwLock::new(FileState::default())),
            models: Arc::new(RwLock::new(ModelState::default())),
            config: Arc::new(RwLock::new(ConfigState::default())),
            ui: Arc::new(RwLock::new(UiState::default())),
            backend_client: Arc::new(RwLock::new(None)),
            file_manager,
            config_manager,
            websocket_client: Arc::new(RwLock::new(None)),
            websocket_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
        })
    }
    
    /// Initialize the application state with default values
    pub async fn initialize(&self) -> Result<(), AppError> {
        // Load configuration from file
        self.config_manager.load_config().await?;
        
        // Initialize configuration
        self.init_config().await?;
        
        // Initialize models
        self.init_models().await?;
        
        // Initialize file directories
        self.init_file_directories().await?;
        
        // Initialize the file manager with loaded config
        self.init_file_manager().await?;
        
        // Initialize WebSocket if enabled
        self.init_websocket().await?;
        
        Ok(())
    }
    
    /// Initialize configuration
    async fn init_config(&self) -> Result<(), AppError> {
        let app_config = self.config_manager.get_config().await;
        let mut config = self.config.write().await;
        
        // Load values from the configuration manager
        config.backend_url = app_config.backend_config.base_url.clone();
        config.api_key = app_config.backend_config.api_key.clone();
        config.dark_mode = app_config.user_preferences.ui_preferences.theme == "dark";
        config.theme = app_config.user_preferences.ui_preferences.theme.clone();
        config.language = app_config.user_preferences.ui_preferences.language.clone();
        config.auto_save = true; // This could be added to AppConfig
        config.auto_save_interval = 30; // This could be added to AppConfig
        config.show_notifications = app_config.user_preferences.notification_preferences.enabled;
        config.log_level = "info".to_string(); // This could be added to AppConfig
        
        Ok(())
    }
    
    /// Initialize the file manager with configuration
    async fn init_file_manager(&self) -> Result<(), AppError> {
        let file_config = self.config_manager.get_file_config().await;
        self.file_manager.update_config(file_config).await?;
        
        // Set the backend client if available
        {
            let backend_client_guard = self.backend_client.read().await;
            if let Some(ref client) = *backend_client_guard {
                // We need to update the file manager with the backend client
                // but FileManager doesn't have a method to update the client after creation
                // For now, we'll just note this limitation
            }
        }
        
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
        let file_config = self.config_manager.get_file_config().await;
        
        // Create directories if they don't exist
        std::fs::create_dir_all(&file_config.temp_dir)
            .map_err(|e| AppError::file_with_source("Failed to create temp directory", e))?;
        
        std::fs::create_dir_all(&file_config.upload_dir)
            .map_err(|e| AppError::file_with_source("Failed to create upload directory", e))?;
        
        // Update the file state with the config
        let mut files = self.files.write().await;
        files.config = file_config;
        
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
        // Get the current configuration state
        let config_state = self.config.read().await;
        
        // Create an AppConfig from the current state
        let mut app_config = self.config_manager.get_config().await;
        
        // Update backend config
        app_config.backend_config.base_url = config_state.backend_url.clone();
        app_config.backend_config.api_key = config_state.api_key.clone();
        
        // Update UI preferences
        app_config.user_preferences.ui_preferences.theme = config_state.theme.clone();
        app_config.user_preferences.ui_preferences.language = config_state.language.clone();
        
        // Update notification preferences
        app_config.user_preferences.notification_preferences.enabled = config_state.show_notifications;
        
        // Save the configuration
        self.config_manager.update_config(app_config).await?;
        self.config_manager.save_config().await?;
        
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
    
    /// Add a file using the file manager
    pub async fn add_file_from_path(&self, file_path: std::path::PathBuf) -> AppResult<uuid::Uuid> {
        let audio_file = self.file_manager.add_file(file_path.clone()).await?;
        let file_id = audio_file.id;
        self.add_audio_file(audio_file).await;
        
        // Update the last opened directory
        if let Some(parent) = file_path.parent() {
            self.config_manager.set_last_opened_directory(parent.to_path_buf()).await?;
        }
        
        Ok(file_id)
    }
    
    /// Upload a file to the backend
    pub async fn upload_file(
        &self,
        file_id: uuid::Uuid,
        progress_callback: Option<std::sync::Arc<dyn Fn(f32) + Send + Sync>>,
    ) -> AppResult<()> {
        // Get the file
        let file = self.get_audio_file(file_id).await
            .ok_or_else(|| AppError::generic(format!("File with ID {} not found", file_id)))?;
        
        // Mark the file as uploading
        self.update_audio_file(file_id, |file| {
            file.mark_uploading();
        }).await?;
        
        // Upload the file
        self.file_manager.upload_file(file_id, &file.file_path, progress_callback).await?;
        
        // Mark the file as ready
        self.update_audio_file(file_id, |file| {
            file.status = FileStatus::Ready;
        }).await?;
        
        Ok(())
    }
    
    /// Get the file manager
    pub fn get_file_manager(&self) -> Arc<FileManager> {
        Arc::clone(&self.file_manager)
    }
    
    /// Get the configuration manager
    pub fn get_config_manager(&self) -> Arc<ConfigManager> {
        Arc::clone(&self.config_manager)
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
    
    /// Initialize WebSocket connection
    async fn init_websocket(&self) -> Result<(), AppError> {
        // Get the backend client
        let backend_client_guard = self.backend_client.read().await;
        if let Some(ref backend_client) = *backend_client_guard {
            // Get the WebSocket client from the backend client
            if let Some(ws_client) = backend_client.get_websocket_client() {
                // Store the WebSocket client
                let mut ws_client_guard = self.websocket_client.write().await;
                *ws_client_guard = Some(ws_client.clone());
                
                // Register event handlers
                self.register_websocket_handlers(&ws_client).await;
                
                // Connect to WebSocket
                if let Err(e) = ws_client.connect().await {
                    eprintln!("Failed to connect to WebSocket: {}", e);
                    // Don't fail initialization, just note the error
                }
            }
        }
        
        Ok(())
    }
    
    /// Register WebSocket event handlers
    async fn register_websocket_handlers(&self, ws_client: &Arc<crate::services::WebSocketClient>) {
        // Register handler for transcription started
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("TranscriptionStarted", move |message| {
            // Handle transcription started
            if let WsMessage::TranscriptionStarted(event) = message {
                // Update UI state
                // Note: This is a simplified example
                println!("Transcription started: {}", event.base.id);
            }
        }).await;
        
        // Register handler for transcription progress
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("TranscriptionProgress", move |message| {
            // Handle transcription progress
            if let WsMessage::TranscriptionProgress(event) = message {
                // Update UI state
                println!("Transcription progress: {}%", event.progress);
            }
        }).await;
        
        // Register handler for transcription completed
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("TranscriptionCompleted", move |message| {
            // Handle transcription completed
            if let WsMessage::TranscriptionCompleted(event) = message {
                // Update UI state
                println!("Transcription completed: {}", event.base.id);
            }
        }).await;
        
        // Register handler for file upload progress
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("FileUploadProgress", move |message| {
            // Handle file upload progress
            if let WsMessage::FileUploadProgress(event) = message {
                // Update UI state
                println!("File upload progress: {}%", event.progress);
            }
        }).await;
        
        // Register handler for model download progress
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("ModelDownloadProgress", move |message| {
            // Handle model download progress
            if let WsMessage::ModelDownloadProgress(event) = message {
                // Update UI state
                println!("Model download progress: {}%", event.progress);
            }
        }).await;
        
        // Register handler for connection events
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("Connected", move |message| {
            // Handle connection established
            if let WsMessage::Connected { session_id } = message {
                // Update UI state
                println!("WebSocket connected: {}", session_id);
            }
        }).await;
        
        // Register handler for disconnection events
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("Disconnected", move |message| {
            // Handle disconnection
            if let WsMessage::Disconnected { reason } = message {
                // Update UI state
                println!("WebSocket disconnected: {}", reason);
            }
        }).await;
        
        // Register handler for error events
        let ws_client = ws_client.clone();
        
        ws_client.register_handler("Error", move |message| {
            // Handle errors
            if let WsMessage::Error { error } = message {
                // Update UI state
                eprintln!("WebSocket error: {}", error);
            }
        }).await;
    }
    
    /// Get the WebSocket client
    pub async fn get_websocket_client(&self) -> Option<Arc<crate::services::WebSocketClient>> {
        let ws_client_guard = self.websocket_client.read().await;
        ws_client_guard.clone()
    }
    
    /// Get the WebSocket connection state
    pub async fn get_websocket_state(&self) -> ConnectionState {
        let state = self.websocket_state.read().await;
        state.clone()
    }
    
    /// Update the WebSocket connection state
    pub async fn update_websocket_state(&self, state: ConnectionState) {
        let mut ws_state = self.websocket_state.write().await;
        *ws_state = state.clone();
        
        // Update UI state
        let status = match state {
            ConnectionState::Connected => "Connected".to_string(),
            ConnectionState::Connecting => "Connecting...".to_string(),
            ConnectionState::Reconnecting => "Reconnecting...".to_string(),
            ConnectionState::Disconnected => "Disconnected".to_string(),
            ConnectionState::Failed => "Connection Failed".to_string(),
        };
        
        self.update_ui_state(|ui| {
            ui.websocket_status = status;
            ui.show_websocket_status = state != ConnectionState::Connected;
        }).await;
    }
    
    /// Subscribe to a WebSocket channel
    pub async fn websocket_subscribe(&self, channel: SubscriptionChannel) -> AppResult<()> {
        if let Some(ws_client) = self.get_websocket_client().await {
            ws_client.subscribe(channel).await?;
        }
        Ok(())
    }
    
    /// Unsubscribe from a WebSocket channel
    pub async fn websocket_unsubscribe(&self, channel: SubscriptionChannel) -> AppResult<()> {
        if let Some(ws_client) = self.get_websocket_client().await {
            ws_client.unsubscribe(channel).await?;
        }
        Ok(())
    }
    
    /// Handle a WebSocket message
    pub async fn handle_websocket_message(&self, message: WsMessage) {
        match message {
            WsMessage::TranscriptionStarted(event) => {
                // Update transcription task
                self.update_transcription_task(event.task_id, |task| {
                    task.start();
                }).await.ok();
                
                // Update UI
                self.set_status_message(format!("Transcription started: {}", event.filename)).await;
            },
            WsMessage::TranscriptionProgress(event) => {
                // Update transcription task
                self.update_transcription_task(event.task_id, |task| {
                    task.update_progress(event.progress);
                }).await.ok();
                
                // Update UI
                self.set_progress(event.progress / 100.0).await;
                self.set_status_message(event.status).await;
            },
            WsMessage::TranscriptionCompleted(event) => {
                // Update transcription task
                self.update_transcription_task(event.task_id, |task| {
                    task.complete(crate::models::TranscriptionResult {
                        id: event.task_id,
                        text: event.text,
                        confidence: 1.0, // Default confidence
                        language: event.language,
                        audio_duration: std::time::Duration::from_secs_f64(event.duration),
                        segments: event.segments.into_iter().map(|s| crate::models::TranscriptionSegment {
                            text: s.text,
                            start: std::time::Duration::from_secs_f64(s.start),
                            end: std::time::Duration::from_secs_f64(s.end),
                            confidence: s.confidence,
                        }).collect(),
                        completed_at: chrono::Utc::now(),
                    });
                }).await.ok();
                
                // Update UI
                self.set_status_message("Transcription completed".to_string()).await;
                self.set_progress(1.0).await;
            },
            WsMessage::TranscriptionFailed(event) => {
                // Update transcription task
                self.update_transcription_task(event.task_id, |task| {
                    task.fail(event.error);
                }).await.ok();
                
                // Update UI
                self.set_status_message(format!("Transcription failed: {}", event.error)).await;
            },
            WsMessage::FileUploadProgress(event) => {
                // Update file
                self.update_audio_file(event.file_id, |file| {
                    file.upload_progress = Some(event.progress);
                }).await.ok();
                
                // Update UI
                self.set_status_message(format!("Uploading {}: {}%", event.filename, event.progress)).await;
            },
            WsMessage::FileUploadCompleted(event) => {
                // Update file
                self.update_audio_file(event.file_id, |file| {
                    file.upload_progress = Some(100.0);
                    file.status = crate::models::FileStatus::Ready;
                }).await.ok();
                
                // Update UI
                self.set_status_message(format!("Upload completed: {}", event.filename)).await;
            },
            WsMessage::FileUploadFailed(event) => {
                // Update file
                self.update_audio_file(event.file_id, |file| {
                    file.status = crate::models::FileStatus::Error;
                }).await.ok();
                
                // Update UI
                self.set_status_message(format!("Upload failed: {}", event.error)).await;
            },
            WsMessage::ModelDownloadProgress(event) => {
                // Update model
                let models = self.models.read().await;
                for model in models.models.values() {
                    if model.name == event.model_name {
                        // Update model download progress
                        // This would require adding download_progress to Model
                        break;
                    }
                }
                
                // Update UI
                self.set_status_message(format!("Downloading {}: {}%", event.model_name, event.progress)).await;
            },
            WsMessage::ModelDownloadCompleted(event) => {
                // Update UI
                self.set_status_message(format!("Model download completed: {}", event.model_name)).await;
            },
            WsMessage::ModelDownloadFailed(event) => {
                // Update UI
                self.set_status_message(format!("Model download failed: {}", event.error)).await;
            },
            WsMessage::Connected { session_id } => {
                self.update_websocket_state(ConnectionState::Connected).await;
                self.set_status_message(format!("Connected: {}", session_id)).await;
            },
            WsMessage::Disconnected { reason } => {
                self.update_websocket_state(ConnectionState::Disconnected).await;
                self.set_status_message(format!("Disconnected: {}", reason)).await;
            },
            WsMessage::Error { error } => {
                self.update_websocket_state(ConnectionState::Failed).await;
                self.set_status_message(format!("WebSocket error: {}", error)).await;
            },
            _ => {
                // Handle other message types as needed
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new().expect("Failed to create default AppState")
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