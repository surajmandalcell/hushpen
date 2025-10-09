//! Transcription Service
//!
//! This module provides a service for managing transcription operations,
//! including submitting files for transcription and processing results.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::AppState;
use crate::models::transcription::{TranscriptionTask, TranscriptionResult, TranscriptionConfig, TranscriptionSegment};
use crate::models::file::AudioFile;
use crate::models::websocket::{SubscriptionChannel, WsMessage};
use crate::services::{BackendClient, FileManager};
use crate::utils::{AppError, AppResult};

/// Transcription service for managing transcription operations
pub struct TranscriptionService {
    /// Application state
    app_state: Arc<RwLock<AppState>>,
    /// Backend client for API communication
    backend_client: Option<Arc<BackendClient>>,
    /// File manager for file operations
    file_manager: Option<Arc<FileManager>>,
}

impl TranscriptionService {
    /// Create a new transcription service
    pub fn new(
        app_state: Arc<RwLock<AppState>>,
        backend_client: Option<Arc<BackendClient>>,
        file_manager: Option<Arc<FileManager>>,
    ) -> Self {
        Self {
            app_state,
            backend_client,
            file_manager,
        }
    }

    /// Start a new transcription task
    pub async fn start_transcription(
        &self,
        file_id: Uuid,
        model_name: String,
        language: Option<String>,
        config: Option<TranscriptionConfig>,
    ) -> AppResult<Uuid> {
        // Get the file
        let file = {
            let state = self.app_state.read().await;
            state.get_audio_file(file_id).await
                .ok_or_else(|| AppError::file(format!("File with ID {} not found", file_id)))?
        };

        // Use default config if none provided
        let config = config.unwrap_or_default();

        // Create a new transcription task
        let mut task = TranscriptionTask::new(
            file.file_path.to_string_lossy().to_string(),
            model_name.clone(),
            language.clone(),
        );

        // Set the task ID for tracking
        let task_id = task.id;

        // Add the task to the state
        {
            let state = self.app_state.read().await;
            state.add_transcription_task(task.clone()).await;
            
            // Subscribe to WebSocket updates for this task
            if let Err(e) = state.websocket_subscribe(SubscriptionChannel::TranscriptionTask(task_id)).await {
                eprintln!("Failed to subscribe to transcription updates: {}", e);
            }
        }

        // Start the transcription in the background
        let app_state = Arc::clone(&self.app_state);
        let backend_client = self.backend_client.clone();
        let file_path = file.file_path.clone();

        tokio::spawn(async move {
            // Mark the task as started
            {
                let state = app_state.read().await;
                let _ = state.update_transcription_task(task_id, |task| {
                    task.start();
                }).await;
            }

            // If we have a backend client, use it
            if let Some(client) = backend_client {
                match Self::transcribe_with_backend(
                    client,
                    &file_path,
                    &model_name,
                    language.as_deref(),
                    &config,
                ).await {
                    Ok(result) => {
                        // Update the task with the result
                        let state = app_state.read().await;
                        let _ = state.update_transcription_task(task_id, |task| {
                            task.complete(result);
                        }).await;
                    },
                    Err(e) => {
                        // Mark the task as failed
                        let state = app_state.read().await;
                        let _ = state.update_transcription_task(task_id, |task| {
                            task.fail(e.to_string());
                        }).await;
                    }
                }
            } else {
                // Simulate transcription for demo purposes
                match Self::simulate_transcription(&file_path, &config).await {
                    Ok(result) => {
                        // Update the task with the result
                        let state = app_state.read().await;
                        let _ = state.update_transcription_task(task_id, |task| {
                            task.complete(result);
                        }).await;
                    },
                    Err(e) => {
                        // Mark the task as failed
                        let state = app_state.read().await;
                        let _ = state.update_transcription_task(task_id, |task| {
                            task.fail(e.to_string());
                        }).await;
                    }
                }
            }
        });

        Ok(task_id)
    }

