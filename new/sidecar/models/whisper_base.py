"""
ONNX Whisper Base model loader for ASR Pro Python Sidecar
"""

from .base import ONNXBaseLoader


class WhisperBaseLoader(ONNXBaseLoader):
    """ONNX loader for Whisper Base model."""

    def _get_model_name(self):
        """Return candidate names for whisper family (quantized first where applicable)."""
        mapping = {
            "whisper-tiny": ["whisper-tiny_q4", "whisper-tiny_q8", "whisper-tiny"],
            "whisper-base": ["whisper-base_q4", "whisper-base_q8", "whisper-base"],
            "whisper-small": ["whisper-small_q4", "whisper-small_q8", "whisper-small"],
            "whisper-medium": [
                "whisper-medium_q4",
                "whisper-medium_q8",
                "whisper-medium",
            ],
            "whisper-large": ["whisper-large_q4", "whisper-large_q8", "whisper-large"],
        }
        return mapping.get(
            self.model_id, ["whisper-base_q4", "whisper-base_q8", "whisper-base"]
        )
