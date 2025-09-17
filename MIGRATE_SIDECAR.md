# ASR Pro Python Sidecar Migration

## Complete Feature Inventory for Python Sidecar

### 1. Core AI Model Management

#### 1.1 Model Registry and Discovery
- [ ] Model specification registry (MODEL_SPECS)
- [ ] Model loader class registry (MODEL_LOADERS)
- [ ] Model metadata management (name, size, description)
- [ ] Model availability checking
- [ ] Model version management

#### 1.2 Model Loading and Unloading
- [ ] Lazy loading system (load on demand)
- [ ] Model unloading with memory cleanup
- [ ] Single resident model management
- [ ] Thread-safe model operations
- [ ] Model switching with proper cleanup
- [ ] Auto-unload timer system (configurable timeout)
- [ ] Memory optimization for different devices

#### 1.3 Device Detection and Optimization
- [ ] Enhanced device detection:
  - Apple Silicon (MPS) support
  - CUDA GPU support
  - Vulkan GPU support
  - CPU fallback
- [ ] Device-specific optimizations:
  - CUDA: Mixed precision (bfloat16, float16)
  - MPS: Metal Performance Shaders optimization
  - Vulkan: ONNX Runtime execution provider
  - CPU: INT8 quantization
- [ ] Device capability testing
- [ ] Graceful degradation on unsupported devices

### 2. AI Model Implementations

#### 2.1 Parakeet Models (NVIDIA)
- [ ] Parakeet TDT 0.6B model loader:
  - NeMo framework integration
  - Device-specific loading
  - Timestamp extraction
  - Segment-based transcription
- [ ] Parakeet TDT 1.1B model loader:
  - Same features as 0.6B but larger model
  - Enhanced accuracy
  - Proper memory management

#### 2.2 Whisper Models
- [ ] Whisper Medium ONNX loader:
  - faster-whisper integration
  - Device and compute type selection
  - VAD filter support
  - Beam size optimization
  - Progress callbacks
- [ ] Model optimization settings:
  - CUDA: float16 compute type
  - CPU: INT8 quantization
  - Fallback mechanisms
  - Error handling and recovery

#### 2.3 Model Abstraction Layer
- [ ] BaseLoader interface standardization
- [ ] Consistent API across all models
- [ ] Progress callback system
- [ ] Error handling standardization
- [ ] Model wrapper pattern implementation

### 3. Audio Processing

#### 3.1 Audio Recording
- [ ] Real-time audio recording:
  - Configurable sample rate (default 16kHz)
  - Mono channel support
  - PCM16 WAV format output
  - Device-specific recording
- [ ] Thread-safe recording system
- [ ] Queue-based audio buffering
- [ ] File writing with proper cleanup
- [ ] Parent directory creation

#### 3.2 Audio Device Management
- [ ] Audio device enumeration:
  - Input device listing
  - Device capability detection
  - Default device identification
  - Device information (name, channels, sample rate)
- [ ] Device selection and validation
- [ ] Device hot-swapping support
- [ ] Device error handling

#### 3.3 Audio Format Conversion
- [ ] FFmpeg integration for format conversion:
  - Video to audio extraction
  - Audio format normalization
  - Sample rate conversion
  - Channel mixing
- [ ] Temporary file management
- [ ] Conversion progress tracking
- [ ] Format validation

#### 3.4 Platform-specific Audio Handling
- [ ] macOS microphone permission management:
  - Permission checking
  - System Settings integration
  - Permission request workflow
  - Error messages and guidance
- [ ] Audio subsystem optimization
- [ ] Platform-specific device handling

### 4. Transcription Services

#### 4.1 File Transcription
- [ ] Batch transcription of audio files:
  - WAV file processing
  - Multi-format support via FFmpeg
  - Large file handling
  - Progress tracking
- [ ] Real-time transcription feedback
- [ ] Error recovery mechanisms
- [ ] Output format options:
  - JSON with segments
  - Plain text
  - SRT subtitle format

#### 4.2 Real-time Transcription
- [ ] Live audio stream processing
- [ ] Chunk-based transcription
- [ ] Low-latency processing
- [ ] Stream management
- [ ] Real-time result delivery

#### 4.3 Transcription Quality Optimization
- [ ] Language detection support
- [ ] Multi-language processing
- [ ] Confidence scoring
- [ ] Post-processing filters
- [ ] Punctuation and capitalization

### 5. HTTP API Server

#### 5.1 FastAPI Server Implementation
- [ ] OpenAI-compatible API endpoints:
  - `/v1/models` - Model listing
  - `/v1/audio/transcriptions` - File transcription
  - `/v1/audio/translations` - Translation (fallback to transcription)
  - `/v1/srt` - SRT generation
  - `/v1/settings/model` - Model selection
  - `/health` - Health check
- [ ] RESTful API design
- [ ] Request/response validation
- [ ] Error handling and HTTP status codes

