# ASR Pro GTK4 Frontend for Linux

A modern, native GTK4 frontend for ASR Pro that provides a professional speech transcription experience on Linux. This application integrates seamlessly with the ASR Pro backend to deliver high-quality audio transcription using state-of-the-art AI models.

![ASR Pro GTK4 Frontend](docs/images/main_window_screenshot.png)

## Table of Contents

- [Features](#features)
- [System Requirements](#system-requirements)
- [Installation](#installation)
  - [System Dependencies](#system-dependencies)
  - [Building from Source](#building-from-source)
  - [Installation Scripts](#installation-scripts)
- [Getting Started](#getting-started)
  - [Launching the Application](#launching-the-application)
  - [Basic Usage](#basic-usage)
  - [Keyboard Shortcuts](#keyboard-shortcuts)
- [Features Overview](#features-overview)
  - [File Management](#file-management)
  - [Model Selection](#model-selection)
  - [Transcription Process](#transcription-process)
  - [Settings and Configuration](#settings-and-configuration)
- [Advanced Usage](#advanced-usage)
  - [Command Line Options](#command-line-options)
  - [Protocol Handler](#protocol-handler)
  - [Batch Processing](#batch-processing)
- [Troubleshooting](#troubleshooting)
  - [Common Issues](#common-issues)
  - [Performance Issues](#performance-issues)
  - [Backend Connection Issues](#backend-connection-issues)
- [FAQ](#faq)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Native GTK4 Interface**: Modern, responsive UI that integrates perfectly with your Linux desktop
- **Multi-format Audio Support**: Support for MP3, WAV, FLAC, OGG, M4A, and more
- **Real-time Progress Tracking**: Live updates during transcription with detailed progress information
- **Multiple AI Models**: Support for Whisper and other transcription models with different sizes and capabilities
- **Language Detection**: Automatic language detection with manual override options
- **Batch Processing**: Transcribe multiple files in sequence
- **Dark/Light Theme**: Automatic theme detection with manual override
- **System Integration**: Desktop notifications, file associations, and system tray support
- **Keyboard Shortcuts**: Productivity-focused keyboard shortcuts for all common actions
- **Project Management**: Save and load transcription projects
- **Export Options**: Export transcriptions in multiple formats (TXT, SRT, VTT, JSON)

## System Requirements

### Minimum Requirements
- **Operating System**: Ubuntu 20.04+, Fedora 35+, Arch Linux, or equivalent
- **Processor**: x86_64 or ARM64 with 2+ cores
- **Memory**: 4GB RAM
- **Storage**: 500MB free space for the application
- **Network**: Internet connection for model downloads

### Recommended Requirements
- **Operating System**: Latest Ubuntu, Fedora, or Arch Linux
- **Processor**: x86_64 or ARM64 with 4+ cores
- **Memory**: 8GB RAM or more
- **Storage**: 2GB free space for application and models
- **Graphics**: GPU acceleration support (NVIDIA CUDA or AMD ROCm)

### Backend Requirements
- **Docker**: Latest version for running AI model containers
- **Python**: 3.8+ for the backend sidecar
- **Network**: Local network access to backend API

## Installation

### System Dependencies

Before building the application, you need to install the required system dependencies:

#### Ubuntu/Debian
```bash
sudo apt update
sudo apt install -y \
    build-essential \
    meson \
    libgtk-4-dev \
    libjson-glib-dev \
    libsoup-3.0-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    pkg-config \
    libssl-dev
```

#### Fedora
```bash
sudo dnf install -y \
    meson \
    gtk4-devel \
    json-glib-devel \
    libsoup3-devel \
    gstreamer1-devel \
    gstreamer1-plugins-base-devel \
    pkg-config \
    openssl-devel
```

#### Arch Linux
```bash
sudo pacman -S --needed \
    meson \
    gtk4 \
    json-glib \
    libsoup \
    gstreamer \
    pkgconf \
    openssl
```

#### openSUSE
```bash
sudo zypper install -y \
    meson \
    gtk4-devel \
    json-glib-devel \
    libsoup-3_0-devel \
    gstreamer-devel \
    gstreamer-plugins-base-devel \
    pkg-config \
    libopenssl-devel
```

### Building from Source

1. **Install Rust**
   If you don't have Rust installed, use the official installer:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Clone the Repository**
   ```bash
   git clone https://github.com/your-org/asrpro.git
   cd asrpro/frontends/linux
   ```

3. **Configure and Build**
   ```bash
   # Create build directory
   meson setup builddir

   # Compile the application
   meson compile -C builddir

   # Optional: Install system-wide
   sudo meson install -C builddir
   ```

4. **Verify Installation**
   ```bash
   # Run from build directory
   ./builddir/asrpro-gtk4 --version

   # Or if installed system-wide
   asrpro-gtk4 --version
   ```

### Installation Scripts

For easier installation, use the provided scripts:

#### System-wide Installation
```bash
sudo ./scripts/install.sh
```

This script will:
- Build the application in release mode
- Install the executable to `/usr/local/bin/`
- Install desktop entry and MIME types
- Install application icons
- Update system databases

#### User Installation
```bash
./scripts/install-user.sh
```

This installs the application only for the current user without requiring root privileges.

## Getting Started

### Launching the Application

You can launch ASR Pro in several ways:

1. **From Application Menu**: Find "ASR Pro" in your desktop's application menu
2. **From Command Line**:
   ```bash
   asrpro-gtk4
   ```
3. **With Files**:
   ```bash
   asrpro-gtk4 /path/to/audio1.mp3 /path/to/audio2.wav
   ```

### Basic Usage

1. **Start the Backend**
   Make sure the ASR Pro backend is running:
   ```bash
   # From the project root
   make run.api
   ```

2. **Launch the Frontend**
   Start the GTK4 application as described above.

3. **Add Audio Files**
   - Drag and drop audio files onto the application
   - Use File → Open or Ctrl+O
   - Use the "Add Files" button in the file panel

4. **Select a Model**
   - Choose from available models in the model panel
   - Models will be downloaded automatically if not available

5. **Start Transcription**
   - Select one or more files
   - Click "Transcribe" or press Ctrl+T
   - Monitor progress in the transcription panel

6. **Review and Export**
   - Review transcriptions in the text view
   - Edit if necessary
   - Export to your preferred format

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Open file(s) |
| `Ctrl+N` | New project |
| `Ctrl+S` | Save project |
| `Ctrl+Shift+S` | Save project as |
| `Ctrl+T` | Start transcription |
| `Ctrl+.` | Stop transcription |
| `Ctrl+Shift+E` | Export results |
| `Ctrl+,` | Open settings |
| `F11` | Toggle fullscreen |
| `Ctrl+Q` | Quit application |
| `Ctrl+1` | Switch to Files tab |
| `Ctrl+2` | Switch to Models tab |
| `Ctrl+3` | Switch to Transcription tab |
| `Ctrl+Shift+L` | Clear file list |
| `Ctrl+Shift+R` | Refresh model list |

## Features Overview

### File Management

#### Adding Files
- **Drag and Drop**: Simply drag audio files onto the application window
- **File Dialog**: Use the standard GTK file picker to select files
- **Command Line**: Pass files as command line arguments
- **Recent Files**: Quick access to recently processed files

#### Supported Formats
- **Audio**: MP3, WAV, FLAC, OGG, M4A, AAC, WMA
- **Video**: MP4, AVI, MKV, MOV (audio extraction)

#### File Information
The application displays detailed information for each file:
- Duration and file size
- Audio format and codec
- Sample rate and channels
- Bit rate (if available)

### Model Selection

#### Available Models
- **Whisper Models**: tiny, base, small, medium, large-v3
- **Specialized Models**: multilingual, English-only
- **Custom Models**: Support for custom-trained models

#### Model Information
For each model, the application shows:
- Model size and download size
- Supported languages
- Performance characteristics
- Recommended use cases

#### Model Management
- **Automatic Downloads**: Models are downloaded on first use
- **Manual Downloads**: Pre-download models for offline use
- **Model Updates**: Check for and apply model updates
- **Storage Management**: View and manage model storage usage

### Transcription Process

#### Configuration Options
- **Language Selection**: Auto-detect or manually specify
- **Timestamp Options**: Include word-level or segment-level timestamps
- **Output Format**: Plain text, formatted text, or raw JSON
- **Quality Settings**: Balance between speed and accuracy

#### Progress Tracking
- **Real-time Updates**: Live progress information
- **Stage Indicators**: Shows current processing stage
- **Time Estimates**: Estimated completion time
- **Performance Metrics**: Processing speed and accuracy

#### Results Management
- **Text Editing**: Built-in text editor with search and replace
- **Timestamp Navigation**: Jump to specific timestamps
- **Confidence Scores**: View confidence for each segment
- **Export Options**: Multiple export formats

### Settings and Configuration

#### General Settings
- **Theme**: Light, dark, or system default
- **Language**: Interface language selection
- **Auto-save**: Automatic project saving
- **Backup**: Create backup copies of projects

#### Audio Settings
- **Default Format**: Preferred audio format
- **Quality Settings**: Default quality for new transcriptions
- **Preprocessing**: Audio normalization and filtering options

#### Backend Settings
- **Connection**: Backend server URL and port
- **Authentication**: API key configuration
- **Timeouts**: Request timeout settings
- **Retries**: Automatic retry configuration

#### Advanced Settings
- **Performance**: Thread pool size and memory limits
- **Logging**: Log level and output configuration
- **Experimental**: Feature flags for experimental features

## Advanced Usage

### Command Line Options

```bash
Usage: asrpro-gtk4 [OPTIONS] [FILES...]

Options:
  -h, --help              Show help message
  -v, --version           Show version information
  -n, --new               Start with a new project
  -f, --file-selector     Open file selector on startup
  -b, --backend <URL>     Specify backend URL (default: http://localhost:8000)
  -m, --model <MODEL>     Specify default model
  -l, --language <LANG>   Specify default language
  -d, --debug             Enable debug logging
  --config <PATH>         Use custom config file
  --project <PATH>        Open project file
  --no-gui                Run in CLI mode (experimental)

Examples:
  # Launch with default settings
  asrpro-gtk4

  # Transcribe specific files
  asrpro-gtk4 audio1.mp3 audio2.wav

  # Launch with custom backend
  asrpro-gtk4 --backend http://192.168.1.100:8000

  # Start with specific model and language
  asrpro-gtk4 --model whisper-large --language en

  # Open existing project
  asrpro-gtk4 --project /path/to/project.asrproj
```

### Protocol Handler

ASR Pro supports custom protocol URLs for integration with other applications:

```bash
# Transcribe a file directly
asrpro://transcribe?file=/path/to/audio.mp3&model=whisper-base&language=en

# Open a project
asrpro://open?project=/path/to/project.asrproj

# Start with specific configuration
asrpro://new?model=whisper-large&language=auto&timestamps=true
```

### Batch Processing

For processing multiple files, you can:

1. **Use the Batch Mode**: Select multiple files and enable batch processing
2. **Create a Project**: Add all files to a project and process them together
3. **Command Line**: Pass multiple files as arguments:
   ```bash
   asrpro-gtk4 --model whisper-base file1.mp3 file2.wav file3.flac
   ```

## Troubleshooting

### Common Issues

#### Application Won't Start
```bash
# Check if all dependencies are installed
ldd ./builddir/asrpro-gtk4

# Check GTK4 installation
pkg-config --modversion gtk4

# Run with debug output
G_MESSAGES_DEBUG=all ./builddir/asrpro-gtk4 --debug
```

#### Missing Icons
```bash
# Update icon cache
gtk-update-icon-cache -f -t ~/.local/share/icons/hicolor/
# or for system-wide installation
sudo gtk-update-icon-cache -f -t /usr/share/icons/hicolor/
```

#### Audio Files Not Supported
```bash
# Check GStreamer plugins
gst-inspect-1.0 | grep -i mp3

# Install additional codecs
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly
```

### Performance Issues

#### Slow Transcription
- Check if a more appropriate model is being used
- Ensure sufficient RAM is available
- Consider using GPU acceleration if available
- Close other applications that might be using resources

#### High Memory Usage
- Limit the number of concurrent transcriptions
- Use smaller models for longer files
- Clear the file list when not needed
- Restart the application periodically

#### UI Responsiveness
- Transcription happens in background threads, but the UI might lag with very large files
- Consider breaking large files into smaller segments
- Check system resource usage

### Backend Connection Issues

#### Connection Refused
```bash
# Check if backend is running
curl http://localhost:8000/health

# Check backend logs
journalctl -u asrpro-backend

# Restart backend
make run.api
```

#### Slow Response
- Check network connection to backend
- Verify backend has sufficient resources
- Check backend logs for errors
- Consider increasing timeouts in settings

#### Authentication Issues
- Verify API key is correct
- Check if backend requires authentication
- Ensure URL is correct (http vs https)

## FAQ

### Q: Can I use ASR Pro without an internet connection?
A: Yes, once the required models are downloaded, ASR Pro can work completely offline. Make sure to download the models you need before going offline.

### Q: How accurate is the transcription?
A: Accuracy depends on the model used and audio quality. The large-v3 Whisper model typically achieves 95%+ accuracy for clear speech.

### Q: Can I transcribe video files?
A: Yes, ASR Pro can extract audio from video files and transcribe them. Supported formats include MP4, AVI, MKV, and MOV.

### Q: How do I add custom models?
A: Place custom model files in the models directory and refresh the model list. See the documentation for model format requirements.

### Q: Is my data sent to external servers?
A: No, all processing happens locally on your machine or your configured backend. No data is sent to external servers.

### Q: Can I use ASR Pro commercially?
A: Yes, ASR Pro is released under a permissive license that allows commercial use. See the LICENSE file for details.

### Q: How do I report bugs or request features?
A: Please use the GitHub issue tracker to report bugs or request features. Include as much detail as possible.

### Q: Can I contribute to the project?
A: Yes, contributions are welcome! See the CONTRIBUTING.md file for guidelines on how to contribute.

### Q: Where are transcriptions saved?
A: By default, projects are saved in `~/Documents/ASRPro/`. You can change this in the settings.

### Q: How do I uninstall ASR Pro?
A: Use the provided uninstall script:
```bash
sudo ./scripts/uninstall.sh
```

## Contributing

We welcome contributions to ASR Pro! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:

- Reporting issues
- Submitting pull requests
- Coding standards
- Testing guidelines
- Documentation improvements

### Development Setup

1. Clone the repository
2. Install dependencies as described above
3. Build in debug mode:
   ```bash
   meson setup builddir --buildtype=debug
   meson compile -C builddir
   ```
4. Run tests:
   ```bash
   meson test -C builddir
   ```

## License

ASR Pro is released under the MIT License. See [LICENSE](LICENSE) for details.

## Support

- **Documentation**: [Full documentation](https://asrpro.docs)
- **Issues**: [GitHub Issue Tracker](https://github.com/your-org/asrpro/issues)
- **Discussions**: [GitHub Discussions](https://github.com/your-org/asrpro/discussions)
- **Email**: support@asrpro.org

---

**ASR Pro** - Professional speech transcription for Linux