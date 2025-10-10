# ASRPro GTK4 Frontend Test Suite

This directory contains the comprehensive test suite for the ASRPro GTK4 frontend application. The test suite is designed to ensure the reliability and quality of the implementation by covering critical functionality and edge cases.

## Test Structure

The test suite is organized into the following categories:

```
tests/
├── common/              # Common test utilities, fixtures, and helpers
├── unit/                # Unit tests for individual components
├── integration/         # Integration tests for component interactions
├── ui/                  # UI component tests
├── e2e/                 # End-to-end workflow tests
├── performance/         # Performance benchmarks and load tests
├── fixtures/            # Test data and fixtures
├── mocks/               # Mock implementations for testing
└── data/                # Static test data
```

## Running Tests

### Quick Start

To run all tests:

```bash
make test
```

To run specific test categories:

```bash
# Run unit tests only
make test-unit

# Run integration tests only
make test-integration

# Run UI component tests only
make test-ui

# Run end-to-end tests only
make test-e2e

# Run performance tests only
make test-performance
```

### Advanced Test Execution

To run tests with coverage reporting:

```bash
make test-coverage
```

To run tests in release mode:

```bash
make test-release
```

To run tests with verbose output:

```bash
make test-verbose
```

To run tests matching a specific pattern:

```bash
make test-filter FILTER="error_handling"
```

For more options, see the Makefile or run:

```bash
make help
```

### Running Tests with Cargo

You can also use Cargo directly to run tests:

```bash
# Run all tests
cargo test

# Run tests in a specific file
cargo test --test error_tests

# Run tests matching a pattern
cargo test error

# Run tests with specific features
cargo test --features "test-feature"

# Run tests in release mode
cargo test --release

# Run tests with verbose output
cargo test -- --nocapture

# Run tests including ignored ones
cargo test -- --ignored
```

## Test Categories

### Unit Tests

Unit tests focus on testing individual components in isolation. They are located in the `tests/unit/` directory and are organized by module:

- `utils/` - Tests for utility modules like error handling, file operations, and audio processing
- `models/` - Tests for data models and their validation
- `services/` - Tests for service layer components
- `ui/` - Tests for UI components without requiring a GUI

### Integration Tests

Integration tests verify that different components work together correctly. They are located in the `tests/integration/` directory:

- `services/` - Tests for service interactions
- `api/` - Tests for API communication
- `websocket/` - Tests for WebSocket functionality

### UI Component Tests

UI component tests verify the behavior of UI components. They are located in the `tests/ui/` directory:

- `components/` - Tests for UI components
- `widgets/` - Tests for custom widgets
- `windows/` - Tests for application windows

Note: UI tests require a display server (X11) to run.

### End-to-End Tests

End-to-end tests verify complete user workflows. They are located in the `tests/e2e/` directory:

- `workflows/` - Tests for complete user workflows
- `scenarios/` - Tests for specific user scenarios

### Performance Tests

Performance tests verify that the application meets performance requirements. They are located in the `tests/performance/` directory:

- `benchmarks/` - Performance benchmarks
- `load/` - Load testing scenarios

## Test Utilities

### Common Test Infrastructure

The `tests/common/` directory contains shared test utilities:

- `mod.rs` - Common test configuration and macros
- `fixtures.rs` - Test data fixtures
- `test_utils.rs` - Common test utilities and assertions
- `test_app.rs` - Test application setup
- `mocks.rs` - Mock implementations

### Test Fixtures

Test fixtures provide sample data for testing:

```rust
use tests::common::fixtures::*;

// Create a test audio file
let fixture = AudioFileFixture::new();
let audio_file = fixture.create_mp3_file();

// Create a test model
let model = ModelFixture::create_whisper_tiny();

// Create a test transcription task
let task = TranscriptionFixture::create_completed_task(
    "/test/file.mp3",
    "whisper-tiny",
    "This is a test transcription."
);
```

### Test Utilities

Common test utilities are available through the `test_utils` module:

```rust
use tests::common::test_utils::*;

// Assert that an error message contains specific text
assert_error_message!(result, "expected message");

// Wait for a condition to be true within a timeout
wait_for_condition(|| async { some_condition() }, Duration::from_secs(5)).await;

// Measure execution time
let (result, duration) = measure_time(|| some_operation());
```

### Mock Implementations

Mock implementations are available for testing external dependencies:

```rust
use tests::common::mocks::*;

// Create a mock backend client
let mock_client = MockBackendClient::new();

// Configure the mock to fail
mock_client.set_health_check_failure(true).await;

// Create a mock model manager
let mock_manager = MockModelManager::new(Arc::new(mock_client));
```

