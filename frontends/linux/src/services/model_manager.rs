//! Model Manager service for handling model operations
//!
//! This module provides a service for managing speech recognition models,
//! including listing, downloading, selecting, and configuring models.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::models::{Model, ModelConfig, ModelStatus, ModelType, ModelStats, presets};
use crate::utils::{AppError, AppResult};

/// Model Manager service for handling model operations
#[derive(Debug, Clone)]
pub struct ModelManager {
    /// Backend client for API communication
    backend_client: Arc<super::BackendClient>,
    /// Available models cache
    models: Arc<RwLock<HashMap<String, Model>>>,
    /// Currently selected model
    selected_model: Arc<RwLock<Option<String>>>,
    /// Model configuration
    config: Arc<RwLock<ModelConfig>>,
    /// Model statistics
    stats: Arc<RwLock<ModelStats>>,
}

impl ModelManager {
    /// Create a new ModelManager instance
    pub fn new(backend_client: Arc<super::BackendClient>) -> Self {
        Self {
            backend_client,
            models: Arc::new(RwLock::new(HashMap::new())),
            selected_model: Arc::new(RwLock::new(None)),
            config: Arc::new(RwLock::new(ModelConfig::default())),
            stats: Arc::new(RwLock::new(ModelStats::default())),
        }
    }

    /// Initialize the model manager with default models
    pub async fn initialize(&self) -> AppResult<()> {
        // Load default Whisper models
        let default_models = presets::whisper_models();
        let mut models = self.models.write().await;
        
        for model in default_models {
            models.insert(model.name.clone(), model);
        }
        
        // Try to fetch available models from the backend
        if let Err(e) = self.refresh_models().await {
            eprintln!("Warning: Failed to fetch models from backend: {}", e);
        }
        
        // Set the default model if none is selected
        let mut selected_model = self.selected_model.write().await;
        if selected_model.is_none() {
            let config = self.config.read().await;
            *selected_model = Some(config.default_model.clone());
        }
        
        Ok(())
    }

    /// Refresh the list of available models from the backend
    pub async fn refresh_models(&self) -> AppResult<()> {
        match self.backend_client.list_models().await {
            Ok(model_list) => {
                let mut models = self.models.write().await;
                
                // Update existing models and add new ones
                for api_model in model_list.data {
                    let model_name = api_model.id.clone();
                    
                    // Check if we already have this model
                    if let Some(existing_model) = models.get_mut(&model_name) {
                        // Update the status
                        if api_model.ready {
                            existing_model.mark_available();
                        } else {
                            existing_model.status = ModelStatus::Unavailable;
                        }
                    } else {
                        // Create a new model from the API response
                        let mut new_model = Model::new(
                            api_model.id.clone(),
                            format!("Model {}", api_model.id),
                            ModelType::Whisper, // Default to Whisper for API models
                            0, // Size unknown from API
                            None, // Language unknown
                        );
                        
                        if api_model.ready {
                            new_model.mark_available();
                        }
                        
                        models.insert(model_name, new_model);
                    }
                }
                
                // Update statistics
                self.update_stats().await;
                
                Ok(())
            },
            Err(e) => {
                eprintln!("Failed to refresh models: {}", e);
                Err(e)
            }
        }
    }

    /// Get all available models
    pub async fn get_models(&self) -> Vec<Model> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// Get a model by name
    pub async fn get_model(&self, name: &str) -> Option<Model> {
        let models = self.models.read().await;
        models.get(name).cloned()
    }

    /// Get models filtered by type
    pub async fn get_models_by_type(&self, model_type: ModelType) -> Vec<Model> {
        let models = self.models.read().await;
        models
            .values()
            .filter(|m| m.model_type == model_type)
            .cloned()
            .collect()
    }

    /// Get models filtered by status
    pub async fn get_models_by_status(&self, status: ModelStatus) -> Vec<Model> {
        let models = self.models.read().await;
        models
            .values()
            .filter(|m| m.status == status)
            .cloned()
            .collect()
    }

