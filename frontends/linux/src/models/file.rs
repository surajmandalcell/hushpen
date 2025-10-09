use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use uuid::Uuid;

/// Status of a file operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileStatus {
    /// File is being processed
    Processing,
    /// File is ready for use
    Ready,
    /// File operation failed
    Failed,
    /// File is being uploaded
    Uploading,
    /// File is being downloaded
    Downloading,
}

/// Type of audio file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AudioFileType {
    /// MPEG Audio Layer 3
    Mp3,
    /// Waveform Audio File Format
    Wav,
    /// Free Lossless Audio Codec
    Flac,
    /// Advanced Audio Coding
    Aac,
    /// Ogg Vorbis
    Ogg,
    /// WebM Audio
    WebM,
    /// M4A Audio
    M4a,
    /// Unknown or unsupported format
    Unknown,
}

impl AudioFileType {
    /// Determine the file type from a file extension
    pub fn from_extension(extension: &str) -> Self {
        match extension.to_lowercase().as_str() {
            "mp3" => AudioFileType::Mp3,
            "wav" => AudioFileType::Wav,
            "flac" => AudioFileType::Flac,
            "aac" => AudioFileType::Aac,
            "ogg" => AudioFileType::Ogg,
            "webm" => AudioFileType::WebM,
            "m4a" => AudioFileType::M4a,
            _ => AudioFileType::Unknown,
        }
    }
    
    /// Get the MIME type for this file type
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFileType::Mp3 => "audio/mpeg",
            AudioFileType::Wav => "audio/wav",
            AudioFileType::Flac => "audio/flac",
            AudioFileType::Aac => "audio/aac",
            AudioFileType::Ogg => "audio/ogg",
            AudioFileType::WebM => "audio/webm",
            AudioFileType::M4a => "audio/mp4",
            AudioFileType::Unknown => "application/octet-stream",
        }
    }
    
    /// Check if this file type is supported
    pub fn is_supported(&self) -> bool {
        !matches!(self, AudioFileType::Unknown)
    }
}

/// Metadata about an audio file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    /// Duration of the audio
    pub duration: Duration,
    /// Sample rate in Hz
    pub sample_rate: u32,
    /// Number of channels
    pub channels: u32,
    /// Bit rate in bits per second
    pub bit_rate: Option<u32>,
    /// File format
    pub format: AudioFileType,
    /// File size in bytes
    pub file_size: u64,
    /// Title metadata
    pub title: Option<String>,
    /// Artist metadata
    pub artist: Option<String>,
    /// Album metadata
    pub album: Option<String>,
    /// Date metadata
    pub date: Option<String>,
    /// Genre metadata
    pub genre: Option<String>,
}

/// Represents an audio file in the system
#[derive(Debug, Clone)]
pub struct AudioFile {
    /// Unique identifier for the file
    pub id: Uuid,
    /// Original file name
    pub file_name: String,
    /// Path to the file
    pub file_path: PathBuf,
    /// Current status of the file
    pub status: FileStatus,
    /// Metadata about the audio file
    pub metadata: Option<AudioMetadata>,
    /// Error message if status is Failed
    pub error_message: Option<String>,
    /// Timestamp when the file was added
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp when the file was last modified
    pub modified_at: chrono::DateTime<chrono::Utc>,
    /// Whether the file has been processed
    pub processed: bool,
    /// Transcription task ID if this file is being transcribed
    pub transcription_task_id: Option<Uuid>,
}

impl AudioFile {
    /// Create a new audio file
    pub fn new(file_path: PathBuf) -> Self {
        let file_name = file_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let now = chrono::Utc::now();
        
        Self {
            id: Uuid::new_v4(),
            file_name,
            file_path,
            status: FileStatus::Processing,
            metadata: None,
            error_message: None,
            created_at: now,
            modified_at: now,
            processed: false,
            transcription_task_id: None,
        }
    }
    
    /// Get the file extension
    pub fn extension(&self) -> Option<String> {
        self.file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
    }
    
    /// Get the file type from the extension
    pub fn file_type(&self) -> AudioFileType {
        self.extension()
            .map(|ext| AudioFileType::from_extension(&ext))
            .unwrap_or(AudioFileType::Unknown)
    }
    
    /// Check if the file type is supported
    pub fn is_supported(&self) -> bool {
        self.file_type().is_supported()
    }
    
    /// Mark the file as ready
    pub fn mark_ready(&mut self, metadata: AudioMetadata) {
        self.status = FileStatus::Ready;
        self.metadata = Some(metadata);
        self.modified_at = chrono::Utc::now();
        self.processed = true;
    }
    
    /// Mark the file as failed
    pub fn mark_failed(&mut self, error_message: String) {
        self.status = FileStatus::Failed;
        self.error_message = Some(error_message);
        self.modified_at = chrono::Utc::now();
    }
    
