//! Test application setup and utilities
//!
//! This module provides utilities for setting up test applications,
//! including mock backends, test state, and application instances.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use asrpro_gtk4::{
    models::{AppState, BackendConfig},
    services::{BackendClient, ModelManager},
};

/// Test application wrapper
pub struct TestApp {
    pub app_state: Arc<AppState>,
    pub runtime: tokio::runtime::Runtime,
    pub backend_client: Option<Arc<BackendClient>>,
    pub model_manager: Option<Arc<ModelManager>>,
    pub mock_server: Option<httpmock::MockServer>,
}

impl TestApp {
    /// Create a new test application with default configuration
    pub async fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create test runtime");
        
        // Create application state
        let app_state = Arc::new(
            AppState::new()
                .expect("Failed to create app state")
        );
        
        // Initialize app state
        app_state.initialize().await
            .expect("Failed to initialize app state");
        
        Self {
            app_state,
            runtime,
            backend_client: None,
            model_manager: None,
            mock_server: None,
        }
    }
    
    /// Create a test application with a mock backend
    pub async fn with_mock_backend() -> Self {
        let mut app = Self::new().await;
        
        // Set up mock backend
        let mock_server = setup_mock_backend();
        let backend_config = BackendConfig {
            base_url: mock_server.url("/"),
            api_key: None,
            timeout: Duration::from_secs(30),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(500),
        };
        
        let backend_client = Arc::new(
            BackendClient::new(backend_config)
                .expect("Failed to create backend client")
        );
        
        let model_manager = Arc::new(
            ModelManager::new(Arc::clone(&backend_client))
        );
        
        app.backend_client = Some(Arc::clone(&backend_client));
        app.model_manager = Some(Arc::clone(&model_manager));
        app.mock_server = Some(mock_server);
        
        app
    }
    
    /// Get the app state
    pub fn app_state(&self) -> &Arc<AppState> {
        &self.app_state
    }
    
    /// Get the backend client
    pub fn backend_client(&self) -> Option<&Arc<BackendClient>> {
        self.backend_client.as_ref()
    }
    
    /// Get the model manager
    pub fn model_manager(&self) -> Option<&Arc<ModelManager>> {
        self.model_manager.as_ref()
    }
    
    /// Get the mock server URL
    pub fn mock_server_url(&self) -> Option<String> {
        self.mock_server.as_ref().map(|s| s.url("/"))
    }
    
    /// Run a function in the application's runtime
    pub fn run_in_runtime<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        self.runtime.block_on(async {
            // In a real implementation, you might need to handle this differently
            // For now, we'll just run the function directly
            tokio::task::spawn_blocking(f).await.unwrap()
        })
    }
    
    /// Run an async function in the application's runtime
    pub async fn run_async<F, R>(&self, f: F) -> R
    where
        F: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        f.await
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        // Clean up resources
        if let Some(mock_server) = &self.mock_server {
            mock_server.shutdown();
        }
    }
}

/// Set up a mock backend server for testing
pub fn setup_mock_backend() -> httpmock::MockServer {
    let server = httpmock::MockServer::start();
    
    // Health check endpoint
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/health");
        then.status(200)
            .json_body(&serde_json::json!({
                "status": "healthy",
                "version": "1.0.0",
                "timestamp": chrono::Utc::now().to_rfc3339()
            }));
    });
    
    // Models endpoint
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/models");
        then.status(200)
            .json_body(&serde_json::json!({
                "models": [
                    {
                        "id": "whisper-tiny",
                        "name": "Whisper Tiny",
                        "display_name": "Whisper Tiny",
                        "type": "whisper",
                        "size_bytes": 39000000,
                        "description": "Tiny Whisper model",
                        "languages": ["en", "es", "fr"],
                        "status": "available"
                    },
                    {
                        "id": "whisper-base",
                        "name": "Whisper Base",
                        "display_name": "Whisper Base",
                        "type": "whisper",
                        "size_bytes": 74000000,
                        "description": "Base Whisper model",
                        "languages": ["en", "es", "fr", "de"],
                        "status": "available"
                    }
                ]
            }));
    });
    
    // Transcription endpoint
    server.mock(|when, then| {
        when.method(httpmock::Method::POST)
            .path("/transcribe");
        then.status(200)
            .json_body(&serde_json::json!({
                "task_id": Uuid::new_v4(),
                "status": "queued",
                "message": "Transcription task created"
            }));
    });
    
    // Transcription status endpoint
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path_regex(r"^/transcribe/[^/]+$");
        then.status(200)
            .json_body(&serde_json::json!({
                "task_id": Uuid::new_v4(),
                "status": "completed",
                "progress": 1.0,
                "result": {
                    "text": "This is a test transcription result.",
                    "confidence": 0.95,
                    "language": "en",
                    "segments": [
                        {
                            "start": 0.0,
                            "end": 2.5,
                            "text": "This is a test",
                            "confidence": 0.95
                        },
                        {
                            "start": 2.5,
                            "end": 5.0,
                            "text": "transcription result.",
                            "confidence": 0.95
                        }
                    ]
                }
            }));
    });
    
    server
}

