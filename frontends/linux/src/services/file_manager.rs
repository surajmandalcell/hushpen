//! File management service for the ASRPro application
//!
//! This module provides a comprehensive file management service that handles
//! file operations, validation, metadata extraction, and integration with the backend.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{AudioFile, AudioMetadata, FileConfig, FileStatus};
use crate::services::BackendClient;
use crate::utils::{file_utils, AppError, AppResult};

/// File upload progress callback type
pub type UploadProgressCallback = Arc<dyn Fn(f32) + Send + Sync>;

/// File manager for handling all file operations
#[derive(Debug)]
pub struct FileManager {
    /// File configuration
    config: Arc<RwLock<FileConfig>>,
    /// Backend client for file uploads
    backend_client: Option<Arc<BackendClient>>,
    /// Active upload operations
    active_uploads: Arc<RwLock<std::collections::HashMap<Uuid, UploadInfo>>>,
}

/// Information about an active upload
struct UploadInfo {
    /// File ID
    file_id: Uuid,
    /// Upload progress (0.0 to 1.0)
    progress: f32,
    /// Optional progress callback
    callback: Option<UploadProgressCallback>,
    /// Upload start time
    start_time: std::time::Instant,
}

impl std::fmt::Debug for UploadInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadInfo")
            .field("file_id", &self.file_id)
            .field("progress", &self.progress)
            .field("callback", &self.callback.is_some())
            .field("start_time", &self.start_time)
            .finish()
    }
}

impl Clone for UploadInfo {
    fn clone(&self) -> Self {
        Self {
            file_id: self.file_id,
            progress: self.progress,
            callback: None, // Can't clone the callback
            start_time: self.start_time,
        }
    }
}

