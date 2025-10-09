//! Services for the ASRPro application
//!
//! This module contains various services that handle communication with external systems,
//! such as the backend API, file operations, and audio processing.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::models::AppState;
use crate::utils::{AppError, AppResult};

pub mod backend_client;
pub mod file_manager;
pub mod config_manager;
pub mod model_manager;
pub mod transcription_service;
pub mod websocket_client;

pub use backend_client::BackendClient;
pub use file_manager::FileManager;
pub use config_manager::ConfigManager;
pub use model_manager::ModelManager;
pub use websocket_client::WebSocketClient;

/// API service for communicating with the backend
pub struct ApiClient {
    http_client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            base_url,
            api_key,
        }
    }
    
    /// Get the health status of the backend
    pub async fn health_check(&self) -> AppResult<serde_json::Value> {
        let mut request = self.http_client.get(format!("{}/health", self.base_url));
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| AppError::api_with_source("Failed to send health check request", e))?;
        
        if response.status().is_success() {
            let health = response
                .json()
                .await
                .map_err(|e| AppError::api_with_source("Failed to parse health check response", e))?;
            Ok(health)
        } else {
            Err(AppError::api(format!(
                "Health check failed with status: {}",
                response.status()
            )))
        }
    }
    
    /// Start a transcription task
    pub async fn start_transcription(
        &self,
        file_path: &str,
        model: &str,
        language: Option<&str>,
        config: &crate::models::TranscriptionConfig,
    ) -> AppResult<uuid::Uuid> {
        let file_bytes = std::fs::read(file_path)
            .map_err(|e| AppError::file_with_source("Failed to read audio file", e))?;
        
        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("audio.mp3")
                .to_string());
        
        let mut form = reqwest::multipart::Form::new()
            .part("audio", file_part)
            .text("model", model.to_string());
        
        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }
        
        if config.include_timestamps {
            form = form.text("timestamp", "true".to_string());
        }
        
        if config.include_segments {
            form = form.text("segments", "true".to_string());
        }
        
        let mut request = self.http_client
            .post(format!("{}/transcribe", self.base_url))
            .multipart(form);
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| AppError::api_with_source("Failed to start transcription", e))?;
        
        if response.status().is_success() {
            let result: serde_json::Value = response
                .json()
                .await
                .map_err(|e| AppError::api_with_source("Failed to parse transcription response", e))?;
            
            let task_id = result
                .get("task_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::api("No task_id in response"))?;
            
            let uuid = uuid::Uuid::parse_str(task_id)
                .map_err(|e| AppError::api_with_source("Invalid task_id in response", e))?;
            
            Ok(uuid)
        } else {
            Err(AppError::api(format!(
                "Failed to start transcription with status: {}",
                response.status()
            )))
        }
    }
    
    /// Get the status of a transcription task
    pub async fn get_transcription_status(&self, task_id: uuid::Uuid) -> AppResult<serde_json::Value> {
        let mut request = self.http_client
            .get(format!("{}/transcribe/{}", self.base_url, task_id));
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| AppError::api_with_source("Failed to get transcription status", e))?;
        
        if response.status().is_success() {
            let status = response
                .json()
                .await
                .map_err(|e| AppError::api_with_source("Failed to parse transcription status", e))?;
            Ok(status)
        } else {
            Err(AppError::api(format!(
                "Failed to get transcription status with status: {}",
                response.status()
            )))
        }
    }
    
    /// Get available models
    pub async fn get_models(&self) -> AppResult<Vec<crate::models::Model>> {
        let mut request = self.http_client.get(format!("{}/models", self.base_url));
        
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        
        let response = request
            .send()
            .await
            .map_err(|e| AppError::api_with_source("Failed to get models", e))?;
        
        if response.status().is_success() {
            let models: Vec<serde_json::Value> = response
                .json()
                .await
                .map_err(|e| AppError::api_with_source("Failed to parse models response", e))?;
            
            // Convert JSON models to Model structs
            let mut result = Vec::new();
            for model_json in models {
                // This is a simplified conversion - in a real implementation,
                // you'd properly map all fields
                let name = model_json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                let display_name = model_json
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&name)
                    .to_string();
                
                let size_bytes = model_json
                    .get("size_bytes")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                
                let model = crate::models::Model::new(
                    name.clone(),
                    display_name,
                    crate::models::ModelType::Whisper,
                    size_bytes,
                    None,
                );
                
                result.push(model);
            }
            
            Ok(result)
        } else {
            Err(AppError::api(format!(
                "Failed to get models with status: {}",
                response.status()
            )))
        }
    }
}

