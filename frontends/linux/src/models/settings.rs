//! Settings models for the ASRPro application
//!
//! This module contains all the data structures for application settings,
//! organized by category for better organization and maintainability.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::utils::{AppError, AppResult};

/// Main settings container that holds all settings categories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// General application settings
    pub general: GeneralSettings,
    /// Audio-related settings
    pub audio: AudioSettings,
    /// Transcription-related settings
    pub transcription: TranscriptionSettings,
    /// Advanced settings
    pub advanced: AdvancedSettings,
    /// UI settings
    pub ui: UiSettings,
    /// File paths settings
    pub file_paths: FilePathSettings,
    /// Notification settings
    pub notifications: NotificationSettings,
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Application language
    pub language: String,
    /// Theme (light, dark, system)
    pub theme: String,
    /// Auto-save interval in minutes
    pub auto_save_interval: u32,
    /// Whether to auto-save
    pub auto_save_enabled: bool,
    /// Whether to start maximized
    pub start_maximized: bool,
    /// Whether to remember window state
    pub remember_window_state: bool,
    /// Whether to check for updates
    pub check_for_updates: bool,
    /// Update check interval in days
    pub update_check_interval: u32,
}

/// Audio-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Default input device ID
    pub default_input_device: Option<String>,
    /// Default output device ID
    pub default_output_device: Option<String>,
    /// Default sample rate
    pub default_sample_rate: u32,
    /// Default audio format
    pub default_format: String,
    /// Default input volume (0.0 to 1.0)
    pub default_input_volume: f32,
    /// Default output volume (0.0 to 1.0)
    pub default_output_volume: f32,
    /// Whether to enable echo cancellation
    pub echo_cancellation: bool,
    /// Whether to enable noise suppression
    pub noise_suppression: bool,
    /// Whether to enable automatic gain control
    pub automatic_gain_control: bool,
    /// Buffer size in milliseconds
    pub buffer_size: u32,
}

/// Transcription-related settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSettings {
    /// Default model name
    pub default_model: String,
    /// Default language code
    pub default_language: String,
    /// Whether to include timestamps
    pub include_timestamps: bool,
    /// Whether to include word timestamps
    pub include_word_timestamps: bool,
    /// Whether to include confidence scores
    pub include_confidence: bool,
    /// Whether to include segments
    pub include_segments: bool,
    /// Whether to translate to English
    pub translate_to_english: bool,
    /// Temperature for transcription (0.0 to 1.0)
    pub temperature: f32,
    /// Whether to enable automatic punctuation
    pub automatic_punctuation: bool,
    /// Whether to enable number formatting
    pub number_formatting: bool,
    /// Whether to enable profanity filter
    pub profanity_filter: bool,
}

/// Advanced settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSettings {
    /// Maximum concurrent transcription threads
    pub max_concurrent_threads: u32,
    /// Maximum file size in MB
    pub max_file_size_mb: u32,
    /// Temporary directory path
    pub temp_dir: Option<String>,
    /// Cache directory path
    pub cache_dir: Option<String>,
    /// Whether to enable debug logging
    pub debug_logging: bool,
    /// Log file path
    pub log_file_path: Option<String>,
    /// Maximum log file size in MB
    pub max_log_size_mb: u32,
    /// Number of log files to keep
    pub log_file_count: u32,
    /// Custom backend URL
    pub backend_url: Option<String>,
    /// API key for backend
    pub api_key: Option<String>,
    /// Request timeout in seconds
    pub request_timeout: u32,
    /// Whether to verify SSL certificates
    pub verify_ssl: bool,
}

/// UI settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Font family
    pub font_family: String,
    /// Font size in points
    pub font_size: f32,
    /// UI density (compact, normal, spacious)
    pub ui_density: String,
    /// Whether to show icons
    pub show_icons: bool,
    /// Whether to show tooltips
    pub show_tooltips: bool,
    /// Whether to animate transitions
    pub animate_transitions: bool,
    /// Whether to show file extensions
    pub show_file_extensions: bool,
    /// Default transcription view mode
    pub default_transcription_view: String,
    /// Whether to enable word wrap in transcription
    pub word_wrap: bool,
    /// Whether to show line numbers
    pub show_line_numbers: bool,
    /// Whether to show confidence scores in UI
    pub show_confidence_scores: bool,
    /// Window state
    pub window_state: Option<WindowState>,
}