impl FileManager {
    /// Create a new file manager with the given configuration
    pub fn new(config: FileConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            backend_client: None,
            active_uploads: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create a file manager with default configuration
    pub fn with_default_config() -> Self {
        Self::new(FileConfig::default())
    }

    /// Set the backend client for file uploads
    pub fn set_backend_client(&mut self, client: Arc<BackendClient>) {
        self.backend_client = Some(client);
    }

    /// Get the current file configuration
    pub async fn get_config(&self) -> FileConfig {
        self.config.read().await.clone()
    }

    /// Update the file configuration
    pub async fn update_config(&self, new_config: FileConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        
        // Ensure directories exist
        file_utils::ensure_directory_exists(&config.temp_dir)?;
        file_utils::ensure_directory_exists(&config.upload_dir)?;
        
        Ok(())
    }

    /// Add a new audio file from a path
    pub async fn add_file(&self, file_path: PathBuf) -> AppResult<AudioFile> {
        // Validate the file
        let config = self.config.read().await;
        file_utils::validate_audio_file(&file_path, Some(config.max_file_size))?;
        
        // Create a new audio file
        let mut audio_file = AudioFile::new(file_path.clone());
        
        // Extract metadata if enabled
        if config.extract_metadata {
            match file_utils::extract_audio_metadata(&file_path) {
                Ok(metadata) => {
                    audio_file.mark_ready(metadata);
                }
                Err(e) => {
                    audio_file.mark_failed(format!("Failed to extract metadata: {}", e));
                }
            }
        } else {
            // Just mark as ready without detailed metadata
            let file_size = std::fs::metadata(&file_path)
                .map(|m| m.len())
                .unwrap_or(0);
            
            let metadata = AudioMetadata {
                duration: Duration::from_secs(60), // Default
                sample_rate: 16000,
                channels: 1,
                bit_rate: None,
                format: audio_file.file_type(),
                file_size,
                title: None,
                artist: None,
                album: None,
                date: None,
                genre: None,
            };
            
            audio_file.mark_ready(metadata);
        }
        
        Ok(audio_file)
    }

    /// Upload a file to the backend
    pub async fn upload_file(
        &self,
        file_id: Uuid,
        file_path: &Path,
        progress_callback: Option<UploadProgressCallback>,
    ) -> AppResult<()> {
        // Check if we have a backend client
        let backend_client = self.backend_client.as_ref()
            .ok_or_else(|| AppError::api("No backend client configured"))?;

        // Create upload info
        let upload_info = UploadInfo {
            file_id,
            progress: 0.0,
            callback: progress_callback.clone(),
            start_time: std::time::Instant::now(),
        };

        // Add to active uploads
        {
            let mut active_uploads = self.active_uploads.write().await;
            active_uploads.insert(file_id, upload_info);
        }

        // Perform the upload
        let result = self.perform_upload(file_id, file_path, backend_client).await;

        // Remove from active uploads
        {
            let mut active_uploads = self.active_uploads.write().await;
            active_uploads.remove(&file_id);
        }

        result
    }

    /// Perform the actual file upload
    async fn perform_upload(
        &self,
        file_id: Uuid,
        file_path: &Path,
        backend_client: &Arc<BackendClient>,
    ) -> AppResult<()> {
        // Update progress to 10%
        self.update_upload_progress(file_id, 0.1).await;

        // Create a transcription request to upload the file
        let request = crate::models::api::TranscriptionRequest {
            file_path: file_path.to_string_lossy().to_string(),
            model: None, // Will use default model
            response_format: Some("json".to_string()),
            language: None,
            timestamp: Some(true),
            segments: Some(true),
        };

        // Update progress to 30%
        self.update_upload_progress(file_id, 0.3).await;

        // Send the request to the backend
        let _response = backend_client.transcribe_audio(request).await?;

        // Update progress to 100%
        self.update_upload_progress(file_id, 1.0).await;

        Ok(())
    }

    /// Update upload progress
    async fn update_upload_progress(&self, file_id: Uuid, progress: f32) {
        let mut active_uploads = self.active_uploads.write().await;
        
        if let Some(upload_info) = active_uploads.get_mut(&file_id) {
            upload_info.progress = progress;
            
            // Call the progress callback if available
            if let Some(ref callback) = upload_info.callback {
                callback(progress);
            }
        }
    }

    /// Get the current upload progress for a file
    pub async fn get_upload_progress(&self, file_id: Uuid) -> Option<f32> {
        let active_uploads = self.active_uploads.read().await;
        active_uploads.get(&file_id).map(|info| info.progress)
    }

    /// Cancel an active upload
    pub async fn cancel_upload(&self, file_id: Uuid) -> AppResult<()> {
        let mut active_uploads = self.active_uploads.write().await;
        
        if active_uploads.remove(&file_id).is_some() {
            // In a real implementation, you would also cancel the actual HTTP request
            // This would require more complex request handling
            Ok(())
        } else {
            Err(AppError::generic(format!("No active upload for file ID: {}", file_id)))
        }
    }

    /// Convert a file to a different format
    pub async fn convert_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        target_format: crate::models::AudioFileType,
    ) -> AppResult<()> {
        // In a real implementation, you would use a library like ffmpeg
        // to convert the audio file to the target format
        
        // For now, we'll just copy the file
        std::fs::copy(input_path, output_path)
            .map_err(|e| AppError::file_with_source(
                format!("Failed to convert file from {} to {}",
                    input_path.display(),
                    output_path.display()
                ),
                e
            ))?;
        
        Ok(())
    }

    /// Clean up temporary files
    pub async fn cleanup_temp_files(&self) -> AppResult<usize> {
        let config = self.config.read().await;
        let max_age = Duration::from_secs(24 * 60 * 60); // 24 hours
        
        file_utils::cleanup_temp_files(&config.temp_dir, max_age)
    }

    /// Get the total size of all files in the upload directory
    pub async fn get_upload_directory_size(&self) -> AppResult<u64> {
        let config = self.config.read().await;
        file_utils::get_directory_size(&config.upload_dir)
    }

    /// Validate a file for processing
    pub async fn validate_file(&self, file_path: &Path) -> AppResult<()> {
        let config = self.config.read().await;
        file_utils::validate_audio_file(file_path, Some(config.max_file_size))
    }

    /// Get file statistics
    pub async fn get_file_stats(&self) -> AppResult<FileStats> {
        let config = self.config.read().await;
        
        let mut stats = FileStats::default();
        
        // Count files in upload directory
        if config.upload_dir.exists() {
            let entries = std::fs::read_dir(&config.upload_dir)
                .map_err(|e| AppError::file_with_source(
                    format!("Failed to read upload directory: {}", config.upload_dir.display()),
                    e
                ))?;
            
            for entry in entries {
                let entry = entry
                    .map_err(|e| AppError::file_with_source(
                        "Failed to read directory entry",
                        e
                    ))?;
                
                let path = entry.path();
                if path.is_file() {
                    stats.total_files += 1;
                    
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        let file_size = metadata.len();
                        stats.total_size += file_size;
                        
                        // Update file type distribution
                        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
                            *stats.files_by_type.entry(extension.to_lowercase()).or_insert(0) += 1;
                        }
                    }
                }
            }
            