/// Set up a mock WebSocket server for testing
pub fn setup_mock_websocket() -> tokio_tungstenite::tungstenite::server::WebSocketServer {
    // This is a placeholder for WebSocket mock setup
    // In a real implementation, you would set up a mock WebSocket server
    // that can send and receive test messages
    
    // For now, we'll return a dummy implementation
    panic!("Mock WebSocket server not implemented yet");
}

/// Test context for integration tests
pub struct TestContext {
    pub test_app: TestApp,
    pub temp_dir: tempfile::TempDir,
}

impl TestContext {
    /// Create a new test context
    pub async fn new() -> Self {
        let test_app = TestApp::with_mock_backend().await;
        let temp_dir = tempfile::TempDir::new()
            .expect("Failed to create temp dir");
        
        Self {
            test_app,
            temp_dir,
        }
    }
    
    /// Create a new test context with a specific backend configuration
    pub async fn with_backend_config(config: BackendConfig) -> Self {
        let runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create test runtime");
        
        let app_state = Arc::new(
            AppState::new()
                .expect("Failed to create app state")
        );
        
        app_state.initialize().await
            .expect("Failed to initialize app state");
        
        let backend_client = Arc::new(
            BackendClient::new(config)
                .expect("Failed to create backend client")
        );
        
        let model_manager = Arc::new(
            ModelManager::new(Arc::clone(&backend_client))
        );
        
        let test_app = TestApp {
            app_state,
            runtime,
            backend_client: Some(Arc::clone(&backend_client)),
            model_manager: Some(Arc::clone(&model_manager)),
            mock_server: None,
        };
        
        let temp_dir = tempfile::TempDir::new()
            .expect("Failed to create temp dir");
        
        Self {
            test_app,
            temp_dir,
        }
    }
    
    /// Get the app state
    pub fn app_state(&self) -> &Arc<AppState> {
        self.test_app.app_state()
    }
    
    /// Get the backend client
    pub fn backend_client(&self) -> Option<&Arc<BackendClient>> {
        self.test_app.backend_client()
    }
    
    /// Get the model manager
    pub fn model_manager(&self) -> Option<&Arc<ModelManager>> {
        self.test_app.model_manager()
    }
    
    /// Get the temporary directory
    pub fn temp_dir(&self) -> &tempfile::TempDir {
        &self.temp_dir
    }
    
    /// Create a test file in the temporary directory
    pub fn create_test_file(&self, name: &str, content: &[u8]) -> std::path::PathBuf {
        let file_path = self.temp_dir.path().join(name);
        std::fs::write(&file_path, content)
            .expect("Failed to write test file");
        file_path
    }
    
    /// Create a test audio file in the temporary directory
    pub fn create_test_audio_file(&self, name: &str, extension: &str) -> std::path::PathBuf {
        let filename = format!("{}.{}", name, extension);
        let file_path = self.temp_dir.path().join(filename);
        
        // Create some dummy audio data
        let dummy_data = vec![0u8; 1024];
        std::fs::write(&file_path, dummy_data)
            .expect("Failed to write test audio file");
        
        file_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_test_app_creation() {
        let app = TestApp::new().await;
        assert!(app.app_state().is_initialized().await);
    }
    
    #[tokio::test]
    async fn test_test_app_with_mock_backend() {
        let app = TestApp::with_mock_backend().await;
        assert!(app.backend_client().is_some());
        assert!(app.model_manager().is_some());
        assert!(app.mock_server_url().is_some());
    }
    
    #[tokio::test]
    async fn test_test_context_creation() {
        let context = TestContext::new().await;
        assert!(context.app_state().is_initialized().await);
        assert!(context.backend_client().is_some());
        assert!(context.model_manager().is_some());
        assert!(context.temp_dir().path().exists());
    }
    
    #[test]
    fn test_mock_backend_setup() {
        let server = setup_mock_backend();
        let url = server.url("/health");
        
        // Make a request to verify the mock is working
        let response = reqwest::blocking::get(&url).unwrap();
        assert_eq!(response.status(), 200);
        
        let body: serde_json::Value = response.json().unwrap();
        assert_eq!(body["status"], "healthy");
        
        server.shutdown();
    }
    
    #[tokio::test]
    async fn test_create_test_file() {
        let context = TestContext::new().await;
        let file_path = context.create_test_file("test.txt", b"Hello, world!");
        
        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, world!");
    }
    
    #[tokio::test]
    async fn test_create_test_audio_file() {
        let context = TestContext::new().await;
        let file_path = context.create_test_audio_file("test", "mp3");
        
        assert!(file_path.exists());
        assert_eq!(file_path.extension().unwrap(), "mp3");
    }
}