/// File service for handling file operations
pub struct FileService {
    app_state: Arc<RwLock<AppState>>,
}

impl FileService {
    /// Create a new file service
    pub fn new(app_state: Arc<RwLock<AppState>>) -> Self {
        Self { app_state }
    }
    
    /// Add a new audio file
    pub async fn add_file(&self, file_path: std::path::PathBuf) -> AppResult<uuid::Uuid> {
        // Check if the file exists
        if !file_path.exists() {
            return Err(AppError::file("File does not exist"));
        }
        
        // Create a new audio file
        let mut audio_file = crate::models::AudioFile::new(file_path.clone());
        
        // Extract metadata if available
        if audio_file.is_supported() {
            // In a real implementation, you'd extract metadata here
            // For now, we'll just mark it as ready
            audio_file.mark_ready(crate::models::AudioMetadata {
                duration: std::time::Duration::from_secs(60), // Default 1 minute
                sample_rate: 16000,
                channels: 1,
                bit_rate: Some(128000),
                format: audio_file.file_type(),
                file_size: std::fs::metadata(&file_path)
                    .map(|m| m.len())
                    .unwrap_or(0),
                title: None,
                artist: None,
                album: None,
                date: None,
                genre: None,
            });
        } else {
            audio_file.mark_failed("Unsupported file format".to_string());
        }
        
        let file_id = audio_file.id;
        
        // Add the file to the application state
        let app_state = self.app_state.read().await;
        app_state.add_audio_file(audio_file).await;
        
        Ok(file_id)
    }
    
    /// Get a file by ID
    pub async fn get_file(&self, file_id: uuid::Uuid) -> AppResult<crate::models::AudioFile> {
        let app_state = self.app_state.read().await;
        
        app_state
            .get_audio_file(file_id)
            .await
            .ok_or_else(|| AppError::file(format!("File with ID {} not found", file_id)))
    }
    
    /// Delete a file
    pub async fn delete_file(&self, file_id: uuid::Uuid) -> AppResult<()> {
        let app_state = self.app_state.read().await;
        
        // Get the file to check if it exists
        let _file = app_state
            .get_audio_file(file_id)
            .await
            .ok_or_else(|| AppError::file(format!("File with ID {} not found", file_id)))?;
        
        // In a real implementation, you'd delete the file from disk here
        // For now, we'll just remove it from the state
        
        // This would require a method to remove files from the state
        // which is not currently implemented
        
        Ok(())
    }
}

/// Transcription service for handling transcription tasks
pub struct TranscriptionService {
    app_state: Arc<RwLock<AppState>>,
    api_client: Arc<ApiClient>,
}

impl TranscriptionService {
    /// Create a new transcription service
    pub fn new(app_state: Arc<RwLock<AppState>>, api_client: Arc<ApiClient>) -> Self {
        Self {
            app_state,
            api_client,
        }
    }
    
