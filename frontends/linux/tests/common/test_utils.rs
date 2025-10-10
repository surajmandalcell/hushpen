//! Test utilities for common testing operations
//!
//! This module provides utility functions for common testing operations
//! including assertions, test data generation, and helper functions.

use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use asrpro_gtk4::models::{AudioFile, Model, TranscriptionTask};
use asrpro_gtk4::utils::{AppError, AppResult};

/// Assert that a result is an error of the given type
#[macro_export]
macro_rules! assert_error {
    ($result:expr, $error_type:pat) => {
        match $result {
            Err($error_type) => (),
            Ok(_) => panic!("Expected error but got Ok"),
            Err(_) => panic!("Expected specific error type"),
        }
    };
}

/// Assert that a result contains a specific error message
#[macro_export]
macro_rules! assert_error_message {
    ($result:expr, $expected_msg:expr) => {
        match $result {
            Err(e) => assert!(e.to_string().contains($expected_msg)),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    };
}

/// Asset that a result contains a specific substring
#[macro_export]
macro_rules! assert_contains {
    ($result:expr, $expected:expr) => {
        assert!($result.contains($expected), "Expected '{}' to contain '{}'", $result, $expected);
    };
}

/// Assert that two durations are approximately equal within a tolerance
pub fn assert_duration_approx(actual: Duration, expected: Duration, tolerance: Duration) {
    let diff = if actual > expected {
        actual - expected
    } else {
        expected - actual
    };
    
    assert!(
        diff <= tolerance,
        "Duration {} is not within {} of {}",
        actual.as_secs_f32(),
        tolerance.as_secs_f32(),
        expected.as_secs_f32()
    );
}

/// Assert that two floats are approximately equal within a tolerance
pub fn assert_float_approx(actual: f32, expected: f32, tolerance: f32) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tolerance,
        "Float {} is not within {} of {}",
        actual,
        tolerance,
        expected
    );
}

/// Wait for a condition to be true within a timeout
pub async fn wait_for_condition<F, Fut>(
    condition: F,
    timeout_duration: Duration,
) -> AppResult<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let result = timeout(timeout_duration, async {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            interval.tick().await;
            if condition().await {
                return Ok(());
            }
        }
    })
    .await;
    
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(AppError::generic("Timeout waiting for condition")),
    }
}

/// Wait for a task to reach a specific status
pub async fn wait_for_task_status(
    task: &TranscriptionTask,
    expected_status: asrpro_gtk4::models::TranscriptionStatus,
    timeout_duration: Duration,
) -> AppResult<()> {
    wait_for_condition(
        || async {
            task.status() == expected_status
        },
        timeout_duration,
    ).await
}

/// Create a test UUID
pub fn test_uuid() -> Uuid {
    Uuid::new_v4()
}

/// Create a test timestamp
pub fn test_timestamp() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

/// Create a test file path
pub fn test_file_path(name: &str, extension: &str) -> String {
    format!("/tmp/test_{}.{}", name, extension)
}

/// Check if two audio files are approximately equal
/// (comparing metadata, not actual audio content)
pub fn assert_audio_files_similar(file1: &AudioFile, file2: &AudioFile) {
    assert_eq!(file1.file_type(), file2.file_type());
    assert_eq!(file1.metadata().duration, file2.metadata().duration);
    assert_eq!(file1.metadata().sample_rate, file2.metadata().sample_rate);
    assert_eq!(file1.metadata().channels, file2.metadata().channels);
    assert_eq!(file1.metadata().file_size, file2.metadata().file_size);
}

/// Check if two models are approximately equal
pub fn assert_models_similar(model1: &Model, model2: &Model) {
    assert_eq!(model1.name, model2.name);
    assert_eq!(model1.display_name, model2.display_name);
    assert_eq!(model1.model_type, model2.model_type);
    assert_eq!(model1.size_bytes, model2.size_bytes);
}

/// Check if two transcription tasks are approximately equal
pub fn assert_transcription_tasks_similar(task1: &TranscriptionTask, task2: &TranscriptionTask) {
    assert_eq!(task1.file_path, task2.file_path);
    assert_eq!(task1.model_name, task2.model_name);
    assert_eq!(task1.status(), task2.status());
    
    if let (Some(result1), Some(result2)) = (task1.result(), task2.result()) {
        assert_eq!(result1.text, result2.text);
        assert_float_approx(result1.confidence, result2.confidence, 0.01);
        assert_eq!(result1.language, result2.language);
    }
}

/// Create a mock HTTP response
pub fn create_mock_response(
    status: u16,
    body: &str,
) -> httpmock::MockServer {
    let server = httpmock::MockServer::start();
    
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/test");
        then.status(status)
            .body(body);
    });
    
    server
}

