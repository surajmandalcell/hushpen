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
        """Initialize available models."""
        return {
            "whisper-tiny": {
                "id": "whisper-tiny",
                "name": "Whisper Tiny",
                "description": "OpenAI Whisper tiny model (39M parameters)",
                "type": "whisper",
                "size": "tiny",
                "loader": "whisper",
                "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
                "sample_rate": 16000
            },
            "whisper-base": {
                "id": "whisper-base",
                "name": "Whisper Base",
                "description": "OpenAI Whisper base model (74M parameters)",
                "type": "whisper",
                "size": "base",
                "loader": "whisper",
                "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
                "sample_rate": 16000
            },
            "whisper-small": {
                "id": "whisper-small",
                "name": "Whisper Small",
                "description": "OpenAI Whisper small model (244M parameters)",
                "type": "whisper",
                "size": "small",
                "loader": "whisper",
                "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
                "sample_rate": 16000
            },
            "whisper-medium": {
                "id": "whisper-medium",
                "name": "Whisper Medium",
                "description": "OpenAI Whisper medium model (769M parameters)",
                "type": "whisper",
                "size": "medium",
                "loader": "whisper",
                "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
                "sample_rate": 16000
            },
            "whisper-large": {
                "id": "whisper-large",
                "name": "Whisper Large",
                "description": "OpenAI Whisper large model (1550M parameters)",
                "type": "whisper",
                "size": "large",
                "loader": "whisper",
                "languages": ["en", "es", "fr", "de", "it", "pt", "ru", "ja", "ko", "zh"],
                "sample_rate": 16000
            },
            "parakeet-ctc": {
                "id": "parakeet-ctc",
                "name": "Parakeet CTC",
                "description": "NVIDIA Parakeet CTC model (1.1B parameters)",
                "type": "parakeet",
                "variant": "ctc",
                "loader": "parakeet",
                "languages": ["en"],
                "sample_rate": 16000
            },
            "parakeet-rnnt": {
                "id": "parakeet-rnnt",
                "name": "Parakeet RNNT",
                "description": "NVIDIA Parakeet RNNT model (1.1B parameters)",
                "type": "parakeet",
                "variant": "rnnt",
                "loader": "parakeet",
                "languages": ["en"],
                "sample_rate": 16000
            },
            "parakeet-transducer": {
                "id": "parakeet-transducer",
                "name": "Parakeet Transducer",
                "description": "NVIDIA Parakeet Transducer model (1.1B parameters)",
                "type": "parakeet",
                "variant": "transducer",
                "loader": "parakeet",
                "languages": ["en"],
                "sample_rate": 16000
            }
        }
    
    def list_models(self) -> List[str]:
        """List all available model IDs."""
        return list(self._models.keys())
    
    def get_model_info(self, model_id: str) -> Optional[Dict[str, Any]]:
        """Get model information by ID."""
        return self._models.get(model_id)
    
    def get_models_by_type(self, model_type: str) -> List[Dict[str, Any]]:
        """Get all models of a specific type."""
        return [info for info in self._models.values() if info.get("type") == model_type]
    
    def get_models_by_language(self, language: str) -> List[Dict[str, Any]]:
        """Get all models that support a specific language."""
        return [info for info in self._models.values() if language in info.get("languages", [])]
    
    def is_model_available(self, model_id: str) -> bool:
        """Check if a model is available."""
        return model_id in self._models
    
    def get_loader_type(self, model_id: str) -> Optional[str]:
        """Get the loader type for a model."""
        model_info = self._models.get(model_id)
        return model_info.get("loader") if model_info else None
