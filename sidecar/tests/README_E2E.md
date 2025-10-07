# End-to-End Tests for Docker-based ASR System

This document describes the end-to-end test suite for the Docker-based ASR system.

## Overview

The end-to-end test suite validates the complete flow from frontend to Docker containers, ensuring that all components work together correctly. The tests cover:

- Backend server startup and health checks
- Docker container lifecycle management
- Model loading and switching
- Audio transcription through Docker containers
- GPU allocation and utilization
- WebSocket progress updates
- Error handling and recovery
- Concurrent request handling

## Test Structure

### Files

1. **`integration_helper.py`** - Helper utilities for test environment setup, audio file generation, and API interactions
2. **`test_config.json`** - Configuration file with test parameters, model settings, and test scenarios
3. **`test_e2e_docker_asr.py`** - Main end-to-end test implementation
4. **`run_e2e_tests.py`** - Test runner script that orchestrates the entire test suite

### Test Categories

1. **Server Health Tests**
   - Verify server startup and health check endpoint
   - Check API availability

2. **Model Management Tests**
   - List available models
   - Set active model
   - Model switching

3. **Transcription Tests**
   - Basic audio transcription
   - Different response formats (JSON, text, SRT)
   - Various audio file types

4. **Docker Container Tests**
   - Container startup and shutdown
   - Container lifecycle management
   - Resource allocation

5. **GPU Tests**
   - GPU memory allocation
   - GPU utilization monitoring
   - Concurrent GPU usage

6. **WebSocket Tests**
   - Real-time progress updates
   - Connection management
   - Multiple client handling

7. **Error Handling Tests**
   - Invalid model requests
   - Invalid audio formats
   - Container failures
   - GPU exhaustion

8. **Concurrency Tests**
   - Multiple simultaneous requests
   - Different models with different files
   - Resource contention

## Prerequisites

1. **Docker** - Ensure Docker is installed and running
2. **NVIDIA Docker** - For GPU support (optional but recommended)
3. **Python Dependencies** - Install required packages:
   ```bash
   cd sidecar
   pip install -r requirements.txt
   pip install -r tests/requirements.txt  # If it exists
   ```

## Running Tests

### Using the Test Runner (Recommended)

The easiest way to run the tests is using the test runner script:

```bash
# Run all tests with default configuration
python scripts/run_e2e_tests.py

# Run with custom configuration
python scripts/run_e2e_tests.py --config sidecar/tests/custom_config.json

# Save results to a specific file
python scripts/run_e2e_tests.py --output my_test_results.json

# Enable verbose logging
python scripts/run_e2e_tests.py --verbose

# Suppress console output
python scripts/run_e2e_tests.py --quiet
```

### Running Individual Tests

You can also run individual tests using pytest:

```bash
# Run all e2e tests
cd sidecar
pytest tests/test_e2e_docker_asr.py -v

# Run a specific test
pytest tests/test_e2e_docker_asr.py::TestE2EDockerASR::test_audio_transcription -v

# Run with custom configuration
pytest tests/test_e2e_docker_asr.py -v --config=tests/test_config.json
```

### Running Tests Directly

For debugging purposes, you can run the test file directly:

```bash
cd sidecar
python tests/test_e2e_docker_asr.py
```

## Configuration

The test suite is configured via `sidecar/tests/test_config.json`. Key configuration sections:

### Server Configuration
```json
{
  "server_host": "127.0.0.1",
  "server_port": 8000
}
```

### Model Configuration
```json
{
  "test_models": ["whisper-tiny", "whisper-base", "whisper-small"],
  "model_configs": {
    "whisper-tiny": {
      "image": "asrpro/whisper-tiny:latest",
      "port": 8001,
      "gpu_memory_mb": 2048,
      "expected_accuracy": 0.8,
      "max_startup_time": 60
    }
  }
}
```

### Test Scenarios
```json
{
  "concurrent_tests": {
    "enabled": true,
    "max_concurrent": 3,
    "test_scenarios": [...]
  },
  "error_scenarios": {
    "invalid_model": {...},
    "invalid_audio_format": {...}
  }
}
```

## Test Results

After running the tests, results are saved to `e2e_test_results.json` (or the specified output file). The results include:

- Summary statistics (total, passed, failed, skipped)
- Detailed results for each test
- Timing information
- Error messages and stack traces
- Environment information

### Example Output

```json
{
  "summary": {
    "total_tests": 11,
    "passed": 10,
    "failed": 1,
    "skipped": 0,
    "success_rate": 90.9
  },
  "timing": {
    "start_time": 1678901234.567,
    "end_time": 1678901290.123,
    "duration": 55.556
  },
  "results": {
    "test_server_health_check": {
      "status": "passed",
      "data": {...}
    },
    "test_audio_transcription": {
      "status": "failed",
      "error": "Container failed to start"
    }
  }
}
```

## Troubleshooting

### Common Issues

1. **Docker not available**
   - Ensure Docker is installed and running
   - Check Docker permissions

2. **GPU tests failing**
   - Ensure NVIDIA Docker is installed
   - Check GPU driver installation
   - Disable GPU tests in config if not available

3. **Server startup failures**
   - Check port availability (default: 8000)
   - Verify Python dependencies
   - Check logs in `test_e2e.log`

4. **Container startup timeouts**
   - Increase `container_startup` timeout in config
   - Check Docker image availability
   - Verify system resources

### Debug Mode

For detailed debugging, run with verbose logging:

```bash
python scripts/run_e2e_tests.py --verbose
```

This will:
- Enable debug-level logging
- Print detailed test information
- Save logs to `test_e2e.log`

## CI/CD Integration

The test suite is designed to be runnable in CI/CD environments:

```yaml
# Example GitHub Actions workflow
- name: Run E2E Tests
  run: |
    python scripts/run_e2e_tests.py --quiet --output test_results.json
    
- name: Upload Test Results
  uses: actions/upload-artifact@v3
  with:
    name: e2e-test-results
    path: test_results.json
```

## Customization

### Adding New Tests

1. Add test method to `TestE2EDockerASR` class in `test_e2e_docker_asr.py`
2. Update `run_all_tests()` function to include new test
3. Add configuration options to `test_config.json` if needed

### Adding New Test Scenarios

1. Update `test_config.json` with new scenario parameters
2. Modify test methods to use new scenarios
3. Update helper functions if new functionality is needed

## Performance Considerations

- Tests are designed to run with minimal resources
- GPU tests can be disabled if GPU is not available
- Concurrent tests are limited to avoid resource exhaustion
- Test audio files are small and generated automatically

## Security Considerations

- Tests use temporary directories that are cleaned up
- Test containers are labeled and cleaned up after tests
- No sensitive data is used in test files
- Server runs on localhost only