/// Window state for remember window position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// Window width
    pub width: i32,
    /// Window height
    pub height: i32,
    /// Whether window is maximized
    pub maximized: bool,
    /// Window x position
    pub x: Option<i32>,
    /// Window y position
    pub y: Option<i32>,
}

/// File path settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePathSettings {
    /// Default audio directory
    pub audio_directory: String,
    /// Default output directory
    pub output_directory: String,
    /// Last opened directory
    pub last_opened_directory: Option<String>,
    /// Custom import directories
    pub import_directories: Vec<String>,
    /// Custom export directories
    pub export_directories: Vec<String>,
}

/// Notification settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSettings {
    /// Whether notifications are enabled
    pub enabled: bool,
    /// Show transcription completion notifications
    pub show_transcription_complete: bool,
    /// Show error notifications
    pub show_errors: bool,
    /// Show file notifications
    pub show_file_events: bool,
    /// Show system notifications
    pub show_system_events: bool,
    /// Sound notifications enabled
    pub sound_enabled: bool,
    /// Sound volume (0.0 to 1.0)
    pub sound_volume: f32,
    /// Custom notification sound path
    pub custom_sound_path: Option<String>,
    /// Notification duration in seconds
    pub notification_duration: u32,
    /// Notification position (top-left, top-right, bottom-left, bottom-right)
    pub notification_position: String,
}

/// Settings validator
pub struct SettingsValidator;

impl SettingsValidator {
    /// Validate all settings
    pub fn validate_settings(settings: &Settings) -> AppResult<()> {
        Self::validate_general(&settings.general)?;
        Self::validate_audio(&settings.audio)?;
        Self::validate_transcription(&settings.transcription)?;
        Self::validate_advanced(&settings.advanced)?;
        Self::validate_ui(&settings.ui)?;
        Self::validate_file_paths(&settings.file_paths)?;
        Self::validate_notifications(&settings.notifications)?;
        Ok(())
    }

    /// Validate general settings
    pub fn validate_general(settings: &GeneralSettings) -> AppResult<()> {
        if settings.auto_save_interval == 0 {
            return Err(AppError::config("Auto-save interval must be greater than 0"));
        }

        if settings.update_check_interval == 0 {
            return Err(AppError::config("Update check interval must be greater than 0"));
        }

        // Validate theme
        if !["light", "dark", "system"].contains(&settings.theme.as_str()) {
            return Err(AppError::config("Invalid theme value"));
        }

        Ok(())
    }

    /// Validate audio settings
    pub fn validate_audio(settings: &AudioSettings) -> AppResult<()> {
        if !(0.0..=1.0).contains(&settings.default_input_volume) {
            return Err(AppError::config("Default input volume must be between 0.0 and 1.0"));
        }

        if !(0.0..=1.0).contains(&settings.default_output_volume) {
            return Err(AppError::config("Default output volume must be between 0.0 and 1.0"));
        }

        if settings.buffer_size == 0 {
            return Err(AppError::config("Buffer size must be greater than 0"));
        }

        Ok(())
    }

    /// Validate transcription settings
    pub fn validate_transcription(settings: &TranscriptionSettings) -> AppResult<()> {
        if settings.default_model.is_empty() {
            return Err(AppError::config("Default model cannot be empty"));
        }

        if !(0.0..=1.0).contains(&settings.temperature) {
            return Err(AppError::config("Temperature must be between 0.0 and 1.0"));
        }

        Ok(())
    }

    /// Validate advanced settings
    pub fn validate_advanced(settings: &AdvancedSettings) -> AppResult<()> {
        if settings.max_concurrent_threads == 0 {
            return Err(AppError::config("Max concurrent threads must be greater than 0"));
        }

        if settings.max_file_size_mb == 0 {
            return Err(AppError::config("Max file size must be greater than 0"));
        }

        if settings.request_timeout == 0 {
            return Err(AppError::config("Request timeout must be greater than 0"));
        }

        Ok(())
    }

