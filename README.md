# ASR Pro

A professional application suite for AI-powered speech recognition and transcription, built with native frontends and Python backend sidecar architecture.

## Features

- Global hotkey transcription with customizable key combinations
- Real-time audio processing and transcription
- Multi-model AI support (Whisper, Parakeet)
- SRT subtitle file generation
- Drag-and-drop file transcription
- Native platform-specific UI components
- Dark theme interface
- System tray integration
- Cross-platform support

## Architecture

- **Backend**: Python FastAPI sidecar server
- **Frontends**:
  - GTK4 for Linux
  - SwiftUI for macOS
  - Windows UI for Windows
- **AI Models**: Docker-based Whisper/Parakeet models
- **Communication**: REST API + WebSocket

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

### Prerequisites

- **Python**: Version 3.8+ with pip
- **Docker**: For AI model containers (optional but recommended)
- **Git**: For cloning and version control

### 1. Clone Repository

```bash
git clone <repository-url>
cd asrpro
```

### 2. Start the Backend Sidecar

```bash
# Using the Makefile (recommended)
make run.api

# Or for development with hot reload:
make dev.api

# Or manually:
cd sidecar
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
python main.py
```

The backend will start on `http://0.0.0.0:8000` by default.

### 3. Configure Your Native Frontend

Each frontend platform (GTK4, SwiftUI, Windows) should connect to the backend API:
- API Base URL: `http://localhost:8000`
- API Documentation: `http://localhost:8000/docs`
- Health Check: `http://localhost:8000/health`

## Frontend Platforms

### Linux (GTK4)
```bash
cd frontends/gtk4
meson setup builddir
meson compile -C builddir
./builddir/src/asrpro
```

### macOS (SwiftUI)
```bash
cd frontends/swiftui
open ASRPro.xcodeproj
# Build and run in Xcode
```

### Windows (Windows UI)
```bash
cd frontends/windows
# Open ASRPro.sln in Visual Studio
# Build and run the solution
```

## Backend Configuration

The backend can be configured by editing `config.json` in the root directory:

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8000
  },
  "docker": {
    "container_timeout": 30,
    "max_concurrent_containers": 3
  },
  "models": {
    "default": "whisper-base",
    "auto_download": true
  }
}
```

## Development

### Start Backend Only

```bash
# Production mode
make run.api

# Development mode with hot reload
make dev.api
```

### Backend API Endpoints

- `GET /health` - Health check endpoint
- `GET /v1/models` - List available AI models
- `POST /v1/settings/model` - Set active model
- `POST /v1/audio/transcriptions` - Transcribe audio files
- `GET /v1/options` - Get API configuration options
- `WS /ws` - WebSocket for real-time updates

### Project Structure

```
asrpro/
├── frontends/              # Native frontend applications
│   ├── gtk4/              # Linux GTK4 frontend
│   ├── swiftui/           # macOS SwiftUI frontend
│   └── windows/           # Windows UI frontend
├── sidecar/               # Python backend
│   ├── api/              # FastAPI server
│   ├── config/           # Configuration management
│   ├── docker/           # Docker integration
│   ├── utils/            # Utilities
│   └── tests/            # Backend tests
├── scripts/              # Utility scripts
├── config.json           # Backend configuration file
├── Makefile              # Build and run targets
├── docker-compose.yml    # Docker service configuration
└── README.md
```

## Testing

### Backend Tests

```bash
# Using the Makefile (recommended)
make test.backend

# Or manually:
cd sidecar
python -m pytest
python -m pytest -v                    # Verbose output
python -m pytest tests/test_api.py     # Specific test file
```

### Clean Python Cache

```bash
# Remove all __pycache__ directories and .pyc files
python scripts/clean_python_cache.py

# Or manually with PowerShell (Windows)
Get-ChildItem -Path . -Name "__pycache__" -Recurse -Directory | Remove-Item -Recurse -Force
```

### Manual Testing

1. Start the backend server: `make run.api`
2. Verify health check: `curl http://localhost:8000/health`
3. Test API endpoints: `curl http://localhost:8000/v1/models`
4. Connect your native frontend application
5. Test transcription with audio files

## Configuration

### Backend Configuration

Edit `config.json` in the root directory to customize backend settings:

```json
{
  "server": {
    "host": "0.0.0.0",
    "port": 8000
  },
  "docker": {
    "container_timeout": 30,
    "max_concurrent_containers": 3,
    "pull_policy": "if_missing"
  },
  "models": {
    "default": "whisper-base",
    "auto_download": true,
    "cache_dir": "./models"
  },
  "logging": {
    "level": "INFO"
  }
}
```

### Frontend Configuration

Each native frontend should be configured to connect to:
- API Base URL: `http://localhost:8000`
- WebSocket URL: `ws://localhost:8000/ws`

## Troubleshooting

### Common Issues

**Backend won't start**
- Check Python version: `python3 --version` (requires 3.8+)
- Verify Docker is installed and running: `docker --version`
- Check port availability: `netstat -an | grep 8000`

**Docker-related errors**
- Install Docker Desktop for your platform
- Verify Docker daemon is running
- Check Docker logs for container issues

**Connection refused from frontend**
- Ensure backend is running: `make run.api`
- Check firewall settings
- Verify host configuration in config.json

**Performance issues**
- Models download automatically on first use
- Ensure sufficient RAM (8GB+ recommended)
- GPU acceleration requires NVIDIA Docker runtime

### API Debugging

```bash
# Check backend health
curl http://localhost:8000/health

# List available models
curl http://localhost:8000/v1/models

# View API documentation
open http://localhost:8000/docs
```

## Contributing

1. Fork the repository
2. Create feature branch: `git checkout -b feature-name`
3. Clean Python cache: `make clean`
4. Run tests: `make test.backend`
5. Commit changes: `git commit -m "Description"`
6. Push and create pull request

### Repository Cleanliness

- Python cache files (`__pycache__/`, `*.pyc`) are automatically ignored
- Run cleanup script before committing: `make clean`
- Virtual environments (`.venv/`, `venv/`) are ignored
- Node modules and build artifacts are ignored

## License

See LICENSE file for details.

## Author

Made by Suraj Mandal