#### 5.2 File Upload Handling
- [ ] Multipart form data support
- [ ] File type validation
- [ ] Size limit management
- [ ] Temporary file handling
- [ ] Security considerations

#### 5.3 Response Format Support
- [ ] JSON response format
- [ ] Plain text response
- [ ] SRT format response
- [ ] Segmented response with timestamps
- [ ] Custom formatting options

#### 5.4 Server Management
- [ ] Thread-safe server operation
- [ ] Graceful shutdown handling
- [ ] Port conflict resolution
- [ ] Connection management
- [ ] Logging and monitoring

### 6. Configuration Management

#### 6.1 Settings Persistence
- [ ] Platform-specific config directories:
  - macOS: ~/Library/Application Support/asrpro/
  - Windows: %APPDATA%/asrpro/
  - Linux: ~/.config/asrpro/
- [ ] JSON-based configuration storage
- [ ] Default configuration management
- [ ] Configuration migration from legacy locations
- [ ] Settings validation and defaults

#### 6.2 Configuration Categories
- [ ] Device preference settings
- [ ] Server configuration (enabled, host, port, auto_start)
- [ ] UI settings (theme, opacity, animations)
- [ ] Audio settings (sample rate, channels, format, quality)
- [ ] Model settings (auto_unload, prefer_gpu, cache_directory)
- [ ] Hotkey settings (combination, overlay, auto_paste)

#### 6.3 Runtime Configuration
- [ ] Hot-reloadable settings
- [ ] Configuration change notifications
- [ ] Settings synchronization with frontend
- [ ] Configuration validation
- [ ] Error handling for corrupted configs

### 7. System Integration

#### 7.1 Process Management
- [ ] Single instance enforcement:
  - PID file management
  - Process detection and cleanup
  - User interaction for existing instances
  - Graceful shutdown handling
- [ ] Signal handling (SIGINT, etc.)
- [ ] Process lifecycle management

#### 7.2 Platform-specific Features
- [ ] macOS-specific optimizations:
  - Apple Silicon (MPS) support
  - Microphone permission handling
  - System integration
- [ ] Windows-specific features:
  - CUDA support
  - System tray integration
  - Registry handling
- [ ] Linux-specific features:
  - Vulkan support
  - Desktop integration
  - Package management

#### 7.3 Resource Management
- [ ] Memory optimization:
  - GPU memory cleanup
  - MPS cache clearing
  - Garbage collection
  - Memory usage monitoring
- [ ] CPU usage optimization
- [ ] Disk space management
- [ ] Network resource management

### 8. Background Services

#### 8.1 Model Auto-unload Service
- [ ] Configurable timeout system
- [ ] Activity tracking and monitoring
- [ ] Timer-based unloading
- [ ] Resource cleanup on unload
- [ ] User notification system

#### 8.2 Health Monitoring
- [ ] Service health checks
- [ ] Resource usage monitoring
- [ ] Error detection and reporting
- [ ] Performance metrics collection
- [ ] Diagnostic information

#### 8.3 Update Management
- [ ] Model update checking
- [ ] Download management
- [ ] Version compatibility
- [ ] Rollback capabilities
- [ ] Update notifications

### 9. Security and Permissions

#### 9.1 Permission Management
- [ ] macOS permissions:
  - Microphone access
  - Accessibility services
  - Full disk access
  - System integration
- [ ] Windows permissions:
  - Microphone access
  - Firewall rules
  - Administrative privileges
- [ ] Linux permissions:
  - Audio device access
  - User permissions
  - System integration

#### 9.2 Data Security
- [ ] Temporary file security
- [ ] Network communication security
- [ ] User data protection
- [ ] API access control
- [ ] Sensitive data handling

#### 9.3 Error Handling and Logging
- [ ] Comprehensive error handling
- [ ] Structured logging
- [ ] Debug information
- [ ] User-friendly error messages
- [ ] Error recovery mechanisms

### 10. Performance Optimization

#### 10.1 Model Optimization
- [ ] Quantization support
- [ ] Pruning and compression
- [ ] Batch processing optimization
- [ ] Memory mapping
- [ ] Caching strategies

#### 10.2 Concurrency and Threading
- [ ] Thread-safe operations
- [ ] Async processing
- [ ] Lock management
- [ ] Thread pool management
- [ ] Deadlock prevention

#### 10.3 Resource Cleanup
- [ ] Proper resource disposal
- [ ] Memory leak prevention
- [ ] File handle management
- [ ] Network connection cleanup
- [ ] GPU resource management

## Implementation Strategy

### Phase 1: Core Architecture
1. Set up Python sidecar project structure
2. Implement model management system
3. Create device detection and optimization
4. Set up configuration management

### Phase 2: Model Integration
1. Implement Parakeet model loaders
2. Implement Whisper model loaders
3. Create model abstraction layer
4. Add progress callback system

### Phase 3: Audio Processing
1. Implement audio recording system
2. Add audio device management
3. Integrate FFmpeg for format conversion
4. Add platform-specific audio handling

