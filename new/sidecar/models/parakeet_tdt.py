"""
ONNX Parakeet TDT model loader for ASR Pro Python Sidecar
"""

from .base import ONNXBaseLoader


class ParakeetTDTLoader(ONNXBaseLoader):
    """ONNX loader for Parakeet TDT model."""

    def _get_model_name(self):
        """Return candidates including quantized variants when available."""
        return [
            "nemo-parakeet-tdt-0.6b-v2_q4",
            "nemo-parakeet-tdt-0.6b-v2_q8",
            "nemo-parakeet-tdt-0.6b-v2",
        ]