    /// Validate UI settings
    pub fn validate_ui(settings: &UiSettings) -> AppResult<()> {
        if settings.font_size <= 0.0 {
            return Err(AppError::config("Font size must be greater than 0"));
        }

        if !["compact", "normal", "spacious"].contains(&settings.ui_density.as_str()) {
            return Err(AppError::config("Invalid UI density value"));
        }
        
        // Validate window state if present
        if let Some(ref window_state) = settings.window_state {
            if window_state.width <= 0 || window_state.height <= 0 {
                return Err(AppError::config("Window dimensions must be positive"));
            }
        }

        Ok(())
    }

    /// Validate file path settings
    pub fn validate_file_paths(settings: &FilePathSettings) -> AppResult<()> {
        if settings.audio_directory.is_empty() {
            return Err(AppError::config("Audio directory cannot be empty"));
        }

        if settings.output_directory.is_empty() {
            return Err(AppError::config("Output directory cannot be empty"));
        }

        Ok(())
    }

    /// Validate notification settings
    pub fn validate_notifications(settings: &NotificationSettings) -> AppResult<()> {
        if !(0.0..=1.0).contains(&settings.sound_volume) {
            return Err(AppError::config("Sound volume must be between 0.0 and 1.0"));
        }

        if settings.notification_duration == 0 {
            return Err(AppError::config("Notification duration must be greater than 0"));
        }

        if !["top-left", "top-right", "bottom-left", "bottom-right"].contains(&settings.notification_position.as_str()) {
            return Err(AppError::config("Invalid notification position"));
        }

        Ok(())
    }
}

/// Settings migration utilities
pub struct SettingsMigration;

impl SettingsMigration {
    /// Migrate settings from an older version
    pub fn migrate_settings(settings: &mut Settings, from_version: u32, to_version: u32) -> AppResult<()> {
        if from_version == to_version {
            return Ok(());
        }

        // Apply migrations in order
        for version in from_version..to_version {
            match version {
                1 => Self::migrate_v1_to_v2(settings)?,
                2 => Self::migrate_v2_to_v3(settings)?,
                // Add future migrations here
                _ => {}
            }
        }

        Ok(())
    }

    /// Migrate from version 1 to 2
    fn migrate_v1_to_v2(settings: &mut Settings) -> AppResult<()> {
        // Example migration: add new field with default value
        if settings.general.check_for_updates {
            settings.general.update_check_interval = 7; // Default weekly
        }
        Ok(())
    }

    /// Migrate from version 2 to 3
    fn migrate_v2_to_v3(settings: &mut Settings) -> AppResult<()> {
        // Example migration: rename field
        let ui_density = if settings.ui.ui_density == "comfortable" {
            "normal".to_string()
        } else {
            settings.ui.ui_density.clone()
        };
        settings.ui.ui_density = ui_density;
        Ok(())
    }

    /// Get current settings version
    pub fn current_version() -> u32 {
        3 // Current version
    }
}

/// Default implementations
impl Default for Settings {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            audio: AudioSettings::default(),
            transcription: TranscriptionSettings::default(),
            advanced: AdvancedSettings::default(),
            ui: UiSettings::default(),
            file_paths: FilePathSettings::default(),
            notifications: NotificationSettings::default(),
        }
    }
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            theme: "system".to_string(),
            auto_save_interval: 5,
            auto_save_enabled: true,
            start_maximized: false,
            remember_window_state: true,
            check_for_updates: true,
            update_check_interval: 7,
        }
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            default_input_device: None,
            default_output_device: None,
            default_sample_rate: 16000,
            default_format: "wav".to_string(),
            default_input_volume: 0.8,
            default_output_volume: 0.8,
            echo_cancellation: false,
            noise_suppression: true,
            automatic_gain_control: false,
            buffer_size: 100,
        }
    }
}

impl Default for TranscriptionSettings {
    fn default() -> Self {
        Self {
            default_model: "whisper-base".to_string(),
            default_language: "auto".to_string(),
            include_timestamps: true,
            include_word_timestamps: false,
            include_confidence: true,
            include_segments: true,
            translate_to_english: false,
            temperature: 0.0,
            automatic_punctuation: true,
            number_formatting: false,
            profanity_filter: false,
        }
    }
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            max_concurrent_threads: 2,
            max_file_size_mb: 100,
            temp_dir: None,
            cache_dir: None,
            debug_logging: false,
            log_file_path: None,
            max_log_size_mb: 10,
            log_file_count: 5,
            backend_url: None,
            api_key: None,
            request_timeout: 30,
            verify_ssl: true,
        }
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            font_family: "System".to_string(),
            font_size: 12.0,
            ui_density: "normal".to_string(),
            show_icons: true,
            show_tooltips: true,
            animate_transitions: true,
            show_file_extensions: true,
            default_transcription_view: "text".to_string(),
            word_wrap: true,
            show_line_numbers: false,
            show_confidence_scores: true,
        }
    }
}