    /// Get available models (ready for use)
    pub async fn get_available_models(&self) -> Vec<Model> {
        self.get_models_by_status(ModelStatus::Available).await
    }

    /// Get the currently selected model
    pub async fn get_selected_model(&self) -> Option<Model> {
        let selected_model = self.selected_model.read().await;
        let models = self.models.read().await;
        
        if let Some(model_name) = selected_model.as_ref() {
            models.get(model_name).cloned()
        } else {
            None
        }
    }

    /// Set the selected model
    pub async fn set_selected_model(&self, model_name: &str) -> AppResult<()> {
        let models = self.models.read().await;
        
        // Check if the model exists
        if !models.contains_key(model_name) {
            return Err(AppError::generic(format!("Model '{}' not found", model_name)));
        }
        
        // Set the selected model
        let mut selected_model = self.selected_model.write().await;
        *selected_model = Some(model_name.to_string());
        
        // Try to set the model in the backend
        if let Err(e) = self.backend_client.set_model(model_name).await {
            eprintln!("Warning: Failed to set model in backend: {}", e);
        }
        
        Ok(())
    }

    /// Download a model
    pub async fn download_model(&self, model_name: &str, progress_callback: Option<Box<dyn Fn(f32) + Send + Sync>>) -> AppResult<()> {
        let mut models = self.models.write().await;
        
        // Check if the model exists
        let model = models.get_mut(model_name)
            .ok_or_else(|| AppError::generic(format!("Model '{}' not found", model_name)))?;
        
        // Check if the model is already downloaded
        if model.is_ready() {
            return Ok(());
        }
        
        // Mark the model as downloading
        model.mark_downloading();
        
        // In a real implementation, you would download the model here
        // For now, we'll simulate the download process
        let model_name_clone = model_name.to_string();
        let models_clone = Arc::clone(&self.models);
        
        tokio::spawn(async move {
            // Simulate download progress
            for i in 0..=100 {
                if let Some(ref callback) = progress_callback {
                    callback(i as f32 / 100.0);
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            
            // Mark the model as available
            let mut models = models_clone.write().await;
            if let Some(model) = models.get_mut(&model_name_clone) {
                model.mark_available();
            }
        });
        
        Ok(())
    }

    /// Delete a model
    pub async fn delete_model(&self, model_name: &str) -> AppResult<()> {
        let mut models = self.models.write().await;
        
        // Check if the model exists
        if !models.contains_key(model_name) {
            return Err(AppError::generic(format!("Model '{}' not found", model_name)));
        }
        
        // Check if the model is currently selected
        let selected_model = self.selected_model.read().await;
        if let Some(selected) = selected_model.as_ref() {
            if selected == model_name {
                return Err(AppError::generic("Cannot delete the currently selected model"));
            }
        }
        
        // Remove the model
        models.remove(model_name);
        
        // Update statistics
        self.update_stats().await;
        
        Ok(())
    }

    /// Get model configuration
    pub async fn get_config(&self) -> ModelConfig {
        self.config.read().await.clone()
    }

    /// Update model configuration
    pub async fn update_config(&self, config: ModelConfig) -> AppResult<()> {
        let mut current_config = self.config.write().await;
        *current_config = config;
        Ok(())
    }

    /// Get model statistics
    pub async fn get_stats(&self) -> ModelStats {
        self.stats.read().await.clone()
    }

    /// Update model statistics
    async fn update_stats(&self) {
        let models = self.models.read().await;
        let mut stats = self.stats.write().await;
        
        // Reset stats
        *stats = ModelStats::default();
        
        // Update with current models
        for model in models.values() {
            stats.update(model);
        }
    }

    /// Search models by name or description
    pub async fn search_models(&self, query: &str) -> Vec<Model> {
        let models = self.models.read().await;
        let query_lower = query.to_lowercase();
        
        models
            .values()
            .filter(|m| {
                m.name.to_lowercase().contains(&query_lower) ||
                m.display_name.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Get recommended models for a given audio file
    pub async fn get_recommended_models(&self, file_path: &str) -> Vec<Model> {
        let models = self.models.read().await;
        
        // In a real implementation, you would analyze the audio file
        // and recommend models based on its characteristics
        // For now, we'll return available models sorted by size (small to large)
        let mut available_models: Vec<_> = models
            .values()
            .filter(|m| m.is_ready())
            .cloned()
            .collect();
        
        available_models.sort_by_key(|m| m.size_bytes);
        available_models
    }

    /// Check for model updates
    pub async fn check_for_updates(&self) -> AppResult<Vec<String>> {
        // In a real implementation, you would check for model updates
        // For now, we'll return an empty list
        Ok(Vec::new())
    }

    /// Update a model
    pub async fn update_model(&self, model_name: &str) -> AppResult<()> {
        // In a real implementation, you would update the model
        // For now, we'll just return an error
        Err(AppError::generic("Model updates not implemented yet"))
    }
}

/// Model download progress callback type
pub type ModelDownloadProgressCallback = Box<dyn Fn(f32) + Send + Sync>;

/// Model manager builder
pub struct ModelManagerBuilder {
    backend_client: Option<Arc<super::BackendClient>>,
    config: Option<ModelConfig>,
}

impl ModelManagerBuilder {
    /// Create a new ModelManager builder
    pub fn new() -> Self {
        Self {
            backend_client: None,
            config: None,
        }
    }

    /// Set the backend client
    pub fn with_backend_client(mut self, backend_client: Arc<super::BackendClient>) -> Self {
        self.backend_client = Some(backend_client);
        self
    }

    /// Set the model configuration
    pub fn with_config(mut self, config: ModelConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Build the ModelManager
    pub fn build(self) -> AppResult<ModelManager> {
        let backend_client = self.backend_client
            .ok_or_else(|| AppError::generic("Backend client is required"))?;
        
        let model_manager = ModelManager::new(backend_client);
        
        // Set the initial config if provided
        if let Some(config) = self.config {
            let config_arc = Arc::new(RwLock::new(config));
            // This would require modifying ModelManager to accept the config in the constructor
            // For now, we'll just return the model manager
        }
        
        Ok(model_manager)
    }
}

impl Default for ModelManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::api::BackendConfig;
    
    #[tokio::test]
    async fn test_model_manager_creation() {
        let config = BackendConfig::default();
        let backend_client = super::BackendClient::new(config).unwrap();
        let backend_client = Arc::new(backend_client);
        
        let model_manager = ModelManager::new(backend_client);
        
        // Test that the model manager was created successfully
        let models = model_manager.get_models().await;
        assert!(!models.is_empty()); // Should have default models
    }
    
    #[tokio::test]
    async fn test_model_selection() {
        let config = BackendConfig::default();
        let backend_client = BackendClient::new(config).unwrap();
        let backend_client = Arc::new(backend_client);
        
        let model_manager = ModelManager::new(backend_client);
        model_manager.initialize().await.unwrap();
        
        // Get available models
        let available_models = model_manager.get_available_models().await;
        
        if !available_models.is_empty() {
            let model_name = &available_models[0].name;
            
            // Set the selected model
            model_manager.set_selected_model(model_name).await.unwrap();
            
            // Check that the model was selected
            let selected_model = model_manager.get_selected_model().await;
            assert!(selected_model.is_some());
            assert_eq!(selected_model.unwrap().name, *model_name);
        }
    }
    
    #[tokio::test]
    async fn test_model_search() {
        let config = BackendConfig::default();
        let backend_client = BackendClient::new(config).unwrap();
        let backend_client = Arc::new(backend_client);
        
        let model_manager = ModelManager::new(backend_client);
        model_manager.initialize().await.unwrap();
        
        // Search for models
        let results = model_manager.search_models("whisper").await;
        assert!(!results.is_empty()); // Should find Whisper models
        
        // Search for non-existent models
        let results = model_manager.search_models("nonexistent").await;
        assert!(results.is_empty()); // Should not find any models
    }
}