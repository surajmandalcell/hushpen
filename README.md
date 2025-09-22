# ASR Pro

A professional desktop application for AI-powered speech recognition and transcription, built with Tauri + React + Python Sidecar architecture.

## Features

- Global hotkey transcription with customizable key combinations
- Real-time audio processing and transcription
- Multi-model AI support (Whisper, Parakeet)
- SRT subtitle file generation
- Drag-and-drop file transcription
- Native macOS-style UI components
- Dark theme interface
- System tray integration
- Cross-platform desktop application

## Architecture

- **Frontend**: React 18 + TypeScript + Vite
- **Desktop**: Tauri v2 (Rust-based)
- **Backend**: Python FastAPI sidecar
- **AI Models**: ONNX Runtime with Whisper/Parakeet models
- **UI Components**: Custom macOS-native component library

## Prerequisites

### Required Software

- **Node.js**: Version 20.19+ or 22.12+ (current: 20.17.0 with warnings)
- **Python**: Version 3.8+ with pip
- **Rust**: Latest stable toolchain via rustup
- **Git**: For cloning and version control

### System Requirements

- **Windows**: Windows 10/11 (x64)
- **macOS**: macOS 10.15+ (Intel/Apple Silicon)
- **Linux**: Ubuntu 18.04+ or equivalent
- **Memory**: 4GB RAM minimum, 8GB recommended
- **Storage**: 2GB free space for models and dependencies

## Installation

### 1. Clone Repository

```bash
git clone <repository-url>
cd asrpro
```

### 2. Install Node.js Dependencies

```bash
npm install
```

### 3. Install Python Dependencies

```bash
cd sidecar
pip install -r requirements.txt
cd ..
```

### 4. Install Rust Toolchain

```bash
# Windows/macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Or use the installer directly
# Windows: Download from https://win.rustup.rs/
```

### 5. Download AI Models

The application will automatically download required ONNX models on first run:
- Whisper-tiny (39MB)
- Whisper-base (74MB)
- Additional models available through UI

## Development

### Start Development Server

```bash
# Terminal 1: Start Python sidecar
cd sidecar
python main.py

# Terminal 2: Start Tauri development
npm run tauri dev
```

### Available Scripts

```bash
# Frontend development
npm run dev          # Start Vite dev server
npm run build        # Build frontend for production
npm run preview      # Preview production build

# Tauri desktop
npm run tauri dev    # Start Tauri development mode
npm run tauri build  # Build desktop application
```

### Project Structure

```
asrpro/
├── src/                    # React frontend source
│   ├── components/         # React components
│   │   ├── macos/         # macOS-native UI components
│   │   └── ...
│   ├── pages/             # Application pages
│   └── ...
├── src-tauri/             # Tauri configuration and Rust code
├── sidecar/               # Python backend
│   ├── api/              # FastAPI server
│   ├── models/           # AI model management
│   ├── utils/            # Utilities
│   └── tests/            # Backend tests
└── dist/                 # Built frontend assets
```

## Testing

### Backend Tests

```bash
cd sidecar
python -m pytest
python -m pytest -v                    # Verbose output
python -m pytest tests/test_api.py     # Specific test file
```

### Frontend Tests

```bash
npm run test            # Run frontend tests (if configured)
npm run type-check      # TypeScript type checking
```

### Clean Python Cache

```bash
# Remove all __pycache__ directories and .pyc files
python scripts/clean_python_cache.py

# Or manually with PowerShell (Windows)
Get-ChildItem -Path . -Name "__pycache__" -Recurse -Directory | Remove-Item -Recurse -Force
```

### Manual Testing

1. Start development servers
2. Test global hotkey functionality
3. Upload audio files for transcription
4. Verify model switching
5. Test system tray integration

## Building for Production

### Desktop Application

```bash
# Build complete desktop application
npm run tauri build
```

**Output files:**
- Windows: `src-tauri/target/release/bundle/nsis/asrpro_0.1.0_x64-setup.exe`
- Windows: `src-tauri/target/release/bundle/msi/asrpro_0.1.0_x64_en-US.msi`
- macOS: `src-tauri/target/release/bundle/macos/asrpro.app`
- Linux: `src-tauri/target/release/bundle/appimage/asrpro_0.1.0_amd64.AppImage`

### Frontend Only

```bash
npm run build
# Output: dist/ directory
```

## Configuration

### Environment Variables

Create `.env` files as needed:

```bash
# .env.local (frontend)
VITE_API_URL=http://localhost:8000

# sidecar/.env (backend)
MODEL_CACHE_DIR=./models
LOG_LEVEL=INFO
```

### Model Configuration

Models are automatically managed through the UI. Manual configuration in `sidecar/models/registry.py`.

## Troubleshooting

### Common Issues

**Node.js Version Warning**
- Current version 20.17.0 works but upgrade to 20.19+ recommended
- Use nvm/volta for Node.js version management

**Rust Installation**
- Ensure `~/.cargo/bin` is in PATH
- Restart terminal after installation
- Run `rustc --version` to verify

**Python Dependencies**
- Use virtual environment: `python -m venv venv && source venv/bin/activate`
- Install Microsoft Visual C++ Build Tools (Windows)
- Install system dependencies for audio processing

**Build Failures**
- Clear node_modules: `rm -rf node_modules && npm install`
- Clear Rust cache: `cargo clean` in src-tauri/
- Verify all prerequisites are installed

### Performance

- Models download automatically but can be pre-cached
- First transcription may be slower due to model loading
- GPU acceleration available with compatible hardware

## Contributing

1. Fork the repository
2. Create feature branch: `git checkout -b feature-name`
3. Clean Python cache: `python scripts/clean_python_cache.py`
4. Run tests: `npm run test && cd sidecar && python -m pytest`
5. Commit changes: `git commit -m "Description"`
6. Push and create pull request

### Repository Cleanliness

- Python cache files (`__pycache__/`, `*.pyc`) are automatically ignored
- Run cleanup script before committing: `python scripts/clean_python_cache.py`
- Virtual environments (`.venv/`, `venv/`) are ignored
- Node modules and build artifacts are ignored

## License

See LICENSE file for details.

## Author

Made by Suraj Mandal