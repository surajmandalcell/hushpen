## Model Loaders and Adding New Models

The sidecar uses a DRY loader pattern with a central registry:

- Add your loader class in `models.py` inheriting from `ONNXBaseLoader`.
- Implement `_get_model_name()` to return either a string or a list of candidate model IDs. Prefer quantized variants first (e.g., `*_q4`, `*_q8`) to enable faster inference. Example:

```python
class MyModelLoader(ONNXBaseLoader):
    def _get_model_name(self):
        return ["my-model_q4", "my-model_q8", "my-model"]
```

- Register the model once in `models.py` `ModelRegistry._initialize_models()` with a unique `id` and a `loader` key matching your loader mapping key.
- Map the loader key to your class in the `loader_map` inside `ModelManager._get_loader`.

This minimizes steps to add/remove a model.

## Backend Priority and Fallback

- Device detection selects backends in order: MPS (macOS) > CUDA > Vulkan > CPU.
- Loaders attempt GPU-first (MPS/CUDA) and fall back to CPU automatically.
- Performance benchmarking runs for detected, CPU, MPS, CUDA, and Vulkan (when available) for apples-to-apples comparisons.

## Quantized Models

- Whisper and Parakeet loaders prefer quantized variants (q4/q8) if available. If a quantized model is not present, the loader falls back to the default fp16 model.
- When downloading quantized assets manually, name them with suffixes `_q4` or `_q8` to be picked up automatically.

## Using Hub vs Local ONNX Models

- Hub (default): Models like `whisper-tiny` and `whisper-base` resolve via the `onnx_asr` hub using their IDs.
- Local files: Use `*-local` variants registered in `models/registry.py`:
  - `whisper-tiny-local` → loads from `models/onnx/whisper-tiny/`
  - `whisper-base-local` → loads from `models/onnx/whisper-base/`

The loader resolves candidates as on-disk paths when `source: "file"`.
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
