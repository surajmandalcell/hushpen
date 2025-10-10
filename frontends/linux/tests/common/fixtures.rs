//! Test fixtures for providing test data
//!
//! This module contains fixtures for creating test data including
//! audio files, models, transcription tasks, and other entities.

use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

use asrpro_gtk4::models::{
    AudioFile, AudioFileType, AudioMetadata, Model, ModelType, ModelStatus,
    TranscriptionTask, TranscriptionStatus, TranscriptionConfig, TranscriptionResult,
    Settings, GeneralSettings, AudioSettings, TranscriptionSettings,
};

/// Fixture for creating test audio files
pub struct AudioFileFixture {
    temp_dir: TempDir,
}

impl AudioFileFixture {
    /// Create a new audio file fixture
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
        }
    }
    
    /// Create a test audio file with the given extension
    pub fn create_audio_file(&self, extension: &str) -> AudioFile {
        let filename = format!("test_audio.{}", extension);
        let file_path = self.temp_dir.path().join(filename);
        
        // Create a dummy audio file
        std::fs::write(&file_path, b"dummy audio data").expect("Failed to write test file");
        
        let mut audio_file = AudioFile::new(file_path.clone());
        
        // Set appropriate metadata based on file type
        let metadata = AudioMetadata {
            duration: Duration::from_secs(120),
            sample_rate: 16000,
            channels: 1,
            bit_rate: Some(128000),
            format: AudioFileType::from_extension(extension),
            file_size: 1024,
            title: Some("Test Audio".to_string()),
            artist: Some("Test Artist".to_string()),
            album: Some("Test Album".to_string()),
            date: Some("2023-01-01".to_string()),
            genre: Some("Test".to_string()),
        };
        
        audio_file.mark_ready(metadata);
        audio_file
    }
    
    /// Create an MP3 audio file
    pub fn create_mp3_file(&self) -> AudioFile {
        self.create_audio_file("mp3")
    }
    
    /// Create a WAV audio file
    pub fn create_wav_file(&self) -> AudioFile {
        self.create_audio_file("wav")
    }
    
    /// Create a FLAC audio file
    pub fn create_flac_file(&self) -> AudioFile {
        self.create_audio_file("flac")
    }
    
    /// Get the temporary directory path
    pub fn temp_dir(&self) -> &PathBuf {
        self.temp_dir.path()
    }
}

/// Fixture for creating test models
pub struct ModelFixture;

impl ModelFixture {
    /// Create a test model with the given parameters
    pub fn create_model(
        name: &str,
        display_name: &str,
        model_type: ModelType,
        size_bytes: u64,
    ) -> Model {
        Model::new(
            name.to_string(),
            display_name.to_string(),
            model_type,
            size_bytes,
            Some("Test description".to_string()),
        )
    }
    
    /// Create a Whisper tiny model
    pub fn create_whisper_tiny() -> Model {
        Self::create_model(
            "whisper-tiny",
            "Whisper Tiny",
            ModelType::Whisper,
            39 * 1024 * 1024, // 39MB
        )
    }
    
    /// Create a Whisper base model
    pub fn create_whisper_base() -> Model {
        Self::create_model(
            "whisper-base",
            "Whisper Base",
            ModelType::Whisper,
            74 * 1024 * 1024, // 74MB
        )
    }
    
    /// Create a custom model
    pub fn create_custom_model() -> Model {
        Self::create_model(
            "custom-model",
            "Custom Model",
            ModelType::Custom,
            100 * 1024 * 1024, // 100MB
        )
    }
    
    /// Create a list of available models
    pub fn create_model_list() -> Vec<Model> {
        vec![
            Self::create_whisper_tiny(),
            Self::create_whisper_base(),
            Self::create_custom_model(),
        ]
    }
}

/// Fixture for creating transcription tasks
pub struct TranscriptionFixture;

impl TranscriptionFixture {
    /// Create a test transcription task
    pub fn create_task(
        file_path: &str,
        model_name: &str,
        language: Option<String>,
    ) -> TranscriptionTask {
        TranscriptionTask::new(
            file_path.to_string(),
            model_name.to_string(),
            language,
        )
    }
    
    /// Create a completed transcription task with result
    pub fn create_completed_task(
        file_path: &str,
        model_name: &str,
        text: &str,
    ) -> TranscriptionTask {
        let mut task = Self::create_task(file_path, model_name, Some("en".to_string()));
        
        let result = TranscriptionResult {
            id: task.id,
            text: text.to_string(),
            confidence: 0.95,
            language: "en".to_string(),
            audio_duration: Duration::from_secs(60),
            segments: vec![],
            completed_at: chrono::Utc::now(),
        };
        
        task.complete(result);
        task
    }
    
    /// Create a failed transcription task
    pub fn create_failed_task(
        file_path: &str,
        model_name: &str,
        error: &str,
    ) -> TranscriptionTask {
        let mut task = Self::create_task(file_path, model_name, None);
        task.fail(error.to_string());
        task
    }
    
    /// Create an in-progress transcription task
    pub fn create_in_progress_task(
        file_path: &str,
        model_name: &str,
        progress: f32,
    ) -> TranscriptionTask {
        let mut task = Self::create_task(file_path, model_name, None);
        task.start();
        task.update_progress(progress);
        task
    }
}

/// Fixture for creating test settings
pub struct SettingsFixture;

