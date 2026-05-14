# ASR Pro

<p>
  <img src="./src/assets/asrpro-logo.svg" alt="ASR Pro logo" width="96" height="96" />
</p>

A professional desktop application for AI-powered speech recognition and transcription, built with Electron + React + Vite and a Python sidecar.

## Features

- Global hotkey transcription with customizable key combinations
- Real-time audio processing and transcription
- Multi-model AI support (Whisper, Parakeet)
- SRT subtitle file generation
- Drag-and-drop file transcription
- Cross-platform Electron shell for macOS, Windows, and Linux
- Seamless macOS-style titlebar and sidebar shared across platforms
- Native-feeling grouped desktop UI with local-first transcription workflows
- Secure preload bridge for file selection, platform info, and window actions

## Architecture

- **Frontend**: React 19 + TypeScript + Vite
- **Desktop**: Electron with context-isolated preload APIs
- **Backend**: Python FastAPI sidecar
- **AI Models**: ONNX Runtime with Whisper/Parakeet models
- **UI Components**: Custom cross-platform desktop UI using Tailwind and lucide icons

## Brand Assets

| Asset | Path | Use |
|---|---|---|
| App logo SVG | `src/assets/asrpro-logo.svg` | README, about page, docs, reusable square logo |
| App logo PNG | `src/assets/asrpro-logo.png` | Raster previews and external surfaces |
| Logo mark SVG | `src/assets/asrpro-logo-mark.svg` | Transparent D07 infinity mark |
| Dark tray SVG/PNG | `src/assets/asrpro-tray-dark.svg`, `src/assets/asrpro-tray-dark.png` | Dark glyph for light tray/menu backgrounds |
| Light tray SVG/PNG | `src/assets/asrpro-tray-light.svg`, `src/assets/asrpro-tray-light.png` | Light glyph for dark tray/menu backgrounds |
| Packaged icons | `src-tauri/icons/` | macOS, Windows, and Linux Electron packaging; retained from the legacy Tauri tree |

## Supported Desktop Runtime

| Runtime | Status | Notes |
|---|---|---|
| Electron | Supported | Canonical desktop shell for macOS, Windows, and Linux. Packaged installs start the Python sidecar automatically. |
| Tauri | Unsupported | Legacy files may remain for icons or historical context, but Tauri is not a maintained release path until it is explicitly revived and `cargo check` passes. |

## Prerequisites

### Required Software

- **Node.js**: Version 20.19+ or 22.12+ (current: 20.17.0 with warnings)
- **Python**: Version 3.8+ with pip for development and sidecar packaging
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

### 3. Install Python Dependencies For Development

```bash
cd sidecar
python3 -m venv .venv
. .venv/bin/activate
pip install -r requirements.txt
cd ..
```

Production installers do not require end users to install Python manually. The release build bundles a platform-specific sidecar executable under Electron `extraResources`.

### 4. Download AI Models

The application will automatically download required ONNX models on first run:
- Whisper-base (default local model)
- Whisper-tiny
- Additional models available through UI

Parakeet/NeMo models are optional and require the extra NeMo dependencies listed in `sidecar/requirements.txt`.

## Development

### Start Development Server

```bash
# Starts the Python sidecar, Vite, and Electron together
npm run electron:dev
```

### Available Scripts

```bash
# Frontend development
npm run dev          # Start Vite dev server on 127.0.0.1:4270
npm run sidecar:dev  # Start Python sidecar on 127.0.0.1:8000
npm run sidecar:build # Build the platform-specific sidecar executable
npm run sidecar:check # Verify the sidecar executable exists for packaging
npm run build        # Typecheck and build renderer assets
npm run preview      # Preview production renderer on 127.0.0.1:4271
npm test             # Run renderer interaction tests

# Electron desktop
npm run electron:dev   # Run the desktop app with the Vite dev server
npm run electron:pack  # Build unpacked desktop app for the current OS
npm run electron:dist  # Build configured installers/packages
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
├── electron/              # Electron main and preload processes
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
npm test                # Run renderer interaction tests
npm run build           # TypeScript build verification
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

Build the sidecar executable on the target OS before creating production installers. Windows packages need `sidecar/bin/asrpro-sidecar.exe`; macOS and Linux packages need `sidecar/bin/asrpro-sidecar`.

```bash
# Build the Python sidecar executable for the current OS
npm run sidecar:build

# Verify the production sidecar executable is present
npm run sidecar:check

# Build renderer and an unpacked Electron app for the current OS
npm run electron:pack

# Build configured installers/packages for the current OS
npm run electron:dist
```

**Output files:**
- Current OS unpacked app: `release/<platform>-<arch>/`
- macOS configured targets: DMG and ZIP
- Windows configured targets: NSIS installer and portable executable
- Linux configured targets: AppImage and DEB

### Packaged Runtime Behavior

| Platform | Sidecar startup | Data directory |
|---|---|---|
| macOS | Electron starts the bundled sidecar from app resources when no healthy sidecar is already running. | App resource `data/` directory, matching the existing macOS release behavior. |
| Windows | Electron starts `resources/sidecar/bin/asrpro-sidecar.exe` and waits for `/health`. | Electron user-writable app data under the current user's profile. |
| Linux | Electron starts `resources/sidecar/bin/asrpro-sidecar` and waits for `/health`. | Electron user-writable app data under the current user's profile. |

Development keeps local state in `tmp/app-data` and can use either `npm run electron:dev` or the Electron main process fallback that starts `sidecar/main.py`.

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
VITE_ASRPRO_API_URL=http://localhost:8000

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

**Python Dependencies**
- Use virtual environment: `python -m venv venv && source venv/bin/activate`
- Install Microsoft Visual C++ Build Tools (Windows)
- Install system dependencies for audio processing

**Build Failures**
- Clear node_modules: `rm -rf node_modules && npm install`
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
