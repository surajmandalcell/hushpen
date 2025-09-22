"""
Model registry metadata and helpers.
"""

from typing import Dict, Any, Optional, List


class ModelRegistry:
    """Registry for available models."""

    def __init__(self):
        self._models = self._initialize_models()

    def _initialize_models(self) -> Dict[str, Dict[str, Any]]:
        # Single source of truth: define all model metadata and candidate names here.
        # Each model supports either hub-loaded identifiers (candidates) or local file paths by setting source="file".
        return {
            "whisper-tiny": {
                "id": "whisper-tiny",
                "name": "Whisper Tiny (ONNX)",
                "description": "OpenAI Whisper tiny model - fast & lightweight - ONNX",
                "type": "onnx",
                "family": "whisper",
                "size": "tiny",
                "loader": "config",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
                "candidates": ["whisper-tiny_q4", "whisper-tiny_q8", "whisper-tiny"],
                "source": "hub",
            },
            # Local model folder present under models/onnx/whisper-tiny
            "whisper-tiny-local": {
                "id": "whisper-tiny-local",
                "name": "Whisper Tiny (Local ONNX)",
                "description": "Local ONNX files for Whisper tiny",
                "type": "onnx",
                "family": "whisper",
                "size": "tiny",
                "loader": "config",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
                "candidates": ["whisper-tiny"],  # Directory name under models/onnx/
                "source": "file",
            },
            "whisper-base": {
                "id": "whisper-base",
                "name": "Whisper Base (ONNX)",
                "description": "OpenAI Whisper base model (74M parameters) - English/Hindi - ONNX",
                "type": "onnx",
                "family": "whisper",
                "size": "base",
                "loader": "config",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
                "candidates": ["whisper-base_q4", "whisper-base_q8", "whisper-base"],
                "source": "hub",
            },
            "whisper-large": {
                "id": "whisper-large",
                "name": "Whisper Large (ONNX)",
                "description": "OpenAI Whisper large model - ONNX",
                "type": "onnx",
                "family": "whisper",
                "size": "large",
                "loader": "config",
                "languages": ["en"],
                "sample_rate": 16000,
                "candidates": ["whisper-large"],
                "source": "hub",
            },
            # Local file-based variant using models/onnx/whisper-base directory
            "whisper-base-local": {
                "id": "whisper-base-local",
                "name": "Whisper Base (Local ONNX)",
                "description": "Local ONNX files for Whisper base",
                "type": "onnx",
                "family": "whisper",
                "size": "base",
                "loader": "config",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
                "candidates": ["whisper-base"],  # Directory name under models/onnx/
                "source": "file",
            },
            "parakeet-tdt-0.6b-v2": {
                "id": "parakeet-tdt-0.6b-v2",
                "name": "Parakeet TDT 0.6B v2 (ONNX)",
                "description": "NVIDIA Parakeet TDT model (0.6B parameters) - English/Hindi - ONNX",
                "type": "onnx",
                "family": "parakeet",
                "size": "0.6b",
                "loader": "config",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
                "candidates": [
                    "nemo-parakeet-tdt-0.6b-v2_q4",
                    "nemo-parakeet-tdt-0.6b-v2_q8",
                    "nemo-parakeet-tdt-0.6b-v2",
                ],
                "source": "hub",
            },
            # Example of local ONNX models (place files under models/onnx/<name>)
            # "whisper-base-local": {
            #     "id": "whisper-base-local",
            #     "name": "Whisper Base (Local ONNX)",
            #     "description": "Local ONNX files for Whisper base",
            #     "type": "onnx",
            #     "family": "whisper",
            #     "size": "base",
            #     "loader": "config",
            #     "languages": ["en"],
            #     "sample_rate": 16000,
            #     "candidates": ["whisper-base/encoder_model.onnx", "whisper-base/decoder_model_merged.onnx"],
            #     "source": "file",
            # },
        }

    def list_models(self) -> List[str]:
        return list(self._models.keys())

    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        return self._models.get(model_id)

    def is_model_available(self, model_id: str) -> bool:
        return model_id in self._models

    def get_loader_type(self, model_id: str) -> Optional[str]:
        model_info = self._models.get(model_id)
        if not model_info:
            return None
        family = model_info.get("family")
        if family == "whisper":
            return "whisper"
        elif family == "parakeet":
            return "parakeet"
        return model_info.get("loader")

    def get_models_by_type(self, model_type: str) -> List[str]:
        """Get models by family type."""
        return [
            model_id for model_id, info in self._models.items()
            if info.get("family") == model_type
        ]

    def get_models_by_language(self, language: str) -> List[str]:
        """Get models that support a specific language."""
        return [
            model_id for model_id, info in self._models.items()
            if language in info.get("languages", [])
        ]