## Writing New Tests

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tests::common::fixtures::*;

    #[test]
    fn test_error_creation() {
        let error = AppError::api("Test error");
        assert!(matches!(error, AppError::Api { .. }));
        assert_eq!(error.to_string(), "API Error: Test error");
    }

    #[test]
    fn test_audio_file_validation() {
        let fixture = AudioFileFixture::new();
        let audio_file = fixture.create_mp3_file();
        
        assert!(audio_file.is_ready());
        assert_eq!(audio_file.file_type(), AudioFileType::Mp3);
    }
}
```

### Integration Test Example

```rust
#[tokio::test]
async fn test_backend_client_integration() {
    let test_context = TestContext::new().await;
    let backend_client = test_context.backend_client().unwrap();
    
    // Test health check
    let health = backend_client.health_check().await;
    assert!(health.is_ok());
    
    // Test model retrieval
    let models = backend_client.get_models().await;
    assert!(models.is_ok());
    assert!(!models.unwrap().is_empty());
}
```

### Performance Test Example

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_audio_processing(c: &mut Criterion) {
    let fixture = AudioFileFixture::new();
    let audio_file = fixture.create_mp3_file();
    
    c.bench_function("process_audio", |b| {
        b.iter(|| {
            process_audio(black_box(&audio_file))
        })
    });
}

criterion_group!(benches, bench_audio_processing);
criterion_main!(benches);
```

## Test Configuration

### Environment Variables

The test suite can be configured with the following environment variables:

- `RUST_LOG` - Log level for tests (default: `debug`)
- `RUST_BACKTRACE` - Enable backtraces on errors (default: `1`)
- `TEST_TIMEOUT` - Default timeout for async tests (default: `30` seconds)
- `TEST_DATA_DIR` - Directory for test data (default: `tests/data/`)
- `SKIP_UI_TESTS` - Skip UI tests that require a display (default: `false`)

### Test Configuration File

Test configuration can be provided in a `tests/config.toml` file:

```toml
[general]
timeout = 30
verbose = true

[backend]
url = "http://localhost:8080"
timeout = 10

[ui]
require_display = true
timeout = 5

[performance]
iterations = 100
warmup_iterations = 10
```

## Test Data

### Audio Files

Sample audio files are located in `tests/fixtures/audio/`:

- `sample.mp3` - Sample MP3 file for testing
- `sample.wav` - Sample WAV file for testing
- `sample.flac` - Sample FLAC file for testing

### Configuration Files

Sample configuration files are located in `tests/fixtures/configs/`:

- `default.json` - Default configuration
- `minimal.json` - Minimal configuration
- `full.json` - Full configuration with all options

### Test Data

Static test data is located in `tests/fixtures/data/`:

- `api_responses.json` - Sample API responses
- `websocket_messages.json` - Sample WebSocket messages
- `transcription_results.json` - Sample transcription results

## Continuous Integration

The test suite is designed to run in CI/CD environments. The following considerations are made:

1. **Headless Testing**: UI tests can be run in a headless environment using Xvfb
2. **Parallel Execution**: Tests are designed to run in parallel where possible
3. **Isolation**: Tests are isolated from each other and don't share state
4. **Deterministic**: Tests are deterministic and produce consistent results
5. **Fast Execution**: Tests are optimized for fast execution in CI environments

### CI Configuration

For GitHub Actions, you can use the following configuration:

```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Install dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y libgtk-4-dev
    
    - name: Run tests
      run: make test-all
    
    - name: Generate coverage report
      run: make test-coverage
    
    - name: Upload coverage to Codecov
      uses: codecov/codecov-action@v1
      with:
        file: coverage/lcov.info
```

## Troubleshooting

### Common Issues

1. **UI Tests Fail**: Make sure you have a display server running (X11) or set `SKIP_UI_TESTS=true`
2. **Tests Time Out**: Increase the timeout by setting `TEST_TIMEOUT` environment variable
3. **Mock Server Issues**: Make sure ports are available and not blocked by firewall
4. **Audio Tests Fail**: Make sure audio system is properly configured

### Debugging Tests

To debug tests:

1. Run tests with verbose output: `make test-verbose`
2. Enable logging: `RUST_LOG=debug cargo test`
3. Run a single test: `cargo test -- --exact test_name`
4. Use a debugger: `cargo test -- --nocapture`

## Contributing

When contributing new tests:

1. Follow the existing test structure and conventions
2. Use descriptive test names that explain what is being tested
3. Include assertions for both success and failure cases
4. Use fixtures and mocks to isolate tests from external dependencies
5. Document complex test scenarios with comments
6. Ensure tests are fast and don't introduce unnecessary delays

## Test Coverage Goals

The test suite aims to achieve the following coverage goals:

1. **Unit Tests**: 90%+ line coverage for core modules
2. **Integration Tests**: 80%+ coverage for service interactions
3. **UI Tests**: 70%+ coverage for UI components
4. **End-to-End Tests**: 100% coverage for critical user workflows
5. **Performance Tests**: Benchmarks for all performance-critical operations

Coverage reports can be generated with:

```bash
make test-coverage
```

The report will be available in `coverage/index.html`.