impl Default for FilePathSettings {
    fn default() -> Self {
        Self {
            audio_directory: dirs::audio_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")))
                .join("ASRPro")
                .to_string_lossy()
                .to_string(),
            output_directory: dirs::document_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/")))
                .join("ASRPro")
                .to_string_lossy()
                .to_string(),
            last_opened_directory: None,
            import_directories: Vec::new(),
            export_directories: Vec::new(),
        }
    }
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            show_transcription_complete: true,
            show_errors: true,
            show_file_events: true,
            show_system_events: false,
            sound_enabled: false,
            sound_volume: 0.5,
            custom_sound_path: None,
            notification_duration: 5,
            notification_position: "bottom-right".to_string(),
        }
    }
}

/// Settings conversion utilities
impl Settings {
    /// Convert to a hashmap for easy serialization
    pub fn to_hashmap(&self) -> HashMap<String, serde_json::Value> {
        let mut map = HashMap::new();
        
        map.insert("general".to_string(), serde_json::to_value(&self.general).unwrap());
        map.insert("audio".to_string(), serde_json::to_value(&self.audio).unwrap());
        map.insert("transcription".to_string(), serde_json::to_value(&self.transcription).unwrap());
        map.insert("advanced".to_string(), serde_json::to_value(&self.advanced).unwrap());
        map.insert("ui".to_string(), serde_json::to_value(&self.ui).unwrap());
        map.insert("file_paths".to_string(), serde_json::to_value(&self.file_paths).unwrap());
        map.insert("notifications".to_string(), serde_json::to_value(&self.notifications).unwrap());
        
        map
    }

    /// Create from a hashmap
    pub fn from_hashmap(map: HashMap<String, serde_json::Value>) -> AppResult<Self> {
        let mut settings = Settings::default();

        if let Some(general) = map.get("general") {
            settings.general = serde_json::from_value(general.clone())?;
        }

        if let Some(audio) = map.get("audio") {
            settings.audio = serde_json::from_value(audio.clone())?;
        }

        if let Some(transcription) = map.get("transcription") {
            settings.transcription = serde_json::from_value(transcription.clone())?;
        }

        if let Some(advanced) = map.get("advanced") {
            settings.advanced = serde_json::from_value(advanced.clone())?;
        }

        if let Some(ui) = map.get("ui") {
            settings.ui = serde_json::from_value(ui.clone())?;
        }

        if let Some(file_paths) = map.get("file_paths") {
            settings.file_paths = serde_json::from_value(file_paths.clone())?;
        }

        if let Some(notifications) = map.get("notifications") {
            settings.notifications = serde_json::from_value(notifications.clone())?;
        }

        Ok(settings)
    }

    /// Merge with another settings instance, preferring values from other
    pub fn merge(&self, other: &Settings) -> Settings {
        Settings {
            general: if self.general == other.general {
                self.general.clone()
            } else {
                other.general.clone()
            },
            audio: if self.audio == other.audio {
                self.audio.clone()
            } else {
                other.audio.clone()
            },
            transcription: if self.transcription == other.transcription {
                self.transcription.clone()
            } else {
                other.transcription.clone()
            },
            advanced: if self.advanced == other.advanced {
                self.advanced.clone()
            } else {
                other.advanced.clone()
            },
            ui: if self.ui == other.ui {
                self.ui.clone()
            } else {
                other.ui.clone()
            },
            file_paths: if self.file_paths == other.file_paths {
                self.file_paths.clone()
            } else {
                other.file_paths.clone()
            },
            notifications: if self.notifications == other.notifications {
                self.notifications.clone()
            } else {
                other.notifications.clone()
            },
        }
    }

    /// Get a setting value by path (e.g., "general.theme", "audio.default_sample_rate")
    pub fn get_setting(&self, path: &str) -> AppResult<serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.is_empty() {
            return Err(AppError::config("Invalid setting path"));
        }

