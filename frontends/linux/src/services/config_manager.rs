//! Configuration manager for the ASRPro application
//!
//! This module provides configuration management functionality including
//! loading, saving, and validating configuration settings.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::models::{FileConfig, ModelConfig, TranscriptionConfig, BackendConfig, Settings, SettingsValidator, SettingsMigration};
use crate::utils::{AppError, AppResult};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// File configuration
    pub file_config: FileConfig,
    /// Model configuration
    pub model_config: ModelConfig,
    /// Transcription configuration
    pub transcription_config: TranscriptionConfig,
    /// Backend configuration
    pub backend_config: BackendConfig,
    /// User preferences
    pub user_preferences: UserPreferences,
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// Default file paths
    pub default_file_paths: DefaultFilePaths,
    /// UI preferences
    pub ui_preferences: UiPreferences,
    /// Notification preferences
    pub notification_preferences: NotificationPreferences,
}

/// Default file paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultFilePaths {
    /// Default audio file directory
    pub audio_directory: PathBuf,
    /// Default output directory
    pub output_directory: PathBuf,
    /// Last opened directory
    pub last_opened_directory: Option<PathBuf>,
}

/// UI preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPreferences {
    /// Theme
    pub theme: String,
    /// Language
    pub language: String,
    /// Font size
    pub font_size: f32,
    /// Window state
    pub window_state: Option<WindowState>,
}

/// Window state for restoring window position and size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// Window width
    pub width: i32,
    /// Window height
    pub height: i32,
    /// Window x position
    pub x: i32,
    /// Window y position
    pub y: i32,
    /// Whether the window is maximized
    pub maximized: bool,
}

/// Notification preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreferences {
    /// Enable notifications
    pub enabled: bool,
    /// Show transcription completion notifications
    pub show_transcription_complete: bool,
    /// Show error notifications
    pub show_errors: bool,
    /// Sound notifications
    pub sound_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            file_config: FileConfig::default(),
            model_config: ModelConfig::default(),
            transcription_config: TranscriptionConfig::default(),
            backend_config: BackendConfig::default(),
            user_preferences: UserPreferences::default(),
        }
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            default_file_paths: DefaultFilePaths::default(),
            ui_preferences: UiPreferences::default(),
            notification_preferences: NotificationPreferences::default(),
        }
    }
}

impl Default for DefaultFilePaths {
    fn default() -> Self {
        Self {
            audio_directory: dirs::audio_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
                .join("ASRPro"),
            output_directory: dirs::document_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
                .join("ASRPro"),
            last_opened_directory: None,
        }
    }
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme: "default".to_string(),
            language: "en".to_string(),
            font_size: 12.0,
            window_state: None,
        }
    }
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            show_transcription_complete: true,
            show_errors: true,
            sound_enabled: false,
        }
    }
}

/// Configuration manager for handling application configuration
#[derive(Debug)]
pub struct ConfigManager {
    /// Configuration file path
    config_path: PathBuf,
    /// Current configuration
    config: Arc<RwLock<AppConfig>>,
    /// Settings file path
    settings_path: PathBuf,
    /// Current settings
    settings: Arc<RwLock<Settings>>,
    /// Settings version
    settings_version: u32,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(config_path: PathBuf) -> Self {
        let settings_path = config_path.with_file_name("settings.json");
        Self {
            config_path,
            config: Arc::new(RwLock::new(AppConfig::default())),
            settings_path,
            settings: Arc::new(RwLock::new(Settings::default())),
            settings_version: SettingsMigration::current_version(),
        }
    }

