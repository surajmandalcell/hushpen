# asrpro - AI Speech Recognition Pro

A modern desktop tray application for AI-powered speech recognition and transcription, built with PySide6 and featuring a clean, linear UI design.

## Features

### 🎯 Core Functionality
- **Global Hotkey Transcription**: Press once to start recording, press again to stop and paste the transcribed text into the last active window
- **SRT Generation**: Drag & drop media files to generate subtitle files with matching filenames
- **Multi-Model Support**: Load-on-demand support for NVIDIA Parakeet TDT (0.6B & 1.1B) and Whisper Medium ONNX models
- **Single Model Residency**: Only one model stays in memory at a time for optimal resource usage

### 🎨 Modern Linear UI
- **Frameless Window**: Fixed 800x600 non-resizable window with custom title bar
- **Linear Design**: Clean, minimal interface with GitHub-inspired dark theme
- **Smooth Animations**: Elegant overlay dialog with fade-in/fade-out effects
- **Two-Tab Layout**: Separate tabs for Models and SRT generation

### ⚡ Performance & Compatibility
- **Device Preference**: Automatically uses CUDA → Vulkan → CPU fallback
- **OpenAI-Compatible API**: Built-in server on port 7341 with familiar endpoints
- **System Integration**: System tray with comprehensive menu options
- **Real-time Processing**: Responsive UI that doesn't disrupt target applications

## Installation

### Prerequisites
```bash
# Python 3.11+ required
python --version

# Install FFmpeg (required for audio processing)
# Windows (using winget):
winget install ffmpeg

# Or download from: https://www.gyan.dev/ffmpeg/builds/
```

### Dependencies
```bash
# Core requirements
pip install -r requirements.txt

# Optional: NeMo for Parakeet models (heavy dependency)
pip install -r requirements-nemo.txt
```

### Key Dependencies
- **PySide6 6.9.2**: Modern Qt6-based GUI framework
- **FastAPI + Uvicorn**: High-performance API server
- **PyTorch 2.8.0**: ML framework with CUDA support
- **faster-whisper 1.2.0**: Optimized Whisper implementation
- **sounddevice**: Real-time audio recording
- **pynput**: Global hotkey and clipboard integration

## Usage

### Running the Application
```bash
# Method 1: Direct execution
python asrpro_run.py

# Method 2: Module execution
python -m asrpro.main

# Method 3: Test script with enhanced output
python test_asrpro.py
```

### Global Hotkey Workflow
1. **Configure Hotkey**: Right-click tray icon → "Hotkey Settings" (default: `Ctrl+Alt+T`)
2. **Start Recording**: Press the hotkey once - overlay appears
3. **Stop & Paste**: Press the hotkey again - transcribed text is automatically pasted

### SRT Generation
1. **Load a Model**: Open the app → Models tab → select and load a model
2. **Process Media**: 
   - Drag & drop files onto the SRT tab, OR
   - Use "Choose Media File" button, OR  
   - Right-click tray icon → "Process Media File"
3. **Output**: `.srt` file created next to the original media file

### API Server Usage

The built-in server runs on `http://127.0.0.1:7341` and provides OpenAI-compatible endpoints:

```bash
# List available models
curl http://127.0.0.1:7341/v1/models

# Transcribe audio file
curl -X POST http://127.0.0.1:7341/v1/audio/transcriptions \
  -F "file=@audio.wav" \
  -F "model=whisper-medium-onnx"

# Generate SRT file
curl -X POST http://127.0.0.1:7341/v1/srt \
  -F "file=@video.mp4" \
  -F "model=parakeet-tdt-0.6b"

# Health check
curl http://127.0.0.1:7341/health
```

## Supported Models

### NVIDIA Parakeet TDT
- **parakeet-tdt-0.6b**: Lightweight model (600M parameters)
- **parakeet-tdt-1.1b**: Larger model (1.1B parameters)
- **Requirements**: NeMo framework (`pip install -r requirements-nemo.txt`)

### Whisper ONNX
- **whisper-medium-onnx**: Optimized Whisper Medium via faster-whisper
- **Requirements**: ONNX Runtime with GPU support