        match parts[0] {
            "general" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "language" => Ok(serde_json::to_value(&self.general.language)?),
                        "theme" => Ok(serde_json::to_value(&self.general.theme)?),
                        "auto_save_interval" => Ok(serde_json::to_value(&self.general.auto_save_interval)?),
                        "auto_save_enabled" => Ok(serde_json::to_value(&self.general.auto_save_enabled)?),
                        "start_maximized" => Ok(serde_json::to_value(&self.general.start_maximized)?),
                        "remember_window_state" => Ok(serde_json::to_value(&self.general.remember_window_state)?),
                        "check_for_updates" => Ok(serde_json::to_value(&self.general.check_for_updates)?),
                        "update_check_interval" => Ok(serde_json::to_value(&self.general.update_check_interval)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "audio" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "default_sample_rate" => Ok(serde_json::to_value(&self.audio.default_sample_rate)?),
                        "default_format" => Ok(serde_json::to_value(&self.audio.default_format)?),
                        "default_input_volume" => Ok(serde_json::to_value(&self.audio.default_input_volume)?),
                        "default_output_volume" => Ok(serde_json::to_value(&self.audio.default_output_volume)?),
                        "echo_cancellation" => Ok(serde_json::to_value(&self.audio.echo_cancellation)?),
                        "noise_suppression" => Ok(serde_json::to_value(&self.audio.noise_suppression)?),
                        "automatic_gain_control" => Ok(serde_json::to_value(&self.audio.automatic_gain_control)?),
                        "buffer_size" => Ok(serde_json::to_value(&self.audio.buffer_size)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "transcription" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "default_model" => Ok(serde_json::to_value(&self.transcription.default_model)?),
                        "default_language" => Ok(serde_json::to_value(&self.transcription.default_language)?),
                        "include_timestamps" => Ok(serde_json::to_value(&self.transcription.include_timestamps)?),
                        "include_word_timestamps" => Ok(serde_json::to_value(&self.transcription.include_word_timestamps)?),
                        "include_confidence" => Ok(serde_json::to_value(&self.transcription.include_confidence)?),
                        "include_segments" => Ok(serde_json::to_value(&self.transcription.include_segments)?),
                        "translate_to_english" => Ok(serde_json::to_value(&self.transcription.translate_to_english)?),
                        "temperature" => Ok(serde_json::to_value(&self.transcription.temperature)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "ui" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "font_family" => Ok(serde_json::to_value(&self.ui.font_family)?),
                        "font_size" => Ok(serde_json::to_value(&self.ui.font_size)?),
                        "ui_density" => Ok(serde_json::to_value(&self.ui.ui_density)?),
                        "show_icons" => Ok(serde_json::to_value(&self.ui.show_icons)?),
                        "show_tooltips" => Ok(serde_json::to_value(&self.ui.show_tooltips)?),
                        "animate_transitions" => Ok(serde_json::to_value(&self.ui.animate_transitions)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "advanced" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "max_concurrent_threads" => Ok(serde_json::to_value(&self.advanced.max_concurrent_threads)?),
                        "max_file_size_mb" => Ok(serde_json::to_value(&self.advanced.max_file_size_mb)?),
                        "debug_logging" => Ok(serde_json::to_value(&self.advanced.debug_logging)?),
                        "request_timeout" => Ok(serde_json::to_value(&self.advanced.request_timeout)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "file_paths" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "audio_directory" => Ok(serde_json::to_value(&self.file_paths.audio_directory)?),
                        "output_directory" => Ok(serde_json::to_value(&self.file_paths.output_directory)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "notifications" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "enabled" => Ok(serde_json::to_value(&self.notifications.enabled)?),
                        "show_transcription_complete" => Ok(serde_json::to_value(&self.notifications.show_transcription_complete)?),
                        "show_errors" => Ok(serde_json::to_value(&self.notifications.show_errors)?),
                        "sound_enabled" => Ok(serde_json::to_value(&self.notifications.sound_enabled)?),
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            _ => Err(AppError::config(format!("Invalid setting category: {}", parts[0]))),
        }
    }

    /// Set a setting value by path
    pub fn set_setting(&mut self, path: &str, value: serde_json::Value) -> AppResult<()> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.is_empty() {
            return Err(AppError::config("Invalid setting path"));
        }

        match parts[0] {
            "general" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "language" => {
                            self.general.language = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "theme" => {
                            self.general.theme = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "auto_save_interval" => {
                            self.general.auto_save_interval = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "auto_save_enabled" => {
                            self.general.auto_save_enabled = serde_json::from_value(value)?;
                            Ok(())
                        },
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            "audio" => {
                if parts.len() == 2 {
                    match parts[1] {
                        "default_sample_rate" => {
                            self.audio.default_sample_rate = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "default_format" => {
                            self.audio.default_format = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "default_input_volume" => {
                            self.audio.default_input_volume = serde_json::from_value(value)?;
                            Ok(())
                        },
                        "default_output_volume" => {
                            self.audio.default_output_volume = serde_json::from_value(value)?;
                            Ok(())
                        },
                        _ => Err(AppError::config(format!("Invalid setting property: {}", path))),
                    }
                } else {
                    Err(AppError::config(format!("Invalid setting path: {}", path)))
                }
            },
            _ => Err(AppError::config(format!("Invalid setting category: {}", parts[0]))),
        }
    }
}

/// Settings backup and restore utilities
impl Settings {
    /// Export settings to a JSON string
    pub fn export_to_json(&self) -> AppResult<String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::config_with_source("Failed to serialize settings", e))?;
        Ok(json)
    }

    /// Import settings from a JSON string
    pub fn import_from_json(json: &str) -> AppResult<Self> {
        let settings: Settings = serde_json::from_str(json)
            .map_err(|e| AppError::config_with_source("Failed to deserialize settings", e))?;
        
        // Validate the imported settings
        SettingsValidator::validate_settings(&settings)?;
        
        Ok(settings)
    }

    /// Create a backup of current settings
    pub fn create_backup(&self) -> AppResult<String> {
        use chrono::Utc;
        
        let now = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_name = format!("asrpro_settings_backup_{}.json", now);
        
        let backup_data = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::config_with_source("Failed to create settings backup", e))?;
        
        // In a real implementation, you would save this to a file
        // For now, we'll just return the backup data
        
        Ok(backup_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        
        assert_eq!(settings.general.language, "en");
        assert_eq!(settings.general.theme, "system");
        assert_eq!(settings.audio.default_sample_rate, 16000);
        assert_eq!(settings.transcription.default_model, "whisper-base");
        assert!(settings.ui.font_size > 0.0);
    }

    #[test]
    fn test_settings_validation() {
        let mut settings = Settings::default();
        
        // Valid settings should pass
        assert!(SettingsValidator::validate_settings(&settings).is_ok());
        
        // Invalid settings should fail
        settings.general.auto_save_interval = 0;
        assert!(SettingsValidator::validate_settings(&settings).is_err());
        
        // Fix and try again
        settings.general.auto_save_interval = 5;
        settings.audio.default_input_volume = 1.5; // Invalid
        assert!(SettingsValidator::validate_settings(&settings).is_err());
    }

    #[test]
    fn test_settings_serialization() {
        let settings = Settings::default();
        
        // Test serialization
        let json = settings.export_to_json().unwrap();
        assert!(!json.is_empty());
        
        // Test deserialization
        let imported = Settings::import_from_json(&json).unwrap();
        assert_eq!(settings.general.language, imported.general.language);
        assert_eq!(settings.audio.default_sample_rate, imported.audio.default_sample_rate);
    }

    #[test]
    fn test_settings_get_set() {
        let mut settings = Settings::default();
        
        // Test getting
        let theme = settings.get_setting("general.theme").unwrap();
        assert_eq!(theme, "system");
        
        // Test setting
        settings.set_setting("general.theme", serde_json::Value::String("dark".to_string())).unwrap();
        let theme = settings.get_setting("general.theme").unwrap();
        assert_eq!(theme, "dark");
        
        // Test invalid path
        assert!(settings.get_setting("invalid.path").is_err());
    }

    #[test]
    fn test_settings_merge() {
        let mut settings1 = Settings::default();
        let mut settings2 = Settings::default();
        
        settings2.general.theme = "dark".to_string();
        settings2.audio.default_sample_rate = 48000;
        
        let merged = settings1.merge(&settings2);
        assert_eq!(merged.general.theme, "dark");
        assert_eq!(merged.audio.default_sample_rate, 48000);
    }
}