            // Calculate average file size
            if stats.total_files > 0 {
                stats.average_file_size = stats.total_size as f64 / stats.total_files as f64;
            }
        }
        
        Ok(stats)
    }

    /// Check if a file is supported
    pub async fn is_file_supported(&self, file_path: &Path) -> bool {
        file_utils::is_supported_audio_file(file_path)
    }

    /// Get the MIME type for a file
    pub async fn get_file_mime_type(&self, file_path: &Path) -> String {
        file_utils::get_mime_type(file_path)
    }

    /// Format a file size for display
    pub async fn format_file_size(&self, size_bytes: u64) -> String {
        file_utils::format_file_size(size_bytes)
    }

    /// Format a duration for display
    pub async fn format_duration(&self, duration: Duration) -> String {
        file_utils::format_duration(duration)
    }

    /// Estimate transcription time for a file
    pub async fn estimate_transcription_time(&self, duration: Duration) -> Duration {
        file_utils::estimate_transcription_time(duration)
    }

    /// Create a unique filename in the upload directory
    pub async fn create_unique_upload_filename(&self, filename: &str) -> PathBuf {
        let config = self.config.read().await;
        file_utils::create_unique_filename(&config.upload_dir, filename)
    }

    /// Get the audio quality category for a file
    pub async fn get_audio_quality_category(&self, bitrate: Option<u32>) -> &'static str {
        file_utils::get_audio_quality_category(bitrate)
    }
}

/// File statistics
#[derive(Debug, Clone, Default)]
pub struct FileStats {
    /// Total number of files
    pub total_files: u64,
    /// Total size of all files
    pub total_size: u64,
    /// Files by type
    pub files_by_type: std::collections::HashMap<String, u64>,
    /// Average file size
    pub average_file_size: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[tokio::test]
    async fn test_file_manager_creation() {
        let config = FileConfig::default();
        let manager = FileManager::new(config);
        
        let retrieved_config = manager.get_config().await;
        assert_eq!(retrieved_config.max_file_size, 100 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_file_validation() {
        let manager = FileManager::with_default_config();
        
        // Create a temporary file
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test.mp3");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"fake audio data").unwrap();
        file.sync_all().unwrap();
        
        // Validate the file
        let result = manager.validate_file(&file_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_upload_progress_tracking() {
        let manager = FileManager::with_default_config();
        let file_id = Uuid::new_v4();
        
        // Initially no progress
        assert_eq!(manager.get_upload_progress(file_id).await, None);
        
        // This would be updated during an actual upload
        // For testing, we can't easily simulate the upload process
    }

    #[tokio::test]
    async fn test_file_stats() {
        let temp_dir = tempdir().unwrap();
        let config = FileConfig {
            upload_dir: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        let manager = FileManager::new(config);
        
        // Create some test files
        for i in 1..=3 {
            let file_path = temp_dir.path().join(format!("test{}.mp3", i));
            let mut file = std::fs::File::create(&file_path).unwrap();
            file.write_all(b"fake audio data").unwrap();
            file.sync_all().unwrap();
        }
        
        let stats = manager.get_file_stats().await.unwrap();
        assert_eq!(stats.total_files, 3);
        assert!(stats.total_size > 0);
        assert!(stats.average_file_size > 0.0);
    }

    #[tokio::test]
    async fn test_format_helpers() {
        let manager = FileManager::with_default_config();
        
        // Test file size formatting
        let size_str = manager.format_file_size(1536).await;
        assert_eq!(size_str, "1.50 KB");
        
        // Test duration formatting
        let duration_str = manager.format_duration(Duration::from_secs(90)).await;
        assert_eq!(duration_str, "01:30");
        
        // Test transcription time estimation
        let estimated = manager.estimate_transcription_time(Duration::from_secs(60)).await;
        assert_eq!(estimated, Duration::from_secs(6));
    }
}