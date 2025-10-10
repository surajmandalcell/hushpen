//! Common test utilities and configurations
//!
//! This module provides shared functionality for all tests including
//! test setup, teardown, and common utilities.

pub mod fixtures;
pub mod mocks;
pub mod test_utils;
pub mod test_app;

use std::sync::Once;
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

/// Initialize the test environment
/// This should be called at the beginning of each test
pub fn init_test_env() {
    INIT.call_once(|| {
        // Initialize logger for tests
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .init();
        
        // Set up test environment variables
        std::env::set_var("RUST_LOG", "debug");
        std::env::set_var("RUST_BACKTRACE", "1");
        
        // Initialize GTK for tests (if needed)
        if std::env::var("DISPLAY").is_ok() {
            gtk4::init().expect("Failed to initialize GTK");
        }
    });
}

/// Create a new async runtime for tests
pub fn create_test_runtime() -> Runtime {
    Runtime::new().expect("Failed to create test runtime")
}

/// Test configuration constants
pub mod config {
    /// Test backend URL
    pub const TEST_BACKEND_URL: &str = "http://localhost:8080";
    
    /// Test WebSocket URL
    pub const TEST_WS_URL: &str = "ws://localhost:8080/ws";
    
    /// Test timeout in seconds
    pub const TEST_TIMEOUT_SECS: u64 = 30;
    
    /// Default test audio file
    pub const TEST_AUDIO_FILE: &str = "test_audio.mp3";
    
    /// Test model name
    pub const TEST_MODEL_NAME: &str = "whisper-tiny";
}

/// Test macros for common operations
#[macro_export]
macro_rules! async_test {
    ($test_name:ident, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            $crate::common::init_test_env();
            $test_body
        }
    };
}

#[macro_export]
macro_rules! setup_test_app {
    () => {{
        use $crate::common::test_app::TestApp;
        let app = TestApp::new().await;
        app
    }};
}

#[macro_export]
macro_rules! assert_error_type {
    ($result:expr, $error_type:pat) => {
        match $result {
            Err($error_type) => (),
            Ok(_) => panic!("Expected error but got Ok"),
            Err(_) => panic!("Expected specific error type"),
        }
    };
}