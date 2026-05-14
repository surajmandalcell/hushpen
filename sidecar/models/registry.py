"""
Model registry metadata and helpers.
"""

from typing import Dict, Any, Optional, List

DISABLED_PLACEHOLDER_REASON = "Future placeholder"
PARAKEET_V3_ID = "parakeet-tdt-0.6b-v3"


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
                "enabled": False,
                "disabled_reason": DISABLED_PLACEHOLDER_REASON,
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
                "enabled": False,
                "disabled_reason": DISABLED_PLACEHOLDER_REASON,
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
                "enabled": False,
                "disabled_reason": DISABLED_PLACEHOLDER_REASON,
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
                "enabled": False,
                "disabled_reason": DISABLED_PLACEHOLDER_REASON,
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
                "enabled": False,
                "disabled_reason": DISABLED_PLACEHOLDER_REASON,
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
            PARAKEET_V3_ID: {
                "id": PARAKEET_V3_ID,
                "name": "Parakeet-TDT-0.6B-v3",
                "enabled": True,
                "disabled_reason": None,
                "description": "NVIDIA Parakeet TDT 0.6B v3 multilingual ASR model via NeMo",
                "type": "nemo",
                "family": "parakeet",
                "size": "0.6b",
                "loader": "nemo",
                "languages": [
                    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de",
                    "el", "hu", "it", "lv", "lt", "mt", "pl", "pt", "ro", "sk",
                    "sl", "es", "sv", "ru", "uk",
                ],
                "sample_rate": 16000,
                "repo": "nvidia/parakeet-tdt-0.6b-v3",
                "candidates": ["nvidia/parakeet-tdt-0.6b-v3"],
                "source": "huggingface",
            },
        }

    def list_models(self, include_disabled: bool = False) -> List[str]:
        return [
            model_id
            for model_id, info in self._models.items()
            if include_disabled or info.get("enabled", True)
        ]

    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        return self._models.get(model_id)

    def is_model_available(self, model_id: str) -> bool:
        return self.is_model_enabled(model_id)

    def is_model_enabled(self, model_id: str) -> bool:
        model_info = self._models.get(model_id)
        return bool(model_info and model_info.get("enabled", True))

    def get_disabled_reason(self, model_id: str) -> Optional[str]:
        model_info = self._models.get(model_id)
        if not model_info or self.is_model_enabled(model_id):
            return None
        return model_info.get("disabled_reason") or "Disabled"

    def get_loader_type(self, model_id: str) -> Optional[str]:
        model_info = self._models.get(model_id)
        if not model_info or not self.is_model_enabled(model_id):
            return None
        configured_loader = model_info.get("loader")
        if configured_loader and configured_loader != "config":
            return configured_loader
        family = model_info.get("family")
        if family == "whisper":
            return "whisper"
        elif family == "parakeet":
            return "parakeet"
        return model_info.get("loader")

    def get_models_by_type(self, model_type: str, include_disabled: bool = False) -> List[str]:
        """Get models by family type."""
        return [
            model_id for model_id, info in self._models.items()
            if info.get("family") == model_type
            and (include_disabled or info.get("enabled", True))
        ]

    def get_models_by_language(self, language: str, include_disabled: bool = False) -> List[str]:
        """Get models that support a specific language."""
        return [
            model_id for model_id, info in self._models.items()
            if language in info.get("languages", [])
            and (include_disabled or info.get("enabled", True))
        ]
