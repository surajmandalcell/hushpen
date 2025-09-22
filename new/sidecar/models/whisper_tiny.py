"""
ONNX Whisper Tiny model loader for ASR Pro Python Sidecar
"""

from .base import ONNXBaseLoader


class WhisperTinyLoader(ONNXBaseLoader):
    """ONNX loader for Whisper Tiny model."""

    def _get_model_name(self):
        """Return preferred model identifiers (quantized first) for onnx-asr."""
        # Prefer quantized variants for speed if available, fallback to fp16 default
        return [
            "whisper-tiny_q4",
            "whisper-tiny_q8",
            "whisper-tiny",
        ]
