# ASR Pro Python Sidecar Requirements

## Core Purpose
The sidecar is an atomic, focused service that does exactly these things:
1. Load models
2. Unload models
3. Download models
4. Manage models
5. Provide OpenAI-like API endpoints

## Required Features

### 1. Model Management
- [ ] Model loading/unloading system
- [ ] Model registry and discovery
- [ ] Single resident model management
- [ ] Memory cleanup on model unload
- [ ] Model switching with proper cleanup
- [ ] Basic device detection (MPS, CUDA, CPU)
- [ ] Model metadata (name, size, description)

### 2. Model Implementations
- [ ] Whisper model loader (faster-whisper)
- [ ] Parakeet model loader (NeMo framework)
- [ ] BaseLoader interface standardization
- [ ] Error handling for model operations

### 3. HTTP API Server
- [ ] FastAPI server implementation
- [ ] OpenAI-compatible endpoints:
  - `GET /v1/models` - List available models
  - `POST /v1/audio/transcriptions` - File transcription
  - `POST /v1/settings/model` - Model selection
  - `GET /health` - Health check
- [ ] Basic file upload handling
- [ ] JSON response format
- [ ] Basic error handling and HTTP status codes

### 4. Configuration
- [ ] Model settings storage
- [ ] Server configuration (host, port)
- [ ] JSON-based config file
- [ ] Default configuration management

### 5. Device Support
- [ ] Apple Silicon (MPS) detection
- [ ] CUDA GPU detection
- [ ] CPU fallback
- [ ] Basic device capability testing

## Architecture

```
sidecar/
├── main.py                 # Entry point
├── models/
│   ├── __init__.py
│   ├── manager.py         # Load/unload/manage models
│   ├── registry.py        # Available models registry
│   ├── base.py           # BaseLoader interface
│   ├── whisper.py        # Whisper model loader
│   └── parakeet.py       # Parakeet model loader
├── api/
│   ├── __init__.py
│   ├── server.py         # FastAPI server
│   ├── endpoints.py      # OpenAI-compatible endpoints
│   └── models.py         # API response models
├── config/
│   ├── __init__.py
│   └── settings.py       # Basic configuration
├── utils/
│   ├── __init__.py
│   ├── device.py         # Basic device detection
│   └── errors.py         # Error handling
└── requirements.txt      # Dependencies
```

## Dependencies

```
fastapi>=0.104.1
uvicorn[standard]>=0.24.0
torch>=2.1.0
faster-whisper>=0.10.0
nemo-toolkit>=1.21.0
pydantic>=2.5.0
```

## API Contract

### Model Management
```typescript
GET /v1/models
Response: {
  "object": "list",
  "data": [
    {
      "id": "whisper-medium-onnx",
      "object": "model",
      "owned_by": "asrpro",
      "ready": true
    }
  ]
}

POST /v1/settings/model
Request: { "model_id": "whisper-medium-onnx" }
Response: { "status": "success", "model": "whisper-medium-onnx" }
```

### Transcription
```typescript
POST /v1/audio/transcriptions
Request: FormData { file: File, model?: string }
Response: { "text": "Transcribed text" }
```

### Health Check
```typescript
GET /health
Response: {
  "status": "healthy",
  "current_model": "whisper-medium-onnx",
  "device": "mps"
}
```

## Implementation Principles

1. **Atomic Focus**: Only model management and basic transcription
2. **No Audio Recording**: Frontend handles audio, sidecar only processes files
3. **No Complex UI**: Basic API responses, frontend handles presentation
4. **No System Integration**: No permissions, no process management, no background services
5. **Simple Configuration**: JSON file with model and server settings only
6. **OpenAI Compatibility**: Stick to standard OpenAI API patterns

## What This Sidecar Does

- Load models on demand
- Unload models to free memory
- Provide model list via API
- Transcribe uploaded audio files
- Switch between available models
- Detect basic device capabilities (MPS/CUDA/CPU)
- Serve OpenAI-compatible HTTP endpoints

## What This Sidecar Does NOT Do

- Record audio
- Manage audio devices
- Handle permissions
- Provide real-time streaming
- Manage hotkeys
- Show notifications
- Handle system integration
- Manage updates
- Provide complex monitoring
- Audio format conversion
- Real-time transcription
- SRT generation
- Complex error handling beyond basic API responses
- Background services
- Process management
- Auto-unload timers
- Progress callbacks
- User notifications
- Platform-specific optimizations
- Security beyond basic API validation