    /// Transcribe using the backend client
    async fn transcribe_with_backend(
        client: Arc<BackendClient>,
        file_path: &std::path::Path,
        model_name: &str,
        language: Option<&str>,
        config: &TranscriptionConfig,
    ) -> AppResult<TranscriptionResult> {
        // This would use the actual backend client to transcribe
        // For now, we'll simulate it
        Self::simulate_transcription(file_path, config).await
    }

    /// Simulate transcription for demo purposes
    async fn simulate_transcription(
        file_path: &std::path::Path,
        config: &TranscriptionConfig,
    ) -> AppResult<TranscriptionResult> {
        // Simulate processing time
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Create a mock transcription result
        let text = "This is a simulated transcription of the audio file. In a real implementation, this would be the actual transcribed text from the speech recognition model. The transcription would include proper punctuation, capitalization, and formatting based on the configuration options provided.";
        
        // Create segments if timestamps are enabled
        let segments = if config.include_timestamps {
            vec![
                TranscriptionSegment {
                    text: "This is a simulated transcription of the audio file.".to_string(),
                    start: Duration::from_secs(0),
                    end: Duration::from_secs(3),
                    confidence: 0.95,
                },
                TranscriptionSegment {
                    text: "In a real implementation, this would be the actual transcribed text from the speech recognition model.".to_string(),
                    start: Duration::from_secs(3),
                    end: Duration::from_secs(8),
                    confidence: 0.92,
                },
                TranscriptionSegment {
                    text: "The transcription would include proper punctuation, capitalization, and formatting based on the configuration options provided.".to_string(),
                    start: Duration::from_secs(8),
                    end: Duration::from_secs(14),
                    confidence: 0.89,
                },
            ]
        } else {
            vec![]
        };

        // Get file metadata
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| AppError::file_with_source("Failed to read file metadata", e))?;
        
        // Estimate audio duration (this is a rough estimate)
        let file_size = metadata.len();
        let estimated_duration = Duration::from_secs(file_size / (16000 * 2)); // Assuming 16kHz, 16-bit audio

