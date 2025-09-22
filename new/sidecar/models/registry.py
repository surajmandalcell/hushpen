"""
Model registry metadata and helpers.
"""

from typing import Dict, Any, Optional, List


class ModelRegistry:
    """Registry for available models."""

    def __init__(self):
        self._models = self._initialize_models()

    def _initialize_models(self) -> Dict[str, Dict[str, Any]]:
        return {
            "whisper-tiny": {
                "id": "whisper-tiny",
                "name": "Whisper Tiny (ONNX)",
                "description": "OpenAI Whisper tiny model - fast & lightweight - ONNX",
                "type": "whisper",
                "size": "tiny",
                "loader": "whisper",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
            "whisper-base": {
                "id": "whisper-base",
                "name": "Whisper Base (ONNX)",
                "description": "OpenAI Whisper base model (74M parameters) - English/Hindi - ONNX",
                "type": "whisper",
                "size": "base",
                "loader": "whisper",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
            "parakeet-tdt-0.6b-v2": {
                "id": "parakeet-tdt-0.6b-v2",
                "name": "Parakeet TDT 0.6B v2 (ONNX)",
                "description": "NVIDIA Parakeet TDT model (0.6B parameters) - English/Hindi - ONNX",
                "type": "parakeet",
                "size": "0.6b",
                "loader": "parakeet_tdt",
                "languages": ["en", "hi"],
                "sample_rate": 16000,
            },
        }

    def list_models(self) -> List[str]:
        return list(self._models.keys())

    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        return self._models.get(model_id)

    def is_model_available(self, model_id: str) -> bool:
        return model_id in self._models

    def get_loader_type(self, model_id: str) -> Optional[str]:
        model_info = self._models.get(model_id)
        return model_info.get("loader") if model_info else None