/// Create a mock JSON response
pub fn create_mock_json_response(
    status: u16,
    json: &serde_json::Value,
) -> httpmock::MockServer {
    let server = httpmock::MockServer::start();
    
    server.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/test");
        then.status(status)
            .header("content-type", "application/json")
            .json_body(json);
    });
    
    server
}

/// Create a temporary file with the given content
pub fn create_temp_file(content: &[u8], extension: &str) -> std::io::Result<tempfile::NamedTempFile> {
    let mut file = tempfile::NamedTempFile::with_suffix(extension)?;
    file.write_all(content)?;
    Ok(file)
}

/// Create a temporary audio file with dummy content
pub fn create_temp_audio_file(extension: &str) -> std::io::Result<tempfile::NamedTempFile> {
    // Create some dummy audio data
    let dummy_data = vec![0u8; 1024];
    create_temp_file(&dummy_data, extension)
}

/// Create a temporary directory with test files
pub fn create_temp_directory_with_files(
    files: &[(&str, &[u8])],
) -> std::io::Result<tempfile::TempDir> {
    let temp_dir = tempfile::TempDir::new()?;
    
    for (filename, content) in files {
        let file_path = temp_dir.path().join(filename);
        std::fs::write(file_path, content)?;
    }
    
    Ok(temp_dir)
}

/// Generate a random string of the given length
pub fn random_string(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789";
    let mut rng = rand::thread_rng();
    
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate a random file name with the given extension
pub fn random_filename(extension: &str) -> String {
    format!("{}.{}", random_string(10), extension)
}

/// Generate a random port number for testing
pub fn random_port() -> u16 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    rng.gen_range(10000..65535)
}

/// Check if a port is available
pub fn is_port_available(port: u16) -> bool {
    use std::net::{TcpListener, SocketAddr};
    
    match TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port))) {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Find an available port for testing
pub fn find_available_port() -> u16 {
    let mut port = random_port();
    
    while !is_port_available(port) {
        port = random_port();
    }
    
    port
}

/// Test wrapper for functions that might panic
pub fn catch_unwind<F, R>(f: F) -> std::thread::Result<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
}

/// Assert that a function panics
pub fn assert_panics<F, R>(f: F)
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let result = catch_unwind(f);
    assert!(result.is_err(), "Expected function to panic");
}

/// Assert that a function does not panic
pub fn assert_does_not_panic<F, R>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let result = catch_unwind(f);
    assert!(result.is_ok(), "Expected function not to panic");
    result.unwrap()
}

/// Measure execution time of a function
pub fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = std::time::Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

/// Assert that a function executes within a time limit
pub fn assert_executes_within<F, R>(f: F, time_limit: Duration) -> R
where
    F: FnOnce() -> R,
{
    let (result, duration) = measure_time(f);
    assert!(
        duration <= time_limit,
        "Function took {} which exceeds the limit of {}",
        duration.as_secs_f32(),
        time_limit.as_secs_f32()
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_assert_duration_approx() {
        assert_duration_approx(
            Duration::from_secs(10),
            Duration::from_secs(9),
            Duration::from_secs(2),
        );
        
        // This should panic
        // assert_duration_approx(
        //     Duration::from_secs(10),
        //     Duration::from_secs(5),
        //     Duration::from_secs(2),
        // );
    }
    
    #[test]
    fn test_assert_float_approx() {
        assert_float_approx(1.0, 1.05, 0.1);
        assert_float_approx(1.0, 0.95, 0.1);
        
        // This should panic
        // assert_float_approx(1.0, 1.2, 0.1);
    }
    
    #[test]
    fn test_random_string() {
        let s1 = random_string(10);
        let s2 = random_string(10);
        
        assert_eq!(s1.len(), 10);
        assert_eq!(s2.len(), 10);
        assert_ne!(s1, s2);
    }
    
    #[test]
    fn test_random_filename() {
        let filename = random_filename("mp3");
        assert!(filename.ends_with(".mp3"));
        assert!(filename.len() > 4); // At least 1 character + ".mp3"
    }
    
    #[test]
    fn test_measure_time() {
        let (result, duration) = measure_time(|| {
            std::thread::sleep(Duration::from_millis(10));
            42
        });
        
        assert_eq!(result, 42);
        assert!(duration >= Duration::from_millis(10));
    }
    
    #[test]
    fn test_assert_executes_within() {
        let result = assert_executes_within(
            || std::thread::sleep(Duration::from_millis(10)),
            Duration::from_millis(50),
        );
        
        // This should not panic
        assert!(result.is_ok() || result.is_err()); // Result doesn't matter, just that it didn't panic
    }
}