    /// Mark the file as uploading
    pub fn mark_uploading(&mut self) {
        self.status = FileStatus::Uploading;
        self.modified_at = chrono::Utc::now();
    }
    
    /// Mark the file as downloading
    pub fn mark_downloading(&mut self) {
        self.status = FileStatus::Downloading;
        self.modified_at = chrono::Utc::now();
    }
    
    /// Associate a transcription task with this file
    pub fn set_transcription_task(&mut self, task_id: Uuid) {
        self.transcription_task_id = Some(task_id);
        self.modified_at = chrono::Utc::now();
    }
    
    /// Clear the associated transcription task
    pub fn clear_transcription_task(&mut self) {
        self.transcription_task_id = None;
        self.modified_at = chrono::Utc::now();
    }
    
    /// Check if the file is ready for transcription
    pub fn is_ready_for_transcription(&self) -> bool {
        self.status == FileStatus::Ready && self.processed && self.is_supported()
    }
    
    /// Check if the file is currently being processed
    pub fn is_processing(&self) -> bool {
        matches!(self.status, FileStatus::Processing | FileStatus::Uploading | FileStatus::Downloading)
    }
    
    /// Get a user-friendly status message
    pub fn status_message(&self) -> String {
        match &self.status {
            FileStatus::Processing => "Processing file...".to_string(),
            FileStatus::Ready => "Ready for transcription".to_string(),
            FileStatus::Failed => {
                match &self.error_message {
                    Some(msg) => format!("Failed: {}", msg),
                    None => "Failed to process file".to_string(),
                }
            },
            FileStatus::Uploading => "Uploading file...".to_string(),
            FileStatus::Downloading => "Downloading file...".to_string(),
        }
    }
    
    /// Get the file size in a human-readable format
    pub fn formatted_file_size(&self) -> String {
        let size = self.metadata
            .as_ref()
            .map(|m| m.file_size)
            .unwrap_or(0);
        
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size_f = size as f64;
        let mut unit_index = 0;
        
        while size_f >= 1024.0 && unit_index < UNITS.len() - 1 {
            size_f /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", size, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size_f, UNITS[unit_index])
        }
    }
    
    /// Get the duration in a human-readable format
    pub fn formatted_duration(&self) -> String {
        match &self.metadata {
            Some(metadata) => {
                let total_seconds = metadata.duration.as_secs();
                let hours = total_seconds / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;
                
                if hours > 0 {
                    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
                } else {
                    format!("{:02}:{:02}", minutes, seconds)
                }
            },
            None => "Unknown".to_string(),
        }
    }
}

/// Configuration for file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    /// Maximum file size in bytes
    pub max_file_size: u64,
    /// Supported file formats
    pub supported_formats: Vec<String>,
    /// Directory for temporary files
    pub temp_dir: PathBuf,
    /// Directory for uploaded files
    pub upload_dir: PathBuf,
    /// Whether to automatically process files
    pub auto_process: bool,
    /// Whether to extract metadata
    pub extract_metadata: bool,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            max_file_size: 100 * 1024 * 1024, // 100MB
            supported_formats: vec![
                "mp3".to_string(),
                "wav".to_string(),
                "flac".to_string(),
                "aac".to_string(),
                "ogg".to_string(),
                "webm".to_string(),
                "m4a".to_string(),
            ],
            temp_dir: std::env::temp_dir().join("asrpro"),
            upload_dir: dirs::cache_dir()
                .unwrap_or_else(|| std::env::temp_dir())
                .join("asrpro").join("uploads"),
            auto_process: true,
            extract_metadata: true,
        }
    }
}

/// Statistics about file operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStats {
    /// Total number of files processed
    pub total_files: u64,
    /// Total size of all files
    pub total_size: u64,
    /// Number of files by type
    pub files_by_type: std::collections::HashMap<String, u64>,
    /// Number of files by status
    pub files_by_status: std::collections::HashMap<String, u64>,
    /// Average file size
    pub average_file_size: f64,
}

impl Default for FileStats {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_size: 0,
            files_by_type: std::collections::HashMap::new(),
            files_by_status: std::collections::HashMap::new(),
            average_file_size: 0.0,
        }
    }
}

impl FileStats {
    /// Update statistics with a new file
    pub fn update(&mut self, file: &AudioFile) {
        self.total_files += 1;
        
        let file_size = file.metadata
            .as_ref()
            .map(|m| m.file_size)
            .unwrap_or(0);
        
        self.total_size += file_size;
        self.average_file_size = self.total_size as f64 / self.total_files as f64;
        
        // Update type distribution
        let type_name = format!("{:?}", file.file_type());
        *self.files_by_type.entry(type_name).or_insert(0) += 1;
        
        // Update status distribution
        let status_name = format!("{:?}", file.status);
        *self.files_by_status.entry(status_name).or_insert(0) += 1;
    }
}