    /// Start a new transcription task
    pub async fn start_transcription(
        &self,
        file_id: uuid::Uuid,
        _model_id: uuid::Uuid,
        language: Option<String>,
    ) -> AppResult<uuid::Uuid> {
        let app_state = self.app_state.read().await;
        
        // Get the file
        let file = app_state
            .get_audio_file(file_id)
            .await
            .ok_or_else(|| AppError::file(format!("File with ID {} not found", file_id)))?;
        
        // Get the model
        let model = app_state.get_selected_model().await
            .ok_or_else(|| AppError::generic("No model selected"))?;
        
        // Create a new transcription task
        let model_name = model.name.clone();
        let task = crate::models::TranscriptionTask::new(
            file.file_path.to_string_lossy().to_string(),
            model_name.clone(),
            language.clone(),
        );
        
        let task_id = task.id;
        
        // Add the task to the state
        app_state.add_transcription_task(task).await;
        
        // Start the transcription in the background
        let api_client = Arc::clone(&self.api_client);
        let app_state_clone = Arc::clone(&self.app_state);
        let file_path = file.file_path.clone();
        
        tokio::spawn(async move {
            // Update task status to InProgress
            let app_state_for_update = app_state_clone.read().await;
            let _ = app_state_for_update.update_transcription_task(task_id, |task| {
                task.start();
            }).await;
            drop(app_state_for_update);
            
            // Start the transcription via API
            let result = api_client.start_transcription(
                &file_path.to_string_lossy(),
                &model_name,
                language.as_deref(),
                &app_state_clone.read().await.get_transcription_state().await.config,
            ).await;
            
            match result {
                Ok(api_task_id) => {
                    // Update the task with the API task ID
                    let app_state_for_update = app_state_clone.read().await;
                    let _ = app_state_for_update.update_transcription_task(task_id, |_task| {
                        // Store the API task ID for future reference
                        // This would require adding an api_task_id field to TranscriptionTask
                    }).await;
                    drop(app_state_for_update);
                    
                    // Start polling for status updates
                    Self::poll_transcription_status(
                        Arc::clone(&app_state_clone),
                        Arc::clone(&api_client),
                        task_id,
                        api_task_id,
                    ).await;
                },
                Err(e) => {
                    // Mark the task as failed
                    let app_state_for_update = app_state_clone.read().await;
                    let _ = app_state_for_update.update_transcription_task(task_id, |task| {
                        task.fail(e.to_string());
                    }).await;
                }
            }
        });
        
        Ok(task_id)
    }
    
    /// Poll for transcription status updates
    async fn poll_transcription_status(
        app_state: Arc<RwLock<AppState>>,
        api_client: Arc<ApiClient>,
        task_id: uuid::Uuid,
        api_task_id: uuid::Uuid,
    ) {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
        
        loop {
            interval.tick().await;
            
            // Get the current task
            let task = {
                let state = app_state.read().await;
                state.get_transcription_task(task_id).await
            };
            
            let task = match task {
                Some(t) => t,
                None => break, // Task no longer exists
            };
            
            // If the task is finished, stop polling
            if task.is_finished() {
                break;
            }
            
            // Poll the API for status
            match api_client.get_transcription_status(api_task_id).await {
                Ok(status) => {
                    let status_str = status
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    
                    let progress = status
                        .get("progress")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) as f32;
                    
                    // Update the task
                    let app_state_for_update = app_state.read().await;
                    let _ = app_state_for_update.update_transcription_task(task_id, |task| {
                        task.update_progress(progress);
                        
                        match status_str {
                            "completed" => {
                                if let Some(result) = status.get("result") {
                                    // Parse the result and mark the task as completed
                                    // This would require proper parsing of the result
                                    task.complete(crate::models::TranscriptionResult {
                                        id: task.id,
                                        text: result
                                            .get("text")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string(),
                                        confidence: result
                                            .get("confidence")
                                            .and_then(|v| v.as_f64())
                                            .unwrap_or(0.0) as f32,
                                        language: result
                                            .get("language")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown")
                                            .to_string(),
                                        audio_duration: std::time::Duration::from_secs(60), // Default
                                        segments: Vec::new(), // Parse segments if available
                                        completed_at: chrono::Utc::now(),
                                    });
                                }
                            },
                            "failed" => {
                                let error = status
                                    .get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Unknown error");
                                task.fail(error.to_string());
                            },
                            _ => {
                                // Task is still in progress
                            }
                        }
                    }).await;
                },
                Err(e) => {
                    // Mark the task as failed
                    let app_state_for_update = app_state.read().await;
                    let _ = app_state_for_update.update_transcription_task(task_id, |task| {
                        task.fail(format!("Failed to get status: {}", e));
                    }).await;
                    break;
                }
            }
        }
    }
    
    /// Get a transcription task by ID
    pub async fn get_task(&self, task_id: uuid::Uuid) -> AppResult<crate::models::TranscriptionTask> {
        let app_state = self.app_state.read().await;
        
        app_state
            .get_transcription_task(task_id)
            .await
            .ok_or_else(|| AppError::generic(format!("Task with ID {} not found", task_id)))
    }
    
    /// Cancel a transcription task
    pub async fn cancel_task(&self, task_id: uuid::Uuid) -> AppResult<()> {
        let app_state = self.app_state.read().await;
        
        app_state
            .update_transcription_task(task_id, |task| {
                task.cancel();
            })
            .await
    }
}