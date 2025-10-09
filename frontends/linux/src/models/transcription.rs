use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Status of a transcription task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TranscriptionStatus {
    /// Task is queued but not started
    Pending,
    /// Task is currently being processed
    InProgress,
    /// Task completed successfully
    Completed,
    /// Task failed with an error
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// Transcription result containing the processed text and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Unique identifier for the transcription
    pub id: Uuid,
    /// The transcribed text
    pub text: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Language detected or used for transcription
    pub language: String,
    /// Duration of the processed audio
    pub audio_duration: Duration,
    /// Timestamps for word segments (if available)
    pub segments: Vec<TranscriptionSegment>,
    /// Timestamp when the transcription was completed
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Segment of transcribed text with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    /// The text for this segment
    pub text: String,
    /// Start time of this segment in the audio
    pub start: Duration,
    /// End time of this segment in the audio
    pub end: Duration,
    /// Confidence score for this segment (0.0 to 1.0)
    pub confidence: f32,
}

/// Represents a transcription task
#[derive(Debug, Clone)]
pub struct TranscriptionTask {
    /// Unique identifier for the task
    pub id: Uuid,
    /// Current status of the task
    pub status: TranscriptionStatus,
    /// Path to the audio file being transcribed
    pub audio_file_path: String,
    /// Model being used for transcription
    pub model_name: String,
    /// Language to use for transcription (empty for auto-detect)
    pub language: Option<String>,
    /// Progress of the transcription (0.0 to 1.0)
    pub progress: f32,
    /// Result of the transcription (available when status is Completed)
    pub result: Option<TranscriptionResult>,
    /// Error message (available when status is Failed)
    pub error_message: Option<String>,
    /// Timestamp when the task was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp when the task was started
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp when the task was completed or failed
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl TranscriptionTask {
    /// Create a new transcription task
    pub fn new(audio_file_path: String, model_name: String, language: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            status: TranscriptionStatus::Pending,
            audio_file_path,
            model_name,
            language,
            progress: 0.0,
            result: None,
            error_message: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }
    
    /// Mark the task as started
    pub fn start(&mut self) {
        self.status = TranscriptionStatus::InProgress;
        self.started_at = Some(chrono::Utc::now());
        self.progress = 0.1;
    }
    
    /// Update the progress of the task
    pub fn update_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }
    
    /// Mark the task as completed with a result
    pub fn complete(&mut self, result: TranscriptionResult) {
        self.status = TranscriptionStatus::Completed;
        self.progress = 1.0;
        self.result = Some(result);
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Mark the task as failed with an error message
    pub fn fail(&mut self, error_message: String) {
        self.status = TranscriptionStatus::Failed;
        self.error_message = Some(error_message);
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Cancel the task
    pub fn cancel(&mut self) {
        self.status = TranscriptionStatus::Cancelled;
        self.completed_at = Some(chrono::Utc::now());
    }
    
    /// Get the elapsed time since the task was created
    pub fn elapsed_time(&self) -> Duration {
        let now = chrono::Utc::now();
        let start = self.started_at.unwrap_or(self.created_at);
        let end = self.completed_at.unwrap_or(now);
        
        (end - start).to_std().unwrap_or_default()
    }
    
    /// Check if the task is active (pending or in progress)
    pub fn is_active(&self) -> bool {
        matches!(self.status, TranscriptionStatus::Pending | TranscriptionStatus::InProgress)
    }
    
    /// Check if the task is finished (completed, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(self.status, 
            TranscriptionStatus::Completed | 
            TranscriptionStatus::Failed | 
            TranscriptionStatus::Cancelled
        )
    }
}

/// Configuration for transcription requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Model to use for transcription
    pub model: String,
    /// Language code (empty for auto-detect)
    pub language: Option<String>,
    /// Whether to include timestamps
    pub include_timestamps: bool,
    /// Whether to include word segments
    pub include_segments: bool,
    /// Temperature for sampling (0.0 to 1.0)
    pub temperature: f32,
    /// Sampling strategy
    pub best_of: Option<u32>,
    /// Beam size for beam search
    pub beam_size: Option<u32>,
    /// Patience for beam search
    pub patience: Option<f32>,
    /// Length penalty
    pub length_penalty: Option<f32>,
    /// Suppression tokens
    pub suppress_tokens: Option<Vec<i32>>,
    /// Initial prompt
    pub initial_prompt: Option<String>,
    /// Condition on previous text
    pub condition_on_previous_text: bool,
    /// Temperature increment on fallback
    pub temperature_inc_on_fallback: Option<f32>,
    /// Compression ratio threshold
    pub compression_ratio_threshold: Option<f32>,
    /// Log probability threshold
    pub logprob_threshold: Option<f32>,
    /// No speech threshold
    pub no_speech_threshold: Option<f32>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            model: "whisper-1".to_string(),
            language: None,
            include_timestamps: true,
            include_segments: false,
            temperature: 0.0,
            best_of: None,
            beam_size: None,
            patience: None,
            length_penalty: None,
            suppress_tokens: None,
            initial_prompt: None,
            condition_on_previous_text: true,
            temperature_inc_on_fallback: None,
            compression_ratio_threshold: Some(2.4),
            logprob_threshold: Some(-1.0),
            no_speech_threshold: Some(0.6),
        }
    }
}

/// Statistics about transcription usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionStats {
    /// Total number of transcriptions
    pub total_transcriptions: u64,
    /// Total audio duration transcribed
    pub total_audio_duration: Duration,
    /// Average confidence score
    pub average_confidence: f32,
    /// Most frequently used languages
    pub language_distribution: std::collections::HashMap<String, u64>,
    /// Most frequently used models
    pub model_distribution: std::collections::HashMap<String, u64>,
}

impl Default for TranscriptionStats {
    fn default() -> Self {
        Self {
            total_transcriptions: 0,
            total_audio_duration: Duration::default(),
            average_confidence: 0.0,
            language_distribution: std::collections::HashMap::new(),
            model_distribution: std::collections::HashMap::new(),
        }
    }
}

impl TranscriptionStats {
    /// Update statistics with a new transcription result
    pub fn update(&mut self, result: &TranscriptionResult) {
        self.total_transcriptions += 1;
        self.total_audio_duration += result.audio_duration;
        
        // Update average confidence
        let total_confidence = self.average_confidence * (self.total_transcriptions - 1) as f32 + result.confidence;
        self.average_confidence = total_confidence / self.total_transcriptions as f32;
        
        // Update language distribution
        *self.language_distribution.entry(result.language.clone()).or_insert(0) += 1;
    }
    
    /// Update model distribution
    pub fn update_model(&mut self, model_name: &str) {
        *self.model_distribution.entry(model_name.to_string()).or_insert(0) += 1;
    }
}