# GPU Testing for ASRPro

This document describes the comprehensive test suite for validating GPU utilization with the RTX 4070 Super and Docker model runner integration.

## Overview

The GPU testing suite includes:

1. **GPU Utilization Tests** - Validates GPU detection, memory allocation, and utilization tracking
2. **Whisper Container Tests** - Tests the Whisper GPU container functionality
3. **Docker Compose Tests** - Validates the Docker Compose configuration
4. **Test Runner** - Orchestrates all tests and generates comprehensive reports

## Prerequisites

Before running the tests, ensure you have the following installed:

- **NVIDIA Drivers** (version 515.65 or later)
- **Docker** (version 20.10 or later)
- **NVIDIA Container Toolkit** (for Docker GPU support)
- **CUDA** (version 11.0 or later)
- **Python** (version 3.8 or later)

### Installing NVIDIA Container Toolkit

```bash
# Add the package repositories
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | sudo tee /etc/apt/sources.list.d/nvidia-docker.list

# Install the nvidia-docker2 package
sudo apt-get update
sudo apt-get install -y nvidia-docker2

# Restart the Docker daemon
sudo systemctl restart docker

# Test the installation
docker run --rm --gpus all nvidia/cuda:12.1.0-base-ubuntu22.04 nvidia-smi
```

## Test Components

### 1. GPU Utilization Tests (`sidecar/tests/test_gpu_utilization.py`)

These tests validate:
- NVIDIA driver availability and version
- GPU detection and properties (RTX 4070 Super)
- CUDA availability and version
- Docker GPU support
- GPU memory allocation and deallocation
- GPU utilization tracking
- Concurrent allocation scenarios
- Real GPU memory allocation using PyTorch

### 2. Whisper Container Tests (`scripts/test_whisper_container.py`)

These tests validate:
- Container build process
- Container startup and health checks
- GPU access inside the container
- Audio transcription functionality
- GPU memory tracking during transcription
- Container log analysis

### 3. Docker Compose Tests

These tests validate:
- Docker Compose build process
- Container orchestration
- Service dependencies
- Network configuration
- Volume mounting

### 4. Test Runner (`scripts/run_gpu_tests.py`)

The test runner orchestrates all tests and generates comprehensive reports in both JSON and HTML formats.

## Running the Tests

### Quick Start

To run all tests with default settings:

```bash
cd /path/to/asrpro
python scripts/run_gpu_tests.py
```

### Advanced Options

```bash
# Run with verbose logging
python scripts/run_gpu_tests.py --verbose

# Specify custom output directory
python scripts/run_gpu_tests.py --output-dir /path/to/reports

# Only check prerequisites
python scripts/run_gpu_tests.py --prerequisites-only
```

### Running Individual Tests

#### GPU Utilization Tests

```bash
cd /path/to/asrpro
python -m pytest sidecar/tests/test_gpu_utilization.py -v
```

#### Whisper Container Tests

```bash
cd /path/to/asrpro
python scripts/test_whisper_container.py --verbose
```

#### Docker Compose Tests

```bash
cd /path/to/asrpro
docker-compose build
docker-compose up -d
# Wait for containers to be ready
docker-compose ps
docker-compose down
```

## Test Reports

After running the test suite, comprehensive reports are generated in the specified output directory (default: `test-reports/`):

- **JSON Report**: Detailed machine-readable results
- **HTML Report**: Human-readable report with visualizations

### Report Structure

The reports include:
- Test run information (timestamp, duration)
- Prerequisites check results
- Individual test results
- Overall summary and status
- Detailed output and error messages

## Manual Testing with Web Interface

For interactive testing, you can use the provided web interface:

```bash
# Start the containers with the UI profile
docker-compose --profile ui up -d

# Access the web interface
open http://localhost:8080
```

The web interface provides:
- Container health monitoring
- GPU information display
- Model management
- Audio transcription testing
- Real-time GPU monitoring

## Troubleshooting

### Common Issues

#### 1. NVIDIA Drivers Not Found

```
Error: NVIDIA drivers not installed or nvidia-smi not in PATH
```

**Solution**: Install NVIDIA drivers and ensure nvidia-smi is in your PATH.

#### 2. Docker GPU Support Not Available

```
Error: NVIDIA Container Toolkit not properly installed
```

**Solution**: Install the NVIDIA Container Toolkit as described in the prerequisites.

#### 3. CUDA Version Incompatible

```
Error: CUDA version too old
```

**Solution**: Update to CUDA 11.0 or later.

#### 4. Container Build Fails

```
Error: Container build failed
```

**Solution**: Check the build logs for specific errors. Common issues include:
- Insufficient disk space
- Network connectivity issues
- Missing dependencies

#### 5. GPU Memory Allocation Fails

```
Error: GPU memory allocation failed
```

**Solution**: Ensure no other GPU-intensive processes are running and that sufficient GPU memory is available.

### Debug Mode

For detailed debugging, you can run tests with verbose logging:

```bash
python scripts/run_gpu_tests.py --verbose
```

### Container Logs

To view container logs:

```bash
# View logs for running container
docker logs whisper-gpu-test

# View logs with follow
docker logs -f whisper-gpu-test
```

## Performance Benchmarking

The test suite includes basic performance monitoring. For more detailed benchmarking:

1. Use the web interface to monitor GPU utilization in real-time
2. Check the test reports for timing information
3. Use NVIDIA Nsight Systems for detailed GPU profiling

## Continuous Integration

These tests can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
name: GPU Tests
on: [push, pull_request]
jobs:
  gpu-tests:
    runs-on: self-hosted
    steps:
      - uses: actions/checkout@v2
      - name: Run GPU Tests
        run: python scripts/run_gpu_tests.py
      - name: Upload Test Reports
        uses: actions/upload-artifact@v2
        with:
          name: test-reports
          path: test-reports/
```

## Contributing

When adding new tests:

1. Follow the existing test structure
2. Include comprehensive error handling
3. Add appropriate logging
4. Update the test runner if necessary
5. Document the new tests in this file

## Support

For issues with the GPU testing suite:

1. Check the troubleshooting section
2. Review the test reports for detailed error messages
3. Check container logs for runtime issues
4. Create an issue in the project repository