    /// Create a configuration manager with the default config path
    pub fn with_default_path() -> AppResult<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::config("Failed to get config directory"))?
            .join("asrpro");
        
        // Ensure config directory exists
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to create config directory: {}", config_dir.display()),
                e
            ))?;
        
        let config_path = config_dir.join("config.json");
        let settings_path = config_dir.join("settings.json");
        
        // Ensure config directory exists
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to create config directory: {}", config_dir.display()),
                e
            ))?;
        
        let mut manager = Self::new(config_path);
        manager.settings_path = settings_path;
        
        Ok(manager)
    }

    /// Load configuration from file
    pub async fn load_config(&self) -> AppResult<()> {
        if self.config_path.exists() {
            let config_content = std::fs::read_to_string(&self.config_path)
                .map_err(|e| AppError::config_with_source(
                    format!("Failed to read config file: {}", self.config_path.display()),
                    e
                ))?;
            
            let config: AppConfig = serde_json::from_str(&config_content)
                .map_err(|e| AppError::config_with_source(
                    format!("Failed to parse config file: {}", self.config_path.display()),
                    e
                ))?;
            
            let mut current_config = self.config.write().await;
            *current_config = config;
        } else {
            // Create default config file
            self.save_config().await?;
        }
        
        // Load settings
        self.load_settings().await?;
        
        Ok(())
    }

    /// Save configuration to file
    pub async fn save_config(&self) -> AppResult<()> {
        let config = self.config.read().await;
        
        let config_content = serde_json::to_string_pretty(&*config)
            .map_err(|e| AppError::config_with_source("Failed to serialize config", e))?;
        
        // Write to a temporary file first, then rename to avoid corruption
        let temp_path = self.config_path.with_extension("tmp");
        
        std::fs::write(&temp_path, config_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to write temp config file: {}", temp_path.display()),
                e
            ))?;
        
        std::fs::rename(&temp_path, &self.config_path)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to rename temp config file to: {}", self.config_path.display()),
                e
            ))?;
        
        Ok(())
    }

    /// Get the current configuration
    pub async fn get_config(&self) -> AppConfig {
        self.config.read().await.clone()
    }

    /// Update the entire configuration
    pub async fn update_config(&self, new_config: AppConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        *config = new_config;
        Ok(())
    }

    /// Get the file configuration
    pub async fn get_file_config(&self) -> FileConfig {
        let config = self.config.read().await;
        config.file_config.clone()
    }

    /// Update the file configuration
    pub async fn update_file_config(&self, file_config: FileConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.file_config = file_config;
        Ok(())
    }

    /// Get the model configuration
    pub async fn get_model_config(&self) -> ModelConfig {
        let config = self.config.read().await;
        config.model_config.clone()
    }

    /// Update the model configuration
    pub async fn update_model_config(&self, model_config: ModelConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.model_config = model_config;
        Ok(())
    }

    /// Get the transcription configuration
    pub async fn get_transcription_config(&self) -> TranscriptionConfig {
        let config = self.config.read().await;
        config.transcription_config.clone()
    }

    /// Update the transcription configuration
    pub async fn update_transcription_config(&self, transcription_config: TranscriptionConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.transcription_config = transcription_config;
        Ok(())
    }

    /// Get the backend configuration
    pub async fn get_backend_config(&self) -> BackendConfig {
        let config = self.config.read().await;
        config.backend_config.clone()
    }

    /// Update the backend configuration
    pub async fn update_backend_config(&self, backend_config: BackendConfig) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.backend_config = backend_config;
        Ok(())
    }

    /// Get user preferences
    pub async fn get_user_preferences(&self) -> UserPreferences {
        let config = self.config.read().await;
        config.user_preferences.clone()
    }

    /// Update user preferences
    pub async fn update_user_preferences(&self, user_preferences: UserPreferences) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.user_preferences = user_preferences;
        Ok(())
    }

    /// Get default file paths
    pub async fn get_default_file_paths(&self) -> DefaultFilePaths {
        let config = self.config.read().await;
        config.user_preferences.default_file_paths.clone()
    }

    /// Update default file paths
    pub async fn update_default_file_paths(&self, file_paths: DefaultFilePaths) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.user_preferences.default_file_paths = file_paths;
        Ok(())
    }

    /// Get UI preferences
    pub async fn get_ui_preferences(&self) -> UiPreferences {
        let config = self.config.read().await;
        config.user_preferences.ui_preferences.clone()
    }

    /// Update UI preferences
    pub async fn update_ui_preferences(&self, ui_preferences: UiPreferences) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.user_preferences.ui_preferences = ui_preferences;
        Ok(())
    }

    /// Get notification preferences
    pub async fn get_notification_preferences(&self) -> NotificationPreferences {
        let config = self.config.read().await;
        config.user_preferences.notification_preferences.clone()
    }

    /// Update notification preferences
    pub async fn update_notification_preferences(&self, notification_preferences: NotificationPreferences) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.user_preferences.notification_preferences = notification_preferences;
        Ok(())
    }

    /// Set the last opened directory
    pub async fn set_last_opened_directory(&self, path: PathBuf) -> AppResult<()> {
        let mut config = self.config.write().await;
        config.user_preferences.default_file_paths.last_opened_directory = Some(path);
        Ok(())
    }

    /// Get the last opened directory
    pub async fn get_last_opened_directory(&self) -> Option<PathBuf> {
        let config = self.config.read().await;
        config.user_preferences.default_file_paths.last_opened_directory.clone()
    }

    /// Reset configuration to defaults
    pub async fn reset_to_defaults(&self) -> AppResult<()> {
        let mut config = self.config.write().await;
        *config = AppConfig::default();
        Ok(())
    }

    /// Export configuration to a specific file
    pub async fn export_config(&self, export_path: &Path) -> AppResult<()> {
        let config = self.config.read().await;
        
        let config_content = serde_json::to_string_pretty(&*config)
            .map_err(|e| AppError::config_with_source("Failed to serialize config for export", e))?;
        
        std::fs::write(export_path, config_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to write exported config to: {}", export_path.display()),
                e
            ))?;
        
        Ok(())
    }

    /// Import configuration from a specific file
    pub async fn import_config(&self, import_path: &Path) -> AppResult<()> {
        let config_content = std::fs::read_to_string(import_path)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to read imported config from: {}", import_path.display()),
                e
            ))?;
        
        let config: AppConfig = serde_json::from_str(&config_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to parse imported config from: {}", import_path.display()),
                e
            ))?;
        
        let mut current_config = self.config.write().await;
        *current_config = config;
        
        Ok(())
    }

    /// Validate the current configuration
    pub async fn validate_config(&self) -> AppResult<()> {
        let config = self.config.read().await;
        
        // Validate file paths
        if !config.file_config.temp_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&config.file_config.temp_dir) {
                return Err(AppError::config_with_source(
                    format!("Failed to create temp directory: {}", config.file_config.temp_dir.display()),
                    e
                ));
            }
        }
        
        if !config.file_config.upload_dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&config.file_config.upload_dir) {
                return Err(AppError::config_with_source(
                    format!("Failed to create upload directory: {}", config.file_config.upload_dir.display()),
                    e
                ));
            }
        }
        
        // Validate backend URL
        if config.backend_config.base_url.is_empty() {
            return Err(AppError::config("Backend URL cannot be empty"));
        }
        
        // Validate model configuration
        if config.model_config.default_model.is_empty() {
            return Err(AppError::config("Default model cannot be empty"));
        }
        
        Ok(())
    }

    /// Get the configuration file path
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    /// Load settings from file
    pub async fn load_settings(&self) -> AppResult<()> {
        if self.settings_path.exists() {
            let settings_content = std::fs::read_to_string(&self.settings_path)
                .map_err(|e| AppError::config_with_source(
                    format!("Failed to read settings file: {}", self.settings_path.display()),
                    e
                ))?;
            
            // Parse the settings JSON
            let settings_json: serde_json::Value = serde_json::from_str(&settings_content)
                .map_err(|e| AppError::config_with_source(
                    format!("Failed to parse settings file: {}", self.settings_path.display()),
                    e
                ))?;
            
            // Check for version information
            let loaded_version = settings_json
                .get("version")
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as u32;
            
            // Convert to Settings struct
            let mut settings: Settings = serde_json::from_value(settings_json)
                .map_err(|e| AppError::config_with_source(
                    format!("Failed to deserialize settings: {}", self.settings_path.display()),
                    e
                ))?;
            
            // Migrate settings if needed
            if loaded_version < self.settings_version {
                SettingsMigration::migrate_settings(&mut settings, loaded_version, self.settings_version)?;
                // Save the migrated settings
                self.save_settings().await?;
            }
            
            // Validate the settings
            SettingsValidator::validate_settings(&settings)?;
            
            let mut current_settings = self.settings.write().await;
            *current_settings = settings;
        } else {
            // Create default settings file
            self.save_settings().await?;
        }
        
        Ok(())
    }

    /// Save settings to file
    pub async fn save_settings(&self) -> AppResult<()> {
        let settings = self.settings.read().await;
        
        // Validate settings before saving
        SettingsValidator::validate_settings(&*settings)?;
        
        // Create a JSON object with version information
        let mut settings_json = serde_json::to_value(&*settings)
            .map_err(|e| AppError::config_with_source("Failed to serialize settings", e))?;
        
        // Add version information
        if let Some(obj) = settings_json.as_object_mut() {
            obj.insert("version".to_string(), serde_json::Value::Number(
                serde_json::Number::from(self.settings_version)
            ));
        }
        
        let settings_content = serde_json::to_string_pretty(&settings_json)
            .map_err(|e| AppError::config_with_source("Failed to serialize settings", e))?;
        
        // Write to a temporary file first, then rename to avoid corruption
        let temp_path = self.settings_path.with_extension("tmp");
        
        std::fs::write(&temp_path, settings_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to write temp settings file: {}", temp_path.display()),
                e
            ))?;
        
        std::fs::rename(&temp_path, &self.settings_path)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to rename temp settings file to: {}", self.settings_path.display()),
                e
            ))?;
        
        Ok(())
    }

    /// Get the current settings
    pub async fn get_settings(&self) -> Settings {
        self.settings.read().await.clone()
    }

    /// Update the entire settings
    pub async fn update_settings(&self, new_settings: Settings) -> AppResult<()> {
        // Validate the new settings
        SettingsValidator::validate_settings(&new_settings)?;
        
        let mut settings = self.settings.write().await;
        *settings = new_settings;
        Ok(())
    }

    /// Get general settings
    pub async fn get_general_settings(&self) -> crate::models::GeneralSettings {
        let settings = self.settings.read().await;
        settings.general.clone()
    }

    /// Update general settings
    pub async fn update_general_settings(&self, general_settings: crate::models::GeneralSettings) -> AppResult<()> {
        // Validate the general settings
        SettingsValidator::validate_general(&general_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.general = general_settings;
        Ok(())
    }

    /// Get audio settings
    pub async fn get_audio_settings(&self) -> crate::models::AudioSettings {
        let settings = self.settings.read().await;
        settings.audio.clone()
    }

    /// Update audio settings
    pub async fn update_audio_settings(&self, audio_settings: crate::models::AudioSettings) -> AppResult<()> {
        // Validate the audio settings
        SettingsValidator::validate_audio(&audio_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.audio = audio_settings;
        Ok(())
    }

    /// Get transcription settings
    pub async fn get_transcription_settings(&self) -> crate::models::TranscriptionSettings {
        let settings = self.settings.read().await;
        settings.transcription.clone()
    }

    /// Update transcription settings
    pub async fn update_transcription_settings(&self, transcription_settings: crate::models::TranscriptionSettings) -> AppResult<()> {
        // Validate the transcription settings
        SettingsValidator::validate_transcription(&transcription_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.transcription = transcription_settings;
        Ok(())
    }

    /// Get advanced settings
    pub async fn get_advanced_settings(&self) -> crate::models::AdvancedSettings {
        let settings = self.settings.read().await;
        settings.advanced.clone()
    }

    /// Update advanced settings
    pub async fn update_advanced_settings(&self, advanced_settings: crate::models::AdvancedSettings) -> AppResult<()> {
        // Validate the advanced settings
        SettingsValidator::validate_advanced(&advanced_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.advanced = advanced_settings;
        Ok(())
    }

    /// Get UI settings
    pub async fn get_ui_settings(&self) -> crate::models::UiSettings {
        let settings = self.settings.read().await;
        settings.ui.clone()
    }

    /// Update UI settings
    pub async fn update_ui_settings(&self, ui_settings: crate::models::UiSettings) -> AppResult<()> {
        // Validate the UI settings
        SettingsValidator::validate_ui(&ui_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.ui = ui_settings;
        Ok(())
    }

    /// Get file path settings
    pub async fn get_file_path_settings(&self) -> crate::models::FilePathSettings {
        let settings = self.settings.read().await;
        settings.file_paths.clone()
    }

    /// Update file path settings
    pub async fn update_file_path_settings(&self, file_path_settings: crate::models::FilePathSettings) -> AppResult<()> {
        // Validate the file path settings
        SettingsValidator::validate_file_paths(&file_path_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.file_paths = file_path_settings;
        Ok(())
    }

    /// Get notification settings
    pub async fn get_notification_settings(&self) -> crate::models::NotificationSettings {
        let settings = self.settings.read().await;
        settings.notifications.clone()
    }

    /// Update notification settings
    pub async fn update_notification_settings(&self, notification_settings: crate::models::NotificationSettings) -> AppResult<()> {
        // Validate the notification settings
        SettingsValidator::validate_notifications(&notification_settings)?;
        
        let mut settings = self.settings.write().await;
        settings.notifications = notification_settings;
        Ok(())
    }

    /// Get a specific setting by path
    pub async fn get_setting(&self, path: &str) -> AppResult<serde_json::Value> {
        let settings = self.settings.read().await;
        settings.get_setting(path)
    }

    /// Set a specific setting by path
    pub async fn set_setting(&self, path: &str, value: serde_json::Value) -> AppResult<()> {
        let mut settings = self.settings.write().await;
        settings.set_setting(path, value)?;
        Ok(())
    }

    /// Reset settings to defaults
    pub async fn reset_settings_to_defaults(&self) -> AppResult<()> {
        let mut settings = self.settings.write().await;
        *settings = Settings::default();
        Ok(())
    }

    /// Export settings to a specific file
    pub async fn export_settings(&self, export_path: &Path) -> AppResult<()> {
        let settings = self.settings.read().await;
        
        let settings_content = serde_json::to_string_pretty(&*settings)
            .map_err(|e| AppError::config_with_source("Failed to serialize settings for export", e))?;
        
        std::fs::write(export_path, settings_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to write exported settings to: {}", export_path.display()),
                e
            ))?;
        
        Ok(())
    }

    /// Import settings from a specific file
    pub async fn import_settings(&self, import_path: &Path) -> AppResult<()> {
        let settings_content = std::fs::read_to_string(import_path)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to read imported settings from: {}", import_path.display()),
                e
            ))?;
        
        let settings: Settings = serde_json::from_str(&settings_content)
            .map_err(|e| AppError::config_with_source(
                format!("Failed to parse imported settings from: {}", import_path.display()),
                e
            ))?;
        
        // Validate the imported settings
        SettingsValidator::validate_settings(&settings)?;
        
        let mut current_settings = self.settings.write().await;
        *current_settings = settings;
        
        Ok(())
    }

    /// Create a backup of current settings
    pub async fn create_settings_backup(&self) -> AppResult<String> {
        let settings = self.settings.read().await;
        settings.create_backup()
    }

    /// Get the settings file path
    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    /// Get the current settings version
    pub fn settings_version(&self) -> u32 {
        self.settings_version
    }

    /// Auto-save settings if enabled
    pub async fn auto_save_settings_if_needed(&self) -> AppResult<()> {
        let general_settings = self.get_general_settings().await;
        if general_settings.auto_save_enabled {
            // In a real implementation, you would check if settings have changed
            // and only save if needed
            self.save_settings().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_config_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        
        assert_eq!(manager.config_path(), temp_dir.path().join("config.json"));
    }

    #[tokio::test]
    async fn test_default_config() {
        let config = AppConfig::default();
        
        assert!(!config.file_config.temp_dir.as_os_str().is_empty());
        assert!(!config.file_config.upload_dir.as_os_str().is_empty());
        assert_eq!(config.user_preferences.ui_preferences.theme, "default");
        assert_eq!(config.user_preferences.ui_preferences.language, "en");
        assert!(config.user_preferences.notification_preferences.enabled);
    }

    #[tokio::test]
    async fn test_save_and_load_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        
        // Create a custom config
        let mut config = AppConfig::default();
        config.user_preferences.ui_preferences.theme = "dark".to_string();
        config.user_preferences.ui_preferences.language = "fr".to_string();
        
        // Update and save
        manager.update_config(config).await.unwrap();
        manager.save_config().await.unwrap();
        
        // Create a new manager and load
        let manager2 = ConfigManager::new(config_path);
        manager2.load_config().await.unwrap();
        
        let loaded_config = manager2.get_config().await;
        assert_eq!(loaded_config.user_preferences.ui_preferences.theme, "dark");
        assert_eq!(loaded_config.user_preferences.ui_preferences.language, "fr");
    }

    #[tokio::test]
    async fn test_update_partial_config() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        
        // Update UI preferences
        let mut ui_prefs = UiPreferences::default();
        ui_prefs.theme = "dark".to_string();
        ui_prefs.font_size = 14.0;
        
        manager.update_ui_preferences(ui_prefs).await.unwrap();
        
        let loaded_prefs = manager.get_ui_preferences().await;
        assert_eq!(loaded_prefs.theme, "dark");
        assert_eq!(loaded_prefs.font_size, 14.0);
        assert_eq!(loaded_prefs.language, "en"); // Should remain default
    }

    #[tokio::test]
    async fn test_last_opened_directory() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        
        // Initially none
        assert!(manager.get_last_opened_directory().await.is_none());
        
        // Set one
        let test_path = PathBuf::from("/test/path");
        manager.set_last_opened_directory(test_path.clone()).await.unwrap();
        
        // Get it back
        let retrieved = manager.get_last_opened_directory().await;
        assert_eq!(retrieved, Some(test_path));
    }

    #[tokio::test]
    async fn test_reset_to_defaults() {
        let temp_dir = tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");
        let manager = ConfigManager::new(config_path);
        
        // Modify config
        let mut ui_prefs = UiPreferences::default();
        ui_prefs.theme = "dark".to_string();
        manager.update_ui_preferences(ui_prefs).await.unwrap();
        
        // Reset
        manager.reset_to_defaults().await.unwrap();
        
        // Check it's back to default
        let ui_prefs = manager.get_ui_preferences().await;
        assert_eq!(ui_prefs.theme, "default");
    }
}