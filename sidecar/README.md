## Model Loaders and Adding New Models

The ASR engine uses a DRY loader pattern with a central registry:

- Add your loader class in `models.py` inheriting from `ONNXBaseLoader`.
- Implement `_get_model_name()` to return either a string or a list of candidate model IDs. Prefer quantized variants first (e.g., `*_q4`, `*_q8`) to enable faster inference. Example:

```python
class MyModelLoader(ONNXBaseLoader):
    def _get_model_name(self):
        return ["my-model_q4", "my-model_q8", "my-model"]
```

- Register the model once in `models/registry.py` with a unique `id` and a `loader` key matching your loader mapping key.
- Map the loader key to your class in the `loader_map` inside `ModelManager._get_loader`.

This minimizes steps to add/remove a model.

## Backend Priority and Fallback

- Device detection selects backends in order: MPS (macOS) > CUDA > Vulkan > CPU.
- Loaders attempt GPU-first (MPS/CUDA) and fall back to CPU automatically.
- Performance benchmarking runs for detected, CPU, MPS, CUDA, and Vulkan (when available) for apples-to-apples comparisons.

## Quantized Models

- Parakeet-TDT-0.6B-v3 is the only enabled model for now. Whisper metadata remains disabled as a future placeholder.
- ONNX placeholder loaders can prefer quantized variants (q4/q8) when re-enabled later.
- When downloading quantized assets manually, name them with suffixes `_q4` or `_q8` to be picked up automatically.

## Using Hub vs Local ONNX Models

- Hub: Parakeet-TDT-0.6B-v3 resolves through NVIDIA NeMo from `nvidia/parakeet-tdt-0.6b-v3`.
- Disabled local placeholders remain registered in `models/registry.py` for future Whisper work:
  - `whisper-tiny-local` maps to the future `models/onnx/whisper-tiny/` location.
  - `whisper-base-local` maps to the future `models/onnx/whisper-base/` location.

Disabled placeholder entries are not exposed in the active model list and cannot be loaded until they are explicitly re-enabled.
# ASR Pro Python ASR Engine

This is the Python ASR engine for the ASR Pro Electron + React application.

## Purpose

The engine provides:
- Parakeet-TDT-0.6B-v3 model management
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

Packaged Electron builds should bundle a platform-specific engine executable instead of requiring end users to install Python:

```bash
npm run sidecar:build
npm run sidecar:check
```

The generated executable is written to `sidecar/bin/` and is intentionally ignored by git.

## API Endpoints

- `GET /v1/models` - List available models
- `POST /v1/audio/transcriptions` - Transcribe audio files
- `POST /v1/settings/model` - Set active model
- `GET /health` - Health check