## Configuration

### Hotkey Configuration
- **Format**: `<modifier>+<key>` (e.g., `<ctrl>+<alt>+t`)
- **Modifiers**: `<ctrl>`, `<alt>`, `<shift>`, `<cmd>`
- **Keys**: Letters, numbers, function keys (F1-F12)
- **Storage**: Saved to `~/.asrpro_hotkey.json`

### Device Selection
Automatic fallback order:
1. **CUDA**: If NVIDIA GPU available with CUDA support
2. **Vulkan**: If Vulkan-compatible GPU available  
3. **CPU**: Universal fallback

## Architecture

### Project Structure
```
asrpro/
├── main.py              # Application bootstrap
├── model_manager.py     # Lazy model loading & management
├── server.py           # FastAPI server with OpenAI endpoints
├── hotkey.py           # Global hotkey handling
├── audio_recorder.py   # Real-time audio capture
├── ui/
│   ├── main_window.py  # Primary application window
│   ├── custom_titlebar.py  # Frameless window controls
│   ├── overlay.py      # Recording status overlay
│   └── tray.py         # System tray integration
└── models/
    ├── base.py         # Model loader interface
    ├── parakeet_06b.py # NVIDIA Parakeet 0.6B loader
    ├── parakeet_11b.py # NVIDIA Parakeet 1.1B loader
    └── whisper_medium_onnx.py  # Whisper ONNX loader
```

### Key Design Principles
- **Single Model Residency**: Automatic unloading when switching models
- **Lazy Loading**: Models load only when selected
- **Non-blocking UI**: Long operations don't freeze the interface
- **Resource Cleanup**: Proper GPU memory management
- **Focus Preservation**: Recording doesn't steal focus from target apps

## API Reference

### OpenAI-Compatible Endpoints

#### `GET /v1/models`
List available models
```json
{
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
```

#### `POST /v1/audio/transcriptions`
Transcribe audio file
- **Parameters**: `file` (audio), `model` (optional), `response_format` (json|srt)
- **Returns**: Transcription with timestamps

#### `POST /v1/srt`  
Generate SRT subtitle file
- **Parameters**: `file` (media), `model` (optional)
- **Returns**: SRT-formatted subtitle text

#### `GET /health`
Health check endpoint
```json
{
  "status": "healthy",
  "current_model": "whisper-medium-onnx", 
  "device": "cuda"
}
```

## Troubleshooting

### Common Issues

**No audio recording**
- Check microphone permissions
- Verify `sounddevice` installation: `python -c "import sounddevice; print(sounddevice.query_devices())"`

**Model loading fails**
- Ensure sufficient GPU/system memory
- Check CUDA installation for NVIDIA models
- Verify internet connection for initial model downloads

**Hotkey not working**
- Run as administrator (Windows)
- Check for conflicting global hotkeys
- Verify `pynput` installation

**API server not accessible**
- Check Windows Firewall settings
- Ensure port 7341 is not occupied: `netstat -an | find "7341"`

### Performance Optimization

**For CUDA users**:
```bash
# Install CUDA-optimized PyTorch
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu121
```

**For CPU-only systems**:
- Use Whisper ONNX models for better performance
- Consider quantized model variants

## Development

### Building from Source
```bash
git clone <repository>
cd jarusasr
python -m venv .venv
.venv\Scripts\activate  # Windows
pip install -r requirements.txt
python asrpro_run.py
```

### Packaging
```bash
# Using Nuitka for standalone executable
pip install nuitka
nuitka --standalone --enable-plugin=pyside6 --onefile \
       --nofollow-import-to=tkinter \
       --include-data-files=assets/*=assets/* \
       --output-dir=dist asrpro_run.py
```

## License

Open source project. See LICENSE file for details.

## Contributing

Contributions welcome! Areas for improvement:
- Additional model support (Whisper variants, language-specific models)
- Enhanced UI themes and customization
- Mobile companion app
- Cloud deployment options
- Advanced audio preprocessing

---

**Built with ❤️ using PySide6, FastAPI, and modern AI models**