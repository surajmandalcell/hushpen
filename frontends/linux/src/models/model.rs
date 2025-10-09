use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Status of a model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelStatus {
    /// Model is available and ready to use
    Available,
    /// Model is currently being downloaded
    Downloading,
    /// Model is being loaded into memory
    Loading,
    /// Model is loaded and ready for inference
    Loaded,
    /// Model is currently being used for inference
    InUse,
    /// Model failed to download or load
    Failed,
    /// Model is not available (e.g., not downloaded)
    Unavailable,
}

/// Type of model
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelType {
    /// Whisper-based model
    Whisper,
    /// Custom model
    Custom,
    /// Fine-tuned model
    FineTuned,
    /// OpenAI API model
    OpenAI,
    /// Local model
    Local,
}

impl ModelType {
    /// Get the display name for the model type
    pub fn display_name(&self) -> &'static str {
        match self {
            ModelType::Whisper => "Whisper",
            ModelType::Custom => "Custom",
            ModelType::FineTuned => "Fine-tuned",
            ModelType::OpenAI => "OpenAI",
            ModelType::Local => "Local",
        }
    }
}

/// Represents a speech recognition model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Unique identifier for the model
    pub id: Uuid,
    /// Name of the model
    pub name: String,
    /// Display name for the model
    pub display_name: String,
    /// Type of the model
    pub model_type: ModelType,
    /// Current status of the model
    pub status: ModelStatus,
    /// Size of the model in bytes
    pub size_bytes: u64,
    /// Language supported by the model (empty for multi-language)
    pub language: Option<String>,
    /// Whether the model supports transcription
    pub supports_transcription: bool,
    /// Whether the model supports translation
    pub supports_translation: bool,
    /// Whether the model supports speaker diarization
    pub supports_diarization: bool,
    /// Supported audio formats
    pub supported_formats: Vec<String>,
    /// Model parameters
    pub parameters: ModelParameters,
    /// Performance metrics
    pub performance: Option<ModelPerformance>,
    /// Error message if status is Failed
    pub error_message: Option<String>,
    /// Timestamp when the model was added
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Timestamp when the model was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Parameters for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelParameters {
    /// Sample rate required by the model
    pub sample_rate: u32,
    /// Maximum audio length in seconds
    pub max_audio_length: u32,
    /// Chunk size for processing
    pub chunk_size: u32,
    /// Whether the model supports streaming
    pub supports_streaming: bool,
    /// Whether the model supports batching
    pub supports_batching: bool,
    /// Maximum batch size
    pub max_batch_size: Option<u32>,
    /// GPU memory requirement in MB
    pub gpu_memory_mb: Option<u32>,
    /// CPU memory requirement in MB
    pub cpu_memory_mb: u32,
    /// Additional model-specific parameters
    pub additional_params: HashMap<String, serde_json::Value>,
}

/// Performance metrics for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformance {
    /// Real-time factor (RTF) - lower is better
    pub real_time_factor: f32,
    /// Word error rate (WER) - lower is better
    pub word_error_rate: Option<f32>,
    /// Average inference time in seconds
    pub avg_inference_time: f32,
    /// Throughput in characters per second
    pub throughput_chars_per_sec: f32,
    /// GPU utilization percentage
    pub gpu_utilization: Option<f32>,
    /// CPU utilization percentage
    pub cpu_utilization: f32,
    /// Memory usage in MB
    pub memory_usage_mb: f32,
    /// Number of inferences performed
    pub total_inferences: u64,
    /// Timestamp when metrics were last updated
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl Model {
    /// Create a new model
    pub fn new(
        name: String,
        display_name: String,
        model_type: ModelType,
        size_bytes: u64,
        language: Option<String>,
    ) -> Self {
        let now = chrono::Utc::now();
        
        Self {
            id: Uuid::new_v4(),
            name,
            display_name,
            model_type,
            status: ModelStatus::Unavailable,
            size_bytes,
            language,
            supports_transcription: true,
            supports_translation: false,
            supports_diarization: false,
            supported_formats: vec![
                "wav".to_string(),
                "mp3".to_string(),
                "flac".to_string(),
                "aac".to_string(),
                "ogg".to_string(),
                "webm".to_string(),
                "m4a".to_string(),
            ],
            parameters: ModelParameters::default(),
            performance: None,
            error_message: None,
            created_at: now,
            updated_at: now,
        }
    }
    
    /// Mark the model as available
    pub fn mark_available(&mut self) {
        self.status = ModelStatus::Available;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as downloading
    pub fn mark_downloading(&mut self) {
        self.status = ModelStatus::Downloading;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as loading
    pub fn mark_loading(&mut self) {
        self.status = ModelStatus::Loading;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as loaded
    pub fn mark_loaded(&mut self) {
        self.status = ModelStatus::Loaded;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as in use
    pub fn mark_in_use(&mut self) {
        self.status = ModelStatus::InUse;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as available after use
    pub fn mark_available_after_use(&mut self) {
        self.status = ModelStatus::Loaded;
        self.updated_at = chrono::Utc::now();
    }
    
    /// Mark the model as failed
    pub fn mark_failed(&mut self, error_message: String) {
        self.status = ModelStatus::Failed;
        self.error_message = Some(error_message);
        self.updated_at = chrono::Utc::now();
    }
    
    /// Check if the model is ready for use
    pub fn is_ready(&self) -> bool {
        matches!(self.status, ModelStatus::Available | ModelStatus::Loaded)
    }
    
    /// Check if the model is currently being used
    pub fn is_in_use(&self) -> bool {
        self.status == ModelStatus::InUse
    }
    
    /// Check if the model is being processed (downloading, loading, etc.)
    pub fn is_processing(&self) -> bool {
        matches!(self.status, 
            ModelStatus::Downloading | 
            ModelStatus::Loading | 
            ModelStatus::InUse
        )
    }
    
    /// Check if the model has failed
    pub fn is_failed(&self) -> bool {
        self.status == ModelStatus::Failed
    }
    
    /// Get a user-friendly status message
    pub fn status_message(&self) -> String {
        match &self.status {
            ModelStatus::Available => "Available".to_string(),
            ModelStatus::Downloading => "Downloading...".to_string(),
            ModelStatus::Loading => "Loading...".to_string(),
            ModelStatus::Loaded => "Loaded".to_string(),
            ModelStatus::InUse => "In use".to_string(),
            ModelStatus::Failed => {
                match &self.error_message {
                    Some(msg) => format!("Failed: {}", msg),
                    None => "Failed".to_string(),
                }
            },
            ModelStatus::Unavailable => "Not available".to_string(),
        }
    }
    
    /// Get the size in a human-readable format
    pub fn formatted_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size_f = self.size_bytes as f64;
        let mut unit_index = 0;
        
        while size_f >= 1024.0 && unit_index < UNITS.len() - 1 {
            size_f /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", self.size_bytes, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size_f, UNITS[unit_index])
        }
    }
    
    /// Update performance metrics
    pub fn update_performance(&mut self, performance: ModelPerformance) {
        self.performance = Some(performance);
        self.updated_at = chrono::Utc::now();
    }
}

impl Default for ModelParameters {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            max_audio_length: 1800, // 30 minutes
            chunk_size: 30, // 30 seconds
            supports_streaming: false,
            supports_batching: false,
            max_batch_size: None,
            gpu_memory_mb: None,
            cpu_memory_mb: 512,
            additional_params: HashMap::new(),
        }
    }
}

/// Configuration for model management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Default model to use
    pub default_model: String,
    /// Whether to automatically download models
    pub auto_download: bool,
    /// Directory for model storage
    pub model_dir: std::path::PathBuf,
    /// Maximum number of models to keep loaded
    pub max_loaded_models: u32,
    /// Whether to use GPU if available
    pub use_gpu: bool,
    /// GPU device to use
    pub gpu_device: Option<u32>,
    /// Timeout for model operations in seconds
    pub operation_timeout: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            default_model: "whisper-1".to_string(),
            auto_download: true,
            model_dir: dirs::cache_dir()
                .unwrap_or_else(|| std::env::temp_dir())
                .join("asrpro").join("models"),
            max_loaded_models: 2,
            use_gpu: true,
            gpu_device: None,
            operation_timeout: 300, // 5 minutes
        }
    }
}

