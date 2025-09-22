"""
Model registry for ASR Pro Python Sidecar
"""

from typing import Dict, List, Any, Optional
import logging

logger = logging.getLogger(__name__)


class ModelRegistry:
    """Registry for available models."""

    def __init__(self):
        self._models = self._initialize_models()

    def _initialize_models(self) -> Dict[str, Dict[str, Any]]:
        """Initialize available models - focused on English and Hindi using ONNX."""
        return {
            "whisper-base": {
                "id": "whisper-base",
                "name": "Whisper Base (ONNX)",
                "description": "OpenAI Whisper base model (74M parameters) - English/Hindi - ONNX",
                "type": "whisper",
                "size": "base",
                "loader": "whisper_onnx",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
            "parakeet-tdt-0.6b-v2": {
                "id": "parakeet-tdt-0.6b-v2",
                "name": "Parakeet TDT 0.6B v2 (ONNX)",
                "description": "NVIDIA Parakeet TDT model (0.6B parameters) - English/Hindi - ONNX",
                "type": "parakeet",
                "size": "0.6b",
                "loader": "parakeet",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
        }

    def list_models(self) -> List[str]:
        """List all available model IDs."""
        return list(self._models.keys())

    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        """Get model information by ID."""
        return self._models.get(model_id)

    def get_models_by_type(self, model_type: str) -> List[Dict[str, Any]]:
        """Get all models of a specific type."""
        return [
            info for info in self._models.values() if info.get("type") == model_type
        ]

    def get_models_by_language(self, language: str) -> List[Dict[str, Any]]:
        """Get all models that support a specific language."""
        return [
            info
            for info in self._models.values()
            if language in info.get("languages", [])
        ]

    def is_model_available(self, model_id: str) -> bool:
        """Check if a model is available."""
        return model_id in self._models

    def get_loader_type(self, model_id: str) -> Optional[str]:
        """Get the loader type for a model."""
        model_info = self._models.get(model_id)
        return model_info.get("loader") if model_info else None
