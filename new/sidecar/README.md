# ASR Pro Python Sidecar

This is the Python sidecar for the ASR Pro Tauri+React application.

## Purpose

The sidecar provides:
- AI model management (Whisper, Parakeet)
- Audio processing and recording
- HTTP API for frontend communication
- Real-time transcription services

## Setup

```bash
cd sidecar
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
```

## Running

```bash
python main.py
```

## API Endpoints

- `GET /v1/models` - List available models
- `POST /v1/audio/transcriptions` - Transcribe audio files
- `POST /v1/settings/model` - Set active model
- `GET /health` - Health check