### Phase 4: API Server
1. Set up FastAPI server
2. Implement OpenAI-compatible endpoints
3. Add file upload handling
4. Create response format support

### Phase 5: System Integration
1. Implement process management
2. Add platform-specific features
3. Create resource management
4. Add background services

### Phase 6: Security and Performance
1. Implement permission management
2. Add security features
3. Optimize performance
4. Add comprehensive error handling

## Technology Stack Recommendations

### Core Framework
- **Web Framework**: FastAPI for HTTP API
- **Async Runtime**: asyncio with uvicorn
- **Configuration**: Pydantic for validation
- **Logging**: structlog or standard logging

### AI/ML Libraries
- **PyTorch**: Core ML framework
- **NVIDIA NeMo**: Parakeet models
- **faster-whisper**: Whisper ONNX implementation
- **ONNX Runtime**: Cross-platform inference
- **sounddevice**: Audio recording
- **numpy**: Audio processing

### System Integration
- **psutil**: Process management
- **platform**: Platform detection
- **subprocess**: External process management
- **threading**: Concurrency
- **tempfile**: Temporary file management

### Audio Processing
- **FFmpeg**: Format conversion (external binary)
- **soundfile**: Audio file handling
- **librosa**: Audio analysis (optional)

### Utilities
- **pathlib**: Path manipulation
- **json**: Configuration storage
- **typing**: Type hints
- **dataclasses**: Data structures

## File Structure Proposal

```
sidecar/
├── __init__.py
├── main.py                 # Entry point
├── config/
│   ├── __init__.py
│   ├── manager.py         # Configuration management
│   └── settings.py        # Settings definitions
├── models/
│   ├── __init__.py
│   ├── manager.py         # Model management
│   ├── base.py           # Base loader interface
│   ├── parakeet.py       # Parakeet implementations
│   ├── whisper.py        # Whisper implementations
│   └── registry.py       # Model registry
├── audio/
│   ├── __init__.py
│   ├── recorder.py       # Audio recording
│   ├── devices.py        # Device management
│   └── converter.py      # Format conversion
├── api/
│   ├── __init__.py
│   ├── server.py         # FastAPI server
│   ├── endpoints.py      # API endpoints
│   └── models.py         # API models/schemas
├── system/
│   ├── __init__.py
│   ├── process.py        # Process management
│   ├── platform.py       # Platform-specific code
│   └── resources.py      # Resource management
├── security/
│   ├── __init__.py
│   ├── permissions.py    # Permission management
│   └── validation.py     # Input validation
├── utils/
│   ├── __init__.py
│   ├── logging.py        # Logging setup
│   ├── errors.py         # Error handling
│   └── performance.py    # Performance monitoring
└── cli.py                # Command line interface
```

## API Contract with Frontend

### HTTP Endpoints

#### Model Management
```typescript
// List available models
GET /v1/models
Response: {
  "object": "list",
  "data": [
    {
      "id": "parakeet-tdt-0.6b",
      "object": "model", 
      "owned_by": "asrpro",
      "ready": true
    }
  ]
}

// Set active model
POST /v1/settings/model
Request: { "model_id": "whisper-medium-onnx" }
Response: { "status": "success", "model": "whisper-medium-onnx" }
```

#### Transcription
```typescript
// Transcribe audio file
POST /v1/audio/transcriptions
Request: FormData {
  file: File,
  model?: string,
  response_format?: "json" | "srt" | "text"
}
Response: {
  "text": "Transcribed text",
  "segments": [
    {
      "start": 0.0,
      "end": 1.5,
      "text": "Hello world"
    }
  ]
}
```

#### System Status
```typescript
// Health check
GET /health
Response: {
  "status": "healthy",
  "current_model": "whisper-medium-onnx",
  "device": "mps"
}
```

### WebSocket Events

```typescript
// Model loading progress
{
  "type": "model_progress",
  "model_id": "whisper-medium-onnx",
  "progress": 0.75,
  "message": "Loading model..."
}

// Transcription progress
{
  "type": "transcription_progress",
  "file_id": "uuid",
  "progress": 0.5,
  "stage": "transcribing"
}

// System notifications
{
  "type": "notification",
  "level": "info" | "warning" | "error",
  "message": "Model unloaded due to inactivity"
}
```

## Key Challenges and Solutions

### 1. Cross-platform Model Support
- **Challenge**: Different GPU backends across platforms
- **Solution**: Unified device detection with fallback chains

### 2. Memory Management
- **Challenge**: Large models consuming significant memory
- **Solution**: Auto-unload system and proper cleanup procedures

### 3. Real-time Performance
- **Challenge**: Low-latency transcription for real-time use
- **Solution**: Optimized model loading and chunked processing

### 4. Permission Handling
- **Challenge**: Platform-specific permission requirements
- **Solution**: Permission detection and user guidance system

### 5. API Compatibility
- **Challenge**: Maintaining OpenAI API compatibility
- **Solution**: Careful endpoint design and response formatting
