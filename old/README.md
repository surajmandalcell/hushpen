# ASR Pro - AI Speech Recognition

A professional desktop application for AI-powered speech recognition and transcription, built with PySide6.

## Features

- **Global Hotkey Transcription**: Press once to record, press again to transcribe and paste
- **SRT Generation**: Drag & drop media files to create subtitle files
- **Multi-Model Support**: Parakeet TDT (0.6B/1.1B) and Whisper Medium ONNX
- **Single Instance**: Automatically kills old instances when launching new ones
- **Modern UI**: Glassmorphism design with dark mode tray icon support
- **System Integration**: Comprehensive system tray with context menu

## Installation

```bash
# Clone repository
git clone https://github.com/surajmandalcell/asrpro.git
cd asrpro

# Quick setup (creates venv and installs dependencies)
make setup

# Or manual installation
python3 -m venv .venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate
pip install -r requirements.txt

# macOS users: install additional dependencies
make install-macos
```

## Usage

### Quick Start
```bash
# Run the application
make run

# Or run in development mode with hot-reload
make dev

# Or use Python directly
python -m asrpro
```

### Initial Setup
1. **System tray**: Look for the asrpro icon in your system tray
2. **Open window**: Double-click tray icon or right-click → "Show Window"
3. **Load model**: Go to Models tab, select a model, click "Load Selected Model"
4. **Set hotkey**: Right-click tray → "Hotkey Settings" (default: Ctrl+Alt+T)
5. **Test**: Press hotkey twice (start recording → stop & transcribe)

### Real-time Transcription
1. Press configured hotkey to start recording
2. Speak naturally
3. Press hotkey again to stop and transcribe
4. Text automatically pastes to active window

### SRT Generation
1. Open main window (double-click tray icon)
2. Go to SRT tab
3. Drag & drop media files or click "Browse Files"
4. Wait for processing to complete
5. Find generated .srt files next to original media

## Project Structure

```
asrpro/
├── asrpro/             # Main application package
│   ├── __init__.py     # Package exports
│   ├── __main__.py     # Entry point
│   ├── main.py         # App bootstrap
│   ├── models.py       # Model loaders
│   ├── model_manager.py # Model lifecycle
│   ├── config.py       # Configuration (uses ~/Library/Application Support/asrpro on macOS)
│   ├── server.py       # FastAPI server
│   ├── audio_recorder.py # Audio recording
│   ├── hotkey.py       # Global hotkeys
│   └── ui/             # User interface
│       ├── components/ # UI components
│       ├── layouts/    # Layout managers
│       ├── pages/      # Application pages
│       ├── styles/     # Styling
│       └── utils/      # UI utilities
├── assets/             # Icons, fonts, resources
├── scripts/            # Build and utility scripts
├── tests/              # Test suite
├── Makefile            # Development commands
├── pyproject.toml      # Modern Python packaging
├── setup.py            # py2app support
├── requirements.txt    # Core dependencies
└── requirements-macos.txt # macOS-specific deps
```

## Configuration

- **Models**: Choose between Parakeet TDT (0.6B/1.1B) or Whisper Medium ONNX
- **Device**: Auto-detects CUDA → MPS (Apple Silicon) → Vulkan → CPU
- **Hotkey**: Configurable via tray menu (default: Ctrl+Alt+T)
- **Server**: Optional OpenAI-compatible API on port 7341
- **Theme**: Automatic dark mode detection with icon inversion
- **Config Location**:
  - macOS: `~/Library/Application Support/asrpro/`
  - Windows: `%APPDATA%/asrpro/`
  - Linux: `~/.config/asrpro/`

## Build & Development

### Development Commands
```bash
# Run tests
make test

# Format code
make format

# Lint code
make lint

# Clean build artifacts
make clean

# See all available commands
make help
```

### Build Standalone Executable
```bash
# Build with Nuitka (cross-platform)
make build

# Build macOS .app bundle
make build-py2app
```

## Requirements

- **Python**: 3.11+
- **OS**: Windows, macOS, Linux
- **Memory**: 4GB+ RAM (model dependent)
- **GPU**: Optional - CUDA (NVIDIA), MPS (Apple Silicon), or Vulkan
- **macOS**: Requires accessibility permissions for global hotkeys
- **FFmpeg**: Required for audio processing
  - macOS: `brew install ffmpeg`
  - Windows: `winget install ffmpeg`
  - Linux: `sudo apt install ffmpeg`

## Architecture

**Single Instance Enforcement**: Only one asrpro instance runs at a time. New launches automatically terminate previous instances.

**Consolidated Architecture**: All model loaders, dependencies, and configurations are consolidated into minimal files for easy maintenance and deployment.

**Modern Design Features**:
- Glassmorphism UI with semi-transparent elements and blur effects
- Automatic dark mode system theme detection
- Smooth animations and responsive transitions
- Minimal, distraction-free interface design
- High contrast, accessible typography

## License

MIT License - see LICENSE file for details.

## Author

Developed by [Suraj Mandal](https://github.com/surajmandalcell)