# ASR Pro GTK4 Frontend

Native GTK4 frontend for Linux that communicates with the ASR Pro backend via REST API.

## Dependencies

### System Dependencies
- GTK4 development libraries
- JSON-GLib development libraries
- libsoup3 development libraries
- Meson build system
- Rust compiler and Cargo

### Rust Dependencies
- gtk4
- gio
- glib
- reqwest
- serde_json
- tokio

## Installation

### Ubuntu/Debian
```bash
sudo apt update
sudo apt install build-essential meson libgtk-4-dev libjson-glib-dev libsoup-3.0-dev
```

### Fedora
```bash
sudo dnf install meson gtk4-devel json-glib-devel libsoup3-devel
```

### Arch Linux
```bash
sudo pacman -S meson gtk4 json-glib libsoup
```

### Rust
If you don't have Rust installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

## Building

1. Navigate to the GTK4 frontend directory:
```bash
cd frontends/gtk4
```

2. Create a build directory:
```bash
mkdir build
cd build
```

3. Configure the project with Meson:
```bash
meson setup
```

4. Compile the project:
```bash
ninja
```

## Running

After building, you can run the application from the build directory:
```bash
./asrpro-gtk4
```

## Usage

1. Make sure the ASR Pro backend is running (default at http://localhost:8000)
2. Launch the GTK4 frontend
3. Click the "Transcribe" button to test the connection to the backend
4. The application will display the backend health status in the text area

## Backend Configuration

The frontend is configured to connect to the backend at `http://localhost:8000` by default. If your backend is running at a different location, you'll need to modify the `BACKEND_URL` constant in `src/main.rs`.

## Features

- Basic GTK4 interface with a transcribe button
- Text area to display results
- HTTP client for backend communication
- Basic error handling
- Simple styling with CSS

## Development

To modify the application:

1. Edit the source files in the `src/` directory
2. Edit style settings in `data/style.css`
3. Rebuild with `ninja` in the build directory

## Troubleshooting

- If you get build errors related to missing dependencies, make sure all system dependencies are installed
- If the application fails to connect to the backend, verify that the backend is running and accessible
- For GTK4-related issues, consult the GTK4 documentation