        Ok(TranscriptionResult {
            id: Uuid::new_v4(),
            text: text.to_string(),
            confidence: 0.92,
            language: "en".to_string(),
            audio_duration: estimated_duration,
            segments,
            completed_at: chrono::Utc::now(),
        })
    }

    /// Get a transcription task by ID
    pub async fn get_task(&self, task_id: Uuid) -> AppResult<TranscriptionTask> {
        let state = self.app_state.read().await;
        state.get_transcription_task(task_id).await
            .ok_or_else(|| AppError::generic(format!("Task with ID {} not found", task_id)))
    }

    /// Cancel a transcription task
    pub async fn cancel_task(&self, task_id: Uuid) -> AppResult<()> {
        let state = self.app_state.read().await;
        state.update_transcription_task(task_id, |task| {
            task.cancel();
        }).await
    }

    /// Get all transcription tasks
    pub async fn get_all_tasks(&self) -> AppResult<Vec<TranscriptionTask>> {
        let state = self.app_state.read().await;
        Ok(state.get_all_transcription_tasks().await)
    }

    /// Get completed transcription results
    pub async fn get_completed_results(&self) -> AppResult<Vec<TranscriptionResult>> {
        let state = self.app_state.read().await;
        let tasks = state.get_all_transcription_tasks().await;
        
        let mut results = Vec::new();
        for task in tasks {
            if let Some(result) = task.result {
                results.push(result);
            }
        }
        
        Ok(results)
    }

    /// Delete a transcription task
    pub async fn delete_task(&self, task_id: Uuid) -> AppResult<()> {
        let state = self.app_state.read().await;
        // This would require implementing a method to remove tasks from the state
        // For now, we'll just mark it as cancelled
        state.update_transcription_task(task_id, |task| {
            task.cancel();
        }).await
    }

    /// Retry a failed transcription task
    pub async fn retry_task(&self, task_id: Uuid) -> AppResult<Uuid> {
        let task = self.get_task(task_id).await?;
        
        if !task.is_finished() {
            return Err(AppError::generic("Task is not finished and cannot be retried"));
        }
        
        // Create a new task with the same parameters
        self.start_transcription_from_file_path(
            &task.audio_file_path,
            task.model_name.clone(),
            task.language.clone(),
            None,
        ).await
    }

    /// Start transcription from a file path
    pub async fn start_transcription_from_file_path(
        &self,
        file_path: &str,
        model_name: String,
        language: Option<String>,
        config: Option<TranscriptionConfig>,
    ) -> AppResult<Uuid> {
        // Create a temporary audio file
        let audio_file = AudioFile::new(std::path::PathBuf::from(file_path));
        let file_id = audio_file.id;
        
        // Add the file to the state
        {
            let state = self.app_state.read().await;
            state.add_audio_file(audio_file).await;
        }
        
        // Start the transcription
        self.start_transcription(file_id, model_name, language, config).await
    }

    /// Update transcription configuration
    pub async fn update_config(&self, config: TranscriptionConfig) -> AppResult<()> {
        let state = self.app_state.read().await;
        state.set_transcription_config(config).await;
        Ok(())
    }

    /// Get transcription configuration
    pub async fn get_config(&self) -> AppResult<TranscriptionConfig> {
        let state = self.app_state.read().await;
        Ok(state.get_transcription_state().await.config.clone())
    }

    /// Export transcription result to a file
    pub async fn export_result(
        &self,
        task_id: Uuid,
        file_path: &str,
        format: ExportFormat,
    ) -> AppResult<()> {
        let task = self.get_task(task_id).await?;
        let result = task.result
            .ok_or_else(|| AppError::generic("Task does not have a result"))?;
        
        let content = match format {
            ExportFormat::PlainText => {
                // Export as plain text
                let mut content = String::new();
                for segment in &result.segments {
                    content.push_str(&segment.text);
                    content.push(' ');
                }
                content
            },
            ExportFormat::SRT => {
                // Export as SRT
                let mut content = String::new();
                for (i, segment) in result.segments.iter().enumerate() {
                    content.push_str(&format!("{}\n", i + 1));
                    
                    let start_time = format_seconds_to_srt_time(
                        segment.start.as_secs() as u32,
                        segment.start.subsec_millis()
                    );
                    let end_time = format_seconds_to_srt_time(
                        segment.end.as_secs() as u32,
                        segment.end.subsec_millis()
                    );
                    content.push_str(&format!("{} --> {}\n", start_time, end_time));
                    
                    content.push_str(&segment.text);
                    content.push_str("\n\n");
                }
                content
            },
            ExportFormat::VTT => {
                // Export as VTT
                let mut content = String::new();
                content.push_str("WEBVTT\n\n");
                
                for segment in &result.segments {
                    let start_time = format_seconds_to_vtt_time(
                        segment.start.as_secs() as u32,
                        segment.start.subsec_millis()
                    );
                    let end_time = format_seconds_to_vtt_time(
                        segment.end.as_secs() as u32,
                        segment.end.subsec_millis()
                    );
                    content.push_str(&format!("{} --> {}\n", start_time, end_time));
                    
                    content.push_str(&segment.text);
                    content.push_str("\n\n");
                }
                content
            },
            ExportFormat::JSON => {
                // Export as JSON
                serde_json::to_string_pretty(&result)
                    .map_err(|e| AppError::generic_with_source("Failed to serialize result", e))?
            },
        };
        
        std::fs::write(file_path, content)
            .map_err(|e| AppError::file_with_source("Failed to write export file", e))?;
        
        Ok(())
    }
}

/// Export format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportFormat {
    PlainText,
    SRT,
    VTT,
    JSON,
}

/// Helper function to format seconds to SRT time format (HH:MM:SS,mmm)
fn format_seconds_to_srt_time(seconds: u32, milliseconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, milliseconds)
}

/// Helper function to format seconds to VTT time format (HH:MM:SS.mmm)
fn format_seconds_to_vtt_time(seconds: u32, milliseconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, milliseconds)
}