impl SettingsFixture {
    /// Create default test settings
    pub fn create_default_settings() -> Settings {
        Settings {
            general: GeneralSettings {
                auto_save: true,
                auto_save_interval: Duration::from_secs(300),
                theme: "light".to_string(),
                language: "en".to_string(),
                check_updates: true,
                telemetry: false,
            },
            audio: AudioSettings {
                input_device: None,
                output_device: None,
                sample_rate: 16000,
                channels: 1,
                buffer_size: 1024,
                input_gain: 1.0,
                output_gain: 1.0,
                noise_suppression: true,
                echo_cancellation: false,
            },
            transcription: TranscriptionSettings {
                model: "whisper-tiny".to_string(),
                language: "auto".to_string(),
                include_timestamps: true,
                include_segments: true,
                confidence_threshold: 0.5,
                max_speakers: 1,
                speaker_diarization: false,
                profanity_filter: false,
                custom_vocabulary: vec![],
            },
            window_state: None,
        }
    }
    
    /// Create custom test settings
    pub fn create_custom_settings(
        model: &str,
        language: &str,
        theme: &str,
    ) -> Settings {
        let mut settings = Self::create_default_settings();
        settings.transcription.model = model.to_string();
        settings.transcription.language = language.to_string();
        settings.general.theme = theme.to_string();
        settings
    }
}

/// Fixture for creating transcription configs
pub struct TranscriptionConfigFixture;

impl TranscriptionConfigFixture {
    /// Create a default transcription config
    pub fn create_default_config() -> TranscriptionConfig {
        TranscriptionConfig {
            language: Some("en".to_string()),
            include_timestamps: true,
            include_segments: true,
            translate: false,
            task: "transcribe".to_string(),
            temperature: 0.0,
            best_of: 1,
            beam_size: 1,
            patience: 1.0,
            length_penalty: 1.0,
            suppress_tokens: vec![],
            initial_prompt: None,
            condition_on_previous_text: true,
            fp16: true,
            temperature_increment_on_fallback: 0.2,
            compression_ratio_threshold: 2.4,
            logprob_threshold: -1.0,
            no_speech_threshold: 0.6,
            word_timestamps: false,
            prepend_punctuations: "\"'¿([{-",
            append_punctuations: "\"'.。,，!！?？:：")]}、",
        }
    }
    
    /// Create a custom transcription config
    pub fn create_custom_config(
        language: Option<String>,
        include_timestamps: bool,
        include_segments: bool,
        translate: bool,
    ) -> TranscriptionConfig {
        let mut config = Self::create_default_config();
        config.language = language;
        config.include_timestamps = include_timestamps;
        config.include_segments = include_segments;
        config.translate = translate;
        config
    }
}

/// Fixture for creating test data files
pub struct DataFixture {
    temp_dir: TempDir,
}

impl DataFixture {
    /// Create a new data fixture
    pub fn new() -> Self {
        Self {
            temp_dir: TempDir::new().expect("Failed to create temp dir"),
        }
    }
    
    /// Create a test JSON file
    pub fn create_json_file(&self, name: &str, content: &serde_json::Value) -> PathBuf {
        let file_path = self.temp_dir.path().join(format!("{}.json", name));
        let content_str = serde_json::to_string_pretty(content).expect("Failed to serialize JSON");
        std::fs::write(&file_path, content_str).expect("Failed to write JSON file");
        file_path
    }
    
    /// Create a test text file
    pub fn create_text_file(&self, name: &str, content: &str) -> PathBuf {
        let file_path = self.temp_dir.path().join(format!("{}.txt", name));
        std::fs::write(&file_path, content).expect("Failed to write text file");
        file_path
    }
    
    /// Create a test binary file
    pub fn create_binary_file(&self, name: &str, size: usize) -> PathBuf {
        let file_path = self.temp_dir.path().join(format!("{}.bin", name));
        let data = vec![0u8; size];
        std::fs::write(&file_path, data).expect("Failed to write binary file");
        file_path
    }
    
    /// Get the temporary directory path
    pub fn temp_dir(&self) -> &PathBuf {
        self.temp_dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audio_file_fixture() {
        let fixture = AudioFileFixture::new();
        let audio_file = fixture.create_mp3_file();
        
        assert_eq!(audio_file.file_type(), AudioFileType::Mp3);
        assert!(audio_file.is_ready());
        assert_eq!(audio_file.metadata().duration, Duration::from_secs(120));
    }
    
    #[test]
    fn test_model_fixture() {
        let model = ModelFixture::create_whisper_tiny();
        
        assert_eq!(model.name, "whisper-tiny");
        assert_eq!(model.model_type, ModelType::Whisper);
        assert_eq!(model.size_bytes, 39 * 1024 * 1024);
    }
    
    #[test]
    fn test_transcription_fixture() {
        let task = TranscriptionFixture::create_completed_task(
            "/test/file.mp3",
            "whisper-tiny",
            "This is a test transcription.",
        );
        
        assert!(task.is_completed());
        assert_eq!(task.result().unwrap().text, "This is a test transcription.");
    }
    
    #[test]
    fn test_settings_fixture() {
        let settings = SettingsFixture::create_default_settings();
        
        assert_eq!(settings.transcription.model, "whisper-tiny");
        assert_eq!(settings.general.theme, "light");
        assert!(settings.general.auto_save);
    }
    
    #[test]
    fn test_data_fixture() {
        let fixture = DataFixture::new();
        let file_path = fixture.create_text_file("test", "Hello, world!");
        
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }
}