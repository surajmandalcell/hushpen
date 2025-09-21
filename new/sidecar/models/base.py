"""
Base loader for ASR Pro Python Sidecar
"""

from abc import ABC, abstractmethod
from typing import Dict, Any, BinaryIO


class BaseLoader(ABC):
    """Base class for model loaders."""

    def __init__(self, model_id: str, config: Dict[str, Any]):
        self.model_id = model_id
        self.config = config
        self.is_loaded = False
        self.model = None

    @abstractmethod
    async def load(self) -> bool:
        """Load the model."""
        pass

    @abstractmethod
    async def unload(self) -> bool:
        """Unload the model."""
        pass

    @abstractmethod
    async def transcribe(self, audio_file: BinaryIO) -> Dict[str, Any]:
        """Transcribe audio file."""
        pass

    def is_ready(self) -> bool:
        """Check if the model is ready."""
        return self.is_loaded

    def get_model_info(self) -> Dict[str, Any]:
        """Get model information."""
        return {
            "model_id": self.model_id,
            "is_loaded": self.is_loaded,
            "config": self.config,
        }
