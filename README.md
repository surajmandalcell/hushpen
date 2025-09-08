# asrpro - AI Speech Recognition Pro

A clean, modern desktop tray application for AI-powered speech recognition and transcription, built with PySide6.

## ✨ Features

- **🎯 Global Hotkey Transcription**: Press once to record, press again to transcribe and paste
- **🎬 SRT Generation**: Drag & drop media files to create subtitle files
- **🤖 Multi-Model Support**: Parakeet TDT (0.6B/1.1B) and Whisper Medium ONNX
- **⚡ Single Instance**: Automatically kills old instances when launching new ones
- **🎨 Modern UI**: Glassmorphism design with dark mode tray icon support
- **🖥️ System Integration**: Comprehensive system tray with context menu

## 🚀 Quick Start

### Installation
```bash
# Clone and setup
git clone <repo>
cd jarusasr
python -m pip install -r requirements.txt
```

### Launch
```bash
# Method 1: Module
python -m asrpro

# Method 2: Simple launcher
python run.py
```

### First Use
1. **System tray**: Look for the asrpro icon in your system tray
2. **Open window**: Double-click tray icon or right-click → "Show Window"
3. **Load model**: Go to Models tab, select a model, click "Load Selected Model"
4. **Set hotkey**: Right-click tray → "Hotkey Settings" (default: Ctrl+Alt+T)
5. **Test**: Press hotkey twice (start recording → stop & transcribe)

## 📁 Project Structure

```
asrpro/
├── __init__.py         # Package exports
├── __main__.py         # Entry point for python -m asrpro
├── main.py             # App bootstrap with single instance enforcement
├── models.py           # All model loaders consolidated
├── model_manager.py    # Model lifecycle management
├── config.py           # Configuration management
├── server.py           # FastAPI server (OpenAI-compatible)
├── audio_recorder.py   # Audio recording utilities
├── hotkey.py           # Global hotkey handling
└── ui/
    ├── main_window.py  # Main application window
    ├── custom_titlebar.py # Frameless window title bar
    ├── overlay.py      # Recording status overlay
    └── tray.py         # System tray with dark mode icon support

assets/
└── icon.png           # Application icon (auto-inverts for dark mode)

run.py                 # Simple launcher script
requirements.txt       # All dependencies in one file
```

## 🔧 Configuration

- **Models**: Choose between Parakeet TDT (0.6B/1.1B) or Whisper Medium ONNX
- **Device**: Auto-detects CUDA → Vulkan → CPU
- **Hotkey**: Configurable via tray menu (default: Ctrl+Alt+T)
- **Server**: Optional OpenAI-compatible API on port 7341
- **Theme**: Automatic dark mode detection with icon inversion

## 🎯 Usage Patterns

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

## 🛠️ Build Standalone

```bash
python build_nuitka.py
```

## 📋 Requirements

- **Python**: 3.11+
- **OS**: Windows (primary), Linux/macOS (community)
- **Memory**: 4GB+ RAM (model dependent)
- **GPU**: Optional CUDA/Vulkan acceleration

## 🎨 Modern Design Features

- **Glassmorphism UI**: Semi-transparent elements with blur effects
- **Dark Mode**: Automatic system theme detection
- **Responsive**: Smooth animations and transitions
- **Clean**: Minimal, distraction-free interface
- **Accessible**: High contrast, readable typography

---

**Single Instance Enforcement**: Only one asrpro instance runs at a time. New launches automatically terminate previous instances.

**Consolidated Architecture**: All model loaders, dependencies, and configurations are consolidated into minimal files for easy maintenance and deployment.