/// Statistics about model usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    /// Total number of models
    pub total_models: u64,
    /// Number of models by type
    pub models_by_type: HashMap<String, u64>,
    /// Number of models by status
    pub models_by_status: HashMap<String, u64>,
    /// Total size of all models
    pub total_size: u64,
    /// Most used model
    pub most_used_model: Option<String>,
    /// Total number of inferences
    pub total_inferences: u64,
    /// Average inference time
    pub avg_inference_time: f32,
}

impl Default for ModelStats {
    fn default() -> Self {
        Self {
            total_models: 0,
            models_by_type: HashMap::new(),
            models_by_status: HashMap::new(),
            total_size: 0,
            most_used_model: None,
            total_inferences: 0,
            avg_inference_time: 0.0,
        }
    }
}

impl ModelStats {
    /// Update statistics with a new model
    pub fn update(&mut self, model: &Model) {
        self.total_models += 1;
        self.total_size += model.size_bytes;
        
        // Update type distribution
        let type_name = format!("{:?}", model.model_type);
        *self.models_by_type.entry(type_name).or_insert(0) += 1;
        
        // Update status distribution
        let status_name = format!("{:?}", model.status);
        *self.models_by_status.entry(status_name).or_insert(0) += 1;
    }
    
    /// Update statistics with an inference
    pub fn update_inference(&mut self, model_name: &str, inference_time: f32) {
        self.total_inferences += 1;
        
        // Update average inference time
        let total_time = self.avg_inference_time * (self.total_inferences - 1) as f32 + inference_time;
        self.avg_inference_time = total_time / self.total_inferences as f32;
        
        // Update most used model (simplified - just tracks the last used)
        self.most_used_model = Some(model_name.to_string());
    }
}

/// Available model presets
pub mod presets {
    use super::*;
    
    /// Get the default Whisper models
    pub fn whisper_models() -> Vec<Model> {
        vec![
            Model::new(
                "whisper-tiny".to_string(),
                "Whisper Tiny (74M)".to_string(),
                ModelType::Whisper,
                74 * 1024 * 1024, // 74MB
                None,
            ),
            Model::new(
                "whisper-base".to_string(),
                "Whisper Base (244M)".to_string(),
                ModelType::Whisper,
                244 * 1024 * 1024, // 244MB
                None,
            ),
            Model::new(
                "whisper-small".to_string(),
                "Whisper Small (769M)".to_string(),
                ModelType::Whisper,
                769 * 1024 * 1024, // 769MB
                None,
            ),
            Model::new(
                "whisper-medium".to_string(),
                "Whisper Medium (1550M)".to_string(),
                ModelType::Whisper,
                1550 * 1024 * 1024, // 1550MB
                None,
            ),
            Model::new(
                "whisper-large".to_string(),
                "Whisper Large (3100M)".to_string(),
                ModelType::Whisper,
                3100 * 1024 * 1024, // 3100MB
                None,
            ),
        ]
    }
}